//! AppHandle：UI 可读的共享状态 + 事件总线。

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::link::{ConnectionState, LinkEvent, LinkManager};
use crate::protocol::DeviceSettings;

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
        }
    }

    /// 绑定 LinkManager（连接成功后调用）
    pub fn attach_link(&self, mut lm: LinkManager) {
        lm.start_heartbeat();
        // take 出 events_rx
        let rx = lm.take_events();
        *self.link_events.lock().unwrap() = Some(rx);
        *self.link.lock().unwrap() = Some(lm);
    }

    /// 关闭连接
    pub fn detach_link(&self) {
        if let Some(mut lm) = self.link.lock().unwrap().take() {
            lm.close();
        }
        *self.state.lock().unwrap() = ConnectionState::Disconnected;
    }

    /// 当前 LinkManager 引用（供 app.rs 直接发请求）
    pub fn with_link<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&LinkManager) -> R,
    {
        let g = self.link.lock().unwrap();
        g.as_ref().map(f)
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
