//! P3 Keymap 页面：键盘图片背景 + 键位栅格 + 右侧功能分配 Drawer。
//!
//! 阶段 05 之前：本地草稿编辑 + DiffPreviewBar，不接协议命令；
//! 阶段 05 起：将 `KeymapData` 通过 CMD_KEYMAP_GET 拉取，Save 时通过
//! CMD_KEYMAP_SET 增量下发。`apply_keymap_diff` 已留好接入点。

use std::time::Instant;

use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::protocol::{
    EncoderAction, KeyAction, KeymapData, KeymapDiffEntry, MediaKey, MouseAction, SlotKind,
};
use crate::state::AppHandle;

const ROW_COUNT: usize = 3;
/// 1u = 48 像素（与渲染区高度计算保持一致）
const UNIT_PX: f32 = 44.0;
const KEY_GAP: f32 = 4.0;
const ROW_GAP: f32 = 4.0;
/// 1.25u / 1.5u 等非整数宽度按键的圆角微调
const KEY_RADIUS: f32 = 5.0;

#[derive(Default)]
pub struct KeymapPanelState {
    /// 当前编辑层
    pub layer: u8,
    /// Drawer 中的动作类型（None 表示未编辑）
    pub draft_action: Option<KeyAction>,
    /// 选中变化是否触发了"未应用"
    pub dirty: bool,
    /// Drawer 普通键分支是否进入"按下捕获"模式
    pub capture_keyboard: bool,
    /// 捕获模式下当前高亮的 keycode（按下时写入）
    pub captured_code: Option<u16>,
    /// 当前是否处于 Profile 重命名模式
    pub renaming_profile: bool,
    /// 重命名模式下 TextEdit 的临时字符串
    pub profile_name_edit: String,
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, st: &mut KeymapPanelState) {
    ui.heading("键映射");
    ui.label(
        egui::RichText::new("对接 CMD_KEYMAP_GET / SET：重新加载从设备拉取，下发写入当前 Profile")
            .weak()
            .size(11.0),
    );
    ui.add_space(6.0);

    // 顶部控制条：Profile 选择 + Layer 选择 + 操作按钮
    top_controls(handle, ui, st);
    ui.separator();
    ui.add_space(4.0);

    // 图例
    ui.horizontal(|ui| {
        legend_dot(
            ui,
            Color32::from_rgb(0x28, 0x2C, 0x36),
            Color32::from_rgb(0x44, 0x4A, 0x55),
            "未绑定",
        );
        legend_dot(
            ui,
            Color32::from_rgb(0x2C, 0x46, 0x7A),
            Color32::from_rgb(0x6A, 0x88, 0xC0),
            "已应用",
        );
        legend_dot(
            ui,
            Color32::from_rgb(0xC0, 0x80, 0x20),
            Color32::from_rgb(0xFF, 0xC8, 0x60),
            "待下发",
        );
        legend_dot(
            ui,
            Color32::from_rgb(0x4F, 0x8C, 0xFF),
            Color32::WHITE,
            "选中",
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Δ 表示差异（右上角琥珀点）")
                .weak()
                .size(11.0),
        );
    });
    ui.add_space(4.0);

    // 主体：左侧键盘图，右侧 Drawer
    let snapshot = handle.keymap.lock().unwrap().clone();
    let draft = handle.keymap_draft.lock().unwrap().clone();
    let diff = draft.diff_bindings(&snapshot);

    let avail = ui.available_size();
    // Drawer 固定 280 宽；键盘图占据剩下的空间。
    let drawer_w = 280.0_f32.min((avail.x - 32.0).max(220.0));
    let keyboard_w = (avail.x - drawer_w - 24.0).max(360.0);
    // 键盘图高度按 3 行 × 4 列 ≈ 0.75 比例自适应（高度 = 宽度 × 3/4），
    // 再额外扣除顶部占位文字与 padding，最少 180，撑满可用高度（再减去 DiffBar 高度）
    let keyboard_h = (keyboard_w * 0.36).clamp(180.0, (avail.y - 80.0).max(220.0));

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;

        // 左：键盘图（固定高度矩形，下面留出 Drawer 完整空间）
        ui.allocate_ui(Vec2::new(keyboard_w, keyboard_h), |ui| {
            draw_keyboard(handle, ui, st, &draft, &snapshot);
        });

        // 右：功能分配 Drawer —— 给定宽度 + **撑满剩余高度**，内容超出滚动
        let drawer_h = (avail.y - 90.0).max(240.0);
        ui.allocate_ui(Vec2::new(drawer_w, drawer_h), |ui| {
            drawer(ui, handle, st, &draft, &snapshot);
        });
    });

    // 底部：DiffPreviewBar
    ui.add_space(8.0);
    let action = show_keymap_diff_bar(ui, &diff);
    match action {
        KeymapDiffAction::Apply => {
            // 0x06 CMD_KEYMAP_SET：把当前 profile 的 11 键整表下发（固件按
            // physical 逐键覆盖，属于"整包覆盖"语义；增量精确定位留待固件
            // 支持按 physical 分项后再说）。
            use crate::protocol::{CMD_KEYMAP_SET, KeymapSetReq};
            use std::time::Duration;
            let req = KeymapSetReq {
                keymap: draft.to_firmware_entries(),
            };
            let data = serde_json::to_value(&req).ok();
            let mut success = false;
            let _ = handle.with_link(|lm| {
                match lm.request(CMD_KEYMAP_SET, data, Duration::from_millis(1500)) {
                    Ok(frame) => {
                        if frame.status() == Some(0) {
                            success = true;
                        } else {
                            let msg = frame.error.unwrap_or_else(|| "固件拒绝键映射".to_string());
                            let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                                crate::state::ToastKind::Error,
                                format!("下发失败: {msg}"),
                            ));
                        }
                    }
                    Err(e) => {
                        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                            crate::state::ToastKind::Error,
                            format!("下发超时: {e}"),
                        ));
                    }
                }
            });
            if success {
                // 固件 ACK 后才落本地快照，避免失败时 UI 状态与实际不符
                let mut snap = handle.keymap.lock().unwrap();
                snap.apply_diff(&diff);
                handle.log_kind(
                    crate::state::LogKind::Tx,
                    format!("下发键映射 → {} 项变更", diff.len()),
                );
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Success,
                    format!("已下发 {} 项键映射变更", diff.len()),
                ));
            } else {
                handle.log_kind(
                    crate::state::LogKind::App,
                    "键映射下发未成功，保留本地草稿待重试".to_string(),
                );
            }
        }
        KeymapDiffAction::Discard => {
            *handle.keymap_draft.lock().unwrap() = snapshot.clone();
            st.dirty = false;
        }
        KeymapDiffAction::None => {}
    }

    // 顶部克隆的 draft 已经在 drawer / top_controls 里按需写入 mutex。
    // 这里不要再无条件回写——会把抽屉里 Save 的改动覆盖掉，导致样式
    // 不更新。`st.dirty` 仅作为旧 API 保留，当前未使用。
}

