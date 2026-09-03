//! P5 WiFi 页面：WiFi 配置。
//!
//! 字段：wifi_switch / connect_host / wifi_ssid / wifi_password / wifi_* 阶段 06 生效
//! 协议 §4.1：ssid ≤32 字节，password ≤64 字节（过长自动截断在固件侧处理）

use std::cell::Cell;

use eframe::egui;

use crate::protocol::DeviceSettings;
use crate::state::AppHandle;
use crate::ui::widgets::{DiffAction, apply_diff, show_diff_bar};

#[derive(Default)]
pub struct WifiPanelState;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, _st: &mut WifiPanelState) {
    ui.heading("WiFi");
    ui.label("阶段 06 生效：WiFi 配置（修改后下次重启生效）");
    ui.add_space(4.0);

    let snapshot = handle.settings.lock().unwrap().clone();
    let mut draft = handle.draft.lock().unwrap().clone();
    let dirty = Cell::new(false);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.group(|ui| {
            ui.label("WiFi 开关");
            let mut on = if draft.wifi_switch != 0 || snapshot.wifi_switch != 0 {
                draft.wifi_switch != 0
            } else {
                snapshot.wifi_switch != 0
            };
            if ui.checkbox(&mut on, "启用 WiFi").changed() {
                draft.wifi_switch = if on { 1 } else { 0 };
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("连接主机");
            let mut host = if draft.connect_host != 0 || snapshot.connect_host != 0 {
                draft.connect_host != 0
            } else {
                snapshot.connect_host != 0
            };
            if ui.checkbox(&mut host, "启用 connect_host").changed() {
                draft.connect_host = if host { 1 } else { 0 };
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("网络名称（≤32 字节）");
            let mut ssid = if draft.wifi_ssid.is_empty() {
                snapshot.wifi_ssid.clone()
            } else {
                draft.wifi_ssid.clone()
            };
            let resp = ui.add(
                egui::TextEdit::singleline(&mut ssid)
                    .hint_text("WiFi SSID")
                    .desired_width(280.0),
            );
            if resp.changed() {
                draft.wifi_ssid = ssid;
                dirty.set(true);
            }

            ui.add_space(6.0);
            ui.label("密码（≤64 字节）");
            let mut pw = if draft.wifi_password.is_empty() {
                snapshot.wifi_password.clone()
            } else {
                draft.wifi_password.clone()
            };
            let resp = ui.add(
                egui::TextEdit::singleline(&mut pw)
                    .hint_text("WiFi 密码")
                    .password(true)
                    .desired_width(280.0),
            );
            if resp.changed() {
                draft.wifi_password = pw;
                dirty.set(true);
            }
        });
    });

    let diff = draft.diff(&snapshot);
    let has_diff = wifi_diff_count(&diff) > 0;

    ui.add_space(8.0);
    let action = show_diff_bar(handle, ui, &diff, has_diff);
    match action {
        DiffAction::Apply => apply_diff(handle, &diff),
        DiffAction::Discard => *handle.draft.lock().unwrap() = snapshot.clone(),
        DiffAction::None => {}
    }

    if dirty.get() {
        *handle.draft.lock().unwrap() = draft;
    } else {
        *handle.draft.lock().unwrap() = draft;
    }
}

fn wifi_diff_count(d: &DeviceSettings) -> usize {
    let mut n = 0;
    if d.wifi_switch != 0 {
        n += 1;
    }
    if d.connect_host != 0 {
        n += 1;
    }
    if !d.wifi_ssid.is_empty() {
        n += 1;
    }
    if !d.wifi_password.is_empty() {
        n += 1;
    }
    n
}
