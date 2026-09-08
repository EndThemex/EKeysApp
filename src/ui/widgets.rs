//! 共享 UI 小部件：Toast / ConfirmDialog / FieldEditor / DiffPreviewBar

use std::time::{Duration, Instant};

use eframe::egui;

use crate::protocol::{DeviceSettings, FieldMask};
use crate::state::{AppHandle, LogKind, ToastKind, UiEvent};

// ============ Toast ============

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub until: Instant,
}

pub fn show_toasts(ctx: &egui::Context, toasts: &mut Vec<Toast>) {
    let now = Instant::now();
    toasts.retain(|t| t.until > now);
    if toasts.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0])
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                for t in toasts.iter() {
                    let (color, icon) = match t.kind {
                        ToastKind::Info => (
                            egui::Color32::from_rgb(80, 130, 180),
                            crate::ui::icons::TOAST_INFO,
                        ),
                        ToastKind::Success => (
                            egui::Color32::from_rgb(80, 160, 90),
                            crate::ui::icons::TOAST_SUCCESS,
                        ),
                        ToastKind::Warning => (
                            egui::Color32::from_rgb(200, 160, 60),
                            crate::ui::icons::TOAST_WARNING,
                        ),
                        ToastKind::Error => (
                            egui::Color32::from_rgb(200, 80, 80),
                            crate::ui::icons::TOAST_ERROR,
                        ),
                    };
                    egui::Frame::new()
                        .fill(color)
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin {
                            left: 14,
                            right: 14,
                            top: 8,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(360.0);
                            // 图标用 Phosphor 字体、文本用 Proportional，避免 icon
                            // 字符被 Proportional 字体“吃掉”。
                            let resp = ui.allocate_response(
                                egui::vec2(ui.available_width(), 16.0),
                                egui::Sense::hover(),
                            );
                            crate::ui::fonts::paint_icon_text_in(
                                ui,
                                resp.rect,
                                icon,
                                &t.text,
                                13.0,
                                egui::Color32::WHITE,
                                6.0,
                            );
                        });
                }
            });
        });
}

pub fn push_toast(toasts: &mut Vec<Toast>, kind: ToastKind, text: impl Into<String>, ttl_ms: u64) {
    toasts.push(Toast {
        kind,
        text: text.into(),
        until: Instant::now() + Duration::from_millis(ttl_ms),
    });
}

// ============ Confirm Dialog ============

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfirmOutcome {
    None,
    Yes,
    No,
}

pub fn show_confirm(
    ctx: &egui::Context,
    title: &str,
    body: &str,
    open: &mut bool,
) -> ConfirmOutcome {
    let mut outcome = ConfirmOutcome::None;
    egui::Window::new(title)
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_size([380.0, 180.0])
        .min_size([320.0, 140.0])
        .max_size([520.0, 320.0])
        .show(ctx, |ui| {
            ui.label(body);
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let primary = egui::Button::new("继续")
                    .fill(crate::ui::ACCENT)
                    .corner_radius(egui::CornerRadius::same(6));
                if ui.add(primary).clicked() {
                    outcome = ConfirmOutcome::Yes;
                }
                if ui.button("取消").clicked() {
                    outcome = ConfirmOutcome::No;
                }
            });
        });
    outcome
}

// ============ Local Settings 弹窗 ============

