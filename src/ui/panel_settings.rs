//! P2 Settings 页面：核心面板。

use eframe::egui;
use std::sync::Arc;

use crate::i18n;
use crate::protocol::DeviceSettings;
use crate::state::{AppHandle, UiConfirmKind, UiEvent};
use crate::ui::widgets::{apply_diff, settings_panel_scaffold};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SettingsTab {
    #[default]
    Display,
    Keyboard,
    Audio,
    /// 暂时隐藏：不出现在 tab 列表中，实现保留以便后续恢复。
    #[allow(dead_code)]
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
            SettingsTab::Display => i18n::t("settings.tab.display"),
            SettingsTab::Keyboard => i18n::t("settings.tab.keyboard"),
            SettingsTab::Audio => i18n::t("settings.tab.audio"),
            SettingsTab::Power => i18n::t("settings.tab.power"),
            SettingsTab::PcStatus => i18n::t("settings.tab.pc_status"),
            SettingsTab::Firmware => i18n::t("settings.tab.firmware"),
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
    /// 切换工作模式确认弹窗期间暂存的用户选择。
    /// 组合框选中值是帧局部变量，跨帧会从 draft/snap 重算回旧值，
    /// 不暂存的话确认时写回的是旧值，导致 work_mode 永远下发不出去。
    pub pending_work_mode: Option<i32>,
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
    /// OTA 触发请求的异步结果接收端（Some = 等待设备确认，每帧轮询）
    pub fw_ota_rx: Option<std::sync::mpsc::Receiver<Result<crate::protocol::Frame, String>>>,
    /// 本次 OTA 的下载 URL（成功后状态文案展示用）
    pub fw_url: String,
    /// OTA 状态文案（None = 无）
    pub fw_status: Option<String>,
    /// 状态是否为错误（控制显示颜色）
    pub fw_status_is_err: bool,
    /// 进入烧录模式（0x14）请求的异步结果接收端（Some = 等待设备确认，每帧轮询）
    pub fw_download_rx: Option<std::sync::mpsc::Receiver<Result<crate::protocol::Frame, String>>>,
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut SettingsPanelState) {
    ui.heading(i18n::t("settings.page.title"));
    ui.add_space(4.0);

    // 在线但尚未成功读到 0x07 全量快照 → 提示用户当前展示的不是设备真实配置
    if handle.state.lock().unwrap().is_online() && !handle.is_config_loaded() {
        ui.label(egui::RichText::new(i18n::t("settings.page.loading")).weak());
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
            // Keyboard tab（工作模式 / 键映射 Profile / 图标上传）随设备端 HID
            // 精简停用（2026-10-12）：设备端已无键盘，工作模式切到蓝牙还会
            // 关闭 WiFi 影响对时。代码与 match 分支保留，恢复时加回此处即可。
            // SettingsTab::Keyboard,
            SettingsTab::Audio,
            // SettingsTab::Power 暂时隐藏，需要恢复时加回此处即可
            // PcStatus tab 随设备端 cmd_pc_status 注销一并停用（2026-10-12）
            // SettingsTab::PcStatus,
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
        // 主题切换随精简停用（2026-10-12）：设备仅剩主页 + 音频设置两屏，
        // 深浅主题意义不大。保留亮度调节。恢复时撤掉注释即可。
        // ui.label(i18n::t("settings.display.theme"));
        // let mut theme = draft.tft_theme.max(snap.tft_theme);
        // egui::ComboBox::from_id_salt("tft-theme")
        //     .selected_text(if theme == 0 {
        //         i18n::t("settings.display.theme.dark")
        //     } else {
        //         i18n::t("settings.display.theme.light")
        //     })
        //     .show_ui(ui, |cb| {
        //         cb.selectable_value(&mut theme, 0, i18n::t("settings.display.theme.dark"));
        //         cb.selectable_value(&mut theme, 1, i18n::t("settings.display.theme.light"));
        //     });
        // if theme != snap.tft_theme {
        //     draft.tft_theme = theme;
        // }
        // ui.add_space(6.0);

        ui.label(i18n::t("settings.display.backlight"));
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
        ui.label(i18n::t("settings.keyboard.mode"));
        let mut mode = if draft.work_mode != 0 || snap.work_mode != 0 {
            draft.work_mode
        } else {
            snap.work_mode
        };
        egui::ComboBox::from_id_salt("work-mode")
            .selected_text(work_mode_label(mode))
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut mode, 0, i18n::t("settings.keyboard.mode.usb"));
                cb.selectable_value(&mut mode, 1, i18n::t("settings.keyboard.mode.bluetooth"));
                cb.selectable_value(&mut mode, 2, i18n::t("settings.keyboard.mode.wireless_24g"));
            });
        // 危险操作 → 走 confirm。
        // 弹窗「是」由 app.rs 下一帧置位 pending_confirm，此时组合框选中值
        // （帧局部变量）已从 draft/snap 重算回旧值，`mode != snap.work_mode`
        // 必然为 false——确认的消费逻辑绝不能挂在该条件之下，否则必须再动
        // 一次下拉框才会真正下发。
        if st.pending_confirm == Some(UiConfirmKind::SwitchWorkMode) {
            st.pending_confirm = None;
            if let Some(target) = st.pending_work_mode.take() {
                // 与 EnterDownloadMode 一致：确认即生效，直接单字段下发
                // （{"config":{"work_mode":N}}）。固件重建键盘后回推新
                // 快照（0x87 seq=0），UI 随之刷新。
                let diff = {
                    let mut d = snap.clone();
                    d.work_mode = target;
                    d
                };
                let mask = crate::protocol::FieldMask::empty().set(crate::protocol::F_WORK_MODE);
                draft.work_mode = target;
                crate::ui::widgets::apply_diff(handle, &diff, mask);
            }
        } else if mode != snap.work_mode && mode != draft.work_mode {
            // 本帧刚选了与 draft/snap 都不同的新值 → 暂存选择并弹出确认
            st.pending_work_mode = Some(mode);
            let _ = handle
                .ui_tx
                .send(UiEvent::ConfirmYes(UiConfirmKind::SwitchWorkMode));
        }
        ui.add_space(6.0);

        ui.label(i18n::t("settings.keyboard.profile.active"));
        ui.label(if snap.active_profile_name.is_empty() {
            i18n::t("settings.common.unknown")
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
                i18n::t("settings.keyboard.icon.custom"),
                13.0,
                ui.style().visuals.text_color(),
                4.0,
            );
        }
        ui.add_space(6.0);

        ui.label(i18n::t("settings.keyboard.profile.switch"));
        let mut p = if draft.active_keymap_profile != 0 || snap.active_keymap_profile != 0 {
            draft.active_keymap_profile
        } else {
            snap.active_keymap_profile
        };
        egui::ComboBox::from_id_salt("profile")
            .selected_text(i18n::t("settings.keyboard.profile.name").replace("{n}", &p.to_string()))
            .show_ui(ui, |cb| {
                for i in 0..=7 {
                    cb.selectable_value(
                        &mut p,
                        i,
                        i18n::t("settings.keyboard.profile.name").replace("{n}", &i.to_string()),
                    );
                }
            });
        if p != snap.active_keymap_profile {
            draft.active_keymap_profile = p;
        }
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Profile 图标：0x11 CMD_PROFILE_ICON_SET 上传 / 清除
        ui.label(i18n::t("settings.keyboard.icon.path"));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut st.icon_path)
                    .hint_text(i18n::t("settings.keyboard.icon.path_hint"))
                    .desired_width(280.0),
            );
            if ui.button(i18n::t("settings.common.browse")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(i18n::t("settings.keyboard.icon.filter"), &["png"])
                    .pick_file()
                {
                    st.icon_path = path.display().to_string();
                }
            }
            let uploading = !st.icon_path.trim().is_empty();
            if ui
                .add_enabled(
                    uploading,
                    egui::Button::new(i18n::t("settings.keyboard.icon.upload")),
                )
                .clicked()
            {
                upload_profile_icon(handle, st, p as u8, st.icon_path.trim().to_string());
            }
            if ui.button(i18n::t("settings.keyboard.icon.clear")).clicked() {
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
            toast(
                crate::state::ToastKind::Error,
                i18n::t("settings.common.read_file_fail").replace("{error}", &e.to_string()),
            );
            return;
        }
    };
    // PNG 签名校验
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        toast(
            crate::state::ToastKind::Error,
            i18n::t("settings.icon.invalid_png").to_string(),
        );
        return;
    }
    // image 解码校验（固件不做尺寸校验，App 自行保证）
    if let Err(e) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
        toast(
            crate::state::ToastKind::Error,
            i18n::t("settings.icon.parse_fail").replace("{error}", &e.to_string()),
        );
        return;
    }
    // 2048 字节单帧上限：Base64 膨胀 4/3，留出帧头余量
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    if b64.len() > 1400 {
        toast(
            crate::state::ToastKind::Error,
            i18n::t("settings.icon.too_large").replace("{bytes}", &b64.len().to_string()),
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
            i18n::t("settings.common.request_build_fail").to_string(),
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
                        i18n::t("settings.icon.upload_done").replace("{n}", &profile.to_string()),
                    );
                    st.icon_path.clear();
                } else {
                    let msg = frame
                        .error
                        .unwrap_or_else(|| i18n::t("settings.icon.device_rejected").to_string());
                    toast(
                        crate::state::ToastKind::Error,
                        i18n::t("settings.icon.upload_fail").replace("{error}", &msg),
                    );
                }
            }
            Err(e) => toast(
                crate::state::ToastKind::Error,
                i18n::t("settings.icon.upload_timeout").replace("{error}", &e),
            ),
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
            i18n::t("settings.common.request_build_fail").to_string(),
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
                        i18n::t("settings.icon.clear_done").replace("{n}", &profile.to_string()),
                    );
                } else {
                    let msg = frame
                        .error
                        .unwrap_or_else(|| i18n::t("settings.icon.clear_rejected").to_string());
                    toast(
                        crate::state::ToastKind::Error,
                        i18n::t("settings.icon.clear_fail").replace("{error}", &msg),
                    );
                }
            }
            Err(e) => toast(
                crate::state::ToastKind::Error,
                i18n::t("settings.icon.clear_timeout").replace("{error}", &e),
            ),
        }
    });
}

