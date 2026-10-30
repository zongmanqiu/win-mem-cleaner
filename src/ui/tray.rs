//! 原生 Win32 系统托盘 + 后台消息循环。
//!
//! 后台线程创建隐藏窗口与托盘图标，独立 Win32 消息循环。
//! - 托盘动作通过 `PostMessage` 发给主窗口：显示 / 按档位清理 / 退出
//! - 图标实时显示「物理内存占用 %」，颜色随占用变（绿→黄→红），仅百分比变化才重绘
//! - tooltip 实时显示 物理/页面/系统缓存 三项指标

#![allow(non_snake_case)]

use std::mem::size_of;
use std::thread;
use std::time::Instant;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM, BOOL};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
    GetStockObject, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TextOutW, TRANSPARENT,
    DEFAULT_GUI_FONT, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowLongPtrW,
    KillTimer, LoadIconW, PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, TrackPopupMenu, TranslateMessage,
    CS_DBLCLKS, GWLP_USERDATA, HICON, ICONINFO, IDI_APPLICATION, MF_SEPARATOR, MF_STRING,
    MSG, TPM_BOTTOMALIGN, TPM_LEFTALIGN, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_COMMAND,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use crate::core::mem;

/// 托盘动作对应的自定义消息（发给主窗口）。
pub const MSG_TRAY_SHOW: u32 = WM_APP + 1;
pub const MSG_TRAY_CLEAN: u32 = WM_APP + 2;
pub const MSG_TRAY_QUIT: u32 = WM_APP + 3;
pub const MSG_TRAY_ABOUT: u32 = WM_APP + 4;

const WM_TRAY: u32 = WM_APP + 10;

/// TaskbarCreated: Explorer 重启后 Windows 发送此消息，应用须重新添加托盘图标。
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
fn taskbar_created_msg() -> u32 {
    *TASKBAR_CREATED.get_or_init(|| unsafe {
        let name = crate::core::util::wide("TaskbarCreated");
        RegisterWindowMessageW(windows::core::PCWSTR(name.as_ptr()))
    })
}
const ID_SHOW: usize = 2001;
const ID_CLEAN_STD: usize = 2002;
const ID_CLEAN_DEEP: usize = 2003;
const ID_ABOUT: usize = 2006;
const ID_QUIT: usize = 2005;
const TRAY_UID: u32 = 1;
const TRAY_TIMER: usize = 2;

/// 主窗口句柄（由 `spawn` 记录，供托盘回调使用）。
static MAIN_HWND: OnceLock<AtomicUsize> = OnceLock::new();

fn main_hwnd() -> HWND {
    let v = MAIN_HWND.get().map(|a| a.load(Ordering::Relaxed)).unwrap_or(0);
    HWND(v as *mut _)
}

/// 启动托盘后台线程。`main_hwnd` 是主设置窗口句柄。
pub fn spawn(main_hwnd: HWND) {
    let a = MAIN_HWND.get_or_init(|| AtomicUsize::new(0));
    a.store(main_hwnd.0 as usize, Ordering::Relaxed);
    thread::spawn(run_tray_thread);
}

fn post_to_main(msg: u32) {
    unsafe {
        let _ = PostMessageW(main_hwnd(), msg, WPARAM(0), LPARAM(0));
    }
}

/// 发送清理指令，wparam 携带档位（1/2/3）。
fn post_clean(level: usize) {
    unsafe {
        let _ = PostMessageW(main_hwnd(), MSG_TRAY_CLEAN, WPARAM(level), LPARAM(0));
    }
}

fn RGB(r: u32, g: u32, b: u32) -> u32 {
    (b << 16) | (g << 8) | r
}
fn icon_bg_color(pct: u32) -> COLORREF {
    // 三档：≤33% 绿，33%~66% 橙，>66% 红（与进度条一致）
    if pct > 66 {
        COLORREF(RGB(0xD0, 0x30, 0x30))
    } else if pct > 33 {
        COLORREF(RGB(0xE0, 0xA0, 0x10))
    } else {
        COLORREF(RGB(0x30, 0xA0, 0x50))
    }
}

