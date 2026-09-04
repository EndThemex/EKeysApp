//! 字体注册：从 main.rs 抽离出来。
//!
//! ## 图标 + 文本混排的正确姿势
//!
//! Phosphor 图标字形位于 PUA（U+E0xx ~ U+EExx），挂在一个独立的命名字体族
//! `"phosphor"` 里，**不会**进入 Proportional 的 fallback 链——因为 Phosphor 对
//! 小写字母只有零宽占位字形，会把 "WiFi" 之类文本吞成 "WF"。
//!
//! 因此，包含图标字面量的字符串**不能**直接交给 `ui.button` / `ui.label` 等普通
//! 控件渲染：那样字形会落到 Proportional（maple_cn + 系统字体），这些字体里没有
//! PUA 码位，于是显示成 □。
//!
//! 正确做法是把图标部分用 `icon_font_id` 渲染、普通文本用 Proportional 渲染，两
//! 段作为独立的 galley 拼到一行。本文件提供以下 helper：
//!
//! - [`icon_rich`] / [`icon_only`] — 仅渲染一个图标字符（用于 icon-only 按钮）。
//! - [`paint_icon_text_in`] — 在给定 `Rect` 内绘制“图标 + 文本”。
//! - [`IconTextButton`] — 一个 `egui::Widget`，行为和 `ui.button(format!(...))`
//!   一致，但内部用两个 galley 混排，等价于把 `ui.button(...)` 改造成支持
//!   Phosphor 图标。

use std::sync::Arc;

use eframe::egui;

const FONT_BYTES: &[u8] = include_bytes!("../../fonts/MapleMono-NF-CN-ExtraLight.ttf");

/// 图标专用的命名字体族名称。配合 `FontId::new(size, FontFamily::Name("phosphor".into()))`
/// 使用，让 Phosphor 图标字形不进入文本字体的 fallback 链，避免它污染普通文本
/// （Phosphor 对小写字母只有 width=0 占位字形，会把 "WiFi" 渲染成 "WF"）。
pub const ICON_FAMILY_NAME: &str = "phosphor";

/// 便捷函数：构造一个图标专用的 `FontId`。
pub fn icon_font_id(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(ICON_FAMILY_NAME.into()))
}

/// 构造一个 `RichText`，里面只放图标字符，渲染时会使用 Phosphor 命名字体族。
/// 用于“纯图标按钮/标签”。
///
/// ```ignore
/// if ui.add(egui::Button::new(icon_rich(crate::ui::icons::GEAR, 16.0))).clicked() {
///     // ...
/// }
/// ```
pub fn icon_rich(icon: &str, size: f32) -> egui::RichText {
    egui::RichText::new(icon).font(icon_font_id(size))
}

/// 便捷：`ui.add(egui::Button::new(icon_rich(icon, size)))` 的薄封装，返回 `Response`。
pub fn icon_only(ui: &mut egui::Ui, icon: &str, size: f32) -> egui::Response {
    ui.add(egui::Button::new(icon_rich(icon, size)))
}

/// 在 `rect` 内绘制“图标 + 空格 + 文本”，并以 `color` 上色。
///
/// 图标用 Phosphor 字体，文本用 Proportional；二者各自 layout 成独立 galley 后
/// 按 `[icon | gap | text]` 顺序水平摆放。`gap` 默认 6.0。
///
/// 垂直方向按 **光学中心对齐**：以每段 galley 的 `mesh_bounds`（字形实际渲染范围
/// 的紧致包围盒）的竖直中心为锚点，把图标与文字的视觉中心都对到 `rect` 的竖直
/// 中线上。相比按字体 metrics 估算基线，这种方式不依赖 ascent/descent 假设，
/// 图标和文字不会上下错开。
pub fn paint_icon_text_in(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    icon: &str,
    text: &str,
    size: f32,
    color: egui::Color32,
    gap: f32,
) -> f32 {
    let icon_font = icon_font_id(size);
    let text_font = egui::FontId::proportional(size);

    let icon_galley =
        ui.painter()
            .layout(icon.to_string(), icon_font.clone(), color, f32::INFINITY);
    let label_galley =
        ui.painter()
            .layout(text.to_string(), text_font.clone(), color, f32::INFINITY);

    let icon_w = icon_galley.rect.width();
    let total_w = icon_w + gap + label_galley.rect.width();
    let start_x = rect.left() + (rect.width() - total_w).max(0.0) * 0.5;

    // 光学居中：两段 galley 的字形中心都对齐到 rect 竖直中线
    let center_y = rect.center().y;
    let icon_top = center_y - galley_mesh_center_y(&icon_galley);
    let text_top = center_y - galley_mesh_center_y(&label_galley);

    ui.painter()
        .galley(egui::pos2(start_x, icon_top), icon_galley.into(), color);
    ui.painter().galley(
        egui::pos2(start_x + icon_w + gap, text_top),
        label_galley.into(),
        color,
    );

    total_w
}

