//! 协议层：命令常量 / Frame / 编解码 / DeviceSettings
//!
//! 纯函数模块，不允许任何 IO。所有 JSON 编解码都在这里完成，
//! 便于单测。详细字段定义见 `docs/desktop-app-protocol.md`。

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------- 字段变更位掩码 ----------
//
// `DeviceSettings` 里很多字段（如 `work_mode`、`rgb_mode`、`wifi_switch`、`voice_enable`）
// 的合法值包含 0；`wifi_ssid` 等 String 字段的合法值也包含空串。
// 之前用 `field != 0` 或 `field.is_empty()` 判"有变化"会把合法值 0 / 空串
// 误判为"无变化"，导致下发时静默丢更新（固件永远不会收到把字段设为
// USB / 清空 SSID 这种合法值）。
//
// 解决：把"是否有变化"显式存到 `FieldMask` 里，与值解耦。
// diff / is_any_diff / apply / merge_push 都以 mask 为准，避免歧义。

/// 每位对应一个字段；位=1 表示该字段在 diff 中是"显式有变化"。
///
/// 位编号必须与 `FIELD_LIST` 严格一一对应。新增字段时同步在末尾追加。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMask(u64);

impl FieldMask {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn set(mut self, bit: u8) -> Self {
        self.0 |= 1u64 << bit;
        self
    }

    pub const fn test(self, bit: u8) -> bool {
        self.0 & (1u64 << bit) != 0
    }

    pub fn bits(self) -> u64 {
        self.0
    }

    /// 交集：仅保留两边都置位的位（用于 merge_push 合并 draft 状态）。
    pub fn intersect(self, other: FieldMask) -> FieldMask {
        FieldMask(self.0 & other.0)
    }

    pub fn union(self, other: FieldMask) -> FieldMask {
        FieldMask(self.0 | other.0)
    }
}

impl std::ops::BitOr for FieldMask {
    type Output = FieldMask;
    fn bitor(self, rhs: FieldMask) -> FieldMask {
        self.union(rhs)
    }
}

/// 字段序号常量。新增字段 → 末尾追加；删字段 → 不要重用编号。
pub const F_WIFI_SWITCH: u8 = 0;
pub const F_CONNECT_HOST: u8 = 1;
pub const F_WIFI_SSID: u8 = 2;
pub const F_WIFI_PASSWORD: u8 = 3;
pub const F_WORK_MODE: u8 = 4;
pub const F_RGB_MODE: u8 = 5;
pub const F_RGB_SINGLE_COLOR: u8 = 6;
pub const F_RGB_CLICK_MODE: u8 = 7;
pub const F_RGB_BRIGHTNESS: u8 = 8;
pub const F_TFT_THEME: u8 = 9;
pub const F_TFT_BRIGHTNESS: u8 = 10;
pub const F_DEVICE_VOLUME: u8 = 11;
pub const F_AUDIO_ENABLE: u8 = 12;
pub const F_POWER_MODE: u8 = 13;
pub const F_VOICE_ENABLE: u8 = 14;
pub const F_VOICE_TRIGGER_KEY: u8 = 15;
pub const F_VOICE_MAX_RECORD_MS: u8 = 16;
pub const F_VOICE_AUTO_ENTER: u8 = 17;
pub const F_VOICE_DEV_PID: u8 = 18;
pub const F_VOICE_CUID: u8 = 19;
pub const F_VOICE_BAIDU_API_KEY: u8 = 20;
pub const F_VOICE_BAIDU_SECRET_KEY: u8 = 21;
pub const F_PC_STATUS_MASK: u8 = 22;
pub const F_ACTIVE_KEYMAP_PROFILE: u8 = 23;
pub const F_ACTIVE_PROFILE_NAME: u8 = 24;
pub const F_ACTIVE_PROFILE_HAS_CUSTOM_ICON: u8 = 25;

/// 字段清单；新增字段时同步追加到末尾，并对应一个新编号。
/// 配合宏使用，确保 `diff` / `is_any_diff` / `merge_push` 不漏字段。
pub const FIELD_COUNT: usize = 26;

// ---------- 命令 ID（与协议 §2 对齐） ----------
//
// 完整 18 个命令定义见 `docs/desktop-app-protocol.md`（固件侧权威）。
// 本文件只声明 App 端**当前已实现或已规划**的命令常量。

/// 配置结构版本：查询
pub const CMD_CONF_VERSION_GET: u8 = 0x01;
/// 配置结构版本：写入
pub const CMD_CONF_VERSION_SET: u8 = 0x02;
/// 设备信息：查询
pub const CMD_DEVICE_INFO_GET: u8 = 0x03;
/// 设备信息：修改（device_name / serial）
pub const CMD_DEVICE_INFO_SET: u8 = 0x04;
/// 键映射：读取当前 Profile
pub const CMD_KEYMAP_GET: u8 = 0x05;
/// 键映射：写入当前 Profile
pub const CMD_KEYMAP_SET: u8 = 0x06;
/// 配置全量快照：查询
pub const CMD_CONFIG_GET: u8 = 0x07;
/// 配置增量：写入
pub const CMD_CONFIG_SET: u8 = 0x08;
/// 物理按键上报（仅定义，未接通）
pub const CMD_KEY_EVENT: u8 = 0x09;
/// 心跳：App 探测设备
pub const CMD_HEARTBEAT: u8 = 0x0a;
/// 固件信息 / OTA 触发
pub const CMD_FIRMWARE_INFO: u8 = 0x0b;
/// 语音识别文本推送（固件 → App）
pub const CMD_VOICE_TEXT: u8 = 0x0c;
/// PC 状态推送（App → 固件）
pub const CMD_PC_STATUS: u8 = 0x0d;
/// 音乐播放器状态推送（App → 固件）
pub const CMD_MUSIC_STATUS: u8 = 0x0e;
/// 音乐控制推送（固件 → App，发送函数已实现，UI 链路未通）
pub const CMD_MUSIC_CONTROL: u8 = 0x0f;
/// Profile 状态（查询 / 推送；**例外**：返回 cmd=0x10 而非 0x90）
pub const CMD_PROFILE_STATE: u8 = 0x10;
/// Profile 图标上传 / 清除
pub const CMD_PROFILE_ICON_SET: u8 = 0x11;
/// HA 状态推送（仅定义，未接通）
pub const CMD_HA_STATUS: u8 = 0x12;
/// 系统时间注入（App → 固件，写入 epoch + tz）
pub const CMD_TIME_SET: u8 = 0x13;

/// 响应帧命令 ID = 请求命令 ID | 0x80
///
/// ⚠️ 例外：`CMD_PROFILE_STATE`（`0x10`）的响应帧就是 `0x10`，不带 `status`，
/// 详见 [protocol-usage.md §7.2](../docs/protocol-usage.md) 与
/// 固件侧 `cmd_profile.cpp`。
pub const fn response_cmd(req: u8) -> u8 {
    req | 0x80
}

/// 是否为响应帧（cmd 最高位为 1）
pub const fn is_response(cmd: u8) -> bool {
    cmd & 0x80 != 0
}

/// body 在帧顶层（非 `data`）的命令。
///
/// 当前包含：
/// - `0x10` Profile State（响应帧 cmd 仍是 0x10，**异类响应**）
/// - `0x0C` Voice Text（固件→App 推送，body 在顶层）
/// - `0x0F` Music Control（固件→App 推送，body 在顶层）
///
/// 路由层应优先识别这些命令再走 `is_response()` 判定，避免被误判。
pub const fn is_top_level_cmd(cmd: u8) -> bool {
    matches!(cmd, CMD_PROFILE_STATE | CMD_VOICE_TEXT | CMD_MUSIC_CONTROL)
}

