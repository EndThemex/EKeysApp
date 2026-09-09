//! P2 Settings 页面：核心面板。

use eframe::egui;

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
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            SettingsTab::Display => "显示",
            SettingsTab::Keyboard => "键盘",
            SettingsTab::Audio => "音频",
            SettingsTab::Power => "电源",
            SettingsTab::PcStatus => "PC 状态",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SettingsTab::Display => crate::ui::icons::TAB_DISPLAY,
            SettingsTab::Keyboard => crate::ui::icons::TAB_KEYBOARD,
            SettingsTab::Audio => crate::ui::icons::TAB_AUDIO,
            SettingsTab::Power => crate::ui::icons::TAB_POWER,
            SettingsTab::PcStatus => crate::ui::icons::TAB_PC,
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
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut SettingsPanelState) {
    ui.heading("设备设置");
    ui.add_space(4.0);

    // 在线但尚未成功读到 0x07 全量快照 → 提示用户当前展示的不是设备真实配置
    if handle.state.lock().unwrap().is_online() && !handle.is_config_loaded() {
        ui.label(
            egui::RichText::new("正在读取设备配置…（若长时间无变化，请查看日志或点顶栏“刷新”）")
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
    });
}

// -------- Tab 实现 --------

fn display_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label("TFT 主题");
        let mut theme = draft.tft_theme.max(snap.tft_theme);
        egui::ComboBox::from_id_salt("tft-theme")
            .selected_text(if theme == 0 {
                "深色".into()
            } else {
                format!("主题 {theme}")
            })
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut theme, 0, "深色");
                cb.selectable_value(&mut theme, 1, "浅色");
            });
        if theme != snap.tft_theme {
            draft.tft_theme = theme;
        }
        ui.add_space(6.0);

        ui.label("屏幕背光（5~100）");
        // 与 tft_theme 同策略：slider 始终展示设备真实值 (snap)，
        // 仅在用户拖动时才写入 draft；用 max(draft, snap) 会在断连重连后
        // 把残留的旧 draft 顶回去，造成"APP 显示与设备不一致"的错觉。
        let mut brightness = snap.tft_brightness.clamp(5, 100);
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
        ui.label("工作模式");
        let mut mode = if draft.work_mode != 0 || snap.work_mode != 0 {
            draft.work_mode
        } else {
            snap.work_mode
        };
        egui::ComboBox::from_id_salt("work-mode")
            .selected_text(work_mode_label(mode))
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut mode, 0, "USB");
                cb.selectable_value(&mut mode, 1, "BLE");
                cb.selectable_value(&mut mode, 2, "2.4G");
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

        ui.label("当前 Profile");
        ui.label(if snap.active_profile_name.is_empty() {
            "(未知)"
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

        ui.label("切换 Profile (0~7)");
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
        ui.label("Profile 图标（PNG，建议 ≤ 48×48，单帧 ≤ 2048 字节）");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut st.icon_path)
                    .hint_text("PNG 文件路径")
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
            toast(crate::state::ToastKind::Error, format!("读取文件失败: {e}"));
            return;
        }
    };
    // PNG 签名校验
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        toast(
            crate::state::ToastKind::Error,
            "文件不是有效 PNG".to_string(),
        );
        return;
    }
    // image 解码校验（固件不做尺寸校验，App 自行保证）
    if let Err(e) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) {
        toast(crate::state::ToastKind::Error, format!("PNG 解码失败: {e}"));
        return;
    }
    // 2048 字节单帧上限：Base64 膨胀 4/3，留出帧头余量
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    if b64.len() > 1400 {
        toast(
            crate::state::ToastKind::Error,
            format!("图片过大（Base64 {} 字节），请压缩到 48×48 以下", b64.len()),
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
        toast(crate::state::ToastKind::Error, "请求序列化失败".to_string());
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
                        format!("Profile {profile} 图标已上传"),
                    );
                    st.icon_path.clear();
                } else {
                    let msg = frame.error.unwrap_or_else(|| "固件拒绝图标".to_string());
                    toast(crate::state::ToastKind::Error, format!("上传失败: {msg}"));
                }
            }
            Err(e) => toast(crate::state::ToastKind::Error, format!("上传超时: {e}")),
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
        toast(crate::state::ToastKind::Error, "请求序列化失败".to_string());
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
                        format!("Profile {profile} 图标已清除"),
                    );
                } else {
                    let msg = frame.error.unwrap_or_else(|| "固件拒绝清除".to_string());
                    toast(crate::state::ToastKind::Error, format!("清除失败: {msg}"));
                }
            }
            Err(e) => toast(crate::state::ToastKind::Error, format!("清除超时: {e}")),
        }
    });
}

