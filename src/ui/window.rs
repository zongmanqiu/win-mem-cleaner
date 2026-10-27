//! 主窗口 —— UI 创建、窗口过程、控件、自绘进度条。
//!
//! 从 main.rs 提取，遵循 Pecia 的 core/ui 分离架构。

#![allow(non_snake_case)]

use std::env;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM, BOOL,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, CreateFontIndirectW, DeleteObject, DrawTextW, EndPaint, FillRect,
    GetDC, GetSysColorBrush, GetTextExtentPoint32W, InvalidateRect, PAINTSTRUCT, ReleaseDC,
    SelectObject, SetBkMode, SetTextColor, COLOR_WINDOW,
    HBRUSH, HFONT, DT_CENTER, DT_SINGLELINE, DT_VCENTER, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetPriorityClass, SetPriorityClass, PROCESS_CREATION_FLAGS,
    BELOW_NORMAL_PRIORITY_CLASS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_GROUPBOX,
    CreateWindowExW, DefWindowProcW,
    DispatchMessageW, EnumChildWindows, GetClientRect, GetDlgItem, GetMessageW, GetWindowLongPtrW,
    GetWindowTextLengthW, GetWindowTextW, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, SystemParametersInfoW,
    TranslateMessage, CS_DBLCLKS, ES_NUMBER, GWLP_USERDATA, HWND_TOP, MSG,
    NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SPI_GETWORKAREA, SW_HIDE, SW_SHOW,
    SWP_NOSIZE, SWP_NOZORDER, SWP_NOMOVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY, WM_PAINT, WM_SETICON, WM_SETFONT, WM_TIMER,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_MINIMIZEBOX, WS_OVERLAPPED,
    WS_SYSMENU, WS_VISIBLE, WS_EX_CLIENTEDGE, HICON,
};

use crate::core::config::AppConfig;

// ---------------------------------------------------------------------------
// 控件 ID
// ---------------------------------------------------------------------------
const IDC_BAR_PHYS: i32 = 3001;
const IDC_BAR_PAGE: i32 = 3002;
const IDC_BAR_CACHE: i32 = 3003;
const IDC_INFO_PHYS: i32 = 3011;
const IDC_INFO_PAGE: i32 = 3012;
const IDC_INFO_CACHE: i32 = 3013;

const IDC_INTERVAL_EDIT: i32 = 2101;
const IDC_THRESHOLD_EDIT: i32 = 2103;

const IDC_AUTOSTART: i32 = 2007;

/// 两行方式各自的「上次自动清理」时刻显示位（从未执行显示 `--:--`）。
const IDC_LAST_INTERVAL: i32 = 9008;
const IDC_LAST_THRESHOLD: i32 = 9009;

/// EDIT 控件失去焦点通知（winuser.h: EN_KILLFOCUS = 0x0200）。
const EN_KILLFOCUS: i32 = 0x0200;

const WIN_W: i32 = 300; // 最小宽度，运行时自动调整
const WIN_H: i32 = 202;
const MARGIN: i32 = 14; // 四周统一边缘间隔
const TIMER_ID: usize = 1;
const TIMER_MS: u32 = 1000;

// ---------------------------------------------------------------------------
// 共享状态
// ---------------------------------------------------------------------------
struct SharedState {
    cfg: Arc<RwLock<AppConfig>>,
    quit: AtomicBool,
    hwnd: AtomicUsize,
}
impl SharedState {
    fn new(cfg: AppConfig) -> Arc<Self> {
        Arc::new(Self {
            cfg: Arc::new(RwLock::new(cfg)),
            quit: AtomicBool::new(false),
            hwnd: AtomicUsize::new(0),
        })
    }
    fn set_hwnd(&self, h: HWND) {
        self.hwnd.store(h.0 as usize, Ordering::Relaxed);
    }
}
static SHARED: OnceLock<Arc<SharedState>> = OnceLock::new();
/// 保持系统 UI 字体句柄存活（存地址，Avoid HFONT 的 Send/Sync 要求）。
pub(crate) static KEEP_FONT: OnceLock<usize> = OnceLock::new();
fn shared() -> &'static Arc<SharedState> {
    SHARED.get().expect("SHARED not initialized")
}

