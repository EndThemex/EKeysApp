//! 媒体控制与状态采集（音乐播放页配套链路）。
//!
//! - **0x0F 入站**：固件音乐二级页旋钮/按钮 → `toggle/prev/next` → 转发为
//!   Windows 系统媒体键（`VK_MEDIA_PLAY_PAUSE / PREV_TRACK / NEXT_TRACK`），
//!   由当前活跃播放器（SMTC 会话）自行响应。
//! - **0x0E 出站**：后台 [`spawn_watcher`] 线程每 500ms 通过 SMTC
//!   （Windows.Media.Control，WinRT）采集"正在播放"快照写入共享槽位；
//!   UI 线程 `tick_music_status_push`（state/mod.rs）按 1s 节拍 diff 后推给固件。
//!
//! 线程模型：watcher 线程独占 WinRT 调用（进入前 `CoInitializeEx(MTA)`），
//! UI 线程只读 `Arc<Mutex<Option<MusicStatus>>>` 共享槽位，不触碰 COM，
//! 避免 UI 线程阻塞（EKeysApp 既有教训：UI 回调里禁止阻塞等待）。

use crate::protocol::{MusicControlAction, MusicStatus};

/// 0x0F 动作 → Windows 系统媒体键。
///
/// 使用 `keybd_event` 注入（媒体键无需前台窗口焦点，由 OS 路由到当前
/// 活跃的 SMTC 会话/播放器）。按下 + 抬起成对发送。
pub fn press_media_key(action: MusicControlAction) {
    platform::press_media_key(action);
}

