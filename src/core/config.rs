//! 配置持久化（JSON，便携：与 exe 同级的 `WinMemCleaner.json`）。
//! 自动清理两种方式（互相独立）：
//! - 定时：每 `interval_minutes` 分钟按 `interval_level` 清理
//! - 阈值：物理内存占用 > `threshold_percent`% 时按 `threshold_level` 清理
//! 另有全屏避让、开机启动。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 方式一：自动清理间隔（分钟）
    pub interval_minutes: u32,
    /// 方式一：清理强度：1=标准 2=深度
    #[serde(alias = "level")]
    pub interval_level: u32,
    /// 方式二：物理内存占用阈值（%），超过即触发
    pub threshold_percent: u32,
    /// 方式二：清理强度：1=标准 2=深度
    pub threshold_level: u32,
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
            interval_level: 1,
            threshold_percent: 85,
            threshold_level: 2,
            fullscreen_avoid: true,
            autostart: false,
        }
    }

    pub fn sanitize(&mut self) {
        self.interval_minutes = self.interval_minutes.clamp(1, 1440);
        // 100% 永远不会被超过，故上限取 99
        self.threshold_percent = self.threshold_percent.clamp(1, 99);
        if !(1..=2).contains(&self.interval_level) {
            self.interval_level = 1;
        }
        if !(1..=2).contains(&self.threshold_level) {
            self.threshold_level = 2;
        }
    }
}

/// 便携程序：配置只放在 exe 同级目录，不往用户目录写任何东西。
/// 取不到 exe 路径时返回 None——宁可不存盘，也不写到别处。
pub fn config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("WinMemCleaner.json"))
}

pub fn load() -> AppConfig {
    let path = match config_path() {
        Some(p) if p.exists() => p,
        _ => return AppConfig::defaults(),
    };
    let mut raw = fs::read_to_string(path).unwrap_or_default();
    if raw.starts_with('\u{FEFF}') {
        raw = raw.trim_start_matches('\u{FEFF}').to_string();
    }
    let mut c = serde_json::from_str::<AppConfig>(&raw).unwrap_or_default();
    c.sanitize();
    c
}

pub fn save(cfg: &AppConfig) {
    let Some(path) = config_path() else { return };
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = fs::write(&path, json);
    }
}