// ---------------------------------------------------------------------------
// 自绘颜色条
// ---------------------------------------------------------------------------
fn RGB(r: u32, g: u32, b: u32) -> u32 {
    (b << 16) | (g << 8) | r
}
fn bar_color(pct: u32) -> u32 {
    if pct > 66 {
        RGB(0xD0, 0x30, 0x30)
    } else if pct > 33 {
        RGB(0xE0, 0xA0, 0x10)
    } else {
        RGB(0x30, 0xA0, 0x50)
    }
}
unsafe extern "system" fn bar_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let w = rc.right - rc.left;
        let bg = CreateSolidBrush(COLORREF(RGB(0x28, 0x28, 0x28)));
        let _ = FillRect(hdc, &rc, bg);
        let _ = DeleteObject(bg);
        let pct = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u32;
        let fill_w = (w as u64 * pct as u64 / 100) as i32;
        if fill_w > 0 {
            let fr = RECT {
                left: rc.left,
                top: rc.top,
                right: rc.left + fill_w,
                bottom: rc.bottom,
            };
            let fg = CreateSolidBrush(COLORREF(bar_color(pct)));
            let _ = FillRect(hdc, &fr, fg);
            let _ = DeleteObject(fg);
        }
        let pct = pct.min(100);
        if let Some(fh) = KEEP_FONT.get() {
            let _ = SelectObject(hdc, HFONT(*fh as *mut _));
        }
        let _ = SetTextColor(hdc, COLORREF(RGB(0xFF, 0xFF, 0xFF)));
        let _ = SetBkMode(hdc, TRANSPARENT);
        let mut txt = crate::core::util::wide(&format!("{}%", pct));
        let _ = DrawTextW(hdc, &mut txt, &mut rc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
        let _ = EndPaint(hwnd, &ps);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
fn register_bar_class(hinst: windows::Win32::Foundation::HINSTANCE) {
    unsafe {
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(bar_wndproc),
            hInstance: hinst.into(),
            lpszClassName: w!("MemCleanBar"),
            hbrBackground: HBRUSH(
                CreateSolidBrush(COLORREF(RGB(0x28, 0x28, 0x28))).0 as isize as *mut _,
            ),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
    }
}
fn make_bar(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("MemCleanBar"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize as *mut _),
            None,
            None,
        )
        .unwrap_or_default()
    }
}
fn bar_set(hwnd: HWND, pct: u32) {
    unsafe {
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, pct as isize);
        let _ = InvalidateRect(hwnd, None, true);
    }
}

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------
fn ws(v: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(v as u32)
}
pub(crate) fn dlg(hwnd: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(hwnd, id).unwrap_or_default() }
}
fn make_child(
    hwnd: HWND,
    class: PCWSTR,
    text: PCWSTR,
    style: WINDOW_STYLE,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> HWND {
    unsafe {
        let child = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            text,
            style | WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize as *mut _),
            None,
            None,
        )
        .unwrap_or_default();
        set_child_font(child);
        child
    }
}

