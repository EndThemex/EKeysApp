//! AppHandle：UI 可读的共享状态 + 事件总线。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::{Language, LocalConfig, Theme};
use crate::link::{ConnectionState, LinkManager};
use crate::protocol::{
    DeviceInfo, DeviceSettings, FieldMask, FirmwareKeyEntry, KeyAction, KeyRef, KeymapData,
    ProfileState,
};
use crate::util::log::SharedLog;

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
    /// 当前连接端口变化（顶栏显示 + 切换页面时保持显示）
    CurrentPort(String),
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
    /// 共享日志缓冲；Link 层（Tx/Rx）与 UI 层（App/Firmware）共用同一个 buffer，
    /// 这样连接/协议帧日志和 UI 业务日志可以统一在 Log 面板查看。
    pub log: SharedLog,
    pub page: Arc<Mutex<Page>>,
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
    /// 设备信息快照（device_name / device_id / firmware_version）。
    /// 启动或重连后由 `0x03 CMD_DEVICE_INFO_GET` 刷新。
    pub device_info: Arc<Mutex<DeviceInfo>>,
    /// 本次连接是否已成功读取全量配置（0x07）。
    /// 断开时重置；Settings 页据此显示"正在读取设备配置…"提示。
    pub config_loaded: Arc<AtomicBool>,
    /// 本次连接累计 Tx 帧数（用于底栏"已发送 N"展示）。
    /// 由 LinkManager writer 写入成功后自增；断开连接 / 新连接开始时清零。
    pub tx_count: Arc<AtomicU64>,
    /// 本次连接累计 Rx 帧数（用于底栏"已接收 N"展示）。
    /// 由 router 线程收到一帧后自增；断开连接 / 新连接开始时清零。
    pub rx_count: Arc<AtomicU64>,
    /// 本次连接起始时刻（用于底栏"运行时长"展示）；断开连接后清零。
    /// 用 `Mutex<Option<Instant>>` 是为了断开后能可靠判 None（避免 `Instant::now() - 0`
    /// 出现在断线状态下的 UI 中）。
    pub uptime_start: Arc<Mutex<Option<Instant>>>,
}

#[derive(Debug, Clone)]
pub struct ReconnectJob {
    pub port_name: String,
    pub attempt: u32,
    pub next_at_ms: u64,
}

/// 给 reader 退出回调用的轻量句柄（只持必要字段，全部 Send + Sync）。
///
/// 调用它的闭包来自 `LinkManager::open` 的 `on_reader_exit` 参数；当 reader
/// 异常退出（且 router 没机会转发 State(Disconnected)）时触发，作为兜底重连。
#[derive(Clone)]
pub struct ReconnectorHandle {
    pub last_port: Arc<Mutex<Option<String>>>,
    pub ui_tx: Sender<UiEvent>,
    pub pending_reconnect: Arc<Mutex<Option<ReconnectJob>>>,
}

impl ReconnectorHandle {
    /// 触发兜底重连调度：仅在 router 没机会转 Disconnected 的极端场景下使用。
    pub fn trigger(&self) {
        let Some(port) = self.last_port.lock().unwrap().clone() else {
            return;
        };
        let mut slot = self.pending_reconnect.lock().unwrap();
        // 已经有任务就别重复塞
        if slot.is_some() {
            return;
        }
        *slot = Some(ReconnectJob {
            port_name: port.clone(),
            attempt: 0,
            next_at_ms: now_ms() + 1000,
        });
        let _ = self.ui_tx.send(UiEvent::Toast(
            ToastKind::Warning,
            format!("检测到 {port} 异常断开，开始自动重连…"),
        ));
    }
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

/// 重连最大尝试次数。超过后取消任务、提示用户手动重连，避免无限循环
/// 占用资源、掩盖真实硬件问题。
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

impl AppHandle {
    pub fn new() -> Self {
        let (ui_tx, ui_rx) = std::sync::mpsc::channel();
        Self {
            settings: Arc::new(Mutex::new(DeviceSettings::default())),
            draft: Arc::new(Mutex::new(DeviceSettings::default())),
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            log: SharedLog::new(),
            page: Arc::new(Mutex::new(Page::Connect)),
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
            device_info: Arc::new(Mutex::new(DeviceInfo::default())),
            config_loaded: Arc::new(AtomicBool::new(false)),
            tx_count: Arc::new(AtomicU64::new(0)),
            rx_count: Arc::new(AtomicU64::new(0)),
            uptime_start: Arc::new(Mutex::new(None)),
        }
    }

