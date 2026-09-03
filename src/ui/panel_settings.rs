//! P2 Settings 页面：核心面板。

use eframe::egui;

use crate::protocol::DeviceSettings;
use crate::state::{AppHandle, UiConfirmKind, UiEvent};
use crate::ui::widgets::{DiffAction, apply_diff, show_diff_bar};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SettingsTab {
    #[default]
    Display,
    Keyboard,
    Audio,
    Power,
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            SettingsTab::Display => "🖥 显示",
            SettingsTab::Keyboard => "⌨ 键盘",
            SettingsTab::Audio => "🔊 音频",
            SettingsTab::Power => "⚡ 电源",
        }
    }
}

#[derive(Default)]
pub struct SettingsPanelState {
    pub tab: SettingsTab,
    /// 等待 Confirm 的危险操作
    pub pending_confirm: Option<UiConfirmKind>,
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut SettingsPanelState) {
    ui.heading("设备设置");
    ui.add_space(4.0);

    // 快捷键：Ctrl+Enter 应用 / Esc 放弃
    let ctrl_enter = ui
        .ctx()
        .input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl);
    let esc = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
    let snapshot = handle.settings.lock().unwrap().clone();
    let draft_now = handle.draft.lock().unwrap().clone();
    let diff_preview = draft_now.diff(&snapshot);
    let has_diff = diff_field_count(&diff_preview) > 0;
    if ctrl_enter && has_diff {
        apply_diff(handle, &diff_preview);
    }
    if esc && has_diff {
        *handle.draft.lock().unwrap() = snapshot.clone();
    }

    // Tabs（分段控件样式）
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for t in [
            SettingsTab::Display,
            SettingsTab::Keyboard,
            SettingsTab::Audio,
            SettingsTab::Power,
        ] {
            let selected = st.tab == t;
            let text = if selected {
                egui::RichText::new(t.label())
                    .size(14.0)
                    .color(egui::Color32::WHITE)
            } else {
                egui::RichText::new(t.label()).size(14.0)
            };
            let btn = egui::Button::new(text)
                .fill(if selected {
                    crate::ui::ACCENT
                } else {
                    ui.visuals().faint_bg_color
                })
                .corner_radius(egui::CornerRadius::same(8));
            if ui.add(btn).clicked() {
                st.tab = t;
            }
        }
    });
    ui.separator();
    ui.add_space(8.0);

    // 复制快照到本地
    let snapshot = handle.settings.lock().unwrap().clone();
    let mut draft = handle.draft.lock().unwrap().clone();
    let mut dirty = false;

    egui::ScrollArea::vertical().show(ui, |ui| match st.tab {
        SettingsTab::Display => display_tab(ui, &snapshot, &mut draft, &mut dirty),
        SettingsTab::Keyboard => keyboard_tab(ui, &snapshot, &mut draft, &mut dirty, st, handle),
        SettingsTab::Audio => audio_tab(ui, &snapshot, &mut draft, &mut dirty),
        SettingsTab::Power => power_tab(ui, &snapshot, &mut draft, &mut dirty),
    });

    // 计算 diff
    let diff = draft.diff(&snapshot);
    let can_apply = diff_field_count(&diff) > 0;

    // DiffPreviewBar
    ui.add_space(8.0);
    let action = show_diff_bar(handle, ui, &diff, can_apply);
    match action {
        DiffAction::Apply => apply_diff(handle, &diff),
        DiffAction::Discard => {
            *handle.draft.lock().unwrap() = snapshot.clone();
        }
        DiffAction::None => {}
    }

    // 把变更写回 draft（dirty 由各 tab 回调标记）
    if dirty {
        *handle.draft.lock().unwrap() = draft.clone();
    } else {
        // 即便 dirty=false，仍同步 draft（用户切换 tab 时需要看到一致视图）
        *handle.draft.lock().unwrap() = draft;
    }
}

// -------- Tab 实现 --------

fn display_tab(
    ui: &mut egui::Ui,
    snap: &DeviceSettings,
    draft: &mut DeviceSettings,
    dirty: &mut bool,
) {
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
            *dirty = true;
        }
        ui.add_space(6.0);

        ui.label("TFT 背光（5~100）");
        let mut brightness = draft.tft_brightness.max(snap.tft_brightness).clamp(5, 100);
        let r = ui.add(egui::Slider::new(&mut brightness, 5..=100).show_value(true));
        if r.changed() {
            draft.tft_brightness = brightness;
            *dirty = true;
        }
    });
}

fn keyboard_tab(
    ui: &mut egui::Ui,
    snap: &DeviceSettings,
    draft: &mut DeviceSettings,
    dirty: &mut bool,
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
                *dirty = true;
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
            ui.label("🖼 已设置自定义图标");
        }
        ui.add_space(6.0);

        ui.label("切换 Profile (0~7)");
        let mut p = if draft.active_keymap_profile != 0 || snap.active_keymap_profile != 0 {
            draft.active_keymap_profile
        } else {
            snap.active_keymap_profile
        };
        egui::ComboBox::from_id_salt("profile")
            .selected_text(format!("Profile {p}"))
            .show_ui(ui, |cb| {
                for i in 0..=7 {
                    cb.selectable_value(&mut p, i, format!("Profile {i}"));
                }
            });
        if p != snap.active_keymap_profile {
            draft.active_keymap_profile = p;
            *dirty = true;
        }
    });
}

fn audio_tab(
    ui: &mut egui::Ui,
    snap: &DeviceSettings,
    draft: &mut DeviceSettings,
    dirty: &mut bool,
) {
    ui.group(|ui| {
        ui.label("音量 (0~100)");
        let mut v = draft.device_volume.max(snap.device_volume).clamp(0, 100);
        if ui
            .add(egui::Slider::new(&mut v, 0..=100).show_value(true))
            .changed()
        {
            draft.device_volume = v;
            *dirty = true;
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
            *dirty = true;
        }
    });
}

fn power_tab(
    ui: &mut egui::Ui,
    snap: &DeviceSettings,
    draft: &mut DeviceSettings,
    dirty: &mut bool,
) {
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
            *dirty = true;
        }
    });
}

// -------- Apply (delegated to widgets::apply_diff) --------

fn diff_field_count(d: &DeviceSettings) -> usize {
    let mut n = 0;
    if d.tft_brightness != 0 {
        n += 1;
    }
    if d.tft_theme != 0 {
        n += 1;
    }
    if d.work_mode != 0 {
        n += 1;
    }
    if d.active_keymap_profile != 0 {
        n += 1;
    }
    if d.device_volume != 0 {
        n += 1;
    }
    if d.audio_enable != 0 {
        n += 1;
    }
    if d.power_mode != 0 {
        n += 1;
    }
    n
}

fn work_mode_label(m: i32) -> String {
    match m {
        0 => "USB".into(),
        1 => "BLE".into(),
        2 => "2.4G".into(),
        _ => format!("未知 ({m})"),
    }
}
