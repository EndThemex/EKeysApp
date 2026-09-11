//! Phosphor 图标集中定义。所有 UI 用到的图标统一在此导出，避免在业务代码里
//! 直接拼接 unicode 字面量，也便于后续切换 variant 或批量调整。

pub use egui_phosphor::regular as r;

// 品牌 / 导航
pub const BRAND_KEYBOARD: &str = r::KEYBOARD;
pub const NAV_CONNECT: &str = r::PLUG;
pub const NAV_SETTINGS: &str = r::SLIDERS_HORIZONTAL;
pub const NAV_KEYMAP: &str = r::KEYBOARD;
pub const NAV_LIGHTING: &str = r::LIGHTBULB;
pub const NAV_WIFI: &str = r::WIFI_HIGH;
pub const NAV_VOICE: &str = r::MICROPHONE_STAGE;
pub const NAV_LOG: &str = r::SCROLL;
pub const NAV_ABOUT: &str = r::INFO;

// 顶栏 / 按钮
pub const GEAR: &str = r::GEAR;
pub const REFRESH: &str = r::ARROW_CLOCKWISE;
pub const SCAN: &str = r::ARROW_CLOCKWISE;

// 状态栏
pub const UPTIME: &str = r::TIMER;
pub const TX: &str = r::ARROW_UP;
pub const RX: &str = r::ARROW_DOWN;
pub const HEARTBEAT: &str = r::HEART;

// Toast
pub const TOAST_INFO: &str = r::INFO;
pub const TOAST_SUCCESS: &str = r::CHECK;
pub const TOAST_WARNING: &str = r::WARNING;
pub const TOAST_ERROR: &str = r::X;

// 日志行首字符
pub const LOG_TX: &str = r::CARET_RIGHT;
pub const LOG_RX: &str = r::CARET_LEFT;
pub const LOG_FIRMWARE: &str = r::INFO;
pub const LOG_APP: &str = r::CIRCLE;

// Settings Tabs
pub const TAB_DISPLAY: &str = r::MONITOR;
pub const TAB_KEYBOARD: &str = r::KEYBOARD;
pub const TAB_AUDIO: &str = r::SPEAKER_HIGH;
pub const TAB_POWER: &str = r::LIGHTNING;
/// Settings 页 → PC 状态 tab（与 TAB_DISPLAY 同用桌面意象，但用 tower
/// 避免和"显示"（显示器）撞视觉）。
pub const TAB_PC: &str = r::DESKTOP_TOWER;
/// Settings 页 → 固件升级 tab（芯片意象）。
pub const TAB_FIRMWARE: &str = r::CPU;
pub const CUSTOM_ICON: &str = r::IMAGE;

// Keymap
pub const KEYMAP_CONFIRM: &str = r::CHECK;
pub const KEYMAP_RENAME_EDIT: &str = r::NOTE_PENCIL;
pub const KEYMAP_RENAME_CLOSE: &str = r::X;
pub const KEYMAP_RELOAD: &str = r::ARROW_CLOCKWISE;
pub const KEYMAP_EXPORT: &str = r::DOWNLOAD_SIMPLE;
pub const KEYMAP_CAPTURE: &str = r::KEYBOARD;

// Lighting — 与固件 RGBMode 0~7 + ClickHighlight 0~2 严格对齐
// （RGBLightControl.h: RGB_NONE/SINGLE/RAINBOW/RAINBOWWARE/COLORCYCLE/METER/FIRE/PULSE）
// （ClickHighlight.h: CLICK_NONE/SINGLE/WARE）
pub const LIGHT_OFF: &str = r::POWER;
pub const LIGHT_SOLID: &str = r::SQUARE;
pub const LIGHT_RAINBOW: &str = r::RAINBOW;
pub const LIGHT_WAVE: &str = r::WAVES;
pub const LIGHT_CYCLE: &str = r::ARROWS_CLOCKWISE;
pub const LIGHT_METER: &str = r::ACTIVITY;
pub const LIGHT_FIRE: &str = r::FLAME;
pub const LIGHT_PULSE: &str = r::HEARTBEAT;
pub const LIGHT_NONE: &str = r::PROHIBIT;
pub const LIGHT_CLICK: &str = r::CURSOR_CLICK;
pub const LIGHT_WARE: &str = r::CIRCLE_HALF;