    /// 绑定 LinkManager（连接成功后调用）
    pub fn attach_link(&self, mut lm: LinkManager) {
        // 防御性：覆盖前先把旧的 LinkManager 卸掉并 close，否则旧 LM 的
        // writer / router / heartbeat 线程会泄漏并继续往断开的串口写。
        if let Some(mut old) = self.link.lock().unwrap().take() {
            old.close();
        }
        lm.start_heartbeat(self.log.clone());
        // events_rx 保留在 LinkManager 内部；UI 每帧通过 poll_events 拉取
        // （内部完成 seq 响应配对 + 心跳 ack 标记 + 状态同步）
        *self.link.lock().unwrap() = Some(lm);

        // 新连接开始：清零计数 + 标记 uptime 起点
        self.tx_count.store(0, Ordering::Relaxed);
        self.rx_count.store(0, Ordering::Relaxed);
        *self.uptime_start.lock().unwrap() = Some(Instant::now());

        // 同步共享 state：UI 顶栏/侧栏/连接页都从这里读
        *self.state.lock().unwrap() = ConnectionState::Online;

        // 连接成功 → 自动 GET 设备信息 + 全量快照
        self.auto_get();
    }

    /// 自动 GET：连接成功后拉取设备信息 + 全量设置。
    /// 失败只写日志，不弹 Toast（避免断线后连刷错误）。
    pub fn auto_get(&self) {
        use crate::protocol::{CMD_CONFIG_GET, CMD_DEVICE_INFO_GET, CMD_TIME_SET, DeviceSettings};
        // 0) 同步本地时间到设备
        //
        // 协议 §9.4：连接后先下发 `0x13 CMD_TIME_SET`，写入 epoch + tz。
        // 设备收到后立即 settimeofday() + setenv("TZ")，并在下一 1s tick 刷新主屏时间/日期/星期。
        // NTP 未同步时这是主控可见时间的唯一来源；NTP 已同步时也会被后续 SNTP 覆盖，行为可接受。
        let _ = self.with_link(|lm| {
            let epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let data = serde_json::json!({
                "epoch": epoch,
                "tz": "CST-8",
            });
            match lm.request(CMD_TIME_SET, Some(data), Duration::from_millis(1000)) {
                Ok(frame) => {
                    let status = frame.status.unwrap_or(0);
                    if status == 0 {
                        self.log_kind(LogKind::Tx, "SET → 系统时间");
                    } else {
                        self.log_kind(
                            LogKind::App,
                            format!("SET 系统时间失败: {}", frame.error.unwrap_or_default()),
                        );
                    }
                }
                Err(e) => {
                    self.log_kind(LogKind::App, format!("SET 系统时间超时: {e}"));
                }
            }
        });
        // 1) 设备信息
        //
        // 协议 §6.1：`0x03` 响应把 `device_info` 放在**帧顶层**，不在 `data` 里。
        // `Frame` 用 `#[serde(flatten)]` 把所有未声明的顶层字段收集到 `extra`，
        // 所以这里从 `frame.extra` 取 `device_info` 而不是 `frame.data`。
        // 连接初期设备可能正忙，超时重试一次（共 2 次尝试）。
        let _ = self.with_link(|lm| {
            let mut resp = None;
            for attempt in 1..=2 {
                match lm.request(CMD_DEVICE_INFO_GET, None, Duration::from_millis(1000)) {
                    Ok(frame) => {
                        resp = Some(frame);
                        break;
                    }
                    Err(e) => {
                        self.log_kind(
                            LogKind::App,
                            format!("GET 设备信息超时（第 {attempt} 次）: {e}"),
                        );
                    }
                }
            }
            let Some(frame) = resp else { return };
            if let Some(v) = frame.extra.get("device_info") {
                match serde_json::from_value::<DeviceInfo>(v.clone()) {
                    Ok(info) => {
                        *self.device_info.lock().unwrap() = info;
                        self.log_kind(LogKind::Rx, "GET → 设备信息");
                    }
                    Err(e) => {
                        self.log_kind(LogKind::App, format!("GET 设备信息解析失败: {e}"));
                    }
                }
            } else {
                self.log_kind(
                    LogKind::App,
                    format!(
                        "GET 设备信息响应缺 device_info 字段: cmd=0x{:02X} status={:?}",
                        frame.cmd, frame.status
                    ),
                );
            }
        });
        // 2) 全量设置（1s 超时 × 2 次尝试；失败仅记日志，UI 显示"正在读取"提示）
        let _ = self.with_link(|lm| {
            let mut resp = None;
            for attempt in 1..=2 {
                match lm.request(CMD_CONFIG_GET, None, Duration::from_millis(1000)) {
                    Ok(frame) => {
                        resp = Some(frame);
                        break;
                    }
                    Err(e) => {
                        self.log_kind(LogKind::App, format!("GET 超时（第 {attempt} 次）: {e}"));
                    }
                }
            }
            let Some(frame) = resp else { return };
            match frame.data.as_ref() {
                Some(data) => match serde_json::from_value::<DeviceSettings>(data.clone()) {
                    Ok(mut snap) => {
                        self.apply_settings_snapshot(&mut snap);
                        self.log_kind(LogKind::Rx, "GET → 全量快照");
                    }
                    Err(e) => {
                        self.log_kind(LogKind::App, format!("GET 配置解析失败: {e}"));
                    }
                },
                None => {
                    self.log_kind(LogKind::App, "GET 配置响应缺 data 字段");
                }
            }
        });
        // 3) 当前 Profile 的键映射（0x05；失败仅记日志，键映射页可手动"重新加载"）
        if let Err(e) = self.refresh_keymap_from_device() {
            self.log_kind(LogKind::App, format!("GET 键映射失败: {e}"));
        }
    }