/// 渲染「内存占用 %」到 16x16 图标。数字按字形实际像素边界几何居中（避免 advance 空距导致的偏左）。
fn render_tray_icon(pct: u32) -> HICON {
    unsafe {
        let s: i32 = 16;
        let hdc = GetDC(HWND::default());
        if hdc.is_invalid() {
            return HICON::default();
        }
        // 32bpp top-down DIB，可直接读写像素数组
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = s;
        bi.bmiHeader.biHeight = -s; // 自顶向下
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(hdc, &bi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
        if dib.is_invalid() || bits.is_null() {
            let _ = ReleaseDC(HWND::default(), hdc);
            return HICON::default();
        }
        let px = std::slice::from_raw_parts_mut(bits as *mut u32, (s * s) as usize);
        // icon_bg_color 返回 COLORREF（0x00BBGGRR），而 DIB 32bpp 像素内存序是 BGRA（低字节=蓝）。
        // 若不转换，红色会被误读为蓝色。这里把 RGB 放到正确位置：0x00RRGGBB。
        let cr = icon_bg_color(pct).0;
        let bg = ((cr & 0xFF) << 16) | (cr & 0x00FF00) | ((cr >> 16) & 0xFF);
        for p in px.iter_mut() {
            *p = bg;
        }

        // 用 GDI 把白色数字画到 DIB
        let mdc = CreateCompatibleDC(hdc);
        let _ = SelectObject(mdc, dib);
        let _ = SetBkMode(mdc, TRANSPARENT);
        let _ = SetTextColor(mdc, COLORREF(RGB(0xFF, 0xFF, 0xFF)));
        let font = GetStockObject(DEFAULT_GUI_FONT);
        let _ = SelectObject(mdc, font);
        let text = util_wide(&format!("{pct}"));

        // 先画到临时位置，测量字形边界，再居中重画
        let _ = TextOutW(mdc, 0, 0, &text);
        // 扫描字形边界
        let mut lo = s; let mut hi = -1i32;
        let mut tr = s; let mut bo = -1i32;
        for row in 0..s {
            for col in 0..s {
                let v = px[(row * s + col) as usize];
                let b = (v & 0xFF) as u8;
                let g = ((v >> 8) & 0xFF) as u8;
                let r = ((v >> 16) & 0xFF) as u8;
                // 白色或近白色像素 = 文字
                if r > 200 && g > 200 && b > 200 {
                    if col < lo { lo = col; }
                    if col > hi { hi = col; }
                    if row < tr { tr = row; }
                    if row > bo { bo = row; }
                }
            }
        }
        // 清除旧位置，居中重画
        if lo <= hi && tr <= bo {
            let gw = hi - lo + 1;
            let gh = bo - tr + 1;
            let dx = (s - gw) / 2 - lo;
            let dy = (s - gh) / 2 - tr;
            // 清除整个背景
            for p in px.iter_mut() { *p = bg; }
            // 在居中位置重画
            let _ = TextOutW(mdc, dx, dy, &text);
        }

        let mask = CreateBitmap(s, s, 1, 1, None);
        let ii = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: dib,
        };
        let icon = CreateIconIndirect(&ii).unwrap_or_default();

        let _ = DeleteObject(dib);
        let _ = DeleteObject(mask);
        let _ = DeleteDC(mdc);
        let _ = ReleaseDC(HWND::default(), hdc);

        icon
    }
}

fn util_wide(s: &str) -> Vec<u16> {
    crate::core::util::wide(s)
}

/// 托盘窗口附加状态。
struct TrayState {
    last_pct: u32,
    icon: HICON,
    /// 图标当前是否真的进了通知区。登录自启时 Explorer 的任务栏常常还没建好，
    /// 第一次 `NIM_ADD` 会失败；失败只记状态，由定时器继续重试。
    added: bool,
}

/// 托盘启动时刻，用于 tooltip 上的「已运行」——一眼看出这个常驻进程到底在不在岗。
static START: OnceLock<Instant> = OnceLock::new();

fn uptime_text() -> String {
    let secs = START.get_or_init(Instant::now).elapsed().as_secs();
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    if d > 0 {
        format!("{d}天{h}时")
    } else if h > 0 {
        format!("{h}时{m}分")
    } else {
        format!("{m}分")
    }
}

/// 三项指标 + 运行时长。`szTip` 上限 127 字符，这里刻意保持简短。
fn build_tip() -> String {
    let snap = mem::memory_snapshot();
    format!(
        "物理内存  {} / {}  ({}%)\n虚拟内存  {} / {}  ({}%)\n系统缓存  {} / {}  ({}%)\n已运行 {}",
        mem::format_bytes(snap.phys_used), mem::format_bytes(snap.phys_total), snap.phys_percent,
        mem::format_bytes(snap.page_used), mem::format_bytes(snap.page_total), snap.page_percent,
        mem::format_bytes(snap.cache_bytes), mem::format_bytes(snap.phys_total), snap.cache_percent,
        uptime_text(),
    )
}

/// 把 tooltip 写进 `NOTIFYICONDATAW`（按字符截断，绝不切半个汉字）。
fn fill_tip(nid: &mut NOTIFYICONDATAW, tip: &str) {
    let wide = crate::core::util::wide(tip);
    let copy_len = wide.len().min(127);
    nid.szTip[..copy_len].copy_from_slice(&wide[..copy_len]);
    nid.szTip[copy_len - 1] = 0;
}