/// 是否为响应类命令（标准响应 + 异类响应）
pub const fn is_response_like(cmd: u8) -> bool {
    is_response(cmd) || (cmd == CMD_PROFILE_STATE)
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
    /// 帧顶层**未声明**的字段（如 `0x05` 响应的顶层 `keymap` 数组）。
    ///
    /// serde `flatten` 会把未知字段收进来，序列化时原样并回顶层。
    /// 请求帧构造时为空；只有收到设备帧时可能非空。
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
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
            extra: serde_json::Map::new(),
        }
    }

    /// 读取帧顶层扩展字段（响应帧顶层 body 用，如 `0x05` 的 `keymap`）。
    pub fn extra_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.extra.get(key)
    }

    /// 序列化为一行 JSON + `\n`。
    ///
    /// 返回 `Err` 表示该帧**无法安全序列化**——此时不应发送"空对象"到设备
    /// （固件收到 `{}` 会当作未知命令），调用方应记录错误并跳过本次发送。
    pub fn encode_line(&self) -> Result<String, ProtocolError> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
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
        if self.is_response_like() {
            self.status
        } else {
            None
        }
    }

    /// 是否为响应类命令（标准响应 + 异类响应 0x10）
    pub fn is_response_like(&self) -> bool {
        is_response_like(self.cmd)
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

/// 按字节截断字符串到 `max_bytes`（不切断 UTF-8 字符边界）。
/// 返回 true 表示发生了截断。
fn truncate_bytes(s: &mut String, max_bytes: usize) -> bool {
    if s.len() <= max_bytes {
        return false;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    true
}

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
    pub rgb_single_color: i32,
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
    /// 协议钳位规则（对齐固件 `parseConfigSetCommand.cpp` 的 §6 行为）。
    ///
    /// App 端先钳位再下发，可以减少固件拒收/修正导致 diff 与实际不符的情况。
    /// 返回 true 表示有字段被修改。
    ///
    /// 注意：`clamp()` 修改值后会改变与固件的 diff 语义 —— 若 UI 正在编辑
    /// 一个超范围值，钳位后下发的是修正值，而非用户输入原值。这是预期行为。
    pub fn clamp(&mut self) -> bool {
        let mut changed = false;
        macro_rules! clamp_min_max {
            ($field:ident, $min:expr, $max:expr) => {
                if self.$field < $min {
                    self.$field = $min;
                    changed = true;
                }
                if self.$field > $max {
                    self.$field = $max;
                    changed = true;
                }
            };
        }
        macro_rules! normalize_01 {
            ($field:ident) => {
                if self.$field != 0 && self.$field != 1 {
                    self.$field = if self.$field != 0 { 1 } else { 0 };
                    changed = true;
                }
            };
        }

        // 亮度：5~100
        clamp_min_max!(tft_brightness, 5, 100);
        // RGB（与固件 parseConfigSetCommand.cpp §RGB + panel_lighting UI 选项对齐）
        // - rgb_mode：固件 RGBLightControl.h:22 RGBMode 枚举 0~7 共 8 种
        //   （关闭/单色/彩虹/彩浪/循环/电平/火焰/脉冲）
        // - rgb_single_color：固件 24 色调色板索引（RGBLightControl.cpp:74 `% 24` 兜底），
        //   UI 与固件实际语义对齐为 0~23
        // - rgb_click_mode：固件 ClickHighlight.h:34 ClickMode 枚举 0~2 共 3 种
        //   （关闭/单色按下点亮/渐变）
        // - rgb_brightness：UI Slider 0~100
        // 固件侧只做 uint8_t 截断、不做范围过滤；这里给 App 端兜底，避免
        // 越界值被静默接受后 UI 仍显示旧值造成"我改了为啥没生效"的歧义。
        clamp_min_max!(rgb_mode, 0, 7);
        clamp_min_max!(rgb_single_color, 0, 23);
        clamp_min_max!(rgb_click_mode, 0, 2);
        clamp_min_max!(rgb_brightness, 0, 100);
        // 工作模式：0=USB 1=BLE 2=2.4G
        if !(0..=2).contains(&self.work_mode) {
            self.work_mode = 0;
            changed = true;
        }
        // Profile：0~7
        if !(0..=7).contains(&self.active_keymap_profile) {
            self.active_keymap_profile = 0;
            changed = true;
        }
        // 开关量：归一化 0/1
        normalize_01!(wifi_switch);
        normalize_01!(connect_host);
        normalize_01!(voice_enable);
        normalize_01!(voice_auto_enter);
        // 语音触发键：0~11
        clamp_min_max!(voice_trigger_key, 0, 11);
        // 最大录音时长：1000~60000 ms
        clamp_min_max!(voice_max_record_ms, 1000, 60000);
        // 百度语音 PID：0~65535
        clamp_min_max!(voice_dev_pid, 0, 65535);
        // 字符串按字节截断（不切断 UTF-8 字符边界）：
        // - wifi_ssid 32 字节
        // - wifi_password / 百度 Key 64 字节
        changed |= truncate_bytes(&mut self.wifi_ssid, 32);
        changed |= truncate_bytes(&mut self.wifi_password, 64);
        changed |= truncate_bytes(&mut self.voice_baidu_api_key, 64);
        changed |= truncate_bytes(&mut self.voice_baidu_secret_key, 64);

        changed
    }

    /// 把敏感字段替换为 `***`（就地）。
    ///
    /// 用于**设备 → App** 方向：推送/GET 快照含 WiFi 密码与百度语音密钥明文。
    /// App 不存储、不展示这些明文；用户需要修改时在 UI 输入新值即可。
    /// 被 mask 的字段若在草稿中保持不变，不会进入 diff 下发（值相同）。
    pub fn mask_sensitive(&mut self) {
        self.wifi_password = "***".into();
        self.voice_baidu_api_key = "***".into();
        self.voice_baidu_secret_key = "***".into();
    }

    /// 计算与另一份快照的差异（仅包含有变化的字段），返回 (差量值, 字段掩码)。
    ///
    /// 配套用 `mask` 显式标记"哪些字段有变化"，避免用 `0` / 空串当哨兵误判合法值。
    /// 调用方应同时使用返回的 mask：`is_any_diff` / `apply` / `merge_push` 都基于 mask。
    pub fn diff(&self, other: &DeviceSettings) -> (DeviceSettings, FieldMask) {
        let mut d = DeviceSettings::default();
        let mut m = FieldMask::empty();
        // 新增字段时同步追加一项；位编号必须与文件顶部的 F_xxx 常量一致。
        macro_rules! cmp {
            ($bit:expr, $f:ident) => {
                if self.$f != other.$f {
                    d.$f = self.$f.clone();
                    m = m.set($bit);
                }
            };
        }
        cmp!(F_WIFI_SWITCH, wifi_switch);
        cmp!(F_CONNECT_HOST, connect_host);
        cmp!(F_WIFI_SSID, wifi_ssid);
        cmp!(F_WIFI_PASSWORD, wifi_password);
        cmp!(F_WORK_MODE, work_mode);
        cmp!(F_RGB_MODE, rgb_mode);
        cmp!(F_RGB_SINGLE_COLOR, rgb_single_color);
        cmp!(F_RGB_CLICK_MODE, rgb_click_mode);
        cmp!(F_RGB_BRIGHTNESS, rgb_brightness);
        cmp!(F_TFT_THEME, tft_theme);
        cmp!(F_TFT_BRIGHTNESS, tft_brightness);
        cmp!(F_DEVICE_VOLUME, device_volume);
        cmp!(F_AUDIO_ENABLE, audio_enable);
        cmp!(F_POWER_MODE, power_mode);
        cmp!(F_VOICE_ENABLE, voice_enable);
        cmp!(F_VOICE_TRIGGER_KEY, voice_trigger_key);
        cmp!(F_VOICE_MAX_RECORD_MS, voice_max_record_ms);
        cmp!(F_VOICE_AUTO_ENTER, voice_auto_enter);
        cmp!(F_VOICE_DEV_PID, voice_dev_pid);
        cmp!(F_VOICE_CUID, voice_cuid);
        cmp!(F_VOICE_BAIDU_API_KEY, voice_baidu_api_key);
        cmp!(F_VOICE_BAIDU_SECRET_KEY, voice_baidu_secret_key);
        cmp!(F_PC_STATUS_MASK, pc_status_mask);
        cmp!(F_ACTIVE_KEYMAP_PROFILE, active_keymap_profile);
        cmp!(F_ACTIVE_PROFILE_NAME, active_profile_name);
        cmp!(
            F_ACTIVE_PROFILE_HAS_CUSTOM_ICON,
            active_profile_has_custom_icon
        );
        (d, m)
    }

    /// 合并一个快照与一份草稿：对每个字段，如果草稿"未改动"（== 旧快照），用新值；
    /// 否则保留草稿值（草稿优先）。
    ///
    /// 这要求传入"推送前旧快照 old_snapshot"、"推送新快照 new_snapshot"、"草稿 draft"。
    /// 仅对 `old_mask` 中置位的字段（即旧快照里"有显式值"）做合并。
    pub fn merge_push(
        new_snapshot: &DeviceSettings,
        old_snapshot: &DeviceSettings,
        old_mask: FieldMask,
        new_mask: FieldMask,
        draft: &mut DeviceSettings,
    ) {
        // 仅当某字段"两边都存在显式值"时，merge_push 才有意义。
        let common = old_mask.intersect(new_mask);
        macro_rules! merge_field {
            ($bit:expr, $f:ident) => {
                if common.test($bit) {
                    // 草稿与旧一致 → 草稿"未改"，用推送值；否则保留草稿
                    if draft.$f == old_snapshot.$f {
                        draft.$f = new_snapshot.$f.clone();
                    }
                }
            };
        }
        merge_field!(F_WIFI_SWITCH, wifi_switch);
        merge_field!(F_CONNECT_HOST, connect_host);
        merge_field!(F_WIFI_SSID, wifi_ssid);
        merge_field!(F_WIFI_PASSWORD, wifi_password);
        merge_field!(F_WORK_MODE, work_mode);
        merge_field!(F_RGB_MODE, rgb_mode);
        merge_field!(F_RGB_SINGLE_COLOR, rgb_single_color);
        merge_field!(F_RGB_CLICK_MODE, rgb_click_mode);
        merge_field!(F_RGB_BRIGHTNESS, rgb_brightness);
        merge_field!(F_TFT_THEME, tft_theme);
        merge_field!(F_TFT_BRIGHTNESS, tft_brightness);
        merge_field!(F_DEVICE_VOLUME, device_volume);
        merge_field!(F_AUDIO_ENABLE, audio_enable);
        merge_field!(F_POWER_MODE, power_mode);
        merge_field!(F_VOICE_ENABLE, voice_enable);
        merge_field!(F_VOICE_TRIGGER_KEY, voice_trigger_key);
        merge_field!(F_VOICE_MAX_RECORD_MS, voice_max_record_ms);
        merge_field!(F_VOICE_AUTO_ENTER, voice_auto_enter);
        merge_field!(F_VOICE_DEV_PID, voice_dev_pid);
        merge_field!(F_VOICE_CUID, voice_cuid);
        merge_field!(F_VOICE_BAIDU_API_KEY, voice_baidu_api_key);
        merge_field!(F_VOICE_BAIDU_SECRET_KEY, voice_baidu_secret_key);
        merge_field!(F_PC_STATUS_MASK, pc_status_mask);
        merge_field!(F_ACTIVE_KEYMAP_PROFILE, active_keymap_profile);
        merge_field!(F_ACTIVE_PROFILE_NAME, active_profile_name);
        merge_field!(
            F_ACTIVE_PROFILE_HAS_CUSTOM_ICON,
            active_profile_has_custom_icon
        );
    }

    /// 应用一个 diff 增量到自身。仅覆盖 `mask` 中置位的字段，其他字段保持不变。
    pub fn apply(&mut self, diff: &DeviceSettings, mask: FieldMask) {
        macro_rules! apply_field {
            ($bit:expr, $f:ident) => {
                if mask.test($bit) {
                    self.$f = diff.$f.clone();
                }
            };
        }
        apply_field!(F_WIFI_SWITCH, wifi_switch);
        apply_field!(F_CONNECT_HOST, connect_host);
        apply_field!(F_WIFI_SSID, wifi_ssid);
        apply_field!(F_WIFI_PASSWORD, wifi_password);
        apply_field!(F_WORK_MODE, work_mode);
        apply_field!(F_RGB_MODE, rgb_mode);
        apply_field!(F_RGB_SINGLE_COLOR, rgb_single_color);
        apply_field!(F_RGB_CLICK_MODE, rgb_click_mode);
        apply_field!(F_RGB_BRIGHTNESS, rgb_brightness);
        apply_field!(F_TFT_THEME, tft_theme);
        apply_field!(F_TFT_BRIGHTNESS, tft_brightness);
        apply_field!(F_DEVICE_VOLUME, device_volume);
        apply_field!(F_AUDIO_ENABLE, audio_enable);
        apply_field!(F_POWER_MODE, power_mode);
        apply_field!(F_VOICE_ENABLE, voice_enable);
        apply_field!(F_VOICE_TRIGGER_KEY, voice_trigger_key);
        apply_field!(F_VOICE_MAX_RECORD_MS, voice_max_record_ms);
        apply_field!(F_VOICE_AUTO_ENTER, voice_auto_enter);
        apply_field!(F_VOICE_DEV_PID, voice_dev_pid);
        apply_field!(F_VOICE_CUID, voice_cuid);
        apply_field!(F_VOICE_BAIDU_API_KEY, voice_baidu_api_key);
        apply_field!(F_VOICE_BAIDU_SECRET_KEY, voice_baidu_secret_key);
        apply_field!(F_PC_STATUS_MASK, pc_status_mask);
        apply_field!(F_ACTIVE_KEYMAP_PROFILE, active_keymap_profile);
        apply_field!(F_ACTIVE_PROFILE_NAME, active_profile_name);
        apply_field!(
            F_ACTIVE_PROFILE_HAS_CUSTOM_ICON,
            active_profile_has_custom_icon
        );
    }
}

// ---------- 配置 SET 请求体 ----------

/// 增量 SET（与固件 `parseConfigSetCommand.cpp` 对齐）。
///
/// 协议约定（§5.2）：固件按"字段是否出现在 `data.config` 中"判断增量，
/// **不读 `mask`**。所以 payload 只放 diff 字段；App 端内部仍用 `FieldMask`
/// 决定哪些字段进 diff（避免合法 0 / 空串丢更新）。
#[derive(Debug, Clone, Serialize)]
pub struct ConfigSetPayload<'a> {
    pub config: &'a DeviceSettings,
}

impl FieldMask {
    /// 全 1 mask：表示"全量下发"。目前仅在测试与 merge_push 全量场景使用。
    pub const fn all() -> Self {
        // 仅置位到 FIELD_COUNT（避免引入未定义位）。使用掩码技巧：低 FIELD_COUNT 位全 1。
        Self((1u64 << FIELD_COUNT) - 1)
    }
}

// ---------- 各命令的请求 / 响应数据模型 ----------
//
// 命名约定：
// - `<Name>Req`     请求体（App → 固件）
// - `<Name>Resp`    响应体（固件 → App）
// - `<Name>Push`    主动推送体（固件 → App，seq=0）
//
// 仅定义"类型 + serde 字段"。具体下发/接收由 LinkManager / UI 层负责。

// ---------- 0x01/0x02 配置版本 ----------

/// `0x01 CMD_CONF_VERSION_GET` 请求：无需 body。
/// `0x02 CMD_CONF_VERSION_SET` 请求。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfVersionSetReq {
    pub version: u32,
}

/// `0x01/0x02` 响应。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfVersionResp {
    pub version: u32,
}

// ---------- 0x03/0x04 设备信息 ----------

/// `0x03 CMD_DEVICE_INFO_GET` 响应（位于 `data.device_info`，顶层也有同名
/// 字段以兼容不同版本固件；详见固件 `cmd_device_info.cpp`）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DeviceInfo {
    pub device_name: String,
    pub device_id: String, // 12 位大写十六进制
    pub firmware_version: String,
}

