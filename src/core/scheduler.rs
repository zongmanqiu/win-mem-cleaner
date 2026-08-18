//! 后台自动清理调度器。
//! 只有一个策略：每隔 N 分钟按用户选择的强度(1/2/3)自动清理一次。

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::core::config::AppConfig;
use crate::core::mem;

pub struct Scheduler {
    running: Arc<AtomicBool>,
    cfg: Arc<RwLock<AppConfig>>,
}

impl Scheduler {
    pub fn new(cfg: Arc<RwLock<AppConfig>>) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
            cfg,
        }
    }

    pub fn start(&self) {
        let running = self.running.clone();
        let cfg = self.cfg.clone();
        std::thread::spawn(move || {
            let mut last_trigger = Instant::now();
            while running.load(Ordering::Relaxed) {
                let snapshot = match cfg.read() {
                    Ok(g) => g.clone(),
                    Err(e) => e.into_inner().clone(),
                };
                let interval = Duration::from_secs(snapshot.interval_minutes.max(1) as u64 * 60);
                if Instant::now().duration_since(last_trigger) >= interval {
                    // 全屏/游戏中自动避让（可开关）：跳过本次清理，计时保持不动（游戏一结束即补清）
                    if snapshot.fullscreen_avoid && mem::is_game_or_fullscreen() {
                        std::thread::sleep(Duration::from_secs(10));
                        continue;
                    }
                    mem::clean_by_level(snapshot.level);
                    last_trigger = Instant::now();
                }
                std::thread::sleep(Duration::from_secs(5));
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
