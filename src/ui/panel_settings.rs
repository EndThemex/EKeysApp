//! P2 Settings 页面：核心面板。

use eframe::egui;
use std::sync::Arc;

use crate::protocol::DeviceSettings;
use crate::state::{AppHandle, UiConfirmKind, UiEvent};
use crate::ui::widgets::{apply_diff, settings_panel_scaffold};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SettingsTab {
    #[default]
    Display,
    Keyboard,
    Audio,
    Power,
    /// PC 状态 tab：主机侧行为配置（推送开关 + 实时采集快照展示）。
    /// 不参与 `DeviceSettings` 的 diff / 下发，写入 `local_config` 持久化。
    PcStatus,
    /// 固件升级 tab：选择本地 `.bin` → 本机临时 HTTP 服务 → `0x0B` 触发设备 OTA。
    /// 同样不参与 `DeviceSettings` 的 diff / 下发。
    Firmware,
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            SettingsTab::Display => "显示",
            SettingsTab::Keyboard => "键盘",
            SettingsTab::Audio => "音频",
            SettingsTab::Power => "电源",
            SettingsTab::PcStatus => "PC 状态",
            SettingsTab::Firmware => "固件升级",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SettingsTab::Display => crate::ui::icons::TAB_DISPLAY,
            SettingsTab::Keyboard => crate::ui::icons::TAB_KEYBOARD,
            SettingsTab::Audio => crate::ui::icons::TAB_AUDIO,
            SettingsTab::Power => crate::ui::icons::TAB_POWER,
            SettingsTab::PcStatus => crate::ui::icons::TAB_PC,
            SettingsTab::Firmware => crate::ui::icons::TAB_FIRMWARE,
        }
    }
}

#[derive(Default)]
pub struct SettingsPanelState {
    pub tab: SettingsTab,
    /// 等待 Confirm 的危险操作
    pub pending_confirm: Option<UiConfirmKind>,
    /// Profile 图标：PNG 路径输入框内容（0x11 上传用）
    pub icon_path: String,
    /// PC 状态 tab 上次刷新实时快照的时刻。每 1s 才重新采集一次，
    /// 避免每帧都 `GetAsyncKeyState` 拖慢 UI。
    pub pc_status_snapshot_at: Option<std::time::Instant>,
    /// PC 状态 tab 当前展示的快照缓存（None 表示尚未采集）
    pub pc_status_snapshot: Option<crate::protocol::PcStatus>,
    // ---- 固件升级 tab ----
    /// 已选固件文件路径（展示用）
    pub fw_path: String,
    /// 已读入内存的固件内容（HTTP 服务直接回这段字节）
    pub fw_bytes: Option<Arc<Vec<u8>>>,
    /// 固件 MD5（32 位 hex）
    pub fw_md5: String,
    /// 固件大小（字节）
    pub fw_size: u64,
    /// 本机临时固件 HTTP 服务（Some = OTA 进行中）
    pub fw_server: Option<crate::ota::FirmwareServer>,
    /// 固件信息查询的异步结果接收端（Some = 查询进行中，每帧轮询）
    pub fw_query_rx:
        Option<std::sync::mpsc::Receiver<Result<crate::protocol::Frame, String>>>,
    /// OTA 触发请求的异步结果接收端（Some = 等待设备确认，每帧轮询）
    pub fw_ota_rx:
        Option<std::sync::mpsc::Receiver<Result<crate::protocol::Frame, String>>>,
    /// 本次 OTA 的下载 URL（成功后状态文案展示用）
    pub fw_url: String,
    /// OTA 状态文案（None = 无）
    pub fw_status: Option<String>,
    /// 状态是否为错误（控制显示颜色）
    pub fw_status_is_err: bool,
    /// 进入烧录模式（0x14）请求的异步结果接收端（Some = 等待设备确认，每帧轮询）
    pub fw_download_rx:
        Option<std::sync::mpsc::Receiver<Result<crate::protocol::Frame, String>>>,
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut SettingsPanelState) {
    ui.heading("设备设置");
    ui.add_space(4.0);

    // 在线但尚未成功读到 0x07 全量快照 → 提示用户当前展示的不是设备真实配置
    if handle.state.lock().unwrap().is_online() && !handle.is_config_loaded() {
        ui.label(
            egui::RichText::new(
                "正在读取设备配置…（若长时间无变化，请查看日志或点击顶栏「刷新」）",
            )
            .weak(),
        );
        ui.add_space(4.0);
    }

    // 快捷键：Ctrl+Enter 应用 / Esc 放弃（基于当前 draft vs snapshot）
    let ctrl_enter = ui
        .ctx()
        .input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl);
    let esc = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
    {
        let snapshot = handle.settings.lock().unwrap().clone();
        let draft_now = handle.draft.lock().unwrap().clone();
        let (diff_preview, mask) = draft_now.diff(&snapshot);
        if !mask.is_empty() {
            if ctrl_enter {
                apply_diff(handle, &diff_preview, mask);
            } else if esc {
                *handle.draft.lock().unwrap() = snapshot.clone();
            }
        }
    }

    // Tabs（分段控件样式）
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for t in [
            SettingsTab::Display,
            SettingsTab::Keyboard,
            SettingsTab::Audio,
            SettingsTab::Power,
            SettingsTab::PcStatus,
            SettingsTab::Firmware,
        ] {
            let selected = st.tab == t;
            // IconTextButton 自动按 Phosphor/Proportional 分字体渲染，
            // 不会把图标字形落到 Proportional fallback 链里变成 □。
            let mut btn = crate::ui::fonts::IconTextButton::new(t.icon(), t.label(), 14.0)
                .fill(if selected {
                    crate::ui::ACCENT
                } else {
                    ui.visuals().faint_bg_color
                })
                .corner_radius(egui::CornerRadius::same(8));
            if selected {
                btn = btn.fg(egui::Color32::WHITE);
            }
            if ui.add(btn).clicked() {
                st.tab = t;
            }
        }
    });
    ui.separator();
    ui.add_space(8.0);

    // 各 tab 的内容；scaffold 负责 snapshot/draft/diff/diff_bar。
    let current_tab = st.tab;
    settings_panel_scaffold(handle, ui, move |ui, snap, draft| match current_tab {
        SettingsTab::Display => display_tab(ui, snap, draft),
        SettingsTab::Keyboard => keyboard_tab(ui, snap, draft, st, handle),
        SettingsTab::Audio => audio_tab(ui, snap, draft),
        SettingsTab::Power => power_tab(ui, snap, draft),
        SettingsTab::PcStatus => pc_status_tab(ui, snap, st, handle),
        SettingsTab::Firmware => firmware_tab(ui, st, handle),
    });
}

