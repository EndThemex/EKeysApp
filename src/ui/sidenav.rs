//! 左侧导航。

use eframe::egui;

use crate::state::{Page, UiEvent};

#[derive(Debug, Clone, Copy)]
pub struct NavItem {
    pub page: Page,
    pub label: &'static str,
    pub icon: &'static str,
    pub enabled: bool,
    pub hint: Option<&'static str>,
}

const ITEMS: &[NavItem] = &[
    NavItem {
        page: Page::Connect,
        label: "连接",
        icon: "🔌",
        enabled: true,
        hint: None,
    },
    NavItem {
        page: Page::Settings,
        label: "设备设置",
        icon: "🎛",
        enabled: true,
        hint: None,
    },
    NavItem {
        page: Page::Lighting,
        label: "灯效",
        icon: "💡",
        enabled: true,
        hint: None,
    },
    NavItem {
        page: Page::Wifi,
        label: "WiFi",
        icon: "📶",
        enabled: true,
        hint: None,
    },
    NavItem {
        page: Page::Voice,
        label: "语音",
        icon: "🎤",
        enabled: true,
        hint: None,
    },
    NavItem {
        page: Page::Log,
        label: "日志",
        icon: "📜",
        enabled: true,
        hint: None,
    },
    NavItem {
        page: Page::About,
        label: "关于",
        icon: "ℹ",
        enabled: true,
        hint: None,
    },
];

pub fn show(handle: &AppHandle, ui: &mut egui::Ui) {
    let current = *handle.page.lock().unwrap();
    for item in ITEMS {
        let selected = current == item.page;
        let btn = egui::Button::new(format!("{} {}", item.icon, item.label))
            .fill(if selected {
                egui::Color32::from_rgb(60, 90, 140)
            } else {
                egui::Color32::TRANSPARENT
            })
            .stroke(egui::Stroke::NONE);
        let resp = ui.add_enabled(item.enabled, btn);
        if resp.clicked() {
            let _ = handle.ui_tx.send(UiEvent::Navigate(item.page));
        }
    }
}

use crate::state::AppHandle;
