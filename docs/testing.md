# WinMemCleaner 测试指南

> 参照 Pecia 的测试体系设计。

## 一、测试策略

| 层级 | 测试类型 | 工具 | 覆盖目标 |
|------|----------|------|----------|
| 单元测试 | `#[test]` 函数 | 内置 `cargo test` | core 模块纯逻辑 |
| 集成测试 | `tests/` 目录 | `cargo test` | 模块间交互 |
| 冒烟测试 | `tests/smoke.rs` | `cargo test` | 基本功能不 panic |
| 手动测试 | 运行 exe | 人工 | UI 交互、托盘、清理效果 |

## 二、运行测试

```powershell
# 运行所有测试
cargo test

# 运行特定测试
cargo test smoke_
cargo test test_config

# 显示测试输出
cargo test -- --nocapture

# 仅运行 Windows 特定测试
cargo test -- --ignored
```

## 三、测试文件说明

### `tests/smoke.rs` — 冒烟测试

验证核心模块可正常加载和基本功能：

| 测试函数 | 验证内容 |
|----------|----------|
| `smoke_core_modules_load` | core 模块可以正常导入和调用 |
| `smoke_config_load` | 配置默认值正确 |
| `smoke_config_sanitize` | 配置校验逻辑正确 |
| `smoke_config边界值` | 配置边界值处理 |
| `smoke_util_roundtrip` | UTF-8 ↔ UTF-16 往返一致性 |
| `smoke_util_empty` | 空字符串处理 |
| `smoke_util_ascii` | 纯 ASCII 处理 |
| `smoke_util_chinese` | 中文字符处理 |
| `smoke_format_bytes` | 字节格式化（B/KB/MB/GB） |
| `smoke_mem_info` | 内存信息可读取（Windows） |
| `smoke_physical_usage` | 物理内存使用率在合理范围（Windows） |
| `smoke_memory_snapshot` | 内存快照数据有效（Windows） |
| `smoke_is_admin` | 管理员检测函数可调用（Windows） |
| `smoke_is_game_or_fullscreen` | 全屏检测函数可调用（Windows） |
| `smoke_clean_standard` | 标准清理不 panic（Windows） |
| `smoke_config_json_roundtrip` | 配置 JSON 序列化往返 |
| `smoke_config_json_unknown_fields` | JSON 未知字段容错 |
| `smoke_config_json_missing_fields` | JSON 缺失字段用默认值 |

### Windows 特定测试

标记 `#[cfg(windows)]` 的测试仅在 Windows 上运行，涉及真实 Win32 API 调用。

## 四、UI 手动测试清单

由于 Win32 GUI 无法自动化测试，以下 UI 功能需手动验证：

### 主窗口
- [ ] 启动后窗口显示在屏幕右下角
- [ ] 标题栏显示「WinMemCleaner-管理员」或「WinMemCleaner-非管理员」
- [ ] 三个进度条正确显示物理/虚拟/缓存内存
- [ ] 进度条颜色随占用变化（绿→黄→红）
- [ ] 进度条内百分比数字正确
- [ ] 窗口宽度自动适应文字长度

### 控件
- [ ] 间隔输入框可编辑，仅允许数字
- [ ] 清理强度下拉框可选择「标准」/「深度(短暂卡顿)」
- [ ] 全屏避让复选框可勾选/取消
- [ ] 开机启动复选框可勾选/取消
- [ ] 修改设置后自动保存到 config.json

### 托盘
- [ ] 托盘图标显示内存占用百分比
- [ ] 托盘图标颜色随占用变化
- [ ] 鼠标悬停显示 tooltip（物理/虚拟/缓存三项）
- [ ] 右键菜单显示：显示窗口 / 标准清理 / 深度清理 / 关于 / 完全退出
- [ ] 双击托盘图标显示主窗口
- [ ] 点击「显示窗口」显示主窗口
- [ ] 点击「标准清理」执行清理
- [ ] 点击「深度清理」执行清理
- [ ] 点击「完全退出」退出程序

### 关于对话框
- [ ] 显示版本号「WinMemCleaner 1.0.0」
- [ ] 显示作者和邮箱
- [ ] Gitee 仓库链接可点击（蓝色下划线）
- [ ] 链接悬停变红色
- [ ] 微信/支付宝二维码正确显示
- [ ] 对话框居中显示

### 自动清理
- [ ] 设置间隔后，等待对应时间自动触发清理
- [ ] 全屏应用运行时跳过自动清理
- [ ] 全屏应用关闭后补清

### 开机启动
- [ ] 勾选开机启动后，注册表 Run 键正确写入
- [ ] 取消开机启动后，注册表 Run 键正确删除
- [ ] 开机启动时静默启动（窗口隐藏）

### 单实例
- [ ] 启动第二个实例时，第一个实例窗口被激活
- [ ] 第二个实例自动退出

## 五、测试覆盖目标

| 模块 | 目标覆盖 | 当前状态 |
|------|----------|----------|
| core/config | 100%（纯逻辑） | ✅ 已覆盖 |
| core/util | 100%（纯逻辑） | ✅ 已覆盖 |
| core/mem | 80%（部分 Win32 API 需手动） | ⚠️ 部分覆盖 |
| core/scheduler | 60%（需运行时验证） | ⚠️ 部分覆盖 |
| ui/* | 手动测试 | 📋 手动清单 |

## 六、新增测试规则

1. **新逻辑必须有测试**：新增 `pub fn` 时同步添加 `#[test]`
2. **测试命名**：`smoke_` 前缀 = 冒烟测试，`test_` 前缀 = 功能测试
3. **Win32 测试**：标记 `#[cfg(windows)]`，不依赖特定系统状态
4. **测试独立**：每个测试可独立运行，不依赖其他测试的状态