// -------- Tab 实现 --------

fn display_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label("屏幕主题");
        let mut theme = draft.tft_theme.max(snap.tft_theme);
        egui::ComboBox::from_id_salt("tft-theme")
            .selected_text(if theme == 0 { "深色" } else { "浅色" })
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut theme, 0, "深色");
                cb.selectable_value(&mut theme, 1, "浅色");
            });
        if theme != snap.tft_theme {
            draft.tft_theme = theme;
        }
        ui.add_space(6.0);

        ui.label("屏幕背光（范围 5~100）");
        // slider 优先展示 draft：存在待下发的亮度变更时显示草稿值，
        // 否则每帧从 snap 取值会在松手后把滑块"顶回"设备旧值，
        // 看起来像被自动覆盖（draft 实际一直持有新值，只是没展示）。
        // draft 与 snap 一致时才显示设备真实值，断连重连后由
        // merge_push 同步 draft，不会残留旧值。
        let mut brightness = if draft.tft_brightness != snap.tft_brightness {
            draft.tft_brightness
        } else {
            snap.tft_brightness
        }
        .clamp(5, 100);
        let r = ui.add(egui::Slider::new(&mut brightness, 5..=100).show_value(true));
        if r.changed() {
            draft.tft_brightness = brightness;
        }
    });
}

