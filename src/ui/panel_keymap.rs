//! P3 Keymap 页面：键盘图片背景 + 键位栅格 + 右侧功能分配 Drawer。
//!
//! 数据流：连接后 `AppHandle::refresh_keymap_from_device()`（0x05）把固件
//! 11 键映射写进快照 + 草稿；本页编辑草稿，DiffPreviewBar"应用"时通过
//! 0x06 CMD_KEYMAP_SET 整表下发（固件按 active profile 逐键覆盖）。
//!
//! 动作模型与固件 KeyResolver/KeyNameTable 真实解析能力对齐（详见
//! `protocol::KeyAction`）：未绑定 / 普通键 / 组合键 / 多键同按 /
//! 文本注入 / 固件功能串。固件只有一层物理映射（无 Layer），本页固定
//! 编辑 layer 0（Base），演示数据里的 Fn/Media/Custom 层不参与编辑。

use std::time::Instant;

use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::protocol::{
    HID_KEY_CHOICES, KeyAction, KeymapData, KeymapDiffEntry, LAYER_BASE, LAYER_FUN1, LAYER_FUN2,
    MOD_ALT, MOD_CTRL, MOD_GUI, MOD_SHIFT, SlotKind, hid_key_label,
};
use crate::state::AppHandle;

const ROW_COUNT: usize = 3;
const KEY_GAP: f32 = 4.0;
const ROW_GAP: f32 = 4.0;
/// 1.25u / 1.5u 等非整数宽度按键的圆角微调
const KEY_RADIUS: f32 = 5.0;
/// 左侧外壳最大宽度（含左右 14px 内边距），超过后不再随窗口放大
const MAX_LEFT_W: f32 = 460.0;
/// 抽屉最小宽度，低于时不再缩小（避免长键名被横向裁断）
const MIN_DRAWER_W: f32 = 360.0;
const MAX_DRAWER_W: f32 = 460.0;

/// 主区几何参数：左侧外壳（屏幕+键盘）与右侧抽屉各占多少。
struct KeymapGeometry {
    drawer_w: f32,
    left_w: f32,
    left_h: f32,
    screen_w: f32,
    screen_h: f32,
}

#[derive(Default)]
pub struct KeymapPanelState {
    /// Drawer 中正在编辑的动作草稿（每帧写回）
    pub draft_action: Option<KeyAction>,
    /// draft_action 所属的选中键与编辑通道；选中键/通道变化时用新键的
    /// 当前绑定重置草稿。通道：0 = 单击，1 = FUN1 组合层，2 = FUN2 组合层。
    pub selected_ref: Option<(crate::protocol::KeyRef, u8)>,
    /// Drawer 当前编辑通道（单击 / FUN1 / FUN2）
    pub edit_channel: u8,
    /// Drawer 普通键/组合键分支是否进入"按下捕获"模式
    pub capture_keyboard: bool,
    /// 当前是否处于 Profile 重命名模式
    pub renaming_profile: bool,
    /// 重命名模式下 TextEdit 的临时字符串
    pub profile_name_edit: String,
}

/// Keymap 页面：上 / 中 / 下 三段式布局。
///
/// - 上（`TopBottomPanel::top`）：页面标题 + 顶部控制条 + 图例，停靠固定。
/// - 中（`CentralPanel`）：键盘图 + 右侧 Drawer；外层保留 ScrollArea，
///   窗口高度不足时主区可滚动。
/// - 下（`TopBottomPanel::bottom`）：DiffPreviewBar 同步下发区，
///   固定在全局状态栏上方，宽度变化时随窗口收放。
pub fn show(ctx: &egui::Context, handle: &AppHandle, st: &mut KeymapPanelState) {
    // 调用顺序很关键：先注册 `keymap-diff` 底部 panel，再注册 `CentralPanel`。
    // 这样 CentralPanel 内部读取 `ui.available_height()` 时，已经能看到
    // 底部下发区所占用的视觉矩形，`band_h` = 中间区域实际可用高度。
    // 若顺序反过来，底部 panel 占用的高度不会被减去，Drawer 会向下溢出并
    // 被 `keymap-diff` 顶部 padding 遮挡。
    //
    // 下：DiffPreviewBar 同步下发区（停靠在状态栏上方）
    egui::TopBottomPanel::bottom("keymap-diff")
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            left: 4,
            right: 4,
            top: 4,
            bottom: 4,
        }))
        .show(ctx, |ui| {
            let snapshot = handle.keymap.lock().unwrap().clone();
            let draft = handle.keymap_draft.lock().unwrap().clone();
            let diff = draft.diff_bindings(&snapshot);
            let action = show_keymap_diff_bar(ui, &diff, &draft);
            handle_keymap_diff_action(handle, action, &diff, &draft, &snapshot);
        });

    // 上：标题 + 控制条 + 图例（停靠固定）
    egui::TopBottomPanel::top("keymap-header")
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            left: 4,
            right: 4,
            top: 4,
            bottom: 4,
        }))
        .show(ctx, |ui| {
            ui.heading("按键映射");
            ui.label(
                egui::RichText::new(
                    "为每个按键自定义触发行为：可设为普通键、组合键、文本片段或内置功能",
                )
                .weak()
                .size(11.0),
            );
            ui.add_space(6.0);

            // 顶部控制条：Profile 选择 + 操作按钮
            top_controls(handle, ui, st);
            ui.separator();
            ui.add_space(4.0);

            // 图例
            ui.horizontal(|ui| {
                legend_dot(
                    ui,
                    Color32::from_rgb(0x28, 0x2C, 0x36),
                    Color32::from_rgb(0x44, 0x4A, 0x55),
                    "未设置",
                );
                legend_dot(
                    ui,
                    Color32::from_rgb(0x2C, 0x46, 0x7A),
                    Color32::from_rgb(0x6A, 0x88, 0xC0),
                    "已同步",
                );
                legend_dot(
                    ui,
                    Color32::from_rgb(0xC0, 0x80, 0x20),
                    Color32::from_rgb(0xFF, 0xC8, 0x60),
                    "待同步",
                );
                legend_dot(
                    ui,
                    Color32::from_rgb(0x4F, 0x8C, 0xFF),
                    // 选中描边在浅色主题下改用近黑：WHITE 描边在白底上不可见。
                    crate::ui::colors::themed(
                        ui.visuals().dark_mode,
                        Color32::WHITE,
                        Color32::from_rgb(0x1A, 0x1D, 0x24),
                    ),
                    "已选中",
                );
                fun_tag(ui, 1);
                ui.label(egui::RichText::new("FUN 键 1").size(11.0));
                fun_tag(ui, 2);
                ui.label(egui::RichText::new("FUN 键 2").size(11.0));
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "键帽左上「F1」/ 右上「F2」= 配置了对应 FUN 层行为；右下三角 = 该键本身是 FUN 键；左下琥珀小点 = 有未同步的修改；右键键帽可快速分配 FUN 键",
                )
                .weak()
                .size(11.0),
            );
            ui.add_space(4.0);
        });

    // 中：键盘图 + Drawer；外层 ScrollArea 保留以便窗口太矮时滚动
    egui::CentralPanel::default().show(ctx, |ui| {
        let snapshot = handle.keymap.lock().unwrap().clone();
        let draft = handle.keymap_draft.lock().unwrap().clone();
        let _diff = draft.diff_bindings(&snapshot);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 水平两段：左 = 键盘外壳（按几何参数固定宽高），
                // 右 = Drawer（占满中间区域剩余高度，与键盘外壳解耦）。
                let geom = compute_keymap_geometry(ui.available_size(), &draft);
                let band_h = ui.available_height();

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    // ─── 左：屏幕占位 + 键盘图 ───
                    ui.allocate_ui(Vec2::new(geom.left_w, geom.left_h), |ui| {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(0x14, 0x17, 0x1E))
                            .stroke(Stroke::new(1.5, Color32::from_rgb(0x32, 0x38, 0x44)))
                            .corner_radius(egui::CornerRadius::same(14))
                            .inner_margin(egui::Margin {
                                left: 14,
                                right: 14,
                                top: 14,
                                bottom: 14,
                            })
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 10.0;
                                    draw_screen(ui, geom.screen_w, geom.screen_h);
                                    draw_keyboard(handle, ui, &draft, &snapshot, st.edit_channel);
                                    // FUN 分配条贴着键盘布局：分配动作与键帽
                                    // 视觉就近，减少与键名的对照成本。
                                    fun_assignment_bar(handle, ui);
                                });
                            });
                    });

                    // ─── 右：按键功能 Drawer ───
                    // 直接把 `band_h` 全部给 `drawer`：`drawer` 内部 `card`
                    // Frame 的 inner_margin(top+bottom=24) 会由 egui 自动从
                    // 分配高度中扣除，无需在外层手动再减一次。
                    //
                    // 底部固定 DiffPreviewBar 由独立 TopBottomPanel 占据，
                    // 并已在本函数开头先于 CentralPanel 注册，所以
                    // `band_h` 已经自动扣除了下发区高度。
                    ui.allocate_ui(Vec2::new(geom.drawer_w, band_h), |ui| {
                        drawer(ui, handle, st, &draft);
                    });
                });
            });
    });
}

