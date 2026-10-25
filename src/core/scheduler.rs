//! 后台自动清理调度器。
//! 两种方式各自独立轮询，互不影响：
//! - 方式一（定时）：每 `interval_minutes` 分钟按 `interval_level` 清理
//! - 方式二（阈值）：物理内存占用 > `threshold_percent`% 时按 `threshold_level` 清理
//! 内存压力优先于定时；两种方式都受「全屏避让」开关约束。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::core::config::AppConfig;
use crate::core::mem;

/// 轮询周期。
const POLL: Duration = Duration::from_secs(5);
/// 阈值清理后的静默期：占用长期停在阈值上方时，不能每轮询一次就清一次。
const THRESHOLD_COOLDOWN: Duration = Duration::from_secs(60);

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
            let mut last_interval = Instant::now();
            let mut last_threshold = Instant::now();
            while running.load(Ordering::Relaxed) {
                let snapshot = match cfg.read() {
                    Ok(g) => g.clone(),
                    Err(e) => e.into_inner().clone(),
                };
                let interval = Duration::from_secs(snapshot.interval_minutes.max(1) as u64 * 60);
                let interval_hit = last_interval.elapsed() >= interval;
                let threshold_hit = last_threshold.elapsed() >= THRESHOLD_COOLDOWN
                    && mem::physical_usage_percent() > snapshot.threshold_percent;

                if interval_hit || threshold_hit {
                    // 全屏/游戏中自动避让（可开关）：跳过本轮，计时保持不动（游戏一结束即补清）
                    if snapshot.fullscreen_avoid && mem::is_game_or_fullscreen() {
                        std::thread::sleep(Duration::from_secs(10));
                        continue;
                    }
                    if threshold_hit {
                        mem::clean_by_level(snapshot.threshold_level);
                        last_threshold = Instant::now();
                    } else {
                        mem::clean_by_level(snapshot.interval_level);
                        last_interval = Instant::now();
                    }
                }
                std::thread::sleep(POLL);
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