// -------- 顶部控制条 --------

/// 在图例栏画一个色块 + 标签
fn legend_dot(ui: &mut egui::Ui, fill: Color32, stroke: Color32, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
    let r = rect.shrink(1.0);
    ui.painter().rect_filled(r, 3.0, fill);
    ui.painter()
        .rect_stroke(r, 3.0, Stroke::new(1.0, stroke), StrokeKind::Middle);
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).size(11.0));
    ui.add_space(10.0);
}

fn top_controls(handle: &AppHandle, ui: &mut egui::Ui, st: &mut KeymapPanelState) {
    let snap = handle.keymap.lock().unwrap().clone();
    let mut draft = handle.keymap_draft.lock().unwrap();

    ui.horizontal(|ui| {
        // Profile
        ui.label("配置:");
        let mut p = draft.active_profile as i32;
        // ComboBox 只显示用户命名，默认值"P{i}"在 make_demo_profile 中设置
        let current_name = draft
            .profile(p as u8)
            .map(|x| x.name.clone())
            .unwrap_or_else(|| format!("P{p}"));
        egui::ComboBox::from_id_salt("keymap-profile")
            .selected_text(current_name.clone())
            .show_ui(ui, |cb| {
                for prof in draft.profiles.iter() {
                    cb.selectable_value(&mut p, prof.index as i32, prof.name.clone());
                }
            });
        if p as u8 != draft.active_profile {
            draft.active_profile = p as u8;
            // 切 profile 时清空选中键（避免上一个 profile 的 KeyRef 误导当前）
            *handle.selected_key.lock().unwrap() = None;
            st.dirty = true;
        }

        // 重命名按钮
        let rename_icon = if st.renaming_profile {
            crate::ui::icons::KEYMAP_RENAME_CLOSE
        } else {
            crate::ui::icons::KEYMAP_RENAME_EDIT
        };
        if ui
            .add(egui::Button::new(crate::ui::fonts::icon_rich(
                rename_icon,
                16.0,
            )))
            .on_hover_text(if st.renaming_profile {
                "取消重命名"
            } else {
                "重命名当前 Profile"
            })
            .clicked()
        {
            st.renaming_profile = !st.renaming_profile;
            if st.renaming_profile {
                st.profile_name_edit = current_name;
            }
        }

        if st.renaming_profile {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut st.profile_name_edit)
                    .hint_text("输入新名称")
                    .desired_width(120.0),
            );
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui
                .add(egui::Button::new(crate::ui::fonts::icon_rich(
                    crate::ui::icons::KEYMAP_CONFIRM,
                    14.0,
                )))
                .on_hover_text("保存重命名")
                .clicked()
                || enter
            {
                commit_rename(handle, &mut *draft, p as u8, &st.profile_name_edit);
                st.renaming_profile = false;
            }
        }

        ui.separator();

        // Layer
        ui.label("层:");
        let layers: Vec<(u8, String)> = snap
            .profile(draft.active_profile)
            .map(|prof| {
                prof.layers
                    .iter()
                    .map(|l| (l.index, l.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let mut li = st.layer as i32;
        egui::ComboBox::from_id_salt("keymap-layer")
            .selected_text(
                layers
                    .iter()
                    .find(|(i, _)| *i == st.layer)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| format!("层 {}", st.layer)),
            )
            .show_ui(ui, |cb| {
                for (i, name) in &layers {
                    cb.selectable_value(&mut li, *i as i32, name.clone());
                }
            });
        if li as u8 != st.layer {
            st.layer = li as u8;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(crate::ui::fonts::IconTextButton::new(
                    crate::ui::icons::KEYMAP_RELOAD,
                    "重新加载",
                    14.0,
                ))
                .clicked()
            {
                // 0x05 CMD_KEYMAP_GET：从固件拉当前 profile 的 11 键，
                // 刷新快照 + 草稿。响应 `keymap` 在帧顶层（Frame::extra），
                // 值是数组本体（不是 {"keymap": [...]} wrapper）。
                use crate::protocol::CMD_KEYMAP_GET;
                use std::time::Duration;
                let mut changed = false;
                let _ = handle.with_link(|lm| {
                    match lm.request(CMD_KEYMAP_GET, None, Duration::from_millis(1000)) {
                        Ok(frame) => {
                            if let Some(v) = frame.extra_value("keymap") {
                                if let Ok(entries) = serde_json::from_value::<
                                    Vec<crate::protocol::FirmwareKeyEntry>,
                                >(v.clone())
                                {
                                    let mut snap = handle.keymap.lock().unwrap();
                                    snap.apply_firmware_entries(&entries);
                                    drop(snap);
                                    let mut draft = handle.keymap_draft.lock().unwrap();
                                    changed = draft.apply_firmware_entries(&entries);
                                    drop(draft);
                                    *handle.selected_key.lock().unwrap() = None;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                                crate::state::ToastKind::Error,
                                format!("重新加载失败: {e}"),
                            ));
                        }
                    }
                });
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    if changed {
                        crate::state::ToastKind::Success
                    } else {
                        crate::state::ToastKind::Info
                    },
                    if changed {
                        "已从设备加载键映射".to_string()
                    } else {
                        "键映射与设备一致".to_string()
                    },
                ));
            }
            if ui
                .add(crate::ui::fonts::IconTextButton::new(
                    crate::ui::icons::KEYMAP_EXPORT,
                    "导出",
                    14.0,
                ))
                .clicked()
            {
                let path = std::env::temp_dir().join("ekey_keymap.json");
                if let Ok(s) = serde_json::to_string_pretty(&*draft) {
                    let _ = std::fs::write(&path, s);
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Success,
                        format!("已导出到 {}", path.display()),
                    ));
                }
            }
        });
    });
}

