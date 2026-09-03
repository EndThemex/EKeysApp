//! P8 About 页。

use eframe::egui;

pub fn show(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading("EKeys Desktop App");
        ui.add_space(8.0);
        ui.label(format!("版本  {}  (wxi)", env!("CARGO_PKG_VERSION")));
        ui.label("固件协议  v0.1（阶段 04）");
        ui.label("对接设备  ESP32-S3 USB CDC (VID 0x303A)");
        ui.add_space(16.0);
        ui.label("基于 eframe + egui + Rust 2024 构建");
    });
}
