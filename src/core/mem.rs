//! Windows 内存清理核心。
//! 参考 memreduct / MemCleaner / memory_cleaner 的实现思路，用 Win32/Native API。

#![allow(non_snake_case)]
#![allow(dead_code)]

use std::collections::HashSet;
use std::mem::size_of;

use windows::core::{w, PCSTR, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::ProcessStatus::{
    EmptyWorkingSet, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
    QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

// ---------------------------------------------------------------------------
// 常量：未文档化的 Native API
// ---------------------------------------------------------------------------
const SYSTEM_MEMORY_LIST_INFORMATION_CLASS: i32 = 80; // 0x50
const SYSTEM_FILE_CACHE_INFORMATION_EX_CLASS: i32 = 81; // 0x51
const MEMORY_EMPTY_WORKING_SETS: u32 = 2;
const MEMORY_FLUSH_MODIFIED_LIST: u32 = 3;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;
const MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST: u32 = 5;

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 权限提升
// ---------------------------------------------------------------------------
pub fn enable_privilege(name: &str) -> bool {
    unsafe {
        let mut token = HANDLE::default();
        let proc = GetCurrentProcess();
        if OpenProcessToken(proc, TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let token = OwnedHandle(token);

        let wide = crate::core::util::wide(name);
        let mut luid = LUID::default();
        if LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(wide.as_ptr()), &mut luid).is_err() {
            return false;
        }

        let tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        AdjustTokenPrivileges(token.0, false, Some(&tp), 0, None, None).is_ok()
    }
}

/// 尝试提升本进程所需的全部特权（静默失败，非致命）。
pub fn raise_privileges() {
    for name in [
        "SeIncreaseQuotaPrivilege",
        "SeProfileSingleProcessPrivilege",
        "SeDebugPrivilege",
    ] {
        let _ = enable_privilege(name);
    }
}

// ---------------------------------------------------------------------------
// 系统内存信息
// ---------------------------------------------------------------------------
pub fn mem_info() -> MEMORYSTATUSEX {
    let mut mem = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = GlobalMemoryStatusEx(&mut mem);
    }
    mem
}

/// 当前物理内存使用百分比 (0..=100)。
pub fn physical_usage_percent() -> u32 {
    let mem = mem_info();
    if mem.ullTotalPhys == 0 {
        return 0;
    }
    let used = mem.ullTotalPhys.saturating_sub(mem.ullAvailPhys);
    (used.saturating_mul(100) / mem.ullTotalPhys).min(100) as u32
}

/// 当前可用物理内存字节数。
pub fn avail_bytes() -> u64 {
    mem_info().ullAvailPhys
}

/// 格式化字节为可读字符串。
pub fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, units[i])
    } else {
        format!("{:.1} {}", v, units[i])
    }
}

// ---------------------------------------------------------------------------
// Native API 动态加载
// ---------------------------------------------------------------------------
#[allow(non_snake_case)]
type NtSetSystemInformationFn = unsafe extern "system" fn(
    SystemInformationClass: i32,
    SystemInformation: *mut core::ffi::c_void,
    SystemInformationLength: u32,
) -> i32;

fn nt_set_system_information() -> Option<NtSetSystemInformationFn> {
    unsafe {
        let module = GetModuleHandleW(w!("ntdll.dll")).ok()?;
        let proc = GetProcAddress(module, PCSTR(c"NtSetSystemInformation".as_ptr().cast()))?;
        Some(std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            NtSetSystemInformationFn,
        >(proc))
    }
}

fn issue_memory_list_command(command: u32) -> bool {
    let _ = enable_privilege("SeProfileSingleProcessPrivilege");
    let Some(f) = nt_set_system_information() else {
        return false;
    };
    let mut cmd = command;
    let status = unsafe {
        f(
            SYSTEM_MEMORY_LIST_INFORMATION_CLASS,
            &mut cmd as *mut u32 as *mut _,
            size_of::<u32>() as u32,
        )
    };
    status == 0
}