fn audio_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label(i18n::t("settings.audio.volume"));
        // 直接读 draft：未编辑时 draft 经 merge_push 始终跟随 snapshot，
        // 不能取 max（否则低于当前值的修改会被立刻回显成旧值，
        // 只能调大不能调小）。
        let mut v = draft.device_volume.clamp(0, 100);
        if ui
            .add(egui::Slider::new(&mut v, 0..=100).show_value(true))
            .changed()
        {
            draft.device_volume = v;
        }
        ui.add_space(6.0);
        let mut enable = if draft.audio_enable != 0 || snap.audio_enable != 0 {
            draft.audio_enable != 0
        } else {
            snap.audio_enable != 0
        };
        if ui
            .checkbox(&mut enable, i18n::t("settings.audio.enable"))
            .changed()
        {
            draft.audio_enable = if enable { 1 } else { 0 };
        }
    });
}

fn power_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label(i18n::t("settings.power.policy"));
        let mut pm = draft.power_mode.max(snap.power_mode);
        egui::ComboBox::from_id_salt("power-mode")
            .selected_text(power_mode_label(pm))
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut pm, 0, i18n::t("settings.power.mode.performance"));
                cb.selectable_value(&mut pm, 1, i18n::t("settings.power.mode.saving"));
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
        ui.strong(i18n::t("settings.pc.group.push"));
        ui.add_space(2.0);
        ui.label(egui::RichText::new(i18n::t("settings.pc.push.desc")).weak());
        ui.add_space(4.0);

        // 1) 启用开关
        let mut enabled = handle.pc_status_push_enabled.load(Ordering::Relaxed);
        let online = handle.state.lock().unwrap().is_online();
        if ui
            .checkbox(&mut enabled, i18n::t("settings.pc.enable"))
            .on_hover_text(i18n::t("settings.pc.enable.hint"))
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
                    i18n::t("settings.pc.toast.on").to_string()
                } else {
                    i18n::t("settings.pc.toast.off").to_string()
                },
            ));
        }

        ui.add_space(4.0);
        let status_text = match (enabled, online) {
            (false, _) => i18n::t("settings.pc.state.disabled"),
            (true, false) => i18n::t("settings.pc.state.waiting"),
            (true, true) => i18n::t("settings.pc.state.pushing"),
        };
        ui.label(i18n::t("settings.pc.state.label").replace("{status}", status_text));

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // 2) 实时快照展示（节流 1s，避免每帧都 GetAsyncKeyState）
        ui.strong(i18n::t("settings.pc.group.live"));
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
            ui.label(i18n::t("settings.pc.empty"));
            return;
        };
        egui::Grid::new("pc-status-grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label(i18n::t("settings.pc.caps"));
                ui.label(lock_label(snap.caps_lock));
                ui.end_row();
                ui.label(i18n::t("settings.pc.num"));
                ui.label(lock_label(snap.num_lock));
                ui.end_row();
                ui.label(i18n::t("settings.pc.scroll"));
                ui.label(lock_label(snap.scroll_lock));
                ui.end_row();
                ui.label(i18n::t("settings.pc.network"));
                ui.label(network_label(snap.network_connected));
                ui.end_row();
            });
    });
}

