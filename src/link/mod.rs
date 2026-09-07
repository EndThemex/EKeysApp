//! Link 层：串口 + 读写线程 + 状态机 + 心跳。
//!
//! 这是协议层与硬件之间的唯一桥梁。UI 通过 `LinkManager` 发请求、订阅事件。
//!
//! 线程模型：
//! - reader 线程：`Arc<Mutex<Box<dyn SerialPort>>>` 共享，循环 read + 行缓冲
//! - writer 线程：消费 `WriterMsg`，按需 flush
//! - heartbeat 线程：周期发 0x0a；超时降级 Reconnecting

pub mod heartbeat;
pub mod reader;
pub mod serial;

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::protocol::{self, DeviceSettings, Frame};
use heartbeat::{HeartbeatHandle, WriterMsg};
pub use serial::PortInfo;

/// 连接状态机
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Online,
    Reconnecting,
    Error(String),
}

impl ConnectionState {
    pub fn is_online(&self) -> bool {
        matches!(self, ConnectionState::Online)
    }
}

/// Link 层对外事件
#[derive(Debug, Clone)]
pub enum LinkEvent {
    /// 协议帧（响应或 seq=0 主动推送）
    Frame(Frame),
    /// 固件日志原文
    LogLine(String),
    /// 状态变化
    State(ConnectionState),
    /// 错误信息
    Error(String),
}

/// seq → 等待该响应的 oneshot Sender
type PendingMap = Arc<Mutex<HashMap<u32, Sender<Frame>>>>;

/// reader 退出回调类型
type OnReaderExit = Arc<dyn Fn() + Send + Sync>;

/// LinkManager：持有串口（共享）+ 三个后台线程
pub struct LinkManager {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    events_rx_slot: Option<Receiver<LinkEvent>>,
    events_tx: Sender<LinkEvent>,
    write_tx: Sender<WriterMsg>,
    pending: PendingMap,
    state: Arc<Mutex<ConnectionState>>,
    hb: HeartbeatHandle,
    stop: Arc<AtomicBool>,
    port_name: String,
    on_reader_exit: OnReaderExit,
    _reader: Option<JoinHandle<()>>,
    _writer: Option<JoinHandle<()>>,
    _heartbeat: Option<JoinHandle<()>>,
}

impl LinkManager {
    /// 枚举端口
    pub fn list_ports() -> Vec<PortInfo> {
        serial::list_ports()
    }

    /// 打开端口并启动后台线程
    pub fn open(name: &str, on_reader_exit: OnReaderExit) -> Result<Self, String> {
        let port = serial::open(name).map_err(|e| format!("打开串口失败: {e}"))?;
        Self::from_port(name.to_string(), port, on_reader_exit)
    }

    fn from_port(
        name: String,
        port: Box<dyn serialport::SerialPort>,
        on_reader_exit: OnReaderExit,
    ) -> Result<Self, String> {
        let (event_tx, event_rx) = channel::<LinkEvent>();
        let (write_tx, write_rx) = channel::<WriterMsg>();
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let hb = HeartbeatHandle::new();
        let stop = Arc::new(AtomicBool::new(false));

        let port: Arc<Mutex<Box<dyn serialport::SerialPort>>> = Arc::new(Mutex::new(port));

        // reader：共享 port
        let port_for_reader = Arc::clone(&port);
        let tx_for_reader = event_tx.clone();
        let on_reader_exit_th = on_reader_exit.clone();
        let reader = thread::spawn(move || {
            reader::run_shared(port_for_reader, tx_for_reader.clone());
            let _ = tx_for_reader.send(LinkEvent::State(ConnectionState::Disconnected));
            // reader 异常退出 → 调用 on_reader_exit 回调（由 AppHandle 注入）
            on_reader_exit_th();
        });

        // writer：共享 port
        let port_for_writer = Arc::clone(&port);
        let writer = thread::spawn(move || {
            loop {
                match write_rx.recv() {
                    Ok(WriterMsg::Frame(frame)) => {
                        // 序列化失败则跳过发送，避免把垃圾帧推给固件
                        let s = match frame.encode_line() {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!("帧序列化失败，跳过发送: {e}");
                                continue;
                            }
                        };
                        if let Ok(mut p) = port_for_writer.lock() {
                            if let Err(e) = p.write_all(s.as_bytes()) {
                                tracing::warn!("串口写入失败: {e}");
                            }
                            let _ = p.flush();
                        }
                    }
                    Ok(WriterMsg::Shutdown) | Err(_) => break,
                }
            }
        });

        // 初始状态序列
        let _ = event_tx.send(LinkEvent::State(ConnectionState::Connecting));
        let _ = event_tx.send(LinkEvent::State(ConnectionState::Online));

