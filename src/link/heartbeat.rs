//! 心跳线程：周期发 0x0a；连续失败累计 → 推送 Reconnecting。
//!
//! 协议 §5：固件侧仅应答，不主动发心跳；App 自行定时（建议 1~3s）。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use crate::link::LinkEvent;
use crate::protocol::{CMD_HEARTBEAT, Frame};
use crate::state::LogKind;
use crate::util::log::SharedLog;
/// 心跳间隔
pub const HEARTBEAT_INTERVAL_MS: u64 = 2000;
/// 允许的最大无响应时间：3 个周期
pub const HEARTBEAT_TIMEOUT_MULTIPLIER: u32 = 3;
/// 心跳日志聚合窗口。每帧心跳仍照常发，但只在累计达到该阈值时记一条
/// 汇总日志（"心跳 Tx×60 …"），避免长连接场景下每秒两条心跳日志把
/// 共享缓冲挤满（5000 条 ring buffer 在 1Hz 心跳下 ~42 分钟就被心跳
/// 刷光，用户真正关心的连接/协议事件反而被淘汰）。
pub const HEARTBEAT_LOG_BATCH: u32 = 60;
/// 宽限期：从**收到首次心跳 ack** 起算的稳定窗口（4 s）。
///
/// 协议 §2.1：USB CDC 打开端口会触发设备短暂复位（约 1~2s）。原实现把宽限期
/// 起点钉在"心跳线程启动时刻"，但 ESP32-S3 实际复位完成时间不确定，4 s
/// 可能不够。改成"首次 ack 后再宽限 N 秒"，能稳定覆盖任何复位窗口。
///
/// 配套有绝对上限 [`HEARTBEAT_BOOT_ABSOLUTE_MAX_MS`]，防止设备永远不应答
/// 时 App 永远停在 Connecting/Online 假象里。
pub const HEARTBEAT_BOOT_GRACE_MS: u64 = 4000;
/// 启动后最长等待首次 ack 的时间；超过即强制判定为 Reconnecting。
pub const HEARTBEAT_BOOT_ABSOLUTE_MAX_MS: u64 = 6_000;

#[derive(Clone)]
pub struct HeartbeatHandle {
    /// 最近一次心跳响应的 seq（用于在 reader 线程外判断"刚收到过响应"）
    pub last_ack_seq: Arc<AtomicU64>,
    /// 最近一次成功响应时刻
    pub last_ack_at: Arc<std::sync::Mutex<Instant>>,
    /// 是否至少收到过一次心跳 ack
    pub first_ack_seen: Arc<AtomicBool>,
}

