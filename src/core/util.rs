//! 小工具函数。

#![allow(dead_code)]

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
