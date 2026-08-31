//! 底部状态条：Uptime、收发计数、心跳间隔。

use eframe::egui;

use crate::state::AppHandle;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui) {
    let s = handle.state.lock().unwrap().clone();
    egui::menu::bar(ui, |ui| {
        ui.style_mut().spacing.item_spacing.x = 12.0;
        let online = matches!(s, crate::link::ConnectionState::Online);
        let (icon, text, color) = if online {
            ("●", "Online", crate::ui::colors::STATUS_GREEN)
        } else {
            ("○", "Offline", crate::ui::colors::STATUS_GREY)
        };
        ui.label(egui::RichText::new(icon).color(color));
        ui.label(egui::RichText::new(text).color(ui.visuals().text_color().gamma_multiply(0.8)));
        ui.separator();
        ui.label(egui::RichText::new("⏱ Uptime: -").weak()); // 阶段 04 未解析心跳 timestamp
        ui.separator();
        ui.label(egui::RichText::new("📤 0 sent").weak());
        ui.label(egui::RichText::new("📥 0 recv").weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("❤️ heartbeat 1s").weak());
        });
    });
}