// -------- 键盘图 + 键位 --------

fn draw_keyboard(
    handle: &AppHandle,
    ui: &mut egui::Ui,
    st: &mut KeymapPanelState,
    draft: &KeymapData,
    snapshot: &KeymapData,
) {
    let (rect, _resp) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);

    // 背景：占位深色块 + "键盘背景图" 文字
    painter.rect_filled(rect, 8.0, Color32::from_rgb(0x18, 0x1B, 0x22));
    painter.text(
        rect.left_top() + Vec2::new(10.0, 6.0),
        egui::Align2::LEFT_TOP,
        "键盘背景图（待接入 PNG）",
        egui::FontId::proportional(11.0),
        Color32::from_rgb(0x70, 0x70, 0x80),
    );

    // 当前 profile 的当前 layer 的 slots。
    // 如果 active_profile 不存在（设备返回了不存在的索引），自动 fallback
    // 到 profile(0) 避免整个键盘区消失。
    let effective_profile_idx = draft
        .profile(draft.active_profile)
        .map(|_| draft.active_profile)
        .or_else(|| draft.profiles.first().map(|p| p.index))
        .unwrap_or(0);
    let Some(profile) = draft.profile(effective_profile_idx) else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "没有可用的 Profile",
            egui::FontId::proportional(14.0),
            Color32::from_gray(140),
        );
        return;
    };
    let Some(layer) = profile.layers.iter().find(|l| l.index == st.layer) else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "当前 Layer 没有按键",
            egui::FontId::proportional(14.0),
            Color32::from_gray(140),
        );
        return;
    };

    // 按 row 分组
    let mut rows: Vec<Vec<&crate::protocol::KeySlot>> = vec![vec![]; ROW_COUNT];
    for s in &layer.slots {
        let r = (s.row as usize).min(ROW_COUNT - 1);
        rows[r].push(s);
    }

    let padding = 10.0;
    let inner_w = rect.width() - padding * 2.0;
    let inner_h = rect.height() - padding * 2.0 - 14.0; // 留 14px 给顶部占位文字
    let row_h = inner_h / ROW_COUNT as f32;

    let selected = handle.selected_key.lock().unwrap().clone();

    for (r_idx, row) in rows.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        // 计算每行的总 units，按比例分配 inner_w
        let total_units: f32 = row.iter().map(|s| s.width_units).sum();
        let avail_w = inner_w;
        let u_px = avail_w / total_units;
        let key_h = (row_h - ROW_GAP).max(20.0);

        let y = rect.top() + padding + 14.0 + r_idx as f32 * row_h;
        let mut x = rect.left() + padding;

        for slot in row {
            let w = slot.width_units * u_px - KEY_GAP;
            let r = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w.max(8.0), key_h));
            // 同时取 snapshot 的 binding 用于判断"是否待下发"
            let snap_bindings = snapshot
                .profile(snapshot.active_profile)
                .map(|p| &p.bindings);
            if matches!(slot.kind, SlotKind::Encoder) {
                draw_encoder(
                    ui,
                    &painter,
                    r,
                    slot,
                    &profile.bindings,
                    snap_bindings,
                    &selected,
                    handle,
                );
            } else {
                draw_key(
                    ui,
                    &painter,
                    r,
                    slot,
                    &profile.bindings,
                    snap_bindings,
                    &selected,
                    handle,
                );
            }
            x += slot.width_units * u_px;
        }
    }
}

