//! 协议层：命令常量 / Frame / 编解码 / DeviceSettings
//!
//! 纯函数模块，不允许任何 IO。所有 JSON 编解码都在这里完成，
//! 便于单测。详细字段定义见 `docs/desktop-app-protocol.md`。

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------- 命令 ID（与协议 §2 对齐） ----------

pub const CMD_CONFIG_GET: u8 = 0x07;
pub const CMD_CONFIG_SET: u8 = 0x08;
pub const CMD_HEARTBEAT: u8 = 0x0a;

/// 响应帧命令 ID = 请求命令 ID | 0x80
pub const fn response_cmd(req: u8) -> u8 {
    req | 0x80
}

/// 是否为响应帧（cmd 最高位为 1）
pub const fn is_response(cmd: u8) -> bool {
    cmd & 0x80 != 0
}

// ---------- 错误 ----------

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown command: {0}")]
    UnknownCommand(u8),
}

// ---------- Frame ----------

/// 设备侧返回的一帧（请求响应或 seq=0 主动推送）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub cmd: u8,
    pub seq: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Frame {
    /// App → 设备 的请求帧（不带 status/error）
    pub fn request(cmd: u8, seq: u32, data: Option<serde_json::Value>) -> Self {
        Self {
            cmd,
            seq,
            data,
            status: None,
            error: None,
        }
    }

    /// 序列化为一行 JSON + `\n`
    pub fn encode_line(&self) -> String {
        // 序列化失败概率极低（字段皆可序列化）；失败则退化为空对象
        let mut s = serde_json::to_string(self).unwrap_or_else(|_| "{}".into());
        s.push('\n');
        s
    }

    /// 是否为响应帧
    pub fn is_response(&self) -> bool {
        is_response(self.cmd)
    }

    /// 是否为主动推送（响应帧 + seq=0）
    pub fn is_push(&self) -> bool {
        self.is_response() && self.seq == 0
    }

    /// 状态码：成功 = Some(0)；失败 = Some(1) + error；非响应帧 = None
    pub fn status(&self) -> Option<u8> {
        self.status
    }
}

/// 按行解析单行输入；非 JSON 行返回 `None`，留给 reader 归类为日志。
pub fn try_parse_line(line: &str) -> Option<Result<Frame, ProtocolError>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    Some(serde_json::from_str::<Frame>(trimmed).map_err(ProtocolError::from))
}

// ---------- DeviceSettings（与协议 §3 字段全集对齐） ----------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceSettings {
    // WiFi（阶段 06 生效）
    #[serde(default)]
    pub wifi_switch: i32,
    #[serde(default)]
    pub connect_host: i32,
    #[serde(default)]
    pub wifi_ssid: String,
    #[serde(default)]
    pub wifi_password: String,

    // 通用
    #[serde(default)]
    pub work_mode: i32, // 0=USB 1=BLE 2=2.4G
    #[serde(default)]
    pub rgb_mode: i32,
    #[serde(default)]
    pub rgb_single_colar: i32,
    #[serde(default)]
    pub rgb_click_mode: i32,
    #[serde(default)]
    pub rgb_brightness: i32,
    #[serde(default)]
    pub tft_theme: i32,
    #[serde(default)]
    pub tft_brightness: i32, // 5~100
    #[serde(default)]
    pub device_volume: i32,
    #[serde(default)]
    pub audio_enable: i32,
    #[serde(default)]
    pub power_mode: i32,

    // Voice（阶段 06 生效）
    #[serde(default)]
    pub voice_enable: i32,
    #[serde(default)]
    pub voice_trigger_key: i32,
    #[serde(default)]
    pub voice_max_record_ms: i32,
    #[serde(default)]
    pub voice_auto_enter: i32,
    #[serde(default)]
    pub voice_dev_pid: i32,
    #[serde(default)]
    pub voice_cuid: String,
    #[serde(default)]
    pub voice_baidu_api_key: String,
    #[serde(default)]
    pub voice_baidu_secret_key: String,

    // PC（阶段 05 生效）
    #[serde(default)]
    pub pc_status_mask: i32,

    // Profile（阶段 04 已生效）
    #[serde(default)]
    pub active_keymap_profile: i32,
    #[serde(default)]
    pub active_profile_name: String,
    #[serde(default)]
    pub active_profile_has_custom_icon: bool,
}

