//! P7 Log 页面：日志查看器。

use eframe::egui;

use crate::state::{AppHandle, LogKind, ToastKind, UiEvent};
use crate::util::log::SharedLog;

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
    ui.heading("日志");
    ui.add_space(8.0);

    // 工具栏卡片：两行布局
    //   第一行：来源过滤（Tx/Rx/固件/应用）+ 等级过滤
    //   第二行：搜索框（左）+ 操作按钮（右）：复制全部 / 清空
    crate::ui::card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("来源")
                    .strong()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.checkbox(&mut st.show_tx, "协议 Tx");
            ui.checkbox(&mut st.show_rx, "协议 Rx");
            ui.checkbox(&mut st.show_fw, "固件日志");
            ui.checkbox(&mut st.show_app, "应用日志");
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("等级")
                    .strong()
                    .color(ui.visuals().weak_text_color()),
            );
            egui::ComboBox::from_id_salt("log-level")
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
                    .desired_width(180.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(format!("{}  清空", crate::ui::icons::r::TRASH))
                    .clicked()
                {
                    handle.log_buf.lock().unwrap().clear();
                }
                let copy_btn =
                    egui::Button::new(format!("{}  复制全部", crate::ui::icons::r::COPY))
                        .fill(crate::ui::ACCENT)
                        .corner_radius(egui::CornerRadius::same(6));
                if ui.add(copy_btn).clicked() {
                    let snapshot = handle.log_buf.lock().unwrap().snapshot();
                    let text = render_entries_text(&snapshot, st);
                    if text.is_empty() {
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            ToastKind::Info,
                            "没有可复制的日志".to_string(),
                        ));
                    } else {
                        ui.ctx().copy_text(text.clone());
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            ToastKind::Success,
                            format!("已复制 {} 行日志", count_visible(&snapshot, st)),
                        ));
                    }
                }
            });
        });
    });
    ui.add_space(8.0);

    let entries = handle.log_buf.lock().unwrap().snapshot();
    let total_visible = count_visible(&entries, st);
    ui.label(
        egui::RichText::new(format!(
            "共 {} 条（显示 {} 条）",
            entries.len(),
            total_visible
        ))
        .small()
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for e in entries.iter().rev() {
                if !kind_visible(st, e.kind) {
                    continue;
                }
                if !level_visible(&st.level, e) {
                    continue;
                }
                if !st.search.is_empty() && !e.text.contains(&st.search) {
                    continue;
                }

                let color = match e.kind {
                    LogKind::Tx => crate::ui::colors::TX,
                    LogKind::Rx => crate::ui::colors::RX,
                    LogKind::Firmware if e.text.contains("[E]") => crate::ui::colors::FW_ERROR,
                    LogKind::Firmware if e.text.contains("[W]") => crate::ui::colors::FW_WARN,
                    LogKind::Firmware => crate::ui::colors::FW_INFO,
                    LogKind::App => crate::ui::colors::APP,
                };
                let arrow = match e.kind {
                    LogKind::Tx => crate::ui::icons::LOG_TX,
                    LogKind::Rx => crate::ui::icons::LOG_RX,
                    LogKind::Firmware => crate::ui::icons::LOG_FIRMWARE,
                    LogKind::App => crate::ui::icons::LOG_APP,
                };
                let ts = format_timestamp(e.ts_ms);
                // 图标用 Phosphor 字体，文本用 Proportional，二者分两个 galley 拼接。
                let resp = ui.allocate_response(
                    egui::vec2(ui.available_width(), 18.0),
                    egui::Sense::hover(),
                );
                // 单行：图标 + 文本（时间戳 + 内容）
                crate::ui::fonts::paint_icon_text_in(
                    ui,
                    resp.rect,
                    arrow,
                    &format!(" {}  {}", ts, e.text),
                    12.0,
                    color,
                    4.0,
                );
                // 单行右键复制
                resp.context_menu(|ui| {
                    if ui.button("复制此行").clicked() {
                        ui.ctx().copy_text(format!("{}  {}", ts, e.text));
                        let _ = handle
                            .ui_tx
                            .send(UiEvent::Toast(ToastKind::Success, "已复制".to_string()));
                        ui.close();
                    }
                    if ui.button("复制消息内容").clicked() {
                        ui.ctx().copy_text(e.text.clone());
                        ui.close();
                    }
                });
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

fn level_visible(st: &LevelFilter, e: &crate::state::LogEntry) -> bool {
    match st {
        LevelFilter::All => true,
        LevelFilter::Info => !e.text.contains("[W]") && !e.text.contains("[E]"),
        LevelFilter::Warn => e.text.contains("[W]"),
        LevelFilter::Error => e.text.contains("[E]"),
    }
}

fn count_visible(entries: &[crate::state::LogEntry], st: &LogPanelState) -> usize {
    entries
        .iter()
        .filter(|e| {
            kind_visible(st, e.kind)
                && level_visible(&st.level, e)
                && (st.search.is_empty() || e.text.contains(&st.search))
        })
        .count()
}

/// 把可见日志条目渲染成纯文本（用于复制到剪贴板）
fn render_entries_text(entries: &[crate::state::LogEntry], st: &LogPanelState) -> String {
    // 按时间正序输出（旧 → 新），符合日志阅读习惯
    let mut visible: Vec<&crate::state::LogEntry> = entries
        .iter()
        .filter(|e| {
            kind_visible(st, e.kind)
                && level_visible(&st.level, e)
                && (st.search.is_empty() || e.text.contains(&st.search))
        })
        .collect();
    visible.sort_by_key(|e| e.ts_ms);

    let mut out = String::new();
    for e in visible.iter() {
        let kind = match e.kind {
            LogKind::Tx => "Tx",
            LogKind::Rx => "Rx",
            LogKind::Firmware => "FW",
            LogKind::App => "App",
        };
        out.push_str(&format!(
            "{} [{}] {}\n",
            format_timestamp(e.ts_ms),
            kind,
            e.text
        ));
    }
    out
}

fn format_timestamp(ms: u64) -> String {
    // 把 epoch ms 渲染成 HH:MM:SS.mmm
    let secs = (ms / 1000) as u64;
    let millis = (ms % 1000) as u32;
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}

// 保留 SharedLog 引用以避免警告（util/log 中定义了 SharedLog，但本面板直接走 handle.log_buf）
#[allow(dead_code)]
fn _shared_log_marker() -> SharedLog {
    SharedLog::new()
}
