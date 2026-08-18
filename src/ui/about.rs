//! 关于对话框 —— 版本、作者、仓库链接、赞助二维码。
//!
//! 从 main.rs 提取。

#![allow(non_snake_case)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateDIBSection, GetDC, ReleaseDC, GetSysColorBrush,
    SetTextColor, COLOR_WINDOW, DIB_RGB_COLORS, HFONT,
    BITMAPINFO, BITMAPINFOHEADER,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetClientRect,
    GetDlgCtrlID, GetWindowRect, RegisterClassW, SendMessageW,
    SetWindowLongPtrW, SetWindowPos, SystemParametersInfoW,
    CS_DBLCLKS, HWND_TOP, HMENU, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS,
    SPI_GETWORKAREA, SW_SHOWNORMAL, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_SETFONT,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
};

use super::window::KEEP_FONT;

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------
const IDC_ABOUT_LINK: i32 = 4001;

// 二维码 PNG（由 build.rs 从 SVG 生成）
const WECHAT_PNG: &[u8] = include_bytes!("../../image/WeChatPay.png");
const ALIPAY_PNG: &[u8] = include_bytes!("../../image/ALiPay.png");

static LINK_RECT: OnceLock<(i32, i32, i32, i32)> = OnceLock::new();
static LINK_OLD_PROC: OnceLock<AtomicUsize> = OnceLock::new();

// ---------------------------------------------------------------------------
// 子类过程（链接点击 + 悬停变色）
// ---------------------------------------------------------------------------
unsafe extern "system" fn link_subclass_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
    _subclass_id: usize, _ref_data: usize,
) -> LRESULT {
    let old_proc_addr = LINK_OLD_PROC.get().map(|a| a.load(Ordering::Relaxed)).unwrap_or(0);
    let old_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
        std::mem::transmute(old_proc_addr);
    match msg {
        0x0084 /* WM_NCHITTEST */ => LRESULT(1),
        0x0200 /* WM_MOUSEMOVE */ => {
            use windows::Win32::UI::WindowsAndMessaging::{GetPropW, SetPropW};
            let key = w!("hover");
            let was_hover = GetPropW(hwnd, PCWSTR(key.as_ptr())).0;
            if was_hover == std::ptr::null_mut() {
                let _ = SetPropW(hwnd, PCWSTR(key.as_ptr()), windows::Win32::Foundation::HANDLE(1 as *mut _));
                let _ = windows::Win32::Graphics::Gdi::InvalidateRect(hwnd, None, true);
            }
            old_proc(hwnd, msg, wparam, lparam)
        }
        0x0201 /* WM_LBUTTONDOWN */ => {
            open_gitee_link();
            LRESULT(0)
        }
        _ => old_proc(hwnd, msg, wparam, lparam)
    }
}