fn draw_key(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    slot: &crate::protocol::KeySlot,
    bindings: &std::collections::HashMap<crate::protocol::KeyRef, KeyAction>,
    snap_bindings: Option<&std::collections::HashMap<crate::protocol::KeyRef, KeyAction>>,
    selected: &Option<crate::protocol::KeyRef>,
    handle: &AppHandle,
) {
    let key_ref = crate::protocol::KeyRef {
        layer: 0, // 当前 layer 在 panel 中用 st.layer 表达；这里只是查找占位
        row: slot.row,
        col: slot.col,
    };
    let binding = bindings.get(&key_ref);
    // draft 与 snapshot 是否一致：决定键格是"已应用"还是"待下发"
    let snap_binding = snap_bindings.and_then(|m| m.get(&key_ref));
    let is_pending = snap_binding != binding;

    let is_sel = selected
        .map(|k| k.row == slot.row && k.col == slot.col)
        .unwrap_or(false);

    // 基础颜色（按 binding 状态决定；选中态在最后再单独叠一层外圈高亮）
    let (fill, stroke) = if is_pending && binding.map(|b| b.is_set()).unwrap_or(false) {
        // 待下发（draft 有但与 snapshot 不同）—— 琥珀色
        (
            Color32::from_rgb(0xC0, 0x80, 0x20),
            Stroke::new(1.0, Color32::from_rgb(0xFF, 0xC8, 0x60)),
        )
    } else if binding.map(|b| b.is_set()).unwrap_or(false) {
        // 已应用（draft 与 snapshot 一致且非空）
        (
            Color32::from_rgb(0x2C, 0x46, 0x7A),
            Stroke::new(1.0, Color32::from_rgb(0x6A, 0x88, 0xC0)),
        )
    } else if is_pending {
        // draft 是 None 但 snapshot 有值 → 也是"待下发"（删绑定）
        (
            Color32::from_rgb(0x44, 0x44, 0x44),
            Stroke::new(1.0, Color32::from_rgb(0x88, 0x88, 0x88)),
        )
    } else {
        (
            Color32::from_rgb(0x28, 0x2C, 0x36),
            Stroke::new(1.0, Color32::from_rgb(0x44, 0x4A, 0x55)),
        )
    };

    painter.rect_filled(rect, KEY_RADIUS, fill);
    painter.rect_stroke(rect, KEY_RADIUS, stroke, StrokeKind::Middle);

    // 选中态：外圈叠加一层白色高亮框（不覆盖基础色）
    if is_sel {
        painter.rect_stroke(
            rect.shrink(0.5),
            KEY_RADIUS,
            Stroke::new(2.0, Color32::WHITE),
            StrokeKind::Middle,
        );
    }

    // 待下发标记：右上角小三角点
    if is_pending {
        let dot_r = 3.5;
        let cx = rect.right() - 6.0;
        let cy = rect.top() + 6.0;
        painter.circle_filled(
            egui::pos2(cx, cy),
            dot_r,
            Color32::from_rgb(0xFF, 0xC0, 0x40),
        );
    }

    // 标签
    let font = egui::FontId::proportional(if rect.width() > 60.0 { 11.0 } else { 9.5 });
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &slot.label,
        font,
        Color32::from_rgb(0xE0, 0xE5, 0xF0),
    );

    // 鼠标点击 / hover
    let id = ui.id().with(("key", slot.row, slot.col));
    let resp = ui.interact(rect, id, Sense::click());
    let hovered = resp.hovered();
    let clicked = resp.clicked();
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        let status = if is_pending { "待下发" } else { "已应用" };
        resp.clone().on_hover_text(format!(
            "{} ({},{}) {status} binding: {b}",
            slot.label,
            slot.row,
            slot.col,
            status = status,
            b = binding
                .map(|b| b.label())
                .unwrap_or_else(|| "未绑定".into())
        ));
    }
    if clicked {
        let mut sel = handle.selected_key.lock().unwrap();
        *sel = Some(crate::protocol::KeyRef {
            layer: 0,
            row: slot.row,
            col: slot.col,
        });
        // 把当前 binding 写到 pending_binding（Drawer 会显示它）
        let mut pending = handle.pending_binding.lock().unwrap();
        *pending = binding.cloned();
    }
}

/// 旋钮渲染：圆盘 + 刻度 + 当前角度（CW → 顺时针点）。选中态同按键逻辑。
fn draw_encoder(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    rect: Rect,
    slot: &crate::protocol::KeySlot,
    bindings: &std::collections::HashMap<crate::protocol::KeyRef, KeyAction>,
    snap_bindings: Option<&std::collections::HashMap<crate::protocol::KeyRef, KeyAction>>,
    selected: &Option<crate::protocol::KeyRef>,
    handle: &AppHandle,
) {
    let key_ref = crate::protocol::KeyRef {
        layer: 0,
        row: slot.row,
        col: slot.col,
    };
    let binding = bindings.get(&key_ref);
    let snap_binding = snap_bindings.and_then(|m| m.get(&key_ref));
    let is_pending = snap_binding != binding;

    let is_sel = selected
        .map(|k| k.row == slot.row && k.col == slot.col)
        .unwrap_or(false);

    // 旋钮按 inset 缩进一圈再画圆
    let pad = 4.0;
    let r = Rect::from_center_size(
        rect.center(),
        Vec2::new(rect.width() - pad, rect.height() - pad),
    );
    let radius = r.width().min(r.height()) * 0.5;

    // 基础颜色（按 binding 状态决定；选中态只改外圈描边）
    let body = if is_pending && binding.map(|b| b.is_set()).unwrap_or(false) {
        Color32::from_rgb(0xC0, 0x80, 0x20)
    } else if binding.map(|b| b.is_set()).unwrap_or(false) {
        Color32::from_rgb(0x2C, 0x46, 0x7A)
    } else {
        Color32::from_rgb(0x28, 0x2C, 0x36)
    };
    painter.circle_filled(r.center(), radius, body);
    let ring_color = if is_pending {
        Color32::from_rgb(0xFF, 0xC8, 0x60)
    } else {
        Color32::from_rgb(0x44, 0x4A, 0x55)
    };
    painter.circle_stroke(
        r.center(),
        radius,
        Stroke::new(if is_pending { 2.0 } else { 1.0 }, ring_color),
    );
    // 选中态：外圈额外加一圈白色高亮（半径略外移以让描边可见）
    if is_sel {
        painter.circle_stroke(r.center(), radius + 2.5, Stroke::new(2.0, Color32::WHITE));
    }

    // 12 段刻度
    let ticks = 12;
    for i in 0..ticks {
        let a = std::f32::consts::TAU * (i as f32) / (ticks as f32) - std::f32::consts::FRAC_PI_2;
        let inner = radius - 3.0;
        let outer = radius;
        let p1 = egui::pos2(
            r.center().x + a.cos() * inner,
            r.center().y + a.sin() * inner,
        );
        let p2 = egui::pos2(
            r.center().x + a.cos() * outer,
            r.center().y + a.sin() * outer,
        );
        painter.line_segment(
            [p1, p2],
            Stroke::new(1.0, Color32::from_rgb(0x6A, 0x88, 0xC0)),
        );
    }

    // 中心文字（标签 / 缩写）
    painter.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        "K",
        egui::FontId::proportional(13.0),
        Color32::from_rgb(0xE0, 0xE5, 0xF0),
    );

    // 待下发标记：右上角小三角点
    if is_pending && !is_sel {
        let dot_r = 3.5;
        let cx = rect.right() - 6.0;
        let cy = rect.top() + 6.0;
        painter.circle_filled(
            egui::pos2(cx, cy),
            dot_r,
            Color32::from_rgb(0xFF, 0xC0, 0x40),
        );
    }

    // 鼠标交互（点击即选中；旋转留待阶段 05 通过拖拽事件触发绑定变更）
    let id = ui.id().with(("enc", slot.row, slot.col));
    let resp = ui.interact(rect, id, Sense::click());
    let hovered = resp.hovered();
    let clicked = resp.clicked();

    // 中心指示点：仅在 hover 或选中时显示（呼吸式脉动），未交互不绘制、不请求重绘。
    if hovered || is_sel {
        let pulse = (0.5 + 0.5 * Instant::now().elapsed().as_secs_f32().sin()) as f32;
        let alpha = (0.55 + pulse * 0.45) as f32;
        let base = Color32::from_rgb(0xFF, 0xC0, 0x60);
        let dot_color =
            Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), (alpha * 255.0) as u8);
        let ir = radius * 0.55;
        let angle = pulse * std::f32::consts::TAU;
        let ip = egui::pos2(
            r.center().x + angle.cos() * ir,
            r.center().y + angle.sin() * ir,
        );
        painter.circle_filled(ip, 3.0, dot_color);
        // 仅在动画进行时按需重绘（~30 fps），避免空闲时持续重绘。
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        let status = if is_pending { "待下发" } else { "已应用" };
        resp.clone().on_hover_text(format!(
            "{} ({},{}) 旋钮 {status} binding: {b}",
            slot.label,
            slot.row,
            slot.col,
            status = status,
            b = binding
                .map(|b| b.label())
                .unwrap_or_else(|| "未绑定".into())
        ));
    }
    if clicked {
        let mut sel = handle.selected_key.lock().unwrap();
        *sel = Some(crate::protocol::KeyRef {
            layer: 0,
            row: slot.row,
            col: slot.col,
        });
        let mut pending = handle.pending_binding.lock().unwrap();
        *pending = binding.cloned();
    }
}

