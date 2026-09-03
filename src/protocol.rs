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

// ---------- Keymap 数据模型（阶段 05 起生效） ----------
//
// 这一组类型只描述"键映射"在桌面 App 侧的内存形态；阶段 05 之前固件侧
// 尚未支持 CMD_KEYMAP_GET/SET，所以这里只做"本地编辑 + 草稿预览"，不接
// 协议命令。设计上完全独立于 DeviceSettings，便于后面直接拆成单独的
// 命令而不影响现有 Settings 面板。

/// 按键可执行的动作（与固件 HID encoder 对齐的最小子集）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyAction {
    /// 未绑定（透传 / 触发默认）
    None,
    /// 普通键：value 为 HID Usage ID（如 'A' = 0x04）
    Keyboard(u16),
    /// 多媒体
    Media(MediaKey),
    /// 鼠标动作
    Mouse(MouseAction),
    /// 宏：按键序列（简化版，延时写死在每步后）
    Macro(Vec<MacroStep>),
    /// 切到指定 layer（按下时进入，松手回到 Base）
    LayerSwitch(u8),
    /// 旋钮动作：旋转或按下
    Encoder(EncoderAction),
}

/// 旋钮子动作：顺时针 / 逆时针 / 按下
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncoderAction {
    /// 顺时针
    Cw,
    /// 逆时针
    Ccw,
    /// 按下
    Press,
}

impl Default for KeyAction {
    fn default() -> Self {
        KeyAction::None
    }
}

impl KeyAction {
    /// 给 UI 展示用的简短标签
    pub fn label(&self) -> String {
        match self {
            KeyAction::None => "未绑定".into(),
            KeyAction::Keyboard(code) => format!("K 0x{code:02X}"),
            KeyAction::Media(m) => format!("Media: {m:?}"),
            KeyAction::Mouse(m) => format!("Mouse: {m:?}"),
            KeyAction::Macro(steps) => format!("Macro ({} 步)", steps.len()),
            KeyAction::LayerSwitch(l) => format!("→ Layer {l}"),
            KeyAction::Encoder(e) => format!("Enc: {e:?}"),
        }
    }

    /// 是否"非空"（用于 Diff 计数）
    pub fn is_set(&self) -> bool {
        !matches!(self, KeyAction::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKey {
    PlayPause,
    Next,
    Prev,
    VolUp,
    VolDown,
    Mute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseAction {
    LeftClick,
    RightClick,
    MiddleClick,
    ScrollUp,
    ScrollDown,
}

/// 宏里的一步：按键 + 延时（毫秒）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroStep {
    pub action: KeyAction,
    pub delay_ms: u32,
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

/// 4×3 小键盘基础层的槽位定义（不含绑定）。仅用于 demo。
///
/// 4 行 × 3 列；row 0 第 3 个槽位是旋钮，其余 11 个是普通键：
///   row 0: K1 / K2 / [KNOB]      （col 2, row 0 → 旋钮）
///   row 1: K3 / K4 / K5
///   row 2: K6 / K7 / K8
///   row 3: K9 / K10/ K11
///
/// 旋钮用 `width_units = 1.0` + `kind = Encoder` 标识；宽度与按键一致，
/// 渲染层判断 `kind` 后画一个圆形刻度盘代替方键。
fn demo_base_4x3() -> Vec<KeySlot> {
    let mut out = Vec::with_capacity(12);
    let normal: [(&str, u8, u8); 11] = [
        ("K1", 0, 0),
        ("K2", 0, 1),
        ("K3", 1, 0),
        ("K4", 1, 1),
        ("K5", 1, 2),
        ("K6", 2, 0),
        ("K7", 2, 1),
        ("K8", 2, 2),
        ("K9", 3, 0),
        ("K10", 3, 1),
        ("K11", 3, 2),
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
        col: 2,
        label: "KNOB".into(),
        width_units: 1.0,
        kind: SlotKind::Encoder,
    });
    out
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
