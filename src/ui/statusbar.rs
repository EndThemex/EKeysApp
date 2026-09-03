//! 底部状态条：Uptime、收发计数、心跳间隔。
//! （连接状态由顶栏胶囊展示，这里只放遥测信息，避免重复。）

use eframe::egui;

use crate::state::AppHandle;

fn icon_label(ui: &mut egui::Ui, icon: &str, text: &str) {
    // 图标走 Phosphor 字体族，文本走 Proportional，二者不共享同一次 text()，
    // 避免 Phosphor 对小写字母的零宽字形污染普通字符（参见 fonts.rs 注释）。
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).font(crate::ui::fonts::icon_font_id(12.0)));
        ui.label(egui::RichText::new(text).weak());
    });
}

pub fn show(_handle: &AppHandle, ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        ui.style_mut().spacing.item_spacing.x = 14.0;
        icon_label(ui, crate::ui::icons::UPTIME, "运行时长: -"); // 阶段 04 未解析心跳 timestamp
        icon_label(ui, crate::ui::icons::TX, "已发送 0");
        icon_label(ui, crate::ui::icons::RX, "已接收 0");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            icon_label(ui, crate::ui::icons::HEARTBEAT, "心跳 1秒");
        });
    });
}