/// DiffPreviewBar 的 Apply / Discard 副作用处理（从 show 抽出，便于底部
/// panel 调用，保持原 Apply / Discard 语义不变）。
fn handle_keymap_diff_action(
    handle: &AppHandle,
    action: KeymapDiffAction,
    diff: &[crate::protocol::KeymapDiffEntry],
    draft: &crate::protocol::KeymapData,
    snapshot: &crate::protocol::KeymapData,
) {
    match action {
        KeymapDiffAction::Apply => {
            // 0x06 SET 写入固件**当前激活 Profile** 的 keymap：设备激活档与
            // 草稿不一致时（典型：本页刚切了 Profile、0x10 推送尚未回来）
            // 先发 0x08 对齐，否则键映射会写错档。
            if draft.active_profile != snapshot.active_profile {
                switch_device_profile(handle, draft.active_profile);
            }
            // 0x06 CMD_KEYMAP_SET：把当前 profile 的 11 键整表下发（固件按
            // physical 逐键覆盖，属于"整包覆盖"语义；增量精确定位留待固件
            // 支持按 physical 分项后再说）。
            //
            // fun_key1/2 仅在本轮 diff 里包含 FUN 分配变更时才携带：旧固件 /
            // 0x05 未回传 fun 字段时，草稿值为 0，无条件下发会误清设备配置。
            use crate::protocol::{CMD_KEYMAP_SET, KeymapSetReq};
            use std::time::Duration;
            let fun_changed = diff
                .iter()
                .any(|e| matches!(e, KeymapDiffEntry::FunKeys { .. }));
            let req = KeymapSetReq {
                keymap: draft.to_firmware_entries(),
                fun_key1: fun_changed.then_some(draft.fun_key1),
                fun_key2: fun_changed.then_some(draft.fun_key2),
            };
            let data = serde_json::to_value(&req).ok();
            let mut success = false;
            let _ = handle.with_link(|lm| {
                match lm.request(CMD_KEYMAP_SET, data, Duration::from_millis(4000)) {
                    Ok(frame) => {
                        if frame.status() == Some(0) {
                            success = true;
                        } else {
                            let msg = frame
                                .error
                                .unwrap_or_else(|| "设备未接受新的按键映射".to_string());
                            let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                                crate::state::ToastKind::Error,
                                format!("下发失败：{msg}"),
                            ));
                        }
                    }
                    Err(e) => {
                        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                            crate::state::ToastKind::Error,
                            format!("下发超时：{e}"),
                        ));
                    }
                }
            });
            if success {
                // 固件 ACK 后才落本地快照，避免失败时 UI 状态与实际不符
                let mut snap = handle.keymap.lock().unwrap();
                snap.apply_diff(diff);
                handle.log_kind(
                    crate::state::LogKind::Tx,
                    format!("下发键映射 → {} 项变更", diff.len()),
                );
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Success,
                    format!("已同步 {} 项按键映射", diff.len()),
                ));
            } else {
                handle.log_kind(
                    crate::state::LogKind::App,
                    "按键映射未成功下发，已保留本地草稿供重试".to_string(),
                );
            }
        }
        KeymapDiffAction::Discard => {
            *handle.keymap_draft.lock().unwrap() = snapshot.clone();
        }
        KeymapDiffAction::None => {}
    }
}

/// 计算 Keymap 主区的几何尺寸：
/// - Drawer 固定 360 宽（不再写死 320，避免长键名被裁断）；左侧按可用空间 +
///   MAX_LEFT_W 双约束。
/// - 屏幕按 428:124 等比缩放至内容宽度。
/// - 键盘高度按 3 行键 + 行间距 + 上下 padding + 顶部文字位精确计算，
///   1u 键宽 = 高（正方形），底部不留空白。
fn compute_keymap_geometry(avail: Vec2, draft: &KeymapData) -> KeymapGeometry {
    // 抽屉最小 360 宽，左侧外壳宽度 = 可用 - 抽屉 - 间距，上限 MAX_LEFT_W。
    let drawer_w = (avail.x - 410.0).min(MAX_DRAWER_W);
    let left_w = ((avail.x - drawer_w - 24.0).max(MIN_DRAWER_W)).min(MAX_LEFT_W);
    // 外壳左右各 14px 内边距，内容宽度 = left_w - 28。
    let screen_w = left_w - 28.0;
    let screen_h = (screen_w * (124.0 / 428.0)).round();
    // 键盘外壳高度：3 行键 + 行间距 + 上下 padding + 顶部文字位。
    let key_padding = 10.0;
    let top_text_h = 14.0;
    let kb_inner_w = screen_w - key_padding * 2.0;
    let max_row_units: f32 = draft
        .profile(draft.active_profile)
        .or_else(|| draft.profiles.first())
        .and_then(|p| p.layers.iter().find(|l| l.index == 0))
        .map(|layer| {
            let mut mx = 0.0_f32;
            for r in 0..ROW_COUNT {
                let sum: f32 = layer
                    .slots
                    .iter()
                    .filter(|s| s.row as usize == r)
                    .map(|s| s.width_units)
                    .sum();
                if sum > mx {
                    mx = sum;
                }
            }
            mx
        })
        .unwrap_or(4.0);
    let u_px = if max_row_units > 0.0 {
        (kb_inner_w / max_row_units).max(8.0)
    } else {
        32.0
    };
    let keyboard_h = key_padding * 2.0 + top_text_h + ROW_COUNT as f32 * u_px;
    // FUN 分配条高度（Frame 上下 margin 5+5 + 一行控件约 22）+ 与键盘的间距。
    let fun_bar_h = 10.0 + 32.0;
    // 外壳总高 = 屏幕 + 间距 + 键盘区 + FUN 分配条 + 底部 padding；与原结构一致。
    let left_h = screen_h + 10.0 + keyboard_h + fun_bar_h + 28.0;
    KeymapGeometry {
        drawer_w,
        left_w,
        left_h,
        screen_w,
        screen_h,
    }
}

// -------- Profile 切换（0x08 同步设备） --------