/// 控件**自己**在创建时就要拿到界面字体。
/// 不能只靠 `apply_ui_font` 事后 `EnumChildWindows` 补一遍——那样字体一设就得在布局之前，
/// 而布局又要量字体宽度，鸡生蛋；实测把顺序换过来之后所有控件退回系统默认字体（更宽），
/// 末位字符被裁成 "15.8 G"。
fn set_child_font(child: HWND) {
    if let Some(&fh) = KEEP_FONT.get() {
        unsafe {
            let _ = SendMessageW(child, WM_SETFONT, WPARAM(fh), LPARAM(1));
        }
    }
}
fn lbl(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    make_child(hwnd, w!("STATIC"), PCWSTR::null(), ws(0x200), id, x, y, w, h)
}
fn chk(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32, text: &str) -> HWND {
    let t = crate::core::util::wide(text);
    make_child(hwnd, w!("BUTTON"), PCWSTR(t.as_ptr()), ws(BS_AUTOCHECKBOX), id, x, y, w, h)
}
fn groupbox(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32, text: &str) -> HWND {
    let t = crate::core::util::wide(text);
    make_child(hwnd, w!("BUTTON"), PCWSTR(t.as_ptr()), ws(BS_GROUPBOX), id, x, y, w, h)
}
fn edt(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    unsafe {
        let child = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            ws(ES_NUMBER) | WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize as *mut _),
            None,
            None,
        )
        .unwrap_or_default();
        set_child_font(child);
        child
    }
}
fn set_text(hwnd: HWND, id: i32, text: &str) {
    unsafe {
        let h = dlg(hwnd, id);
        if !h.0.is_null() {
            let _ = SetWindowTextW(h, PCWSTR(crate::core::util::wide(text).as_ptr()));
        }
    }
}
fn text_of(hwnd: HWND, id: i32) -> String {
    unsafe {
        let h = dlg(hwnd, id);
        let len = GetWindowTextLengthW(h);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        GetWindowTextW(h, &mut buf);
        crate::core::util::wide_to_string(&buf)
    }
}
fn checked(hwnd: HWND, id: i32) -> bool {
    unsafe {
        let h = dlg(hwnd, id);
        SendMessageW(h, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 != 0
    }
}
fn set_checked(hwnd: HWND, id: i32, on: bool) {
    unsafe {
        let h = dlg(hwnd, id);
        let _ = SendMessageW(h, BM_SETCHECK, WPARAM(if on { 1 } else { 0 }), LPARAM(0));
    }
}
fn set_edit(hwnd: HWND, id: i32, v: u32) {
    let s = format!("{v}");
    unsafe {
        let h = dlg(hwnd, id);
        let _ = SetWindowTextW(h, PCWSTR(crate::core::util::wide(&s).as_ptr()));
    }
}
fn edit_val(hwnd: HWND, id: i32) -> Option<u32> {
    text_of(hwnd, id).trim().parse::<u32>().ok()
}

// ---------------------------------------------------------------------------
// Logo 加载（从嵌入的 ICO 资源加载）
// ---------------------------------------------------------------------------
fn load_logo_icon() -> HICON {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR};
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let hinst: windows::Win32::Foundation::HINSTANCE =
            windows::Win32::Foundation::HINSTANCE(hinst.0);
        let icon = LoadImageW(
            hinst,
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTCOLOR,
        );
        match icon {
            Ok(h) => HICON(h.0 as *mut _),
            Err(_) => HICON::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// 刷新信息
// ---------------------------------------------------------------------------
fn refresh_info(hwnd: HWND) {
    let s = crate::core::mem::memory_snapshot();
    bar_set(dlg(hwnd, IDC_BAR_PHYS), s.phys_percent);
    bar_set(dlg(hwnd, IDC_BAR_PAGE), s.page_percent);
    bar_set(dlg(hwnd, IDC_BAR_CACHE), s.cache_percent);
    set_text(
        hwnd,
        IDC_INFO_PHYS,
        &format!(
            "物理内存 {} / {}",
            crate::core::mem::format_bytes(s.phys_used),
            crate::core::mem::format_bytes(s.phys_total),
        ),
    );
    set_text(
        hwnd,
        IDC_INFO_PAGE,
        &format!(
            "虚拟内存 {} / {}",
            crate::core::mem::format_bytes(s.page_used),
            crate::core::mem::format_bytes(s.page_total),
        ),
    );
    set_text(
        hwnd,
        IDC_INFO_CACHE,
        &format!(
            "系统缓存 {} / {}",
            crate::core::mem::format_bytes(s.cache_bytes),
            crate::core::mem::format_bytes(s.phys_total),
        ),
    );

    // 两行各自的「上次自动清理」时刻；本次运行以来没跑过就是 --:--
    use crate::core::scheduler::Slot;
    for (id, slot) in [
        (IDC_LAST_INTERVAL, Slot::Interval),
        (IDC_LAST_THRESHOLD, Slot::Threshold),
    ] {
        let t = match crate::core::scheduler::last_run(slot) {
            Some((h, m, _)) => format!("{h:02}:{m:02}"),
            None => "--:--".to_string(),
        };
        set_text(hwnd, id, &t);
    }
}

// ---------------------------------------------------------------------------
// 清理（按档位后台执行）
// ---------------------------------------------------------------------------
fn run_clean_level(level: u32) {
    std::thread::spawn(move || {
        let prev = lower_self_priority();
        let _ = crate::core::mem::clean_by_level(level);
        restore_self_priority(prev);
    });
}

fn lower_self_priority() -> PROCESS_CREATION_FLAGS {
    unsafe {
        let h = GetCurrentProcess();
        let prev = GetPriorityClass(h);
        let _ = SetPriorityClass(h, BELOW_NORMAL_PRIORITY_CLASS);
        PROCESS_CREATION_FLAGS(prev)
    }
}

fn restore_self_priority(prev: PROCESS_CREATION_FLAGS) {
    unsafe {
        let h = GetCurrentProcess();
        let _ = SetPriorityClass(h, prev);
    }
}

// ---------------------------------------------------------------------------
// 开机启动
// ---------------------------------------------------------------------------
/// 计划任务名。本程序清单是 `requireAdministrator`，走 `HKCU\Run` 会在**登录时弹 UAC**，
/// 没人点就不启动——这是「常驻」最不可靠的一条路。计划任务 + `/rl HIGHEST` 才是正解。
const TASK_NAME: &str = "WinMemCleaner";
const RUN_VALUE: &str = "memclean";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn schtasks(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW) // 别在用户桌面上闪一个控制台
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 注册/刷新计划任务（登录触发、最高权限）。失败返回 false，由调用方退回 Run 键。
fn schedule_task(exe: &std::path::Path) -> bool {
    let action = format!("\"{}\" silent", exe.display());
    schtasks(&[
        "/create", "/f", "/tn", TASK_NAME,
        "/sc", "ONLOGON", "/rl", "HIGHEST", "/tr", &action,
    ])
}

fn delete_task() {
    schtasks(&["/delete", "/f", "/tn", TASK_NAME]);
}

/// 写/删 `HKCU\...\Run` 里的启动项。`None` = 删除。
fn write_run_key(cmd: Option<&str>) {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPEN_CREATE_OPTIONS, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
    };
    let key = crate::core::util::wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let value = crate::core::util::wide(RUN_VALUE);
    unsafe {
        let mut hkey = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            0,
            None,
            REG_OPEN_CREATE_OPTIONS(0),
            KEY_READ | KEY_WRITE,
            None,
            &mut hkey,
            None,
        )
        .is_err()
        {
            return;
        }
        match cmd {
            Some(cmd) => {
                let mut data = crate::core::util::wide(cmd);
                data.push(0);
                let bytes: &[u8] =
                    std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
                let _ = RegSetValueExW(hkey, PCWSTR(value.as_ptr()), 0, REG_SZ, Some(bytes));
            }
            None => {
                let _ = RegDeleteValueW(hkey, PCWSTR(value.as_ptr()));
            }
        }
        let _ = RegCloseKey(hkey);
    }
}