/// 启动后台采集线程：每 500ms 轮询一次 SMTC，把最新快照写入共享槽位。
///
/// 全进程只启动一次（由 `main.rs` 在 `AppHandle` 构建后调用）。
/// 槽位初值 `None`：watcher 首拍完成前 UI tick 视为"无会话"，发断开基线。
pub fn spawn_watcher(shared: std::sync::Arc<std::sync::Mutex<Option<MusicStatus>>>) {
    platform::spawn_watcher(shared);
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::sync::{Arc, Mutex, OnceLock};
    use windows::core::Result as WinResult;
    use windows::Foundation::TimeSpan;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };
    use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE,
        VK_MEDIA_PREV_TRACK,
    };

    /// 采集轮询周期。500ms 拍：切歌/暂停后设备端 UI 最多延迟 0.5s +
    /// 推送节拍 1s；SMTC 单次查询 <1ms，开销可忽略。
    pub const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

    pub fn press_media_key(action: MusicControlAction) {
        // windows-sys 的 VK_MEDIA_* 定义为 u16，keybd_event 取 u8；
        // 媒体键都在 0xB1~0xB3，截断安全。
        let vk: u8 = match action {
            MusicControlAction::Toggle => VK_MEDIA_PLAY_PAUSE as u8,
            MusicControlAction::Prev => VK_MEDIA_PREV_TRACK as u8,
            MusicControlAction::Next => VK_MEDIA_NEXT_TRACK as u8,
        };
        unsafe {
            keybd_event(vk, 0, 0, 0);
            keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        }
    }

    /// SMTC 管理器进程级单例：`RequestAsync` 拿的是全局会话管理器，
    /// 重复创建既慢又可能拿到不同的聚合视图，用 `OnceLock` 缓存。
    fn manager()
    -> WinResult<&'static GlobalSystemMediaTransportControlsSessionManager> {
        static MANAGER: OnceLock<
            GlobalSystemMediaTransportControlsSessionManager,
        > = OnceLock::new();
        if let Some(m) = MANAGER.get() {
            return Ok(m);
        }
        let m = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
        let _ = MANAGER.set(m);
        Ok(MANAGER.get().unwrap())
    }

    /// TimeSpan（100ns）→ 毫秒。
    fn ts_to_ms(ts: TimeSpan) -> u64 {
        (ts.Duration.max(0) as u64) / 10_000
    }

    /// 采集单个会话的完整快照。子调用失败逐字段降级（如个别播放器
    /// 不给 Timeline），已拿到的字段仍然有效。
    fn session_snapshot(
        session: &GlobalSystemMediaTransportControlsSession,
    ) -> WinResult<MusicStatus> {
        let mut st = MusicStatus::default();
        st.connected = true;

        // 媒体元数据（标题 / 艺术家）
        if let Ok(props) = session.TryGetMediaPropertiesAsync()?.get() {
            st.title = props.Title().map(|s| s.to_string()).unwrap_or_default();
            st.artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
        }

        // 播放状态 + 可用控制
        if let Ok(info) = session.GetPlaybackInfo() {
            if let Ok(status) = info.PlaybackStatus() {
                st.is_playing =
                    status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
                st.is_paused =
                    status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused;
            }
            if let Ok(controls) = info.Controls() {
                st.can_prev = controls.IsPreviousEnabled().unwrap_or(false);
                st.can_next = controls.IsNextEnabled().unwrap_or(false);
            }
        }

        // 进度 / 时长（部分播放器不支持 → 保持 0，固件显示 "--:--"）
        if let Ok(timeline) = session.GetTimelineProperties() {
            st.position_ms = timeline.Position().map(ts_to_ms).unwrap_or(0);
            st.duration_ms = timeline.EndTime().map(ts_to_ms).unwrap_or(0);
        }

        // 播放器标识（AUMID，如 "Spotify.exe"）；截到文件名部分更友好
        if let Ok(aumid) = session.SourceAppUserModelId() {
            st.player = aumid.to_string();
            if let Some(idx) = st.player.rfind('!') {
                st.player.truncate(idx); // 去 AUMID 的 "!App" 等后缀
            }
        }

        Ok(st)
    }

    pub fn snapshot() -> MusicStatus {
        let inner = || -> WinResult<MusicStatus> {
            let m = manager()?;
            let sessions = m.GetSessions()?;
            if sessions.Size()? == 0 {
                // 无活跃会话：发断开基线
                return Ok(MusicStatus::default());
            }
            // 优先取"正在播放"的会话（可能同时存在多个播放器会话）；
            // 没有播放中的则取第一个。
            let mut chosen: Option<GlobalSystemMediaTransportControlsSession> = None;
            for i in 0..sessions.Size()? {
                let s = sessions.GetAt(i)?;
                let playing = s
                    .GetPlaybackInfo()
                    .ok()
                    .and_then(|p| p.PlaybackStatus().ok())
                    .map(|st| {
                        st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
                    })
                    .unwrap_or(false);
                if playing {
                    chosen = Some(s);
                    break;
                }
                if chosen.is_none() {
                    chosen = Some(s);
                }
            }
            let Some(session) = chosen else {
                return Ok(MusicStatus::default());
            };
            session_snapshot(&session)
        };
        match inner() {
            Ok(st) => st,
            Err(e) => {
                tracing::warn!("SMTC 采集失败，按无会话处理: {e}");
                MusicStatus::default()
            }
        }
    }

    pub fn spawn_watcher(shared: Arc<Mutex<Option<MusicStatus>>>) {
        std::thread::Builder::new()
            .name("smtc-watcher".into())
            .spawn(move || {
                // WinRT 调用需要 COM apartment；MTA 允许阻塞等待异步操作。
                // 已初始化成别的模式（STA）时继续跑——manager() 仍可用，
                // 只是 .get() 内部会走消息泵。
                unsafe {
                    // COINIT_MULTITHREADED 在 windows-sys 里是 i32 常量，
                    // CoInitializeEx 形参为 u32，显式转换。
                    CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32);
                }
                loop {
                    let snap = snapshot();
                    if let Ok(mut slot) = shared.lock() {
                        *slot = Some(snap);
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
            })
            .expect("启动 SMTC watcher 线程失败");
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub fn press_media_key(_action: MusicControlAction) {}
    pub fn spawn_watcher(_shared: std::sync::Arc<std::sync::Mutex<Option<MusicStatus>>>) {}
}