        Ok(Self {
            port,
            events_rx_slot: Some(event_rx),
            events_tx: event_tx,
            write_tx,
            pending,
            state: Arc::new(Mutex::new(ConnectionState::Online)),
            hb,
            stop,
            port_name: name,
            on_reader_exit,
            _reader: Some(reader),
            _writer: Some(writer),
            _heartbeat: None,
        })
    }

    /// 启动心跳
    pub fn start_heartbeat(&mut self) {
        if self._heartbeat.is_some() {
            return;
        }
        let write_tx = self.write_tx.clone();
        let link_tx = self.events_tx.clone();
        let handle = self.hb.clone();
        let stop = Arc::clone(&self.stop);
        self._heartbeat = Some(heartbeat::spawn(write_tx, handle, link_tx, stop));
    }

    /// 关闭
    pub fn close(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.write_tx.send(WriterMsg::Shutdown);
        if let Some(h) = self._heartbeat.take() {
            let _ = h.join();
        }
        if let Some(h) = self._writer.take() {
            let _ = h.join();
        }
        if let Some(h) = self._reader.take() {
            let _ = h.join();
        }
        *self.state.lock().unwrap() = ConnectionState::Disconnected;
    }

    /// 当前状态
    pub fn state(&self) -> ConnectionState {
        self.state.lock().unwrap().clone()
    }

    /// 订阅事件占位 —— 实际订阅发生在 attach 时（见 AppHandle::attach_link）。
    /// 保留 API 以兼容未来扩展。
    pub fn _subscribe_compat() {}

    /// 取走内部的 events_rx（一次性；attach 时使用）
    pub fn take_events(&mut self) -> Receiver<LinkEvent> {
        std::mem::replace(&mut self.events_rx_slot, None).expect("events_rx already consumed")
    }

    /// 当前连接的端口名
    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    /// 直接发一帧（不等响应）
    pub fn send(&self, frame: Frame) {
        if self.write_tx.send(WriterMsg::Frame(frame)).is_err() {
            tracing::warn!("writer 线程已退出");
        }
    }

    /// 请求-响应：seq 自增 + 配对 + 超时
    pub fn request(
        &self,
        cmd: u8,
        data: Option<serde_json::Value>,
        timeout: Duration,
    ) -> Result<Frame, String> {
        let seq = self.next_seq();
        let (tx, rx) = channel::<Frame>();
        self.pending.lock().unwrap().insert(seq, tx);
        let frame = Frame::request(cmd, seq, data);
        self.send(frame);

        match rx.recv_timeout(timeout) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                self.pending.lock().unwrap().remove(&seq);
                Err(format!("请求超时: {e}"))
            }
        }
    }

    /// 拉一批事件并在内部完成 seq 配对与状态更新。
    /// UI 每帧调用一次。
    pub fn poll_events(&self) -> Vec<LinkEvent> {
        let mut out = Vec::new();
        let Some(rx) = self.events_rx_slot.as_ref() else {
            return out;
        };
        while let Ok(ev) = rx.try_recv() {
            match ev {
                LinkEvent::Frame(f) => {
                    // 心跳响应 → mark_ack
                    if f.cmd == protocol::response_cmd(protocol::CMD_HEARTBEAT) {
                        self.hb.mark_ack(f.seq as u64);
                    }

                    // 异类命令识别（body 在帧顶层，非 `data`）：
                    // - 0x10 Profile State：响应帧 cmd 仍是 0x10（不是 0x90）
                    // - 0x0C Voice Text：固件→App 推送，cmd 保持 0x0c
                    // - 0x0F Music Control：固件→App 推送，cmd 保持 0x0f
                    //
                    // 这些命令的响应/推送**不应**走 `is_response()` 判定（因为
                    // 0x10 / 0x0c / 0x0f 的最高位都是 0）。下面用 `is_top_level_cmd`
                    // 单独识别，再走"响应 + seq!=0 配对"或"seq=0 推送"两条路径。
                    let is_response_like = f.is_response()
                        || (f.cmd == protocol::CMD_PROFILE_STATE
                            && protocol::is_top_level_cmd(f.cmd));

                    // 配对响应：响应类命令且 seq != 0 且在 pending 表中
                    if is_response_like && f.seq != 0 {
                        if let Some(tx) = self.pending.lock().unwrap().remove(&f.seq) {
                            let _ = tx.send(f);
                            continue; // 已配对，不向 UI 推送
                        }
                    }
                    // seq=0 主动推送 / 未配对响应 → 交给 UI
                    out.push(LinkEvent::Frame(f));
                }
                LinkEvent::State(s) => {
                    *self.state.lock().unwrap() = s.clone();
                    out.push(LinkEvent::State(s));
                }
                other => out.push(other),
            }
        }
        out
    }

    fn next_seq(&self) -> u32 {
        static SEQ: AtomicU32 = AtomicU32::new(1);
        SEQ.fetch_add(1, Ordering::Relaxed)
    }
}

// 占位：让 DeviceSettings 被引用（避免未用警告；同时给 future 扩展保留位置）
#[allow(dead_code)]
fn _ensure_used(_: DeviceSettings) {}
