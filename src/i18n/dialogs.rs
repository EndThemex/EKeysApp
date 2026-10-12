//! dialogs 区域翻译表：`widgets.rs`（确认弹窗 / 本地设置弹窗 / Diff 预览条 /
//! Toast）与 `app.rs`（确认弹窗标题正文 / 错误 Toast）。
//!
//! 保留前缀：`dlg.*`（弹窗）、`toast.*`（Toast 文案）、`diff.*`（Diff 预览条
//! 及其字段值）、`common.*`（两文件共用的通用文案）。

use super::Entry;

pub(super) const ENTRIES: &[Entry] = &[
    // ---- common：通用文案 ----
    ("common.close", "关闭", "Close"),
    ("common.language", "语言", "Language"),
    ("common.list_sep", "、", ", "),
    ("common.theme", "主题", "Theme"),
    ("common.unknown_error", "未知错误", "Unknown error"),
    // ---- diff：预览条框架 ----
    ("diff.apply", "应用 (Ctrl+Enter)", "Apply (Ctrl+Enter)"),
    ("diff.discard", "放弃 (Esc)", "Discard (Esc)"),
    (
        "diff.pending",
        "待下发 {count} 项",
        "{count} pending changes",
    ),
    // ---- diff：字段明细行 ----
    (
        "diff.field.audio_enable",
        "音频输出 → {value}",
        "Audio output → {value}",
    ),
    (
        "diff.field.keymap_profile",
        "按键配置 → 配置 {value}",
        "Keymap profile → Profile {value}",
    ),
    (
        "diff.field.password",
        "网络密码 → 已修改",
        "Network password → changed",
    ),
    (
        "diff.field.power_mode",
        "节能策略 → {value}",
        "Power policy → {value}",
    ),
    (
        "diff.field.rgb_brightness",
        "整体亮度 → {value}",
        "Overall brightness → {value}",
    ),
    (
        "diff.field.rgb_click_mode",
        "按下灯光反馈 → {value}",
        "Key light feedback → {value}",
    ),
    (
        "diff.field.rgb_mode",
        "灯效模式 → {value}",
        "Lighting mode → {value}",
    ),
    (
        "diff.field.rgb_single_color",
        "单色颜色 → 调色板 {value}",
        "Solid color → palette {value}",
    ),
    (
        "diff.field.ssid",
        "网络名称 → {value}",
        "Network name → {value}",
    ),
    (
        "diff.field.tft_brightness",
        "屏幕背光 → {value}",
        "Screen backlight → {value}",
    ),
    (
        "diff.field.tft_theme",
        "屏幕主题 → {value}",
        "Screen theme → {value}",
    ),
    ("diff.field.volume", "音量 → {value}", "Volume → {value}"),
    (
        "diff.field.wifi_switch",
        "WiFi 开关 → {value}",
        "WiFi switch → {value}",
    ),
    (
        "diff.field.work_mode",
        "连接方式 → {value}",
        "Connection → {value}",
    ),
    // ---- diff：字段取值 ----
    ("diff.power_mode.performance", "性能优先", "Performance"),
    ("diff.power_mode.saving", "省电优先", "Power saving"),
    ("diff.rgb.cycle", "循环", "Cycle"),
    ("diff.rgb.flame", "火焰", "Flame"),
    ("diff.rgb.gradient", "渐变", "Gradient"),
    ("diff.rgb.level", "电平", "Level"),
    ("diff.rgb.off", "关闭", "Off"),
    ("diff.rgb.pulse", "脉冲", "Pulse"),
    ("diff.rgb.rainbow", "彩虹", "Rainbow"),
    ("diff.rgb.rhythm", "律动", "Rhythm"),
    ("diff.rgb.solid", "单色", "Solid"),
    ("diff.rgb.voice", "拾音", "Voice"),
    ("diff.rgb.wave", "彩浪", "Color wave"),
    // ---- diff：敏感字段名（SET 成功 Toast 内联展示） ----
    ("diff.secret.secret_id", "SecretId", "SecretId"),
    ("diff.secret.secret_key", "SecretKey", "SecretKey"),
    ("diff.secret.wifi_password", "WiFi 密码", "WiFi password"),
    ("diff.theme.dark", "深色", "Dark"),
    ("diff.theme.light", "浅色", "Light"),
    ("diff.unknown", "未知", "Unknown"),
    ("diff.value.off", "关", "Off"),
    ("diff.value.on", "开", "On"),
    ("diff.work_mode.ble", "蓝牙", "Bluetooth"),
    ("diff.work_mode.usb", "USB 有线", "USB wired"),
    ("diff.work_mode.wireless", "2.4G 无线", "2.4G wireless"),
    // ---- dlg：确认弹窗 ----
    ("dlg.confirm.cancel", "取消", "Cancel"),
    ("dlg.confirm.continue", "继续", "Continue"),
    (
        "dlg.download.body",
        "设备将立即复位进入烧录（下载）模式，当前连接会断开，之后可用 idf.py flash / esptool 烧录固件。烧录完成后请在顶栏重新连接设备。是否继续？",
        "The device will immediately reset into flash (download) mode and the current connection will drop. You can then flash firmware with idf.py flash / esptool. After flashing, reconnect the device from the top bar. Continue?",
    ),
    ("dlg.download.title", "进入烧录模式", "Enter flash mode"),
    ("dlg.local_settings.title", "本地设置", "Local settings"),
    (
        "dlg.work_mode.body",
        "切换工作模式将重建键盘实例，期间无法响应按键，是否继续？",
        "Switching the work mode will recreate the keyboard instance, and keys will not respond during the process. Continue?",
    ),
    ("dlg.work_mode.title", "切换工作模式", "Switch work mode"),
    // ---- toast：业务 Toast ----
    ("toast.diff.applied", "已应用", "Applied"),
    (
        "toast.diff.applied_secrets",
        "已应用（含敏感字段：{names}）",
        "Applied (secrets included: {names})",
    ),
    (
        "toast.diff.no_response",
        "设备无响应",
        "Device not responding",
    ),
];
