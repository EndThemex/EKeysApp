//! P4 Lighting 页面：键盘 RGB 灯效。
//!
//! 字段：rgb_mode / rgb_single_color / rgb_click_mode / rgb_brightness
//! 协议阶段 06（设备侧未接入前 UI 可改但不生效）

use eframe::egui;

use crate::i18n;
use crate::state::AppHandle;
use crate::ui::widgets::settings_panel_scaffold;

#[derive(Default)]
pub struct LightingPanelState;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, _st: &mut LightingPanelState) {
    ui.heading(i18n::t("lighting.title"));
    ui.label(
        egui::RichText::new(i18n::t("lighting.desc"))
            .weak()
            .size(12.0),
    );
    ui.add_space(6.0);

    settings_panel_scaffold(handle, ui, |ui, _snap, draft| {
        ui.group(|ui| {
            // 整体亮度放在最上方：作为整个键盘灯亮度的总控，影响所有灯效模式的实际亮度输出
            ui.label(i18n::t("lighting.brightness"))
                .on_hover_text(i18n::t("lighting.brightness.hint"));
            let mut bri = draft.rgb_brightness.clamp(0, 100);
            if ui
                .add(egui::Slider::new(&mut bri, 0..=100).show_value(true))
                .changed()
            {
                draft.rgb_brightness = bri;
            }

            ui.add_space(8.0);
            // 与固件 RGBMode 枚举（RGBLightControl.h:22）严格 1:1 对齐
            ui.label(i18n::t("lighting.mode"));
            let mut mode = draft.rgb_mode;
            let modes: &[(i32, &str, &str, &str)] = &[
                (
                    0,
                    i18n::t("lighting.mode.off"),
                    crate::ui::icons::LIGHT_OFF,
                    i18n::t("lighting.mode.off.tip"),
                ),
                (
                    1,
                    i18n::t("lighting.mode.solid"),
                    crate::ui::icons::LIGHT_SOLID,
                    i18n::t("lighting.mode.solid.tip"),
                ),
                (
                    2,
                    i18n::t("lighting.mode.rainbow"),
                    crate::ui::icons::LIGHT_RAINBOW,
                    i18n::t("lighting.mode.rainbow.tip"),
                ),
                (
                    3,
                    i18n::t("lighting.mode.wave"),
                    crate::ui::icons::LIGHT_WAVE,
                    i18n::t("lighting.mode.wave.tip"),
                ),
                (
                    4,
                    i18n::t("lighting.mode.cycle"),
                    crate::ui::icons::LIGHT_CYCLE,
                    i18n::t("lighting.mode.cycle.tip"),
                ),
                (
                    5,
                    i18n::t("lighting.mode.meter"),
                    crate::ui::icons::LIGHT_METER,
                    i18n::t("lighting.mode.meter.tip"),
                ),
                (
                    6,
                    i18n::t("lighting.mode.fire"),
                    crate::ui::icons::LIGHT_FIRE,
                    i18n::t("lighting.mode.fire.tip"),
                ),
                (
                    7,
                    i18n::t("lighting.mode.pulse"),
                    crate::ui::icons::LIGHT_PULSE,
                    i18n::t("lighting.mode.pulse.tip"),
                ),
                (
                    8,
                    i18n::t("lighting.mode.sound"),
                    crate::ui::icons::LIGHT_SOUND,
                    i18n::t("lighting.mode.sound.tip"),
                ),
                (
                    9,
                    i18n::t("lighting.mode.rhythm"),
                    crate::ui::icons::LIGHT_SOUND,
                    i18n::t("lighting.mode.rhythm.tip"),
                ),
                (
                    10,
                    i18n::t("lighting.mode.gradient"),
                    crate::ui::icons::LIGHT_SOUND,
                    i18n::t("lighting.mode.gradient.tip"),
                ),
            ];
            mode_grid(ui, modes, &mut mode, crate::ui::ACCENT);
            if mode != draft.rgb_mode {
                draft.rgb_mode = mode;
            }

            ui.add_space(8.0);
            // rgb_single_color 在固件中是 24 色调色板索引
            // （RGBLightControl.cpp:74 `single_index_ = snap.rgb_single_color % 24`）；
            // 固件 uint8_t 接受 0~255，但 24 之外的索引会被 %24 兜底，
            // 这里把 UI 范围收紧到 0~23 与调色板 1:1 对齐，避免越界值被静默绕回。
            ui.horizontal(|ui| {
                ui.label(i18n::t("lighting.single_color"));
                ui.label(
                    egui::RichText::new(i18n::t("lighting.single_color.hint"))
                        .weak()
                        .size(11.0),
                );
            });
            let mut color = draft.rgb_single_color.clamp(0, 23);
            // 当前索引对应的实际颜色（与固件 kPalette24 严格 1:1），提前取出供 hover 文本使用
            let (r, g, b) = PALETTE24[color as usize];
            let row = ui.horizontal(|ui| {
                if ui
                    .add(egui::Slider::new(&mut color, 0..=23).show_value(true))
                    .changed()
                {
                    draft.rgb_single_color = color;
                }
                // 当前索引对应的实际颜色色块（与固件 kPalette24 严格 1:1）
                let preview_size = egui::vec2(20.0, 20.0);
                let (rect, _) = ui.allocate_exact_size(preview_size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(3), palette_color(r, g, b));
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(3),
                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    egui::StrokeKind::Middle,
                );
                ui.label(format!("#{:02X}{:02X}{:02X}", r, g, b));
            });
            row.response.on_hover_text(
                i18n::t("lighting.palette.tip")
                    .replace("{index}", &color.to_string())
                    .replace("{r}", &r.to_string())
                    .replace("{g}", &g.to_string())
                    .replace("{b}", &b.to_string()),
            );

            ui.add_space(8.0);
            // 与固件 ClickHighlight.h:34 严格 1:1
            ui.horizontal(|ui| {
                ui.label(i18n::t("lighting.click.title"));
                ui.label(
                    egui::RichText::new(i18n::t("lighting.click.subtitle"))
                        .weak()
                        .size(11.0),
                );
            });
            let mut cm = draft.rgb_click_mode;
            let clicks: &[(i32, &str, &str, &str)] = &[
                (
                    0,
                    i18n::t("lighting.mode.off"),
                    crate::ui::icons::LIGHT_NONE,
                    i18n::t("lighting.click.off.tip"),
                ),
                (
                    1,
                    i18n::t("lighting.mode.solid"),
                    crate::ui::icons::LIGHT_CLICK,
                    i18n::t("lighting.click.solid.tip"),
                ),
                (
                    2,
                    i18n::t("lighting.mode.gradient"),
                    crate::ui::icons::LIGHT_WARE,
                    i18n::t("lighting.click.gradient.tip"),
                ),
            ];
            mode_grid(ui, clicks, &mut cm, crate::ui::ACCENT);
            if cm != draft.rgb_click_mode {
                draft.rgb_click_mode = cm;
            }
        });
    });
}

