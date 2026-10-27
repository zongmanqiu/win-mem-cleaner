//! Windows 内存清理核心。
//! 参考 memreduct / MemCleaner / memory_cleaner 的实现思路，用 Win32/Native API。

#![allow(non_snake_case)]

use std::collections::HashSet;
use std::mem::size_of;

use windows::core::{w, PCSTR, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::SetSystemFileCacheSize;
use windows::Win32::System::ProcessStatus::{
    EmptyWorkingSet, GetPerformanceInfo, GetProcessMemoryInfo, PERFORMANCE_INFORMATION,
    PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
};

// ---------------------------------------------------------------------------
// 常量：未文档化的 Native API
// ---------------------------------------------------------------------------
const SYSTEM_MEMORY_LIST_INFORMATION_CLASS: i32 = 80; // 0x50
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

/// 系统缓存实时字节数（≈ 可回收的缓存 / 备用内存）。
///
/// 用文档化 API `GetPerformanceInfo`——它的每个数值字段单位都是「页」，故乘 `PageSize`。
/// 原先读的是未文档化的 `NtQuerySystemInformation(SystemFileCacheInformation)`，结构布局靠猜：
/// 同机实测 0.36 GB，而文档口径是 4.75 GB，差 13 倍，故弃用。
pub fn system_cache_bytes() -> u64 {
    unsafe {
        let mut pi = PERFORMANCE_INFORMATION {
            cb: size_of::<PERFORMANCE_INFORMATION>() as u32,
            ..Default::default()
        };
        if GetPerformanceInfo(&mut pi, pi.cb).is_err() {
            return 0;
        }
        (pi.SystemCache as u64).saturating_mul(pi.PageSize.max(1) as u64)
    }
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

/// 清空系统文件缓存：把缓存上限临时压到 0 逼系统回收，随后恢复为不限。
///
/// `SetSystemFileCacheSize` 自 Windows 7 起就是 kernel32 的导出，故直接用文档化绑定。
/// 原先那条「取不到导出就用 NtSetSystemInformation 兜底」的分支已删：它把猜出来的
/// 未文档化结构发给 class 80（那是 `SystemMemoryListInformation`，不是文件缓存），
/// 在本项目支持的 Win10+ 上永不可达，一旦真跑到反而会发出错误的内存列表命令。
pub fn clear_system_file_cache() -> bool {
    let _ = enable_privilege("SeIncreaseQuotaPrivilege");
    unsafe {
        let ok = SetSystemFileCacheSize(0, 0, 0).is_ok();
        let _ = SetSystemFileCacheSize(0, usize::MAX, 0); // 恢复默认，交回系统管理
        ok
    }
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
    pub cache_bytes: u64,
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
    // 系统缓存：实时字节，百分比按「占物理内存总量」算
    s.cache_bytes = system_cache_bytes();
    if s.phys_total > 0 {
        s.cache_percent = (s.cache_bytes.saturating_mul(100) / s.phys_total).min(100) as u32;
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