// -------- 右侧 Drawer --------

fn drawer(
    ui: &mut egui::Ui,
    handle: &AppHandle,
    st: &mut KeymapPanelState,
    draft: &KeymapData,
    snapshot: &KeymapData,
) {
    let selected = handle.selected_key.lock().unwrap().clone();
    crate::ui::card(ui, |ui| {
        // 高度撑满父容器
        ui.set_min_height(ui.available_height());
        ui.vertical(|ui| {
            // 内容超出时滚动，避免子控件被截断
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(ui.available_height() - 6.0)
                .show(ui, |ui| {
                    ui.strong("分配功能");
                    ui.add_space(4.0);
                    match selected {
                        None => {
                            // 切走选中时清掉草稿动作
                            st.draft_action = None;
                            ui.label(
                                egui::RichText::new("点击键盘上的任意按键，开始分配功能")
                                    .weak()
                                    .size(12.0),
                            );
                        }
                        Some(kref) => {
                            // 当前 draft 中的绑定（持久化的最新状态）
                            let cur_binding = draft
                                .profile(draft.active_profile)
                                .and_then(|p| p.bindings.get(&kref))
                                .cloned()
                                .unwrap_or(KeyAction::None);

                            // 选中键变化时，用 cur_binding 同步一次草稿动作
                            if st.draft_action.is_none() {
                                st.draft_action = Some(cur_binding.clone());
                            }
                            // 注意：这里**不要**每帧从 cur_binding 反推 kind——会导致
                            // 用户改了类型后下一帧又被冲掉。详情见本文件顶部注释。
                            // 找一下对应的 slot label
                            let label = draft
                                .profile(draft.active_profile)
                                .and_then(|p| p.layers.iter().find(|l| l.index == kref.layer))
                                .and_then(|l| {
                                    l.slots
                                        .iter()
                                        .find(|s| s.row == kref.row && s.col == kref.col)
                                })
                                .map(|s| s.label.clone())
                                .unwrap_or_else(|| format!("({}, {})", kref.row, kref.col));

                            ui.label(format!("位置：({}, {})", kref.row, kref.col));
                            ui.label(format!("键帽：{label}"));
                            ui.label(format!(
                                "Layer：{}",
                                draft
                                    .profile(draft.active_profile)
                                    .and_then(|p| p.layers.iter().find(|l| l.index == kref.layer))
                                    .map(|l| l.name.clone())
                                    .unwrap_or_else(|| format!("层 {}", kref.layer))
                            ));

                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // 当前 binding（在 draft 中的快照）
                            ui.label(format!("当前绑定：{}", cur_binding.label()));

                            ui.add_space(8.0);
                            ui.label("动作类型：");
                            // 草稿动作（用户正在编辑中的版本，每帧持久）
                            let mut draft_action =
                                st.draft_action.clone().unwrap_or(KeyAction::None);
                            let mut kind_idx = action_kind_index(&draft_action);
                            let kinds =
                                ["未绑定", "普通键", "多媒体", "鼠标", "宏", "切层", "旋钮"];
                            let prev_kind = kind_idx;
                            egui::ComboBox::from_id_salt("action-kind")
                                .selected_text(kinds[kind_idx])
                                .show_ui(ui, |cb| {
                                    for (i, k) in kinds.iter().enumerate() {
                                        cb.selectable_value(&mut kind_idx, i, *k);
                                    }
                                });
                            // 类型切换时，把 draft_action 替换成该类型的默认值（保留
                            // 同类变体时由 edit_action_params 内部修改即可）
                            if kind_idx != prev_kind {
                                draft_action = default_for_kind(kind_idx);
                            }

                            // 各类型的参数（就地改 draft_action，写回 st）
                            edit_action_params(ui, kind_idx, &mut draft_action, st);
                            st.draft_action = Some(draft_action.clone());
                            // 把 capture 状态同步到 AppHandle，供 app.rs 全局快捷键判断
                            *handle.capture_keyboard.lock().unwrap() = st.capture_keyboard;

                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                if ui.button("保存").clicked() {
                                    // 写回 draft（draft_action 是当前用户在 Drawer
                                    // 里编辑出来的最终结果）
                                    let to_save =
                                        st.draft_action.clone().unwrap_or(KeyAction::None);
                                    let mut d = handle.keymap_draft.lock().unwrap();
                                    let active = d.active_profile;
                                    if let Some(p) = d.profile_mut(active) {
                                        if to_save.is_set() {
                                            p.bindings.insert(kref, to_save.clone());
                                        } else {
                                            p.bindings.remove(&kref);
                                        }
                                    }
                                    st.dirty = true;
                                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                                        crate::state::ToastKind::Success,
                                        format!("{} → {}（待下发）", label, to_save.label()),
                                    ));
                                }
                                if ui.button("清除").clicked() {
                                    let mut d = handle.keymap_draft.lock().unwrap();
                                    let active = d.active_profile;
                                    if let Some(p) = d.profile_mut(active) {
                                        p.bindings.remove(&kref);
                                    }
                                    st.draft_action = Some(KeyAction::None);
                                    st.dirty = true;
                                }
                                if ui.button("关闭").clicked() {
                                    let mut sel = handle.selected_key.lock().unwrap();
                                    *sel = None;
                                }
                            });

                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "改动不会立即生效，需点击下方\"应用\"统一下发。",
                                )
                                .weak()
                                .size(11.0),
                            );
                        }
                    }

                    // 静默使用 snapshot（避免 dead_code warning）
                    let _ = snapshot;
                });
        });
    });
}

