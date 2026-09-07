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

/// 心跳间隔（可由 UI 配置；阶段 04 先硬编码 1s）
pub const HEARTBEAT_INTERVAL_MS: u64 = 1000;
/// 允许的最大无响应时间：3 个周期
pub const HEARTBEAT_TIMEOUT_MULTIPLIER: u32 = 3;
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
pub const HEARTBEAT_BOOT_ABSOLUTE_MAX_MS: u64 = 30_000;

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
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let interval = Duration::from_millis(HEARTBEAT_INTERVAL_MS);
        let timeout = interval * HEARTBEAT_TIMEOUT_MULTIPLIER;
        let grace = Duration::from_millis(HEARTBEAT_BOOT_GRACE_MS);
        let absolute_max = Duration::from_millis(HEARTBEAT_BOOT_ABSOLUTE_MAX_MS);
        let started = Instant::now();
        let mut seq: u32 = 1;

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
                return; // writer 已退出
            }
            seq = seq.wrapping_add(1);

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
                // 退避：每个周期继续发心跳探测，直到收到一次 ack。
                // 旧实现只在循环里 sleep，等不到 ack 就一直卡在 Reconnecting。
                while handle.last_ack_age() > timeout {
                    if stop_flag.load(Ordering::Relaxed) {
                        return;
                    }
                    if !send_one(seq) {
                        return; // writer 已退出
                    }
                    seq = seq.wrapping_add(1);
                    thread::sleep(interval);
                }
                let _ = link_tx.send(LinkEvent::State(crate::link::ConnectionState::Online));
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