/// `0x04 CMD_DEVICE_INFO_SET` 请求：`device_name` / `serial` 至少一个。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceInfoSetReq {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
}

/// `0x04` 响应。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceInfoSetResp {
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub serial: String,
}

// ---------- 0x05/0x06 键映射 ----------
//
// 固件当前用简化模型（`physical`/`normal`/`macro`/`function`）描述每个键。
// App 侧 `KeymapData`（下文）描述完整 4 层 + 绑定表模型。
// 两者目前**没有一一对应**——这里只声明固件侧协议字段，供 App 端收发时使用。

/// 单个物理键的固件侧表示（最大 11 键）。
///
/// 固件优先级：`function` > `text` > `normal` > `macro`（cmd_keymap.cpp）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FirmwareKeyEntry {
    pub physical: u8, // 1~11
    #[serde(default)]
    pub normal: String,
    #[serde(rename = "macro", default)]
    pub macro_: String, // C++ 字段名 macro；Rust 保留字所以改 macro_
    /// 文本注入串（ASCII ≤128；按键触发整串输出一次）
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub function: String,
}

/// `0x05 CMD_KEYMAP_GET` 响应（位于 `data.keymap`）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeymapGetResp {
    #[serde(default)]
    pub keymap: Vec<FirmwareKeyEntry>,
}

/// `0x06 CMD_KEYMAP_SET` 请求。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeymapSetReq {
    #[serde(default)]
    pub keymap: Vec<FirmwareKeyEntry>,
}

// ---------- 0x0B 固件信息 / OTA ----------

/// `0x0B CMD_FIRMWARE_INFO` 查询响应（位于 `data.firmware`）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FirmwareInfo {
    pub version: String,
    pub device: String,
    pub build_date: String,
    pub build_time: String,
}

/// `0x0B` OTA 触发请求。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FirmwareOtaReq {
    pub url: String,
    /// MD5 十六进制字符串，长度 32
    pub checksum: String,
}

// ---------- 0x10 Profile State（异类响应） ----------
//
// ⚠️ 例外：响应帧 `cmd = 0x10`（不是 `0x90`），不带 `status`，body 在**顶层**
// 而非 `data.profile_state`。解析时需走单独路径。

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileState {
    pub active_profile: u8,
    pub profile_number: u8,
    pub profile_name: String,
    pub has_custom_icon: bool,
    pub icon_path: String,
}

/// 帧 wrapper：`ProfileState` 在固件 JSON 顶层 `profile_state` 字段里。
#[derive(Debug, Deserialize)]
struct ProfileStateFrame {
    #[serde(default)]
    profile_state: Option<ProfileState>,
}

impl<'de> serde::Deserialize<'de> for ProfileState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // 接受两种格式：
        // 1) 整行 JSON: { "cmd":.., "seq":.., "profile_state": {...} }
        // 2) 直接 body: { "active_profile":.., "profile_number":.., ... }
        // 注意：必须**避免**在自定义 Deserialize 里再次调用 from_value::<Self>，会无限递归。
        let v = serde_json::Value::deserialize(deserializer)?;
        let inner = v.get("profile_state").cloned().unwrap_or(v);
        #[derive(serde::Deserialize)]
        struct Body {
            #[serde(default)]
            active_profile: u8,
            #[serde(default)]
            profile_number: u8,
            #[serde(default)]
            profile_name: String,
            #[serde(default)]
            has_custom_icon: bool,
            #[serde(default)]
            icon_path: String,
        }
        let b: Body = serde_json::from_value(inner).map_err(serde::de::Error::custom)?;
        Ok(ProfileState {
            active_profile: b.active_profile,
            profile_number: b.profile_number,
            profile_name: b.profile_name,
            has_custom_icon: b.has_custom_icon,
            icon_path: b.icon_path,
        })
    }
}

impl Serialize for ProfileState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // 序列化时直接走普通结构体字段（用于 App 内部传递 / 测试）。
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ProfileState", 5)?;
        s.serialize_field("active_profile", &self.active_profile)?;
        s.serialize_field("profile_number", &self.profile_number)?;
        s.serialize_field("profile_name", &self.profile_name)?;
        s.serialize_field("has_custom_icon", &self.has_custom_icon)?;
        s.serialize_field("icon_path", &self.icon_path)?;
        s.end()
    }
}

// ---------- 0x11 Profile 图标 ----------

/// `0x11 CMD_PROFILE_ICON_SET` 请求。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileIconSetReq {
    pub profile: u8, // 0~7
    pub clear: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub png_base64: String,
}

/// `0x11` 请求的 `data` 载体：固件要求 body 包在 `profile_icon` 字段里
/// （见固件文档 §7.3），不是直接放 `ProfileIconSetReq`。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileIconSetPayload {
    pub profile_icon: ProfileIconSetReq,
}

/// `0x11` 响应（位于 `data`，注意与固件 profile 状态可能略有差异）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileIconSetResp {
    pub profile: u8,
    pub profile_number: u8,
    pub has_custom_icon: bool,
    pub profile_name: String,
}

// ---------- 0x0C 语音文本（推送） ----------

/// `0x0C CMD_VOICE_TEXT` 主动推送。
/// 注意：`text` / `timestamp` 在**顶层**，不在 `data`。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoiceTextPush {
    pub text: String,
    pub timestamp: u64,
}

/// 接受两种 JSON 形状：
/// - 整行 frame: `{ "cmd":.., "seq":.., "text":.., "timestamp":.. }`
/// - 单独 body: `{ "text":.., "timestamp":.. }`
impl<'de> serde::Deserialize<'de> for VoiceTextPush {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        // 直接从 v 取字段；找不到时走默认。
        let text = v
            .get("text")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let timestamp = v.get("timestamp").and_then(|x| x.as_u64()).unwrap_or(0);
        Ok(VoiceTextPush { text, timestamp })
    }
}

impl Serialize for VoiceTextPush {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("VoiceTextPush", 2)?;
        s.serialize_field("text", &self.text)?;
        s.serialize_field("timestamp", &self.timestamp)?;
        s.end()
    }
}

// ---------- 0x0D PC 状态（App → 固件） ----------

/// `0x0D CMD_PC_STATUS` 请求：`pc_status` 顶层位于 `data`。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PcStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caps_lock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_lock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_lock: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_connected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_usage_percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_usage_percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_temp_c: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_io_percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_up_kbps: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_down_kbps: Option<f32>,
}

/// `0x0D` 请求体。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PcStatusReq {
    #[serde(default)]
    pub pc_status: PcStatus,
}

/// `0x0D` 也支持配置 PC 状态显示掩码：`{ "type": "config", "mask": u32 }`。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PcStatusConfigReq {
    /// 字面量 `"config"`，用于在固件侧区分数据 / 配置两种用途。
    #[serde(default = "default_pc_config_type")]
    pub r#type: String,
    pub mask: u32,
}

fn default_pc_config_type() -> String {
    "config".into()
}

// ---------- 0x0E 音乐状态（App → 固件） ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MusicStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_playing: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_paused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_prev: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_next: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyric_current: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyric_next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MusicStatusReq {
    #[serde(default)]
    pub music_status: MusicStatus,
}

// ---------- 0x0F 音乐控制（固件 → App） ----------

/// 音乐控制动作（`0x0F` 推送或响应）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MusicControlAction {
    #[default]
    Toggle, // 播放 / 暂停
    Prev,
    Next,
}

/// `0x0F` 体（注意 `music_control` 在**顶层**，不在 `data`）。
#[derive(Debug, Clone, Default)]
pub struct MusicControl {
    pub action: MusicControlAction,
}

/// 接受两种 JSON 形状：
/// - 整行 frame: `{ "cmd":15, "seq":0, "music_control": { "action": "toggle" } }`
/// - 直接 body: `{ "action": "toggle" }`
impl<'de> serde::Deserialize<'de> for MusicControl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer)?;
        let inner = v.get("music_control").cloned().unwrap_or(v);
        #[derive(serde::Deserialize)]
        struct Body {
            action: MusicControlAction,
        }
        let b: Body = serde_json::from_value(inner).map_err(serde::de::Error::custom)?;
        Ok(MusicControl { action: b.action })
    }
}

impl Serialize for MusicControl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("MusicControl", 1)?;
        s.serialize_field("action", &self.action)?;
        s.end()
    }
}

// ---------- 心跳响应 ----------

/// `0x0A` 响应（位于 `data`）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HeartbeatResp {
    /// 设备开机后 millis()，可用于粗略判断设备是否重启
    #[serde(default)]
    pub timestamp: u64,
    #[serde(default)]
    pub device: String,
}

// ---------- 解析辅助 ----------

/// 从 `Frame::data` 取 Owned 类型；data 为 None 时返回 Default。
pub fn data_or_default<T: Default + serde::de::DeserializeOwned>(
    f: &Frame,
) -> Result<T, ProtocolError> {
    match &f.data {
        Some(v) => serde_json::from_value(v.clone()).map_err(ProtocolError::from),
        None => Ok(T::default()),
    }
}

/// 把一整行 JSON（包括异类响应字段）解析为目标类型。
///
/// 用于 `0x10` Profile State / `0x0C` Voice Text 这类 body 在**帧顶层**的命令：
/// 它们的字段不在 `Frame::data` 里，因此不能用 `data_or_default`。
pub fn parse_top_level<T: serde::de::DeserializeOwned>(line: &str) -> Result<T, ProtocolError> {
    serde_json::from_str::<T>(line.trim()).map_err(ProtocolError::from)
}

/// 读取 `Frame` 顶层字段（用于异类响应）。
///
/// 注意：本函数依赖 `serde_json::Value` 对未知字段的保留 —— 它把 Frame
/// 重新 to_value / from_value，因此**只能解析 Frame 已声明的字段**。
/// 异类命令（如 0x10 / 0x0C）的 body 字段不在 Frame 里，请用 `parse_top_level`。
pub fn top_level<T: serde::de::DeserializeOwned>(f: &Frame) -> Result<T, ProtocolError> {
    serde_json::from_value(serde_json::to_value(f).map_err(ProtocolError::from)?)
        .map_err(ProtocolError::from)
}

// ---------- Keymap 数据模型（阶段 05 起生效） ----------
//
// 这一组类型只描述"键映射"在桌面 App 侧的内存形态；阶段 05 之前固件侧
// 尚未支持 CMD_KEYMAP_GET/SET，所以这里只做"本地编辑 + 草稿预览"，不接
// 协议命令。设计上完全独立于 DeviceSettings，便于后面直接拆成单独的
// 命令而不影响现有 Settings 面板。

/// 修饰键位掩码（`KeyAction::Combo::mods` 用；bit 位与 HID modifier 顺序对齐）。
pub const MOD_CTRL: u8 = 1 << 0; // LCtrl 0xE0
pub const MOD_SHIFT: u8 = 1 << 1; // LShift 0xE1
pub const MOD_ALT: u8 = 1 << 2; // LAlt 0xE2
pub const MOD_GUI: u8 = 1 << 3; // LWin 0xE3

/// 键动作。模型与固件 KeyResolver/KeyNameTable 的**真实解析能力**对齐：
/// - 固件 `normal` 通道：`+` 分隔多段**同时按下**，段可为单字符键名 / `0xNN` /
///   修饰键名（Ctrl/Shift/Alt/Win，大小写不敏感）；
/// - 固件 `text` 通道：ASCII 文本注入（≤128 字符），按键触发整串输出一次；
/// - 固件 `function` 通道：单槽组合键（如 `Ctrl+c`，整串传入）、单独修饰键、
///   功能串 `KEY_FUNCTION_ASR`；`MEDIA_*` / `MOUSE_*` 等其它串固件
///   **无法解析**（按键无效），App 仅原样透传展示，不伪造语义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyAction {
    /// 未绑定（透传 / 触发默认）
    None,
    /// 普通单键：value 为 HID Usage ID（如 'A' = 0x04；0xE0~0xE3 修饰键单独成键）
    Keyboard(u16),
    /// 组合键：修饰键 + 主键同时按（如 Ctrl+Shift+S）
    Combo { mods: u8, code: u16 },
    /// 多个非修饰键同时按（固件 normal 通道 "a+b" 语义）
    Chord(Vec<u16>),
    /// 文本注入：按键触发整串输出一次（ASCII ≤128）
    Text(String),
    /// 固件功能串原样透传（`KEY_FUNCTION_ASR` / `MEDIA_PLAY` / 任意未识别串）
    Function(String),
}