fn keyboard_tab(
    ui: &mut egui::Ui,
    snap: &DeviceSettings,
    draft: &mut DeviceSettings,
    st: &mut SettingsPanelState,
    handle: &AppHandle,
) {
    ui.group(|ui| {
        ui.label("连接方式");
        let mut mode = if draft.work_mode != 0 || snap.work_mode != 0 {
            draft.work_mode
        } else {
            snap.work_mode
        };
        egui::ComboBox::from_id_salt("work-mode")
            .selected_text(work_mode_label(mode))
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut mode, 0, "USB 有线");
                cb.selectable_value(&mut mode, 1, "蓝牙");
                cb.selectable_value(&mut mode, 2, "2.4G 无线");
            });
        if mode != snap.work_mode {
            // 危险操作 → 走 confirm
            if st.pending_confirm == Some(UiConfirmKind::SwitchWorkMode) {
                draft.work_mode = mode;
                st.pending_confirm = None;
            } else if mode != draft.work_mode && mode != snap.work_mode {
                let _ = handle
                    .ui_tx
                    .send(UiEvent::ConfirmYes(UiConfirmKind::SwitchWorkMode));
            }
        }
        ui.add_space(6.0);

        ui.label("当前按键配置");
        ui.label(if snap.active_profile_name.is_empty() {
            "（未知）"
        } else {
            &snap.active_profile_name
        });
        if snap.active_profile_has_custom_icon {
            // 图标 + 文本分两个 galley 拼接，避免图标字形被 Proportional 字体吞掉。
            let resp =
                ui.allocate_response(egui::vec2(ui.available_width(), 16.0), egui::Sense::hover());
            crate::ui::fonts::paint_icon_text_in(
                ui,
                resp.rect,
                crate::ui::icons::CUSTOM_ICON,
                "已设置自定义图标",
                13.0,
                ui.style().visuals.text_color(),
                4.0,
            );
        }
        ui.add_space(6.0);

        ui.label("切换按键配置（共 8 组）");
        let mut p = if draft.active_keymap_profile != 0 || snap.active_keymap_profile != 0 {
            draft.active_keymap_profile
        } else {
            snap.active_keymap_profile
        };
        egui::ComboBox::from_id_salt("profile")
            .selected_text(format!("配置 {p}"))
            .show_ui(ui, |cb| {
                for i in 0..=7 {
                    cb.selectable_value(&mut p, i, format!("配置 {i}"));
                }
            });
        if p != snap.active_keymap_profile {
            draft.active_keymap_profile = p;
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Profile 图标：0x11 CMD_PROFILE_ICON_SET 上传 / 清除
        ui.label("按键配置图标（建议 PNG，≤ 48×48，文件 ≤ 2 KB）");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut st.icon_path)
                    .hint_text("选择 PNG 图片文件…")
                    .desired_width(260.0),
            );
            let uploading = !st.icon_path.trim().is_empty();
            if ui
                .add_enabled(uploading, egui::Button::new("上传"))
                .clicked()
            {
                upload_profile_icon(handle, st, p as u8, st.icon_path.trim().to_string());
            }
            if ui.button("清除图标").clicked() {
                clear_profile_icon(handle, p as u8);
            }
        });
    });
}

/// `0x11` 上传图标：读 PNG → 校验 → Base64 → 下发。
fn upload_profile_icon(handle: &AppHandle, st: &mut SettingsPanelState, profile: u8, path: String) {
    use crate::protocol::{CMD_PROFILE_ICON_SET, ProfileIconSetPayload, ProfileIconSetReq};
    use std::time::Duration;
    let toast = |k: crate::state::ToastKind, t: String| {
        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(k, t));
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            toast(crate::state::ToastKind::Error, format!("读取文件失败：{e}"));
            return;
        }
    };
    // PNG 签名校验
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        toast(
            crate::state::ToastKind::Error,
            "所选文件不是有效的 PNG 图片".to_string(),
        );
        return;
    }
    // image 解码校验（固件不做尺寸校验，App 自行保证）
    if let Err(e) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
        toast(
            crate::state::ToastKind::Error,
            format!("无法解析 PNG 图片：{e}"),
        );
        return;
    }
    // 2048 字节单帧上限：Base64 膨胀 4/3，留出帧头余量
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    if b64.len() > 1400 {
        toast(
            crate::state::ToastKind::Error,
            format!("图片过大（编码后 {} 字节），请压缩到 48×48 以下", b64.len()),
        );
        return;
    }
    let req = ProfileIconSetPayload {
        profile_icon: ProfileIconSetReq {
            profile,
            clear: false,
            png_base64: b64,
        },
    };
    let Some(data) = serde_json::to_value(&req).ok() else {
        toast(
            crate::state::ToastKind::Error,
            "请求数据生成失败".to_string(),
        );
        return;
    };
    let _ = handle.with_link(|lm| {
        match lm.request(
            CMD_PROFILE_ICON_SET,
            Some(data),
            Duration::from_millis(2000),
        ) {
            Ok(frame) => {
                if frame.status() == Some(0) {
                    // 更新本地快照的图标标记（响应 data 里也有，简化直接置位）
                    let mut snap = handle.settings.lock().unwrap();
                    snap.active_profile_has_custom_icon = true;
                    let mut d = handle.draft.lock().unwrap();
                    d.active_profile_has_custom_icon = true;
                    toast(
                        crate::state::ToastKind::Success,
                        format!("配置 {profile} 的图标已更新"),
                    );
                    st.icon_path.clear();
                } else {
                    let msg = frame
                        .error
                        .unwrap_or_else(|| "设备未接受新图标".to_string());
                    toast(crate::state::ToastKind::Error, format!("上传失败：{msg}"));
                }
            }
            Err(e) => toast(crate::state::ToastKind::Error, format!("上传超时：{e}")),
        }
    });
}

