//! 左侧导航：品牌区 + 分组导航（设备 / 系统）。

use eframe::egui;

use crate::state::{AppHandle, Page, UiEvent};

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
        hint: Some("端口选择与连接 (Ctrl+1)"),
    },
    NavItem {
        page: Page::Settings,
        label: "设备设置",
        icon: "🎛",
        enabled: true,
        hint: Some("设备参数配置 (Ctrl+2)"),
    },
    NavItem {
        page: Page::Keymap,
        label: "键映射",
        icon: "🎹",
        enabled: true,
        hint: Some("按键盘自定义按键功能 (Ctrl+3)"),
    },
    NavItem {
        page: Page::Lighting,
        label: "灯效",
        icon: "💡",
        enabled: true,
        hint: Some("灯光效果 (Ctrl+4)"),
    },
    NavItem {
        page: Page::Wifi,
        label: "WiFi",
        icon: "📶",
        enabled: true,
        hint: Some("无线网络 (Ctrl+5)"),
    },
    NavItem {
        page: Page::Voice,
        label: "语音",
        icon: "🎤",
        enabled: true,
        hint: Some("语音设置 (Ctrl+6)"),
    },
    NavItem {
        page: Page::Log,
        label: "日志",
        icon: "📜",
        enabled: true,
        hint: Some("协议与应用日志 (Ctrl+7)"),
    },
    NavItem {
        page: Page::About,
        label: "关于",
        icon: "ℹ",
        enabled: true,
        hint: Some("版本信息 (Ctrl+8)"),
    },
];

pub fn show(handle: &AppHandle, ui: &mut egui::Ui) {
    let current = *handle.page.lock().unwrap();
    ui.spacing_mut().item_spacing.y = 2.0;

    // 品牌区
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("⌨").size(20.0).color(crate::ui::ACCENT));
        ui.vertical(|ui| {
            ui.strong(egui::RichText::new("EKeys").size(16.0));
            ui.label(
                egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .weak()
                    .size(11.0),
            );
        });
    });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // 主导航：设备相关（连接 / 设置 / 键映射 / 灯效 / WiFi / 语音 → 共 6 项）
    nav_group(ui, handle, current, "设备", &ITEMS[..6]);
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // 系统组：日志 / 关于
    nav_group(ui, handle, current, "系统", &ITEMS[6..]);
}

fn nav_group(ui: &mut egui::Ui, handle: &AppHandle, current: Page, title: &str, items: &[NavItem]) {
    ui.label(egui::RichText::new(title).weak().size(11.0));
    ui.add_space(4.0);
    for item in items {
        nav_item(ui, handle, current, item);
    }
}

fn nav_item(ui: &mut egui::Ui, handle: &AppHandle, current: Page, item: &NavItem) {
    let selected = current == item.page;
    let width = ui.available_width();
    let height = 32.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let vis = ui.visuals();
    let (bg, fg) = if selected {
        (crate::ui::ACCENT, egui::Color32::WHITE)
    } else if !item.enabled {
        (
            egui::Color32::TRANSPARENT,
            vis.widgets.inactive.fg_stroke.color.gamma_multiply(0.5),
        )
    } else if resp.hovered() {
        (vis.widgets.hovered.bg_fill, vis.text_color())
    } else {
        (egui::Color32::TRANSPARENT, vis.text_color())
    };

    if bg != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), bg);
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{}  {}", item.icon, item.label),
        egui::FontId::proportional(if selected { 14.5 } else { 14.0 }),
        fg,
    );

    if resp.clicked() {
        let _ = handle.ui_tx.send(UiEvent::Navigate(item.page));
    }
    if let Some(hint) = item.hint {
        resp.on_hover_text(hint);
    }
}