/// 显示本地设置弹窗。返回用户操作：
/// - `None`：保持打开 / 什么都没做
/// - `Some(true)`：点确定
/// - `Some(false)`：点关闭（X）或取消
pub fn show_local_settings(
    ctx: &egui::Context,
    handle: &AppHandle,
    open: &mut bool,
) -> Option<bool> {
    let mut result = None;
    egui::Window::new(format!("{} 本地设置", crate::ui::icons::GEAR))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_size([420.0, 360.0])
        .show(ctx, |ui| {
            // 1) 自动连接
            ui.group(|ui| {
                ui.strong("连接");
                let mut ac = *handle.auto_connect.lock().unwrap();
                if ui.checkbox(&mut ac, "启动时自动连接上次端口").changed() {
                    *handle.auto_connect.lock().unwrap() = ac;
                }
            });

            // 2) 语言
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong("语言");
                let mut lang = handle.language();
                egui::ComboBox::from_id_salt("lang-combo")
                    .selected_text(lang.label())
                    .show_ui(ui, |cb| {
                        cb.selectable_value(&mut lang, crate::config::Language::Chinese, "中文");
                        cb.selectable_value(&mut lang, crate::config::Language::English, "English");
                    });
                if lang != handle.language() {
                    handle.local_config.lock().unwrap().language = lang;
                }
                ui.label("（阶段 04 仅中文生效；切换后 UI 文案尚未本地化）");
            });

            // 3) 主题
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong("主题");
                let mut theme = handle.theme();
                egui::ComboBox::from_id_salt("theme-combo")
                    .selected_text(theme.label())
                    .show_ui(ui, |cb| {
                        cb.selectable_value(&mut theme, crate::config::Theme::Dark, "深色");
                        cb.selectable_value(&mut theme, crate::config::Theme::Light, "浅色");
                    });
                if theme != handle.theme() {
                    handle.local_config.lock().unwrap().theme = theme;
                    let _ = handle.ui_tx.send(UiEvent::Toast(
                        ToastKind::Info,
                        "主题切换将在下次启动生效".to_string(),
                    ));
                }
            });

            // 4) 窗口大小（只读展示）
            ui.add_space(4.0);
            ui.group(|ui| {
                ui.strong("窗口");
                ui.label("当前大小会在退出时自动保存，下次启动恢复。");
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("关闭").clicked() {
                    result = Some(false);
                }
            });
        });
    result
}

// ============ DiffPreviewBar ============

/// 按 `mask` 把 `diff` 中被修改的字段逐个写入 JSON 对象（键名用协议字段名）。
///
/// 必须手动逐字段构建，**不能**序列化整个 `DeviceSettings`：
/// `serde_json::json!({"config": &settings})` 会把未修改字段以 `Default`
/// 值（0 / 空串）全部输出；固件按"字段是否出现在 `data.config`"判定增量
/// （协议 §5.2），这些默认值会被当成显式配置写回设备 ——
/// tft_brightness=0 被钳到 5（背光变最小）、WiFi SSID 被清空、
/// 音量 / 灯效模式被重置等。
///
/// `active_profile_name` / `active_profile_has_custom_icon` 是固件→App 的
/// 展示字段，0x08 不下发（固件 parseConfigSetCommand 也不处理）。
/// 新增协议字段时在此追加一项，与 `diff()` / `merge_push()` / `apply()` 同步。
fn build_config_payload(diff: &DeviceSettings, mask: FieldMask) -> serde_json::Value {
    let mut cfg = serde_json::Map::new();
    macro_rules! put_if {
        ($bit:expr, $name:literal, $f:ident) => {
            if mask.test($bit) {
                cfg.insert($name.to_string(), serde_json::json!(diff.$f));
            }
        };
    }
    put_if!(crate::protocol::F_WIFI_SWITCH, "wifi_switch", wifi_switch);
    put_if!(
        crate::protocol::F_CONNECT_HOST,
        "connect_host",
        connect_host
    );
    put_if!(crate::protocol::F_WIFI_SSID, "wifi_ssid", wifi_ssid);
    put_if!(
        crate::protocol::F_WIFI_PASSWORD,
        "wifi_password",
        wifi_password
    );
    put_if!(crate::protocol::F_WORK_MODE, "work_mode", work_mode);
    put_if!(crate::protocol::F_RGB_MODE, "rgb_mode", rgb_mode);
    put_if!(
        crate::protocol::F_RGB_SINGLE_COLOR,
        "rgb_single_color",
        rgb_single_color
    );
    put_if!(
        crate::protocol::F_RGB_CLICK_MODE,
        "rgb_click_mode",
        rgb_click_mode
    );
    put_if!(
        crate::protocol::F_RGB_BRIGHTNESS,
        "rgb_brightness",
        rgb_brightness
    );
    put_if!(crate::protocol::F_TFT_THEME, "tft_theme", tft_theme);
    put_if!(
        crate::protocol::F_TFT_BRIGHTNESS,
        "tft_brightness",
        tft_brightness
    );
    put_if!(
        crate::protocol::F_DEVICE_VOLUME,
        "device_volume",
        device_volume
    );
    put_if!(
        crate::protocol::F_AUDIO_ENABLE,
        "audio_enable",
        audio_enable
    );
    put_if!(crate::protocol::F_POWER_MODE, "power_mode", power_mode);
    put_if!(
        crate::protocol::F_VOICE_ENABLE,
        "voice_enable",
        voice_enable
    );
    put_if!(
        crate::protocol::F_VOICE_TRIGGER_KEY,
        "voice_trigger_key",
        voice_trigger_key
    );
    put_if!(
        crate::protocol::F_VOICE_MAX_RECORD_MS,
        "voice_max_record_ms",
        voice_max_record_ms
    );
    put_if!(
        crate::protocol::F_VOICE_AUTO_ENTER,
        "voice_auto_enter",
        voice_auto_enter
    );
    put_if!(
        crate::protocol::F_VOICE_DEV_PID,
        "voice_dev_pid",
        voice_dev_pid
    );
    put_if!(crate::protocol::F_VOICE_CUID, "voice_cuid", voice_cuid);
    put_if!(
        crate::protocol::F_VOICE_BAIDU_API_KEY,
        "voice_baidu_api_key",
        voice_baidu_api_key
    );
    put_if!(
        crate::protocol::F_VOICE_BAIDU_SECRET_KEY,
        "voice_baidu_secret_key",
        voice_baidu_secret_key
    );
    put_if!(
        crate::protocol::F_PC_STATUS_MASK,
        "pc_status_mask",
        pc_status_mask
    );
    put_if!(
        crate::protocol::F_ACTIVE_KEYMAP_PROFILE,
        "active_keymap_profile",
        active_keymap_profile
    );
    serde_json::Value::Object(cfg)
}

