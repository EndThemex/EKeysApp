//! AppHandle：UI 可读的共享状态 + 事件总线。

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::{Language, LocalConfig, Theme};
use crate::link::{ConnectionState, LinkEvent, LinkManager};
use crate::protocol::{DeviceSettings, KeyAction, KeyRef, KeymapData};

/// 单条日志条目（应用层日志 + 固件日志共用）
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub ts_ms: u64,
    pub kind: LogKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogKind {
    Tx,
    Rx,
    Firmware,
    App,
}

/// 简易 ring buffer 日志（5000 条）
#[derive(Debug)]
pub struct LogBuffer {
    cap: usize,
    inner: VecDeque<LogEntry>,
}

impl LogBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            inner: VecDeque::with_capacity(cap),
        }
    }
    pub fn push(&mut self, e: LogEntry) {
        if self.inner.len() == self.cap {
            self.inner.pop_front();
        }
        self.inner.push_back(e);
    }
    pub fn clear(&mut self) {
        self.inner.clear();
    }
    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.inner.iter().cloned().collect()
    }
}

/// UI 主动事件（Toast / 切页 / 危险操作确认）
#[derive(Debug, Clone)]
pub enum UiEvent {
    Toast(ToastKind, String),
    Navigate(Page),
    OpenLocalSettings,
    ConfirmYes(UiConfirmKind),
    ConfirmNo(UiConfirmKind),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiConfirmKind {
    SwitchWorkMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Page {
    Connect,
    Settings,
    Keymap,
    Lighting,
    Wifi,
    Voice,
    Log,
    About,
}

/// 应用共享状态
pub struct AppHandle {
    pub settings: Arc<Mutex<DeviceSettings>>, // 设备最新快照
    pub draft: Arc<Mutex<DeviceSettings>>,    // 用户编辑未下发的草稿
    pub state: Arc<Mutex<ConnectionState>>,
    pub log_buf: Arc<Mutex<LogBuffer>>,
    pub page: Arc<Mutex<Page>>,
    pub link_events: Mutex<Option<Receiver<LinkEvent>>>,
    pub ui_tx: Sender<UiEvent>,
    pub ui_rx: Receiver<UiEvent>,
    pub link: Mutex<Option<LinkManager>>,
    /// Settings 待下发的 diff
    pub pending_diff: Arc<Mutex<DeviceSettings>>,
    /// 上次连接的端口名（用于"启动自动连接"）
    pub last_port: Arc<Mutex<Option<String>>>,
    /// 自动连接开关
    pub auto_connect: Arc<Mutex<bool>>,
    /// 重连任务（断线后调度）
    pub pending_reconnect: Arc<Mutex<Option<ReconnectJob>>>,
    /// 本地 App 配置（语言/主题等；启动时加载，退出时由 on_exit 写回）
    pub local_config: Arc<Mutex<LocalConfig>>,
    /// Keymap 数据：设备最新快照（阶段 05 由 CMD_KEYMAP_GET 刷新）
    pub keymap: Arc<Mutex<KeymapData>>,
    /// Keymap 用户编辑未下发的草稿
    pub keymap_draft: Arc<Mutex<KeymapData>>,
    /// Keymap 面板中当前选中的键（None = 未选中）
    pub selected_key: Arc<Mutex<Option<KeyRef>>>,
    /// Keymap 面板右侧 Drawer 中的临时编辑（None = 当前 binding 无未保存编辑）
    pub pending_binding: Arc<Mutex<Option<KeyAction>>>,
    /// Keymap 面板是否进入"按下任意键捕获"模式（期间全局快捷键应让路）
    pub capture_keyboard: Arc<Mutex<bool>>,
}

#[derive(Debug, Clone)]
pub struct ReconnectJob {
    pub port_name: String,
    pub attempt: u32,
    pub next_at_ms: u64,
}

/// 给 reader 退出回调用的轻量句柄（只持必要字段，全部 Send + Sync）
#[derive(Clone)]
pub struct ReconnectorHandle {
    pub last_port: Arc<Mutex<Option<String>>>,
    pub ui_tx: Sender<UiEvent>,
    pub pending_reconnect: Arc<Mutex<Option<ReconnectJob>>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn backoff_secs(attempt: u32) -> u64 {
    match attempt {
        0 => 1000,
        1 => 2000,
        2 => 4000,
        _ => 5000,
    }
}

impl AppHandle {
    pub fn new() -> Self {
        let (ui_tx, ui_rx) = std::sync::mpsc::channel();
        Self {
            settings: Arc::new(Mutex::new(DeviceSettings::default())),
            draft: Arc::new(Mutex::new(DeviceSettings::default())),
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            log_buf: Arc::new(Mutex::new(LogBuffer::new(5000))),
            page: Arc::new(Mutex::new(Page::Connect)),
            link_events: Mutex::new(None),
            ui_tx,
            ui_rx,
            link: Mutex::new(None),
            pending_diff: Arc::new(Mutex::new(DeviceSettings::default())),
            last_port: Arc::new(Mutex::new(None)),
            auto_connect: Arc::new(Mutex::new(false)),
            pending_reconnect: Arc::new(Mutex::new(None)),
            local_config: Arc::new(Mutex::new(LocalConfig::default())),
            keymap: Arc::new(Mutex::new(KeymapData::default())),
            keymap_draft: Arc::new(Mutex::new(KeymapData::default())),
            selected_key: Arc::new(Mutex::new(None)),
            pending_binding: Arc::new(Mutex::new(None)),
            capture_keyboard: Arc::new(Mutex::new(false)),
        }
    }

    /// 绑定 LinkManager（连接成功后调用）
    pub fn attach_link(&self, mut lm: LinkManager) {
        lm.start_heartbeat();
        // take 出 events_rx
        let rx = lm.take_events();
        *self.link_events.lock().unwrap() = Some(rx);
        *self.link.lock().unwrap() = Some(lm);

        // 连接成功 → 自动 GET 全量快照
        self.auto_get();
    }

    /// 自动 GET：拉取一次全量设置
    pub fn auto_get(&self) {
        use crate::protocol::{CMD_CONFIG_GET, DeviceSettings};
        let _ = self.with_link(|lm| {
            match lm.request(CMD_CONFIG_GET, None, Duration::from_millis(1000)) {
                Ok(frame) => {
                    if let Some(data) = frame.data.as_ref() {
                        if let Ok(snap) = serde_json::from_value::<DeviceSettings>(data.clone()) {
                            *self.settings.lock().unwrap() = snap;
                            self.log_kind(LogKind::Rx, "GET → 全量快照");
                        }
                    }
                }
                Err(e) => {
                    self.log_kind(LogKind::App, format!("GET 超时: {e}"));
                }
            }
        });
    }

    /// 关闭连接
    pub fn detach_link(&self) {
        if let Some(mut lm) = self.link.lock().unwrap().take() {
            lm.close();
        }
        *self.state.lock().unwrap() = ConnectionState::Disconnected;
    }

    /// 启动后台重连循环（指数退避 1→2→4→5s）。
    /// 由 App 每帧 tick_reconnect() 驱动。
    pub fn schedule_reconnect(&self, port_name: String) {
        let mut slot = self.pending_reconnect.lock().unwrap();
        if slot.is_some() {
            return; // 已有重连任务
        }
        *slot = Some(ReconnectJob {
            port_name,
            attempt: 0,
            next_at_ms: now_ms() + 1000,
        });
    }

    /// 每帧调用：处理重连状态机
    pub fn tick_reconnect(&self) {
        let mut slot = self.pending_reconnect.lock().unwrap();
        let Some(mut job) = slot.clone() else { return };
        let now = now_ms();
        if now < job.next_at_ms {
            return;
        }
        // 探测
        match crate::link::serial::open(&job.port_name) {
            Ok(_port) => {
                // 探测成功：通知 UI 让用户重新点 Connect 接管
                *slot = None;
                let _ = self.ui_tx.send(UiEvent::Toast(
                    ToastKind::Success,
                    format!("{} 已就绪，请重新连接", job.port_name),
                ));
                *self.state.lock().unwrap() = ConnectionState::Online;
            }
            Err(_) => {
                job.attempt = job.attempt.saturating_add(1);
                job.next_at_ms = now + backoff_secs(job.attempt);
                *slot = Some(job.clone());
                let _ = self.ui_tx.send(UiEvent::Toast(
                    ToastKind::Info,
                    format!("重连 {} 第 {} 次…", job.port_name, job.attempt),
                ));
            }
        }
    }

    /// 取消待重连任务
    pub fn cancel_reconnect(&self) {
        *self.pending_reconnect.lock().unwrap() = None;
    }

    /// 当前 LinkManager 引用（供 app.rs 直接发请求）
    pub fn with_link<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&LinkManager) -> R,
    {
        let g = self.link.lock().unwrap();
        g.as_ref().map(f)
    }

    /// 生成一个 ReconnectorHandle（给 reader 退出回调用）
    pub fn reconnector(&self) -> ReconnectorHandle {
        ReconnectorHandle {
            last_port: Arc::clone(&self.last_port),
            ui_tx: self.ui_tx.clone(),
            pending_reconnect: Arc::clone(&self.pending_reconnect),
        }
    }

    /// 推一条应用日志
    pub fn log_app(&self, text: impl Into<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.log_buf.lock().unwrap().push(LogEntry {
            ts_ms: now,
            kind: LogKind::App,
            text: text.into(),
        });
    }

    /// 便捷访问当前语言设置（避免每次 clone Arc）
    pub fn language(&self) -> Language {
        self.local_config.lock().unwrap().language
    }

    /// 便捷访问当前主题设置（避免每次 clone Arc）
    pub fn theme(&self) -> Theme {
        self.local_config.lock().unwrap().theme
    }

    /// 推一条 Tx/Rx/Firmware 日志
    pub fn log_kind(&self, kind: LogKind, text: impl Into<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.log_buf.lock().unwrap().push(LogEntry {
            ts_ms: now,
            kind,
            text: text.into(),
        });
    }
}

impl Default for AppHandle {
    fn default() -> Self {
        Self::new()
    }
}
