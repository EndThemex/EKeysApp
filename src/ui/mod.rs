//! UI 模块聚合 + 共享主题常量。

pub mod fonts;
pub mod panel_about;
pub mod panel_connection;
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