/// 把 diff 通过 0x08 下发；成功后让 settings 刷新
///
/// 协议约定：固件按"字段是否出现在 `data.config` 中"判断增量（详见
/// `docs/protocol-usage.md` §4 与固件侧 `parseConfigSetCommand.cpp`）。
/// 因此这里**不发 `mask`** —— 内部 `FieldMask` 仅用于 App 端的 diff 判定。
pub fn apply_diff(handle: &AppHandle, diff: &DeviceSettings, mask: FieldMask) {
    use crate::protocol::CMD_CONFIG_SET;
    // 仅把 mask 置位的字段写进 data.config（见 build_config_payload 的注释：
    // 整体序列化会把未修改字段以 Default 值发出，被固件当显式配置写回）。
    let payload = serde_json::json!({ "config": build_config_payload(diff, mask) });
    let _ = handle.with_link(|lm| {
        match lm.request(
            CMD_CONFIG_SET,
            Some(payload),
            std::time::Duration::from_millis(1000),
        ) {
            Ok(resp) => {
                if resp.status() == Some(1) {
                    let msg = resp.error.unwrap_or_else(|| "未知错误".into());
                    handle.log_kind(crate::state::LogKind::App, format!("SET 失败: {msg}"));
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Error,
                        msg,
                    ));
                } else {
                    handle.log_kind(crate::state::LogKind::Tx, "SET → 已下发");
                    let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                        crate::state::ToastKind::Success,
                        "已应用".to_string(),
                    ));
                }
            }
            Err(e) => {
                handle.log_kind(crate::state::LogKind::App, format!("SET 超时: {e}"));
                let _ = handle.ui_tx.send(crate::state::UiEvent::Toast(
                    crate::state::ToastKind::Warning,
                    "设备无响应".to_string(),
                ));
            }
        }
    });
}