/// galley 内部字形紧致包围盒（mesh_bounds）的竖直中心（galley 本地坐标）。
/// 空文本等 mesh_bounds 为空时退化为 rect 中心。
fn galley_mesh_center_y(g: &egui::Galley) -> f32 {
    if g.mesh_bounds.is_positive() {
        g.mesh_bounds.center().y
    } else {
        g.rect.center().y
    }
}

/// 一个“图标 + 文本”按钮：`egui::Widget` 实现。
///
/// 用法和 `ui.button(format!("{icon} {text}"))` 完全等价，但内部用两个 galley
/// 混排——图标用 Phosphor，文本用 Proportional——从而保证图标字形不被 Proportional
/// 字体“吃掉”。
///
/// 可选参数：
/// - `fill` / `stroke` —— 自定义背景与边框，未指定时跟随 egui `Visuals`。
/// - `min_size` —— 按钮最小尺寸，默认 `vec2(0.0, 0.0)`，通常直接交给 layout 自然撑开。
///
/// 示例：
/// ```ignore
/// if ui.add(IconTextButton::new(
///     crate::ui::icons::REFRESH,
///     "刷新",
///     14.0,
/// )).clicked() { ... }
/// ```
pub struct IconTextButton {
    icon: String,
    text: String,
    size: f32,
    gap: f32,
    fill: Option<egui::Color32>,
    stroke: Option<egui::Stroke>,
    fg: Option<egui::Color32>,
    min_size: egui::Vec2,
    corner_radius: Option<egui::CornerRadius>,
    frame: bool,
    selected: bool,
}

impl IconTextButton {
    pub fn new(icon: &str, text: impl Into<String>, size: f32) -> Self {
        Self {
            icon: icon.to_string(),
            text: text.into(),
            size,
            gap: 6.0,
            fill: None,
            stroke: None,
            fg: None,
            min_size: egui::Vec2::ZERO,
            corner_radius: None,
            frame: true,
            selected: false,
        }
    }

    /// 图标与文本之间的像素间距（默认 6.0）。
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// 按钮背景色，`None` 表示跟随 `Visuals::widgets.inactive.bg_fill`。
    pub fn fill(mut self, fill: egui::Color32) -> Self {
        self.fill = Some(fill);
        self
    }

    /// 按钮边框。`None` 表示跟随 `Visuals`。
    pub fn stroke(mut self, stroke: egui::Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// 显式指定图标 + 文本的颜色。`None` 表示跟随 `Visuals`。
    pub fn fg(mut self, fg: egui::Color32) -> Self {
        self.fg = Some(fg);
        self
    }

    /// 最小尺寸（含 padding）。
    pub fn min_size(mut self, size: egui::Vec2) -> Self {
        self.min_size = size;
        self
    }

    /// 自定义圆角；不指定时跟随 `Visuals`。
    pub fn corner_radius(mut self, radius: egui::CornerRadius) -> Self {
        self.corner_radius = Some(radius);
        self
    }

    /// 是否绘制默认按钮边框/背景。`false` 时退化为纯可点击矩形。
    pub fn frame(mut self, frame: bool) -> Self {
        self.frame = frame;
        self
    }

    /// `true` 时使用主题的 selection 配色（语义同 `Button::selectable`）。
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl egui::Widget for IconTextButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self {
            icon,
            text,
            size,
            gap,
            fill,
            stroke,
            fg,
            min_size,
            corner_radius,
            frame,
            selected,
        } = self;

        // 1) 先 layout 出两段 galley，需要用 text_color 提前着色。
        let text_color = ui.style().visuals.text_color();
        let icon_font = icon_font_id(size);
        let text_font = egui::FontId::proportional(size);
        let icon_galley =
            ui.painter()
                .layout(icon.clone(), icon_font.clone(), text_color, f32::INFINITY);
        let label_galley =
            ui.painter()
                .layout(text.clone(), text_font.clone(), text_color, f32::INFINITY);

        // 2) 按内容 + button_padding 计算期望尺寸。
        let pad = ui.spacing().button_padding;
        let icon_w = icon_galley.rect.width();
        let label_w = label_galley.rect.width();
        let inner_w = icon_w + gap + label_w;
        let inner_h = icon_galley.rect.height().max(label_galley.rect.height());
        let desired = egui::vec2(
            (inner_w + pad.x * 2.0).max(min_size.x),
            (inner_h + pad.y * 2.0).max(min_size.y),
        );

        // 3) 分配响应区。
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let rounding = corner_radius.unwrap_or(visuals.corner_radius);

            // 主题 selection 调色板（供 selected=true 的卡片使用）
            let sel_bg = ui.style().visuals.selection.bg_fill;
            let sel_stroke = ui.style().visuals.selection.stroke;
            let sel_fg = ui.style().visuals.selection.stroke.color;

            let (bg, bg_stroke, fg) = if response.is_pointer_button_down_on() {
                (
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                    fg.unwrap_or_else(|| visuals.text_color()),
                )
            } else if response.hovered() {
                (
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                    fg.unwrap_or_else(|| visuals.text_color()),
                )
            } else if selected {
                (sel_bg, sel_stroke, fg.unwrap_or(sel_fg))
            } else {
                (
                    fill.unwrap_or(visuals.bg_fill),
                    stroke.unwrap_or(visuals.bg_stroke),
                    fg.unwrap_or_else(|| visuals.text_color()),
                )
            };

            if frame {
                ui.painter().rect_filled(rect, rounding, bg);
                ui.painter()
                    .rect_stroke(rect, rounding, bg_stroke, egui::StrokeKind::Middle);
            }

            // 内容绘制区
            let inner_rect = rect.shrink2(pad);
            let total_w = icon_w + gap + label_w;
            let start_x = inner_rect.left() + (inner_rect.width() - total_w).max(0.0) * 0.5;

            // 光学居中：两段 galley 的字形中心都对齐到 inner_rect 竖直中线，
            // 保证图标与文字处于同一视觉高度（不依赖字体 metrics 估算）。
            let center_y = inner_rect.center().y;
            let icon_top = center_y - galley_mesh_center_y(&icon_galley);
            let text_top = center_y - galley_mesh_center_y(&label_galley);

            ui.painter()
                .galley(egui::pos2(start_x, icon_top), icon_galley.into(), fg);
            ui.painter().galley(
                egui::pos2(start_x + icon_w + gap, text_top),
                label_galley.into(),
                fg,
            );
        }