// 系统文件缓存结构（未文档化）
#[repr(C)]
#[derive(Default)]
struct SystemFileCacheInformation {
    current_size: usize,
    peak_size: usize,
    page_fault_count: u32,
    minimum_working_set: usize,
    maximum_working_set: usize,
    current_size_including_transition_in_pages: usize,
    peak_size_including_transition_in_pages: usize,
    transition_repurpose_count: u32,
    flags: u32,
}

fn measure_avail_delta(action: impl FnOnce() -> bool) -> (bool, u64) {
    let before = avail_bytes();
    let ok = action();
    if !ok {
        return (false, 0);
    }
    let after = avail_bytes();
    (true, after.saturating_sub(before))
}

/// 清空系统中所有进程的工作集（系统级工作集修剪）。
pub fn clear_system_working_sets() -> bool {
    issue_memory_list_command(MEMORY_EMPTY_WORKING_SETS)
}

/// 清空系统文件缓存。
pub fn clear_system_file_cache() -> bool {
    let _ = enable_privilege("SeIncreaseQuotaPrivilege");
    // 使用文档化 API SetSystemFileCacheSize 清空缓存
    // 传入 0, 0 会禁用文件缓存，系统会自动回收
    type SetSysCacheSizeFn = unsafe extern "system" fn(usize, usize, u32) -> bool;
    unsafe {
        let module = GetModuleHandleW(w!("kernel32.dll")).ok().unwrap_or_default();
        let proc = GetProcAddress(module, PCSTR(c"SetSystemFileCacheSize".as_ptr().cast()));
        if let Some(f) = proc {
            let f: SetSysCacheSizeFn = std::mem::transmute(f);
            // 先设为最小，再恢复默认
            let ok = f(0, 0, 0);
            // 恢复默认（让系统自动管理）
            f(0, usize::MAX, 0);
            return ok;
        }
    }
    // 回退：用 NtSetSystemInformation
    let Some(f) = nt_set_system_information() else {
        return false;
    };
    let mut info = SystemFileCacheInformation {
        minimum_working_set: 0,
        maximum_working_set: 0,
        current_size: 0,
        ..Default::default()
    };
    let status = unsafe {
        f(
            80, // SystemFileCacheInformation（非 Ex）
            &mut info as *mut SystemFileCacheInformation as *mut _,
            size_of::<SystemFileCacheInformation>() as u32,
        )
    };
    // 恢复默认
    info.minimum_working_set = 0;
    info.maximum_working_set = usize::MAX;
    let _ = unsafe {
        f(
            80,
            &mut info as *mut SystemFileCacheInformation as *mut _,
            size_of::<SystemFileCacheInformation>() as u32,
        )
    };
    status == 0
}

/// 冲刷 Modified 页列表（脏页写盘后回收）。
pub fn flush_modified_page_list() -> bool {
    issue_memory_list_command(MEMORY_FLUSH_MODIFIED_LIST)
}

/// 清空 Standby 备用内存列表。
pub fn clear_standby() -> bool {
    issue_memory_list_command(MEMORY_PURGE_STANDBY_LIST)
}

/// 清空低优先级 Standby（较温和）。
pub fn clear_standby_low_priority() -> bool {
    issue_memory_list_command(MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST)
}

// ---------------------------------------------------------------------------
// 进程工作集修剪
// ---------------------------------------------------------------------------
pub fn working_set_for(pid: u32) -> Option<u64> {
    unsafe {
        let handle =
            OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let size = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(handle.0, &mut counters, size).is_ok() {
            Some(counters.WorkingSetSize as u64)
        } else {
            None
        }
    }
}

/// 修剪单个进程的工作集，返回释放的工作集字节数（若成功）。
pub fn trim_process(pid: u32) -> Option<u64> {
    unsafe {
        let handle = OwnedHandle(
            OpenProcess(
                PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            )
            .ok()?,
        );
        let before = working_set_for(pid)?;
        if EmptyWorkingSet(handle.0).is_err() {
            return None;
        }
        let after = working_set_for(pid).unwrap_or(before);
        Some(before.saturating_sub(after))
    }
}