/// 0x08 CMD_CONFIG_SET：仅下发 `active_keymap_profile` 字段，切换设备激活
/// Profile。设备随后推送 0x10 ProfileState → app.rs 触发 0x05 重拉键映射，
/// 快照与草稿随之对齐。离线时静默跳过（本地草稿切换仍然生效）。
fn switch_device_profile(handle: &AppHandle, profile: u8) {
    use crate::protocol::{DeviceSettings, F_ACTIVE_KEYMAP_PROFILE, FieldMask};
    let diff = DeviceSettings {
        active_keymap_profile: profile as i32,
        ..Default::default()
    };
    let mask = FieldMask::empty().set(F_ACTIVE_KEYMAP_PROFILE);
    crate::ui::widgets::apply_diff(handle, &diff, mask);
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
    ui.horizontal(|ui| {
        // Profile + 重命名：draft 写锁只在这一块持有，块结束即释放，
        // 避免与下方"重新加载"路径里的 draft 二次加锁死锁。
        {
            let mut draft = handle.keymap_draft.lock().unwrap();

            ui.label("配置：");
            let mut p = draft.active_profile as i32;
            // ComboBox 只显示用户命名，默认值"P{i}"在 make_demo_profile 中设置
            let current_name = draft
                .profile(p as u8)
                .map(|x| x.name.clone())
                .unwrap_or_else(|| format!("配置 {p}"));
            egui::ComboBox::from_id_salt("keymap-profile")
                .selected_text(current_name.clone())
                .show_ui(ui, |cb| {
                    for prof in draft.profiles.iter() {
                        cb.selectable_value(&mut p, prof.index as i32, prof.name.clone());
                    }
                });
            if p as u8 != draft.active_profile {
                draft.active_profile = p as u8;
                // 切 profile 时清空选中键与编辑草稿（避免上一个 profile 的
                // KeyRef / 草稿动作误导当前）。Profile 切换本身进入
                // DiffPreviewBar，由"应用"按钮统一发 0x08 + 0x06 下发。
                *handle.selected_key.lock().unwrap() = None;
                st.draft_action = None;
                st.selected_ref = None;
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
                    "放弃重命名"
                } else {
                    "为当前配置改名"
                })
                .clicked()
            {
                st.renaming_profile = !st.renaming_profile;
                if st.renaming_profile {
                    st.profile_name_edit = current_name.clone();
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
                    .on_hover_text("保存新名称")
                    .clicked()
                    || enter
                {
                    commit_rename(handle, &mut *draft, p as u8, &st.profile_name_edit);
                    st.renaming_profile = false;
                }
            }
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
                // 刷新快照 + 草稿（统一走 state 层入口）。
                match handle.refresh_keymap_from_device() {
                    Ok(n) => {
                        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                            crate::state::ToastKind::Success,
                            format!("已从设备同步 {n} 个按键的映射"),
                        ));
                    }
                    Err(e) => {
                        let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                            crate::state::ToastKind::Error,
                            format!("重新加载失败：{e}"),
                        ));
                    }
                }
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
                let s = serde_json::to_string_pretty(&*handle.keymap_draft.lock().unwrap());
                if let Ok(s) = s {
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

// -------- FUN 组合键分配 --------

/// 分配 / 取消 FUN 键（`phys` = 0 表示取消）。写入草稿顶层，进入
/// DiffPreviewBar，由「同步到设备」统一随 0x06 下发（fun_key 字段仅在
/// FUN 分配变更时携带）。同侧冲突自动清掉另一侧的分配。
fn set_fun_key(draft: &mut KeymapData, side: u8, phys: u8) {
    // 组合层依赖对应 FUN 键才可触发：取消分配时清空该层全部孤儿绑定，
    // 避免重新分配后残留的旧行为突然生效。
    let unassign = |draft: &mut KeymapData, layer: u8| {
        for p in &mut draft.profiles {
            p.bindings.retain(|k, _| k.layer != layer);
        }
    };
    if phys == 0 {
        let layer = if side == 1 { LAYER_FUN1 } else { LAYER_FUN2 };
        unassign(draft, layer);
    }
    let mut conflict_side = 0u8;
    match side {
        1 => {
            draft.fun_key1 = phys;
            if phys != 0 && draft.fun_key2 == phys {
                draft.fun_key2 = 0;
                conflict_side = 2;
            }
        }
        2 => {
            draft.fun_key2 = phys;
            if phys != 0 && draft.fun_key1 == phys {
                draft.fun_key1 = 0;
                conflict_side = 1;
            }
        }
        _ => {}
    }
    if conflict_side != 0 {
        let layer = if conflict_side == 1 {
            LAYER_FUN1
        } else {
            LAYER_FUN2
        };
        unassign(draft, layer);
    }
    // 键成为 FUN 键后按住不再产生输出，其自身的组合层绑定一并清除。
    if phys != 0 {
        if let Some((r, c)) = draft.fun_key_slot(phys) {
            for p in &mut draft.profiles {
                for layer in [LAYER_FUN1, LAYER_FUN2] {
                    p.bindings.remove(&crate::protocol::KeyRef {
                        layer,
                        row: r,
                        col: c,
                    });
                }
            }
        }
    }
}

/// 键帽的 FUN 键相关信息（渲染时随槽位传入 draw_key）。
#[derive(Clone, Copy)]
struct FunKeyInfo {
    /// 该键被分配为 FUN 键 1/2 时的角标文案
    badge: Option<&'static str>,
    /// FUN 键 1 已分配 → FUN1 组合层可触发
    layer1_active: bool,
    /// FUN 键 2 已分配 → FUN2 组合层可触发
    layer2_active: bool,
}

/// FUN1 / FUN2 各自的配色：(亮色底, 强调色, 底上的深色文字)。
/// FUN1 = 琥珀，FUN2 = 青。色牌 / 三角用亮底 + 深字，小字号也可读；
/// 深色界面上的彩色文字（Drawer 通道名等）用强调色。
fn fun_colors(side: u8) -> (Color32, Color32, Color32) {
    if side == 1 {
        (
            Color32::from_rgb(0xFF, 0xB0, 0x3A),
            Color32::from_rgb(0xFF, 0xC9, 0x7A),
            Color32::from_rgb(0x2B, 0x1A, 0x02),
        )
    } else {
        (
            Color32::from_rgb(0x3E, 0xCB, 0xEE),
            Color32::from_rgb(0x8A, 0xE2, 0xF6),
            Color32::from_rgb(0x03, 0x26, 0x30),
        )
    }
}

/// 彩色文字用的 FUN 强调色（随主题）：深色 UI 用亮色强调（fun_colors.1），
/// 浅色 UI（白底卡片）用加深变体，否则亮琥珀 / 亮青文字在白底上不可读。
fn fun_text_accent(dark: bool, side: u8) -> Color32 {
    match (side, dark) {
        (1, true) => fun_colors(1).1,
        (2, true) => fun_colors(2).1,
        (1, false) => Color32::from_rgb(0x9A, 0x62, 0x00),
        _ => Color32::from_rgb(0x04, 0x6C, 0x7E),
    }
}

/// FUN 角标（分配条 / Drawer / 图例共用样式）：FUN1 琥珀、FUN2 青，亮底深字。
///
/// 高度取 `interact_size.y`。注意 egui 的 horizontal 布局会把
/// `allocate_exact_size` 元素在整块剩余高度内垂直居中，而 ComboBox 按钮
/// 是「贴顶」分配，两者混排时必须把行高固定为下拉按钮实际高度
/// （见 fun_assignment_bar 中 combo_h），否则标签与下拉框不在同一中线。
/// 文字按 galley `mesh_bounds` 光学居中（同 fonts.rs 约定）："FUN1" 这类
/// 无下延字形的墨迹高于 galley 几何中心，直接 CENTER_CENTER 会显得偏上。
fn fun_tag(ui: &mut egui::Ui, side: u8) {
    let (bg, stroke_c, text_c) = fun_colors(side);
    let h = ui.spacing().interact_size.y.max(18.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(42.0, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 4.0, bg);
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, stroke_c), StrokeKind::Inside);

    let text = if side == 1 { "FUN1" } else { "FUN2" };
    let galley = p.layout(
        text.to_owned(),
        egui::FontId::proportional(12.0),
        text_c,
        f32::INFINITY,
    );
    let center = rect.center();
    let ink_cy = crate::ui::fonts::galley_mesh_center_y(&galley);
    p.galley(
        egui::pos2(center.x - galley.rect.width() / 2.0, center.y - ink_cy),
        galley,
        text_c,
    );
}

/// 键帽 FUN 色牌的角落位置。角落同时承担语义：
/// 左上 = FUN1 层行为，右上 = FUN2 层行为；右下的 FUN 键本体
/// 标记改用三角形（draw_fun_corner），与层行为色牌形状区分。
#[derive(Clone, Copy)]
enum FunChipCorner {
    TopLeft,
    TopRight,
}

/// 键帽角落的 FUN 实色小牌（比色条醒目得多）。`side` 决定配色（1 琥珀 / 2 青）。
/// 亮底 + 深字，保证小字号下的可读性。
fn draw_fun_chip(painter: &egui::Painter, key: Rect, corner: FunChipCorner, side: u8, label: &str) {
    let (bg, _, text_c) = fun_colors(side);
    let h = 13.0_f32.min(key.height() - 2.0).max(6.0);
    let w = 19.0_f32.min(key.width() - 2.0).max(h);
    let pos = match corner {
        FunChipCorner::TopLeft => egui::pos2(key.left() + 1.5, key.top() + 1.5),
        FunChipCorner::TopRight => egui::pos2(key.right() - w - 1.5, key.top() + 1.5),
    };
    let r = Rect::from_min_size(pos, Vec2::new(w, h));
    painter.rect_filled(r, 3.0, bg);
    painter.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(10.0),
        text_c,
    );
}

/// 键帽右下角的 FUN 键本体标记：三角形包裹住键帽角（与上层角的方形色牌
/// 形成形状区分）。三角直角边贴住键帽右、下边缘，直角处用与键帽一致的
/// `KEY_RADIUS` 圆角过渡，亮色填充 + 深色角标。
fn draw_fun_corner(painter: &egui::Painter, key: Rect, side: u8, label: &str) {
    let (bg, _, text_c) = fun_colors(side);
    let s = 20.0_f32
        .min(key.height() - 2.0)
        .min(key.width() - 2.0)
        .max(12.0);
    let (r, b) = (key.right(), key.bottom());
    // 直角顶点换成半径 KEY_RADIUS 的四分之一圆弧，贴合键帽圆角
    let rad = KEY_RADIUS.min(s * 0.5);
    let (cx, cy) = (r - rad, b - rad);
    let arc_steps = 6;
    let mut pts = vec![egui::pos2(r - s, b)];
    for i in 0..=arc_steps {
        let t = std::f32::consts::FRAC_PI_2 * (1.0 - i as f32 / arc_steps as f32);
        pts.push(egui::pos2(cx + rad * t.cos(), cy + rad * t.sin()));
    }
    pts.push(egui::pos2(r, b - s));
    painter.add(egui::Shape::convex_polygon(pts, bg, Stroke::NONE));
    // 文字沿对角线向直角顶点内收，保证墨迹落在三角形内
    painter.text(
        egui::pos2(r - s * 0.28, b - s * 0.28),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(7.5),
        text_c,
    );
}