fn action_kind_index(a: &KeyAction) -> usize {
    match a {
        KeyAction::None => 0,
        KeyAction::Keyboard(_) => 1,
        KeyAction::Media(_) => 2,
        KeyAction::Mouse(_) => 3,
        KeyAction::Macro(_) => 4,
        KeyAction::LayerSwitch(_) => 5,
        KeyAction::Encoder(_) => 6,
    }
}

/// egui::Key → HID Usage ID (Keyboard/Keypad Page 0x07)
fn egui_key_to_hid(k: egui::Key) -> Option<u16> {
    use egui::Key::*;
    Some(match k {
        A => 0x04,
        B => 0x05,
        C => 0x06,
        D => 0x07,
        E => 0x08,
        F => 0x09,
        G => 0x0A,
        H => 0x0B,
        I => 0x0C,
        J => 0x0D,
        K => 0x0E,
        L => 0x0F,
        M => 0x10,
        N => 0x11,
        O => 0x12,
        P => 0x13,
        Q => 0x14,
        R => 0x15,
        S => 0x16,
        T => 0x17,
        U => 0x18,
        V => 0x19,
        W => 0x1A,
        X => 0x1B,
        Y => 0x1C,
        Z => 0x1D,
        Num0 => 0x27,
        Num1 => 0x1E,
        Num2 => 0x1F,
        Num3 => 0x20,
        Num4 => 0x21,
        Num5 => 0x22,
        Num6 => 0x23,
        Num7 => 0x24,
        Num8 => 0x25,
        Num9 => 0x26,
        Enter => 0x28,
        Escape => 0x29,
        Backspace => 0x2A,
        Tab => 0x2B,
        Space => 0x2C,
        Minus => 0x2D,
        Equals => 0x2E,
        OpenBracket => 0x2F,
        CloseBracket => 0x30,
        Backslash => 0x31,
        Semicolon => 0x33,
        Quote => 0x34,
        Backtick => 0x35,
        Comma => 0x36,
        Period => 0x37,
        Slash => 0x38,
        F1 => 0x3A,
        F2 => 0x3B,
        F3 => 0x3C,
        F4 => 0x3D,
        F5 => 0x3E,
        F6 => 0x3F,
        F7 => 0x40,
        F8 => 0x41,
        F9 => 0x42,
        F10 => 0x43,
        F11 => 0x44,
        F12 => 0x45,
        ArrowDown => 0x51,
        ArrowLeft => 0x50,
        ArrowRight => 0x4F,
        ArrowUp => 0x52,
        _ => return None,
    })
}

/// 捕获键盘按键：返回下一个按下的键的 HID Usage ID。
/// `Esc` 视为取消，返回 None。
fn capture_keycode(ctx: &egui::Context) -> Option<Option<u16>> {
    ctx.input(|i| {
        for event in &i.events {
            if let egui::Event::Key {
                key, pressed: true, ..
            } = event
            {
                if *key == egui::Key::Escape {
                    return Some(None);
                }
                if let Some(c) = egui_key_to_hid(*key) {
                    return Some(Some(c));
                }
                // 未识别的键也忽略，让用户换
            }
        }
        None
    })
}

/// 提交 Profile 重命名。
/// 命名属于本地元数据，**直接同步进 snapshot**（不走 DiffPreviewBar），
/// 避免用户后续"放弃改动"时把命名回滚。等协议 `CMD_PROFILE_RENAME` 接入后再
/// 把这一行改成发到设备。
fn commit_rename(handle: &AppHandle, draft: &mut KeymapData, idx: u8, name: &str) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Some(p) = draft.profile_mut(idx) {
        p.name = trimmed.to_string();
    }
    let mut snap = handle.keymap.lock().unwrap();
    if let Some(p) = snap.profile_mut(idx) {
        p.name = trimmed.to_string();
    }
}