fn tick_update(hwnd: HWND) {
    unsafe {
        let st = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
        if st.is_null() {
            return;
        }
        if !(*st).added {
            (*st).added = add_tray_icon(hwnd, (*st).icon);
            if !(*st).added {
                return; // shell 还没就绪，下一拍再试
            }
        }
        let st = &mut *st;
        let snap = mem::memory_snapshot();
        let pct = snap.phys_percent;

        // 重新渲染 tooltip（每秒更新三项指标 + 运行时长）
        let tip = build_tip();

        let mut modify_flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        if st.last_pct != pct {
            let new_icon = render_tray_icon(pct);
            if !st.icon.is_invalid() {
                let _ = DestroyIcon(st.icon);
            }
            st.icon = new_icon;
            st.last_pct = pct;
        } else {
            // 百分比未变，只需更新 tooltip，不重绘图标
            modify_flags = NIF_TIP;
        }

        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: modify_flags,
            uCallbackMessage: WM_TRAY,
            hIcon: st.icon,
            ..Default::default()
        };
        fill_tip(&mut nid, &tip);
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

fn run_tray_thread() {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("MemCleanerTrayWindow");
        let _ = RegisterClassW(&WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        });
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("MemCleanerTray"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            hinstance,
            None,
        ) else {
            return;
        };
        if hwnd.0.is_null() {
            return;
        }

        // 初始状态
        let initial_pct = mem::physical_usage_percent();
        let initial_icon = render_tray_icon(initial_pct);
        let st = Box::into_raw(Box::new(TrayState {
            last_pct: initial_pct,
            icon: initial_icon,
            added: false,
        }));
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, st as isize);

        // 登录自启时 Explorer 的任务栏常常还没建好，第一次 NIM_ADD 会失败。
        // 失败不能 return：线程一退出，隐藏窗口和消息循环一起没了，之后既不会重试，
        // 也收不到 TaskbarCreated——整轮登录都没有托盘图标（进程还在，只是没有入口）。
        (*st).added = add_tray_icon(hwnd, (*st).icon);
        // 每秒刷新图标 + tooltip；图标还没加上的那几秒顺带重试 NIM_ADD
        let _ = SetTimer(hwnd, TRAY_TIMER, 1000, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // 清理
        let _ = KillTimer(hwnd, TRAY_TIMER);
        let st = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
        if !st.is_null() {
            let boxed: Box<TrayState> = Box::from_raw(st);
            if !boxed.icon.is_invalid() {
                let _ = DestroyIcon(boxed.icon);
            }
        }
        delete_tray_icon(hwnd);
    }
}

fn add_tray_icon(hwnd: HWND, icon: HICON) -> bool {
    unsafe {
        let icon = if icon.is_invalid() {
            LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
        } else {
            icon
        };
        // 初始 tooltip（与每秒刷新同一份构造，避免两处漂移）
        let tip = build_tip();
        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        fill_tip(&mut nid, &tip);
        Shell_NotifyIconW(NIM_ADD, &nid).as_bool()
    }
}

fn delete_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Explorer 重启 → 重新添加托盘图标；这一拍没成就交给定时器继续重试
    if msg == taskbar_created_msg() {
        delete_tray_icon(hwnd);
        let st = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
        let icon = if st.is_null() { HICON::default() } else { (*st).icon };
        let added = add_tray_icon(hwnd, icon);
        if !st.is_null() {
            (*st).added = added;
        }
        return LRESULT(0);
    }
    match msg {
        WM_TRAY => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP {
                show_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                post_to_main(MSG_TRAY_SHOW);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xffff {
                ID_SHOW => post_to_main(MSG_TRAY_SHOW),
                ID_CLEAN_STD => post_clean(1),
                ID_CLEAN_DEEP => post_clean(2),
                ID_ABOUT => post_to_main(MSG_TRAY_ABOUT),
                ID_QUIT => {
                    post_to_main(MSG_TRAY_QUIT);
                    PostQuitMessage(0);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TRAY_TIMER {
                tick_update(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let menu = CreatePopupMenu().unwrap_or_default();
    let show = crate::core::util::wide("窗口");
    let std = crate::core::util::wide("标准清理");
    let deep = crate::core::util::wide("深度清理");
    let about = crate::core::util::wide("关于");
    let quit = crate::core::util::wide("退出");
    let _ = AppendMenuW(menu, MF_STRING, ID_SHOW, windows::core::PCWSTR(show.as_ptr()));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_CLEAN_STD, windows::core::PCWSTR(std.as_ptr()));
    let _ = AppendMenuW(menu, MF_STRING, ID_CLEAN_DEEP, windows::core::PCWSTR(deep.as_ptr()));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, ID_ABOUT, windows::core::PCWSTR(about.as_ptr()));
    let _ = AppendMenuW(menu, MF_STRING, ID_QUIT, windows::core::PCWSTR(quit.as_ptr()));
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, None);
    let _ = DestroyMenu(menu);
}
