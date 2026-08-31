//! 本地 App 配置（最近端口、UI 选项）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalConfig {
    #[serde(default)]
    pub last_port: Option<String>,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default)]
    pub window_size: Option<[f32; 2]>,
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
