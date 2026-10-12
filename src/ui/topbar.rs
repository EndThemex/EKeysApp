//! 顶部状态条：
//! - 端口下拉 + 扫描
//! - 连接 / 断开按钮
//! - 状态胶囊（连接指示灯 + 状态文字）
//! - 端口名 + 设备信息（device_name / firmware_version）
//! - 自动连接开关 + 停止重连
//! - 本地设置 / 刷新

use eframe::egui;

use crate::i18n;
use crate::link::serial::WCH_VID;
use crate::link::{ConnectionState, LinkManager, PortInfo};
use crate::state::{AppHandle, UiEvent};
use crate::ui::colors;

/// 原 Page::Connect 的状态：端口列表 + 当前选中 + 自动连接抑制标记。
///
/// 这里不再用单独面板承载，而被吸收到顶栏；保留独立 struct 是为了：
/// - 端口列表扫描是相对昂贵操作（系统调用），不能每帧重建；
/// - "会话内已建立过连接"抑制必须跨帧记录，否则主动断开后会被自动
///   连接块当帧又拉起来（参见原 panel_connection.rs 注释）。
#[derive(Default)]
pub struct ConnectPanelState {
    pub ports: Vec<PortInfo>,
    pub selected: Option<String>,
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

/// 顶栏交互控件的次级背景：与 chrome 形成轻微分层，比纯透明更显眼。
fn interactive_fill(dark: bool) -> egui::Color32 {
    if dark {
        egui::Color32::from_rgb(0x24, 0x2A, 0x33)
    } else {
        egui::Color32::from_rgb(0xE9, 0xEC, 0xF1)
    }
}

/// 顶栏交互控件的边框：比 egui 默认 `inactive.bg_stroke` 更深一档，
/// 让 ComboBox / Button 在 chrome 上有清晰可辨的轮廓。
fn interactive_border(dark: bool) -> egui::Stroke {
    let color = if dark {
        egui::Color32::from_rgb(0x44, 0x4B, 0x56)
    } else {
        egui::Color32::from_rgb(0xAE, 0xB4, 0xBE)
    };
    egui::Stroke::new(1.0, color)
}

pub fn show(
    handle: &AppHandle,
    ui: &mut egui::Ui,
    port_name: Option<&str>,
    st: &mut ConnectPanelState,
) {
    let state = handle.state.lock().unwrap().clone();
    let is_online = matches!(state, ConnectionState::Online);
    // 状态色按主题取色：亮黄 / 亮绿在白底上对比不足，浅色主题用加深变体。
    let dark = ui.visuals().dark_mode;
    let (color, label) = match &state {
        ConnectionState::Online => (
            colors::themed(dark, colors::STATUS_GREEN, colors::STATUS_GREEN_L),
            i18n::t("top.status.online"),
        ),
        ConnectionState::Connecting | ConnectionState::Reconnecting => (
            colors::themed(dark, colors::STATUS_YELLOW, colors::STATUS_YELLOW_L),
            if matches!(state, ConnectionState::Reconnecting) {
                i18n::t("top.status.reconnecting")
            } else {
                i18n::t("top.status.connecting")
            },
        ),
        ConnectionState::Disconnected => (
            colors::themed(dark, colors::STATUS_GREY, colors::STATUS_GREY_L),
            i18n::t("top.status.disconnected"),
        ),
    };

    egui::MenuBar::new().ui(ui, |ui| {
        // 状态胶囊：圆点 + 状态文字，底色随状态着色。
        // 深色主题：把状态色压暗成深色底；浅色主题：把状态色向白色稀释成浅色底。
        let chip_bg = if dark {
            color.gamma_multiply(0.22)
        } else {
            colors::mix(egui::Color32::WHITE, color, 0.16)
        };
        egui::Frame::new()
            .fill(chip_bg)
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top: 3,
                bottom: 3,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::Vec2::new(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                    ui.label(egui::RichText::new(label).strong().size(13.0).color(color));
                });
            });

        ui.add_space(6.0);

