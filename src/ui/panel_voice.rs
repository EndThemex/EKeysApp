//! P6 Voice 页面：腾讯云一句话识别配置。
//!
//! 字段：voice_enable / voice_trigger_key / voice_max_record_ms / voice_auto_enter /
//!       voice_cuid / voice_tencent_secret_id / voice_tencent_secret_key
//! 阶段 06 生效；阶段 08 由百度短语音迁移为腾讯云一句话识别（SentenceRecognition）

use eframe::egui;

use crate::state::AppHandle;
use crate::ui::widgets::settings_panel_scaffold;

#[derive(Default)]
pub struct VoicePanelState;

/// 生成"首尾可见、中间 ***"的预览字符串，避免肩窥又能让用户感知到"这里有值"。
///
/// - 空串：原样返回空串（区分"未配置"和"已配置"）。
/// - 短串（≤ 8 字符）：全 `*` 防止头尾即可推断长度。
/// - 普通长度：保留首 4 + 末 4，中间 `***`。
/// - 长串（> 40 字符）：保留首 4 + 末 4，中间 `…`，提示被截断。
fn preview_mask(s: &str) -> String {
    let len = s.chars().count();
    if len == 0 {
        return String::new();
    }
    if len <= 8 {
        return "*".repeat(len);
    }
    let prefix: String = s.chars().take(4).collect();
    let suffix: String = s.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    let mid = if len > 40 { "…" } else { "***" };
    format!("{prefix}{mid}{suffix}")
}

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, _st: &mut VoicePanelState) {
    ui.heading("语音识别");
    ui.label("腾讯云一句话识别（SentenceRecognition，16k_zh）");
    ui.add_space(4.0);

    settings_panel_scaffold(handle, ui, |ui, snapshot, draft| {
        ui.group(|ui| {
            ui.label("启用语音");
            let mut on = if draft.voice_enable != 0 || snapshot.voice_enable != 0 {
                draft.voice_enable != 0
            } else {
                snapshot.voice_enable != 0
            };
            if ui.checkbox(&mut on, "启用").changed() {
                draft.voice_enable = if on { 1 } else { 0 };
            }

            ui.add_space(6.0);
            ui.label("触发键 ID");
            // 触发键 / 录音时长直接读 draft：未编辑时 draft 经 merge_push
            // 始终跟随 snapshot，不能取 max（否则低于快照值的修改会被
            // 立刻回显成旧值，只能调大不能调小）。
            let mut trig = draft.voice_trigger_key;
            if ui.add(egui::DragValue::new(&mut trig).speed(1)).changed() {
                draft.voice_trigger_key = trig;
            }

            ui.add_space(6.0);
            ui.label("最长录音时长（毫秒）");
            let mut ms = draft.voice_max_record_ms;
            if ui
                .add(egui::DragValue::new(&mut ms).range(1000..=60000).speed(100))
                .changed()
            {
                draft.voice_max_record_ms = ms;
            }

            ui.add_space(6.0);
            ui.label("自动进入识别");
            let mut ae = if draft.voice_auto_enter != 0 || snapshot.voice_auto_enter != 0 {
                draft.voice_auto_enter != 0
            } else {
                snapshot.voice_auto_enter != 0
            };
            if ui.checkbox(&mut ae, "按下触发键后自动进入识别").changed() {
                draft.voice_auto_enter = if ae { 1 } else { 0 };
            }
        });

        ui.group(|ui| {
            ui.label("腾讯云 API 配置");

            ui.add_space(4.0);
            ui.label("SecretId（≤64 字节）");
            // SecretId 与 SecretKey 一样，在设备回读时被 mask_sensitive
            // 统一替换为 "***"（协议 §7，App 不存储密钥明文）。
            // 编辑判断必须用"草稿 != 旧快照"而不是 is_empty()：
            // 这样"清空"也是一种可见的合法编辑态；且快照值是掩码后的
            // "***"，is_empty() 无法据此区分用户是否编辑过。
            //
            // 展示策略：用户未编辑时（草稿 == 快照）显示首尾可见的掩码预览，
            // 防止肩窥并提示"这里有值"；用户编辑后则原样显示草稿明文。
            // 渲染用 `displayed`，但写回 draft 时仍用真实值（real）。
            let id_real = if draft.voice_tencent_secret_id != snapshot.voice_tencent_secret_id {
                draft.voice_tencent_secret_id.clone()
            } else {
                snapshot.voice_tencent_secret_id.clone()
            };
            let id_draft_modified = draft.voice_tencent_secret_id != snapshot.voice_tencent_secret_id;
            let mut id_displayed = if id_draft_modified {
                id_real.clone()
            } else {
                preview_mask(&id_real)
            };
            if ui
                .add(egui::TextEdit::singleline(&mut id_displayed).desired_width(280.0))
                .changed()
            {
                // 用户在预览态改了字符：判定为开始编辑，覆盖草稿为显示值。
                // 草稿 == 快照时把"未编辑的预览"也当作清空态，避免无意义 diff。
                if id_draft_modified || id_displayed != preview_mask(&id_real) {
                    draft.voice_tencent_secret_id = id_displayed;
                }
            }

            ui.add_space(6.0);
            ui.label("SecretKey（≤64 字节）");
            let mut sk = if draft.voice_tencent_secret_key.is_empty() {
                snapshot.voice_tencent_secret_key.clone()
            } else {
                draft.voice_tencent_secret_key.clone()
            };
            if ui
                .add(
                    egui::TextEdit::singleline(&mut sk)
                        .password(true)
                        .desired_width(280.0),
                )
                .changed()
            {
                draft.voice_tencent_secret_key = sk;
            }

            ui.add_space(6.0);
            ui.label("CUID（≤32 字节，腾讯协议不使用，保留）");
            let cuid_real = if draft.voice_cuid != snapshot.voice_cuid {
                draft.voice_cuid.clone()
            } else {
                snapshot.voice_cuid.clone()
            };
            let cuid_draft_modified = draft.voice_cuid != snapshot.voice_cuid;
            let mut cuid_displayed = if cuid_draft_modified {
                cuid_real.clone()
            } else {
                preview_mask(&cuid_real)
            };
            if ui
                .add(egui::TextEdit::singleline(&mut cuid_displayed).desired_width(280.0))
                .changed()
            {
                if cuid_draft_modified || cuid_displayed != preview_mask(&cuid_real) {
                    draft.voice_cuid = cuid_displayed;
                }
            }
        });
    });
}
