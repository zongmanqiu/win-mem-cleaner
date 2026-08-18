//! 核心纯逻辑层 —— 不依赖任何 UI 代码。
//!
//! 包含内存清理、配置持久化、自动调度、工具函数。

pub mod config;
pub mod mem;
pub mod scheduler;
pub mod util;
