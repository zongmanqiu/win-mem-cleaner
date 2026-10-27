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
    assert_eq!(cfg.threshold_percent, 85);
    assert!(!cfg.autostart);
}

/// 验证配置路径落在 exe 同级目录（便携程序，不往用户目录写文件）
#[test]
fn smoke_config_path_next_to_exe() {
    let p = memclean::core::config::config_path().expect("current_exe 应可用");
    let exe_dir = std::env::current_exe().unwrap();
    let exe_dir = exe_dir.parent().unwrap();
    assert_eq!(p.parent().unwrap(), exe_dir);
    assert_eq!(p.file_name().unwrap(), "WinMemCleaner.json");
    if let Some(appdata) = std::env::var_os("APPDATA") {
        assert!(!p.starts_with(std::path::PathBuf::from(appdata)));
    }
}

/// 验证存盘真的落在 exe 同级（跑完即删，不留垃圾在 target 目录）
#[test]
fn smoke_config_save_next_to_exe() {
    let p = memclean::core::config::config_path().expect("current_exe 应可用");
    let _ = std::fs::remove_file(&p);
    memclean::core::config::save(&memclean::core::config::AppConfig::defaults());
    assert!(p.exists(), "save() 应把配置文件写在 exe 同级: {}", p.display());
    let back = memclean::core::config::load();
    assert_eq!(back.threshold_percent, 85);
    let _ = std::fs::remove_file(&p);
}

/// 验证配置 sanitize
#[test]
fn smoke_config_sanitize() {
    let mut cfg = memclean::core::config::AppConfig::defaults();
    cfg.interval_minutes = 0;   // 超出范围
    cfg.threshold_percent = 0;   // 超出范围
    cfg.sanitize();
    assert_eq!(cfg.interval_minutes, 1);   // 被 clamp 到最小值
    assert_eq!(cfg.threshold_percent, 1);  // 被 clamp 到最小值
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

    // 阈值 100% 永远不会被超过，上限取 99
    cfg.threshold_percent = 100;
    cfg.sanitize();
    assert_eq!(cfg.threshold_percent, 99);

    cfg.threshold_percent = 99;
    cfg.sanitize();
    assert_eq!(cfg.threshold_percent, 99);
}

/// 验证「上次自动清理」初始为「从未执行」——主窗口据此显示 `--:--`
#[test]
fn smoke_last_run_starts_empty() {
    use memclean::core::scheduler::Slot;
    assert_eq!(memclean::core::scheduler::last_run(Slot::Interval), None);
    assert_eq!(memclean::core::scheduler::last_run(Slot::Threshold), None);
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
    // 系统缓存：实时字节（文档化 API 应有读数，不能静默为 0）
    assert!(snap.cache_bytes > 0, "GetPerformanceInfo().SystemCache 读数为 0");
    assert!(snap.cache_bytes <= snap.phys_total);
    // 百分比的分母必须是物理内存总量，不是缓存峰值
    let expect = (snap.cache_bytes.saturating_mul(100) / snap.phys_total).min(100) as u32;
    assert_eq!(snap.cache_percent, expect);
}

/// 验证管理员检测函数可以调用
#[test]
#[cfg(windows)]
fn smoke_is_admin() {
    // 不管当前是否管理员，函数应该能正常返回
    let _ = memclean::core::mem::is_admin();
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
    assert_eq!(cfg.threshold_percent, back.threshold_percent);
    assert_eq!(cfg.autostart, back.autostart);
}

/// 验证配置 JSON 反序列化容错（未知字段）
#[test]
fn smoke_config_json_unknown_fields() {
    let json = r#"{"interval_minutes": 10, "threshold_percent": 90, "unknown_key": true}"#;
    let cfg: memclean::core::config::AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.interval_minutes, 10);
    assert_eq!(cfg.threshold_percent, 90);
    assert!(!cfg.autostart); // 缺失字段用默认值
}

/// 验证配置 JSON 反序列化容错（缺失字段用默认值）
#[test]
fn smoke_config_json_missing_fields() {
    let json = r#"{}"#;
    let cfg: memclean::core::config::AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.interval_minutes, 5); // 默认值
    assert_eq!(cfg.threshold_percent, 85); // 默认值
}

/// 验证历史版本删掉的键全部被安全忽略：老用户磁盘上的 JSON 带着这些键，
/// 不能因此读失败、也不能把用户保留的值清零。（1.1.2 删了全屏避让与两个档位字段）
#[test]
fn smoke_config_json_忽略已删除的旧键() {
    let json = r#"{"interval_minutes": 7, "threshold_percent": 80,
                   "fullscreen_avoid": true, "interval_level": 1, "threshold_level": 1, "level": 2}"#;
    let cfg: memclean::core::config::AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.interval_minutes, 7); // 用户设的值原样保留
    assert_eq!(cfg.threshold_percent, 80);
    // 存盘走结构体字段，旧键不会被写回去——线上那份脏文件下次保存自然变干净
    let out = serde_json::to_string(&cfg).unwrap();
    for dead in ["fullscreen_avoid", "interval_level", "threshold_level", "\"level\""] {
        assert!(!out.contains(dead), "重新存盘后仍写着已删除的键 {dead}：{out}");
    }
}
