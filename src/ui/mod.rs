//! UI 模块聚合 + 共享主题常量。

pub mod fonts;
pub mod panel_about;
pub mod panel_connection;
pub mod panel_keymap;
pub mod panel_lighting;
pub mod panel_log;
pub mod panel_settings;
pub mod panel_voice;
pub mod panel_wifi;
pub mod sidenav;
pub mod statusbar;
pub mod topbar;
pub mod widgets;

use eframe::egui;

/// 全局配色（与 ui-design.md §6.3 对齐）
pub mod colors {
    use eframe::egui::Color32;

    pub const TX: Color32 = Color32::from_rgb(120, 180, 240); // 浅蓝
    pub const RX: Color32 = Color32::from_rgb(140, 220, 140); // 浅绿
    pub const FW_INFO: Color32 = Color32::from_rgb(180, 180, 180); // 浅灰
    pub const FW_WARN: Color32 = Color32::from_rgb(230, 180, 90); // 浅黄
    pub const FW_ERROR: Color32 = Color32::from_rgb(230, 100, 100); // 浅红
    pub const APP: Color32 = Color32::from_rgb(220, 220, 220); // 白

    pub const STATUS_GREY: Color32 = Color32::from_rgb(150, 150, 150);
    pub const STATUS_YELLOW: Color32 = Color32::from_rgb(230, 200, 90);
    pub const STATUS_GREEN: Color32 = Color32::from_rgb(90, 200, 90);
    pub const STATUS_RED: Color32 = Color32::from_rgb(220, 80, 80);
}

/// 渲染一个状态灯圆点
pub fn status_dot(ui: &mut egui::Ui, color: eframe::egui::Color32) {
    let (r, painter) = ui.allocate_painter(egui::Vec2::new(12.0, 12.0), egui::Sense::hover());
    painter.circle_filled(r.rect.center(), 5.0, color);
}

/// 卡片容器：内容页分区统一使用（圆角 + 底色 + 细边框），自动撑满可用宽度。
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 12,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// 品牌主色（按钮高亮、选中态、Toast 等）
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x4F, 0x8C, 0xFF);

/// 按本地配置应用全局主题（在 fonts::install 之后、首个窗口创建前调用）
pub fn apply_theme(ctx: &egui::Context, theme: crate::config::Theme) {
    let mut vis = match theme {
        crate::config::Theme::Dark => egui::Visuals::dark(),
        crate::config::Theme::Light => egui::Visuals::light(),
    };

    // 选中态 / 链接使用品牌色
    vis.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    vis.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    vis.hyperlink_color = ACCENT;
    vis.widgets.hovered.expansion = 2.0;

    match theme {
        crate::config::Theme::Dark => {
            vis.panel_fill = egui::Color32::from_rgb(0x1B, 0x1F, 0x26);
            vis.window_fill = egui::Color32::from_rgb(0x20, 0x25, 0x2D);
            vis.extreme_bg_color = egui::Color32::from_rgb(0x12, 0x15, 0x1A);
            vis.faint_bg_color = egui::Color32::from_rgb(0x24, 0x2A, 0x33);
        }
        crate::config::Theme::Light => {
            vis.panel_fill = egui::Color32::from_rgb(0xF2, 0xF4, 0xF8);
            vis.window_fill = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
            vis.extreme_bg_color = egui::Color32::from_rgb(0xE4, 0xE7, 0xEC);
        }
    }

    // 圆角：窗口 10，控件 6
    vis.window_corner_radius = egui::CornerRadius::same(10);
    vis.menu_corner_radius = egui::CornerRadius::same(8);
    for w in [
        &mut vis.widgets.noninteractive,
        &mut vis.widgets.inactive,
        &mut vis.widgets.hovered,
        &mut vis.widgets.active,
        &mut vis.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(6);
    }
    ctx.set_visuals(vis);

    // 间距与字号
    ctx.style_mut(|style| {
        let sp = &mut style.spacing;
        sp.item_spacing = egui::vec2(8.0, 8.0);
        sp.button_padding = egui::vec2(12.0, 5.0);
        sp.menu_margin = egui::Margin {
            left: 10,
            right: 10,
            top: 6,
            bottom: 6,
        };
        if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Heading) {
            f.size = 22.0;
        }
        if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Body) {
            f.size = 15.0;
        }
    });
}