fn open_gitee_link() {
    unsafe {
        let url = crate::core::util::wide("https://gitee.com/qiuzongman/win-mem-cleaner");
        let _ = ShellExecuteW(
            HWND::default(), w!("open"),
            PCWSTR(url.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL,
        );
    }
}

// ---------------------------------------------------------------------------
// 窗口过程
// ---------------------------------------------------------------------------
unsafe extern "system" fn about_wndproc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as i32;
            if id == IDC_ABOUT_LINK {
                open_gitee_link();
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
            let ctrl = HWND(lparam.0 as *mut _);
            let ctrl_id = GetDlgCtrlID(ctrl);
            if ctrl_id == IDC_ABOUT_LINK {
                use windows::Win32::UI::WindowsAndMessaging::GetPropW;
                let key = w!("hover");
                let is_hover = GetPropW(ctrl, PCWSTR(key.as_ptr())).0 as usize != 0;
                if is_hover {
                    let _ = SetTextColor(hdc, COLORREF(0x000000FF));
                } else {
                    let _ = SetTextColor(hdc, COLORREF(0x00FF0000));
                }
            }
            let br = GetSysColorBrush(COLOR_WINDOW);
            LRESULT(br.0 as isize)
        }
        0x0200 /* WM_MOUSEMOVE */ => {
            if let Some(&(lx, ly, lw, lh)) = LINK_RECT.get() {
                let mx = (lparam.0 & 0xFFFF) as i16 as i32;
                let my = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                let inside = mx >= lx && mx < lx + lw && my >= ly && my < ly + lh;
                let link = super::window::dlg(hwnd, IDC_ABOUT_LINK);
                use windows::Win32::UI::WindowsAndMessaging::{GetPropW, SetPropW};
                let key = w!("hover");
                let was_hover = GetPropW(link, PCWSTR(key.as_ptr())).0;
                let is_hover_now = if inside { 1usize } else { 0usize };
                let was_hover_val = was_hover as usize;
                if is_hover_now != was_hover_val {
                    let _ = SetPropW(link, PCWSTR(key.as_ptr()), windows::Win32::Foundation::HANDLE(is_hover_now as *mut _));
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(link, None, true);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------
fn ws_win(v: u32) -> WINDOW_STYLE { WINDOW_STYLE(v) }

fn make_static_center(hwnd: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, font: HFONT) -> HWND {
    let t = crate::core::util::wide(text);
    let h = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(t.as_ptr()),
            ws_win(0x01 /* SS_CENTER */) | WS_CHILD | WS_VISIBLE,
            x, y, w, h, hwnd, HMENU(0 as _), None, None,
        ).unwrap_or_default()
    };
    let _ = unsafe { SendMessageW(h, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1)) };
    h
}

/// 从 PNG 字节解码创建 HBITMAP
fn decode_png_to_bitmap(png_data: &[u8]) -> Option<(windows::Win32::Graphics::Gdi::HBITMAP, i32, i32)> {
    unsafe {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_data));
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).ok()?;
        let raw = &buf[..info.buffer_size()];
        let w = info.width as i32;
        let h = info.height as i32;
        let hdc = GetDC(HWND::default());
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(hdc, &bi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
        if dib.is_invalid() || bits.is_null() {
            let _ = ReleaseDC(HWND::default(), hdc);
            return None;
        }
        let dst = std::slice::from_raw_parts_mut(bits as *mut u32, (w * h) as usize);
        for (d, s) in dst.iter_mut().zip(raw.chunks_exact(4)) {
            *d = (s[2] as u32) | (s[1] as u32) << 8 | (s[0] as u32) << 16 | (s[3] as u32) << 24;
        }
        let _ = ReleaseDC(HWND::default(), hdc);
        Some((dib, w, h))
    }
}

