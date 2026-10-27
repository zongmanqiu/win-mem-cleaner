//! 后台自动清理调度器。两种方式各自独立轮询，**档位固定**：
//! - 定时：每 `interval_minutes` 分钟做一次**标准**清理（温和，不打扰）
//! - 阈值：物理内存占用 > `threshold_percent`% 时做一次**深度**清理（真到吃紧才彻底清）
//!
//! 为什么不给人选档：两个非默认组合都更差——定时+深度＝周期性主动卡顿；
//! 阈值+标准＝实测几乎清不动（标准档连缓存读数都不变，深度档才把 6.3 GB 打到 0.25 GB）。
//! 想手动选档，走托盘右键菜单。
//!
//! 阈值方式带静默期与**回弹退避**：清理后 30 秒复查一次，若占用又越过阈值，
//! 说明是活跃负载在 refill（这块内存清不动），把静默期翻倍（上限 15 分钟），
//! 免得每分钟让用户卡一次；复查通过则回到 60 秒。
//!
//! 两种方式各记录「上次执行时刻」供主窗口显示；手动托盘清理不记录（那是用户自己按的）。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::core::config::AppConfig;
use crate::core::{mem, util};

/// 轮询周期。改完设置最多这么久就生效（每轮都重读配置）。
const POLL: Duration = Duration::from_secs(5);
/// 阈值清理后的最短静默期。
const COOLDOWN_MIN: Duration = Duration::from_secs(60);
/// 回弹退避的上限。
const COOLDOWN_MAX: Duration = Duration::from_secs(15 * 60);
/// 清理后复查「有没有 hold 住」的等待时间。
const REBOUND_CHECK_AFTER: Duration = Duration::from_secs(30);
/// 两种方式各自的固定档位：定时=标准，阈值=深度。
const INTERVAL_LEVEL: u32 = 1;
const THRESHOLD_LEVEL: u32 = 2;

/// 打包成 `h<<16 | m<<8 | s`，所以取值永远小于 2^21，可以用 MAX 当「从未执行」。
const NEVER: usize = usize::MAX;

/// 两种自动清理方式，同时用作 `LAST_RUN` 的下标。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Interval,
    Threshold,
}

static LAST_RUN: [AtomicUsize; 2] = [AtomicUsize::new(NEVER), AtomicUsize::new(NEVER)];

/// 该方式上次自动清理的 `(时, 分, 秒)`；`None` = 本次运行以来从未执行。
pub fn last_run(slot: Slot) -> Option<(u32, u32, u32)> {
    let v = LAST_RUN[slot as usize].load(Ordering::Relaxed);
    if v == NEVER {
        None
    } else {
        Some((
            (v >> 16) as u32,
            ((v >> 8) & 0xFF) as u32,
            (v & 0xFF) as u32,
        ))
    }
}

fn record(slot: Slot) {
    let (h, m, s) = util::local_time();
    LAST_RUN[slot as usize].store((h << 16 | m << 8 | s) as usize, Ordering::Relaxed);
}

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
            let mut cooldown = COOLDOWN_MIN;
            // (复查起点, 触发当时的阈值) —— 只挂一个，因为一次只会刚清理一轮
            let mut probe: Option<(Instant, u32)> = None;

            while running.load(Ordering::Relaxed) {
                // 每轮重读：用户改完设置最迟 5 秒生效，不需要重启
                let snapshot = match cfg.read() {
                    Ok(g) => g.clone(),
                    Err(e) => e.into_inner().clone(),
                };
                let pct = mem::physical_usage_percent();

                if let Some((at, thr)) = probe.take() {
                    if pct <= thr {
                        cooldown = COOLDOWN_MIN; // 清理 hold 住了 → 恢复灵敏度
                    } else if at.elapsed() >= REBOUND_CHECK_AFTER {
                        cooldown = (cooldown * 2).min(COOLDOWN_MAX); // 又满了 → 这块清不动，退避
                    } else {
                        probe = Some((at, thr)); // 还没到复查时间，继续等
                    }
                }

                if pct > snapshot.threshold_percent && last_threshold.elapsed() >= cooldown {
                    mem::clean_by_level(THRESHOLD_LEVEL);
                    let now = Instant::now();
                    last_threshold = now;
                    // 阈值方式比定时方式更彻底，定时那一轮顺延，避免 5 秒内连清两次
                    last_interval = now;
                    record(Slot::Threshold);
                    probe = Some((now, snapshot.threshold_percent));
                } else if last_interval.elapsed()
                    >= Duration::from_secs(snapshot.interval_minutes.max(1) as u64 * 60)
                {
                    mem::clean_by_level(INTERVAL_LEVEL);
                    last_interval = Instant::now();
                    record(Slot::Interval);
                }
                std::thread::sleep(POLL);
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