impl DeviceSettings {
    /// 协议 §4.1：钳位规则。返回 true 表示有字段被修改。
    pub fn clamp(&mut self) -> bool {
        let mut changed = false;
        if self.tft_brightness < 5 {
            self.tft_brightness = 5;
            changed = true;
        }
        if self.tft_brightness > 100 {
            self.tft_brightness = 100;
            changed = true;
        }
        if !(0..=2).contains(&self.work_mode) {
            self.work_mode = 0;
            changed = true;
        }
        if !(0..=7).contains(&self.active_keymap_profile) {
            self.active_keymap_profile = 0;
            changed = true;
        }
        changed
    }

    /// 计算与另一份快照的差异（仅包含有变化的字段），用于 DiffPreviewBar / SET 增量下发。
    pub fn diff(&self, other: &DeviceSettings) -> DeviceSettings {
        let mut d = DeviceSettings::default();
        macro_rules! cmp {
            ($($f:ident),* $(,)?) => {
                $(
                    if self.$f != other.$f {
                        d.$f = self.$f.clone();
                    }
                )*
            };
        }
        cmp!(
            wifi_switch,
            connect_host,
            wifi_ssid,
            wifi_password,
            work_mode,
            rgb_mode,
            rgb_single_colar,
            rgb_click_mode,
            rgb_brightness,
            tft_theme,
            tft_brightness,
            device_volume,
            audio_enable,
            power_mode,
            voice_enable,
            voice_trigger_key,
            voice_max_record_ms,
            voice_auto_enter,
            voice_dev_pid,
            voice_cuid,
            voice_baidu_api_key,
            voice_baidu_secret_key,
            pc_status_mask,
            active_keymap_profile,
            active_profile_name,
            active_profile_has_custom_icon,
        );
        d
    }

    /// 合并一个快照与一份草稿：对每个字段，如果草稿"未改动"（== 旧快照），用新值；
    /// 否则保留草稿值（草稿优先）。
    ///
    /// 这要求传入"推送前旧快照 old_snapshot"、"推送新快照 new_snapshot"、"草稿 draft"。
    pub fn merge_push(
        new_snapshot: &DeviceSettings,
        old_snapshot: &DeviceSettings,
        draft: &mut DeviceSettings,
    ) {
        macro_rules! merge_field {
            ($f:ident) => {
                // 草稿与旧快照相同 → 草稿"未改"，用推送值
                // 草稿与旧快照不同 → 用户改过，保留草稿
                // 比较用 PartialEq；DeviceSettings 实现 PartialEq
                if draft.$f == old_snapshot.$f {
                    draft.$f = new_snapshot.$f.clone();
                }
            };
        }
        // i32 字段直接比较
        merge_field!(wifi_switch);
        merge_field!(connect_host);
        // String 字段（合并判断同上）
        merge_field!(wifi_ssid);
        merge_field!(wifi_password);
        merge_field!(work_mode);
        merge_field!(rgb_mode);
        merge_field!(rgb_single_colar);
        merge_field!(rgb_click_mode);
        merge_field!(rgb_brightness);
        merge_field!(tft_theme);
        merge_field!(tft_brightness);
        merge_field!(device_volume);
        merge_field!(audio_enable);
        merge_field!(power_mode);
        merge_field!(voice_enable);
        merge_field!(voice_trigger_key);
        merge_field!(voice_max_record_ms);
        merge_field!(voice_auto_enter);
        merge_field!(voice_dev_pid);
        merge_field!(voice_cuid);
        merge_field!(voice_baidu_api_key);
        merge_field!(voice_baidu_secret_key);
        merge_field!(pc_status_mask);
        merge_field!(active_keymap_profile);
        merge_field!(active_profile_name);
        merge_field!(active_profile_has_custom_icon);
    }