impl Default for KeyAction {
    fn default() -> Self {
        KeyAction::None
    }
}

/// 修饰键掩码 → 固件修饰键段名（顺序固定，用于编码/展示）。
pub fn mods_names(mods: u8) -> Vec<&'static str> {
    let mut out = Vec::new();
    if mods & MOD_CTRL != 0 {
        out.push("Ctrl");
    }
    if mods & MOD_SHIFT != 0 {
        out.push("Shift");
    }
    if mods & MOD_ALT != 0 {
        out.push("Alt");
    }
    if mods & MOD_GUI != 0 {
        out.push("Win");
    }
    out
}

/// 修饰键掩码 → 主修饰键的 HID Usage ID（0xE0~0xE3；掩码为 0 返回 0xE0）。
pub fn mods_primary_usage(mods: u8) -> u16 {
    0xE0 + mods.trailing_zeros() as u16
}

/// 修饰键段名 → 位掩码（大小写不敏感；与固件 resolveModifierName 同名单集）。
pub fn modifier_name_to_mod(name: &str) -> Option<u8> {
    let n = name.trim();
    if n.eq_ignore_ascii_case("Ctrl")
        || n.eq_ignore_ascii_case("Control")
        || n.eq_ignore_ascii_case("Ctrl_L")
    {
        return Some(MOD_CTRL);
    }
    if n.eq_ignore_ascii_case("Shift") || n.eq_ignore_ascii_case("Shift_L") {
        return Some(MOD_SHIFT);
    }
    if n.eq_ignore_ascii_case("Alt")
        || n.eq_ignore_ascii_case("Option")
        || n.eq_ignore_ascii_case("Alt_L")
    {
        return Some(MOD_ALT);
    }
    if n.eq_ignore_ascii_case("Win")
        || n.eq_ignore_ascii_case("GUI")
        || n.eq_ignore_ascii_case("Meta")
        || n.eq_ignore_ascii_case("Cmd")
        || n.eq_ignore_ascii_case("Super")
    {
        return Some(MOD_GUI);
    }
    None
}

impl KeyAction {
    /// 给 UI 展示用的简短标签
    pub fn label(&self) -> String {
        match self {
            KeyAction::None => "未绑定".into(),
            KeyAction::Keyboard(code) => hid_key_label(*code)
                .map(str::to_string)
                .unwrap_or_else(|| format!("0x{code:02X}")),
            KeyAction::Combo { mods, code } => {
                let mut s = mods_names(*mods).join("+");
                if !s.is_empty() {
                    s.push('+');
                }
                s.push_str(
                    &hid_key_label(*code)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("0x{code:02X}")),
                );
                s
            }
            KeyAction::Chord(codes) => codes
                .iter()
                .map(|c| {
                    hid_key_label(*c)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("0x{c:02X}"))
                })
                .collect::<Vec<_>>()
                .join("+"),
            KeyAction::Text(t) => {
                // 过长只展示前 12 字符，避免 DiffBar/Drawer 撑爆
                let mut d: String = t.chars().take(12).collect();
                if t.chars().count() > 12 {
                    d.push('…');
                }
                format!("文本:{d}")
            }
            KeyAction::Function(f) => function_label(f),
        }
    }

    /// 是否"非空"（用于 Diff 计数）
    pub fn is_set(&self) -> bool {
        !matches!(self, KeyAction::None)
    }

    /// 转成固件 `0x06` 单键条目。
    ///
    /// 与固件 `cmd_keymap.cpp` / `KeyNameTable.cpp` 规则对齐：
    /// - `Keyboard` → `normal = "0xNN"`（固件字面量可无损解析，覆盖全部键位）；
    /// - `Combo` → `normal = "Ctrl+Shift+0xNN"`（前缀修饰键段 + 字面量，
    ///   固件 normal 通道逐段 press，即标准组合键）；
    /// - `Chord` → `normal = "0xNN+0xMM"`（多键同按）；
    /// - `Text` → `text` 通道；
    /// - `Function` → `function` 通道原样透传（含 KEY_FUNCTION_ASR）。
    pub fn to_firmware_entry(&self, physical: u8) -> FirmwareKeyEntry {
        let mut e = FirmwareKeyEntry {
            physical,
            ..Default::default()
        };
        match self {
            KeyAction::None => {}
            KeyAction::Keyboard(code) => e.normal = format!("0x{code:02X}"),
            KeyAction::Combo { mods, code } => {
                let mut s = mods_names(*mods).join("+");
                if !s.is_empty() {
                    s.push('+');
                }
                s.push_str(&format!("0x{code:02X}"));
                e.normal = s;
            }
            KeyAction::Chord(codes) => {
                e.normal = codes
                    .iter()
                    .map(|c| format!("0x{c:02X}"))
                    .collect::<Vec<_>>()
                    .join("+");
            }
            KeyAction::Text(t) => {
                // 固件上限 128 字符（kMaxTextLen），App 端先行截断兜底
                e.text = t.chars().take(128).collect();
            }
            KeyAction::Function(f) => e.function = f.trim().to_string(),
        }
        e
    }

    /// 从固件 `0x05` 单键条目还原动作。
    ///
    /// 通道优先级与固件一致：`function` > `text` > `normal` > `macro`。
    /// 无法识别的内容一律落入 `Function(原文)` 原样保留——保证 GET → 应用
    /// → SET 的整表下发**不会静默清掉**设备上 App 不认识的配置。
    pub fn from_firmware_entry(e: &FirmwareKeyEntry) -> KeyAction {
        let f = e.function.trim();
        if !f.is_empty() {
            return KeyAction::Function(f.to_string());
        }
        if !e.text.is_empty() {
            return KeyAction::Text(e.text.clone());
        }
        let n = e.normal.trim();
        if !n.is_empty() {
            return parse_normal_string(n);
        }
        let m = e.macro_.trim();
        if !m.is_empty() {
            // 固件宏通道暂未实现播放（协议文档 §7.1"请勿使用"）；
            // 原样透传到 function 通道仅保留可见性，不会凭空产生输出。
            return KeyAction::Function(m.to_string());
        }
        KeyAction::None
    }
}

/// 解析固件 `normal` 通道字符串（`+` 分段同时按下）。
///
/// 规则（与 `KeyNameTable::resolveKeyName` 对齐）：
/// - 除末段外若全为修饰键名 → `Combo`（末段为键）或全修饰 `Chord`（如 `Ctrl+Shift`）；
/// - 无修饰前缀、单段 → `Keyboard`（修饰键名单独成键也算）；
/// - 无修饰前缀、多段非修饰键 → `Chord`；
/// - 其余无法解析 → `Function(原文)` 原样保留，避免下发时静默丢失。
fn parse_normal_string(n: &str) -> KeyAction {
    let segments: Vec<&str> = n
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        return KeyAction::None;
    }
    // 前缀修饰键段
    let mut mods = 0u8;
    let mut idx = 0usize;
    while idx + 1 < segments.len() {
        match modifier_name_to_mod(segments[idx]) {
            Some(m) => {
                mods |= m;
                idx += 1;
            }
            None => break,
        }
    }
    let rest = &segments[idx..];
    if rest.len() == 1 {
        let seg = rest[0];
        if idx == 0 {
            // 单段：修饰键单独成键（如 "Ctrl"）或普通键
            if let Some(m) = modifier_name_to_mod(seg) {
                return KeyAction::Keyboard(mods_primary_usage(m));
            }
            return match name_to_hid(seg) {
                Some(code) => KeyAction::Keyboard(code),
                None => KeyAction::Function(n.to_string()),
            };
        }
        // 修饰前缀 + 单末段
        if let Some(m) = modifier_name_to_mod(seg) {
            // "Ctrl+Shift" 全修饰：同按两个修饰键
            mods |= m;
            return KeyAction::Chord(
                segments[..idx + 1]
                    .iter()
                    .filter_map(|s| modifier_name_to_mod(s))
                    .map(mods_primary_usage)
                    .collect(),
            );
        }
        return match name_to_hid(seg) {
            Some(code) => KeyAction::Combo { mods, code },
            None => KeyAction::Function(n.to_string()),
        };
    }
    if idx == 0 {
        // 多段非修饰键同按（如 "a+b"）
        let codes: Option<Vec<u16>> = rest.iter().map(|s| name_to_hid(s)).collect();
        return match codes {
            Some(codes) if !codes.is_empty() => KeyAction::Chord(codes),
            _ => KeyAction::Function(n.to_string()),
        };
    }
    // 修饰前缀 + 多个非修饰段：固件语义未定义，整串透传
    KeyAction::Function(n.to_string())
}

/// 固件功能串 → 友好展示名；未识别原样返回。
fn function_label(f: &str) -> String {
    match f {
        "KEY_FUNCTION_ASR" => "语音识别 (ASR)".into(),
        "MEDIA_PLAY" | "MEDIA_PLAY_PAUSE" | "MEDIA_PlayPause" => "媒体: 播放/暂停".into(),
        "MEDIA_NEXT" | "MEDIA_Next" => "媒体: 下一曲".into(),
        "MEDIA_PREV" | "MEDIA_Prev" => "媒体: 上一曲".into(),
        "MEDIA_VOLUME_UP" | "MEDIA_VolUp" => "媒体: 音量+".into(),
        "MEDIA_VOLUME_DOWN" | "MEDIA_VolDown" => "媒体: 音量-".into(),
        "MEDIA_MUTE" | "MEDIA_Mute" => "媒体: 静音".into(),
        "MOUSE_LEFT" | "MOUSE_LeftClick" => "鼠标: 左键".into(),
        "MOUSE_RIGHT" | "MOUSE_RightClick" => "鼠标: 右键".into(),
        "MOUSE_MIDDLE" | "MOUSE_MiddleClick" => "鼠标: 中键".into(),
        "MOUSE_SCROLL_UP" | "MOUSE_ScrollUp" => "鼠标: 滚轮上".into(),
        "MOUSE_SCROLL_DOWN" | "MOUSE_ScrollDown" => "鼠标: 滚轮下".into(),
        other => other.to_string(),
    }
}

/// HID Usage ID → 展示用键名（覆盖 App 编辑器提供的全部键位）。
///
/// 仅用于 UI 展示；下发编码始终用 `0xNN` 字面量（固件可无损解析），
/// 与固件 KeyNameTable 的具名键子集（Enter/Backspace/Space）无关。
pub fn hid_key_label(code: u16) -> Option<&'static str> {
    HID_KEY_CHOICES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| *n)
}

