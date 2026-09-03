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
            let mut mode = draft.rgb_mode;
            let modes: &[(i32, &str, &str)] = &[
                (0, "关闭", "✕"),
                (1, "静态单色", "■"),
                (2, "流光", "→"),
                (3, "呼吸", "◐"),
                (4, "按键触发", "◉"),
                (5, "彩虹", "❉"),
            ];
            mode_grid(ui, modes, &mut mode, crate::ui::ACCENT);
            if mode != draft.rgb_mode {
                draft.rgb_mode = mode;
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("单色色值（0~255）");
            let mut colar = draft.rgb_single_colar;
            if ui
                .add(egui::Slider::new(&mut colar, 0..=255).show_value(true))
                .changed()
            {
                draft.rgb_single_colar = colar;
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("按键触发模式");
            let mut cm = draft.rgb_click_mode;
            let clicks: &[(i32, &str, &str)] = &[
                (0, "无", "✕"),
                (1, "按下时亮", "●"),
                (2, "按下闪一下", "✦"),
                (3, "按下渐变", "❉"),
            ];
            mode_grid(ui, clicks, &mut cm, crate::ui::ACCENT);
            if cm != draft.rgb_click_mode {
                draft.rgb_click_mode = cm;
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("灯效亮度（0~100）");
            let mut bri = draft.rgb_brightness.clamp(0, 100);
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

/// 灯效选择网格：把每个选项渲染成一张卡片，点击即选中。
///
/// `items` 中每个元素为 `(value, label, icon)`：
/// - `value`  选中后写入的数值
/// - `label`  卡片主标题
/// - `icon`   卡片顶部大号图标（emoji/符号均可）
///
/// `selected` 为当前值，函数会把它就地改为用户点击的项。
/// `accent`  为主题强调色，用于高亮当前选中卡片。
fn mode_grid(
    ui: &mut egui::Ui,
    items: &[(i32, &str, &str)],
    selected: &mut i32,
    accent: egui::Color32,
) {
    let card_w = 96.0;
    let card_h = 72.0;
    let spacing = 8.0;

    // 用 wrap 自动按可用宽度折行，避免手算列数 + end_row 引发 Grid cell 分配异常
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
        for (val, label, icon) in items.iter() {
            let is_sel = *selected == *val;
            let text_color = if is_sel {
                egui::Color32::WHITE
            } else {
                ui.style().visuals.text_color()
            };

            // 用 Button::selectable 做单选语义：点击立刻 selected=true 并触发 clicked。
            // 这样不依赖外部 fill/stroke 计算，事件链不会被吞。
            let resp = ui.add(
                egui::Button::selectable(
                    is_sel,
                    egui::RichText::new(format!("{icon}\n{label}"))
                        .color(text_color)
                        .size(12.0),
                )
                .corner_radius(egui::CornerRadius::same(6))
                .min_size(egui::vec2(card_w, card_h)),
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && !is_sel {
                *selected = *val;
            }
            // 选中时叠一层强调色（避免与 SelectableLabel 默认 fill 冲突）
            if is_sel {
                let rect = resp.rect.shrink(1.0);
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(6),
                    egui::Stroke::new(1.5, accent),
                    egui::StrokeKind::Middle,
                );
                let _ = text_color;
            }
        }
    });
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
