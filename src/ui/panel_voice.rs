//! P6 Voice 页面：百度语音识别配置。
//!
//! 字段：voice_enable / voice_trigger_key / voice_max_record_ms / voice_auto_enter /
//!       voice_dev_pid / voice_cuid / voice_baidu_api_key / voice_baidu_secret_key
//! 协议阶段 06 生效

use eframe::egui;

use crate::state::AppHandle;
use crate::ui::widgets::settings_panel_scaffold;

#[derive(Default)]
pub struct VoicePanelState;

pub fn show(handle: &AppHandle, ui: &mut egui::Ui, _st: &mut VoicePanelState) {
    ui.heading("语音识别");
    ui.label("阶段 06 生效：百度语音识别配置");
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
            let mut trig = draft.voice_trigger_key.max(snapshot.voice_trigger_key);
            if ui.add(egui::DragValue::new(&mut trig).speed(1)).changed() {
                draft.voice_trigger_key = trig;
            }

            ui.add_space(6.0);
            ui.label("最长录音时长（毫秒）");
            let mut ms = draft.voice_max_record_ms.max(snapshot.voice_max_record_ms);
            if ui
                .add(egui::DragValue::new(&mut ms).range(500..=30000).speed(100))
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
            ui.label("百度 API 配置");

            ui.add_space(4.0);
            ui.label("识别模型 ID");
            let mut pid = draft.voice_dev_pid.max(snapshot.voice_dev_pid);
            if ui.add(egui::DragValue::new(&mut pid).speed(1)).changed() {
                draft.voice_dev_pid = pid;
            }
            ui.label("(常用: 1537=中文, 1737=英文, 1637=日语, 1837=韩语)");

            ui.add_space(6.0);
            ui.label("CUID（≤32 字节）");
            let mut cuid = if draft.voice_cuid.is_empty() {
                snapshot.voice_cuid.clone()
            } else {
                draft.voice_cuid.clone()
            };
            if ui
                .add(egui::TextEdit::singleline(&mut cuid).desired_width(280.0))
                .changed()
            {
                draft.voice_cuid = cuid;
            }

            ui.add_space(6.0);
            ui.label("API Key（≤64 字节）");
            let mut ak = if draft.voice_baidu_api_key.is_empty() {
                snapshot.voice_baidu_api_key.clone()
            } else {
                draft.voice_baidu_api_key.clone()
            };
            if ui
                .add(
                    egui::TextEdit::singleline(&mut ak)
                        .password(true)
                        .desired_width(280.0),
                )
                .changed()
            {
                draft.voice_baidu_api_key = ak;
            }

            ui.add_space(6.0);
            ui.label("安全密钥（≤64 字节）");
            let mut sk = if draft.voice_baidu_secret_key.is_empty() {
                snapshot.voice_baidu_secret_key.clone()
            } else {
                draft.voice_baidu_secret_key.clone()
            };
            if ui
                .add(
                    egui::TextEdit::singleline(&mut sk)
                        .password(true)
                        .desired_width(280.0),
                )
                .changed()
            {
                draft.voice_baidu_secret_key = sk;
            }
        });
    });
}
