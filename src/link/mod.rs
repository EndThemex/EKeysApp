//! Link 层：串口 + 后台线程（reader / writer / router / heartbeat）+ 状态机。
//!
//! 这是协议层与硬件之间的唯一桥梁。UI 通过 `LinkManager` 发请求、每帧 `poll_events` 拉事件。
//!
//! 线程模型：
//! - reader 线程：`Arc<Mutex<Box<dyn SerialPort>>>` 共享，循环 read + 行缓冲
//! - writer 线程：消费 `WriterMsg`，按需 flush
//! - router 线程：消费内部事件流 → 心跳 ack 标记 + seq 响应配对 + 状态同步，转发 UI 事件
//! - heartbeat 线程：周期发 0x0a；超时降级 Reconnecting
//!
//! 退出语义：
//! - reader 退出 → router 收到 State(Disconnected) → 清空 pending map（让阻塞中的
//!   request() 立即收到 Disconnected，而不是等各自的 timeout）
//! - close()：先停心跳、关 writer、关 reader；router 此时收到 Disconnected 后会
//!   主动退出（pending 清空 → sender drop → event_rx.recv() 返回 Err），最后 join router

pub mod heartbeat;
pub mod reader;
pub mod serial;

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::protocol::{self, Frame, HeartbeatResp};
use crate::state::LogKind;
use crate::util::log::SharedLog;
use heartbeat::{HEARTBEAT_LOG_BATCH, HeartbeatHandle, WriterMsg};
pub use serial::PortInfo;

/// 连接状态机
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Online,
    Reconnecting,
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

/// Tx / Rx 计数 + uptime 起点（与 AppHandle 共享）。
/// 写在 LinkManager 上是为了让 writer / router 闭包能直接 clone 使用，
/// 避免在线程内部再做"延迟初始化"的二次锁。
type Counters = (Arc<AtomicU64>, Arc<AtomicU64>, Arc<Mutex<Option<Instant>>>);

/// LinkManager：持有串口（共享）+ 后台线程（reader / writer / router / heartbeat）
pub struct LinkManager {
    /// 共享日志缓冲；request() 失败时也会在这里推 App 日志。
    log: SharedLog,
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
    pub fn open(
        name: &str,
        log: SharedLog,
        on_reader_exit: OnReaderExit,
        counters: Counters,
    ) -> Result<Self, String> {
        let port = serial::open(name).map_err(|e| format!("打开串口失败: {e}"))?;
        Self::from_port(name.to_string(), port, log, on_reader_exit, counters)
    }