    /// 将一份新配置快照落地：脱敏 → 更新 `settings` → 同步 `draft` 中未编辑字段。
    /// 三条来源统一走这里，保证 draft 合并行为一致（否则连接后 draft 停留在
    /// 全默认值，Settings 页会误报"有未应用更改"，Ctrl+Enter 还会把默认值刷到设备）：
    /// - 连接后 `auto_get()` 的 0x07 响应
    /// - 顶栏"刷新"的 0x07 响应
    /// - 设备主动推送（0x87 seq=0）
    pub fn apply_settings_snapshot(&self, snap: &mut DeviceSettings) {
        // 脱敏：不存储设备回传的密钥明文
        snap.mask_sensitive();
        let old = self.settings.lock().unwrap().clone();
        *self.settings.lock().unwrap() = snap.clone();
        // draft 合并：与旧快照一致的字段（未被编辑）跟随新快照，已编辑字段保留草稿值
        let mut draft = self.draft.lock().unwrap();
        DeviceSettings::merge_push(snap, &old, FieldMask::all(), FieldMask::all(), &mut draft);
        drop(draft);
        self.config_loaded.store(true, Ordering::Release);
    }

    /// 应用设备推送的 Profile 状态（0x10）：更新快照与草稿中的 Profile 展示字段。
    /// 草稿未被编辑的字段跟随新值（与 merge_push 同语义）。
    pub fn apply_profile_state(&self, ps: &ProfileState) {
        let old = {
            let s = self.settings.lock().unwrap();
            (
                s.active_keymap_profile,
                s.active_profile_name.clone(),
                s.active_profile_has_custom_icon,
            )
        };
        {
            let mut s = self.settings.lock().unwrap();
            s.active_keymap_profile = ps.active_profile as i32;
            s.active_profile_name = ps.profile_name.clone();
            s.active_profile_has_custom_icon = ps.has_custom_icon;
        }
        let mut d = self.draft.lock().unwrap();
        if d.active_keymap_profile == old.0 {
            d.active_keymap_profile = ps.active_profile as i32;
        }
        if d.active_profile_name == old.1 {
            d.active_profile_name = ps.profile_name.clone();
        }
        if d.active_profile_has_custom_icon == old.2 {
            d.active_profile_has_custom_icon = ps.has_custom_icon;
        }
    }

    /// 本次连接是否已成功读取全量配置
    pub fn is_config_loaded(&self) -> bool {
        self.config_loaded.load(Ordering::Acquire)
    }