// 保护名单 —— 不清理的系统关键进程
fn critical_protected_names() -> HashSet<&'static str> {
    [
        "system",
        "registry",
        "memory compression",
        "idle",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "winlogon.exe",
        "services.exe",
        "lsass.exe",
        "dwm.exe",
        "fontdrvhost.exe",
    ]
    .into_iter()
    .collect()
}

// 常用程序保护名单（防止清掉正在用的，导致卡顿）
fn comfort_protected_names() -> HashSet<&'static str> {
    [
        "explorer.exe",
        "sihost.exe",
        "shellexperiencehost.exe",
        "startmenuexperiencehost.exe",
        "textinputhost.exe",
        "searchhost.exe",
        "taskmgr.exe",
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "conhost.exe",
    ]
    .into_iter()
    .collect()
}

/// trim 配置
#[derive(Clone, Debug)]
pub struct TrimOptions {
    /// 清所有进程工作集（含常用程序保护名单中的）还是只用临界保护
    pub aggressive: bool,
    /// 排除的进程名列表（小写，不含 .exe）
    pub exclude: HashSet<String>,
}

impl Default for TrimOptions {
    fn default() -> Self {
        Self {
            aggressive: false,
            exclude: HashSet::new(),
        }
    }
}

/// 进程修剪汇总
#[derive(Default)]
pub struct TrimReport {
    pub trimmed: u64,
    pub process_freed_bytes: u64,
}

pub fn current_foreground_pid() -> Option<u32> {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        (pid != 0).then_some(pid)
    }
}

/// 遍历所有进程，修剪其工作集。跳过自身、保护名单、排除名单。
pub fn trim_all_processes(opts: &TrimOptions) -> TrimReport {
    let mut report = TrimReport::default();
    let current_pid = unsafe { GetCurrentProcessId() };

    let mut protected = critical_protected_names();
    if !opts.aggressive {
        protected.extend(comfort_protected_names());
    }

    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return report;
        };
        let snapshot = OwnedHandle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot.0, &mut entry).is_ok() {
            let mut first = true;
            while first || Process32NextW(snapshot.0, &mut entry).is_ok() {
                first = false;
                let pid = entry.th32ProcessID;
                if pid == 0 || pid == current_pid {
                    continue;
                }
                let exe_name = crate::core::util::wide_to_string(&entry.szExeFile);
                let key = exe_name.trim().to_ascii_lowercase();
                if protected.contains(key.as_str()) || opts.exclude.contains(&key) {
                    continue;
                }
                // 只修剪工作集较大的进程，避免对微小进程无谓调用
                if let Some(ws) = working_set_for(pid) {
                    if ws < 8 * 1024 * 1024 {
                        // <8MB 跳过
                        continue;
                    }
                }
                if let Some(freed) = trim_process(pid) {
                    report.trimmed += 1;
                    report.process_freed_bytes += freed;
                }
            }
        }
    }
    report
}

// ---------------------------------------------------------------------------
// 三种核心清理方式（供按钮 / 阈值 / 定时复用）
// ---------------------------------------------------------------------------

/// 「裁剪进程工作集」的结果。
#[derive(Default)]
pub struct TrimResult {
    pub trimmed: u64,
    pub freed_bytes: u64,
}

/// 裁剪进程工作集。
/// `brute` 为 false 时逐个进程 EmptyWorkingSet（较温和）；为 true 时用系统级 MemoryEmptyWorkingSets 一次全清（强力）。
pub fn do_trim(brute: bool) -> TrimResult {
    if brute {
        // 强力模式：清空系统中所有进程工作集
        clear_system_working_sets();
        // 释放量用全局可用内存差值衡量
        let freed = measure_avail_delta(|| {
            clear_system_working_sets();
            true
        })
        .1;
        return TrimResult { trimmed: 0, freed_bytes: freed };
    }
    let opts = TrimOptions {
        aggressive: false,
        exclude: HashSet::new(),
    };
    let tr = trim_all_processes(&opts);
    TrimResult {
        trimmed: tr.trimmed,
        freed_bytes: tr.process_freed_bytes,
    }
}

/// 「清理系统缓存」的结果。
#[derive(Default)]
pub struct CacheResult {
    pub freed_bytes: u64,
}

