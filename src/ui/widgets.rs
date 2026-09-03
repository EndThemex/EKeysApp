//! 共享 UI 小部件：Toast / ConfirmDialog / FieldEditor / DiffPreviewBar

use std::time::{Duration, Instant};

use eframe::egui;

use crate::protocol::DeviceSettings;
use crate::state::{AppHandle, LogKind, ToastKind, UiEvent};

// ============ Toast ============

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub until: Instant,
}

pub fn show_toasts(ctx: &egui::Context, toasts: &mut Vec<Toast>) {
    let now = Instant::now();
    toasts.retain(|t| t.until > now);
    if toasts.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0])
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                for t in toasts.iter() {
                    let (color, icon) = match t.kind {
                        ToastKind::Info => (egui::Color32::from_rgb(80, 130, 180), "ℹ"),
                        ToastKind::Success => (egui::Color32::from_rgb(80, 160, 90), "✔"),
                        ToastKind::Warning => (egui::Color32::from_rgb(200, 160, 60), "⚠"),
                        ToastKind::Error => (egui::Color32::from_rgb(200, 80, 80), "✖"),
                    };
                    egui::Frame::new()
                        .fill(color)
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin {
                            left: 14,
                            right: 14,
                            top: 8,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(360.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(
                                    egui::Color32::WHITE,
                                    format!("{icon} {}", t.text),
                                );
                            });
                        });
                }
            });
        });
}

pub fn push_toast(toasts: &mut Vec<Toast>, kind: ToastKind, text: impl Into<String>, ttl_ms: u64) {
    toasts.push(Toast {
        kind,
        text: text.into(),
        until: Instant::now() + Duration::from_millis(ttl_ms),
    });
}

// ============ Confirm Dialog ============

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfirmOutcome {
    None,
    Yes,
    No,
}

pub fn show_confirm(
    ctx: &egui::Context,
    title: &str,
    body: &str,
    open: &mut bool,
) -> ConfirmOutcome {
    let mut outcome = ConfirmOutcome::None;
    egui::Window::new(title)
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_size([380.0, 180.0])
        .min_size([320.0, 140.0])
        .max_size([520.0, 320.0])
        .show(ctx, |ui| {
            ui.label(body);
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let primary = egui::Button::new("继续")
                    .fill(crate::ui::ACCENT)
                    .corner_radius(egui::CornerRadius::same(6));
                if ui.add(primary).clicked() {
                    outcome = ConfirmOutcome::Yes;
                }
                if ui.button("取消").clicked() {
                    outcome = ConfirmOutcome::No;
                }
            });
        });
    outcome
}

// ============ Local Settings 弹窗 ============

/// 显示本地设置弹窗。返回用户操作：
/// - `None`：保持打开 / 什么都没做
/// - `Some(true)`：点确定
/// - `Some(false)`：点关闭（X）或取消
pub fn show_local_settings(
    ctx: &egui::Context,
    handle: &AppHandle,
    open: &mut bool,
) -> Option<bool> {
    let mut result = None;
    egui::Window::new("⚙ 本地设置")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_size([420.0, 360.0])
        .show(ctx, |ui| {
            // 1) 自动连接
            ui.group(|ui| {
                ui.strong("连接");
                let mut ac = *handle.auto_connect.lock().unwrap();
                if ui.checkbox(&mut ac, "启动时自动连接上次端口").changed() {
                    *handle.auto_connect.lock().unwrap() = ac;
                }
            });

            // 2) 语言
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong("语言");
                let mut lang = handle.language();
                egui::ComboBox::from_id_source("lang-combo")
                    .selected_text(lang.label())
                    .show_ui(ui, |cb| {
                        cb.selectable_value(&mut lang, crate::config::Language::Chinese, "中文");
                        cb.selectable_value(&mut lang, crate::config::Language::English, "English");
                    });
                if lang != handle.language() {
                    handle.local_config.lock().unwrap().language = lang;
                }
                ui.label("（阶段 04 仅中文生效；切换后 UI 文案尚未本地化）");
            });

            // 3) 主题
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong("主题");
                let mut theme = handle.theme();
                egui::ComboBox::from_id_source("theme-combo")
                    .selected_text(theme.label())
                    .show_ui(ui, |cb| {
                        cb.selectable_value(&mut theme, crate::config::Theme::Dark, "深色");
                        cb.selectable_value(&mut theme, crate::config::Theme::Light, "浅色");
                    });
                if theme != handle.theme() {
                    handle.local_config.lock().unwrap().theme = theme;
                    let _ = handle.ui_tx.send(UiEvent::Toast(
                        ToastKind::Info,
                        "主题切换将在下次启动生效".to_string(),
                    ));
                }
            });

            // 4) 窗口大小（只读展示）
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong("窗口");
                ui.label("当前大小会在退出时自动保存，下次启动恢复。");
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("关闭").clicked() {
                    result = Some(false);
                }
            });
        });
    result
}

