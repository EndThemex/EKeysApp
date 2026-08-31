//! 共享 UI 小部件：Toast / ConfirmDialog / FieldEditor / DiffPreviewBar

use std::time::{Duration, Instant};

use eframe::egui;

use crate::protocol::DeviceSettings;
use crate::state::{AppHandle, LogKind, ToastKind};

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
                    let color = match t.kind {
                        ToastKind::Info => egui::Color32::from_rgb(80, 130, 180),
                        ToastKind::Success => egui::Color32::from_rgb(80, 160, 90),
                        ToastKind::Warning => egui::Color32::from_rgb(200, 160, 60),
                        ToastKind::Error => egui::Color32::from_rgb(200, 80, 80),
                    };
                    egui::Frame::group(ui.style()).fill(color).show(ui, |ui| {
                        ui.set_max_width(360.0);
                        ui.colored_label(egui::Color32::WHITE, &t.text);
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
        .show(ctx, |ui| {
            ui.label(body);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("取消").clicked() {
                    outcome = ConfirmOutcome::No;
                }
                if ui.button("继续").clicked() {
                    outcome = ConfirmOutcome::Yes;
                }
            });
        });
    outcome
}

// ============ DiffPreviewBar ============

pub fn show_diff_bar(
    handle: &AppHandle,
    ui: &mut egui::Ui,
    diff: &DeviceSettings,
    can_apply: bool,
) -> DiffAction {
    let mut action = DiffAction::None;
    let count = diff_field_count(diff);
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(50, 60, 70))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 200, 100),
                    format!("待下发 {count} 项"),
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
                    if ui
                        .add_enabled(can_apply, egui::Button::new("应用 (Ctrl+Enter)"))
                        .clicked()
                    {
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