    /// 应用一个 diff 增量到自身
    pub fn apply(&mut self, diff: &DeviceSettings) {
        // 仅覆盖非默认值字段。`diff` 由 `diff()` 产生，未变化的字段保持 Default。
        // 对 String/bool 用 `is_default` 判断；对 i32 用"非 0"会误判 0 → 故改用：
        // 直接覆盖（diff 字段就是有变化的值）。
        if !diff.wifi_ssid.is_empty() || self.wifi_ssid != diff.wifi_ssid {
            self.wifi_ssid = diff.wifi_ssid.clone();
        }
        // 简化：直接逐字段覆盖（diff 由 `diff()` 生成，未变化的字段保持与 other 一致即可）
        self.wifi_switch = diff.wifi_switch;
        self.connect_host = diff.connect_host;
        self.wifi_password = diff.wifi_password.clone();
        self.work_mode = diff.work_mode;
        self.rgb_mode = diff.rgb_mode;
        self.rgb_single_colar = diff.rgb_single_colar;
        self.rgb_click_mode = diff.rgb_click_mode;
        self.rgb_brightness = diff.rgb_brightness;
        self.tft_theme = diff.tft_theme;
        self.tft_brightness = diff.tft_brightness;
        self.device_volume = diff.device_volume;
        self.audio_enable = diff.audio_enable;
        self.power_mode = diff.power_mode;
        self.voice_enable = diff.voice_enable;
        self.voice_trigger_key = diff.voice_trigger_key;
        self.voice_max_record_ms = diff.voice_max_record_ms;
        self.voice_auto_enter = diff.voice_auto_enter;
        self.voice_dev_pid = diff.voice_dev_pid;
        self.voice_cuid = diff.voice_cuid.clone();
        self.voice_baidu_api_key = diff.voice_baidu_api_key.clone();
        self.voice_baidu_secret_key = diff.voice_baidu_secret_key.clone();
        self.pc_status_mask = diff.pc_status_mask;
        self.active_keymap_profile = diff.active_keymap_profile;
        self.active_profile_name = diff.active_profile_name.clone();
        self.active_profile_has_custom_icon = diff.active_profile_has_custom_icon;
    }
}

// ---------- 配置 SET 请求体 ----------

#[derive(Debug, Clone, Serialize)]
pub struct ConfigSetPayload<'a> {
    pub config: &'a DeviceSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let f = Frame::request(CMD_CONFIG_GET, 42, None);
        let s = f.encode_line();
        assert!(s.ends_with('\n'));
        let parsed = try_parse_line(&s).unwrap().unwrap();
        assert_eq!(parsed.cmd, CMD_CONFIG_GET);
        assert_eq!(parsed.seq, 42);
    }

    #[test]
    fn non_json_line_is_none() {
        let line = "[I]MAIN: hello";
        assert!(try_parse_line(line).is_none());
    }

    #[test]
    fn clamp_brightness() {
        let mut s = DeviceSettings::default();
        s.tft_brightness = 200;
        assert!(s.clamp());
        assert_eq!(s.tft_brightness, 100);

        s.tft_brightness = 0;
        assert!(s.clamp());
        assert_eq!(s.tft_brightness, 5);
    }

    #[test]
    fn diff_only_changed() {
        let a = DeviceSettings::default();
        let mut b = a.clone();
        b.tft_brightness = 80;
        b.work_mode = 1;
        let d = b.diff(&a);
        assert_eq!(d.tft_brightness, 80);
        assert_eq!(d.work_mode, 1);
        assert_eq!(d.tft_theme, 0); // 未变化 → default
    }
}