        response
    }
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 1) 注册 Maple Mono CN 字体（中文等宽 + UI 文本）
    fonts.font_data.insert(
        "maple_cn".to_owned(),
        Arc::new(egui::FontData::from_static(FONT_BYTES)),
    );

    // 2) 注册 Phosphor 图标字体（regular variant）。
    //    注意：egui-phosphor 0.11 的 add_to_fonts 只会把 Phosphor 塞进 Proportional 的 fallback，
    //    这会让所有经过 Proportional 的文本（包括标签）都被 Phosphor “过一遍”，
    //    进而出现图标不一致 / 普通字符被吞等诡异问题。
    //    所以我们只取它的字节，不调用 add_to_fonts 的家族挂载，而是手动注册到独立命名族。
    let variant = egui_phosphor::Variant::Regular;
    let font_data = variant.font_data();
    fonts
        .font_data
        .insert(ICON_FAMILY_NAME.to_owned(), Arc::new(font_data));

    // 创建独立的命名字体族：以 Phosphor 为主，渲染图标时使用 icon_font_id(size)。
    // 追加 maple_cn 作为兜底：Phosphor 是纯图标字体，不含 '◻'/'?' 等替代字形，
    // egui 在首次创建该字体族时会因找不到替代字形而告警
    // "Failed to find replacement characters '◻' or '?'". 挂上 maple_cn 后
    // 替代字形可解析（且误入该族的文本字符也能降级显示，而非渲染成空）。
    // 不影响普通文本：Proportional 族不包含 Phosphor。
    {
        let icon_family = fonts
            .families
            .entry(egui::FontFamily::Name(ICON_FAMILY_NAME.into()))
            .or_default();
        if !icon_family.iter().any(|k| k == ICON_FAMILY_NAME) {
            icon_family.push(ICON_FAMILY_NAME.to_owned());
        }
        if !icon_family.iter().any(|k| k == "maple_cn") {
            icon_family.push("maple_cn".to_owned());
        }
    }

    // 重排 Proportional 字体优先级：仅保留 maple_cn + 原有系统字体。
    //   [maple_cn, ...原有]
    // 关键：Phosphor 必须从此处移除，否则下面两类问题会持续出现：
    //   (a) "WiFi" 等文本里的 'i' 被 Phosphor 的零宽字形吞掉；
    //   (b) 任何混排字符串（图标 + 文本）都会先尝试 Phosphor，导致图标渲染依赖顺序。
    let prop = fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap();
    prop.retain(|k| k != "Proportional" && k != ICON_FAMILY_NAME && k != "maple_cn");
    prop.insert(0, "maple_cn".to_owned());

    // Monospace：等宽场景仍用 Maple Mono（中文等宽显示）
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("maple_cn".to_owned());

    ctx.set_fonts(fonts);
}
