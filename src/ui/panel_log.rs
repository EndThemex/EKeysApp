//! P7 Log 页面：日志查看器。

use std::sync::Arc;

use eframe::egui;

use crate::state::{AppHandle, LogEntry, LogKind, ToastKind, UiEvent};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LevelFilter {
    #[default]
    All,
    Info,
    Warn,
    Error,
}

/// 跨帧缓存的单行 galley。颜色已 bake 进 galley 内部 section 的
/// `TextFormat::color`，所以 `Label::new(galley)` 渲染时无需再传颜色。
#[derive(Clone)]
struct CachedRow {
    galley: Arc<egui::Galley>,
}

#[derive(Default)]
pub struct LogPanelState {
    pub show_tx: bool,
    pub show_rx: bool,
    pub show_fw: bool,
    pub show_app: bool,
    pub level: LevelFilter,
    pub search: String,
    /// 跟随最新日志（粘底自动滚动）。用户滚离底部后自动解除，滚回底部自动恢复。
    pub follow: bool,
    /// 一次性请求：滚动到最新日志（点击"跳转最新"按钮时置位）。
    pub jump_to_latest: bool,
    /// 跨帧持有的日志快照：每帧先比对 `SharedLog::current_version()`，
    /// 不变则 clone Arc 复用本字段；变化才调 `snapshot_arc()` 重取。
    /// 渲染循环里直接拿 `&Arc<Vec<LogEntry>>` 迭代，零字符串克隆。
    cached_entries: Option<Arc<Vec<LogEntry>>>,
    /// 上次 `cached_entries` 对应的 `SharedLog` 修订号。
    cached_version: u64,
    /// per-row galley cache：长度等于 `cached_entries.len()`。
    /// 缓存命中条件 = 同一 `SharedLog` 修订号（快照内容完全未变）。
    /// 主题切换 / dark 改变 → 整批失效（galley 颜色已 bake）。
    /// **关键收益**：稳态帧（无新日志）跳过全部 `Label` 内部
    /// `WidgetText::into_galley` 的 glyph 排布计算，渲染只是
    /// `Label::new(galley.clone()).selectable(true).wrap_mode(Extend)`
    /// 走一遍 paint 路径。
    cached_galleys: Vec<Option<CachedRow>>,
    /// `cached_galleys` 写入时的主题状态；下一次 dark 与之不等则整批重建。
    cached_dark: bool,
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut LogPanelState) {
    // 跨帧快照：仅当 SharedLog 修订号前进时才 clone 新 Arc，否则复用上一帧
    // 的 Arc<Vec<LogEntry>>，渲染循环里 0 次字符串深拷贝。
    let cur_version = handle.log.current_version();
    if st.cached_version != cur_version || st.cached_entries.is_none() {
        st.cached_entries = Some(handle.log.snapshot_arc());
        st.cached_version = cur_version;
        // 快照变更 → 清空 per-row galley 缓存，新版本会逐行重建。
        st.cached_galleys.clear();
    }
    let cached_entries: Arc<Vec<LogEntry>> = st
        .cached_entries
        .as_ref()
        .map(Arc::clone)
        .expect("刚刚填充");
    let dark = ui.visuals().dark_mode;
    // 主题切换：颜色已 bake 进 galley，旧缓存的颜色不再准确 → 整批失效。
    if st.cached_dark != dark {
        st.cached_galleys.clear();
        st.cached_dark = dark;
    }

    ui.heading("运行日志");
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
            ui.checkbox(&mut st.show_tx, "上行数据");
            ui.checkbox(&mut st.show_rx, "下行数据");
            ui.checkbox(&mut st.show_fw, "设备日志");
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
                    LevelFilter::Info => "提示".to_string(),
                    LevelFilter::Warn => "警告".to_string(),
                    LevelFilter::Error => "错误".to_string(),
                })
                .show_ui(ui, |cb| {
                    cb.selectable_value(&mut st.level, LevelFilter::All, "全部");
                    cb.selectable_value(&mut st.level, LevelFilter::Info, "提示");
                    cb.selectable_value(&mut st.level, LevelFilter::Warn, "警告");
                    cb.selectable_value(&mut st.level, LevelFilter::Error, "错误");
                });
            ui.add(
                egui::TextEdit::singleline(&mut st.search)
                    .hint_text("搜索日志内容…")
                    .desired_width(280.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // 清空日志 —— 必须走 IconTextButton：直接把 PUA 图标塞进 ui.button
                // 字符串会让字形落到 Proportional（无 PUA 字形），渲染成 □。
                if ui
                    .add(
                        crate::ui::fonts::IconTextButton::new(
                            crate::ui::icons::LOG_CLEAR,
                            "清空日志",
                            13.0,
                        )
                        .corner_radius(egui::CornerRadius::same(6)),
                    )
                    .clicked()
                {
                    handle.log.clear();
                }
                // 复制全部 —— 主操作：ACCENT 底 + 白字。
                let copy_btn = crate::ui::fonts::IconTextButton::new(
                    crate::ui::icons::LOG_COPY,
                    "复制全部",
                    13.0,
                )
                .fill(crate::ui::ACCENT)
                .fg(egui::Color32::WHITE)
                .corner_radius(egui::CornerRadius::same(6));
                if ui.add(copy_btn).clicked() {
                    let text = render_entries_text(&cached_entries, st);
                    if text.is_empty() {
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            ToastKind::Info,
                            "当前筛选下没有可复制的日志".to_string(),
                        ));
                    } else {
                        ui.ctx().copy_text(text.clone());
                        let _ = handle.ui_tx.send(UiEvent::Toast(
                            ToastKind::Success,
                            format!(
                                "已复制 {count} 行日志",
                                count = count_visible(&cached_entries, st)
                            ),
                        ));
                    }
                }
            });
        });
    });
    ui.add_space(8.0);

    let total_visible = count_visible(&cached_entries, st);
    ui.label(
        egui::RichText::new(format!(
            "共 {total} 条，当前筛选显示 {visible} 条",
            total = cached_entries.len(),
            visible = total_visible
        ))
        .small()
        .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(4.0);

    // 时间正序渲染（旧 → 新，最新在底部）：新日志追加在内容末尾，
    // 用户上滚查看历史时视口不会被新日志推动。
    //
    // per-row galley 缓存：稳定状态下（SharedLog 修订号未变 + 主题未变）
    // 直接 `Label::new(galley.clone())`，跳过 egui 内部的 glyph 排布计算。
    // cache 长度保持等于 `cached_entries.len()`：被过滤掉的行用 None 占位
    // （保留槽位以索引 log_row id）；新增行触发 vec resize + 末尾逐行 layout。
    let snap_len = cached_entries.len();
    if st.cached_galleys.len() < snap_len {
        st.cached_galleys.resize(snap_len, None);
    } else if st.cached_galleys.len() > snap_len {
        // 环缓冲弹出最旧条目：缩到当前长度。
        st.cached_galleys.truncate(snap_len);
    }

    let out = egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .stick_to_bottom(st.follow)
        .show(ui, |ui| {
            for (i, e) in cached_entries.iter().enumerate() {
                if !kind_visible(st, e.kind) {
                    continue;
                }
                if !level_visible(&st.level, e) {
                    continue;
                }
                if !st.search.is_empty() && !e.text.contains(&st.search) {
                    continue;
                }

                // 日志配色按主题取色：原色按深底调亮，白底下不可读
                // （FW_INFO / APP 近白，TX 低对比）。
                let color = log_row_color(dark, e);
                let arrow = match e.kind {
                    LogKind::Tx => crate::ui::icons::LOG_TX,
                    LogKind::Rx => crate::ui::icons::LOG_RX,
                    LogKind::Firmware => crate::ui::icons::LOG_FIRMWARE,
                    LogKind::App => crate::ui::icons::LOG_APP,
                };

                // 取/建 galley：galley 颜色已 bake，无需再传 RichText。
                let galley = if let Some(c) = st.cached_galleys.get(i).and_then(|x| x.as_ref()) {
                    c.galley.clone()
                } else {
                    let ts = format_timestamp(e.ts_ms);
                    let text = format!("{}  {}", ts, e.text);
                    let g = ui.painter().layout(
                        text,
                        egui::FontId::proportional(12.0),
                        color,
                        f32::INFINITY,
                    );
                    // cache 该行：跨帧复用。注意 vec 容量已经按 snap_len 调整好。
                    if i < st.cached_galleys.len() {
                        st.cached_galleys[i] = Some(CachedRow { galley: g.clone() });
                    }
                    g
                };

                let row = ui.horizontal(|ui| {
                    // 图标占位：高度 0，行高由 galley 决定；egui 在行内垂直居中。
                    let slot = ui.allocate_exact_size(egui::vec2(12.0, 0.0), egui::Sense::hover());
                    // 直接喂 pre-laid-out galley：Label 只负责 paint + selectable + hit-test。
                    let text_resp = ui.add(
                        egui::Label::new(galley.clone())
                            .selectable(true)
                            .wrap_mode(egui::TextWrapMode::Extend),
                    );
                    // 图标按光学中心对齐到文本行竖直中线。
                    crate::ui::fonts::paint_icon_at(
                        ui,
                        egui::pos2(slot.0.left(), text_resp.rect.center().y),
                        arrow,
                        color,
                        12.0,
                    );
                });
                // 整行右键复制
                let row_id = egui::Id::new(("log_row", e.ts_ms, e.text.as_str()));
                ui.interact(row.response.rect, row_id, egui::Sense::hover())
                    .context_menu(|ui| {
                        let ts = format_timestamp(e.ts_ms);
                        let line = format!("{}  {}", ts, e.text);
                        if ui.button("复制此行（含时间）").clicked() {
                            ui.ctx().copy_text(line.clone());
                            let _ = handle
                                .ui_tx
                                .send(UiEvent::Toast(ToastKind::Success, "已复制此行".to_string()));
                            ui.close();
                        }
                        if ui.button("仅复制消息内容").clicked() {
                            ui.ctx().copy_text(e.text.clone());
                            ui.close();
                        }
                    });
            }

            // 跳转最新：把内容末尾滚到视口底部；到底后 egui 会自动恢复粘底跟随。
            if st.jump_to_latest {
                ui.scroll_to_cursor(Some(egui::Align::Max));
                st.jump_to_latest = false;
            }
        });

    // 跟随状态收敛：
    // - 位于底部（offset == max）→ 保持/恢复跟随；
    // - 用户滚离底部 → 解除跟随，新日志不再推动视口。
    let diff = out.content_size.y - out.inner_rect.height() - out.state.offset.y;
    let at_bottom = diff <= 0.5;
    st.follow = at_bottom;

    // “跳转最新”悬浮按钮：用户滚离底部时显示在日志区右下角。
    // 悬浮在 ScrollArea 内容之上属于无法用常规布局表达的定位，故使用精确 Rect。
    if !st.follow {
        let label = "回到最新";
        let font = egui::TextStyle::Button.resolve(ui.style());
        let galley = ui
            .painter()
            .layout_no_wrap(label.to_owned(), font, egui::Color32::WHITE);
        let pad = ui.spacing().button_padding;
        let size = egui::vec2(
            galley.rect.width() + pad.x * 2.0,
            galley.rect.height() + pad.y * 2.0,
        );
        let margin = 12.0;
        let rect = egui::Rect::from_min_size(
            out.inner_rect.right_bottom() - egui::vec2(size.x + margin, size.y + margin),
            size,
        );
        let btn = egui::Button::new(
            egui::RichText::new(label)
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(crate::ui::ACCENT)
        .corner_radius(egui::CornerRadius::same(6));
        if ui.put(rect, btn).clicked() {
            st.jump_to_latest = true;
        }
    }
}

fn kind_visible(st: &LogPanelState, k: LogKind) -> bool {
    match k {
        LogKind::Tx => st.show_tx,
        LogKind::Rx => st.show_rx,
        LogKind::Firmware => st.show_fw,
        LogKind::App => st.show_app,
    }
}

/// 单行日志的渲染颜色。深 / 浅主题走 `colors::themed`；Firmware 行
/// 还根据 `[W]` / `[E]` 标记切到警告 / 错误色。仅依赖 `dark + LogEntry`，
/// 提出来便于在循环外预计算（未来如果需要更细的缓存键）。
fn log_row_color(dark: bool, e: &LogEntry) -> egui::Color32 {
    match e.kind {
        LogKind::Tx => crate::ui::colors::themed(
            dark,
            crate::ui::colors::TX,
            crate::ui::colors::TX_L,
        ),
        LogKind::Rx => crate::ui::colors::themed(
            dark,
            crate::ui::colors::RX,
            crate::ui::colors::RX_L,
        ),
        LogKind::Firmware if e.text.contains("[E]") => crate::ui::colors::themed(
            dark,
            crate::ui::colors::FW_ERROR,
            crate::ui::colors::FW_ERROR_L,
        ),
        LogKind::Firmware if e.text.contains("[W]") => crate::ui::colors::themed(
            dark,
            crate::ui::colors::FW_WARN,
            crate::ui::colors::FW_WARN_L,
        ),
        LogKind::Firmware => crate::ui::colors::themed(
            dark,
            crate::ui::colors::FW_INFO,
            crate::ui::colors::FW_INFO_L,
        ),
        LogKind::App => crate::ui::colors::themed(
            dark,
            crate::ui::colors::APP,
            crate::ui::colors::APP_L,
        ),
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

fn count_visible(entries: &Arc<Vec<LogEntry>>, st: &LogPanelState) -> usize {
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
fn render_entries_text(entries: &Arc<Vec<LogEntry>>, st: &LogPanelState) -> String {
    // 按时间正序输出（旧 → 新），符合日志阅读习惯
    let mut visible: Vec<&LogEntry> = entries
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
            LogKind::Tx => "上行",
            LogKind::Rx => "下行",
            LogKind::Firmware => "设备",
            LogKind::App => "应用",
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