    fn from_port(
        name: String,
        port: Box<dyn serialport::SerialPort>,
        log: SharedLog,
        on_reader_exit: OnReaderExit,
        counters: Counters,
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

        let (tx_counter, rx_counter, _uptime_start) = counters;

        let port: Arc<Mutex<Box<dyn serialport::SerialPort>>> = Arc::new(Mutex::new(port));

        // reader：共享 port
        let port_for_reader = Arc::clone(&port);
        let tx_for_reader = event_tx.clone();
        let stop_for_reader = Arc::clone(&stop);
        let on_reader_exit_th = on_reader_exit.clone();
        let log_for_reader = log.clone();
        let reader = thread::spawn(move || {
            reader::run_shared(
                port_for_reader,
                tx_for_reader.clone(),
                stop_for_reader.clone(),
                log_for_reader,
            );
            let _ = tx_for_reader.send(LinkEvent::State(ConnectionState::Disconnected));
            // reader 退出后关闭 event_tx，让 router 的 recv() 返回 Err，
            // 从而 router 线程自然退出（这是 close() 流程能 join router 的关键）。
            // 注意：必须在 on_reader_exit 之前 drop 这个 sender —— 否则 router
            // 会等不到 close 信号一直挂着，close() 的 join 永远不会返回。
            drop(tx_for_reader);
            // 主动断开时 stop 已被 close() 置位，user 主动断开不应该被当"异常断开"
            // 处理（也就不会触发 "检测到 xxx 异常断开" 的兜底重连 Toast）；
            // 只有 reader 因为串口掉线 / 设备复位等异常原因退出时，stop 仍为 false，
            // 此时才调用 on_reader_exit 触发兜底重连。
            if !stop_for_reader.load(Ordering::Relaxed) {
                on_reader_exit_th();
            }
        });

        // writer：共享 port
        let port_for_writer = Arc::clone(&port);
        let log_for_writer = log.clone();
        // Tx 计数（成功写入底层串口后才 +1，序列化/写入失败的帧不计入）。
        // 由 AppHandle 通过 counters 参数传入；UI 在 statusbar 读这个值。
        let tx_counter_w = Arc::clone(&tx_counter);
        let writer = thread::spawn(move || {
            loop {
                match write_rx.recv() {
                    Ok(WriterMsg::Frame(frame)) => {
                        // 序列化失败则跳过发送，避免把垃圾帧推给固件
                        let s = match frame.encode_line() {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!("帧序列化失败，跳过发送: {e}");
                                log_for_writer.push(
                                    LogKind::App,
                                    format!(
                                        "帧序列化失败 cmd=0x{:02X} seq={}: {e}",
                                        frame.cmd, frame.seq
                                    ),
                                );
                                continue;
                            }
                        };
                        let cmd = frame.cmd;
                        let seq = frame.seq;
                        if let Ok(mut p) = port_for_writer.lock() {
                            if let Err(e) = p.write_all(s.as_bytes()) {
                                tracing::warn!("串口写入失败: {e}");
                                log_for_writer.push(
                                    LogKind::App,
                                    format!("串口写入失败 cmd=0x{cmd:02X} seq={seq}: {e}"),
                                );
                            } else {
                                // 实际字节已经写到 OS 缓冲，Tx 计数 +1。
                                tx_counter_w.fetch_add(1, Ordering::Relaxed);
                                if cmd != protocol::CMD_HEARTBEAT {
                                    // 心跳 Tx 已在 heartbeat.rs 中按完整 JSON 记录，
                                    // 这里跳过避免同一秒两条 Tx 日志。
                                    log_for_writer.push(
                                        LogKind::Tx,
                                        format!("Tx → cmd=0x{cmd:02X} seq={seq}"),
                                    );
                                }
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
        let log_for_router = log.clone();
        // Rx 计数（router 每收到一帧就 +1；heartbeat ack、seq 配对成功、推送
        // 全部计入，便于 UI 直观看到下行流量。心跳 1Hz 时该值每秒 +1）。
        let rx_counter_r = Arc::clone(&rx_counter);
        let router = thread::spawn(move || {
            log_for_router.push(LogKind::App, "router 线程启动".to_string());
            // 心跳 Rx 聚合窗口：与 heartbeat.rs 的 HEARTBEAT_LOG_BATCH 同步使用。
            // 仅在累计 N 次心跳 ack 后写一条汇总日志，避免每秒一条心跳 Rx
            // 噪声；需要逐帧抓包时临时改阈值为 1 即可。
            // 额外保留 last_ack_at_ms 字段以便聚合时附"最近一次 ack 距今多久"。
            // `#[allow(unused_assignments)]`：rx_batch / last_ts 实际在
            // 循环内被读（format!），但 borrow checker 跨 while 闭包看不到
            // 使用点，标记消除误报。
            #[allow(unused_assignments)]
            let mut hb_rx_batch: u32 = 0;
            #[allow(unused_assignments)]
            let mut hb_rx_last_ts: Option<String> = None;
            loop {
                match event_rx.recv() {
                    Ok(LinkEvent::Frame(f)) => {
                        // 任何一帧都算 Rx（heartbeat ack、seq 配对成功、主动推送全包含）。
                        rx_counter_r.fetch_add(1, Ordering::Relaxed);
                        // 心跳响应 → mark_ack
                        if f.cmd == protocol::response_cmd(protocol::CMD_HEARTBEAT) {
                            hb_for_router.mark_ack(f.seq as u64);
                            hb_rx_batch = hb_rx_batch.saturating_add(1);
                            // 解析 timestamp（仅用来在汇总日志里附"最近一次 ack 时间"，
                            // 失败就退化为 None，长时间连不上时由 heartbeat.rs 的
                            // "心跳超时" 兜底日志体现）。
                            hb_rx_last_ts = f
                                .data
                                .as_ref()
                                .and_then(|v| {
                                    serde_json::from_value::<HeartbeatResp>(v.clone()).ok()
                                })
                                .map(|hb| hb.timestamp.to_string());
                            // 达到聚合窗口：把累计 N 次心跳 ack 合并成一条 Rx 汇总日志。
                            // 与 heartbeat.rs 的 Tx 汇总语义对齐（HEARTBEAT_LOG_BATCH 帧）。
                            // 每个 batch 的首帧单独写一条 App 日志，便于
                            // 排查设备刚启动 / 固件时间字段异常等异常路径。
                            if hb_rx_batch == 1 {
                                log_for_router.push(
                                    LogKind::App,
                                    format!("心跳 Rx 新批次 首帧 seq={}", f.seq),
                                );
                            }
                            if hb_rx_batch >= HEARTBEAT_LOG_BATCH {
                                let ts_part = hb_rx_last_ts
                                    .as_deref()
                                    .map(|s| format!(" device_ts={s}"))
                                    .unwrap_or_default();
                                log_for_router.push(
                                    LogKind::Rx,
                                    format!("心跳 Rx×{hb_rx_batch} (last seq={}){ts_part}", f.seq),
                                );
                                hb_rx_batch = 0;
                            }
                            // 后续按完整 data 记录 Rx 日志的旧路径已合并到上面的
                            // 聚合逻辑，避免重复。
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
                        let f_for_log = f.clone();
                        let _ = ui_tx.send(LinkEvent::Frame(f));
                        // 记录 Rx 日志：响应类附 status，方便排查固件错误；
                        // 推送类标注 is_push 以便区分主动上报。
                        // 心跳响应已在上面按完整 data 记录，这里跳过避免重复。
                        if f_for_log.cmd == protocol::response_cmd(protocol::CMD_HEARTBEAT) {
                            // already logged
                        } else if f_for_log.is_response_like() {
                            let tag = if f_for_log.is_push() { "PUSH" } else { "Rx" };
                            log_for_router.push(
                                LogKind::Rx,
                                format!(
                                    "{tag} ← cmd=0x{:02X} seq={} status={:?}",
                                    f_for_log.cmd, f_for_log.seq, f_for_log.status
                                ),
                            );
                        } else {
                            // 协议帧（异类 cmd）也记一下，便于抓包回溯
                            log_for_router.push(
                                LogKind::Rx,
                                format!("Rx ← cmd=0x{:02X} seq={}", f_for_log.cmd, f_for_log.seq),
                            );
                        }
                    }
                    Ok(LinkEvent::State(s)) => {
                        // 异常断开 / 主动断开都会经过这里。
                        // 主动断开的特征：reader 没退出 → 我们会先收到 Disconnected
                        // 是因为 close() 主动让 reader 走完循环；这里统一按"断开"
                        // 处理，清空 pending map，让所有阻塞的 request() 立即
                        // 拿到 RecvTimeoutError::Disconnected，而不是等各自的 timeout。
                        let drain_pending = matches!(
                            s,
                            ConnectionState::Disconnected | ConnectionState::Reconnecting
                        );
                        *state_for_router.lock().unwrap() = s.clone();
                        if drain_pending {
                            // take 走全部 pending 的 sender，drop 后对端的
                            // rx.recv() 会立即返回 Err(Disconnected)。
                            let drained: Vec<Sender<Frame>> = pending_for_router
                                .lock()
                                .unwrap()
                                .drain()
                                .map(|(_, tx)| tx)
                                .collect();
                            drop(drained);
                        }
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
            log,
            ui_rx_slot: Some(ui_rx),
            events_tx: event_tx,
            write_tx,
            pending,
            state,
            hb,
            stop,
            port_name: name,
            _reader: Some(reader),
            _writer: Some(writer),
            _router: Some(router),
            _heartbeat: None,
        })
    }

    /// 启动心跳
    pub fn start_heartbeat(&mut self, log: SharedLog) {
        if self._heartbeat.is_some() {
            return;
        }
        let write_tx = self.write_tx.clone();
        let link_tx = self.events_tx.clone();
        let handle = self.hb.clone();
        let stop = Arc::clone(&self.stop);
        self._heartbeat = Some(heartbeat::spawn(write_tx, handle, link_tx, log, stop));
    }

    /// 关闭
    ///
    /// 顺序：
    /// 1. 置 stop flag
    /// 2. writer 收到 Shutdown → 不再处理请求
    /// 3. heartbeat 停 → 不再发心跳帧
    /// 4. reader 看到 stop flag → 退出 read 循环 → 发 Disconnected → drop 自己的 event_tx clone
    /// 5. **drop LinkManager 持有的 events_tx**（关键）：
    ///    内部事件通道的最后一个发送端是 LinkManager.events_tx，reader/heartbeat
    ///    退出后 channel 还靠它活。close() 在 join router 之前必须显式 drop 掉
    ///    这个 sender，否则 router 的 `event_rx.recv()` 永远收不到 Err、router
    ///    永不退出、`_router.join()` 死锁，整个 UI 卡死。
    /// 6. join router（channel 关闭后 router 自然退出）
    /// 7. join reader / writer / heartbeat 的兜底（多数情况下它们已先于 router 退出）
    ///
    /// 注意：close() 必须在 writer 完全停止之后才能 join reader，否则 reader
    /// 在 writer 还活着时持锁退出可能跟 writer 的 lock() 冲突（虽然这里是
    /// Mutex<Box<dyn ...>>，但 drop 时机仍以 join 顺序为准最稳）。
    pub fn close(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.write_tx.send(WriterMsg::Shutdown);

        // 关键修复：在 join router 之前 drop 掉 events_tx（最后一个内部通道发送端）。
        // 否则 reader/heartbeat 退出后 channel 还活着，router 的 recv() 永不返回 Err，
        // _router.join() 永久阻塞 → UI 线程在 detach_link() 里卡死。
        // events_tx 本身在 LinkManager 里是 Option<Sender<_>>，替换掉就能立即释放；
        // 旧 sender 在表达式结束时 drop，channel 关闭。
        self.events_tx = std::sync::mpsc::channel().0;

        if let Some(h) = self._heartbeat.take() {
            let _ = h.join();
        }
        if let Some(h) = self._writer.take() {
            let _ = h.join();
        }
        if let Some(h) = self._reader.take() {
            let _ = h.join();
        }
        if let Some(h) = self._router.take() {
            let _ = h.join();
        }
        *self.state.lock().unwrap() = ConnectionState::Disconnected;
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
    ///
    /// 失败分两种：
    /// - `Err("disconnected")`：链路已断开（router 清空了 pending map，sender 被 drop）
    /// - `Err("timeout: ...")`：超时未收到响应（设备没回 / seq 不匹配 / 忙）
    ///
    /// 调用方可以用这个区分断线和设备忙。
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
                // sender 已被 router 回收（断线） vs 超时，区分开来便于上层决定
                // 是立刻放弃还是继续等待。
                let still_pending = self.pending.lock().unwrap().remove(&seq).is_some();
                if !still_pending {
                    self.log.push(
                        LogKind::App,
                        format!("request cmd=0x{cmd:02X} seq={seq}: disconnected"),
                    );
                    return Err("disconnected".to_string());
                }
                self.log.push(
                    LogKind::App,
                    format!("request cmd=0x{cmd:02X} seq={seq}: timeout ({e})"),
                );
                Err(format!("timeout: {e}"))
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
