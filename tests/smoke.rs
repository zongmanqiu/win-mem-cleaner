//! 冒烟测试 —— 验证核心模块可正常加载和基本功能。
//!
//! 参照 Pecia 的 test_ui_smoke.cpp 设计理念：
//! "catch the window crashes when you open it regressions"
//!
//! 运行方式：cargo test

/// 验证 core 模块可以正常导入
#[test]
fn smoke_core_modules_load() {
    // 这些模块在编译时就被链接，如果能通过编译就说明基本结构正确
    // 运行时验证：确保模块可以被调用
    let _ = memclean::core::util::wide("hello");
    let _ = memclean::core::util::wide_to_string(&[0x0068, 0x0065, 0x006C, 0x006C, 0x006F, 0x0000]);
}

/// 验证配置加载（使用默认值）
#[test]
fn smoke_config_load() {
    let cfg = memclean::core::config::AppConfig::defaults();
    assert_eq!(cfg.interval_minutes, 5);
    assert_eq!(cfg.level, 1);
    assert!(cfg.fullscreen_avoid);
    assert!(!cfg.autostart);
}

/// 验证配置 sanitize
#[test]
fn smoke_config_sanitize() {
    let mut cfg = memclean::core::config::AppConfig::defaults();
    cfg.interval_minutes = 0; // 超出范围
    cfg.level = 99;           // 非法值
    cfg.sanitize();
    assert_eq!(cfg.interval_minutes, 1); // 被 clamp 到最小值
    assert_eq!(cfg.level, 1);            // 被重置为默认
}

/// 验证配置边界值
#[test]
fn smoke_config边界值() {
    let mut cfg = memclean::core::config::AppConfig::defaults();
    cfg.interval_minutes = 1440; // 最大值
    cfg.sanitize();
    assert_eq!(cfg.interval_minutes, 1440);

    cfg.interval_minutes = 1441; // 超出最大值
    cfg.sanitize();
    assert_eq!(cfg.interval_minutes, 1440);
}

/// 验证工具函数 wide/wide_to_string 往返一致性
#[test]
fn smoke_util_roundtrip() {
    let original = "Hello, 世界!";
    let wide = memclean::core::util::wide(original);
    // wide 以 NUL 结尾
    assert_eq!(*wide.last().unwrap(), 0);
    // 去掉 NUL 后转换回来
    let back = memclean::core::util::wide_to_string(&wide[..wide.len() - 1]);
    assert_eq!(back, original);
}

/// 验证工具函数 wide 空字符串
#[test]
fn smoke_util_empty() {
    let wide = memclean::core::util::wide("");
    assert_eq!(wide.len(), 1); // 仅 NUL
    assert_eq!(wide[0], 0);
}

/// 验证工具函数 wide 纯 ASCII
#[test]
fn smoke_util_ascii() {
    let wide = memclean::core::util::wide("abc");
    assert_eq!(wide.len(), 4); // a + b + c + NUL
    assert_eq!(wide[0], 'a' as u16);
    assert_eq!(wide[1], 'b' as u16);
    assert_eq!(wide[2], 'c' as u16);
    assert_eq!(wide[3], 0);
}

/// 验证工具函数 wide 中文
#[test]
fn smoke_util_chinese() {
    let wide = memclean::core::util::wide("你好");
    assert_eq!(wide.len(), 3); // 你 + 好 + NUL
    assert!(wide[0] > 0x7F);   // 非 ASCII
    assert!(wide[1] > 0x7F);
}

/// 验证 format_bytes 基本功能
#[test]
fn smoke_format_bytes() {
    let s = memclean::core::mem::format_bytes(0);
    assert!(s.contains("0"));

    let s = memclean::core::mem::format_bytes(1024);
    assert!(s.contains("KB"));

    let s = memclean::core::mem::format_bytes(1024 * 1024);
    assert!(s.contains("MB"));

    let s = memclean::core::mem::format_bytes(1024u64 * 1024 * 1024);
    assert!(s.contains("GB"));
}

/// 验证内存信息可以读取（Windows 特定）
#[test]
#[cfg(windows)]
fn smoke_mem_info() {
    let mem = memclean::core::mem::mem_info();
    assert!(mem.ullTotalPhys > 0);
    assert!(mem.ullAvailPhys > 0);
    assert!(mem.ullAvailPhys <= mem.ullTotalPhys);
}

/// 验证物理内存使用百分比在合理范围
#[test]
#[cfg(windows)]
fn smoke_physical_usage() {
    let pct = memclean::core::mem::physical_usage_percent();
    assert!(pct <= 100);
}

/// 验证 memory_snapshot 返回有效数据
#[test]
#[cfg(windows)]
fn smoke_memory_snapshot() {
    let snap = memclean::core::mem::memory_snapshot();
    assert!(snap.phys_total > 0);
    assert!(snap.phys_percent <= 100);
    assert!(snap.page_total > 0);
    assert!(snap.page_percent <= 100);
}

/// 验证管理员检测函数可以调用
#[test]
#[cfg(windows)]
fn smoke_is_admin() {
    // 不管当前是否管理员，函数应该能正常返回
    let _ = memclean::core::mem::is_admin();
}

/// 验证全屏检测函数可以调用
#[test]
#[cfg(windows)]
fn smoke_is_game_or_fullscreen() {
    // 在测试环境中应该返回 false
    let result = memclean::core::mem::is_game_or_fullscreen();
    // 不做断言，只验证不 panic
    let _ = result;
}

/// 验证清理函数可以调用（标准级别）
#[test]
#[cfg(windows)]
fn smoke_clean_standard() {
    // 标准清理应该不会 panic
    let result = memclean::core::mem::clean_by_level(1);
    assert!(!result.is_empty());
}

/// 验证配置 JSON 序列化/反序列化往返
#[test]
fn smoke_config_json_roundtrip() {
    let cfg = memclean::core::config::AppConfig::defaults();
    let json = serde_json::to_string(&cfg).unwrap();
    let back: memclean::core::config::AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(cfg.interval_minutes, back.interval_minutes);
    assert_eq!(cfg.level, back.level);
    assert_eq!(cfg.fullscreen_avoid, back.fullscreen_avoid);
    assert_eq!(cfg.autostart, back.autostart);
}

/// 验证配置 JSON 反序列化容错（未知字段）
#[test]
fn smoke_config_json_unknown_fields() {
    let json = r#"{"interval_minutes": 10, "level": 2, "unknown_key": true}"#;
    let cfg: memclean::core::config::AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.interval_minutes, 10);
    assert_eq!(cfg.level, 2);
}

/// 验证配置 JSON 反序列化容错（缺失字段用默认值）
#[test]
fn smoke_config_json_missing_fields() {
    let json = r#"{}"#;
    let cfg: memclean::core::config::AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.interval_minutes, 5); // 默认值
    assert_eq!(cfg.level, 1);            // 默认值
}
