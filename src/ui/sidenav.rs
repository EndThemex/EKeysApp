//! 左侧导航：品牌区 + 分组导航（设备 / 系统）。

use eframe::egui;

use crate::i18n;
use crate::state::{AppHandle, Page, UiEvent};

#[derive(Debug, Clone, Copy)]
pub struct NavItem {
    pub page: Page,
    /// i18n key（渲染时经 `i18n::t` 取当前语言文案）
    pub label: &'static str,
    pub icon: &'static str,
    pub enabled: bool,
    /// i18n key
    pub hint: Option<&'static str>,
}

const ITEMS: &[NavItem] = &[
    NavItem {
        page: Page::Settings,
        label: "nav.device",
        icon: crate::ui::icons::NAV_SETTINGS,
        enabled: true,
        hint: Some("nav.device.hint"),
    },
    NavItem {
        page: Page::Keymap,
        label: "nav.keymap",
        icon: crate::ui::icons::NAV_KEYMAP,
        enabled: true,
        hint: Some("nav.keymap.hint"),
    },
    NavItem {
        page: Page::Lighting,
        label: "nav.lighting",
        icon: crate::ui::icons::NAV_LIGHTING,
        enabled: true,
        hint: Some("nav.lighting.hint"),
    },
    NavItem {
        page: Page::Wifi,
        label: "nav.wifi",
        icon: crate::ui::icons::NAV_WIFI,
        enabled: true,
        hint: Some("nav.wifi.hint"),
    },
    NavItem {
        page: Page::Audio,
        label: "nav.audio",
        icon: crate::ui::icons::NAV_AUDIO,
        enabled: true,
        hint: Some("nav.audio.hint"),
    },
    NavItem {
        page: Page::Voice,
        label: "nav.voice",
        icon: crate::ui::icons::NAV_VOICE,
        enabled: true,
        hint: Some("nav.voice.hint"),
    },
    NavItem {
        page: Page::Log,
        label: "nav.log",
        icon: crate::ui::icons::NAV_LOG,
        enabled: true,
        hint: Some("nav.log.hint"),
    },
    NavItem {
        page: Page::About,
        label: "nav.about",
        icon: crate::ui::icons::NAV_ABOUT,
        enabled: true,
        hint: Some("nav.about.hint"),
    },
];

pub fn show(handle: &AppHandle, ui: &mut egui::Ui) {
    let current = *handle.page.lock().unwrap();
    ui.spacing_mut().item_spacing.y = 2.0;

    // 品牌区
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(crate::ui::icons::BRAND_KEYBOARD)
                .font(crate::ui::fonts::icon_font_id(22.0))
                .strong()
                .color(crate::ui::ACCENT),
        );
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

    // 主导航：设备相关（设置 / 键映射 / 灯效 / WiFi / 语音 / 音效 → 共 6 项）
    nav_group(
        ui,
        handle,
        current,
        i18n::t("nav.group.device"),
        &ITEMS[..6],
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // 系统组：日志 / 关于
    nav_group(
        ui,
        handle,
        current,
        i18n::t("nav.group.system"),
        &ITEMS[6..],
    );
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

    // 图标和文字分两次绘制：分别用 Phosphor 和 Proportional 字体族，
    // 避免在同一次 text() 调用里让两种字体的 cmap 互相干扰（Phosphor 对小写
    // 字母只有零宽占位字形，会把"WiFi"吞成"WF"）。
    let text_size = if selected { 14.5 } else { 14.0 };
    let icon_font = crate::ui::fonts::icon_font_id(text_size);
    let label_font = egui::FontId::proportional(text_size);
    let icon_galley = ui
        .painter()
        .layout(item.icon.to_string(), icon_font, fg, f32::INFINITY);
    let label_galley = ui.painter().layout(
        i18n::t(item.label).to_string(),
        label_font,
        fg,
        f32::INFINITY,
    );
    let icon_w = icon_galley.rect.width();
    let gap = 6.0;
    let start_x = rect.left_center().x + 10.0;
    let center_y = rect.left_center().y;

    ui.painter().galley(
        egui::pos2(start_x, center_y - icon_galley.rect.height() / 2.0),
        icon_galley.into(),
        fg,
    );
    ui.painter().galley(
        egui::pos2(
            start_x + icon_w + gap,
            center_y - label_galley.rect.height() / 2.0,
        ),
        label_galley.into(),
        fg,
    );

    if resp.clicked() {
        let _ = handle.ui_tx.send(UiEvent::Navigate(item.page));
    }
    if let Some(hint) = item.hint {
        resp.on_hover_text(i18n::t(hint));
    }
}