/// 键盘下方的 FUN 分配条：琥珀角标 + 紧凑下拉，直接贴着键盘布局，
/// 替代原先塞在顶部控制条里的两行式下拉。提示文案收进 hover tooltip。
fn fun_assignment_bar(handle: &AppHandle, ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(Color32::from_rgb(0x1C, 0x20, 0x2A))
        .stroke(Stroke::new(1.0, Color32::from_rgb(0x2C, 0x32, 0x3E)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 5,
            bottom: 5,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // fun_key1/2 写进草稿顶层；锁只在本闭包内持有。
            let mut draft = handle.keymap_draft.lock().unwrap();
            let phys = draft.physical_key_slots();
            let key_label = |n: u8| -> String {
                match n {
                    0 => "未分配".to_string(),
                    n => phys
                        .iter()
                        .find(|(p, _, _, _)| *p == n)
                        .map(|(_, _, _, l)| l.clone())
                        .unwrap_or_else(|| format!("键 {n}")),
                }
            };

            ui.label(
                egui::RichText::new("FUN 组合键")
                    .strong()
                    .size(12.0)
                    .color(Color32::from_rgb(0xC8, 0xCE, 0xD8)),
            )
            .on_hover_text(
                "按住 FUN 键再按其它键，触发该键的 FUN 层行为。\n\
                 右键键盘上的键帽可快速分配 / 取消 FUN 键。",
            );
            ui.add_space(4.0);

            // 两组 FUN 分配并排在一行：[FUN1 下拉] | [FUN2 下拉]。
            // 注意：ComboBox 按钮在 horizontal 里是「贴顶」分配（内部
            // button_frame 基于 available_rect_before_wrap 自顶向下布局），
            // 而 fun_tag 这类 allocate_exact_size 元素按剩余空间整体居中，
            // 两种锚定基准不同，直接混排会导致标签与下拉框不在同一水平线。
            // 这里给整行固定高度 = 下拉按钮实际高度（Button 字体行高与
            // icon_width 取大者，加 button_padding，且不低于 interact_size），
            // 使两种分配方式的中心重合，FUN1/FUN2 两组必然共处一条中线。
            let combo_h = {
                let pad = ui.spacing().button_padding;
                let font = ui.style().text_styles[&egui::TextStyle::Button].clone();
                let galley = ui
                    .painter()
                    .layout_no_wrap("Ag".to_owned(), font, Color32::WHITE);
                (galley.size().y.max(ui.spacing().icon_width) + 2.0 * pad.y)
                    .max(ui.spacing().interact_size.y)
            };
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), combo_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (side, id_salt) in [(1u8, "fun-key1"), (2u8, "fun-key2")] {
                        fun_tag(ui, side);
                        let value = if side == 1 {
                            draft.fun_key1
                        } else {
                            draft.fun_key2
                        };
                        let other = if side == 1 {
                            draft.fun_key2
                        } else {
                            draft.fun_key1
                        };
                        egui::ComboBox::from_id_salt(id_salt)
                            .selected_text(key_label(value))
                            .width(96.0)
                            .show_ui(ui, |cb| {
                                fun_combo_options(cb, &mut draft, side, &phys, other)
                            });
                        if side == 1 {
                            ui.separator();
                        }
                    }
                },
            );

            // 右对齐提示行：显式给固定行高（interact_size.y）。若直接
            // with_layout(RTL, Center)，子 Ui 的 max_rect 是「本行到容器底」
            // 的剩余高度，文字会被垂直居中到剩余空间中部而非本行。
            let hint_h = ui.spacing().interact_size.y;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), hint_h),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.label(egui::RichText::new("右键键帽可快速分配").weak().size(10.0));
                },
            );
        });
}

/// FUN 分配下拉的选项列表：未分配 + 11 个物理键；另一侧已占用的键禁用。
/// 点击即写入草稿（自动处理冲突清理）。
fn fun_combo_options(
    cb: &mut egui::Ui,
    draft: &mut KeymapData,
    side: u8,
    phys: &[(u8, u8, u8, String)],
    other: u8,
) {
    let cur = if side == 1 {
        draft.fun_key1
    } else {
        draft.fun_key2
    };
    if cb
        .add(egui::Button::selectable(cur == 0, "未分配"))
        .clicked()
    {
        set_fun_key(draft, side, 0);
    }
    for (p, _, _, l) in phys.iter() {
        let disabled = *p == other;
        if cb
            .add_enabled(!disabled, egui::Button::selectable(cur == *p, l.clone()))
            .clicked()
        {
            set_fun_key(draft, side, *p);
        }
    }
}

// -------- 键盘图 + 键位 --------

/// 设备屏幕占位：428 × 124 等比缩放，仅外形展示，不做功能。
fn draw_screen(ui: &mut egui::Ui, width: f32, height: f32) {
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);

    // 屏幕外壳（深色边框 + 玻璃质感渐变）。注意：rect_stroke 在 Middle
    // 模式下描边会向两侧各延 0.5px，可能被父容器裁掉；这里把 bezel 整体
    // 向内缩 1px，并把描边改成 Inside，保证右/下边线完整可见。
    let bezel = rect.shrink(1.0);
    let screen = bezel.shrink(2.0);
    painter.rect_filled(bezel, 8.0, Color32::from_rgb(0x10, 0x12, 0x18));
    painter.rect_stroke(
        bezel,
        8.0,
        Stroke::new(1.0, Color32::from_rgb(0x3A, 0x40, 0x4C)),
        StrokeKind::Inside,
    );
    painter.rect_filled(screen, 6.0, Color32::from_rgb(0x0A, 0x12, 0x1E));

    // 屏幕内左上角小指示 + 右下角比例标签
    painter.text(
        screen.left_top() + Vec2::new(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        "屏幕布局占位",
        egui::FontId::proportional(11.0),
        Color32::from_rgb(0x70, 0x88, 0xA8),
    );
    painter.text(
        screen.right_bottom() + Vec2::new(-8.0, -6.0),
        egui::Align2::RIGHT_BOTTOM,
        "428 × 124",
        egui::FontId::proportional(10.0),
        Color32::from_rgb(0x55, 0x60, 0x78),
    );
    // 中心提示文字
    painter.text(
        screen.center(),
        egui::Align2::CENTER_CENTER,
        "（待接入屏幕布局）",
        egui::FontId::proportional(12.0),
        Color32::from_gray(110),
    );
}

fn draw_keyboard(
    handle: &AppHandle,
    ui: &mut egui::Ui,
    draft: &KeymapData,
    snapshot: &KeymapData,
    edit_channel: u8,
) {
    let (rect, _resp) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);

    // 背景：占位深色块 + "键盘背景图" 文字
    painter.rect_filled(rect, 8.0, Color32::from_rgb(0x18, 0x1B, 0x22));
    painter.text(
        rect.left_top() + Vec2::new(10.0, 6.0),
        egui::Align2::LEFT_TOP,
        "键盘外观图（待接入图片）",
        egui::FontId::proportional(11.0),
        Color32::from_rgb(0x70, 0x70, 0x80),
    );

    // 固件只有一层物理映射，本页固定编辑 layer 0（Base）。
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
            "暂无可用的按键配置",
            egui::FontId::proportional(14.0),
            Color32::from_gray(140),
        );
        return;
    };
    let Some(layer) = profile.layers.iter().find(|l| l.index == 0) else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "当前配置缺少基础按键层",
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
    let top_text_h = 14.0; // 顶部"键盘外观图（待接入图片）"占位文字高度
    let inner_w = rect.width() - padding * 2.0;

    // 取所有行中 units 总和的最大值作为 u_px 的宽度基准；
    // 这样 1u 按键去掉 KEY_GAP/ROW_GAP 之后宽 = 高 = 正方形。
    let max_units: f32 = rows
        .iter()
        .map(|r| r.iter().map(|s| s.width_units).sum::<f32>())
        .fold(0.0_f32, f32::max);
    let u_px = if max_units > 0.0 {
        (inner_w / max_units).max(8.0)
    } else {
        0.0
    };

    let selected = handle.selected_key.lock().unwrap().clone();
    // FUN 组合键标记位置：(row, col) → 物理键画布上的角标文字
    let fun1_slot = draft.fun_key_slot(draft.fun_key1);
    let fun2_slot = draft.fun_key_slot(draft.fun_key2);
    // (row, col) → physical 编号：右键菜单就地分配 FUN 键时需要
    let phys_map: std::collections::HashMap<(u8, u8), u8> = draft
        .physical_key_slots()
        .into_iter()
        .map(|(p, r, c, _)| ((r, c), p))
        .collect();

    // 有效编辑通道：与 Drawer 的回落逻辑一致 —— FUN 层未分配对应 FUN 键、
    // 或选中键本身是 FUN 键时，该通道不可编辑，选中边框回落到单击层配色。
    let is_fun_key_sel = selected.as_ref().map(|k| {
        phys_map
            .get(&(k.row, k.col))
            .copied()
            .map(|n| {
                (draft.fun_key1 != 0 && n == draft.fun_key1)
                    || (draft.fun_key2 != 0 && n == draft.fun_key2)
            })
            .unwrap_or(false)
    });
    let edit_channel = match (edit_channel.min(2), is_fun_key_sel) {
        (LAYER_FUN1, Some(true)) | (LAYER_FUN2, Some(true)) => LAYER_BASE,
        (LAYER_FUN1, _) if draft.fun_key1 == 0 => LAYER_BASE,
        (LAYER_FUN2, _) if draft.fun_key2 == 0 => LAYER_BASE,
        (ch, _) => ch,
    };

    for (r_idx, row) in rows.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let key_h = (u_px - ROW_GAP).max(8.0);

        let y = rect.top() + padding + top_text_h + r_idx as f32 * u_px;
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
                    edit_channel,
                );
            } else {
                let fun = FunKeyInfo {
                    badge: if fun1_slot == Some((slot.row, slot.col)) {
                        Some("FUN1")
                    } else if fun2_slot == Some((slot.row, slot.col)) {
                        Some("FUN2")
                    } else {
                        None
                    },
                    layer1_active: fun1_slot.is_some(),
                    layer2_active: fun2_slot.is_some(),
                };
                let slot_phys = phys_map.get(&(slot.row, slot.col)).copied();
                draw_key(
                    ui,
                    &painter,
                    r,
                    slot,
                    &profile.bindings,
                    snap_bindings,
                    &selected,
                    handle,
                    fun,
                    slot_phys,
                    edit_channel,
                );
            }
            x += slot.width_units * u_px;
        }
    }
}