/// `0x11` 清除图标：`clear = true`，不携带 Base64。
fn clear_profile_icon(handle: &AppHandle, profile: u8) {
    use crate::protocol::{CMD_PROFILE_ICON_SET, ProfileIconSetPayload, ProfileIconSetReq};
    use std::time::Duration;
    let toast = |k: crate::state::ToastKind, t: String| {
        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(k, t));
    };
    let req = ProfileIconSetPayload {
        profile_icon: ProfileIconSetReq {
            profile,
            clear: true,
            png_base64: String::new(),
        },
    };
    let Some(data) = serde_json::to_value(&req).ok() else {
        toast(
            crate::state::ToastKind::Error,
            "请求数据生成失败".to_string(),
        );
        return;
    };
    let _ = handle.with_link(|lm| {
        match lm.request(
            CMD_PROFILE_ICON_SET,
            Some(data),
            Duration::from_millis(2000),
        ) {
            Ok(frame) => {
                if frame.status() == Some(0) {
                    let mut snap = handle.settings.lock().unwrap();
                    snap.active_profile_has_custom_icon = false;
                    let mut d = handle.draft.lock().unwrap();
                    d.active_profile_has_custom_icon = false;
                    toast(
                        crate::state::ToastKind::Success,
                        format!("配置 {profile} 的图标已清除"),
                    );
                } else {
                    let msg = frame
                        .error
                        .unwrap_or_else(|| "设备未接受清除请求".to_string());
                    toast(crate::state::ToastKind::Error, format!("清除失败：{msg}"));
                }
            }
            Err(e) => toast(crate::state::ToastKind::Error, format!("清除超时：{e}")),
        }
    });
}

fn audio_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label("音量（范围 0~100）");
        let mut v = draft.device_volume.max(snap.device_volume).clamp(0, 100);
        if ui
            .add(egui::Slider::new(&mut v, 0..=100).show_value(true))
            .changed()
        {
            draft.device_volume = v;
        }
        ui.add_space(6.0);
        ui.label("音频开关");
        let mut enable = if draft.audio_enable != 0 || snap.audio_enable != 0 {
            draft.audio_enable != 0
        } else {
            snap.audio_enable != 0
        };
        if ui.checkbox(&mut enable, "启用音频输出").changed() {
            draft.audio_enable = if enable { 1 } else { 0 };
        }
    });
}

fn power_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label("节能策略");
        let mut pm = draft.power_mode.max(snap.power_mode);
        egui::ComboBox::from_id_salt("power-mode")
            .selected_text(power_mode_label(pm))
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut pm, 0, "性能优先");
                cb.selectable_value(&mut pm, 1, "省电优先");
            });
        if pm != snap.power_mode {
            draft.power_mode = pm;
        }
    });
}

/// PC 状态 tab：
/// 1) 启用开关：默认关闭；勾选后 `tick_pc_status_push` 会周期性推送 PC 状态。
/// 2) 实时快照展示：每 1s 重新采集一次 `pc_status::snapshot()`（限速避免
///    每帧都触发 `GetAsyncKeyState`，开销与采集内容成正比）。
///
/// 注意：本 tab 不改 `DeviceSettings` draft——它是主机侧行为开关，
/// 改动写入 `LocalConfig` 持久化。`settings_panel_scaffold` 仍会画 diff bar，
/// 但因为本 tab 不动 draft，diff 始终为空、bar 自然不显示。
fn pc_status_tab(
    ui: &mut egui::Ui,
    _snap: &DeviceSettings,
    st: &mut SettingsPanelState,
    handle: &AppHandle,
) {
    use crate::protocol::PcStatus;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    ui.group(|ui| {
        ui.strong("主机状态推送");
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "启用后，应用会定时把大写锁定、数字锁定、滚动锁定与网络连通状态推送给设备，\
                 设备屏幕上的指示灯即可随之刷新。默认关闭，避免无意间共享主机状态。",
            )
            .weak(),
        );
        ui.add_space(4.0);

        // 1) 启用开关
        let mut enabled = handle.pc_status_push_enabled.load(Ordering::Relaxed);
        let online = handle.state.lock().unwrap().is_online();
        if ui
            .checkbox(&mut enabled, "向设备同步主机状态")
            .on_hover_text(
                "勾选后立即生效；启用时只在「在线」状态下发送，频率约每秒一次；退出应用时会自动记住选择。",
            )
            .changed()
        {
            handle
                .pc_status_push_enabled
                .store(enabled, Ordering::Relaxed);
            // 同步写一份到 LocalConfig，UI 关闭/重启不必等 on_exit 也能保留
            // （极端场景下进程被杀时也不会丢）。
            handle.local_config.lock().unwrap().pc_status_push = enabled;
            let _ = handle.ui_tx.send(UiEvent::Toast(
                crate::state::ToastKind::Info,
                if enabled {
                    "已开始向设备推送主机状态".to_string()
                } else {
                    "已停止向设备推送主机状态".to_string()
                },
            ));
        }

        ui.add_space(4.0);
        let status_text = match (enabled, online) {
            (false, _) => "未启用",
            (true, false) => "等待设备连接…",
            (true, true) => "正在推送（每秒一次）",
        };
        ui.label(format!("当前状态：{status_text}"));

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // 2) 实时快照展示（节流 1s，避免每帧都 GetAsyncKeyState）
        ui.strong("主机实时状态");
        ui.add_space(2.0);
        let due = st
            .pc_status_snapshot_at
            .map(|t| t.elapsed() >= Duration::from_secs(1))
            .unwrap_or(true);
        if due {
            st.pc_status_snapshot = Some(crate::pc_status::snapshot());
            st.pc_status_snapshot_at = Some(Instant::now());
        }
        let Some(snap): Option<PcStatus> = st.pc_status_snapshot.clone() else {
            ui.label("（尚未采集）");
            return;
        };
        egui::Grid::new("pc-status-grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("大写锁定 Caps");
                ui.label(lock_label(snap.caps_lock));
                ui.end_row();
                ui.label("数字锁定 Num");
                ui.label(lock_label(snap.num_lock));
                ui.end_row();
                ui.label("滚动锁定 Scroll");
                ui.label(lock_label(snap.scroll_lock));
                ui.end_row();
                ui.label("网络连通");
                ui.label(network_label(snap.network_connected));
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "此处仅展示当前主机状态；启用推送后，相同数据每秒同步给设备一次。",
            )
            .weak()
            .size(11.0),
        );
    });
}

