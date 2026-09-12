//! P8 About 页：应用版本 + 已连接设备的设备信息 + 当前选中端口的厂商信息。

use eframe::egui;

use crate::state::AppHandle;
use crate::ui::topbar::ConnectPanelState;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, connect_st: &ConnectPanelState) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading("EKeys 桌面端");
        ui.add_space(8.0);
        ui.label(format!("版本  {}（wxi）", env!("CARGO_PKG_VERSION")));
        ui.label("适配固件协议  v0.1（阶段 04）");
        ui.label("对接设备  ESP32-S3 USB CDC（VID 0x303A）");
        ui.add_space(16.0);
        ui.label("由 Rust 与 egui 框架构建");
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

    ui.add_space(12.0);

    // 当前选中端口的厂商信息（USB 描述符）
    //
    // 来源：顶栏 port_combo 当前选中项对应的 PortInfo；无论是否已连接都会展示，
    // 这样未连接时也能查看「这台机器上有哪些端口、各自的厂商是什么」。
    let port_view: Option<(String, crate::link::PortInfo)> = match &connect_st.selected {
        Some(name) => connect_st
            .ports
            .iter()
            .find(|p| &p.name == name)
            .cloned()
            .map(|p| (name.clone(), p)),
        None => None,
    };

    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.strong("当前选中端口");
            ui.add_space(8.0);

            let Some((name, p)) = port_view else {
                ui.label("（未选择端口）");
                return;
            };

            egui::Grid::new("about_port_info")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("端口名");
                    ui.label(name);
                    ui.end_row();

                    ui.label("厂商 ID");
                    ui.label(
                        p.vid
                            .map(|v| format!("0x{v:04X}"))
                            .unwrap_or_else(|| "—".into()),
                    );
                    ui.end_row();

                    ui.label("产品 ID");
                    ui.label(
                        p.pid
                            .map(|v| format!("0x{v:04X}"))
                            .unwrap_or_else(|| "—".into()),
                    );
                    ui.end_row();

                    ui.label("厂商");
                    ui.label(p.manufacturer.clone().unwrap_or_else(|| "—".into()));
                    ui.end_row();

                    ui.label("产品");
                    ui.label(p.product.clone().unwrap_or_else(|| "—".into()));
                    ui.end_row();

                    ui.label("序列号");
                    ui.label(p.serial_number.clone().unwrap_or_else(|| "—".into()));
                    ui.end_row();
                });
        });
}