/// 键帽主文字：绑定了动作 → 显示动作标签；未绑定 → 显示物理键帽名。
fn keycap_text(slot: &crate::protocol::KeySlot, binding: Option<&KeyAction>) -> String {
    match binding {
        Some(b) if b.is_set() => b.label(),
        _ => slot.label.clone(),
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
    // FUN 键相关信息：本体角标 + 组合层是否激活（对应 FUN 键已分配）
    fun: FunKeyInfo,
    // 该键的 physical 编号（不在 11 键内时为 None），用于右键 FUN 分配
    slot_phys: Option<u8>,
    // Drawer 当前编辑通道（0 = 单击，1 = FUN1，2 = FUN2）：决定选中边框配色
    edit_channel: u8,
) {
    let key_ref = crate::protocol::KeyRef {
        layer: 0,
        row: slot.row,
        col: slot.col,
    };
    let binding = bindings.get(&key_ref);
    // draft 与 snapshot 是否一致（含 FUN 组合层 1/2）：决定键格是"已应用"
    // 还是"待下发"。
    let is_pending = (LAYER_BASE..=LAYER_FUN2).any(|l| {
        let k = crate::protocol::KeyRef {
            layer: l,
            ..key_ref
        };
        bindings.get(&k) != snap_bindings.and_then(|m| m.get(&k))
    });

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
        // 已同步（draft 与 snapshot 一致且非空）
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

    // 选中态：外圈叠加一层高亮框（不覆盖基础色）。单击层用白色；
    // FUN1 / FUN2 通道用对应层色（琥珀 / 青，深底上取强调色），
    // 与键帽角标 F1 / F2 的层色一致，提示当前正在编辑哪一层。
    if is_sel {
        let sel_color = match edit_channel {
            LAYER_FUN1 => fun_colors(1).1,
            LAYER_FUN2 => fun_colors(2).1,
            _ => Color32::WHITE,
        };
        painter.rect_stroke(
            rect.shrink(0.5),
            KEY_RADIUS,
            Stroke::new(2.0, sel_color),
            StrokeKind::Middle,
        );
    }

    // 待下发标记：左下角琥珀点（右上角让位给 FUN2 层色牌）
    if is_pending {
        let dot_r = 3.5;
        let cx = rect.left() + 6.0;
        let cy = rect.bottom() - 6.0;
        painter.circle_filled(
            egui::pos2(cx, cy),
            dot_r,
            Color32::from_rgb(0xFF, 0xC0, 0x40),
        );
    }

    // 标签：绑定动作优先显示；已绑定时物理键帽名缩小放到左上角
    // （左上角被 FUN1 层色牌占用时右移让位）。
    let action_label = keycap_text(slot, binding);
    let bound = binding.map(|b| b.is_set()).unwrap_or(false);
    let base_font = if rect.width() > 60.0 { 11.0 } else { 9.5 };
    let font_size = if action_label.chars().count() > 6 {
        base_font - 1.5
    } else {
        base_font
    };
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &action_label,
        egui::FontId::proportional(font_size),
        Color32::from_rgb(0xE0, 0xE5, 0xF0),
    );

    // FUN1 / FUN2 组合层是否配置了行为 → 键帽上角的实色小牌：
    // 左上「F1」琥珀 = FUN1 层，右上「F2」青色 = FUN2 层，一眼可区分。
    // 仅当对应 FUN 键已分配（组合层真实可触发）且本键不是 FUN 键时才显示，
    // 避免孤儿绑定（曾配置过、后取消分配）造成误导。
    let has_fun_layer = |layer: u8| {
        bindings
            .get(&crate::protocol::KeyRef { layer, ..key_ref })
            .map(|b| b.is_set())
            .unwrap_or(false)
    };
    let is_fun_key = fun.badge.is_some();
    let has_fun1 = fun.layer1_active && !is_fun_key && has_fun_layer(LAYER_FUN1);
    let has_fun2 = fun.layer2_active && !is_fun_key && has_fun_layer(LAYER_FUN2);
    if has_fun1 {
        draw_fun_chip(painter, rect, FunChipCorner::TopLeft, 1, "F1");
    }
    if has_fun2 {
        draw_fun_chip(painter, rect, FunChipCorner::TopRight, 2, "F2");
    }

    if bound {
        let x_off = if has_fun1 { 23.0 } else { 3.0 };
        painter.text(
            rect.left_top() + Vec2::new(x_off, 2.0),
            egui::Align2::LEFT_TOP,
            &slot.label,
            egui::FontId::proportional(10.5),
            Color32::from_rgb(0x90, 0x98, 0xA8),
        );
    }

    // FUN 键本体角标：右下角三角形包裹键帽角（与左上/右上的方形层色牌
    // 形状区分），FUN1 琥珀 / FUN2 青，亮底深字。
    if let Some(badge) = fun.badge {
        let side = if badge == "FUN1" { 1 } else { 2 };
        let label = if side == 1 { "F1" } else { "F2" };
        draw_fun_corner(painter, rect, side, label);
    }

    // 鼠标点击 / hover
    let id = ui.id().with(("key", slot.row, slot.col));
    let resp = ui.interact(rect, id, Sense::click());
    let hovered = resp.hovered();
    let clicked = resp.clicked();
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        let status = if is_pending { "待下发" } else { "已应用" };
        // FUN 组合层行为摘要（层激活且本键不是 FUN 键时才有意义）
        let mut combo_info = String::new();
        for (l, name, active) in [
            (LAYER_FUN1, "FUN1", fun.layer1_active),
            (LAYER_FUN2, "FUN2", fun.layer2_active),
        ] {
            if active && !is_fun_key {
                if let Some(b) = bindings.get(&crate::protocol::KeyRef {
                    layer: l,
                    ..key_ref
                }) {
                    if b.is_set() {
                        combo_info.push_str(&format!(" · {name}:{}", b.label()));
                    }
                }
            }
        }
        let fun_note = if fun.badge.is_some() {
            " · 本键为 FUN 键（按住不产生输出）"
        } else if slot_phys.is_some() {
            " · 右键可设为 FUN 键"
        } else {
            ""
        };
        resp.clone().on_hover_text(format!(
            "「{label}」第 {row} 行 第 {col} 列 · {status} · 当前行为：{behavior}{combo_info}{fun_note}",
            label = slot.label,
            row = slot.row,
            col = slot.col,
            status = status,
            behavior = binding
                .map(|b| b.label())
                .unwrap_or_else(|| "未设置".into()),
        ));
    }
    // 右键快捷分配 FUN 键：就地操作，免于在下拉列表里对照键名。
    if let Some(n) = slot_phys {
        resp.context_menu(|ui| {
            let (f1, f2) = {
                let d = handle.keymap_draft.lock().unwrap();
                (d.fun_key1, d.fun_key2)
            };
            if f1 == n || f2 == n {
                let side = if f1 == n { 1 } else { 2 };
                ui.label(
                    egui::RichText::new(format!("当前为 FUN 键 {side}"))
                        .weak()
                        .size(11.0),
                );
                if ui.button("取消 FUN 分配").clicked() {
                    set_fun_key(&mut handle.keymap_draft.lock().unwrap(), side, 0);
                    ui.close();
                }
            } else {
                ui.set_min_width(130.0);
                if ui.button("设为 FUN 键 1").clicked() {
                    set_fun_key(&mut handle.keymap_draft.lock().unwrap(), 1, n);
                    ui.close();
                }
                if ui.button("设为 FUN 键 2").clicked() {
                    set_fun_key(&mut handle.keymap_draft.lock().unwrap(), 2, n);
                    ui.close();
                }
            }
        });
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
    // Drawer 当前编辑通道（0 = 单击，1 = FUN1，2 = FUN2）：决定选中圆环配色
    edit_channel: u8,
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
    // 选中态：外圈额外加一圈高亮（半径略外移以让描边可见）。
    // 配色随编辑通道：单击层白色，FUN1 / FUN2 用对应层色强调色。
    if is_sel {
        let sel_color = match edit_channel {
            LAYER_FUN1 => fun_colors(1).1,
            LAYER_FUN2 => fun_colors(2).1,
            _ => Color32::WHITE,
        };
        painter.circle_stroke(r.center(), radius + 2.5, Stroke::new(2.0, sel_color));
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

    // 中心文字：绑定动作标签优先，未绑定显示物理标签
    let action_label = keycap_text(slot, binding);
    let font_size = if action_label.chars().count() > 4 {
        9.0
    } else {
        13.0
    };
    painter.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        &action_label,
        egui::FontId::proportional(font_size),
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

    // 鼠标交互（点击即选中）
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
            "「{label}」旋钮 第 {row} 行 第 {col} 列 · {status} · 当前行为：{behavior}",
            label = slot.label,
            row = slot.row,
            col = slot.col,
            status = status,
            behavior = binding
                .map(|b| b.label())
                .unwrap_or_else(|| "未设置".into())
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

fn drawer(ui: &mut egui::Ui, handle: &AppHandle, st: &mut KeymapPanelState, draft: &KeymapData) {
    let selected = handle.selected_key.lock().unwrap().clone();
    crate::ui::card(ui, |ui| {
        // Drawer 占满调用方分配的高度（中间区域剩余高度）；
        // 内部 ScrollArea 在内容超出时独立滚动，与左侧键盘外壳完全解耦。
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.strong("按键功能");
                    ui.add_space(4.0);
                    match selected {
                        None => {
                            // 切走选中时清掉草稿动作
                            st.draft_action = None;
                            st.selected_ref = None;
                            ui.label(
                                egui::RichText::new("点击键盘图上的任意按键，开始设置触发行为")
                                    .weak()
                                    .size(12.0),
                            );
                        }
                        Some(kref) => {
                            // 当前编辑通道（0 = 单击，1 = FUN1 层，2 = FUN2 层）；
                            // 编辑目标 KeyRef 与选中键 kref 仅 layer 字段不同。
                            // FUN 层未分配对应 FUN 键时该通道不可编辑；且 FUN 键
                            // 自身按住不产生输出，它的任何 FUN 层行为都无意义，
                            // 两个 FUN 通道一并禁用。两种情况都自动回落到单击层
                            // （FUN 分配入口在键盘下方 FUN 分配条 / 右键）。
                            let phys_of_selected = draft
                                .physical_key_slots()
                                .into_iter()
                                .find(|(_, r, c, _)| *r == kref.row && *c == kref.col)
                                .map(|(p, _, _, _)| p);
                            let is_fun_key = match phys_of_selected {
                                Some(n) => {
                                    (draft.fun_key1 != 0 && n == draft.fun_key1)
                                        || (draft.fun_key2 != 0 && n == draft.fun_key2)
                                }
                                None => false,
                            };
                            let mut channel = st.edit_channel.min(2);
                            if (channel == LAYER_FUN1 && (draft.fun_key1 == 0 || is_fun_key))
                                || (channel == LAYER_FUN2 && (draft.fun_key2 == 0 || is_fun_key))
                            {
                                channel = LAYER_BASE;
                                st.edit_channel = LAYER_BASE;
                            }
                            let edit_ref = crate::protocol::KeyRef {
                                layer: channel,
                                ..kref
                            };
                            // 当前 draft 中该键该通道的绑定（持久化的最新状态）
                            let cur_binding = draft
                                .profile(draft.active_profile)
                                .and_then(|p| p.bindings.get(&edit_ref))
                                .cloned()
                                .unwrap_or(KeyAction::None);

                            // 选中键/通道变化时（含首次选中 / 换键 / 切通道），用
                            // 该键当前绑定重置草稿动作；同键同通道编辑期间草稿
                            // 保持用户输入。
                            if st.selected_ref.as_ref() != Some(&(kref, channel)) {
                                st.selected_ref = Some((kref, channel));
                                st.draft_action = Some(cur_binding.clone());
                            }
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

                            ui.label(format!("位置：第 {} 行 第 {} 列", kref.row, kref.col));
                            ui.label(format!("按键标识：{label}"));

                            ui.add_space(6.0);
                            // FUN 角色快捷操作：就地分配 / 取消，与键盘下方
                            // FUN 分配条、键帽右键菜单共用 set_fun_key 语义。
                            if let Some(n) = phys_of_selected {
                                ui.horizontal_wrapped(|ui| {
                                    if draft.fun_key1 == n || draft.fun_key2 == n {
                                        let side = if draft.fun_key1 == n { 1 } else { 2 };
                                        fun_tag(ui, side);
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "本键是 FUN 键 {side}（按住不产生输出）"
                                            ))
                                            .size(11.0),
                                        );
                                        if ui.small_button("取消分配").clicked() {
                                            set_fun_key(
                                                &mut handle.keymap_draft.lock().unwrap(),
                                                side,
                                                0,
                                            );
                                        }
                                    } else {
                                        ui.label(
                                            egui::RichText::new("FUN 角色：").weak().size(11.0),
                                        );
                                        if ui
                                            .small_button("设为 FUN1")
                                            .on_hover_text("按住该键时触发其它键的 FUN1 层行为")
                                            .clicked()
                                        {
                                            set_fun_key(
                                                &mut handle.keymap_draft.lock().unwrap(),
                                                1,
                                                n,
                                            );
                                        }
                                        if ui
                                            .small_button("设为 FUN2")
                                            .on_hover_text("按住该键时触发其它键的 FUN2 层行为")
                                            .clicked()
                                        {
                                            set_fun_key(
                                                &mut handle.keymap_draft.lock().unwrap(),
                                                2,
                                                n,
                                            );
                                        }
                                    }
                                });
                            }

                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // 编辑通道选择：单击 / FUN1 组合层 / FUN2 组合层。
                            // 对应 FUN 键未分配、或选中键自身就是该 FUN 键时，
                            // 禁用该通道（tooltip 说明原因）。
                            ui.label("编辑通道：");
                            ui.horizontal(|ui| {
                                for (ch, name) in [
                                    (LAYER_BASE, "单击"),
                                    (LAYER_FUN1, "FUN1"),
                                    (LAYER_FUN2, "FUN2"),
                                ] {
                                    // FUN 通道文字沿用各自强调色（FUN1 琥珀 / FUN2 青），
                                    // 浅色主题用加深变体保证白底可读。
                                    let dark = ui.visuals().dark_mode;
                                    let text = match ch {
                                        LAYER_FUN1 => {
                                            egui::RichText::new(name)
                                                .size(12.0)
                                                .color(fun_text_accent(dark, 1))
                                        }
                                        LAYER_FUN2 => {
                                            egui::RichText::new(name)
                                                .size(12.0)
                                                .color(fun_text_accent(dark, 2))
                                        }
                                        _ => egui::RichText::new(name).size(12.0),
                                    };
                                    let enabled = match ch {
                                        LAYER_FUN1 => draft.fun_key1 != 0 && !is_fun_key,
                                        LAYER_FUN2 => draft.fun_key2 != 0 && !is_fun_key,
                                        _ => true,
                                    };
                                    let resp = ui
                                        .add_enabled(
                                            enabled,
                                            egui::Button::selectable(channel == ch, text),
                                        )
                                        .on_hover_text(match ch {
                                            LAYER_FUN1 => "按住 FUN 键 1 时按下该键触发的行为",
                                            LAYER_FUN2 => "按住 FUN 键 2 时按下该键触发的行为",
                                            _ => "直接按下该键触发的行为",
                                        });
                                    if !enabled {
                                        let reason = if is_fun_key {
                                            "本键是 FUN 键（按住不产生输出），不能配置 FUN 层行为"
                                                .to_string()
                                        } else {
                                            format!(
                                                "请先分配 FUN 键 {}（键盘下方 FUN 分配条或右键键帽）",
                                                if ch == LAYER_FUN1 { 1 } else { 2 }
                                            )
                                        };
                                        resp.clone().on_disabled_hover_text(reason);
                                    }
                                    if resp.clicked() {
                                        st.edit_channel = ch;
                                    }
                                }
                            });

                            ui.add_space(6.0);
                            // 当前 binding（在 draft 中的快照）
                            ui.label(format!("当前行为：{}", cur_binding.label()));

                            ui.add_space(8.0);
                            ui.label("触发行为：");
                            // 草稿动作（用户正在编辑中的版本，每帧持久）
                            let mut draft_action =
                                st.draft_action.clone().unwrap_or(KeyAction::None);
                            let mut kind_idx = action_kind_index(&draft_action);
                            let kinds = [
                                "未绑定",
                                "普通键",
                                "组合键",
                                "多键同按",
                                "文本注入",
                                "固件功能",
                            ];
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
                                draft_action = default_for_kind(kind_idx, &draft_action);
                            }

                            // 各类型的参数（就地改 draft_action，写回 st）
                            edit_action_params(ui, kind_idx, &mut draft_action, st);
                            st.draft_action = Some(draft_action.clone());
                            // 把 capture 状态同步到 AppHandle，供 app.rs 全局快捷键判断
                            *handle.capture_keyboard.lock().unwrap() = st.capture_keyboard;

                            ui.add_space(10.0);
                            let channel_name = match channel {
                                LAYER_FUN1 => "FUN1",
                                LAYER_FUN2 => "FUN2",
                                _ => "单击",
                            };
                            ui.horizontal(|ui| {
                                if ui.button("保存").clicked() {
                                    // 写回 draft（draft_action 是当前用户在 Drawer
                                    // 里编辑出来的最终结果）；按编辑通道写入
                                    // layer 0（单击）/ 1（FUN1）/ 2（FUN2）。
                                    let to_save =
                                        st.draft_action.clone().unwrap_or(KeyAction::None);
                                    let mut d = handle.keymap_draft.lock().unwrap();
                                    let active = d.active_profile;
                                    if let Some(p) = d.profile_mut(active) {
                                        if to_save.is_set() {
                                            p.bindings.insert(edit_ref, to_save.clone());
                                        } else {
                                            p.bindings.remove(&edit_ref);
                                        }
                                    }
                                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                                        crate::state::ToastKind::Success,
                                        format!(
                                            "已为「{label}」{chan}通道设为 {}（待同步）",
                                            to_save.label(),
                                            chan = if channel == LAYER_BASE {
                                                String::new()
                                            } else {
                                                format!("{channel_name} ")
                                            }
                                        ),
                                    ));
                                }
                                if ui.button("清除").clicked() {
                                    let mut d = handle.keymap_draft.lock().unwrap();
                                    let active = d.active_profile;
                                    if let Some(p) = d.profile_mut(active) {
                                        p.bindings.remove(&edit_ref);
                                    }
                                    st.draft_action = Some(KeyAction::None);
                                }
                                if ui.button("关闭").clicked() {
                                    let mut sel = handle.selected_key.lock().unwrap();
                                    *sel = None;
                                }
                            });

                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("改动需点击下方「应用」按钮才会同步到设备。")
                                    .weak()
                                    .size(11.0),
                            );
                        }
                    }
                });
        });
    });
}