fn audio_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label("音量 (0~100)");
        let mut v = draft.device_volume.max(snap.device_volume).clamp(0, 100);
        if ui
            .add(egui::Slider::new(&mut v, 0..=100).show_value(true))
            .changed()
        {
            draft.device_volume = v;
        }
        ui.add_space(6.0);
        ui.label("启用音频");
        let mut enable = if draft.audio_enable != 0 || snap.audio_enable != 0 {
            draft.audio_enable != 0
        } else {
            snap.audio_enable != 0
        };
        if ui.checkbox(&mut enable, "启用").changed() {
            draft.audio_enable = if enable { 1 } else { 0 };
        }
    });
}

fn power_tab(ui: &mut egui::Ui, snap: &DeviceSettings, draft: &mut DeviceSettings) {
    ui.group(|ui| {
        ui.label("电源模式");
        let mut pm = draft.power_mode.max(snap.power_mode);
        egui::ComboBox::from_id_salt("power-mode")
            .selected_text(format!("模式 {pm}"))
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut pm, 0, "模式 0");
                cb.selectable_value(&mut pm, 1, "模式 1");
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
        ui.strong("PC 状态推送");
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "启用后，App 周期向设备推送当前主机的 Caps/Num/Scroll Lock 与网络状态，\
                 设备可据此刷新主屏指示灯。默认关闭。",
            )
            .weak(),
        );
        ui.add_space(4.0);

        // 1) 启用开关
        let mut enabled = handle.pc_status_push_enabled.load(Ordering::Relaxed);
        let online = handle.state.lock().unwrap().is_online();
        if ui
            .checkbox(&mut enabled, "启用 PC 状态向设备推送")
            .on_hover_text(
                "切换后立即生效；启用时仅在「在线」状态下发送（1s 周期）；退出 App 时自动保存。",
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
                    "已启用 PC 状态推送".to_string()
                } else {
                    "已停止 PC 状态推送".to_string()
                },
            ));
        }

        ui.add_space(4.0);
        let status_text = match (enabled, online) {
            (false, _) => "未启用",
            (true, false) => "等待设备连接…",
            (true, true) => "运行中（1s/次）",
        };
        ui.label(format!("当前状态：{status_text}"));

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // 2) 实时快照展示（节流 1s，避免每帧都 GetAsyncKeyState）
        ui.strong("实时快照");
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
            ui.label("(尚未采集)");
            return;
        };
        egui::Grid::new("pc-status-grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("Caps Lock");
                ui.label(lock_label(snap.caps_lock));
                ui.end_row();
                ui.label("Num Lock");
                ui.label(lock_label(snap.num_lock));
                ui.end_row();
                ui.label("Scroll Lock");
                ui.label(lock_label(snap.scroll_lock));
                ui.end_row();
                ui.label("网络连通");
                ui.label(network_label(snap.network_connected));
                ui.end_row();
            });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "快照仅用于本地展示；启用推送后，相同数据会按 1s 周期通过 0x0D 推送到设备。",
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
        None => "(未采集)".into(),
    }
}

fn network_label(v: Option<bool>) -> String {
    match v {
        Some(true) => "已连接".into(),
        Some(false) => "未连接".into(),
        None => "(未采集)".into(),
    }
}

fn work_mode_label(m: i32) -> String {
    match m {
        0 => "USB".into(),
        1 => "BLE".into(),
        2 => "2.4G".into(),
        _ => format!("未知 ({m})"),
    }
}