fn edit_action_params(
    ui: &mut egui::Ui,
    kind: usize,
    current: &mut KeyAction,
    st: &mut KeymapPanelState,
) {
    match kind {
        0 => {
            *current = KeyAction::None;
        }
        1 => {
            // 普通键：列出 HID Usage ID 常见子集（用名字展示）
            let mut code = match current {
                KeyAction::Keyboard(c) => *c,
                _ => 0x04,
            };
            let labels: &[(u16, &str)] = &[
                (0x04, "A"),
                (0x05, "B"),
                (0x06, "C"),
                (0x07, "D"),
                (0x08, "E"),
                (0x09, "F"),
                (0x0A, "G"),
                (0x0B, "H"),
                (0x0C, "I"),
                (0x0D, "J"),
                (0x0E, "K"),
                (0x0F, "L"),
                (0x10, "M"),
                (0x11, "N"),
                (0x12, "O"),
                (0x13, "P"),
                (0x14, "Q"),
                (0x15, "R"),
                (0x16, "S"),
                (0x17, "T"),
                (0x18, "U"),
                (0x19, "V"),
                (0x1A, "W"),
                (0x1B, "X"),
                (0x1C, "Y"),
                (0x1D, "Z"),
                (0x1E, "1"),
                (0x1F, "2"),
                (0x20, "3"),
                (0x21, "4"),
                (0x22, "5"),
                (0x23, "6"),
                (0x24, "7"),
                (0x25, "8"),
                (0x26, "9"),
                (0x27, "0"),
                (0x28, "Enter"),
                (0x29, "Escape"),
                (0x2A, "Backspace"),
                (0x2B, "Tab"),
                (0x2C, "Space"),
                (0x2D, "-"),
                (0x2E, "="),
                (0x2F, "["),
                (0x30, "]"),
                (0x31, "\\"),
                (0x33, ";"),
                (0x34, "'"),
                (0x35, "`"),
                (0x36, ","),
                (0x37, "."),
                (0x38, "/"),
                (0x39, "CapsLock"),
                (0x3A, "F1"),
                (0x3B, "F2"),
                (0x3C, "F3"),
                (0x3D, "F4"),
                (0x3E, "F5"),
                (0x3F, "F6"),
                (0x40, "F7"),
                (0x41, "F8"),
                (0x42, "F9"),
                (0x43, "F10"),
                (0x44, "F11"),
                (0x45, "F12"),
                (0x46, "PrintScreen"),
                (0x47, "ScrollLock"),
                (0x48, "Pause"),
                (0x49, "Insert"),
                (0x4A, "Home"),
                (0x4B, "PageUp"),
                (0x4C, "Delete"),
                (0x4D, "End"),
                (0x4E, "PageDown"),
                (0x4F, "→"),
                (0x50, "←"),
                (0x51, "↓"),
                (0x52, "↑"),
                (0xE0, "LCtrl"),
                (0xE1, "LShift"),
                (0xE2, "LAlt"),
                (0xE3, "LGUI"),
                (0xE4, "RCtrl"),
                (0xE5, "RShift"),
                (0xE6, "RAlt"),
                (0xE7, "RGUI"),
            ];
            let display = labels
                .iter()
                .find(|(c, _)| *c == code)
                .map(|(_, n)| format!("{n} (0x{code:02X})"))
                .unwrap_or_else(|| format!("0x{:02X}", code));
            egui::ComboBox::from_id_salt("kbd-key")
                .selected_text(display)
                .show_ui(ui, |cb| {
                    for (c, name) in labels.iter() {
                        cb.selectable_value(&mut code, *c, format!("{name} (0x{c:02X})"));
                    }
                });

            // 捕获模式：按下后下次键盘事件自动写入 code
            ui.add_space(4.0);
            ui.vertical(|ui| {
                if st.capture_keyboard {
                    // 激活态：明显的捕获提示条
                    let pulse = (0.5 + 0.5 * Instant::now().elapsed().as_secs_f32().sin()) as f32;
                    let border_color =
                        Color32::from_rgb(0xFF, 0xA0, 0x40).gamma_multiply(0.6 + pulse * 0.4);
                    let t = 0.15 + pulse * 0.10;
                    let a = Color32::from_rgb(0x40, 0x28, 0x10);
                    let b = Color32::from_rgb(0xFF, 0xA0, 0x40);
                    let bg_color = Color32::from_rgb(
                        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
                        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
                        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
                    );
                    egui::Frame::new()
                        .fill(bg_color)
                        .stroke(Stroke::new(2.0, border_color))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin {
                            left: 10,
                            right: 10,
                            top: 10,
                            bottom: 10,
                        })
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("●").size(16.0).color(border_color));
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("正在捕获按键…")
                                            .strong()
                                            .color(Color32::WHITE),
                                    );
                                    ui.label(egui::RichText::new("按 Esc 退出").weak().size(10.0));
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("取消").on_hover_text("退出捕获模式").clicked()
                                        {
                                            st.capture_keyboard = false;
                                        }
                                    },
                                );
                            });
                        });
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(60));
                } else {
                    // 非激活态：明显的可点击提示条
                    let resp = egui::Frame::new()
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(Stroke::new(
                            1.0,
                            ui.visuals().widgets.noninteractive.bg_stroke.color,
                        ))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin {
                            left: 10,
                            right: 10,
                            top: 8,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(crate::ui::icons::KEYMAP_CAPTURE)
                                        .font(crate::ui::fonts::icon_font_id(16.0))
                                        .color(crate::ui::ACCENT),
                                );
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("按任意键捕获").strong());
                                    ui.label(
                                        egui::RichText::new("把键盘按下的键映射到此按键")
                                            .weak()
                                            .size(10.0),
                                    );
                                });
                                // ui.with_layout(
                                //     egui::Layout::right_to_left(egui::Align::Center),
                                //     |ui| {
                                //         ui.label("点击开始 ▶");
                                //     },
                                // );
                            });
                        });
                    // 点击整块框进入捕获
                    let rect = resp.response.rect;
                    if ui
                        .interact(rect, ui.id().with("capture-bar"), Sense::click())
                        .clicked()
                    {
                        st.capture_keyboard = true;
                    }
                }

                if st.capture_keyboard {
                    if let Some(res) = capture_keycode(ui.ctx()) {
                        // res = Some(Some(code)) = 捕获成功
                        // res = Some(None)        = Esc 取消
                        // res = None              = 还没按下
                        if let Some(c) = res {
                            st.captured_code = Some(c);
                        }
                        st.capture_keyboard = false;
                    }
                }
                if let Some(c) = st.captured_code.take() {
                    code = c;
                }
            });
            *current = KeyAction::Keyboard(code);
        }
        2 => {
            let mut m = match current {
                KeyAction::Media(x) => *x,
                _ => MediaKey::PlayPause,
            };
            egui::ComboBox::from_id_salt("media-key")
                .selected_text(format!("{:?}", m))
                .show_ui(ui, |cb| {
                    cb.selectable_value(&mut m, MediaKey::PlayPause, "PlayPause");
                    cb.selectable_value(&mut m, MediaKey::Next, "Next");
                    cb.selectable_value(&mut m, MediaKey::Prev, "Prev");
                    cb.selectable_value(&mut m, MediaKey::VolUp, "VolUp");
                    cb.selectable_value(&mut m, MediaKey::VolDown, "VolDown");
                    cb.selectable_value(&mut m, MediaKey::Mute, "Mute");
                });
            *current = KeyAction::Media(m);
        }
        3 => {
            let mut m = match current {
                KeyAction::Mouse(x) => *x,
                _ => MouseAction::LeftClick,
            };
            egui::ComboBox::from_id_salt("mouse-act")
                .selected_text(format!("{:?}", m))
                .show_ui(ui, |cb| {
                    cb.selectable_value(&mut m, MouseAction::LeftClick, "LeftClick");
                    cb.selectable_value(&mut m, MouseAction::RightClick, "RightClick");
                    cb.selectable_value(&mut m, MouseAction::MiddleClick, "MiddleClick");
                    cb.selectable_value(&mut m, MouseAction::ScrollUp, "ScrollUp");
                    cb.selectable_value(&mut m, MouseAction::ScrollDown, "ScrollDown");
                });
            *current = KeyAction::Mouse(m);
        }
        4 => {
            ui.label(
                egui::RichText::new("宏编辑器（阶段 05 接入）")
                    .weak()
                    .size(11.0),
            );
            // 保留已有宏内容；空时给个空 Vec
            if !matches!(current, KeyAction::Macro(_)) {
                *current = KeyAction::Macro(vec![]);
            }
        }
        5 => {
            let mut l = match current {
                KeyAction::LayerSwitch(x) => *x,
                _ => 0,
            };
            egui::ComboBox::from_id_salt("layer-switch")
                .selected_text(format!("层 {l}"))
                .show_ui(ui, |cb| {
                    for i in 0..4 {
                        cb.selectable_value(&mut l, i, format!("层 {i}"));
                    }
                });
            *current = KeyAction::LayerSwitch(l);
        }
        6 => {
            let mut e = match current {
                KeyAction::Encoder(x) => *x,
                _ => EncoderAction::Cw,
            };
            egui::ComboBox::from_id_salt("encoder-act")
                .selected_text(format!("{:?}", e))
                .show_ui(ui, |cb| {
                    cb.selectable_value(&mut e, EncoderAction::Cw, "顺时针 (Cw)");
                    cb.selectable_value(&mut e, EncoderAction::Ccw, "逆时针 (Ccw)");
                    cb.selectable_value(&mut e, EncoderAction::Press, "按下 (Press)");
                });
            *current = KeyAction::Encoder(e);
        }
        _ => {}
    }
}

