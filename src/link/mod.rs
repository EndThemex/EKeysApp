//! Link 层：串口 + 后台线程（reader / writer / router / heartbeat）+ 状态机。
//!
//! 这是协议层与硬件之间的唯一桥梁。UI 通过 `LinkManager` 发请求、每帧 `poll_events` 拉事件。
//!
//! 线程模型：
//! - reader 线程：`Arc<Mutex<Box<dyn SerialPort>>>` 共享，循环 read + 行缓冲
//! - writer 线程：消费 `WriterMsg`，按需 flush
//! - router 线程：消费内部事件流 → 心跳 ack 标记 + seq 响应配对 + 状态同步，转发 UI 事件
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

/// LinkManager：持有串口（共享）+ 后台线程（reader / writer / router / heartbeat）
pub struct LinkManager {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    /// UI 侧事件接收端（由 router 线程投递）；`poll_events` 每帧拉取
    ui_rx_slot: Option<Receiver<LinkEvent>>,
    /// 内部事件通道发送端（reader / heartbeat → router）
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
    _router: Option<JoinHandle<()>>,
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
        // 内部通道：reader / heartbeat → router
        let (event_tx, event_rx) = channel::<LinkEvent>();
        // UI 通道：router → poll_events
        let (ui_tx, ui_rx) = channel::<LinkEvent>();
        let (write_tx, write_rx) = channel::<WriterMsg>();
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let hb = HeartbeatHandle::new();
        let stop = Arc::new(AtomicBool::new(false));
        let state: Arc<Mutex<ConnectionState>> = Arc::new(Mutex::new(ConnectionState::Online));

        let port: Arc<Mutex<Box<dyn serialport::SerialPort>>> = Arc::new(Mutex::new(port));

        // reader：共享 port
        let port_for_reader = Arc::clone(&port);
        let tx_for_reader = event_tx.clone();
        let stop_for_reader = Arc::clone(&stop);
        let on_reader_exit_th = on_reader_exit.clone();
        let reader = thread::spawn(move || {
            reader::run_shared(port_for_reader, tx_for_reader.clone(), stop_for_reader);
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

        // router：持续消费内部事件流，独立于 UI 线程完成
        // 1) 心跳 ack 标记  2) seq 响应配对（request() 依赖，UI 阻塞等待时也能送达）
        // 3) 状态同步       4) 其余事件转发给 UI
        //
        // 注意：不能把配对放在 UI 线程的 poll_events 里——request() 会阻塞 UI
        // 线程等待响应，此时无人配对，所有请求必然超时。
        let pending_for_router = Arc::clone(&pending);
        let state_for_router = Arc::clone(&state);
        let hb_for_router = hb.clone();
        let router = thread::spawn(move || {
            loop {
                match event_rx.recv() {
                    Ok(LinkEvent::Frame(f)) => {
                        // 心跳响应 → mark_ack
                        if f.cmd == protocol::response_cmd(protocol::CMD_HEARTBEAT) {
                            hb_for_router.mark_ack(f.seq as u64);
                        }

                        // 异类命令识别（body 在帧顶层，非 `data`）：
                        // - 0x10 Profile State：响应帧 cmd 仍是 0x10（不是 0x90）
                        // - 0x0C Voice Text / 0x0F Music Control：固件→App 推送
                        let is_response_like = f.is_response()
                            || (f.cmd == protocol::CMD_PROFILE_STATE
                                && protocol::is_top_level_cmd(f.cmd));

                        // 配对响应：响应类命令且 seq != 0 且在 pending 表中
                        if is_response_like && f.seq != 0 {
                            if let Some(tx) = pending_for_router.lock().unwrap().remove(&f.seq) {
                                let _ = tx.send(f);
                                continue; // 已配对，不向 UI 转发
                            }
                        }
                        // seq=0 主动推送 / 未配对响应 → 交给 UI
                        let _ = ui_tx.send(LinkEvent::Frame(f));
                    }
                    Ok(LinkEvent::State(s)) => {
                        *state_for_router.lock().unwrap() = s.clone();
                        let _ = ui_tx.send(LinkEvent::State(s));
                    }
                    Ok(other) => {
                        let _ = ui_tx.send(other);
                    }
                    Err(_) => return, // 所有发送端已关闭（LinkManager 析构）
                }
            }
        });

        // 初始状态序列
        let _ = event_tx.send(LinkEvent::State(ConnectionState::Connecting));
        let _ = event_tx.send(LinkEvent::State(ConnectionState::Online));

        Ok(Self {
            port,
            ui_rx_slot: Some(ui_rx),
            events_tx: event_tx,
            write_tx,
            pending,
            state,
            hb,
            stop,
            port_name: name,
            on_reader_exit,
            _reader: Some(reader),
            _writer: Some(writer),
            _router: Some(router),
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

    /// 拉一批 UI 事件（router 线程已完成 seq 配对 / 心跳 ack / 状态同步）。
    /// UI 每帧调用一次。
    pub fn poll_events(&self) -> Vec<LinkEvent> {
        let mut out = Vec::new();
        let Some(rx) = self.ui_rx_slot.as_ref() else {
            return out;
        };
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
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