    /// 从设备拉取当前 Profile 的 11 键映射（0x05），同步进快照与草稿。
    ///
    /// 响应的 `keymap` 数组位于帧顶层（Frame::extra），不是 `data.keymap`。
    /// 拉取前先把快照/草稿的 active_profile 对齐设备当前 Profile
    /// （settings.active_keymap_profile），保证条目写入正确的 Profile。
    /// 返回 Ok(条目数) / Err(原因)。
    pub fn refresh_keymap_from_device(&self) -> Result<usize, String> {
        self.with_link(|lm| {
            let frame = lm
                .request(
                    crate::protocol::CMD_KEYMAP_GET,
                    None,
                    Duration::from_millis(1000),
                )
                .map_err(|e| format!("0x05 请求失败: {e}"))?;
            let v = frame.extra_value("keymap").ok_or_else(|| {
                format!(
                    "0x05 响应缺 keymap 字段: cmd=0x{:02X} status={:?}",
                    frame.cmd, frame.status
                )
            })?;
            let entries = serde_json::from_value::<Vec<FirmwareKeyEntry>>(v.clone())
                .map_err(|e| format!("0x05 keymap 解析失败: {e}"))?;
            let n = entries.len();
            let dev_profile = self.settings.lock().unwrap().active_keymap_profile as u8;
            {
                let mut snap = self.keymap.lock().unwrap();
                if snap.profile(dev_profile).is_some() {
                    snap.active_profile = dev_profile;
                }
                snap.apply_firmware_entries(&entries);
            }
            {
                let mut draft = self.keymap_draft.lock().unwrap();
                if draft.profile(dev_profile).is_some() {
                    draft.active_profile = dev_profile;
                }
                draft.apply_firmware_entries(&entries);
            }
            // 选中键引用可能属于旧 Profile，直接清掉避免误导
            *self.selected_key.lock().unwrap() = None;
            self.log_kind(LogKind::Rx, format!("GET → 键映射（{n} 键）"));
            Ok(n)
        })
        .unwrap_or_else(|| Err("未连接".into()))
    }

    /// 关闭连接
    pub fn detach_link(&self) {
        let prev_port = self.port_name_or_last();
        if let Some(mut lm) = self.link.lock().unwrap().take() {
            lm.close();
        }
        *self.state.lock().unwrap() = ConnectionState::Disconnected;
        // 本次连接的配置读取状态作废；重连成功后由 auto_get() 重新置位
        self.config_loaded.store(false, Ordering::Release);
        // 断开连接：清零 Tx/Rx 计数 + 清掉 uptime，UI 立即回到"未连接"展示
        self.tx_count.store(0, Ordering::Relaxed);
        self.rx_count.store(0, Ordering::Relaxed);
        *self.uptime_start.lock().unwrap() = None;
        // 主动断开也清掉挂起重连任务，避免下一次连接时还在跑旧 job
        *self.pending_reconnect.lock().unwrap() = None;
        if let Some(p) = prev_port {
            self.log_kind(LogKind::App, format!("已断开 {p}"));
        } else {
            self.log_kind(LogKind::App, "已断开");
        }
    }

