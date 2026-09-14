//! P5 WiFi 页面：WiFi 配置。
//!
//! 字段：wifi_switch / connect_host / wifi_ssid / wifi_password / wifi_*
//! 协议 §4.1：ssid ≤32 字节，password ≤64 字节（过长自动截断在固件侧处理）

use eframe::egui;

use crate::state::AppHandle;
use crate::ui::widgets::settings_panel_scaffold;

#[derive(Default)]
pub struct WifiPanelState;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, _st: &mut WifiPanelState) {
    ui.heading("无线网络");
    ui.label("无线网络配置（修改后下次重启生效）");
    ui.add_space(4.0);

    settings_panel_scaffold(handle, ui, |ui, snapshot, draft| {
        ui.group(|ui| {
            ui.label("无线网络开关");
            let mut on = if draft.wifi_switch != 0 || snapshot.wifi_switch != 0 {
                draft.wifi_switch != 0
            } else {
                snapshot.wifi_switch != 0
            };
            if ui.checkbox(&mut on, "启用无线网络").changed() {
                draft.wifi_switch = if on { 1 } else { 0 };
            }

            ui.add_space(6.0);
            ui.label("与电脑的双向连接");
            let mut host = if draft.connect_host != 0 || snapshot.connect_host != 0 {
                draft.connect_host != 0
            } else {
                snapshot.connect_host != 0
            };
            if ui.checkbox(&mut host, "启用与电脑的连接通道").changed() {
                draft.connect_host = if host { 1 } else { 0 };
            }

            ui.add_space(6.0);
            ui.label("网络名称（最多 32 个字符）");
            // SSID 是明文回读字段：用"草稿 != 旧快照"判断是否被编辑，
            // 否则用户输入的 SSID 会被设备回传值覆盖。
            let mut ssid = if draft.wifi_ssid != snapshot.wifi_ssid {
                draft.wifi_ssid.clone()
            } else {
                snapshot.wifi_ssid.clone()
            };
            let resp = ui.add(
                egui::TextEdit::singleline(&mut ssid)
                    .hint_text("请输入 WiFi 名称…")
                    .desired_width(280.0),
            );
            if resp.changed() {
                draft.wifi_ssid = ssid;
            }

            ui.add_space(6.0);
            ui.label("网络密码（最多 64 个字符）");
            let mut pw = if draft.wifi_password.is_empty() {
                snapshot.wifi_password.clone()
            } else {
                draft.wifi_password.clone()
            };
            let resp = ui.add(
                egui::TextEdit::singleline(&mut pw)
                    .hint_text("请输入 WiFi 密码…")
                    .password(true)
                    .desired_width(280.0),
            );
            if resp.changed() {
                draft.wifi_password = pw;
            }
        });
    });
}
