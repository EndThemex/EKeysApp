//! 底部状态条：Uptime、收发计数、心跳间隔。
//! （连接状态由顶栏胶囊展示，这里只放遥测信息，避免重复。）

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::link::heartbeat::HEARTBEAT_INTERVAL_MS;
use crate::state::AppHandle;

fn icon_label(ui: &mut egui::Ui, icon: &str, text: &str) {
    // 图标走 Phosphor 字体族，文本走 Proportional，二者不共享同一次 text()，
    // 避免 Phosphor 对小写字母的零宽字形污染普通字符（参见 fonts.rs 注释）。
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).font(crate::ui::fonts::icon_font_id(12.0)));
        ui.label(egui::RichText::new(text).weak());
    });
}

/// 渲染"运行时长 HH:MM:SS"：断线时返回 "-"，避免和 uptime_start = None 撞车。
fn format_uptime(start: Option<Instant>) -> String {
    match start {
        Some(t) => format_duration(t.elapsed()),
        None => "-".to_string(),
    }
}

fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let h = total / 3600;
    let m = (total / 60) % 60;
    let s = total % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui) {
    // 从共享计数读取：AppHandle 与 LinkManager.writer/router 共享同一 Arc,
    // 所以这里看到的就是真实已发送 / 已接收帧数（断线时由 detach_link 清零）。
    let tx = handle.tx_count.load(Ordering::Relaxed);
    let rx = handle.rx_count.load(Ordering::Relaxed);
    let uptime = format_uptime(*handle.uptime_start.lock().unwrap());

    egui::menu::bar(ui, |ui| {
        ui.style_mut().spacing.item_spacing.x = 14.0;
        icon_label(ui, crate::ui::icons::UPTIME, &format!("运行时长 {uptime}"));
        icon_label(ui, crate::ui::icons::TX, &format!("已发送 {tx}"));
        icon_label(ui, crate::ui::icons::RX, &format!("已接收 {rx}"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 心跳间隔当前硬编码 1s；预留接口给 UI 配置（见 heartbeat.rs 注释）。
            icon_label(
                ui,
                crate::ui::icons::HEARTBEAT,
                &format!("心跳 {}秒", HEARTBEAT_INTERVAL_MS / 1000),
            );
        });
    });
}