/// App 编辑器可选键位表（(HID Usage ID, 展示名)）。
pub const HID_KEY_CHOICES: &[(u16, &str)] = &[
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

/// HID Usage ID → 键名字符串（与固件 `KeyNameTable` 支持的子集对齐）。
///
/// 覆盖 a-z / 0-9 / Enter / Backspace / Space。未知 code 返回 `None`。
pub fn hid_to_name(code: u16) -> Option<String> {
    match code {
        0x04..=0x1D => Some(((b'a' + (code - 0x04) as u8) as char).to_string()),
        0x1E..=0x26 => Some(((b'1' + (code - 0x1E) as u8) as char).to_string()),
        0x27 => Some("0".to_string()),
        0x28 => Some("Enter".to_string()),
        0x2A => Some("Backspace".to_string()),
        0x2C => Some("Space".to_string()),
        _ => None,
    }
}

/// 键名字符串 → HID Usage ID（与固件 `KeyNameTable` 支持的子集对齐）。
///
/// 支持 a-z / 0-9 / Enter / Backspace / Space，以及固件字面量 `0xNN`。
/// 固件区分大小写（"A" ≠ "a"），App 统一用**小写**与固件默认映射一致。
pub fn name_to_hid(name: &str) -> Option<u16> {
    let n = name.trim();
    if let Some(hex) = n.strip_prefix("0x").or_else(|| n.strip_prefix("0X")) {
        return u16::from_str_radix(hex, 16).ok();
    }
    let mut it = n.chars();
    let c = it.next()?;
    if it.next().is_some() {
        // 单字符键名之外的具名键
        return match n {
            "Enter" => Some(0x28),
            "Backspace" => Some(0x2A),
            "Space" => Some(0x2C),
            _ => None,
        };
    }
    match c {
        'a'..='z' => Some(0x04 + (c as u16) - ('a' as u16)),
        'A'..='Z' => Some(0x04 + (c as u16) - ('A' as u16)),
        '1'..='9' => Some(0x1E + (c as u16) - ('1' as u16)),
        '0' => Some(0x27),
        _ => None,
    }
}

/// 物理槽位类型：普通按键还是旋钮（编码器）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SlotKind {
    #[default]
    Key,
    /// 旋钮（编码器）：渲染时画圆盘；动作额外有 顺时针/逆时针/按下 三种。
    Encoder,
}

/// 物理槽位：键盘上某个 (row, col) 坐标对应的键。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeySlot {
    pub row: u8,
    pub col: u8,
    /// 显示在键帽上的标签（如 "A"、"F1"、"Space"）
    pub label: String,
    /// 1u = 1, 1.25u, 1.5u, 1.75u, 2u ... 用于渲染时决定宽度
    pub width_units: f32,
    /// 槽位种类（按键 / 旋钮）
    #[serde(default)]
    pub kind: SlotKind,
}

/// 一层（Base / Fn / Media / Custom 等）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyLayer {
    pub index: u8,
    pub name: String,
    pub slots: Vec<KeySlot>,
}

/// 一个 Profile（一般有 8 个）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeymapProfile {
    pub index: u8,
    pub name: String,
    pub icon_set: bool,
    pub layers: Vec<KeyLayer>,
    /// 绑定表：key=(layer_index, slot_row, slot_col) → 动作
    pub bindings: std::collections::HashMap<KeyRef, KeyAction>,
}

/// 引用某个具体槽位的三元组
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyRef {
    pub layer: u8,
    pub row: u8,
    pub col: u8,
}

/// 整把键盘的键映射数据
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeymapData {
    pub active_profile: u8,
    pub profiles: Vec<KeymapProfile>,
}

impl Default for KeymapData {
    fn default() -> Self {
        Self::demo_60()
    }
}

impl KeymapData {
    /// 计算两个 KeymapData 的差异（仅 bindings 表层级；profile 结构变化暂不考虑）。
    pub fn diff_bindings(&self, other: &KeymapData) -> Vec<KeymapDiffEntry> {
        let mut out = Vec::new();
        let active = self.active_profile;
        let other_active = other.active_profile;

        // 1. active_profile 切换
        if active != other_active {
            out.push(KeymapDiffEntry::ActiveProfile(active));
        }

        // 2. 当前 profile 的 bindings 差异
        let Some(profile) = self.profile(active) else {
            return out;
        };
        let Some(other_profile) = other.profile(other_active) else {
            return out;
        };

        for (k, v) in &profile.bindings {
            let prev = other_profile.bindings.get(k);
            if prev != Some(v) {
                out.push(KeymapDiffEntry::Binding {
                    key: *k,
                    from: prev.cloned().unwrap_or(KeyAction::None),
                    to: v.clone(),
                });
            }
        }
        // 删除：在 self 中缺失而 other 中存在 → 当作 None
        for (k, v) in &other_profile.bindings {
            if !profile.bindings.contains_key(k) {
                out.push(KeymapDiffEntry::Binding {
                    key: *k,
                    from: v.clone(),
                    to: KeyAction::None,
                });
            }
        }
        out
    }

    /// 取得当前 profile（克隆）
    pub fn profile(&self, idx: u8) -> Option<&KeymapProfile> {
        self.profiles.iter().find(|p| p.index == idx)
    }

