//! 小工具函数。

/// 将 UTF-8 字符串转换为以 NUL 结尾的 UTF-16 宽字符向量。
pub fn wide(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain([0]).collect()
}

/// 将 UTF-16 缓冲区（可能含 NUL）转换为 String。
pub fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// 本机当前时间 `(时, 分, 秒)`，供「上次自动清理」显示。
/// 直接读 `GetLocalTime`（已按本地时区换算），不为一个时间戳引入日期时间库。
pub fn local_time() -> (u32, u32, u32) {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let t = unsafe { GetLocalTime() };
    (t.wHour as u32, t.wMinute as u32, t.wSecond as u32)
}