/// 灯效选择网格：把每个选项渲染成一张卡片，点击即选中。
///
/// `items` 中每个元素为 `(value, label, icon, tooltip)`：
/// - `value`   选中后写入的数值
/// - `label`   卡片主标题
/// - `icon`    卡片顶部大号图标
/// - `tooltip` hover 时显示的说明（解释该模式的实际效果）
///
/// `selected` 为当前值，函数会把它就地改为用户点击的项。
/// `accent`  为主题强调色，用于高亮当前选中卡片。
fn mode_grid(
    ui: &mut egui::Ui,
    items: &[(i32, &str, &str, &str)],
    selected: &mut i32,
    accent: egui::Color32,
) {
    let card_w = 72.0;
    let card_h = 64.0;
    let spacing = 6.0;

    // 用 wrap 自动按可用宽度折行，避免手算列数 + end_row 引发 Grid cell 分配异常
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
        for (val, label, icon, tooltip) in items.iter() {
            let is_sel = *selected == *val;
            // 用 IconTextButton 做单选语义：图标走 Phosphor 字体、文本走 Proportional，
            // 不会被“混排字符串”吃掉图标字形。
            let resp = ui
                .add(
                    crate::ui::fonts::IconTextButton::new(*icon, *label, 14.0)
                        .selected(is_sel)
                        .min_size(egui::vec2(card_w, card_h))
                        .gap(6.0),
                )
                .on_hover_text(*tooltip);
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

/// 24 色调色板（与固件 RGBLightControl.h:40 `kPalette24` 严格 1:1）。
///
/// 固件在 `RGBLightControl.cpp:74` 用 `% 24` 兜底，因此 UI 索引也应严格落在 0~23 范围内。
const PALETTE24: [(u8, u8, u8); 24] = [
    (255, 255, 255),
    (255, 0, 0),
    (255, 64, 0),
    (255, 128, 0),
    (255, 192, 0),
    (255, 255, 0),
    (192, 255, 0),
    (128, 255, 0),
    (64, 255, 0),
    (0, 255, 0),
    (0, 255, 64),
    (0, 255, 128),
    (0, 255, 192),
    (0, 255, 255),
    (0, 192, 255),
    (0, 128, 255),
    (0, 64, 255),
    (0, 0, 255),
    (64, 0, 255),
    (128, 0, 255),
    (192, 0, 255),
    (255, 0, 255),
    (255, 0, 192),
    (255, 0, 128),
];

fn palette_color(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}