        // 端口下拉 + 扫描
        ui.horizontal(|ui| {
            let display = st
                .selected
                .clone()
                .unwrap_or_else(|| i18n::t("top.port.none").to_string());
            // ComboBox 自身在两种主题下的边框都偏淡，套一层 Frame 强制画出
            // 清晰轮廓 + 次级底色，让下拉看上去是可交互的控件。
            egui::Frame::new()
                .fill(interactive_fill(dark))
                .stroke(interactive_border(dark))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    egui::ComboBox::from_id_salt("topbar-port-combo")
                        .selected_text(display)
                        .width(90.0)
                        .show_ui(ui, |cb| {
                            for p in &st.ports {
                                // CH340 系列桥接端口：列表里直接标注不支持
                                // （连接时 attempt_connect 也会拒绝并弹 Toast）
                                let label = if p.vid == Some(WCH_VID) {
                                    i18n::t("top.port.ch340").replace("{name}", &p.name)
                                } else if let Some(prod) = &p.product {
                                    i18n::t("top.port.product")
                                        .replace("{name}", &p.name)
                                        .replace("{product}", prod)
                                } else {
                                    p.name.clone()
                                };
                                cb.selectable_value(&mut st.selected, Some(p.name.clone()), label);
                            }
                        });
                });
            // 「扫描」是次要按钮：明确底色 + 边框，与 ComboBox 保持同一视觉层级。
            // IconTextButton 的边框默认跟随 visuals.bg_stroke，在 chrome 上偏淡，
            // 这里显式传入 fill 让它看上去"可点击"。
            let scan_btn = crate::ui::fonts::IconTextButton::new(
                crate::ui::icons::SCAN,
                i18n::t("top.scan"),
                13.0,
            )
            .fill(interactive_fill(dark))
            .corner_radius(egui::CornerRadius::same(6));
            if ui.add(scan_btn).clicked() {
                st.refresh();
            }
        });

        ui.add_space(4.0);

        // 连接 / 断开按钮：明确底色 + 边框，避免在 chrome 上"消失"。
        // 连接按钮用品牌色填充强调主操作；断开按钮用次级底色避免视觉过重。
        let can_connect = !is_online && st.selected.is_some();
        let connect_btn = egui::Button::new(
            egui::RichText::new(i18n::t("top.connect")).color(egui::Color32::WHITE),
        )
        .fill(crate::ui::ACCENT)
        .stroke(egui::Stroke::new(1.0, crate::ui::ACCENT))
        .corner_radius(egui::CornerRadius::same(6))
        .min_size(egui::vec2(76.0, 24.0));
        if ui.add_enabled(can_connect, connect_btn).clicked() {
            if let Some(name) = st.selected.clone() {
                match handle.attempt_connect(&name) {
                    Ok(()) => {
                        // 会话内已建立过连接：抑制底部自动连接块，避免之后
                        // 主动断开时它当帧又把连接拉起来（断不开 = 卡死）。
                        st.auto_connect_done = true;
                    }
                    Err(e) => {
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            crate::state::ToastKind::Error,
                            i18n::t("top.connect.fail").replace("{error}", &e.to_string()),
                        ));
                    }
                }
            }
        }
        let disconnect_btn = egui::Button::new(i18n::t("top.disconnect"))
            .fill(interactive_fill(dark))
            .stroke(interactive_border(dark))
            .corner_radius(egui::CornerRadius::same(6))
            .min_size(egui::vec2(76.0, 24.0));
        if ui.add_enabled(is_online, disconnect_btn).clicked() {
            // 用户主动断开 = 明确要停在未连接状态：抑制底部自动连接块，
            // 否则当帧它就按 last_port 重新连上（无限拉锯）。
            st.auto_connect_done = true;
            handle.detach_link();
            handle.log_kind(crate::state::LogKind::App, "已断开");
            // 清空顶栏端口名显示
            let _ = handle.ui_tx.send(UiEvent::CurrentPort(String::new()));
        }

        ui.add_space(6.0);

        // 端口名（波特率固定 115200，不再重复展示）
        let port_text = match port_name {
            Some(name) => name.to_string(),
            None => match &st.selected {
                Some(name) => name.clone(),
                None => i18n::t("top.port.unselected").to_string(),
            },
        };
        ui.label(egui::RichText::new(port_text).weak());

        // 设备信息：连接成功后显示 device_name + firmware_version
        if matches!(state, ConnectionState::Online) {
            let info = handle.device_info.lock().unwrap().clone();
            if !info.device_name.is_empty() || !info.firmware_version.is_empty() {
                ui.add_space(8.0);
                let mut parts = Vec::new();
                if !info.device_name.is_empty() {
                    parts.push(info.device_name);
                }
                if !info.firmware_version.is_empty() {
                    parts.push(format!("v{}", info.firmware_version));
                }
                ui.label(egui::RichText::new(parts.join(" · ")).weak().size(12.0));
            }
        }

        // 自动连接 checkbox（顶栏直接放，不另起面板）
        ui.add_space(6.0);
        let mut ac = *handle.auto_connect.lock().unwrap();
        if ui
            .checkbox(&mut ac, i18n::t("top.auto_connect"))
            .on_hover_text(i18n::t("top.auto_connect.hint"))
            .changed()
        {
            *handle.auto_connect.lock().unwrap() = ac;
            // 勾选只改变"下次启动"的行为，会话内不立即发起连接：
            // 否则 auto_get 的多个 1s 超时请求会阻塞 UI 数秒（体感卡死）。
            // 标记 done 抑制自动连接块。
            if ac && !is_online {
                st.auto_connect_done = true;
            }
        }

        // CH340 警告：当前选中端口是 WCH 桥接 → 给个内联提示，比 Toast 更直接。
        if let Some(name) = st.selected.as_ref() {
            if let Some(info) = st.ports.iter().find(|p| &p.name == name) {
                if info.vid == Some(WCH_VID) {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(i18n::t("top.auto_connect.ch340_warning"))
                            .color(ui.visuals().warn_fg_color)
                            .size(11.0),
                    );
                }
            }
        }

        // 重连中：显示取消按钮，直接停止自动重连循环。
        // 仅在确实有挂起重连任务时出现，避免误触发。
        let reconnecting = handle.pending_reconnect.lock().unwrap().is_some();
        if reconnecting && !is_online {
            let stop_btn = egui::Button::new(i18n::t("top.stop_reconnect"))
                .fill(interactive_fill(dark))
                .stroke(interactive_border(dark))
                .corner_radius(egui::CornerRadius::same(6));
            if ui
                .add(stop_btn)
                .on_hover_text(i18n::t("top.stop_reconnect.hint"))
                .clicked()
            {
                handle.cancel_reconnect();
                let _ = handle.ui_tx.send(UiEvent::Toast(
                    crate::state::ToastKind::Info,
                    i18n::t("top.stop_reconnect.done").into(),
                ));
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(crate::ui::fonts::IconTextButton::new(
                    crate::ui::icons::GEAR,
                    i18n::t("top.settings"),
                    13.0,
                ))
                .clicked()
            {
                let _ = handle.ui_tx.send(UiEvent::OpenLocalSettings);
            }
            let can_refresh = matches!(state, ConnectionState::Online);
            if ui
                .add_enabled(
                    can_refresh,
                    crate::ui::fonts::IconTextButton::new(
                        crate::ui::icons::REFRESH,
                        i18n::t("top.refresh"),
                        13.0,
                    ),
                )
                .on_hover_text(i18n::t("top.refresh.hint"))
                .clicked()
            {
                handle_refresh(handle);
            }
        });
    });

    // 自动连接（仅当 Disconnected + 有 last_port + auto_connect=true + 还未尝试过）
    //
    // 关键：必须先把 last_port 取出并释放锁，再进入分支。if let 的 scrutinee
    // 临时 MutexGuard 会存活到整个 then 块结束，若在块内调用 attempt_connect
    // （其成功路径会再次锁 last_port），同线程对 std Mutex 二次加锁 → 永久
    // 死锁：UI 冻结、窗口不出现（启动自动连接正是这条路径）。
    let last_port = handle.last_port.lock().unwrap().clone();
    if !is_online && !st.auto_connect_done && *handle.auto_connect.lock().unwrap() {
        if let Some(p) = last_port {
            st.auto_connect_done = true;
            st.selected = Some(p.clone());
            // CH340 等 WCH 桥接端口：与手动连接同一策略，拒绝连接并提示。
            if let Some((vid, pid)) = crate::link::serial::is_wch_bridge(&p) {
                handle.log_kind(
                    crate::state::LogKind::App,
                    format!(
                        "跳过自动连接 {p}：当前为 CH340 系列 USB 转串口芯片（{vid:04X}:{pid:04X}）"
                    ),
                );
                let _ = handle.ui_tx.send(UiEvent::Toast(
                    crate::state::ToastKind::Warning,
                    i18n::t("top.auto_connect.ch340_skipped").replace("{port}", &p),
                ));
                return;
            }
            // 自动连接路径不发 Toast 错误，由后续手动连接 / 重连任务继续兜底。
            if let Err(e) = handle.attempt_connect(&p) {
                handle.log_kind(crate::state::LogKind::App, format!("自动连接失败：{e}"));
            }
        }
    }
}

fn handle_refresh(handle: &AppHandle) {
    use crate::protocol::{CMD_CONFIG_GET, DeviceSettings};
    use std::time::Duration;
    // 直接在 UI 线程发请求；超时 1s。
    // 落地统一走 apply_settings_snapshot（含脱敏 + draft 同步）。
    let _ = handle.with_link(|lm| {
        match lm.request(CMD_CONFIG_GET, None, Duration::from_millis(1000)) {
            Ok(frame) => match frame.data.as_ref() {
                Some(data) => match serde_json::from_value::<DeviceSettings>(data.clone()) {
                    Ok(mut s) => {
                        handle.apply_settings_snapshot(&mut s);
                        handle.log_kind(crate::state::LogKind::Rx, "GET → 全量快照");
                    }
                    Err(e) => {
                        handle
                            .log_kind(crate::state::LogKind::App, format!("GET 配置解析失败: {e}"));
                    }
                },
                None => {
                    handle.log_kind(crate::state::LogKind::App, "GET 配置响应缺 data 字段");
                }
            },
            Err(e) => {
                handle.log_kind(crate::state::LogKind::App, format!("GET 失败: {e}"));
            }
        }
    });
}