/// 切类型时给的默认值（避免从"鼠标"切到"多媒体"时残留旧的 MouseAction）
fn default_for_kind(kind: usize) -> KeyAction {
    match kind {
        0 => KeyAction::None,
        1 => KeyAction::Keyboard(0x04),
        2 => KeyAction::Media(MediaKey::PlayPause),
        3 => KeyAction::Mouse(MouseAction::LeftClick),
        4 => KeyAction::Macro(vec![]),
        5 => KeyAction::LayerSwitch(0),
        6 => KeyAction::Encoder(EncoderAction::Cw),
        _ => KeyAction::None,
    }
}

// -------- DiffPreviewBar --------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeymapDiffAction {
    None,
    Apply,
    Discard,
}

fn show_keymap_diff_bar(ui: &mut egui::Ui, diff: &[KeymapDiffEntry]) -> KeymapDiffAction {
    let mut action = KeymapDiffAction::None;
    let count = diff.len();
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, crate::ui::ACCENT.gamma_multiply(0.6)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(
                    egui::RichText::new(format!("待下发 {count} 项"))
                        .color(ui.visuals().warn_fg_color),
                );
                ui.separator();
                egui::ScrollArea::horizontal()
                    .max_width(420.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for e in diff {
                                let label = match e {
                                    KeymapDiffEntry::ActiveProfile(i) => {
                                        format!("profile→P{i}")
                                    }
                                    KeymapDiffEntry::Binding { key, from, to } => format!(
                                        "({},{}) {} → {}",
                                        key.row,
                                        key.col,
                                        from.label(),
                                        to.label()
                                    ),
                                };
                                ui.label(label);
                            }
                        });
                    });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("放弃 (Esc)").clicked() {
                        action = KeymapDiffAction::Discard;
                    }
                    let apply_btn = egui::Button::new("应用 (Ctrl+Enter)")
                        .fill(crate::ui::ACCENT)
                        .corner_radius(egui::CornerRadius::same(6));
                    if ui.add_enabled(count > 0, apply_btn).clicked() {
                        action = KeymapDiffAction::Apply;
                    }
                });
            });
        });
    action
}

#[allow(dead_code)]
const _: Vec2 = Vec2::new(UNIT_PX, UNIT_PX);