fn lock_label(v: Option<bool>) -> String {
    match v {
        Some(true) => "已开启".into(),
        Some(false) => "未开启".into(),
        None => "（未采集）".into(),
    }
}

fn network_label(v: Option<bool>) -> String {
    match v {
        Some(true) => "已连接".into(),
        Some(false) => "未连接".into(),
        None => "（未采集）".into(),
    }
}

/// 固件升级 tab：选择本地 `.bin` → 计算并展示 MD5 → 本机起临时 HTTP 服务 →
/// `0x0B CMD_FIRMWARE_INFO` 下发 `url` + `checksum` 触发设备 OTA
/// （固件端实现见 EKeys `src/upgrade/Upgrade.cpp`：流式下载、边下边校验 MD5，
/// 校验失败不影响当前固件，成功自动重启）。
///
/// 前置条件：设备已通过 Wi-Fi 页连接到与 PC 相同的局域网（设备走 HTTP 下载）。
fn firmware_tab(
    ui: &mut egui::Ui,
    st: &mut SettingsPanelState,
    handle: &AppHandle,
) {
    use crate::protocol::CMD_FIRMWARE_INFO;
    use std::time::Duration;
    use std::sync::mpsc::TryRecvError;

    let online = handle.state.lock().unwrap().is_online();

    // 用户已在确认弹窗点了"是"：真正执行进入烧录模式
    if st.pending_confirm == Some(UiConfirmKind::EnterDownloadMode) {
        st.pending_confirm = None;
        start_download_mode(handle, st);
    }

    // ---- 异步结果回收（避免同步 request 阻塞 UI 线程导致窗口卡死）----

    // 固件信息查询结果
    if st.fw_query_rx.is_some() {
        let outcome = st.fw_query_rx.as_ref().unwrap().try_recv();
        match outcome {
            Ok(Ok(frame)) => {
                if let Some(v) = frame.extra_value("firmware") {
                    match serde_json::from_value::<crate::protocol::FirmwareInfo>(v.clone())
                    {
                        Ok(fw) => {
                            handle.device_info.lock().unwrap().firmware_version =
                                fw.version.clone();
                            handle.log_kind(
                                crate::state::LogKind::Rx,
                                format!(
                                    "GET → 固件信息 v{}（{} {}）",
                                    fw.version, fw.build_date, fw.build_time
                                ),
                            );
                        }
                        Err(e) => {
                            handle.log_kind(
                                crate::state::LogKind::App,
                                format!("固件信息解析失败: {e}"),
                            );
                        }
                    }
                } else {
                    handle.log_kind(crate::state::LogKind::App, "固件信息响应缺 firmware 字段");
                }
                st.fw_query_rx = None;
            }
            Ok(Err(e)) => {
                handle.log_kind(
                    crate::state::LogKind::App,
                    format!("查询固件信息失败: {e}"),
                );
                st.fw_query_rx = None;
            }
            Err(TryRecvError::Empty) => {} // 仍在等待，下一帧继续
            Err(TryRecvError::Disconnected) => {
                st.fw_query_rx = None;
            }
        }
    }

    // OTA 触发结果（设备确认 or 拒绝）
    if st.fw_ota_rx.is_some() {
        let outcome = st.fw_ota_rx.as_ref().unwrap().try_recv();
        match outcome {
            Ok(Ok(frame)) => {
                if frame.status() == Some(0) {
                    st.fw_status = Some(format!(
                        "设备已确认，正在从本机下载固件（{}）…下载并校验通过后自动重启，全程约 1~2 分钟",
                        st.fw_url
                    ));
                    st.fw_status_is_err = false;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Success,
                        "OTA 已触发，设备开始下载固件".to_string(),
                    ));
                    handle.log_kind(
                        crate::state::LogKind::Tx,
                        format!("OTA → {}", st.fw_url),
                    );
                } else {
                    let msg = frame.error.unwrap_or_else(|| "设备拒绝升级".to_string());
                    if let Some(mut srv) = st.fw_server.take() {
                        srv.stop();
                    }
                    st.fw_status = Some(format!("升级失败：{msg}"));
                    st.fw_status_is_err = true;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Error,
                        format!("升级失败：{msg}"),
                    ));
                }
                st.fw_ota_rx = None;
            }
            Ok(Err(e)) => {
                if let Some(mut srv) = st.fw_server.take() {
                    srv.stop();
                }
                st.fw_status = Some(format!("升级请求失败：{e}"));
                st.fw_status_is_err = true;
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Error,
                    format!("升级请求失败：{e}"),
                ));
                st.fw_ota_rx = None;
            }
            Err(TryRecvError::Empty) => {} // 等待设备确认中（最多 3s）
            Err(TryRecvError::Disconnected) => {
                st.fw_ota_rx = None;
            }
        }
    }

    // 烧录模式（0x14）请求结果
    if st.fw_download_rx.is_some() {
        let outcome = st.fw_download_rx.as_ref().unwrap().try_recv();
        match outcome {
            Ok(Ok(frame)) => {
                st.fw_download_rx = None;
                if frame.status() == Some(0) {
                    handle.log_kind(
                        crate::state::LogKind::Tx,
                        "设备已确认，复位进入烧录模式",
                    );
                    // 设备即将复位：主动断开连接让出 COM 口给烧录工具
                    //（download_mode_armed 已在请求发出前置位，重连被抑制）
                    handle.detach_link();
                    st.fw_status = Some(
                        "设备已重启进入烧录模式，连接已断开。\
                         请使用 idf.py flash / esptool 烧录；\
                         烧录完成后在「连接」页手动重连。"
                            .into(),
                    );
                    st.fw_status_is_err = false;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Success,
                        "设备已进入烧录模式".to_string(),
                    ));
                } else {
                    // 设备明确拒绝（如旧固件不认识 0x14）：解除重连抑制
                    handle
                        .download_mode_armed
                        .store(false, std::sync::atomic::Ordering::Release);
                    let msg =
                        frame.error.unwrap_or_else(|| "设备拒绝进入烧录模式".to_string());
                    st.fw_status = Some(format!("进入烧录模式失败：{msg}"));
                    st.fw_status_is_err = true;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Error,
                        format!("进入烧录模式失败：{msg}"),
                    ));
                }
            }
            Ok(Err(e)) => {
                // 请求超时：固件在发出响应约 100ms 后就复位，响应若没来得及
                // flush 便会丢失——极可能设备已进入烧录模式。保持重连抑制
                // （armed 置位），主动断开，由用户手动确认设备状态。
                st.fw_download_rx = None;
                handle.log_kind(
                    crate::state::LogKind::App,
                    format!("烧录模式请求未收到确认: {e}（设备可能已复位）"),
                );
                handle.detach_link();
                st.fw_status = Some(
                    "未收到设备确认（响应可能在复位前丢失）。\
                     设备大概率已进入烧录模式，可直接尝试烧录；\
                     若未进入，请手动重连后重试。"
                        .into(),
                );
                st.fw_status_is_err = true;
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Warning,
                    "未收到设备确认，可能已进入烧录模式".to_string(),
                ));
            }
            Err(TryRecvError::Empty) => {} // 等待设备确认中（最多 3s）
            Err(TryRecvError::Disconnected) => {
                st.fw_download_rx = None;
            }
        }
    }

    ui.group(|ui| {
        ui.strong("当前固件");
        ui.add_space(2.0);
        let info = handle.device_info.lock().unwrap();
        egui::Grid::new("fw-info-grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("设备名称");
                ui.label(if info.device_name.is_empty() {
                    "（未知）"
                } else {
                    &info.device_name
                });
                ui.end_row();
                ui.label("固件版本");
                ui.label(if info.firmware_version.is_empty() {
                    "（未知）"
                } else {
                    &info.firmware_version
                });
                ui.end_row();
            });
        // 异步查询：请求发出后立即返回，结果由上方每帧轮询回收
        let querying = st.fw_query_rx.is_some();
        if ui
            .add_enabled(
                online && !querying,
                egui::Button::new(if querying {
                    "查询中…"
                } else {
                    "查询固件信息"
                }),
            )
            .clicked()
        {
            st.fw_query_rx = handle.with_link(|lm| {
                lm.request_async(CMD_FIRMWARE_INFO, None, Duration::from_millis(2000))
            });
        }
    });

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.strong("固件升级（OTA）");
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "选择固件文件后，应用会在本机临时开启一个 HTTP 服务，\
                 设备通过 Wi-Fi 下载并自动校验 MD5。升级期间请保持设备供电、\
                 不要断开 Wi-Fi；成功后设备会自动重启进入新固件。",
            )
            .weak(),
        );
        ui.add_space(4.0);

        // 1) 选择固件文件
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut st.fw_path)
                    .hint_text("点击「浏览…」选择 .bin 固件文件")
                    .desired_width(280.0),
            );
            if ui.button("浏览…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("固件文件", &["bin"])
                    .pick_file()
                {
                    pick_firmware_file(handle, st, &path);
                }
            }
        });
        if st.fw_bytes.is_some() {
            ui.label(format!(
                "大小：{} 字节（{:.1} KB）",
                st.fw_size,
                st.fw_size as f64 / 1024.0
            ));
            ui.monospace(format!("MD5：{}", st.fw_md5));
        }

        ui.add_space(6.0);

        // 2) 触发升级
        let ready = online
            && st.fw_bytes.is_some()
            && st.fw_ota_rx.is_none()
            && st.fw_query_rx.is_none();
        ui.horizontal(|ui| {
            let starting = st.fw_ota_rx.is_some();
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new(if starting {
                        "等待设备确认…"
                    } else {
                        "开始升级"
                    }),
                )
                .on_disabled_hover_text(if !online {
                    "设备未连接"
                } else if st.fw_bytes.is_none() {
                    "请先选择固件文件"
                } else {
                    "正在处理中"
                })
                .clicked()
            {
                start_ota(handle, st);
            }
            if (st.fw_server.is_some() || starting) && ui.button("取消升级").clicked() {
                if let Some(mut srv) = st.fw_server.take() {
                    srv.stop();
                }
                st.fw_ota_rx = None;
                st.fw_status = Some("已取消：本地下载服务已关闭".into());
                st.fw_status_is_err = false;
            }
        });

        // 3) 状态展示
        if let Some(status) = &st.fw_status {
            ui.add_space(4.0);
            let text = egui::RichText::new(status);
            ui.label(if st.fw_status_is_err {
                text.color(egui::Color32::from_rgb(0xE5, 0x6C, 0x5C))
            } else {
                text
            });
        }
        if !online {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "提示：设备未连接。升级前请先在「连接」页连接设备，\
                     并确认设备已加入与电脑相同的 Wi-Fi 网络（Wi-Fi 页可配置）。",
                )
                .weak()
                .size(11.0),
            );
        }
    });

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.strong("进入烧录模式");
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "设备将立即复位进入 USB 下载模式（无需按 BOOT 键），当前连接会断开，\
                 应用在烧录完成前不会自动重连，以免占用串口。\
                 烧录完成后请在「连接」页手动重连。",
            )
            .weak(),
        );
        ui.add_space(4.0);
        let waiting = st.fw_download_rx.is_some();
        if ui
            .add_enabled(
                online && !waiting,
                egui::Button::new(if waiting {
                    "等待设备确认…"
                } else {
                    "进入烧录模式"
                }),
            )
            .on_disabled_hover_text(if !online {
                "设备未连接"
            } else if waiting {
                "正在处理中"
            } else {
                ""
            })
            .clicked()
        {
            // 危险操作 → 走确认弹窗（与切换工作模式同机制）
            let _ = handle
                .ui_tx
                .send(UiEvent::ConfirmYes(UiConfirmKind::EnterDownloadMode));
        }
    });
}

