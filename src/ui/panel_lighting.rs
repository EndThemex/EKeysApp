//! P4 Lighting 页面：键盘 RGB 灯效。
//!
//! 字段：rgb_mode / rgb_single_colar / rgb_click_mode / rgb_brightness
//! 协议阶段 06（设备侧未接入前 UI 可改但不生效）

use std::cell::Cell;

use eframe::egui;

use crate::protocol::DeviceSettings;
use crate::state::AppHandle;
use crate::ui::widgets::{DiffAction, apply_diff, show_diff_bar};

#[derive(Default)]
pub struct LightingPanelState;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, _st: &mut LightingPanelState) {
    ui.heading("灯效");
    ui.label("阶段 06 生效：键盘 RGB 灯效");
    ui.add_space(4.0);

    let snapshot = handle.settings.lock().unwrap().clone();
    let mut draft = handle.draft.lock().unwrap().clone();
    let dirty = Cell::new(false);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.group(|ui| {
            ui.label("RGB 模式");
            let mut mode = if draft.rgb_mode != 0 || snapshot.rgb_mode != 0 {
                draft.rgb_mode
            } else {
                snapshot.rgb_mode
            };
            egui::ComboBox::from_id_source("rgb-mode")
                .selected_text(rgb_mode_label(mode))
                .show_ui(ui, |cb| {
                    cb.selectable_value(&mut mode, 0, "关闭");
                    cb.selectable_value(&mut mode, 1, "静态单色");
                    cb.selectable_value(&mut mode, 2, "流光");
                    cb.selectable_value(&mut mode, 3, "呼吸");
                    cb.selectable_value(&mut mode, 4, "按键触发");
                    cb.selectable_value(&mut mode, 5, "彩虹");
                });
            if mode != snapshot.rgb_mode {
                draft.rgb_mode = mode;
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("单色色值（0~255）");
            let mut colar = draft.rgb_single_colar.max(snapshot.rgb_single_colar);
            if ui
                .add(egui::Slider::new(&mut colar, 0..=255).show_value(true))
                .changed()
            {
                draft.rgb_single_colar = colar;
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("按键触发模式");
            let mut cm = draft.rgb_click_mode.max(snapshot.rgb_click_mode);
            egui::ComboBox::from_id_source("rgb-click")
                .selected_text(rgb_click_label(cm))
                .show_ui(ui, |cb| {
                    cb.selectable_value(&mut cm, 0, "无");
                    cb.selectable_value(&mut cm, 1, "按下时亮");
                    cb.selectable_value(&mut cm, 2, "按下闪一下");
                    cb.selectable_value(&mut cm, 3, "按下渐变");
                });
            if cm != snapshot.rgb_click_mode {
                draft.rgb_click_mode = cm;
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("灯效亮度（0~100）");
            let mut bri = draft
                .rgb_brightness
                .max(snapshot.rgb_brightness)
                .clamp(0, 100);
            if ui
                .add(egui::Slider::new(&mut bri, 0..=100).show_value(true))
                .changed()
            {
                draft.rgb_brightness = bri;
                dirty.set(true);
            }
        });
    });

    // 计算 diff
    let diff = draft.diff(&snapshot);
    let has_diff = lighting_diff_count(&diff) > 0;

    ui.add_space(8.0);
    let action = show_diff_bar(handle, ui, &diff, has_diff);
    match action {
        DiffAction::Apply => apply_diff(handle, &diff),
        DiffAction::Discard => *handle.draft.lock().unwrap() = snapshot.clone(),
        DiffAction::None => {}
    }

    if dirty.get() {
        *handle.draft.lock().unwrap() = draft;
    } else {
        *handle.draft.lock().unwrap() = draft;
    }
}

fn rgb_mode_label(m: i32) -> String {
    match m {
        0 => "关闭".into(),
        1 => "静态单色".into(),
        2 => "流光".into(),
        3 => "呼吸".into(),
        4 => "按键触发".into(),
        5 => "彩虹".into(),
        _ => format!("未知 ({m})"),
    }
}

fn rgb_click_label(m: i32) -> String {
    match m {
        0 => "无".into(),
        1 => "按下时亮".into(),
        2 => "按下闪一下".into(),
        3 => "按下渐变".into(),
        _ => format!("未知 ({m})"),
    }
}

fn lighting_diff_count(d: &DeviceSettings) -> usize {
    let mut n = 0;
    if d.rgb_mode != 0 {
        n += 1;
    }
    if d.rgb_single_colar != 0 {
        n += 1;
    }
    if d.rgb_click_mode != 0 {
        n += 1;
    }
    if d.rgb_brightness != 0 {
        n += 1;
    }
    n
}
