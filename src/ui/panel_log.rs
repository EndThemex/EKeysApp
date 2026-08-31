//! P7 Log 页面：日志查看器。

use eframe::egui;

use crate::state::{AppHandle, LogKind};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LevelFilter {
    #[default]
    All,
    Info,
    Warn,
    Error,
}

#[derive(Default)]
pub struct LogPanelState {
    pub show_tx: bool,
    pub show_rx: bool,
    pub show_fw: bool,
    pub show_app: bool,
    pub level: LevelFilter,
    pub search: String,
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut LogPanelState) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut st.show_tx, "协议 Tx");
        ui.checkbox(&mut st.show_rx, "协议 Rx");
        ui.checkbox(&mut st.show_fw, "固件日志");
        ui.checkbox(&mut st.show_app, "应用日志");
        ui.separator();
        egui::ComboBox::from_id_source("log-level")
            .selected_text(match st.level {
                LevelFilter::All => "全部".to_string(),
                LevelFilter::Info => "Info".to_string(),
                LevelFilter::Warn => "Warn".to_string(),
                LevelFilter::Error => "Error".to_string(),
            })
            .show_ui(ui, |cb| {
                cb.selectable_value(&mut st.level, LevelFilter::All, "全部");
                cb.selectable_value(&mut st.level, LevelFilter::Info, "Info");
                cb.selectable_value(&mut st.level, LevelFilter::Warn, "Warn");
                cb.selectable_value(&mut st.level, LevelFilter::Error, "Error");
            });
        ui.add(
            egui::TextEdit::singleline(&mut st.search)
                .hint_text("搜索…")
                .desired_width(160.0),
        );
        if ui.button("清空").clicked() {
            handle.log_buf.lock().unwrap().clear();
        }
    });
    ui.separator();

    let entries = handle.log_buf.lock().unwrap().snapshot();
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for e in entries.iter().rev() {
                if !kind_visible(st, e.kind) {
                    continue;
                }
                let text_lvl = match st.level {
                    LevelFilter::All => true,
                    LevelFilter::Info => !e.text.contains("[W]") && !e.text.contains("[E]"),
                    LevelFilter::Warn => e.text.contains("[W]"),
                    LevelFilter::Error => e.text.contains("[E]"),
                };
                if !text_lvl {
                    continue;
                }
                if !st.search.is_empty() && !e.text.contains(&st.search) {
                    continue;
                }

                let color = match e.kind {
                    LogKind::Tx => crate::ui::colors::TX,
                    LogKind::Rx => crate::ui::colors::RX,
                    LogKind::Firmware => crate::ui::colors::FW_INFO,
                    LogKind::App => crate::ui::colors::APP,
                };
                let arrow = match e.kind {
                    LogKind::Tx => "▶ ",
                    LogKind::Rx => "◀ ",
                    LogKind::Firmware => "ℹ ",
                    LogKind::App => "• ",
                };
                ui.colored_label(color, format!("{}{}", arrow, e.text));
            }
        });
}

fn kind_visible(st: &LogPanelState, k: LogKind) -> bool {
    match k {
        LogKind::Tx => st.show_tx,
        LogKind::Rx => st.show_rx,
        LogKind::Firmware => st.show_fw,
        LogKind::App => st.show_app,
    }
}