fn action_kind_index(a: &KeyAction) -> usize {
    match a {
        KeyAction::None => 0,
        KeyAction::Keyboard(_) => 1,
        KeyAction::Combo { .. } => 2,
        KeyAction::Chord(_) => 3,
        KeyAction::Text(_) => 4,
        KeyAction::Function(_) => 5,
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

/// 捕获键盘按键：返回 `(修饰键掩码, HID Usage ID)`。
/// `Esc` 视为取消（返回 `Some(None)`）；修饰键本身的按下不产生 code（被忽略）。
fn capture_combo(ctx: &egui::Context) -> Option<Option<(u8, u16)>> {
    ctx.input(|i| {
        for event in &i.events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                if *key == egui::Key::Escape {
                    return Some(None);
                }
                if let Some(c) = egui_key_to_hid(*key) {
                    let mut mods = 0u8;
                    if modifiers.ctrl {
                        mods |= MOD_CTRL;
                    }
                    if modifiers.shift {
                        mods |= MOD_SHIFT;
                    }
                    if modifiers.alt {
                        mods |= MOD_ALT;
                    }
                    if modifiers.mac_cmd {
                        mods |= MOD_GUI;
                    }
                    return Some(Some((mods, c)));
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

/// "按任意键捕获" 提示条（激活 / 非激活两态），点击切换进入捕获模式。
fn capture_bar_ui(ui: &mut egui::Ui, st: &mut KeymapPanelState) {
    if st.capture_keyboard {
        // 激活态：明显的捕获提示条（配色随主题：深色 = 深琥珀底 + 白字，
        // 浅色 = 浅琥珀底 + 深琥珀字，保证 weak 提示在底色上可读）。
        let pulse = (0.5 + 0.5 * Instant::now().elapsed().as_secs_f32().sin()) as f32;
        let dark = ui.visuals().dark_mode;
        let border_color = Color32::from_rgb(0xFF, 0xA0, 0x40).gamma_multiply(0.6 + pulse * 0.4);
        let bg_color = if dark {
            let t = 0.15 + pulse * 0.10;
            let a = Color32::from_rgb(0x40, 0x28, 0x10);
            let b = Color32::from_rgb(0xFF, 0xA0, 0x40);
            crate::ui::colors::mix(a, b, t)
        } else {
            let t = 0.25 + pulse * 0.15;
            crate::ui::colors::mix(Color32::WHITE, Color32::from_rgb(0xFF, 0xC8, 0x80), t)
        };
        let (title_color, hint_color) = if dark {
            (Color32::WHITE, Color32::from_rgb(0xD8, 0xC8, 0xA8))
        } else {
            (
                Color32::from_rgb(0x5A, 0x3A, 0x05),
                Color32::from_rgb(0x8A, 0x6A, 0x38),
            )
        };
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
                            egui::RichText::new("正在记录按键…")
                                .strong()
                                .color(title_color),
                        );
                        // 显式配色：weak() 取主题弱文字色，在异色底上会失配
                        ui.label(
                            egui::RichText::new("按 Esc 退出")
                                .size(10.0)
                                .color(hint_color),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("取消").on_hover_text("退出按键捕获").clicked() {
                            st.capture_keyboard = false;
                        }
                    });
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
                        ui.label(egui::RichText::new("记录一次按键").strong());
                        ui.label(
                            egui::RichText::new("把键盘按下的键作为此按键的触发行为")
                                .weak()
                                .size(10.0),
                        );
                    });
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
}

/// HID 主键选择下拉（覆盖 App 编辑器提供的全部键位）。
fn hid_key_combo(ui: &mut egui::Ui, id_salt: &str, code: &mut u16) {
    let display = hid_key_label(*code)
        .map(|n| format!("{n} (0x{code:02X})"))
        .unwrap_or_else(|| format!("0x{code:02X}"));
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(display)
        .show_ui(ui, |cb| {
            for (c, name) in HID_KEY_CHOICES.iter() {
                cb.selectable_value(code, *c, format!("{name} (0x{c:02X})"));
            }
        });
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
            // 普通键：单键（含修饰键单独成键）
            let mut code = match current {
                KeyAction::Keyboard(c) => *c,
                KeyAction::Combo { code, .. } => *code,
                _ => 0x04,
            };
            hid_key_combo(ui, "kbd-key", &mut code);

            // 捕获模式：捕获到的 (mods, code) 只取 code（普通键不带修饰）
            ui.add_space(4.0);
            capture_bar_ui(ui, st);
            if st.capture_keyboard {
                if let Some(res) = capture_combo(ui.ctx()) {
                    st.capture_keyboard = false;
                    if let Some((_, c)) = res {
                        code = c;
                    }
                }
            }
            *current = KeyAction::Keyboard(code);
        }
        2 => {
            // 组合键：修饰键掩码 + 主键
            let (mut mods, mut code) = match current {
                KeyAction::Combo { mods, code } => (*mods, *code),
                KeyAction::Keyboard(c) => (0u8, *c),
                _ => (0u8, 0x04),
            };
            // Drawer 固定 280px，"Ctrl/Shift/Alt/Win" 4 个 toggle + 标签 + 主键 ComboBox
            // 横向放不下，用 horizontal_wrapped 让修饰键行自然换行。
            ui.horizontal_wrapped(|ui| {
                ui.label("修饰键：");
                for (bit, name) in [
                    (MOD_CTRL, "Ctrl"),
                    (MOD_SHIFT, "Shift"),
                    (MOD_ALT, "Alt"),
                    (MOD_GUI, "Win"),
                ] {
                    let mut on = mods & bit != 0;
                    if ui.toggle_value(&mut on, name).changed() {
                        mods = if on { mods | bit } else { mods & !bit };
                    }
                }
            });
            ui.vertical(|ui| {
                ui.label("主键：");
                // ComboBox 在 vertical 子层里直接拿父容器全宽，长键名（如
                // "Arrow Left (0x50)"）不会再被横向裁断。
                hid_key_combo(ui, "combo-key", &mut code);
            });

            // 捕获模式：修饰键取按下时的实际组合（如按住 Ctrl 再按 S → Ctrl+S）
            ui.add_space(4.0);
            capture_bar_ui(ui, st);
            if st.capture_keyboard {
                if let Some(res) = capture_combo(ui.ctx()) {
                    st.capture_keyboard = false;
                    if let Some((m, c)) = res {
                        mods = m;
                        code = c;
                    }
                }
            }
            *current = KeyAction::Combo { mods, code };
        }
        3 => {
            // 多键同按（固件 normal 通道 "a+b" 语义）；至少保留 1 键
            let mut codes = match current {
                KeyAction::Chord(c) if !c.is_empty() => c.clone(),
                KeyAction::Chord(_) => vec![0x04],
                _ => vec![0x04],
            };
            ui.label(egui::RichText::new("同时按下以下按键：").weak().size(11.0));
            let mut i = 0;
            while i < codes.len() {
                let name = hid_key_label(codes[i])
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("0x{:02X}", codes[i]));
                let row = ui.horizontal(|ui| {
                    ui.label(name);
                    let removable = codes.len() > 1;
                    ui.add_enabled(removable, egui::Button::new("×"))
                        .on_disabled_hover_text("至少需要保留一个按键")
                        .on_hover_text("移除该键")
                        .clicked()
                });
                if row.inner {
                    codes.remove(i);
                } else {
                    i += 1;
                }
            }
            let mut add: u16 = 0;
            egui::ComboBox::from_id_salt("chord-add")
                .selected_text("＋ 添加按键…")
                .show_ui(ui, |cb| {
                    for (c, name) in HID_KEY_CHOICES.iter() {
                        cb.selectable_value(&mut add, *c, format!("{name} (0x{c:02X})"));
                    }
                });
            if add != 0 {
                if !codes.contains(&add) {
                    codes.push(add);
                }
            }
            *current = KeyAction::Chord(codes);
        }
        4 => {
            // 文本注入：ASCII ≤128，按键触发整串输出一次
            let mut text = match current {
                KeyAction::Text(t) => t.clone(),
                _ => String::new(),
            };
            let resp = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .hint_text("按键触发时输出的文本（仅限英文与符号）")
                    .desired_width(ui.available_width()),
            );
            let non_ascii = text.chars().any(|c| c as u32 > 0x7F);
            let over = text.chars().count() > 128;
            ui.label(
                egui::RichText::new(format!("{} / 128 字符", text.chars().count()))
                    .weak()
                    .size(10.0)
                    .color(if over {
                        ui.visuals().error_fg_color
                    } else {
                        ui.visuals().weak_text_color()
                    }),
            );
            if non_ascii {
                ui.label(
                    egui::RichText::new("⚠ 含中文或特殊符号，设备端可能无法输出")
                        .size(10.0)
                        .color(ui.visuals().warn_fg_color),
                );
            }
            if !resp.has_focus() && text.is_empty() {
                ui.label(
                    egui::RichText::new("例如：常用邮箱、口令、命令行片段")
                        .weak()
                        .size(10.0),
                );
            }
            *current = KeyAction::Text(text);
        }
        5 => {
            // 固件功能串：KEY_FUNCTION_ASR 有内置行为；其它串原样透传
            let mut f = match current {
                KeyAction::Function(s) => s.clone(),
                _ => String::new(),
            };
            ui.label("内置功能：");
            if ui
                .selectable_label(f == "KEY_FUNCTION_ASR", "语音识别（ASR）")
                .clicked()
            {
                f = "KEY_FUNCTION_ASR".to_string();
            }
            ui.add_space(4.0);
            ui.label("自定义功能串：");
            ui.add(
                egui::TextEdit::singleline(&mut f)
                    .hint_text("如：语音识别 / Ctrl+c …")
                    .desired_width(ui.available_width()),
            );
            f = f.trim().to_string();
            ui.label(
                egui::RichText::new(
                    "设备支持：语音识别、组合键串（如 Ctrl+c）、单独修饰键；\
                     其它未识别的串按键无效果，但会原样保留不下丢。",
                )
                .weak()
                .size(10.0),
            );
            *current = KeyAction::Function(f);
        }
        _ => {}
    }
}

