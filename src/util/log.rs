//! tracing 初始化 + 共享日志缓冲。
//!
//! 阶段 04 简化：tracing 默认打到 stderr；面板直接通过 AppHandle.log_* 写入日志。

use std::sync::{Arc, Mutex};

use crate::state::{LogBuffer, LogEntry, LogKind};

/// 共享日志缓冲（5000 条 ring buffer）
#[derive(Clone)]
pub struct SharedLog {
    inner: Arc<Mutex<LogBuffer>>,
}

impl SharedLog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogBuffer::new(5000))),
        }
    }
    pub fn push(&self, kind: LogKind, text: impl Into<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.inner.lock().unwrap().push(LogEntry {
            ts_ms: now,
            kind,
            text: text.into(),
        });
    }
    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.inner.lock().unwrap().snapshot()
    }
    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }
}

impl Default for SharedLog {
    fn default() -> Self {
        Self::new()
    }
}

/// tracing 初始化（写日志到 stderr，阶段 04 简化）
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .try_init();
}