/// 设置开机启动。计划任务优先；建不了才退回 Run 键。
/// 两条路只留一条——同时存在会在开机时拉起两个实例去抢单实例互斥。
fn set_autostart(enable: bool) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    if enable {
        if schedule_task(&exe) {
            write_run_key(None); // 迁移掉老版本的 Run 项
        } else {
            write_run_key(Some(&format!("\"{}\" silent", exe.display())));
        }
    } else {
        delete_task();
        write_run_key(None);
    }
}

// ---------------------------------------------------------------------------
// 界面同步 / 保存
// ---------------------------------------------------------------------------
fn sync_ui(hwnd: HWND, c: &AppConfig) {
    set_edit(hwnd, IDC_INTERVAL_EDIT, c.interval_minutes);
    set_edit(hwnd, IDC_THRESHOLD_EDIT, c.threshold_percent);
    set_checked(hwnd, IDC_AUTOSTART, c.autostart);
}

fn save_from_ui(hwnd: HWND) {
    let mut c = shared().cfg.read().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(v) = edit_val(hwnd, IDC_INTERVAL_EDIT) {
        c.interval_minutes = v;
    }
    if let Some(v) = edit_val(hwnd, IDC_THRESHOLD_EDIT) {
        c.threshold_percent = v;
    }
    c.autostart = checked(hwnd, IDC_AUTOSTART);
    c.sanitize();
    *shared().cfg.write().unwrap_or_else(|e| e.into_inner()) = c.clone();
    crate::core::config::save(&c);
    sync_ui(hwnd, &c);
}

