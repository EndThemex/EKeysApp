//! P1 Connect 页面：端口选择 + 连接/断开。

use eframe::egui;

use crate::link::{LinkManager, PortInfo};
use crate::state::{AppHandle, Page, UiEvent};

#[derive(Default)]
pub struct ConnectPanelState {
    pub ports: Vec<PortInfo>,
    pub selected: Option<String>,
    pub scanning: bool,
}

impl ConnectPanelState {
    pub fn refresh(&mut self) {
        self.ports = LinkManager::list_ports();
        if self.selected.is_none() {
            self.selected = self.ports.first().map(|p| p.name.clone());
        } else if !self
            .ports
            .iter()
            .any(|p| Some(&p.name) == self.selected.as_ref())
        {
            // 之前选中的端口已消失
            self.selected = self.ports.first().map(|p| p.name.clone());
        }
    }
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut ConnectPanelState) {
    let state = handle.state.lock().unwrap().clone();
    let is_online = matches!(state, crate::link::ConnectionState::Online);

    ui.heading("连接");
    ui.add_space(8.0);

    egui::Grid::new("connect-grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("端口");
            ui.horizontal(|ui| {
                let display = st.selected.clone().unwrap_or_else(|| "(无)".to_string());
                egui::ComboBox::from_id_source("port-combo")
                    .selected_text(display)
                    .show_ui(ui, |cb| {
                        for p in &st.ports {
                            let label = if let Some(prod) = &p.product {
                                format!("{} ({})", p.name, prod)
                            } else {
                                p.name.clone()
                            };
                            cb.selectable_value(&mut st.selected, Some(p.name.clone()), label);
                        }
                    });
                if ui.button("🔄 扫描").clicked() {
                    st.refresh();
                }
            });
            ui.end_row();

            ui.label("波特率");
            ui.label("115200（固定）");
            ui.end_row();

            ui.label("设备信息");
            ui.vertical(|ui| {
                if let Some(name) = &st.selected {
                    if let Some(info) = st.ports.iter().find(|p| &p.name == name) {
                        ui.label(format!(
                            "VID: {}  PID: {}",
                            info.vid.map(|v| format!("0x{v:04X}")).unwrap_or("-".into()),
                            info.pid.map(|v| format!("0x{v:04X}")).unwrap_or("-".into())
                        ));
                        if let Some(m) = &info.manufacturer {
                            ui.label(format!("Manufacturer: {m}"));
                        }
                        if let Some(s) = &info.serial_number {
                            ui.label(format!("Serial: {s}"));
                        }
                    } else {
                        ui.label("(端口信息不可用)");
                    }
                } else {
                    ui.label("(未选择端口)");
                }
            });
            ui.end_row();
        });

    ui.add_space(12.0);

    ui.horizontal(|ui| {
        let can_connect = !is_online && st.selected.is_some();
        if ui
            .add_enabled(can_connect, egui::Button::new("● Connect"))
            .clicked()
        {
            if let Some(name) = st.selected.clone() {
                match LinkManager::open(&name) {
                    Ok(lm) => {
                        handle.attach_link(lm);
                        handle.log_kind(crate::state::LogKind::App, format!("已连接到 {name}"));
                        let _ = handle.ui_tx.send(UiEvent::Navigate(Page::Settings));
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            crate::state::ToastKind::Success,
                            "已连接".into(),
                        ));
                    }
                    Err(e) => {
                        handle.log_kind(crate::state::LogKind::App, format!("连接失败: {e}"));
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            crate::state::ToastKind::Error,
                            format!("连接失败: {e}"),
                        ));
                    }
                }
            }
        }

        if ui
            .add_enabled(is_online, egui::Button::new("■ Disconnect"))
            .clicked()
        {
            handle.detach_link();
            handle.log_kind(crate::state::LogKind::App, "已断开");
        }
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
    ui.label(format!("状态: {state:?}"));
}