    /// 应用一个 diff 列表（合并到自身）；返回是否有变化
    pub fn apply_diff(&mut self, diff: &[KeymapDiffEntry]) -> bool {
        let mut changed = false;
        for e in diff {
            match e {
                KeymapDiffEntry::ActiveProfile(idx) => {
                    if self.active_profile != *idx {
                        self.active_profile = *idx;
                        changed = true;
                    }
                }
                KeymapDiffEntry::Binding { key, to, .. } => {
                    let Some(p) = self.profile_mut(self.active_profile) else {
                        continue;
                    };
                    let new_val = if to.is_set() { Some(to.clone()) } else { None };
                    let prev = p.bindings.get(key);
                    if prev != new_val.as_ref() {
                        if new_val.is_some() {
                            p.bindings.insert(*key, to.clone());
                        } else {
                            p.bindings.remove(key);
                        }
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    pub fn profile_mut(&mut self, idx: u8) -> Option<&mut KeymapProfile> {
        self.profiles.iter_mut().find(|p| p.index == idx)
    }

    /// 与 Settings::merge_push 对齐：草稿优先，未修改字段用新值刷新。
    pub fn merge_push(
        new_snapshot: &KeymapData,
        old_snapshot: &KeymapData,
        draft: &mut KeymapData,
    ) {
        // 1. active_profile：草稿与旧一致才用新值
        if draft.active_profile == old_snapshot.active_profile {
            draft.active_profile = new_snapshot.active_profile;
        }
        // 2. bindings：仅当 profile 结构存在时合并。
        //    先把所有需要"未改"→ 用推送值替换的项收集出来，再统一写入，
        //    避免在 `iter_mut` 中再次借用 `draft`。
        let mut to_overwrite: Vec<(
            u8,
            std::collections::HashMap<crate::protocol::KeyRef, KeyAction>,
        )> = Vec::new();
        for profile in draft.profiles.iter() {
            let Some(old_p) = old_snapshot.profile(profile.index) else {
                continue;
            };
            let Some(new_p) = new_snapshot.profile(profile.index) else {
                continue;
            };
            let mut overlay = std::collections::HashMap::new();
            for (k, v) in &new_p.bindings {
                let prev_in_old = old_p.bindings.get(k);
                let prev_in_draft = profile.bindings.get(k);
                // 草稿与旧一致（包含"两边都没有"）→ 用推送值
                let draft_unmodified = match (prev_in_old, prev_in_draft) {
                    (Some(o), Some(d)) => o == d,
                    (None, None) => true,
                    _ => false,
                };
                if draft_unmodified {
                    overlay.insert(*k, v.clone());
                }
            }
            if !overlay.is_empty() {
                to_overwrite.push((profile.index, overlay));
            }
        }
        for (idx, overlay) in to_overwrite {
            if let Some(p) = draft.profile_mut(idx) {
                for (k, v) in overlay {
                    p.bindings.insert(k, v);
                }
            }
        }
    }
}

/// 单条绑定变更描述（用于 DiffPreviewBar 列表展示 / 协议 SET 增量下发）
#[derive(Debug, Clone, PartialEq)]
pub enum KeymapDiffEntry {
    /// 切换活动 Profile
    ActiveProfile(u8),
    /// 某个槽位的绑定变更
    Binding {
        key: KeyRef,
        from: KeyAction,
        to: KeyAction,
    },
}

// ---------- Keymap 演示数据（4 行 × 3 列小键盘） ----------
//
// 阶段 05 之前 KeymapData 默认填 8 份静态 4×3 布局，保证 UI 有内容可显示。
// 等 CMD_KEYMAP_GET 落地后会被设备真实数据覆盖。
impl KeymapData {
    /// 把当前 active profile 的键映射转成固件 `0x06` 的 11 键数组。
    ///
    /// 映射规则（与固件 `cmd_keymap.cpp` / `MatrixScanner.h` 对齐）：
    /// - 只取 **layer 0（Base）** 的槽位（固件每 Profile 只有 11 个物理键）；
    /// - 跳过旋钮槽（`SlotKind::Encoder`，固件不支持）；
    /// - 剩余按键按 `(row, col)` 升序编号为 `physical` 1~11（与 App 4×3
    ///   布局和固件 `kMatrixKeyCount = 11` 一致）。
    pub fn to_firmware_entries(&self) -> Vec<FirmwareKeyEntry> {
        let Some(profile) = self.profile(self.active_profile) else {
            return Vec::new();
        };
        let Some(base) = profile.layers.iter().find(|l| l.index == 0) else {
            return Vec::new();
        };
        let mut slots: Vec<&KeySlot> = base
            .slots
            .iter()
            .filter(|s| s.kind == SlotKind::Key)
            .collect();
        slots.sort_by_key(|s| (s.row, s.col));
        slots
            .into_iter()
            .take(11)
            .enumerate()
            .map(|(i, s)| {
                let action = profile
                    .bindings
                    .get(&KeyRef {
                        layer: 0,
                        row: s.row,
                        col: s.col,
                    })
                    .cloned()
                    .unwrap_or(KeyAction::None);
                action.to_firmware_entry(i as u8 + 1)
            })
            .collect()
    }

    /// 把固件 `0x05` 返回的 11 键写回当前 active profile 的 layer 0。
    ///
    /// 槽位顺序与 `to_firmware_entries` 相同（按键按 `(row, col)` 升序），
    /// `entries` 下标 i ↔ physical i+1。返回是否有变化。
    pub fn apply_firmware_entries(&mut self, entries: &[FirmwareKeyEntry]) -> bool {
        let mut changed = false;
        let Some(profile) = self.profile_mut(self.active_profile) else {
            return false;
        };
        let Some(base) = profile.layers.iter().find(|l| l.index == 0) else {
            return false;
        };
        let mut slots: Vec<(u8, u8)> = base
            .slots
            .iter()
            .filter(|s| s.kind == SlotKind::Key)
            .map(|s| (s.row, s.col))
            .collect();
        slots.sort();
        for (i, e) in entries.iter().enumerate() {
            let Some(&(row, col)) = slots.get(i) else {
                break;
            };
            let key = KeyRef { layer: 0, row, col };
            let action = KeyAction::from_firmware_entry(e);
            let is_set = action.is_set();
            let prev = profile.bindings.get(&key).cloned();
            match (&prev, is_set) {
                (None, false) => continue,
                (Some(p), true) if *p == action => continue,
                _ => {}
            }
            if is_set {
                profile.bindings.insert(key, action);
            } else {
                profile.bindings.remove(&key);
            }
            changed = true;
        }
        changed
    }

    pub fn demo_60() -> Self {
        let mut profiles = Vec::with_capacity(8);
        for i in 0..8u8 {
            profiles.push(Self::make_demo_profile(i, format!("P{i}")));
        }
        Self {
            active_profile: 0,
            profiles,
        }
    }

    fn make_demo_profile(idx: u8, name: String) -> KeymapProfile {
        let base = KeyLayer {
            index: 0,
            name: "Base".into(),
            slots: demo_base_4x3(),
        };
        let fn_layer = KeyLayer {
            index: 1,
            name: "Fn".into(),
            slots: base.slots.clone(),
        };
        let media = KeyLayer {
            index: 2,
            name: "Media".into(),
            slots: vec![KeySlot {
                row: 0,
                col: 0,
                label: "Play".into(),
                width_units: 1.0,
                kind: SlotKind::Key,
            }],
        };
        let custom = KeyLayer {
            index: 3,
            name: "Custom".into(),
            slots: vec![],
        };

        // 给字母/数字键塞默认 Keyboard 绑定，演示用
        let mut bindings = std::collections::HashMap::new();
        for s in &base.slots {
            let c = s.label.chars().next().unwrap_or('?');
            if c.is_ascii_alphabetic() || c.is_ascii_digit() {
                let k = hid_kbd_from_char(c);
                bindings.insert(
                    KeyRef {
                        layer: 0,
                        row: s.row,
                        col: s.col,
                    },
                    KeyAction::Keyboard(k),
                );
            }
        }

        KeymapProfile {
            index: idx,
            name,
            icon_set: false,
            layers: vec![base, fn_layer, media, custom],
            bindings,
        }
    }
}

fn hid_kbd_from_char(c: char) -> u16 {
    // HID Usage IDs (Keyboard/Keypad Page 0x07) 的子集
    match c {
        'A'..='Z' => 0x04 + (c as u16) - ('A' as u16),
        'a'..='z' => 0x04 + (c as u16) - ('a' as u16),
        '1'..='9' => 0x1E + (c as u16) - ('1' as u16),
        '0' => 0x27,
        _ => 0,
    }
}

/// 3 行 × 4 列小键盘基础层的槽位定义（不含绑定）。仅用于 demo。
///
/// 3 行 × 4 列；row 0 第 4 个槽位是旋钮，其余 11 个是普通键：
///   row 0: K1 / K2 / K3 / [KNOB]   （col 3, row 0 → 旋钮，第一行第四个）
///   row 1: K4 / K5 / K6 / K7
///   row 2: K8 / K9 / K10/ K11
///
/// 旋钮用 `width_units = 1.0` + `kind = Encoder` 标识；宽度与按键一致，
/// 渲染层判断 `kind` 后画一个圆形刻度盘代替方键。
fn demo_base_4x3() -> Vec<KeySlot> {
    let mut out = Vec::with_capacity(12);
    let normal: [(&str, u8, u8); 11] = [
        ("K1", 0, 0),
        ("K2", 0, 1),
        ("K3", 0, 2),
        ("K4", 1, 0),
        ("K5", 1, 1),
        ("K6", 1, 2),
        ("K7", 1, 3),
        ("K8", 2, 0),
        ("K9", 2, 1),
        ("K10", 2, 2),
        ("K11", 2, 3),
    ];
    for (label, row, col) in normal.iter() {
        out.push(KeySlot {
            row: *row,
            col: *col,
            label: (*label).to_string(),
            width_units: 1.0,
            kind: SlotKind::Key,
        });
    }
    out.push(KeySlot {
        row: 0,
        col: 3,
        label: "KNOB".into(),
        width_units: 1.0,
        kind: SlotKind::Encoder,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把 `Result` / `Option` 在测试里 unwrap 时附带上下文标签，
    /// 避免 panic 信息只显示 thread 'X' panicked at ... 而看不到是哪个字段。
    trait TestUnwrap<T> {
        fn to(self, ctx: &str) -> T;
    }
    impl<T, E: std::fmt::Debug> TestUnwrap<T> for Result<T, E> {
        fn to(self, ctx: &str) -> T {
            self.unwrap_or_else(|e| panic!("[{ctx}] {e:?}"))
        }
    }
    impl<T> TestUnwrap<T> for Option<T> {
        fn to(self, ctx: &str) -> T {
            self.unwrap_or_else(|| panic!("[{ctx}] None"))
        }
    }

    #[test]
    fn frame_roundtrip() {
        let f = Frame::request(CMD_CONFIG_GET, 42, None);
        let s = f.encode_line().unwrap();
        assert!(s.ends_with('\n'));
        let parsed = try_parse_line(&s).unwrap().unwrap();
        assert_eq!(parsed.cmd, CMD_CONFIG_GET);
        assert_eq!(parsed.seq, 42);
    }

    #[test]
    fn status_only_for_response_frames() {
        // 请求帧：status() 必须为 None（即使底层字段被误设）
        let mut req = Frame::request(CMD_CONFIG_GET, 1, None);
        req.status = Some(0);
        assert_eq!(req.status(), None);

        // 标准响应帧：返回 status
        let mut resp = Frame::request(response_cmd(CMD_CONFIG_GET), 1, None);
        resp.status = Some(1);
        assert_eq!(resp.status(), Some(1));

        // 异类响应 0x10：is_response_like 成立，status 可见
        let mut p10 = Frame::request(CMD_PROFILE_STATE, 3, None);
        p10.status = Some(0);
        assert_eq!(p10.status(), Some(0));
        assert!(p10.is_response_like());
    }

    #[test]
    fn non_json_line_is_none() {
        let line = "[I]MAIN: hello";
        assert!(try_parse_line(line).is_none());
    }

    /// 字段常量必须从 0 连续递增、无空洞，且数量 == FIELD_COUNT。
    #[test]
    fn field_constants_are_contiguous() {
        let all: [u8; FIELD_COUNT] = [
            F_WIFI_SWITCH,
            F_CONNECT_HOST,
            F_WIFI_SSID,
            F_WIFI_PASSWORD,
            F_WORK_MODE,
            F_RGB_MODE,
            F_RGB_SINGLE_COLOR,
            F_RGB_CLICK_MODE,
            F_RGB_BRIGHTNESS,
            F_TFT_THEME,
            F_TFT_BRIGHTNESS,
            F_DEVICE_VOLUME,
            F_AUDIO_ENABLE,
            F_POWER_MODE,
            F_VOICE_ENABLE,
            F_VOICE_TRIGGER_KEY,
            F_VOICE_MAX_RECORD_MS,
            F_VOICE_AUTO_ENTER,
            F_VOICE_DEV_PID,
            F_VOICE_CUID,
            F_VOICE_BAIDU_API_KEY,
            F_VOICE_BAIDU_SECRET_KEY,
            F_PC_STATUS_MASK,
            F_ACTIVE_KEYMAP_PROFILE,
            F_ACTIVE_PROFILE_NAME,
            F_ACTIVE_PROFILE_HAS_CUSTOM_ICON,
        ];
        for (i, &c) in all.iter().enumerate() {
            assert_eq!(i as u8, c, "F_* 常量必须从 0 连续递增，第 {i} 个不匹配");
        }
    }

    /// 覆盖测试：所有字段都不同的两个快照 → diff 的 mask 必须置满 FIELD_COUNT 位，
    /// 且 apply(diff, mask) 能完整还原目标快照。
    ///
    /// 若有人给 `DeviceSettings` 加了字段但忘了加进 `diff()` 或 `apply()` 的
    /// 宏清单，此测试会在 "count_ones" 或 "applied == b" 处失败。
    #[test]
    fn diff_and_apply_cover_all_fields() {
        let a = DeviceSettings::default();
        let b = DeviceSettings {
            wifi_switch: 1,
            connect_host: 1,
            wifi_ssid: "ssid".into(),
            wifi_password: "pw".into(),
            work_mode: 1,
            rgb_mode: 1,
            rgb_single_color: 1,
            rgb_click_mode: 1,
            rgb_brightness: 50,
            tft_theme: 1,
            tft_brightness: 60,
            device_volume: 70,
            audio_enable: 1,
            power_mode: 1,
            voice_enable: 1,
            voice_trigger_key: 1,
            voice_max_record_ms: 1000,
            voice_auto_enter: 1,
            voice_dev_pid: 1,
            voice_cuid: "cuid".into(),
            voice_baidu_api_key: "ak".into(),
            voice_baidu_secret_key: "sk".into(),
            pc_status_mask: 1,
            active_keymap_profile: 1,
            active_profile_name: "P1".into(),
            active_profile_has_custom_icon: true,
        };
        let (d, m) = b.diff(&a);
        assert_eq!(
            m.bits().count_ones() as usize,
            FIELD_COUNT,
            "diff 必须覆盖全部 {FIELD_COUNT} 个字段（当前 {} 个）；检查 diff() 的 cmp! 清单",
            m.bits().count_ones()
        );
        let mut applied = a.clone();
        applied.apply(&d, m);
        assert_eq!(
            applied, b,
            "apply() 必须覆盖全部字段；检查 apply() 的 apply_field! 清单"
        );
    }

    /// 覆盖测试：merge_push 在"草稿未改动"时应把每个字段都刷成新值。
    /// 若 merge_field! 清单缺字段，该字段会保持旧值 → 测试失败。
    #[test]
    fn merge_push_covers_all_fields() {
        let old = DeviceSettings::default();
        let new = DeviceSettings {
            wifi_switch: 1,
            connect_host: 1,
            wifi_ssid: "ssid".into(),
            wifi_password: "pw".into(),
            work_mode: 1,
            rgb_mode: 1,
            rgb_single_color: 1,
            rgb_click_mode: 1,
            rgb_brightness: 50,
            tft_theme: 1,
            tft_brightness: 60,
            device_volume: 70,
            audio_enable: 1,
            power_mode: 1,
            voice_enable: 1,
            voice_trigger_key: 1,
            voice_max_record_ms: 1000,
            voice_auto_enter: 1,
            voice_dev_pid: 1,
            voice_cuid: "cuid".into(),
            voice_baidu_api_key: "ak".into(),
            voice_baidu_secret_key: "sk".into(),
            pc_status_mask: 1,
            active_keymap_profile: 1,
            active_profile_name: "P1".into(),
            active_profile_has_custom_icon: true,
        };
        // 草稿 == 旧快照（用户未改动任何字段）→ 推送后草稿应完全变为 new
        let mut draft = old.clone();
        DeviceSettings::merge_push(&new, &old, FieldMask::all(), FieldMask::all(), &mut draft);
        assert_eq!(
            draft, new,
            "merge_push 必须覆盖全部字段；检查 merge_field! 清单"
        );
    }

    #[test]
    fn mask_sensitive_blankets_secrets() {
        let mut s = DeviceSettings {
            wifi_password: "real-password".into(),
            voice_baidu_api_key: "ak-123".into(),
            voice_baidu_secret_key: "sk-456".into(),
            ..Default::default()
        };
        s.mask_sensitive();
        assert_eq!(s.wifi_password, "***");
        assert_eq!(s.voice_baidu_api_key, "***");
        assert_eq!(s.voice_baidu_secret_key, "***");
        // 非敏感字段不受影响
        assert_eq!(s.wifi_ssid, "");
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
    fn clamp_all_rules() {
        let mut s = DeviceSettings::default();
        // 覆盖所有钳位字段的越界输入
        s.work_mode = 9; // → 0
        s.active_keymap_profile = 10; // → 0
        s.wifi_switch = 7; // → 1
        s.connect_host = -1; // → 1（非 0 归一化为 1）
        s.voice_enable = 3; // → 1
        s.voice_auto_enter = -2; // → 1
        s.voice_trigger_key = 99; // → 11
        s.voice_max_record_ms = 50; // → 1000
        s.voice_dev_pid = -1; // → 0
        // RGB 钳位（与固件 RGBLightControl.h:22 / ClickHighlight.h:34 对齐）
        s.rgb_mode = 99; // → 7
        s.rgb_single_color = -10; // → 0
        s.rgb_single_color = 1000; // → 23（再赋一次，覆盖前面）
        s.rgb_click_mode = 42; // → 2
        s.rgb_brightness = -5; // → 0
        s.rgb_brightness = 999; // → 100（再赋一次，覆盖前面）
        s.wifi_ssid = "x".repeat(64); // → 32 字节
        s.wifi_password = "p".repeat(128); // → 64 字节
        s.voice_baidu_api_key = "k".repeat(100); // → 64 字节
        assert!(s.clamp());

        assert_eq!(s.work_mode, 0);
        assert_eq!(s.active_keymap_profile, 0);
        assert_eq!(s.wifi_switch, 1);
        assert_eq!(s.connect_host, 1);
        assert_eq!(s.voice_enable, 1);
        assert_eq!(s.voice_auto_enter, 1);
        assert_eq!(s.voice_trigger_key, 11);
        assert_eq!(s.voice_max_record_ms, 1000);
        assert_eq!(s.voice_dev_pid, 0);
        assert_eq!(s.rgb_mode, 7);
        assert_eq!(s.rgb_single_color, 23);
        assert_eq!(s.rgb_click_mode, 2);
        assert_eq!(s.rgb_brightness, 100);
        assert_eq!(s.wifi_ssid.len(), 32);
        assert_eq!(s.wifi_password.len(), 64);
        assert_eq!(s.voice_baidu_api_key.len(), 64);

        // 二次 clamp 无变化
        assert!(!s.clamp());
    }

    #[test]
    fn clamp_truncate_keeps_utf8_boundary() {
        // 3 字节的中文字符 + 4 字节 emoji，截断不能切断字符
        let mut s = DeviceSettings::default();
        s.wifi_ssid = "中".repeat(20); // 60 字节，超 32
        assert!(s.clamp());
        // 截断后的字节数 ≤ 32 且落在字符边界
        assert!(s.wifi_ssid.len() <= 32);
        assert!(s.wifi_ssid.is_char_boundary(s.wifi_ssid.len()));

        // emoji 是 4 字节
        let mut s = DeviceSettings::default();
        s.wifi_password = "🦀".repeat(30); // 120 字节
        assert!(s.clamp());
        assert!(s.wifi_password.len() <= 64);
        assert!(s.wifi_password.is_char_boundary(s.wifi_password.len()));
    }

    #[test]
    fn diff_only_changed() {
        let a = DeviceSettings::default();
        let mut b = a.clone();
        b.tft_brightness = 80;
        b.work_mode = 1;
        let (d, m) = b.diff(&a);
        assert_eq!(d.tft_brightness, 80);
        assert_eq!(d.work_mode, 1);
        assert_eq!(d.tft_theme, 0); // 未变化 → default
        assert!(m.test(F_TFT_BRIGHTNESS));
        assert!(m.test(F_WORK_MODE));
        assert!(!m.test(F_TFT_THEME));
        assert_eq!(m.bits().count_ones(), 2);
    }

    #[test]
    fn is_any_diff_round_trip() {
        // 默认 diff mask 应为空
        let (_, m) = DeviceSettings::default().diff(&DeviceSettings::default());
        assert!(m.is_empty());
        // 改一个标量字段
        let mut b = DeviceSettings::default();
        b.tft_brightness = 80;
        let (_, m) = b.diff(&DeviceSettings::default());
        assert!(!m.is_empty());
        // 改一个 String 字段
        let mut b = DeviceSettings::default();
        b.wifi_ssid = "home".into();
        let (_, m) = b.diff(&DeviceSettings::default());
        assert!(!m.is_empty());
        // 改一个 bool 字段
        let mut b = DeviceSettings::default();
        b.active_profile_has_custom_icon = true;
        let (_, m) = b.diff(&DeviceSettings::default());
        assert!(!m.is_empty());
    }

    /// 回归测试：把字段显式设为合法 0 / 空串，必须能进 diff 而不是被吞。
    /// 之前的 `is_any_diff` 用 `field != 0` / `!is_empty()` 判"有变化"，
    /// 会把这种合法值判为"无变化"，导致固件收不到更新。
    #[test]
    fn diff_keeps_legitimate_zero_and_empty() {
        // 场景 A：把 work_mode 从 1 改回 0（合法值 USB）
        let mut old = DeviceSettings::default();
        old.work_mode = 1;
        let new = DeviceSettings::default(); // work_mode = 0
        let (_, m) = new.diff(&old);
        assert!(m.test(F_WORK_MODE), "work_mode: 1→0 必须出现在 diff 中");

        // 场景 B：清空 SSID（合法操作）
        let mut old = DeviceSettings::default();
        old.wifi_ssid = "home".into();
        let new = DeviceSettings::default(); // wifi_ssid = ""
        let (_, m) = new.diff(&old);
        assert!(m.test(F_WIFI_SSID), "空 SSID 必须出现在 diff 中");

        // 场景 C：关闭 wifi_switch
        let mut old = DeviceSettings::default();
        old.wifi_switch = 1;
        let new = DeviceSettings::default();
        let (_, m) = new.diff(&old);
        assert!(m.test(F_WIFI_SWITCH), "wifi_switch: 1→0 必须出现在 diff 中");

        // 场景 D：active_profile_has_custom_icon: false → true
        let old = DeviceSettings::default();
        let mut new = DeviceSettings::default();
        new.active_profile_has_custom_icon = true;
        let (_, m) = new.diff(&old);
        assert!(m.test(F_ACTIVE_PROFILE_HAS_CUSTOM_ICON));
    }

    /// `apply()` 只覆盖 mask 置位的字段，不应污染其它字段。
    #[test]
    fn apply_respects_mask() {
        let mut base = DeviceSettings::default();
        base.tft_brightness = 50;
        base.work_mode = 1;

        // 构造一个 diff：只标 tft_brightness
        let mut diff = DeviceSettings::default();
        diff.tft_brightness = 80;
        let mask = FieldMask::empty().set(F_TFT_BRIGHTNESS);

        base.apply(&diff, mask);

        assert_eq!(base.tft_brightness, 80); // 被覆盖
        assert_eq!(base.work_mode, 1, "mask 外字段不应被覆盖");
    }

    /// merge_push：草稿改过 → 保留草稿；草稿未改 → 用推送值。
    #[test]
    fn merge_push_draft_priority() {
        let mut old = DeviceSettings::default();
        old.work_mode = 1;
        let mut new = DeviceSettings::default();
        new.work_mode = 2;
        let mut draft = DeviceSettings::default();
        draft.work_mode = 5; // 用户已改成 5（与旧不同 → 保留）

        DeviceSettings::merge_push(&new, &old, FieldMask::all(), FieldMask::all(), &mut draft);
        assert_eq!(draft.work_mode, 5);

        // 草稿与旧相同 → 用推送值
        let mut draft2 = old.clone();
        DeviceSettings::merge_push(&new, &old, FieldMask::all(), FieldMask::all(), &mut draft2);
        assert_eq!(draft2.work_mode, 2);
    }

    /// FieldMask：位操作基本正确性。
    #[test]
    fn field_mask_basics() {
        let m = FieldMask::empty().set(F_WIFI_SSID).set(F_WORK_MODE);
        assert!(m.test(F_WIFI_SSID));
        assert!(m.test(F_WORK_MODE));
        assert!(!m.test(F_TFT_BRIGHTNESS));
        assert_eq!(m.bits().count_ones(), 2);

        let all = FieldMask::all();
        assert!(!all.is_empty());
        // all() 应覆盖全部 FIELD_COUNT 位
        assert_eq!(all.bits().count_ones() as usize, FIELD_COUNT);
    }

    /// ConfigSetPayload 序列化：仅含 `config`，不含 `mask`（固件侧按字段存在判断）。
    #[test]
    fn config_set_payload_serializes() {
        let mut diff = DeviceSettings::default();
        diff.work_mode = 0; // 关键场景：合法 0 必须保留
        diff.tft_brightness = 80;
        let payload = ConfigSetPayload { config: &diff };
        let v = serde_json::to_value(&payload).unwrap();
        assert!(v.get("mask").is_none(), "固件按字段存在判断增量，不读 mask");
        assert_eq!(v["config"]["work_mode"], 0);
        assert_eq!(v["config"]["tft_brightness"], 80);
    }

    // ---- 新增命令类型的序列化往返 ----

    #[test]
    fn conf_version_roundtrip() {
        let v = serde_json::to_value(ConfVersionSetReq { version: 1 }).to("req");
        assert_eq!(v["version"], 1);
        let resp: ConfVersionResp =
            serde_json::from_value(serde_json::json!({ "version": 1 })).to("resp");
        assert_eq!(resp.version, 1);
    }

    #[test]
    fn device_info_roundtrip() {
        let info = DeviceInfo {
            device_name: "EKeys".into(),
            device_id: "AABBCCDDEEFF".into(),
            firmware_version: "0.6.0".into(),
        };
        let v = serde_json::to_value(&info).to("info");
        assert_eq!(v["device_id"], "AABBCCDDEEFF");
        let back: DeviceInfo = serde_json::from_value(v).to("back");
        assert_eq!(back, info);
    }

    #[test]
    fn device_info_set_omits_none() {
        // 只设 device_name 时，serial 不应出现在 JSON 里（固件按字段存在判断）。
        let req = DeviceInfoSetReq {
            device_name: Some("X".into()),
            serial: None,
        };
        let v = serde_json::to_value(&req).to("v");
        assert!(v.get("device_name").is_some());
        assert!(v.get("serial").is_none());
    }

    #[test]
    fn firmware_key_entry_uses_macro_field() {
        // 固件字段名是 macro（Rust 侧 macro_ 经 serde rename 对齐）；
        // 这里只校验序列化稳定、不报 panic。
        let e = FirmwareKeyEntry {
            physical: 1,
            normal: "a".into(),
            macro_: "Ctrl+c".into(),
            text: String::new(),
            function: String::new(),
        };
        let s = serde_json::to_string(&e).to("s");
        assert!(s.contains("physical"));
        assert!(s.contains("\"macro\""), "序列化字段名必须是 macro");
    }

    #[test]
    fn firmware_info_roundtrip() {
        let info = FirmwareInfo {
            version: "0.6.0".into(),
            device: "EKeys".into(),
            build_date: "Sep  7 2026".into(),
            build_time: "12:00:00".into(),
        };
        let v = serde_json::to_value(&info).to("info");
        let back: FirmwareInfo = serde_json::from_value(v).to("back");
        assert_eq!(back, info);
    }

    #[test]
    fn profile_state_top_level() {
        // 模拟固件实际响应：cmd=0x10, profile_state 在顶层。
        let raw = r#"{"cmd":16,"seq":3,"profile_state":{"active_profile":0,"profile_number":1,"profile_name":"Profile 1","has_custom_icon":true,"icon_path":"/icon1.png"}}"#;
        let ps: ProfileState = parse_top_level(raw).to("profile_state");
        assert_eq!(ps.active_profile, 0);
        assert_eq!(ps.profile_number, 1);
        assert!(ps.has_custom_icon);
        assert_eq!(ps.profile_name, "Profile 1");
        // 同时 Frame 自身也能解析（顶层 cmd/seq 在 Frame 里）。
        let f: Frame = serde_json::from_str(raw).to("frame");
        assert_eq!(f.cmd, CMD_PROFILE_STATE);
        assert_eq!(f.seq, 3);
    }

    #[test]
    fn profile_state_direct_body() {
        // 不带 wrapper 的 body 直接 deserialize（便于 App 内部构造）。
        let body = r#"{"active_profile":2,"profile_number":3,"profile_name":"P3","has_custom_icon":false,"icon_path":""}"#;
        let ps: ProfileState = serde_json::from_str(body).to("ps");
        assert_eq!(ps.active_profile, 2);
        assert_eq!(ps.profile_number, 3);
    }

    #[test]
    fn music_control_top_level() {
        let raw = r#"{"cmd":15,"seq":0,"music_control":{"action":"toggle"}}"#;
        let mc: MusicControl = parse_top_level(raw).to("mc");
        assert_eq!(mc.action, MusicControlAction::Toggle);
    }

    #[test]
    fn profile_icon_set_clear_skips_base64() {
        let req = ProfileIconSetReq {
            profile: 2,
            clear: true,
            png_base64: String::new(),
        };
        let v = serde_json::to_value(&req).to("v");
        assert_eq!(v["clear"], true);
        assert!(v.get("png_base64").is_none(), "clear=true 时不应发 base64");
    }

    /// 0x11 请求 body 必须包在 `profile_icon` 字段里（固件 §7.3）。
    #[test]
    fn profile_icon_payload_wraps_profile_icon() {
        let req = ProfileIconSetPayload {
            profile_icon: ProfileIconSetReq {
                profile: 3,
                clear: false,
                png_base64: "aGVsbG8=".into(),
            },
        };
        let v = serde_json::to_value(&req).to("v");
        assert_eq!(v["profile_icon"]["profile"], 3);
        assert_eq!(v["profile_icon"]["png_base64"], "aGVsbG8=");
        assert!(v.get("profile").is_none(), "字段必须包在 profile_icon 内");
    }

    /// 0x85 响应帧：顶层 `keymap` 数组必须被 Frame::extra 保留。
    #[test]
    fn frame_preserves_top_level_keymap() {
        let raw = r#"{"cmd":133,"seq":7,"status":0,"keymap":[{"physical":1,"normal":"a","macro":"","function":""}]}"#;
        let f = try_parse_line(raw).to("parse").to("frame");
        assert_eq!(f.cmd, 133);
        assert_eq!(f.seq, 7);
        // 顶层 keymap 是数组本体（不是 {"keymap": [...]} wrapper）
        let v = f.extra_value("keymap").to("keymap").clone();
        let entries: Vec<FirmwareKeyEntry> = serde_json::from_value(v).to("entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].physical, 1);
        assert_eq!(entries[0].normal, "a");
        // 序列化时 extra 原样并回顶层
        let out = f.encode_line().to("encode");
        assert!(out.contains("\"keymap\""));
        assert!(out.contains("\"normal\":\"a\""));
    }

    /// HID ↔ 键名表往返（含固件 0xNN 字面量）。
    #[test]
    fn hid_name_roundtrip() {
        assert_eq!(hid_to_name(0x04).as_deref(), Some("a"));
        assert_eq!(hid_to_name(0x1D).as_deref(), Some("z"));
        assert_eq!(hid_to_name(0x27).as_deref(), Some("0"));
        assert_eq!(hid_to_name(0x28).as_deref(), Some("Enter"));
        assert_eq!(name_to_hid("a"), Some(0x04));
        assert_eq!(name_to_hid("0"), Some(0x27));
        assert_eq!(name_to_hid("Enter"), Some(0x28));
        assert_eq!(name_to_hid("0x2A"), Some(0x2A));
        assert_eq!(name_to_hid("Backspace"), Some(0x2A));
        assert_eq!(name_to_hid("???"), None);
        // 往返
        for code in [0x04u16, 0x1E, 0x27, 0x28, 0x2A, 0x2C] {
            let name = hid_to_name(code).unwrap();
            assert_eq!(
                name_to_hid(&name),
                Some(code),
                "{name} 应还原为 0x{code:02X}"
            );
        }
    }

    /// KeyAction ↔ 固件条目：单键/组合键/多键同按/文本/功能串 全部无损往返。
    #[test]
    fn firmware_strings_roundtrip() {
        // Keyboard 无损
        let k = KeyAction::Keyboard(0x04);
        let e = k.to_firmware_entry(1);
        assert_eq!(e.normal, "0x04");
        assert_eq!(KeyAction::from_firmware_entry(&e), k);

        // 修饰键单独成键（"Ctrl" → 0xE0）
        let e = FirmwareKeyEntry {
            physical: 1,
            normal: "Ctrl".into(),
            ..Default::default()
        };
        assert_eq!(
            KeyAction::from_firmware_entry(&e),
            KeyAction::Keyboard(0xE0)
        );

        // 组合键：normal "Ctrl+Shift+c" → Combo，往返一致
        let combo = KeyAction::Combo {
            mods: MOD_CTRL | MOD_SHIFT,
            code: 0x06,
        };
        let e = combo.to_firmware_entry(1);
        assert_eq!(e.normal, "Ctrl+Shift+0x06");
        assert_eq!(KeyAction::from_firmware_entry(&e), combo);

        // 多键同按："a+b" → Chord，往返一致
        let e = FirmwareKeyEntry {
            physical: 1,
            normal: "a+b".into(),
            ..Default::default()
        };
        assert_eq!(
            KeyAction::from_firmware_entry(&e),
            KeyAction::Chord(vec![0x04, 0x05])
        );

        // 文本通道往返
        let text = KeyAction::Text("hello@example.com".into());
        let e = text.to_firmware_entry(1);
        assert_eq!(e.text, "hello@example.com");
        assert_eq!(e.normal, "");
        assert_eq!(KeyAction::from_firmware_entry(&e), text);

        // 功能串（ASR）往返
        let asr = KeyAction::Function("KEY_FUNCTION_ASR".into());
        let e = asr.to_firmware_entry(1);
        assert_eq!(e.function, "KEY_FUNCTION_ASR");
        assert_eq!(KeyAction::from_firmware_entry(&e), asr);

        // 未知 function 串原样保留（不丢数据）
        let e = FirmwareKeyEntry {
            physical: 1,
            function: "MEDIA_PLAY".into(),
            ..Default::default()
        };
        assert_eq!(
            KeyAction::from_firmware_entry(&e),
            KeyAction::Function("MEDIA_PLAY".into())
        );

        // macro 通道内容透传保留（固件宏未实现，不静默丢弃）
        let e = FirmwareKeyEntry {
            physical: 1,
            macro_: "a+b".into(),
            ..Default::default()
        };
        assert_eq!(
            KeyAction::from_firmware_entry(&e),
            KeyAction::Function("a+b".into())
        );

        // 空 → None
        let e = FirmwareKeyEntry::default();
        assert_eq!(KeyAction::from_firmware_entry(&e), KeyAction::None);
    }

    /// KeymapData ↔ 固件 11 键往返：demo 布局 11 个按键全部可还原。
    #[test]
    fn keymap_firmware_roundtrip() {
        let mut kd = KeymapData::demo_60();
        // 给几个键设不同动作
        kd.active_profile = 2;
        {
            let p = kd.profile_mut(2).unwrap();
            p.bindings.insert(
                KeyRef {
                    layer: 0,
                    row: 0,
                    col: 0,
                },
                KeyAction::Keyboard(0x04), // K1 → 'a'
            );
            p.bindings.insert(
                KeyRef {
                    layer: 0,
                    row: 2,
                    col: 3,
                },
                KeyAction::Function("KEY_FUNCTION_ASR".into()), // K11 → 语音识别
            );
        }
        let entries = kd.to_firmware_entries();
        assert_eq!(entries.len(), 11, "固件 11 个物理键");
        assert_eq!(entries[0].physical, 1);
        assert_eq!(entries[0].normal, "0x04");
        assert_eq!(entries[9].physical, 10);
        assert_eq!(entries[10].physical, 11);
        assert_eq!(entries[10].function, "KEY_FUNCTION_ASR");
        // 旋钮槽（KNOB，row0 col3）不在 11 键里
        assert!(
            !entries.iter().any(|e| e.normal == "0x00"),
            "旋钮不应被编码"
        );

        // 回写
        let mut kd2 = KeymapData::demo_60();
        kd2.active_profile = 2;
        assert!(kd2.apply_firmware_entries(&entries));
        let p = kd2.profile(2).unwrap();
        assert_eq!(
            p.bindings.get(&KeyRef {
                layer: 0,
                row: 0,
                col: 0
            }),
            Some(&KeyAction::Keyboard(0x04))
        );
        assert_eq!(
            p.bindings.get(&KeyRef {
                layer: 0,
                row: 2,
                col: 3
            }),
            Some(&KeyAction::Function("KEY_FUNCTION_ASR".into()))
        );
    }

    #[test]
    fn voice_text_push_top_level() {
        // 0x0C 的 text/timestamp 在顶层，不在 data。
        let raw = r#"{"cmd":12,"seq":0,"text":"你好","timestamp":123456}"#;
        let vt: VoiceTextPush = parse_top_level(raw).to("vt");
        assert_eq!(vt.text, "你好");
        assert_eq!(vt.timestamp, 123456);
        // Frame 自身也能解析
        let f: Frame = serde_json::from_str(raw).to("frame");
        assert_eq!(f.cmd, CMD_VOICE_TEXT);
        assert_eq!(f.seq, 0);
        // ⚠️ 0x0C 的 cmd=0x0C 最高位为 0，所以 is_push() 不会判定为 push。
        // 固件 → App 的主动消息需要按 cmd 类型识别（详见 protocol-usage.md §3.3）。
        assert!(!f.is_response());
    }

    #[test]
    fn music_control_action_serde_lowercase() {
        let j = serde_json::to_value(MusicControlAction::Toggle).to("j");
        assert_eq!(j, "toggle");
        let j = serde_json::to_value(MusicControlAction::Prev).to("j");
        assert_eq!(j, "prev");
    }

    #[test]
    fn pc_status_omits_none_fields() {
        let s = PcStatus {
            caps_lock: Some(true),
            ..Default::default()
        };
        let v = serde_json::to_value(&s).to("v");
        assert_eq!(v["caps_lock"], true);
        assert!(v.get("cpu_temp_c").is_none());
        assert!(v.get("network_down_kbps").is_none());
    }

    #[test]
    fn heartbeat_resp_roundtrip() {
        let r = HeartbeatResp {
            timestamp: 12345,
            device: "EKeys".into(),
        };
        let v = serde_json::to_value(&r).to("v");
        let back: HeartbeatResp = serde_json::from_value(v).to("back");
        assert_eq!(back, r);
    }

    /// 异类命令识别：0x10 / 0x0c / 0x0f 的 body 在帧顶层。
    #[test]
    fn top_level_cmd_recognition() {
        assert!(is_top_level_cmd(CMD_PROFILE_STATE));
        assert!(is_top_level_cmd(CMD_VOICE_TEXT));
        assert!(is_top_level_cmd(CMD_MUSIC_CONTROL));
        assert!(!is_top_level_cmd(CMD_CONFIG_GET));
        assert!(!is_top_level_cmd(CMD_HEARTBEAT));
        // 0x10 的"响应" cmd 仍是 0x10，需要 is_response_like 才能识别
        assert!(!is_response(CMD_PROFILE_STATE));
        assert!(is_response_like(CMD_PROFILE_STATE));
        // 0x0c / 0x0f 是单向推送（固件→App），不是响应
        assert!(!is_response_like(CMD_VOICE_TEXT));
        assert!(!is_response_like(CMD_MUSIC_CONTROL));
    }

    /// 回归：命令 ID 与 response_cmd 对齐（0x10 例外需另行处理）。
    #[test]
    fn cmd_ids_match_protocol_doc() {
        assert_eq!(CMD_CONF_VERSION_GET, 0x01);
        assert_eq!(CMD_CONF_VERSION_SET, 0x02);
        assert_eq!(CMD_DEVICE_INFO_GET, 0x03);
        assert_eq!(CMD_DEVICE_INFO_SET, 0x04);
        assert_eq!(CMD_KEYMAP_GET, 0x05);
        assert_eq!(CMD_KEYMAP_SET, 0x06);
        assert_eq!(CMD_CONFIG_GET, 0x07);
        assert_eq!(CMD_CONFIG_SET, 0x08);
        assert_eq!(CMD_KEY_EVENT, 0x09);
        assert_eq!(CMD_HEARTBEAT, 0x0a);
        assert_eq!(CMD_FIRMWARE_INFO, 0x0b);
        assert_eq!(CMD_VOICE_TEXT, 0x0c);
        assert_eq!(CMD_PC_STATUS, 0x0d);
        assert_eq!(CMD_MUSIC_STATUS, 0x0e);
        assert_eq!(CMD_MUSIC_CONTROL, 0x0f);
        assert_eq!(CMD_PROFILE_STATE, 0x10);
        assert_eq!(CMD_PROFILE_ICON_SET, 0x11);
        assert_eq!(CMD_HA_STATUS, 0x12);
        // 标准响应关系
        assert_eq!(response_cmd(CMD_CONFIG_SET), 0x88);
        assert_eq!(response_cmd(CMD_CONFIG_GET), 0x87);
        // 0x10 例外：响应就是自己
        assert_eq!(response_cmd(CMD_PROFILE_STATE), 0x90);
    }
}