// ---------------------------------------------------------------------------
// 窗口过程
// ---------------------------------------------------------------------------
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            shared().set_hwnd(hwnd);
            // 字体必须先于布局：`create_ui` 要用 `measure_text_width` 量出窗口宽度，
            // 而它读的是 `KEEP_FONT`。顺序颠倒 ⇒ 用默认字体量 ⇒ 整个窗口宽度算错。
            apply_ui_font(hwnd);
            create_ui(hwnd);
            refresh_info(hwnd);
            let _ = SetTimer(hwnd, TIMER_ID, TIMER_MS, None);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_ID {
                refresh_info(hwnd);
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let br = GetSysColorBrush(COLOR_WINDOW);
            LRESULT(br.0 as isize)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as i32;
            let notify = (wparam.0 >> 16) as i32;
            match id {
                IDC_INTERVAL_EDIT | IDC_THRESHOLD_EDIT => {
                    if notify == EN_KILLFOCUS {
                        save_from_ui(hwnd);
                    }
                }
                IDC_AUTOSTART => {
                    let on = checked(hwnd, IDC_AUTOSTART);
                    set_autostart(on);
                    save_from_ui(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            // 关窗只是隐藏到托盘——先存盘，否则「敲完数字直接点 X」会把设置丢掉
            save_from_ui(hwnd);
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_SHOW => {
            position_bottom_right(hwnd);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_CLEAN => {
            run_clean_level(wparam.0 as u32);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_QUIT => {
            save_from_ui(hwnd); // 退出前同样存一次，别让最后一次改动蒸发
            shared().quit.store(true, Ordering::Relaxed);
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_ABOUT => {
            super::about::show_about_dialog(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------------------
// 文本宽度测量（用于自动计算窗口宽度）
// ---------------------------------------------------------------------------
fn measure_text_width(hwnd: HWND, text: &str) -> i32 {
    unsafe {
        let hdc = GetDC(hwnd);
        if let Some(&fh) = KEEP_FONT.get() {
            let _ = SelectObject(hdc, HFONT(fh as *mut _));
        }
        let wide = crate::core::util::wide(text);
        // `util::wide` 末尾带 NUL，而 windows-rs 把切片长度当作 Win32 的字符数 `c` 传下去——
        // 不去掉这枚 NUL 就会多量一个字模的推进宽度
        let chars = &wide[..wide.len().saturating_sub(1)];
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, chars, &mut size);
        let _ = ReleaseDC(hwnd, hdc);
        size.cx
    }
}

// ---------------------------------------------------------------------------
// 界面创建
// ---------------------------------------------------------------------------
fn create_ui(hwnd: HWND) {
    let cfg = shared().cfg.read().unwrap_or_else(|e| e.into_inner()).clone();
    let bar_w = 100;
    let bar_gap = 10;
    let row = 24;

    let s = crate::core::mem::memory_snapshot();
    let texts = [
        format!("物理内存 {} / {}", crate::core::mem::format_bytes(s.phys_used), crate::core::mem::format_bytes(s.phys_total)),
        format!("虚拟内存 {} / {}", crate::core::mem::format_bytes(s.page_used), crate::core::mem::format_bytes(s.page_total)),
        format!("系统缓存 {} / {}", crate::core::mem::format_bytes(s.cache_bytes), crate::core::mem::format_bytes(s.phys_total)),
    ];
    let max_text_w = texts.iter().map(|t| measure_text_width(hwnd, t)).max().unwrap_or(160);
    // 下面所有坐标都是**客户区**坐标，最后一步才换算成外框尺寸交给 SetWindowPos。
    // 混用两者是本窗口一直存在的布局错误：分组框右线被画到窗口边框上（右边距实测 0）。
    let client_w = (MARGIN + bar_w + bar_gap + max_text_w + MARGIN).max(300);

    let y1 = MARGIN;
    let y2 = y1 + row;
    let y3 = y2 + row;
    let x = MARGIN;
    let text_x = x + bar_w + bar_gap;
    let text_w = client_w - MARGIN - text_x;

    make_bar(hwnd, IDC_BAR_PHYS, x, y1, bar_w, 16);
    lbl(hwnd, IDC_INFO_PHYS, text_x, y1, text_w, 16);
    make_bar(hwnd, IDC_BAR_PAGE, x, y2, bar_w, 16);
    lbl(hwnd, IDC_INFO_PAGE, text_x, y2, text_w, 16);
    make_bar(hwnd, IDC_BAR_CACHE, x, y3, bar_w, 16);
    lbl(hwnd, IDC_INFO_CACHE, text_x, y3, text_w, 16);

    let box_x = x;
    let box_w = client_w - MARGIN * 2;
    let box_y = y3 + row;
    let box_h = 66;
    groupbox(hwnd, 9001, box_x, box_y, box_w, box_h, "自动清理");

    // 两种方式各占一行，列位置对齐：条件 / 数值 / 单位 / 强度 / 动作
    let col_cond = box_x + 12;
    let col_edit = box_x + 64;
    let col_unit = box_x + 100;
    let col_act = box_x + 130;
    // 上次自动清理时刻：贴着分组框右缘右对齐，窗口变宽也不会错位
    let col_time = box_x + box_w - 12 - 40;

    // 方式一：每 x 分钟 标准清理（档位固定，直接写在文字里）
    let row1 = box_y + 20;
    lbl(hwnd, 9002, col_cond, row1, 48, 18);
    set_text(hwnd, 9002, "每");
    edt(hwnd, IDC_INTERVAL_EDIT, col_edit, row1 - 1, 32, 18);
    lbl(hwnd, 9003, col_unit, row1, 26, 18);
    set_text(hwnd, 9003, "分钟");
    lbl(hwnd, 9005, col_act, row1, 60, 18);
    set_text(hwnd, 9005, "标准清理");
    lbl(hwnd, IDC_LAST_INTERVAL, col_time, row1, 40, 18);

    // 方式二：内存 > x % 深度清理
    let row2 = row1 + 22;
    lbl(hwnd, 9004, col_cond, row2, 48, 18);
    set_text(hwnd, 9004, "内存 >");
    edt(hwnd, IDC_THRESHOLD_EDIT, col_edit, row2 - 1, 32, 18);
    lbl(hwnd, 9006, col_unit, row2, 26, 18);
    set_text(hwnd, 9006, "%");
    lbl(hwnd, 9007, col_act, row2, 60, 18);
    set_text(hwnd, 9007, "深度清理");
    lbl(hwnd, IDC_LAST_THRESHOLD, col_time, row2, 40, 18);

    let chk_y = box_y + box_h + 6;
    chk(hwnd, IDC_AUTOSTART, box_x, chk_y, 90, 18, "开机启动");

    sync_ui(hwnd, &cfg);

    let client_h = chk_y + 18 + MARGIN;
    // 外框 − 客户区的差值**实测**，不写死：边框宽度和标题栏高度都随系统版本 / 主题 / DPI 变。
    // 之前这里把外框尺寸当客户区尺寸用，还硬编码了 `+31` 当标题栏高度。
    let (fw, fh) = frame_delta(hwnd);
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            client_w + fw,
            client_h + fh,
            SWP_NOMOVE | SWP_NOZORDER,
        );
    }
}

/// 外框尺寸 − 客户区尺寸（宽 / 高）。窗口还没定稿时调用也安全：拿的是当前样式下的真实差值。
fn frame_delta(hwnd: HWND) -> (i32, i32) {
    unsafe {
        let mut wr = RECT::default();
        let mut cr = RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wr);
        let _ = GetClientRect(hwnd, &mut cr);
        let fw = (wr.right - wr.left) - (cr.right - cr.left);
        let fh = (wr.bottom - wr.top) - (cr.bottom - cr.top);
        (fw.max(0), fh.max(0))
    }
}

// ---------------------------------------------------------------------------
// 窗口位置
// ---------------------------------------------------------------------------
fn position_bottom_right(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut _),
            windows::Win32::UI::WindowsAndMessaging::SPIF_SENDCHANGE,
        );
        let wa_left = rect.left as i32;
        let wa_top = rect.top as i32;
        let wa_w = rect.right - rect.left;
        let wa_h = rect.bottom - rect.top;
        let mut wrect = RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wrect);
        let w_w = wrect.right - wrect.left;
        let w_h = wrect.bottom - wrect.top;
        let m = 10i32;
        let x = wa_left + wa_w - w_w - m;
        let y = wa_top + wa_h - w_h - m;
        let _ = SetWindowPos(hwnd, HWND_TOP, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    }
}

// ---------------------------------------------------------------------------
// UI 字体
// ---------------------------------------------------------------------------
fn apply_ui_font(hwnd: HWND) {
    unsafe {
        let mut ncm = NONCLIENTMETRICSW::default();
        ncm.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;
        if SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut _),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_err()
        {
            return;
        }
        let font = CreateFontIndirectW(&ncm.lfMessageFont);
        if font.is_invalid() {
            return;
        }
        let _ = KEEP_FONT.set(font.0 as usize);
        let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        let _ = EnumChildWindows(hwnd, Some(enum_child_setfont), LPARAM(font.0 as isize));
    }
}

unsafe extern "system" fn enum_child_setfont(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let font = HFONT(lparam.0 as *mut _);
    let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    BOOL(1)
}

fn init_common_controls() {
    unsafe {
        let icc = windows::Win32::UI::Controls::INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<windows::Win32::UI::Controls::INITCOMMONCONTROLSEX>()
                as u32,
            dwICC: windows::Win32::UI::Controls::ICC_STANDARD_CLASSES
                | windows::Win32::UI::Controls::ICC_WIN95_CLASSES,
        };
        let _ = windows::Win32::UI::Controls::InitCommonControlsEx(&icc);
    }
}

