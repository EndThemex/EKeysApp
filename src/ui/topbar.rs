//! 顶部状态条：连接指示灯 + 端口 + 刷新 + 本地设置入口。

use eframe::egui;

use crate::link::ConnectionState;
use crate::state::AppHandle;
use crate::ui::colors;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, port_name: Option<&str>) {
    let state = handle.state.lock().unwrap().clone();
    let color = match &state {
        ConnectionState::Online => colors::STATUS_GREEN,
        ConnectionState::Connecting | ConnectionState::Reconnecting => colors::STATUS_YELLOW,
        ConnectionState::Error(_) => colors::STATUS_RED,
        ConnectionState::Disconnected => colors::STATUS_GREY,
    };
    let label = match &state {
        ConnectionState::Online => "在线",
        ConnectionState::Connecting => "连接中",
        ConnectionState::Reconnecting => "重连中",
        ConnectionState::Error(e) => &*format!("错误：{e}"),
        ConnectionState::Disconnected => "未连接",
    };

    egui::menu::bar(ui, |ui| {
        // 状态胶囊：圆点 + 状态文字，底色随状态着色
        egui::Frame::new()
            .fill(color.gamma_multiply(0.22))
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

        ui.add_space(4.0);
        let port_text = match port_name {
            Some(name) => format!("{name} · 115200"),
            None => "(未选择端口)".to_string(),
        };
        ui.label(egui::RichText::new(port_text).weak());

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(crate::ui::fonts::IconTextButton::new(
                    crate::ui::icons::GEAR,
                    "本地设置",
                    13.0,
                ))
                .clicked()
            {
                use crate::state::UiEvent;
                let _ = handle.ui_tx.send(UiEvent::OpenLocalSettings);
            }
            let can_refresh = matches!(state, ConnectionState::Online);
            if ui
                .add_enabled(
                    can_refresh,
                    crate::ui::fonts::IconTextButton::new(crate::ui::icons::REFRESH, "刷新", 13.0),
                )
                .on_hover_text("重新拉取全量设置 (F5)")
                .clicked()
            {
                handle_refresh(handle);
            }
        });
    });
}

fn handle_refresh(handle: &AppHandle) {
    use crate::protocol::{CMD_CONFIG_GET, DeviceSettings};
    use std::time::Duration;
    // 直接在 UI 线程发请求；超时 1s
    let _ = handle.with_link(|lm| {
        match lm.request(CMD_CONFIG_GET, None, Duration::from_millis(1000)) {
            Ok(frame) => {
                if let Some(data) = frame.data.as_ref() {
                    if let Ok(s) = serde_json::from_value::<DeviceSettings>(data.clone()) {
                        *handle.settings.lock().unwrap() = s;
                        handle.log_kind(crate::state::LogKind::Rx, "GET → 全量快照");
                    }
                }
            }
            Err(e) => {
                handle.log_kind(crate::state::LogKind::App, format!("GET 失败: {e}"));
            }
        }
    });
}