/// 进入烧录模式：先置位 `download_mode_armed`（抑制自动重连），再发
/// `0x14 CMD_FIRMWARE_DOWNLOAD` 请求（异步）。设备确认后约 100ms 复位，
/// 响应回收后由 `firmware_tab` 主动断开连接，把 COM 口让给烧录工具。
fn start_download_mode(handle: &AppHandle, st: &mut SettingsPanelState) {
    use crate::protocol::CMD_FIRMWARE_DOWNLOAD;
    use std::sync::atomic::Ordering;

    // 先武装抑制标记再发请求：设备复位导致的被动断开也必须跳过重连
    handle.download_mode_armed.store(true, Ordering::Release);
    st.fw_status = Some("已发送进入烧录模式请求，等待设备确认…".into());
    st.fw_status_is_err = false;

    match handle.with_link(|lm| {
        lm.request_async(CMD_FIRMWARE_DOWNLOAD, None, std::time::Duration::from_millis(3000))
    }) {
        Some(rx) => {
            st.fw_download_rx = Some(rx);
        }
        None => {
            // 未连接：解除抑制，避免残留标记影响后续正常重连
            handle.download_mode_armed.store(false, Ordering::Release);
            st.fw_status = Some("设备未连接".into());
            st.fw_status_is_err = true;
            let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                crate::state::ToastKind::Error,
                "设备未连接".to_string(),
            ));
        }
    }
}

