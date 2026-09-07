//! 心跳线程：周期发 0x0a；连续失败累计 → 推送 Reconnecting。
//!
//! 协议 §5：固件侧仅应答，不主动发心跳；App 自行定时（建议 1~3s）。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use crate::link::LinkEvent;
use crate::protocol::{CMD_HEARTBEAT, Frame};

/// 心跳间隔（可由 UI 配置；阶段 04 先硬编码 1s）
pub const HEARTBEAT_INTERVAL_MS: u64 = 1000;
/// 允许的最大无响应时间：3 个周期
pub const HEARTBEAT_TIMEOUT_MULTIPLIER: u32 = 3;
/// 连接初期的宽限时长：USB-CDC 打开端口可能触发设备复位重启（约 1~2s），
/// 期间心跳必然无响应，不做 Reconnecting 判定，避免"刚连上就变重连"。
pub const HEARTBEAT_BOOT_GRACE_MS: u64 = 4000;

#[derive(Clone)]
pub struct HeartbeatHandle {
    /// 最近一次心跳响应的 seq（用于在 reader 线程外判断"刚收到过响应"）
    pub last_ack_seq: Arc<AtomicU64>,
    /// 最近一次成功响应时刻
    pub last_ack_at: Arc<std::sync::Mutex<Instant>>,
}

impl HeartbeatHandle {
    pub fn new() -> Self {
        Self {
            last_ack_seq: Arc::new(AtomicU64::new(0)),
            last_ack_at: Arc::new(std::sync::Mutex::new(Instant::now())),
        }
    }

    /// 收到 0x8a（CMD_HEARTBEAT 响应）时调用
    pub fn mark_ack(&self, seq: u64) {
        self.last_ack_seq.store(seq, Ordering::Relaxed);
        if let Ok(mut t) = self.last_ack_at.lock() {
            *t = Instant::now();
        }
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
        let started = Instant::now();
        let mut seq: u32 = 1;
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            // 发送心跳
            let frame = Frame::request(CMD_HEARTBEAT, seq, None);
            if write_tx.send(WriterMsg::Frame(frame)).is_err() {
                return; // writer 已退出
            }
            seq = seq.wrapping_add(1);

            // 等待一个周期
            thread::sleep(interval);

            // 检查超时（启动宽限期内跳过：设备可能正在复位重启）
            if started.elapsed() >= grace && handle.last_ack_age() > timeout {
                let _ = link_tx.send(LinkEvent::State(crate::link::ConnectionState::Reconnecting));
                // 退避：等到至少收到一次 ack 才继续高频探测
                while handle.last_ack_age() > timeout {
                    if stop_flag.load(Ordering::Relaxed) {
                        return;
                    }
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
