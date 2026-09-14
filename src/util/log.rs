//! tracing 初始化 + 共享日志缓冲。
//!
//! 阶段 04 简化：tracing 默认打到 stderr；面板直接通过 AppHandle.log_* 写入日志。

use std::sync::{Arc, Mutex};

use crate::state::{LogBuffer, LogEntry, LogKind};

/// 共享日志缓冲（5000 条 ring buffer）
#[derive(Clone)]
pub struct SharedLog {
    inner: Arc<Mutex<LogBuffer>>,
    /// 缓存的 `Arc<Vec<LogEntry>>` 快照 + 对应 `LogBuffer::version`：
    /// 下次 `snapshot_arc` 时若 version 未变，直接 clone Arc 返回，避免
    /// 重新 lock + iter().cloned().collect() ~5000 条的字符串深拷贝。
    /// `Mutex` 只在 push / 版本推进的极少数路径里短暂持有，UI 读路径
    /// 只剩一次 `try_lock`（取 version）+ 一次 Arc clone。
    cached: Arc<Mutex<Option<(u64, Arc<Vec<LogEntry>>)>>>,
}

impl SharedLog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogBuffer::new(5000))),
            cached: Arc::new(Mutex::new(None)),
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
        // 缓存命中条件是 version 未变；push 已经 bump，缓存必失效。
        // 主动失效省掉 snapshot_arc 的 try_lock 命中分支再读 inner。
        *self.cached.lock().unwrap() = None;
    }
    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.inner.lock().unwrap().snapshot()
    }
    /// 取当前日志快照的 `Arc<Vec<LogEntry>>`：仅在底层 `LogBuffer::version`
    /// 变化时才重建 Arc 并写缓存，未变化时直接 clone 已缓存 Arc 返回。
    ///
    /// 调用方应把返回的 `Arc` 跨帧持有（放进 panel state），并对比
    /// [`Self::current_version`] 判定是否要刷新本地 `Arc` 句柄。
    pub fn snapshot_arc(&self) -> Arc<Vec<LogEntry>> {
        // 先快路径：仅读 version，未变化则复用缓存的 Arc。
        let buf_version = self.inner.lock().unwrap().version();
        {
            let cache = self.cached.lock().unwrap();
            if let Some((v, arc)) = cache.as_ref() {
                if *v == buf_version {
                    return arc.clone();
                }
            }
        }
        // 缓存失效：重建并写回。
        let snapshot: Arc<Vec<LogEntry>> = Arc::new(self.inner.lock().unwrap().snapshot());
        *self.cached.lock().unwrap() = Some((buf_version, snapshot.clone()));
        snapshot
    }
    /// 当前修订号；与上次的 `snapshot_arc` 缓存键对比即可判定是否要重取。
    /// 读锁路径（避免在持锁状态下调用 `snapshot_arc` 再次锁同一互斥体）。
    pub fn current_version(&self) -> u64 {
        self.inner.lock().unwrap().version()
    }
    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
        *self.cached.lock().unwrap() = None;
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
