//! P8 About 页：显示应用版本 + 已连接设备的设备信息。

use eframe::egui;

use crate::state::AppHandle;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui) {
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

    ui.add_space(24.0);
    ui.separator();
    ui.add_space(12.0);

    // 设备信息：连接后由 0x03 CMD_DEVICE_INFO_GET 刷新
    let info = handle.device_info.lock().unwrap().clone();
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.strong("当前设备");
            ui.add_space(8.0);
            egui::Grid::new("about_device_info")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("名称");
                    ui.label(if info.device_name.is_empty() {
                        "（未拉取）"
                    } else {
                        info.device_name.as_str()
                    });
                    ui.end_row();

                    ui.label("设备 ID");
                    ui.label(if info.device_id.is_empty() {
                        "—"
                    } else {
                        info.device_id.as_str()
                    });
                    ui.end_row();

                    ui.label("固件版本");
                    ui.label(if info.firmware_version.is_empty() {
                        "（未拉取）"
                    } else {
                        info.firmware_version.as_str()
                    });
                    ui.end_row();
                });
        });
}