fn lock_label(v: Option<bool>) -> String {
    match v {
        Some(true) => i18n::t("settings.pc.lock.on").into(),
        Some(false) => i18n::t("settings.pc.lock.off").into(),
        None => i18n::t("settings.common.not_captured").into(),
    }
}

fn network_label(v: Option<bool>) -> String {
    match v {
        Some(true) => i18n::t("settings.pc.network.connected").into(),
        Some(false) => i18n::t("settings.pc.network.disconnected").into(),
        None => i18n::t("settings.common.not_captured").into(),
    }
}

/// OTA 固件升级 UI 暂时隐藏；需要恢复时把返回值改回 `true`。
fn show_ota_ui() -> bool {
    false
}

/// 固件升级 tab：选择本地 `.bin` → 计算并展示 MD5 → 本机起临时 HTTP 服务 →
/// `0x0B CMD_FIRMWARE_INFO` 下发 `url` + `checksum` 触发设备 OTA
/// （固件端实现见 EKeys `src/upgrade/Upgrade.cpp`：流式下载、边下边校验 MD5，
/// 校验失败不影响当前固件，成功自动重启）。
///
/// 前置条件：设备已通过 Wi-Fi 页连接到与 PC 相同的局域网（设备走 HTTP 下载）。
fn firmware_tab(ui: &mut egui::Ui, st: &mut SettingsPanelState, handle: &AppHandle) {
    use std::sync::mpsc::TryRecvError;

    let online = handle.state.lock().unwrap().is_online();

    // 用户已在确认弹窗点了"是"：真正执行进入烧录模式
    if st.pending_confirm == Some(UiConfirmKind::EnterDownloadMode) {
        st.pending_confirm = None;
        start_download_mode(handle, st);
    }

    // ---- 异步结果回收（避免同步 request 阻塞 UI 线程导致窗口卡死）----

    // OTA 触发结果（设备确认 or 拒绝）
    if st.fw_ota_rx.is_some() {
        let outcome = st.fw_ota_rx.as_ref().unwrap().try_recv();
        match outcome {
            Ok(Ok(frame)) => {
                if frame.status() == Some(0) {
                    st.fw_status =
                        Some(i18n::t("settings.fw.ota.confirmed").replace("{url}", &st.fw_url));
                    st.fw_status_is_err = false;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Success,
                        i18n::t("settings.fw.ota.triggered").to_string(),
                    ));
                    handle.log_kind(crate::state::LogKind::Tx, format!("OTA → {}", st.fw_url));
                } else {
                    let msg = frame
                        .error
                        .unwrap_or_else(|| i18n::t("settings.fw.ota.rejected").to_string());
                    if let Some(mut srv) = st.fw_server.take() {
                        srv.stop();
                    }
                    st.fw_status = Some(i18n::t("settings.fw.ota.fail").replace("{error}", &msg));
                    st.fw_status_is_err = true;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Error,
                        i18n::t("settings.fw.ota.fail").replace("{error}", &msg),
                    ));
                }
                st.fw_ota_rx = None;
            }
            Ok(Err(e)) => {
                if let Some(mut srv) = st.fw_server.take() {
                    srv.stop();
                }
                st.fw_status = Some(i18n::t("settings.fw.ota.request_fail").replace("{error}", &e));
                st.fw_status_is_err = true;
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Error,
                    i18n::t("settings.fw.ota.request_fail").replace("{error}", &e),
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
                    handle.log_kind(crate::state::LogKind::Tx, "设备已确认，复位进入烧录模式");
                    // 设备即将复位：主动断开连接让出 COM 口给烧录工具
                    //（download_mode_armed 已在请求发出前置位，重连被抑制）
                    handle.detach_link();
                    st.fw_status = Some(i18n::t("settings.fw.download.done").into());
                    st.fw_status_is_err = false;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Success,
                        i18n::t("settings.fw.download.toast_ok").to_string(),
                    ));
                } else {
                    // 设备明确拒绝（如旧固件不认识 0x14）：解除重连抑制
                    handle
                        .download_mode_armed
                        .store(false, std::sync::atomic::Ordering::Release);
                    let msg = frame
                        .error
                        .unwrap_or_else(|| i18n::t("settings.fw.download.rejected").to_string());
                    st.fw_status =
                        Some(i18n::t("settings.fw.download.fail").replace("{error}", &msg));
                    st.fw_status_is_err = true;
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Error,
                        i18n::t("settings.fw.download.fail").replace("{error}", &msg),
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
                st.fw_status = Some(i18n::t("settings.fw.download.no_confirm").into());
                st.fw_status_is_err = true;
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Warning,
                    i18n::t("settings.fw.download.no_confirm_toast").to_string(),
                ));
            }
            Err(TryRecvError::Empty) => {} // 等待设备确认中（最多 3s）
            Err(TryRecvError::Disconnected) => {
                st.fw_download_rx = None;
            }
        }
    }

    ui.group(|ui| {
        ui.strong(i18n::t("settings.fw.group.info"));
        ui.add_space(2.0);
        let info = handle.device_info.lock().unwrap();
        egui::Grid::new("fw-info-grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label(i18n::t("settings.fw.info.name"));
                ui.label(if info.device_name.is_empty() {
                    i18n::t("settings.common.unknown")
                } else {
                    &info.device_name
                });
                ui.end_row();
                ui.label(i18n::t("settings.fw.info.version"));
                ui.label(if info.firmware_version.is_empty() {
                    i18n::t("settings.common.unknown")
                } else {
                    &info.firmware_version
                });
                ui.end_row();
            });
    });

    // 固件升级（OTA）相关 UI 暂时隐藏（show_ota_ui 置回 true 即恢复）
    if show_ota_ui() {
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.strong(i18n::t("settings.fw.ota.group"));
            ui.add_space(2.0);
            ui.label(egui::RichText::new(i18n::t("settings.fw.ota.desc")).weak());
            ui.add_space(4.0);

            // 1) 选择固件文件
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut st.fw_path)
                        .hint_text(i18n::t("settings.fw.ota.pick_hint"))
                        .desired_width(280.0),
                );
                if ui.button(i18n::t("settings.common.browse")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(i18n::t("settings.fw.ota.filter"), &["bin"])
                        .pick_file()
                    {
                        pick_firmware_file(handle, st, &path);
                    }
                }
            });
            if st.fw_bytes.is_some() {
                ui.label(
                    i18n::t("settings.fw.ota.size")
                        .replace("{bytes}", &st.fw_size.to_string())
                        .replace("{kb}", &format!("{:.1}", st.fw_size as f64 / 1024.0)),
                );
                ui.monospace(i18n::t("settings.fw.ota.md5").replace("{md5}", &st.fw_md5));
            }

            ui.add_space(6.0);

            // 2) 触发升级
            let ready = online && st.fw_bytes.is_some() && st.fw_ota_rx.is_none();
            ui.horizontal(|ui| {
                let starting = st.fw_ota_rx.is_some();
                if ui
                    .add_enabled(
                        ready,
                        egui::Button::new(if starting {
                            i18n::t("settings.common.waiting_confirm")
                        } else {
                            i18n::t("settings.fw.ota.start")
                        }),
                    )
                    .on_disabled_hover_text(if !online {
                        i18n::t("settings.common.device_offline")
                    } else if st.fw_bytes.is_none() {
                        i18n::t("settings.fw.ota.pick_first")
                    } else {
                        i18n::t("settings.common.busy")
                    })
                    .clicked()
                {
                    start_ota(handle, st);
                }
                if (st.fw_server.is_some() || starting)
                    && ui.button(i18n::t("settings.fw.ota.cancel")).clicked()
                {
                    if let Some(mut srv) = st.fw_server.take() {
                        srv.stop();
                    }
                    st.fw_ota_rx = None;
                    st.fw_status = Some(i18n::t("settings.fw.ota.cancelled").into());
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
                    egui::RichText::new(i18n::t("settings.fw.ota.offline_hint"))
                        .weak()
                        .size(11.0),
                );
            }
        });
    }

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.strong(i18n::t("settings.fw.download.enter"));
        ui.add_space(2.0);
        ui.label(egui::RichText::new(i18n::t("settings.fw.download.desc")).weak());
        ui.add_space(4.0);
        let waiting = st.fw_download_rx.is_some();
        if ui
            .add_enabled(
                online && !waiting,
                egui::Button::new(if waiting {
                    i18n::t("settings.common.waiting_confirm")
                } else {
                    i18n::t("settings.fw.download.enter")
                }),
            )
            .on_disabled_hover_text(if !online {
                i18n::t("settings.common.device_offline")
            } else if waiting {
                i18n::t("settings.common.busy")
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
    st.fw_status = Some(i18n::t("settings.fw.download.request_sent").into());
    st.fw_status_is_err = false;

    match handle.with_link(|lm| {
        lm.request_async(
            CMD_FIRMWARE_DOWNLOAD,
            None,
            std::time::Duration::from_millis(3000),
        )
    }) {
        Some(rx) => {
            st.fw_download_rx = Some(rx);
        }
        None => {
            // 未连接：解除抑制，避免残留标记影响后续正常重连
            handle.download_mode_armed.store(false, Ordering::Release);
            st.fw_status = Some(i18n::t("settings.common.device_offline").into());
            st.fw_status_is_err = true;
            let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                crate::state::ToastKind::Error,
                i18n::t("settings.common.device_offline").to_string(),
            ));
        }
    }
}