    fn port_name_or_last(&self) -> Option<String> {
        self.link
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|lm| lm.port_name().to_string()))
            .or_else(|| self.last_port.lock().ok().and_then(|p| p.clone()))
    }

    /// 建立一条新连接（共享路径：手动按钮 / 自动连接 / 重连都走这里）。
    /// 成功会附带：写入 `last_port`、顶栏 `CurrentPort`、成功 Toast；
    /// 失败仅返回错误，调用方决定是否推 Toast / 是否调度重连。
    pub fn attempt_connect(&self, name: &str) -> Result<(), String> {
        // 把 ReconnectorHandle 包成 reader 退出回调。router 转发 State(Disconnected)
        // 是主路径；这里只在 router 也挂了时兜底触发重连。
        let recon = self.reconnector();
        let on_exit: std::sync::Arc<dyn Fn() + Send + Sync> =
            std::sync::Arc::new(move || recon.trigger());
        // 把底栏用的 Tx / Rx / uptime 计数句柄传给 LinkManager。
        // writer 写入成功 +1 tx，router 收到帧 +1 rx；uptime 起点在 attach_link 里写。
        let counters = (
            Arc::clone(&self.tx_count),
            Arc::clone(&self.rx_count),
            Arc::clone(&self.uptime_start),
        );
        match crate::link::LinkManager::open(name, self.log.clone(), on_exit, counters) {
            Ok(lm) => {
                *self.last_port.lock().unwrap() = Some(name.to_string());
                // 切到新端口 → 清掉旧的重连任务，避免新连接跑起来后还在探测旧端口
                self.cancel_reconnect();
                self.log_kind(LogKind::App, format!("打开串口成功 {name}，启动后台线程"));
                self.attach_link(lm);
                self.log_kind(LogKind::App, format!("已连接到 {name}"));
                let _ = self.ui_tx.send(UiEvent::CurrentPort(name.to_string()));
                let _ = self
                    .ui_tx
                    .send(UiEvent::Toast(ToastKind::Success, "已连接".into()));
                Ok(())
            }
            Err(e) => {
                self.log_kind(LogKind::App, format!("连接失败: {e}"));
                Err(e)
            }
        }
    }

    /// 启动后台重连循环（指数退避 1→2→4→5s，5 次后放弃）。
    /// 由 App 每帧 tick_reconnect() 驱动。
    pub fn schedule_reconnect(&self, port_name: String) {
        let mut slot = self.pending_reconnect.lock().unwrap();
        // 已存在任务则不重复调度；如果旧任务的目标端口不同，则替换为新端口
        // （用户可能手动切换端口后断线）。attempt 不重置，保留退避节奏。
        if let Some(existing) = slot.as_ref() {
            if existing.port_name == port_name {
                return;
            }
        }
        *slot = Some(ReconnectJob {
            port_name: port_name.clone(),
            attempt: 0,
            next_at_ms: now_ms() + 1000,
        });
        drop(slot);
        self.log_kind(LogKind::App, format!("调度自动重连 {port_name}"));
    }

    /// 每帧调用：处理重连状态机
    pub fn tick_reconnect(&self) {
        // 先复制一份 job，避免长时间持锁（包括 attempt_connect 内部也会 lock）
        let mut slot = self.pending_reconnect.lock().unwrap();
        let Some(mut job) = slot.clone() else { return };
        let now = now_ms();
        if now < job.next_at_ms {
            return;
        }
        // 防御性：探测前再确认一次没有残留的 LinkManager 占着串口。
        // （正常路径下 reader 退出 → app.rs 收到 Disconnected → 主动 detach；
        //  这里兜底覆盖"手动 reset、固件重启但 USB 不掉"等异常路径。）
        if self.link.lock().unwrap().is_some() {
            return; // 仍有连接占用，等下一次 tick 或用户操作
        }
        // 先释放 pending_reconnect 的锁，再做可能阻塞的探测/连接。
        // 否则 attempt_connect 内部若再 lock 同一把锁会死锁。
        *slot = None;
        drop(slot);

        // 探测：仅确认设备是否回来了
        match crate::link::serial::open(&job.port_name) {
            Ok(_port) => {
                // 探测成功：直接建立完整连接。
                // 这里不再"仅弹 Toast 让用户手动接管"，否则重连名存实亡。
                match self.attempt_connect(&job.port_name) {
                    Ok(()) => {
                        // 连接成功：attempt_connect 已经清掉 pending_reconnect 并推 Success Toast
                        let _ = self.ui_tx.send(UiEvent::Toast(
                            ToastKind::Info,
                            format!("已自动重连到 {}", job.port_name),
                        ));
                    }
                    Err(_) => {
                        // 设备能开但 LinkManager 启动失败（极少见）→ 计入 attempt
                        job.attempt = job.attempt.saturating_add(1);
                        self.apply_reconnect_failure(&mut job);
                    }
                }
            }
            Err(_) => {
                job.attempt = job.attempt.saturating_add(1);
                self.apply_reconnect_failure(&mut job);
            }
        }
    }

    /// 重连失败统一处理：写日志 + 推 Toast + 决定是否继续退避 / 放弃。
    fn apply_reconnect_failure(&self, job: &mut ReconnectJob) {
        if job.attempt >= MAX_RECONNECT_ATTEMPTS {
            *self.pending_reconnect.lock().unwrap() = None;
            let _ = self.ui_tx.send(UiEvent::Toast(
                ToastKind::Warning,
                format!(
                    "{} 重连失败（已尝试 {} 次），请检查设备后手动重连",
                    job.port_name, MAX_RECONNECT_ATTEMPTS
                ),
            ));
            return;
        }
        let next_at_ms = now_ms() + backoff_secs(job.attempt);
        let next_job = ReconnectJob {
            port_name: job.port_name.clone(),
            attempt: job.attempt,
            next_at_ms,
        };
        *self.pending_reconnect.lock().unwrap() = Some(next_job);
        let _ = self.ui_tx.send(UiEvent::Toast(
            ToastKind::Info,
            format!("重连 {} 第 {} 次…", job.port_name, job.attempt),
        ));
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
        self.log.push(LogKind::App, text);
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
        self.log.push(kind, text);
    }
}

impl Default for AppHandle {
    fn default() -> Self {
        Self::new()
    }
}
