//! memclean — 轻量 Windows 内存清理工具
//!
//! 程序入口：单实例互斥 → 配置加载 → 权限提升 → 启动 UI。
//! 架构遵循 Pecia 的 core/ui 分离模式：
//!   core/（纯逻辑）← ui/（窗口/控件/托盘）

#![cfg_attr(windows, windows_subsystem = "windows")]

use memclean::core;
use memclean::ui;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

// ---------------------------------------------------------------------------
// 单实例互斥
// ---------------------------------------------------------------------------
struct InstanceGuard(HANDLE);
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.0); }
    }
}

fn activate_existing_window() {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SetForegroundWindow, SW_RESTORE, SW_SHOW};
        let found = FindWindowW(w!("MemCleanerMainWindow"), PCWSTR::null()).unwrap_or_default();
        if found.0.is_null() { return; }
        let _ = ShowWindow(found, SW_RESTORE);
        let _ = ShowWindow(found, SW_SHOW);
        let _ = SetForegroundWindow(found);
    }
}

fn lock_single_instance() -> Option<InstanceGuard> {
    unsafe {
        let h = CreateMutexW(None, false, w!("Local\\MemCleanerMainInstance")).unwrap_or_default();
        let err = GetLastError().0;
        if err == ERROR_ALREADY_EXISTS.0 || err == ERROR_ACCESS_DENIED.0 {
            None
        } else {
            Some(InstanceGuard(h))
        }
    }
}

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------
fn main() {
    let cfg = core::config::load();

    // 单实例互斥（优先于提权，避免重复弹 UAC）
    // --restart 参数：权限切换后的新实例，跳过互斥
    let is_restart = std::env::args().any(|a| a == "--restart");
    let _instance_guard = if is_restart {
        None
    } else {
        let g = lock_single_instance();
        if g.is_none() {
            activate_existing_window();
            return;
        }
        g
    };

    // 提升权限
    core::mem::raise_privileges();

    // 启动 UI（阻塞，直到用户退出）
    ui::window::run(cfg);
}