/// 清理系统文件缓存（及系统工作集）。
pub fn do_flush_cache() -> CacheResult {
    let mut freed = 0u64;
    // 清理系统文件缓存
    freed = freed.saturating_add(measure_avail_delta(|| {
        clear_system_file_cache();
        true
    })
    .1);
    CacheResult { freed_bytes: freed }
}

/// 「执行全部已知清理」的结果。
#[derive(Default)]
pub struct AllCleanResult {
    pub standby_cleared: bool,
    pub standby_freed_bytes: u64,
    pub syscache_cleared: bool,
    pub syscache_freed_bytes: u64,
    pub modified_cleared: bool,
    pub modified_freed_bytes: u64,
    pub system_ws_cleared: bool,
    pub global_freed_bytes: u64,
}

/// 执行全部已知的内存清理：
/// 合并物理内存 → 清系统文件缓存 → 全局清空工作集(强力) → Standby → Modified → 系统工作集。
/// `brute` 决定工作集清空用系统级还是逐进程。
pub fn do_clean_all(brute: bool) -> AllCleanResult {
    let mut r = AllCleanResult::default();
    let global_before = avail_bytes();

    // 1) 合并物理内存页 (Win8+)
    let _ = enable_privilege("SeIncreaseQuotaPrivilege");
    let _ = combine_physical_memory();

    // 2) 系统文件缓存
    (r.syscache_cleared, r.syscache_freed_bytes) = measure_avail_delta(|| {
        clear_system_file_cache();
        true
    });

    // 3) 工作集（强力=系统级，否则逐进程）
    if brute {
        r.system_ws_cleared = clear_system_working_sets();
    } else {
        let opts = TrimOptions {
            aggressive: false,
            exclude: HashSet::new(),
        };
        trim_all_processes(&opts);
    }

    // 4) Standby 备用内存
    (r.standby_cleared, r.standby_freed_bytes) = measure_avail_delta(clear_standby);

    // 5) Modified 页列表
    (r.modified_cleared, r.modified_freed_bytes) = measure_avail_delta(flush_modified_page_list);

    r.global_freed_bytes = avail_bytes().saturating_sub(global_before);
    r
}

/// 合并物理内存页列表（Win8+，未文档化）。
fn combine_physical_memory() -> bool {
    #[repr(C)]
    #[derive(Default)]
    struct MemoryCombineInformationEx {
        pages_combined: usize,
        page_flags: u32,
    }
    let Some(f) = nt_set_system_information() else {
        return false;
    };
    let mut info = MemoryCombineInformationEx::default();
    let status = unsafe {
        f(
            130,                                             // SystemCombinePhysicalMemoryInformation
            &mut info as *mut MemoryCombineInformationEx as *mut _,
            size_of::<MemoryCombineInformationEx>() as u32,
        )
    };
    status == 0
}

// ---------------------------------------------------------------------------
// 内存使用信息（供界面实时显示）
// ---------------------------------------------------------------------------
/// 一套内存用量快照。
#[derive(Default, Clone, Copy)]
pub struct MemSnapshot {
    pub phys_total: u64,
    pub phys_used: u64,
    pub phys_percent: u32,
    pub page_total: u64,
    pub page_used: u64,
    pub page_percent: u32,
    pub cache_used: u64,
    pub cache_total: u64,
    pub cache_percent: u32,
}

/// 采集物理 / 虚拟内存（页面文件）/ 系统缓存用量。
pub fn memory_snapshot() -> MemSnapshot {
    let mut s = MemSnapshot::default();
    let mem = mem_info();
    s.phys_total = mem.ullTotalPhys;
    s.phys_used = mem.ullTotalPhys.saturating_sub(mem.ullAvailPhys);
    if s.phys_total > 0 {
        s.phys_percent = (s.phys_used.saturating_mul(100) / s.phys_total).min(100) as u32;
    }
    s.page_total = mem.ullTotalPageFile;
    s.page_used = mem.ullTotalPageFile.saturating_sub(mem.ullAvailPageFile);
    if s.page_total > 0 {
        s.page_percent = (s.page_used.saturating_mul(100) / s.page_total).min(100) as u32;
    }
    // 系统缓存（经 NtQuerySystemInformation(SystemFileCacheInformation)）
    let (cache_used, cache_total) = system_cache_info();
    s.cache_used = cache_used;
    s.cache_total = cache_total;
    if cache_total > 0 {
        s.cache_percent = (cache_used.saturating_mul(100) / cache_total).min(100) as u32;
    }
    s
}