// ---------------------------------------------------------------------------
// 入口（由 main.rs 调用）
// ---------------------------------------------------------------------------
pub fn run(cfg: AppConfig) {
    let shared = SharedState::new(cfg);
    let _ = SHARED.set(shared.clone());

    // 自愈：用户勾了开机启动，就把启动项刷成**当前这个 exe 的路径**。
    // 否则改了产物名 / 换了目录后，注册表里会留一条指向旧副本的死项——
    // 表现为「我明明设了自动清理，可它从来没跑」。
    if shared.cfg.read().map(|g| g.autostart).unwrap_or(false) {
        set_autostart(true);
    }

    let scheduler = crate::core::scheduler::Scheduler::new(shared.cfg.clone());
    scheduler.start();

    init_common_controls();

    unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        register_bar_class(hinst.into());

        let class_name = w!("MemCleanerMainWindow");
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            lpszClassName: class_name,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 as i32 + 1) as isize as *mut _),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let style: WINDOW_STYLE = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("WinMemCleaner"),
            style | WS_VISIBLE,
            -100,
            -100,
            WIN_W,
            WIN_H,
            None,
            None,
            hinst,
            None,
        ) else {
            return;
        };
        let title = "WinMemCleaner";
        let _ = SetWindowTextW(hwnd, PCWSTR(crate::core::util::wide(title).as_ptr()));

        let logo = load_logo_icon();
        if !logo.is_invalid() {
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(0), LPARAM(logo.0 as isize));
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(1), LPARAM(logo.0 as isize));
        }
        position_bottom_right(hwnd);

        let silent = env::args().any(|a| a == "silent" || a == "slient");
        if silent {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }

        super::tray::spawn(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