/// 选择固件文件：读入内存 + 基础校验（ESP32 应用镜像首字节 0xE9）+ 计算 MD5。
fn pick_firmware_file(
    handle: &AppHandle,
    st: &mut SettingsPanelState,
    path: &std::path::Path,
) {
    let toast = |k: crate::state::ToastKind, t: String| {
        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(k, t));
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            toast(
                crate::state::ToastKind::Error,
                format!("读取文件失败：{e}"),
            );
            return;
        }
    };
    // ESP32 app image 魔数校验（esp_image_header_t magic = 0xE9）
    if bytes.first() != Some(&0xE9) {
        toast(
            crate::state::ToastKind::Error,
            "所选文件不是有效的 ESP32 固件镜像（首字节非 0xE9）".to_string(),
        );
        return;
    }
    if bytes.is_empty() {
        toast(crate::state::ToastKind::Error, "固件文件为空".to_string());
        return;
    }
    st.fw_path = path.display().to_string();
    st.fw_size = bytes.len() as u64;
    st.fw_md5 = crate::ota::md5_hex(&bytes);
    st.fw_bytes = Some(Arc::new(bytes));
    st.fw_status = None;
    st.fw_status_is_err = false;
    handle.log_kind(
        crate::state::LogKind::App,
        format!("已选择固件 {}（{} 字节）", st.fw_path, st.fw_size),
    );
}

