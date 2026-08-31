//! 底部状态条：Uptime、收发计数、心跳间隔。

use eframe::egui;

use crate::state::AppHandle;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui) {
    let s = handle.state.lock().unwrap().clone();
    egui::menu::bar(ui, |ui| {
        let online = matches!(s, crate::link::ConnectionState::Online);
        ui.label(if online {
            "● Online"
        } else {
            "○ Offline"
        });
        ui.separator();
        ui.label("⏱ Uptime: -"); // 阶段 04 未解析心跳 timestamp
        ui.separator();
        ui.label("📤 0 sent");
        ui.label("📥 0 recv");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label("❤️ heartbeat 1s");
        });
    });
}