// ---------------------------------------------------------------------------
// 档位清理（对用户隐藏技术细节）
// ---------------------------------------------------------------------------
/// 判断当前进程是否真正以管理员权限运行（通过 UAC 提权）。
/// 注意：IsUserAnAdmin() 只检查用户是否在 Administrators 组，
/// 即使没走 UAC 也会返回 true。这里用 TokenElevation 检查进程令牌是否真正提权。
pub fn is_admin() -> bool {
    unsafe {
        use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};

        let proc = GetCurrentProcess();
        let mut token = HANDLE::default();
        if OpenProcessToken(proc, TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let token = OwnedHandle(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_length = 0u32;
        let ok = GetTokenInformation(
            token.0,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

/// 判断当前是否在「全屏 / 游戏 / 演示 / 忙碌」状态下——自动清理应避开，避免造成卡顿。
pub fn is_game_or_fullscreen() -> bool {
    unsafe {
        if let Ok(state) = SHQueryUserNotificationState() {
            state.0 == QUNS_RUNNING_D3D_FULL_SCREEN.0
                || state.0 == QUNS_PRESENTATION_MODE.0
                || state.0 == QUNS_BUSY.0
        } else {
            false
        }
    }
}

/// 按档位执行清理：
/// - 标准：合并物理内存页 + 裁剪进程工作集 + 清理系统文件缓存 + 清理低优先级备用内存
/// - 深度：标准的全部内容 + 系统级清空工作集 + 清空 Standby + 冲刷 Modified（会短暂卡顿）
pub fn clean_by_level(level: u32) -> String {
    // ---- 标准清理（基础操作，所有档位都执行）----
    let _ = enable_privilege("SeIncreaseQuotaPrivilege");
    let _ = combine_physical_memory();
    do_trim(false);
    do_flush_cache();
    clear_standby_low_priority();

    if level >= 2 {
        // ---- 深度额外操作 ----
        clear_system_working_sets();   // 系统级清空所有工作集（主要卡顿来源）
        clear_standby();               // 清空 Standby 列表
        flush_modified_page_list();    // 冲刷 Modified 页列表
        "深度清理完成".to_string()
    } else {
        "标准清理完成".to_string()
    }
}

/// 查询系统缓存占用（未文档化 API）。
/// 返回 (当前缓存大小, 峰值缓存大小) —— 用于显示"已用 / 峰值"而非"已用 / 总量"。
fn system_cache_info() -> (u64, u64) {
    #[repr(C)]
    #[derive(Default)]
    struct SystemFileCacheInfo {
        current_size: usize,
        peak_size: usize,
        page_fault_count: u32,
        minimum_working_set: usize,
        maximum_working_set: usize,
        current_size_including_transition_in_pages: usize,
        peak_size_including_transition_in_pages: usize,
        transition_repurpose_count: u32,
        flags: u32,
    }
    unsafe {
        let Ok(module) = GetModuleHandleW(w!("ntdll.dll")) else {
            return (0, 0);
        };
        let Some(proc) = GetProcAddress(
            module,
            PCSTR(c"NtQuerySystemInformation".as_ptr().cast()),
        ) else {
            return (0, 0);
        };
        type Fn = unsafe extern "system" fn(i32, *mut core::ffi::c_void, u32, *mut u32) -> i32;
        let f: Fn = std::mem::transmute(proc);
        let mut info = SystemFileCacheInfo::default();
        let status = f(
            81, // SystemFileCacheInformation
            &mut info as *mut SystemFileCacheInfo as *mut _,
            size_of::<SystemFileCacheInfo>() as u32,
            std::ptr::null_mut(),
        );
        if status == 0 {
            (info.current_size as u64, info.peak_size as u64)
        } else {
            (0, 0)
        }
    }
}