// ---------------------------------------------------------------------------
// 公开接口
// ---------------------------------------------------------------------------
pub fn show_about_dialog(parent: HWND) {
    unsafe {
        let class_name = w!("MemCleanerAbout");
        let existing = FindWindowW(class_name, PCWSTR::null()).unwrap_or_default();
        if !existing.0.is_null() {
            let _ = DestroyWindow(existing);
        }

        let hinst = GetModuleHandleW(None).unwrap_or_default();

        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(about_wndproc),
            hInstance: hinst.into(),
            lpszClassName: class_name,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH((COLOR_WINDOW.0 as i32 + 1) as isize as *mut _),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let style: WINDOW_STYLE = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
        let qr_size_init = 200i32;
        let qr_gap_init = 40i32;
        let qr_total_w = qr_size_init * 2 + qr_gap_init;
        let dlg_w = qr_total_w + 40;
        let dlg_h = 500i32;

        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0), class_name,
            w!("关于"),
            style | WS_VISIBLE,
            0, 0, dlg_w, dlg_h,
            HWND(parent.0), None, hinst, None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };

        let mut client_rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut client_rc);
        let cw = client_rc.right - client_rc.left;

        let mut ncm = NONCLIENTMETRICSW::default();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        let _ = SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS, ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut _),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let font = windows::Win32::Graphics::Gdi::CreateFontIndirectW(&ncm.lfMessageFont);

        let mut lf = ncm.lfMessageFont;
        lf.lfHeight = lf.lfHeight * 16 / 10;
        lf.lfWeight = 700;
        let title_font = windows::Win32::Graphics::Gdi::CreateFontIndirectW(&lf);

        let mut lf_u = ncm.lfMessageFont;
        lf_u.lfUnderline = 1;
        let link_font = windows::Win32::Graphics::Gdi::CreateFontIndirectW(&lf_u);

        let margin = 20i32;
        let content_w = cw - margin * 2;
        let mut y = 16i32;

        // 1. 大标题
        let title_text = crate::core::util::wide("WinMemCleaner 1.0.0");
        let h_title = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(title_text.as_ptr()),
            ws_win(0x01) | WS_CHILD | WS_VISIBLE,
            margin, y, content_w, 30, hwnd, HMENU(0 as _), None, None,
        ).unwrap_or_default();
        let _ = SendMessageW(h_title, WM_SETFONT, WPARAM(title_font.0 as usize), LPARAM(1));
        y += 34;

        // 2. Gitee 仓库链接
        let link_text = crate::core::util::wide("Gitee仓库");
        let mut text_size = SIZE::default();
        let hdc = GetDC(hwnd);
        if let Some(&fh) = KEEP_FONT.get() {
            let _ = windows::Win32::Graphics::Gdi::SelectObject(hdc, HFONT(fh as *mut _));
        }
        let _ = windows::Win32::Graphics::Gdi::GetTextExtentPoint32W(hdc, &link_text, &mut text_size);
        let _ = ReleaseDC(hwnd, hdc);
        let link_w = text_size.cx + 8;
        let link_x = (content_w - link_w) / 2 + margin;
        let h_link = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(link_text.as_ptr()),
            ws_win(0x01) | WS_CHILD | WS_VISIBLE | ws_win(0x0200),
            link_x, y, link_w, 20, hwnd, HMENU(IDC_ABOUT_LINK as _), None, None,
        ).unwrap_or_default();
        let _ = SendMessageW(h_link, WM_SETFONT, WPARAM(link_font.0 as usize), LPARAM(1));
        let _ = LINK_RECT.set((link_x, y, link_w, 20));
        let old_proc = SetWindowLongPtrW(h_link, windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX(-4), link_subclass_proc as *const () as isize);
        let _ = LINK_OLD_PROC.set(AtomicUsize::new(old_proc as usize));
        y += 44;

        // 3. 作者和邮箱
        make_static_center(hwnd, "Copyright \u{00A9} 2026 邱宗满", margin, y, content_w, 20, font);
        y += 22;
        make_static_center(hwnd, "qiuzongman@foxmail.com", margin, y, content_w, 20, font);
        y += 22;
        make_static_center(hwnd, "License: AGPL-3.0", margin, y, content_w, 20, font);
        y += 44;

        // 4. 赞助
        make_static_center(hwnd, "赞助", margin, y, content_w, 20, font);
        y += 24;

        let qr_size = 200i32;
        let qr_gap = 40i32;
        let qr_total = qr_size * 2 + qr_gap;
        let qr_start_x = (cw - qr_total) / 2;

        make_static_center(hwnd, "微信", qr_start_x, y, qr_size, 18, font);
        y += 20;
        let h_wechat = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR::null(),
            ws_win(0x0000000E /* SS_BITMAP */) | WS_CHILD | WS_VISIBLE,
            qr_start_x, y, qr_size, qr_size, hwnd, HMENU(0 as _), None, None,
        ).unwrap_or_default();
        if let Some((bmp, _, _)) = decode_png_to_bitmap(WECHAT_PNG) {
            let _ = SendMessageW(h_wechat, 0x0172 /* STM_SETIMAGE */, WPARAM(0), LPARAM(bmp.0 as isize));
        }

        make_static_center(hwnd, "支付宝", qr_start_x + qr_size + qr_gap, y - 20, qr_size, 18, font);
        let h_alipay = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR::null(),
            ws_win(0x0000000E /* SS_BITMAP */) | WS_CHILD | WS_VISIBLE,
            qr_start_x + qr_size + qr_gap, y, qr_size, qr_size, hwnd, HMENU(0 as _), None, None,
        ).unwrap_or_default();
        if let Some((bmp, _, _)) = decode_png_to_bitmap(ALIPAY_PNG) {
            let _ = SendMessageW(h_alipay, 0x0172 /* STM_SETIMAGE */, WPARAM(0), LPARAM(bmp.0 as isize));
        }
        y += qr_size + 8;

        // 调整窗口高度
        let mut wrc = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wrc);
        let non_client_h = (wrc.bottom - wrc.top) - (client_rc.bottom - client_rc.top);
        let final_h = y + non_client_h;
        let _ = SetWindowPos(hwnd, HWND_TOP, 0, 0, dlg_w, final_h, SWP_NOMOVE | SWP_NOZORDER);

        // 居中显示
        let mut wa = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA, 0,
            Some(&mut wa as *mut _ as *mut _),
            windows::Win32::UI::WindowsAndMessaging::SPIF_SENDCHANGE,
        );
        let _ = GetWindowRect(hwnd, &mut wrc);
        let ww = wrc.right - wrc.left;
        let wh = wrc.bottom - wrc.top;
        let x = (wa.right - wa.left - ww) / 2 + wa.left;
        let y = (wa.bottom - wa.top - wh) / 2 + wa.top;
        let _ = SetWindowPos(hwnd, HWND_TOP, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    }
}