/// 触发 OTA：起本地 HTTP 服务 → `0x0B` 下发 URL + MD5。
/// 请求走异步（`request_async`），结果由 `firmware_tab` 每帧轮询回收，
/// 避免同步等待阻塞 UI 线程。设备确认后自行下载，下载完成自动重启
/// （串口会断开，自动重连逻辑会接管重连）。
fn start_ota(handle: &AppHandle, st: &mut SettingsPanelState) {
    use crate::ota::{local_lan_ip, FirmwareServer};

    let toast = |k: crate::state::ToastKind, t: String| {
        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(k, t));
    };
    let Some(bytes) = st.fw_bytes.clone() else { return };

    // 停掉上一次的服务（幂等），再起本次的
    if let Some(mut old) = st.fw_server.take() {
        old.stop();
    }
    let server = match FirmwareServer::spawn(bytes) {
        Ok(s) => s,
        Err(e) => {
            toast(
                crate::state::ToastKind::Error,
                format!("启动本地下载服务失败：{e}"),
            );
            return;
        }
    };
    let port = server.port;
    let Some(ip) = local_lan_ip() else {
        toast(
            crate::state::ToastKind::Error,
            "无法获取本机局域网 IP，请确认电脑已联网".to_string(),
        );
        return;
    };
    let url = format!("http://{ip}:{port}/firmware.bin");
    st.fw_server = Some(server);
    st.fw_url = url.clone();

    let data = serde_json::json!({ "url": url, "checksum": st.fw_md5 });
    match handle.with_link(|lm| {
        lm.request_async(
            crate::protocol::CMD_FIRMWARE_INFO,
            Some(data),
            std::time::Duration::from_millis(3000),
        )
    }) {
        Some(rx) => {
            st.fw_ota_rx = Some(rx);
            st.fw_status = Some("已发送升级请求，等待设备确认…".into());
            st.fw_status_is_err = false;
        }
        None => {
            if let Some(mut srv) = st.fw_server.take() {
                srv.stop();
            }
            st.fw_status = Some("设备未连接".into());
            st.fw_status_is_err = true;
            toast(crate::state::ToastKind::Error, "设备未连接".to_string());
        }
    }
}

fn work_mode_label(m: i32) -> String {
    match m {
        0 => "USB 有线".into(),
        1 => "蓝牙".into(),
        2 => "2.4G 无线".into(),
        _ => format!("未知（{m}）"),
    }
}

fn power_mode_label(m: i32) -> String {
    match m {
        0 => "性能优先".into(),
        1 => "省电优先".into(),
        _ => format!("模式 {m}"),
    }
}
