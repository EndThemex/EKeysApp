//! P4 Lighting 页面：键盘 RGB 灯效。
//!
//! 字段：rgb_mode / rgb_single_colar / rgb_click_mode / rgb_brightness
//! 协议阶段 06（设备侧未接入前 UI 可改但不生效）

use eframe::egui;

use crate::state::AppHandle;
use crate::ui::widgets::settings_panel_scaffold;

#[derive(Default)]
pub struct LightingPanelState;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, _st: &mut LightingPanelState) {
    ui.heading("灯效");
    ui.label("阶段 06 生效：键盘 RGB 灯效");
    ui.add_space(4.0);

    settings_panel_scaffold(handle, ui, |ui, _snap, draft| {
        ui.group(|ui| {
            ui.label("RGB 模式");
            let mut mode = draft.rgb_mode;
            let modes: &[(i32, &str, &str)] = &[
                (0, "关闭", crate::ui::icons::LIGHT_OFF),
                (1, "静态单色", crate::ui::icons::LIGHT_SOLID),
                (2, "流光", crate::ui::icons::LIGHT_FLOW),
                (3, "呼吸", crate::ui::icons::LIGHT_BREATHE),
                (4, "按键触发", crate::ui::icons::LIGHT_CLICK),
                (5, "彩虹", crate::ui::icons::LIGHT_RAINBOW),
            ];
            mode_grid(ui, modes, &mut mode, crate::ui::ACCENT);
            if mode != draft.rgb_mode {
                draft.rgb_mode = mode;
            }

            ui.add_space(6.0);
            ui.label("单色色值（0~255）");
            let mut colar = draft.rgb_single_colar;
            if ui
                .add(egui::Slider::new(&mut colar, 0..=255).show_value(true))
                .changed()
            {
                draft.rgb_single_colar = colar;
            }

            ui.add_space(6.0);
            ui.label("按键触发模式");
            let mut cm = draft.rgb_click_mode;
            let clicks: &[(i32, &str, &str)] = &[
                (0, "无", crate::ui::icons::LIGHT_NONE),
                (1, "按下时亮", crate::ui::icons::LIGHT_CLICK),
                (2, "按下闪一下", crate::ui::icons::LIGHT_FLASH),
                (3, "按下渐变", crate::ui::icons::LIGHT_FADE),
            ];
            mode_grid(ui, clicks, &mut cm, crate::ui::ACCENT);
            if cm != draft.rgb_click_mode {
                draft.rgb_click_mode = cm;
            }

            ui.add_space(6.0);
            ui.label("灯效亮度（0~100）");
            let mut bri = draft.rgb_brightness.clamp(0, 100);
            if ui
                .add(egui::Slider::new(&mut bri, 0..=100).show_value(true))
                .changed()
            {
                draft.rgb_brightness = bri;
            }
        });
    });
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
    let card_w = 72.0;
    let card_h = 64.0;
    let spacing = 6.0;

    // 用 wrap 自动按可用宽度折行，避免手算列数 + end_row 引发 Grid cell 分配异常
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
        for (val, label, icon) in items.iter() {
            let is_sel = *selected == *val;
            // 用 IconTextButton 做单选语义：图标走 Phosphor 字体、文本走 Proportional，
            // 不会被“混排字符串”吃掉图标字形。
            let resp = ui.add(
                crate::ui::fonts::IconTextButton::new(*icon, *label, 14.0)
                    .selected(is_sel)
                    .min_size(egui::vec2(card_w, card_h))
                    .gap(6.0),
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && !is_sel {
                *selected = *val;
            }
            // 选中时叠一层强调色（避免与 selection 默认 fill 冲突）
            if is_sel {
                let rect = resp.rect.shrink(1.0);
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(6),
                    egui::Stroke::new(1.5, accent),
                    egui::StrokeKind::Middle,
                );
            }
        }
    });
}