impl HeartbeatHandle {
    pub fn new() -> Self {
        Self {
            last_ack_seq: Arc::new(AtomicU64::new(0)),
            last_ack_at: Arc::new(std::sync::Mutex::new(Instant::now())),
            first_ack_seen: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 收到 0x8a（CMD_HEARTBEAT 响应）时调用
    pub fn mark_ack(&self, seq: u64) {
        self.last_ack_seq.store(seq, Ordering::Relaxed);
        if let Ok(mut t) = self.last_ack_at.lock() {
            *t = Instant::now();
        }
        // 只在首次 ack 时翻转，避免每次写都要 atomic 同步
        self.first_ack_seen.store(true, Ordering::Relaxed);
    }

    pub fn last_ack_age(&self) -> Duration {
        self.last_ack_at
            .lock()
            .map(|t| t.elapsed())
            .unwrap_or(Duration::from_secs(u64::MAX))
    }
}

/// 后台心跳线程入口
pub fn spawn(
    write_tx: Sender<WriterMsg>,
    handle: HeartbeatHandle,
    link_tx: Sender<LinkEvent>,
    log: SharedLog,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        log.push(LogKind::App, "heartbeat 线程启动".to_string());
        let interval = Duration::from_millis(HEARTBEAT_INTERVAL_MS);
        let timeout = interval * HEARTBEAT_TIMEOUT_MULTIPLIER;
        let grace = Duration::from_millis(HEARTBEAT_BOOT_GRACE_MS);
        let absolute_max = Duration::from_millis(HEARTBEAT_BOOT_ABSOLUTE_MAX_MS);
        let started = Instant::now();
        let mut seq: u32 = 1;
        // 自上次写心跳 Tx 汇总日志以来累计发送的心跳帧数。
        // 达到 HEARTBEAT_LOG_BATCH 时合并成一条 "心跳 Tx×N (last seq=X)"，
        // 避免每帧一条 Tx 风暴；需要逐帧抓包时把阈值临时改 1 即可。
        // `#[allow(unused_assignments)]`：变量实际在循环内被读（format!），
        // 但 borrow checker 跨 while 闭包看不到使用点，标记消除误报。
        #[allow(unused_assignments)]
        let mut tx_batch: u32 = 0;

        // 闭包：发一个心跳帧；writer 退出时返回 false 让外层线程退出。
        // 把发送逻辑抽出是为了让退避循环也能继续探测（而不是纯 sleep）。
        let send_one = |s: u32| -> bool {
            let frame = Frame::request(CMD_HEARTBEAT, s, None);
            write_tx.send(WriterMsg::Frame(frame)).is_ok()
        };

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            // 发送心跳
            if !send_one(seq) {
                // writer 已退出前先 flush 一次汇总，避免最后一段心跳 Tx 没记录
                // （这条只在异常路径下出现，常见情况下 writer 退出意味着连接
                //  断开，外层 detach_link 已经写过 "已断开" 日志）
                if tx_batch > 0 {
                    log.push(
                        LogKind::Tx,
                        format!("心跳 Tx×{tx_batch} (last seq={})", seq.wrapping_sub(1)),
                    );
                }
                return;
            }
            seq = seq.wrapping_add(1);
            tx_batch = tx_batch.saturating_add(1);
            // 达到聚合窗口：把累计 N 帧合并成一条 Tx 汇总日志。
            // 写完后立刻把上次已 ack 的 seq 范围一并带上，便于面板里看
            // "最近一次心跳 seq"，不需要再展开逐条日志。
            if tx_batch >= HEARTBEAT_LOG_BATCH {
                log.push(
                    LogKind::Tx,
                    format!("心跳 Tx×{tx_batch} (last seq={})", seq.wrapping_sub(1)),
                );
                tx_batch = 0;
            }

            // 等待一个周期
            thread::sleep(interval);

            // 宽限期语义：
            //   - 启动后未收到过任何 ack：处于"等待首次 ack"阶段，
            //     不做 Reconnecting 判定（设备可能正在复位）。
            //     但有 `absolute_max` 兜底：超过该时间仍无 ack，按无应答处理。
            //   - 收到过 ack：进入正常超时判定，但首次 ack 后再宽限
            //     `grace` 秒，给协议栈 / UI 一个稳定窗口，避免刚连上就误判。
            let age = handle.last_ack_age();
            let in_grace = if handle.first_ack_seen.load(Ordering::Relaxed) {
                // 首次 ack 后：再宽限 grace 秒
                age <= grace
            } else {
                // 首次 ack 前：宽限到 absolute_max 秒
                started.elapsed() < absolute_max
            };

            if !in_grace && age > timeout {
                let _ = link_tx.send(LinkEvent::State(crate::link::ConnectionState::Reconnecting));
                log.push(
                    LogKind::App,
                    format!(
                        "心跳超时 (age={:?}, timeout={timeout:?})，进入 Reconnecting",
                        age
                    ),
                );
                // 退避：每个周期继续发心跳探测，直到收到一次 ack。
                // 旧实现只在循环里 sleep，等不到 ack 就一直卡在 Reconnecting。
                #[allow(unused_assignments)]
                while handle.last_ack_age() > timeout {
                    if stop_flag.load(Ordering::Relaxed) {
                        return;
                    }
                    if !send_one(seq) {
                        // writer 退出：先把尚未 flush 的心跳 Tx 批次写出来
                        if tx_batch > 0 {
                            log.push(
                                LogKind::Tx,
                                format!("心跳 Tx×{tx_batch} (last seq={})", seq.wrapping_sub(1)),
                            );
                            tx_batch = 0;
                        }
                        return; // writer 已退出
                    }
                    seq = seq.wrapping_add(1);
                    tx_batch = tx_batch.saturating_add(1);
                    if tx_batch >= HEARTBEAT_LOG_BATCH {
                        log.push(
                            LogKind::Tx,
                            format!("心跳 Tx×{tx_batch} (last seq={})", seq.wrapping_sub(1)),
                        );
                        tx_batch = 0;
                    }
                    thread::sleep(interval);
                }
                let _ = link_tx.send(LinkEvent::State(crate::link::ConnectionState::Online));
                log.push(LogKind::App, "心跳恢复，重连为 Online".to_string());
            }
        }
    })
}

/// writer 线程的命令
#[derive(Debug)]
pub enum WriterMsg {
    Frame(Frame),
    Shutdown,
}