/// 选择固件文件：读入内存 + 基础校验（ESP32 应用镜像首字节 0xE9）+ 计算 MD5。
fn pick_firmware_file(handle: &AppHandle, st: &mut SettingsPanelState, path: &std::path::Path) {
    let toast = |k: crate::state::ToastKind, t: String| {
        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(k, t));
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            toast(
                crate::state::ToastKind::Error,
                i18n::t("settings.common.read_file_fail").replace("{error}", &e.to_string()),
            );
            return;
        }
    };
    // ESP32 app image 魔数校验（esp_image_header_t magic = 0xE9）
    if bytes.first() != Some(&0xE9) {
        toast(
            crate::state::ToastKind::Error,
            i18n::t("settings.fw.pick.invalid").to_string(),
        );
        return;
    }
    if bytes.is_empty() {
        toast(
            crate::state::ToastKind::Error,
            i18n::t("settings.fw.pick.empty").to_string(),
        );
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
    use crate::ota::{FirmwareServer, local_lan_ip};

    let toast = |k: crate::state::ToastKind, t: String| {
        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(k, t));
    };
    let Some(bytes) = st.fw_bytes.clone() else {
        return;
    };

    // 停掉上一次的服务（幂等），再起本次的
    if let Some(mut old) = st.fw_server.take() {
        old.stop();
    }
    let server = match FirmwareServer::spawn(bytes) {
        Ok(s) => s,
        Err(e) => {
            toast(
                crate::state::ToastKind::Error,
                i18n::t("settings.fw.ota.server_fail").replace("{error}", &e.to_string()),
            );
            return;
        }
    };
    let port = server.port;
    let Some(ip) = local_lan_ip() else {
        toast(
            crate::state::ToastKind::Error,
            i18n::t("settings.fw.ota.no_ip").to_string(),
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
            st.fw_status = Some(i18n::t("settings.fw.ota.request_sent").into());
            st.fw_status_is_err = false;
        }
        None => {
            if let Some(mut srv) = st.fw_server.take() {
                srv.stop();
            }
            st.fw_status = Some(i18n::t("settings.common.device_offline").into());
            st.fw_status_is_err = true;
            toast(
                crate::state::ToastKind::Error,
                i18n::t("settings.common.device_offline").to_string(),
            );
        }
    }
}

fn work_mode_label(m: i32) -> String {
    match m {
        0 => i18n::t("settings.keyboard.mode.usb").into(),
        1 => i18n::t("settings.keyboard.mode.bluetooth").into(),
        2 => i18n::t("settings.keyboard.mode.wireless_24g").into(),
        _ => i18n::t("settings.keyboard.mode.unknown").replace("{n}", &m.to_string()),
    }
}

fn power_mode_label(m: i32) -> String {
    match m {
        0 => i18n::t("settings.power.mode.performance").into(),
        1 => i18n::t("settings.power.mode.saving").into(),
        _ => i18n::t("settings.power.mode.unknown").replace("{n}", &m.to_string()),
    }
}