// ============ DiffPreviewBar ============

/// 把 diff 通过 0x08 下发；成功后让 settings 刷新
pub fn apply_diff(handle: &AppHandle, diff: &DeviceSettings) {
    use crate::protocol::CMD_CONFIG_SET;
    let payload = serde_json::json!({ "config": diff });
    let _ = handle.with_link(|lm| {
        match lm.request(
            CMD_CONFIG_SET,
            Some(payload),
            std::time::Duration::from_millis(1000),
        ) {
            Ok(resp) => {
                if resp.status() == Some(1) {
                    let msg = resp.error.unwrap_or_else(|| "未知错误".into());
                    handle.log_kind(crate::state::LogKind::App, format!("SET 失败: {msg}"));
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Error,
                        msg,
                    ));
                } else {
                    handle.log_kind(crate::state::LogKind::Tx, "SET → 已下发");
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Success,
                        "已应用".to_string(),
                    ));
                }
            }
            Err(e) => {
                handle.log_kind(crate::state::LogKind::App, format!("SET 超时: {e}"));
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Warning,
                    "设备无响应".to_string(),
                ));
            }
        }
    });
}

pub fn show_diff_bar(
    handle: &AppHandle,
    ui: &mut egui::Ui,
    diff: &DeviceSettings,
    can_apply: bool,
) -> DiffAction {
    let mut action = DiffAction::None;
    let count = diff_field_count(diff);
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            crate::ui::ACCENT.gamma_multiply(0.6),
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(
                    egui::RichText::new(format!("待下发 {count} 项"))
                        .color(ui.visuals().warn_fg_color),
                );
                ui.separator();
                egui::ScrollArea::horizontal()
                    .max_width(420.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if diff.tft_brightness != 0 {
                                ui.label(format!("tft_brightness={}", diff.tft_brightness));
                            }
                            if diff.tft_theme != 0 {
                                ui.label(format!("tft_theme={}", diff.tft_theme));
                            }
                            if diff.work_mode != 0 {
                                ui.label(format!("work_mode={}", diff.work_mode));
                            }
                            if diff.active_keymap_profile != 0 {
                                ui.label(format!(
                                    "active_keymap_profile={}",
                                    diff.active_keymap_profile
                                ));
                            }
                            if diff.device_volume != 0 {
                                ui.label(format!("device_volume={}", diff.device_volume));
                            }
                            if diff.audio_enable != 0 {
                                ui.label(format!("audio_enable={}", diff.audio_enable));
                            }
                            if diff.power_mode != 0 {
                                ui.label(format!("power_mode={}", diff.power_mode));
                            }
                        });
                    });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("放弃 (Esc)").clicked() {
                        action = DiffAction::Discard;
                    }
                    let apply_btn = egui::Button::new("应用 (Ctrl+Enter)")
                        .fill(crate::ui::ACCENT)
                        .corner_radius(egui::CornerRadius::same(6));
                    if ui.add_enabled(can_apply, apply_btn).clicked() {
                        action = DiffAction::Apply;
                    }
                });
            });
        });
    action
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffAction {
    None,
    Apply,
    Discard,
}

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

// ============ 简易 FieldEditor 辅助 ============

/// 字段变更事件（面板 → AppHandle）
#[derive(Debug, Clone)]
pub enum FieldChange {
    Set(DeviceSettings),
}

/// 占位：导出 LogKind 以便 panel_log 引用
#[allow(dead_code)]
pub fn _kind_marker() -> LogKind {
    LogKind::App
}
