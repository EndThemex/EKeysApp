//! 字体注册：从 main.rs 抽离出来。

use std::sync::Arc;

use eframe::egui;

const FONT_BYTES: &[u8] = include_bytes!("../../fonts/MapleMono-NF-CN-ExtraLight.ttf");

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "maple_cn".to_owned(),
        Arc::new(egui::FontData::from_static(FONT_BYTES)),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "maple_cn".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("maple_cn".to_owned());

    ctx.set_fonts(fonts);
}