/// 切类型时给的默认值（尽量保留可迁移的数据：Keyboard → Combo 保留主键等）
fn default_for_kind(kind: usize, prev: &KeyAction) -> KeyAction {
    match kind {
        0 => KeyAction::None,
        1 => match prev {
            KeyAction::Combo { code, .. } => KeyAction::Keyboard(*code),
            _ => KeyAction::Keyboard(0x04),
        },
        2 => match prev {
            KeyAction::Keyboard(c) => KeyAction::Combo { mods: 0, code: *c },
            _ => KeyAction::Combo {
                mods: 0,
                code: 0x04,
            },
        },
        3 => match prev {
            KeyAction::Keyboard(c) => KeyAction::Chord(vec![*c]),
            KeyAction::Combo { code, .. } => KeyAction::Chord(vec![*code]),
            _ => KeyAction::Chord(vec![0x04]),
        },
        4 => KeyAction::Text(String::new()),
        5 => KeyAction::Function(String::new()),
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

fn show_keymap_diff_bar(
    ui: &mut egui::Ui,
    diff: &[KeymapDiffEntry],
    draft: &KeymapData,
) -> KeymapDiffAction {
    // FUN 键编号 → 槽位标签（用于 FunKeys 变更的展示）
    let fun_name = |n: u8| -> String {
        match n {
            0 => "关闭".to_string(),
            n => draft
                .fun_key_slot(n)
                .and_then(|(r, c)| {
                    draft
                        .profile(draft.active_profile)
                        .and_then(|p| p.layers.iter().find(|l| l.index == 0))
                        .and_then(|l| l.slots.iter().find(|s| s.row == r && s.col == c))
                        .map(|s| s.label.clone())
                })
                .unwrap_or_else(|| format!("键 {n}")),
        }
    };
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
            // 两行布局：标题 + 按钮固定在第一行（右对齐、不被内容挤压），
            // 明细放在第二行自动换行。避免明细过长时把"同步到设备"按钮
            // 挤出可视区，导致点击时误点落到按钮位置上的明细文本，也避免
            // 撑出父面板的横向滚动条。
            ui.horizontal(|ui| {
                ui.strong(
                    egui::RichText::new(format!("{count} 项待同步到设备"))
                        .color(ui.visuals().warn_fg_color),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("放弃修改（Esc）").clicked() {
                        action = KeymapDiffAction::Discard;
                    }
                    let apply_btn = egui::Button::new(
                        egui::RichText::new("同步到设备（Ctrl+Enter）").color(egui::Color32::WHITE),
                    )
                    .fill(crate::ui::ACCENT)
                    .corner_radius(egui::CornerRadius::same(6));
                    if ui.add_enabled(count > 0, apply_btn).clicked() {
                        action = KeymapDiffAction::Apply;
                    }
                });
            });
            if count > 0 {
                ui.add_space(4.0);
                // 横向自动换行展示，不用 ScrollArea：内容永远不会超出面板宽度。
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for e in diff {
                        let label = match e {
                            KeymapDiffEntry::ActiveProfile(i) => {
                                format!("切换到「配置 {i}」")
                            }
                            KeymapDiffEntry::FunKeys { f1, f2 } => format!(
                                "FUN 键 1 → {}，FUN 键 2 → {}",
                                fun_name(*f1),
                                fun_name(*f2)
                            ),
                            KeymapDiffEntry::Binding { key, from, to } => {
                                let layer_tag = match key.layer {
                                    LAYER_FUN1 => "FUN1 组合：",
                                    LAYER_FUN2 => "FUN2 组合：",
                                    _ => "",
                                };
                                format!(
                                    "{layer_tag}第 {} 行 第 {} 列：{} → {}",
                                    key.row,
                                    key.col,
                                    from.label(),
                                    to.label()
                                )
                            }
                        };
                        ui.label(label);
                    }
                });
            }
        });
    action
}