pub fn show_diff_bar(
    handle: &AppHandle,
    ui: &mut egui::Ui,
    diff: &DeviceSettings,
    mask: FieldMask,
    can_apply: bool,
) -> DiffAction {
    let mut action = DiffAction::None;
    let count = diff_field_count(diff, mask);
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(
            1.0,
            crate::ui::ACCENT.gamma_multiply(0.6),
        ))
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
                            // 严格按 mask 决定展示哪些字段，合法 0 / 空串也能正确呈现。
                            if mask.test(crate::protocol::F_TFT_BRIGHTNESS) {
                                ui.label(format!("tft_brightness={}", diff.tft_brightness));
                            }
                            if mask.test(crate::protocol::F_TFT_THEME) {
                                ui.label(format!("tft_theme={}", diff.tft_theme));
                            }
                            if mask.test(crate::protocol::F_WORK_MODE) {
                                ui.label(format!("work_mode={}", diff.work_mode));
                            }
                            if mask.test(crate::protocol::F_ACTIVE_KEYMAP_PROFILE) {
                                ui.label(format!(
                                    "active_keymap_profile={}",
                                    diff.active_keymap_profile
                                ));
                            }
                            if mask.test(crate::protocol::F_DEVICE_VOLUME) {
                                ui.label(format!("device_volume={}", diff.device_volume));
                            }
                            if mask.test(crate::protocol::F_AUDIO_ENABLE) {
                                ui.label(format!("audio_enable={}", diff.audio_enable));
                            }
                            if mask.test(crate::protocol::F_POWER_MODE) {
                                ui.label(format!("power_mode={}", diff.power_mode));
                            }
                            if mask.test(crate::protocol::F_RGB_MODE) {
                                ui.label(format!("rgb_mode={}", diff.rgb_mode));
                            }
                            if mask.test(crate::protocol::F_RGB_SINGLE_COLOR) {
                                ui.label(format!("rgb_single_color={}", diff.rgb_single_color));
                            }
                            if mask.test(crate::protocol::F_RGB_CLICK_MODE) {
                                ui.label(format!("rgb_click_mode={}", diff.rgb_click_mode));
                            }
                            if mask.test(crate::protocol::F_RGB_BRIGHTNESS) {
                                ui.label(format!("rgb_brightness={}", diff.rgb_brightness));
                            }
                        });
                    });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("放弃 (Esc)").clicked() {
                        action = DiffAction::Discard;
                    }
                    let apply_btn = egui::Button::new("应用 (Ctrl+Enter)")
                        .fill(crate::ui::ACCENT)
                        .corner_radius(egui::CornerRadius::same(6));
                    if ui.add_enabled(can_apply, apply_btn).clicked() {
                        action = DiffAction::Apply;
                    }
                });
            });
        });
    action
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffAction {
    None,
    Apply,
    Discard,
}

fn diff_field_count(_d: &DeviceSettings, mask: FieldMask) -> usize {
    // 与 diff() 的 mask 严格对齐：count = mask 中置位的位数。
    // 这样当 diff 含合法 0 / 空串字段时也能正确计入"几处变更"。
    mask.bits().count_ones() as usize
}

// ============ 简易 FieldEditor 辅助 ============

/// 字段变更事件（面板 → AppHandle）
#[derive(Debug, Clone)]
pub enum FieldChange {
    Set(DeviceSettings),
}

/// 占位：导出 LogKind 以便 panel_log 引用
#[allow(dead_code)]
pub fn _kind_marker() -> LogKind {
    LogKind::App
}

// ============ Settings Panel 通用脚手架 ============

/// 统一的"settings 类"面板脚手架：clone snapshot/draft → 调回调填表 →
/// 计算 diff → 渲染 DiffPreviewBar → 把变更写回 draft。
///
/// `panel_body` 直接在传入的 `Ui` 内执行（外层 `CentralPanel` 已统一提供
/// 垂直滚动，这里不再嵌套 `ScrollArea`，避免出现两个滚动条）。
/// 写回始终发生（即使没改），因为 tab 切换时也要把当前显示状态
/// 同步到草稿，避免下次进入面板看到过期数据。
///
/// 适用面板：Settings / Lighting / WiFi / Voice。
pub fn settings_panel_scaffold(
    handle: &AppHandle,
    ui: &mut egui::Ui,
    panel_body: impl FnOnce(&mut egui::Ui, &DeviceSettings, &mut DeviceSettings),
) {
    let snapshot = handle.settings.lock().unwrap().clone();
    let mut draft = handle.draft.lock().unwrap().clone();

    panel_body(ui, &snapshot, &mut draft);

    let (diff, mask) = draft.diff(&snapshot);
    let has_diff = !mask.is_empty();

    ui.add_space(8.0);
    let action = show_diff_bar(handle, ui, &diff, mask, has_diff);
    match action {
        DiffAction::Apply => apply_diff(handle, &diff, mask),
        DiffAction::Discard => {
            *handle.draft.lock().unwrap() = snapshot.clone();
        }
        DiffAction::None => {}
    }

    // 始终同步 draft（tab 切换 / 切走再回来时保持一致）
    *handle.draft.lock().unwrap() = draft;
}
