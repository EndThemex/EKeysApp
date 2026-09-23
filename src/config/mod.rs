//! 本地 App 配置（最近端口、UI 选项、语言、主题）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    Chinese,
    English,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::Chinese => "中文",
            Language::English => "English",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "深色",
            Theme::Light => "浅色",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    #[serde(default)]
    pub last_port: Option<String>,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default)]
    pub window_size: Option<[f32; 2]>,
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub theme: Theme,
    /// 是否启用 PC 状态向设备的周期推送（0x0D CMD_PC_STATUS）。
    /// 主机侧行为开关，与设备 `DeviceSettings` 解耦；默认关闭，避免用户
    /// 不清楚时主动暴露 Lock / 网络状态到固件。
    #[serde(default)]
    pub pc_status_push: bool,
    /// 是否启用音乐状态向设备的周期推送（0x0E CMD_MUSIC_STATUS）。
    /// 默认开启：音乐播放页（固件）没有状态推送就完全不可用。
    #[serde(default = "default_true")]
    pub music_status_push: bool,
}

/// serde 默认值：true（`#[serde(default)]` 只能给 false）。
fn default_true() -> bool {
    true
}

/// 手动实现 Default 而非派生：`music_status_push` 需要默认 true
/// （派生 bool 默认 false，会与 `#[serde(default = "default_true")]`
/// 产生"配置文件缺字段=true / 全新安装=false"的不一致）。
impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            last_port: None,
            auto_connect: false,
            window_size: None,
            language: Language::default(),
            theme: Theme::default(),
            pc_status_push: false,
            music_status_push: true,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let dir = base.join("wxi");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("config.json"))
}

pub fn load() -> LocalConfig {
    let Some(path) = config_path() else {
        return LocalConfig::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return LocalConfig::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(cfg: &LocalConfig) {
    if let Some(path) = config_path() {
        if let Ok(text) = serde_json::to_string_pretty(cfg) {
            let _ = std::fs::write(path, text);
        }
    }
}
