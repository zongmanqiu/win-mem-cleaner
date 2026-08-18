# 更新日志

## [1.1.0] - 2026-08-18

### 架构重构
- 模仿 Pecia 的 `core/` ← `ui/` 分层架构
- `main.rs` 从 1241 行精简到 74 行（纯入口）
- 新增 `src/core/` 目录（纯逻辑层）：mem、config、scheduler、util
- 新增 `src/ui/` 目录（界面层）：window、tray、about
- 依赖方向强制：`core/` 禁止引用 `ui/`

### 新增文件
- `build/build.ps1` — PowerShell 构建脚本
- `docs/architecture.md` — 架构总览
- `docs/structure.md` — 目录结构说明
- `docs/api.md` — API 参考文档
- `docs/testing.md` — 测试指南
- `docs/changes.md` — 更新日志（本文件）
- `tests/smoke.rs` — 冒烟测试（18 个测试用例）
- `DOCS_MANIFEST.md` — 文档清单

### 代码修复
- 移除 `dump_ui_theme()` 调试代码
- 修正 `slient` → `silent` 拼写错误（向后兼容）
- 实现托盘 tooltip 显示物理/虚拟/缓存三项指标
- 清理 `mem.rs` 中的死代码（`skipped_protected` 字段）
- 移除 `mem.rs` 中重复的 `wide`/`wide_to_string`，改用 `crate::core::util`
- 修复 `--restart` 参数丢失问题（提权重启绕过单实例互斥）

### 目录整理
- 删除 `.main/build/` 和 `.main/target/`（478MB 编译产物）
- 图片资源集中到 `image/`（logo + 二维码）
- 创建 `.gitignore` 排除编译产物

## [1.0.0] - 2026-08-18

### 初始版本
- 两级内存清理（标准/深度）
- 实时内存监控（进度条 + 托盘图标）
- 自动清理调度（定时 + 全屏避让）
- 配置持久化（JSON）
- 开机启动（注册表）
- 管理员权限提升
- 单实例互斥
- 关于对话框（作者/仓库/赞助二维码）
