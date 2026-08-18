//! 配置持久化（JSON）。
//! 极简模型：自动清理间隔 N 分钟 + 清理强度(1=标准/2=深度) + 全屏避让 + 开机启动。

#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 自动清理间隔（分钟）
    pub interval_minutes: u32,
    /// 自动清理强度：1=标准 2=深度
    pub level: u32,
    /// 全屏/游戏中自动避让
    pub fullscreen_avoid: bool,
    /// 开机启动
    pub autostart: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

impl AppConfig {
    pub fn defaults() -> Self {
        Self {
            interval_minutes: 5,
            level: 1,
            fullscreen_avoid: true,
            autostart: false,
        }
    }

    pub fn sanitize(&mut self) {
        self.interval_minutes = self.interval_minutes.clamp(1, 1440);
        if !(1..=2).contains(&self.level) {
            self.level = 1;
        }
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("memclean").join("config.json")
}

pub fn load() -> AppConfig {
    let cfg = match config_path() {
        p if !p.exists() => AppConfig::defaults(),
        p => {
            let mut raw = std::fs::read_to_string(p).unwrap_or_default();
            if raw.starts_with('\u{FEFF}') {
                raw = raw.trim_start_matches('\u{FEFF}').to_string();
            }
            let mut c = serde_json::from_str::<AppConfig>(&raw).unwrap_or_default();
            c.sanitize();
            c
        }
    };
    cfg
}

pub fn save(cfg: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = fs::write(&path, json);
    }
}
