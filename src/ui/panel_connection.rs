//! P1 Connect 页面：端口选择 + 连接/断开。

use eframe::egui;

use crate::link::{LinkManager, PortInfo};
use crate::state::{AppHandle, Page, UiEvent};

#[derive(Default)]
pub struct ConnectPanelState {
    pub ports: Vec<PortInfo>,
    pub selected: Option<String>,
    pub scanning: bool,
    /// 自动连接是否已尝试过（避免每次刷新都触发）
    pub auto_connect_done: bool,
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
    ui.add_space(10.0);

    // 卡片 1：端口配置
    crate::ui::card(ui, |ui| {
        egui::Grid::new("connect-grid")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("端口");
                ui.horizontal(|ui| {
                    let display = st.selected.clone().unwrap_or_else(|| "(无)".to_string());
                    egui::ComboBox::from_id_salt("port-combo")
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
                    if ui
                        .add(crate::ui::fonts::IconTextButton::new(
                            crate::ui::icons::SCAN,
                            "扫描",
                            14.0,
                        ))
                        .clicked()
                    {
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
                                "厂商 ID: {}  产品 ID: {}",
                                info.vid.map(|v| format!("0x{v:04X}")).unwrap_or("-".into()),
                                info.pid.map(|v| format!("0x{v:04X}")).unwrap_or("-".into())
                            ));
                            if let Some(m) = &info.manufacturer {
                                ui.label(format!("厂商名称: {m}"));
                            }
                            if let Some(s) = &info.serial_number {
                                ui.label(format!("序列号: {s}"));
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
    });

    ui.add_space(10.0);

    // 操作按钮
    ui.horizontal(|ui| {
        let can_connect = !is_online && st.selected.is_some();
        let connect_btn = egui::Button::new("● 连接")
            .fill(crate::ui::ACCENT)
            .corner_radius(egui::CornerRadius::same(6))
            .min_size(egui::vec2(96.0, 32.0));
        if ui.add_enabled(can_connect, connect_btn).clicked() {
            if let Some(name) = st.selected.clone() {
                match handle.attempt_connect(&name) {
                    Ok(()) => {
                        // 会话内已建立过连接：抑制底部自动连接块，避免之后
                        // 主动断开时它当帧又把连接拉起来（断不开 = 卡死）。
                        st.auto_connect_done = true;
                        // 连接成功 → 默认跳到设置页（沿用原有交互）
                        let _ = handle.ui_tx.send(UiEvent::Navigate(Page::Settings));
                    }
                    Err(e) => {
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            crate::state::ToastKind::Error,
                            format!("连接失败: {e}"),
                        ));
                    }
                }
            }
        }

        let disconnect_btn = egui::Button::new("■ 断开")
            .corner_radius(egui::CornerRadius::same(6))
            .min_size(egui::vec2(96.0, 32.0));
        if ui.add_enabled(is_online, disconnect_btn).clicked() {
            // 用户主动断开 = 明确要停在未连接状态：抑制底部自动连接块，
            // 否则当帧它就按 last_port 重新连上（无限拉锯，UI 反复被
            // attempt_connect 的 auto_get 阻塞数秒）。
            st.auto_connect_done = true;
            handle.detach_link();
            handle.log_kind(crate::state::LogKind::App, "已断开");
            // 清空顶栏端口名显示
            let _ = handle.ui_tx.send(UiEvent::CurrentPort(String::new()));
        }
    });

    ui.add_space(10.0);

    // 卡片 2：状态 + 自动连接
    crate::ui::card(ui, |ui| {
        ui.horizontal(|ui| {
            let (color, text) = match &state {
                crate::link::ConnectionState::Online => (crate::ui::colors::STATUS_GREEN, "在线"),
                crate::link::ConnectionState::Connecting
                | crate::link::ConnectionState::Reconnecting => {
                    (crate::ui::colors::STATUS_YELLOW, "连接中…")
                }
                crate::link::ConnectionState::Error(_) => {
                    (crate::ui::colors::STATUS_RED, "连接错误")
                }
                crate::link::ConnectionState::Disconnected => {
                    (crate::ui::colors::STATUS_GREY, "未连接")
                }
            };
            crate::ui::status_dot(ui, color);
            ui.strong(text);
            if let crate::link::ConnectionState::Error(e) = &state {
                ui.label(egui::RichText::new(e.clone()).weak());
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        let mut ac = *handle.auto_connect.lock().unwrap();
        if ui.checkbox(&mut ac, "启动时自动连接上次端口").changed() {
            *handle.auto_connect.lock().unwrap() = ac;
            // 勾选只改变"下次启动"的行为，会话内不立即发起连接：
            // 底部自动连接块会在当帧同步 attempt_connect，auto_get 的多个
            // 1s 超时请求会阻塞 UI 数秒（体感卡死）。标记 done 抑制它。
            if ac && !is_online {
                st.auto_connect_done = true;
            }
        }

        // 重连中：显示取消按钮，直接停止自动重连循环。
        // 仅在确实有挂起重连任务时出现，避免误触发；点击后清空 pending_reconnect，
        // 下次 tick_reconnect() 就不会再发起探测/连接。
        let reconnecting = handle.pending_reconnect.lock().unwrap().is_some();
        if reconnecting && !is_online {
            ui.add_space(6.0);
            if ui
                .button("✕ 取消重连")
                .on_hover_text("停止当前正在进行的自动重连")
                .clicked()
            {
                handle.cancel_reconnect();
                let _ = handle.ui_tx.send(UiEvent::Toast(
                    crate::state::ToastKind::Info,
                    "已取消重连".into(),
                ));
            }
        }
    });

    // 自动连接（仅当 Disconnected + 有 last_port + auto_connect=true + 还未尝试过）
    if !is_online && !st.auto_connect_done && *handle.auto_connect.lock().unwrap() {
        if let Some(p) = handle.last_port.lock().unwrap().clone() {
            st.auto_connect_done = true;
            st.selected = Some(p.clone());
            // 自动连接路径不发 Navigate，避免抢 UI；错误也只记日志，
            // 由后续手动连接 / 重连任务继续兜底。
            if let Err(e) = handle.attempt_connect(&p) {
                handle.log_kind(crate::state::LogKind::App, format!("自动连接失败: {e}"));
            }
        }
    }
}
