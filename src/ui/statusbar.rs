//! 底部状态条：Uptime、收发计数、心跳间隔。
//! （连接状态由顶栏胶囊展示，这里只放遥测信息，避免重复。）

use eframe::egui;

use crate::state::AppHandle;

pub fn show(_handle: &AppHandle, ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        ui.style_mut().spacing.item_spacing.x = 14.0;
        ui.label(egui::RichText::new("⏱ Uptime: -").weak()); // 阶段 04 未解析心跳 timestamp
        ui.label(egui::RichText::new("📤 0 sent").weak());
        ui.label(egui::RichText::new("📥 0 recv").weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("❤️ heartbeat 1s").weak());
        });
    });
}
