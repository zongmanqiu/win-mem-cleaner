# 更新日志

## [1.1.1] - 2026-10-26

### 修复
- **阈值自动清理从不触发**：`is_game_or_fullscreen()` 原先把 `SHQueryUserNotificationState()` 的
  `QUNS_BUSY` 当作「正在全屏」，但它只表示系统里**存在**全屏窗口（不要求前台）。后台常驻的
  全屏覆盖层（截图/录屏/悬浮窗）会让 `QUNS_BUSY` 长期为真，调度器每轮都走避让分支，内存再高也不清。
  现改为只看**前台**窗口是否铺满其所在显示器（容 1px 误差，排除桌面/任务栏），并保留
  独占全屏 D3D（`QUNS_RUNNING_D3D_FULL_SCREEN`）与演示模式（`QUNS_PRESENTATION_MODE`）信号
- 版本号 1.1.0 → 1.1.1（`Cargo.toml` 单一来源，同步 `app.manifest` 的 assemblyIdentity）
- 发布产物更名 `WinMemCleaner_x64.exe` → `WinMemCleaner.exe`（x64 与版本号只体现在发布 zip 名上）
- **`build/build.ps1` 拷不进 `.build`**：脚本是无 BOM 的 UTF-8，PowerShell 5.1 按 GBK 解码时注释行末的
  全角 `）` 吞掉换行，`$env:CARGO_TARGET_DIR` 赋值被并进注释从未执行，最后 `Join-Path` 拿到 null。
  改存为带 BOM 的 UTF-8（PS 5.1 只认 BOM）

### 文档同步
- `docs/platform_notes.md` 新增第 9 条：`QUNS_BUSY` 的真实语义与实测判据
- `AGENTS.md` §六、`docs/api.md`、`docs/architecture.md`、`docs/testing.md` 同步全屏避让判据

## [1.1.0] - 2026-10-25

### 新增
- 自动清理拆分为两种方式，互相独立、可同时生效：
  - 方式一：每 x 分钟按 y 强度清理（默认 每 5 分钟 / 标准）
  - 方式二：物理内存占用 > x% 时按 y 强度清理（默认 超过 85% / 深度）
  - x 为整数输入框，y 为「标准 / 深度」下拉，用户可自行组合
- 阈值方式触发后有 60 秒静默期，避免占用长期停留在阈值上方时反复清理；
  同一轮两种方式都到期时，优先执行阈值方式
- 关于对话框在「Gitee仓库」右侧以竖线间隔新增「GitHub仓库」链接
- 配置新增 `interval_level` / `threshold_percent` / `threshold_level` 三个字段；
  旧的单一 `level` 字段读入时自动迁移为定时强度

### 改动
- **便携化**：配置文件从 `%APPDATA%\memclean\config.json` 改到 exe 同级的 `WinMemCleaner.json`，
  程序不再往用户目录写文件；取不到 exe 路径时宁可不存盘也不写到别处。
  旧文件不迁移、不删除，留着的那台机器首次会以默认值起盘
- 托盘右键菜单文案精简为：窗口 / 标准清理 / 深度清理 / 关于 / 退出
- 清理强度下拉去掉括号提示，统一为「标准」/「深度」
- 版本号 1.0.1 → 1.1.0（`Cargo.toml` 单一来源，同步 `app.manifest` 的 assemblyIdentity）
- `build/build.ps1` 中间产物目录改到 `.temp/target`（原先落在仓库根目录）

### 修复
- **系统缓存读数错误**：原先读未文档化的 `NtQuerySystemInformation(SystemFileCacheInformation)`，
  结构布局靠猜（SDK 里没有这个结构），并且拿「缓存峰值」当分母显示成"已用 / 总量"的样子。
  现改用文档化 API `GetPerformanceInfo().SystemCache`（单位是页，乘 `PageSize`），
  显示实时字节 + 占物理内存百分比。同机实测：旧读数 0.36 GB / "峰值" 2.61 GB，
  文档口径实际是 4.75 GB（占物理内存 30%）——差 13 倍
- 数值输入框的保存时机改用 `EN_KILLFOCUS`(0x0200)：此前误用 `8`（那是 `CBN_CLOSEUP`），
  只改数字不改其它控件时设置不会落盘
- 关于对话框链接控件改用 `SS_NOTIFY`(0x0100)：此前误写 `0x0200`（`SS_RIGHT`），文字右偏
- 移除无调用方的 `run_manual()`，托盘清理消息直接按档位执行

### 文档同步
- README 新增「运行环境」：明确只支持 Windows 10 及以上 x64（Rust 官方对 `x86_64-pc-windows-msvc`
  的底线就是 Win10，1.77.2 是最后一个还能跑 Win7 的版本）；构建工具链最低 1.80；
  并写清内存阈值只判物理内存、`page_percent`（提交量/提交上限）与 `cache_percent`（实时缓存占物理内存比）的真实含义
- `README.md`、`docs/api.md`、`docs/architecture.md`、`docs/structure.md`、`docs/testing.md`
- 测试补至 21 个冒烟用例（阈值边界、两种方式默认值、旧字段迁移、配置落在 exe 同级）

### 工程与文档（向 CurSeen 学的纪律，不搬体量只搬机制）
- 新增 `tests/docs.rs` **文档一致性门禁**（7 条事实检查：文档存在、`docs/` 全登记、结构文档覆盖
  每个 src 模块且点名的 `.rs` 真实存在、`main/` 根无游离文件、LICENSE 逐字、无本机路径泄漏）
- 新增 `AGENTS.md`（工程规则全集）与 `docs/platform_notes.md`（Win32 实测库，每条带判据）
- 新增 `.cargo/config.toml`：把编译产物钉到 `../.temp/target`，克隆者无需手设环境变量
- `DOCS_MANIFEST.md` 升级为「何时更新 + 由谁验证」表；`docs/structure.md` 重写

### 清理死代码
- 删除 `do_clean_all` + `AllCleanResult` + `current_foreground_pid`（全仓仅定义处一次引用，
  被 `#![allow(dead_code)]` 掩盖），`mem.rs` 625 → 570 行；随之移除其独占的
  `GetForegroundWindow` / `GetWindowThreadProcessId` / `HWND` 导入
- 摘除全仓 4 处 `#![allow(dead_code)]`：它对 `pub` 项本来就不起作用，却会掩盖未来的私有死代码
  （`do_clean_all` 正是被它养烂的）。摘除后全目标编译零警告，编译器重新成为死代码的守门人

### 文档与代码不符（核对时逐条查出并修正）
- `docs/architecture.md`：数据流写「控件消息 → 保存配置/触发清理」，实际主窗口**没有清理按钮**，
  手动清理只走托盘菜单
- `docs/testing.md`：UI 清单写「标题栏显示 WinMemCleaner-管理员/-非管理员」，实际标题恒为
  `WinMemCleaner`（`is_admin()` 只被测试用到，UI 不显示权限）
- `app.manifest`：注释写「不强制管理员」，实际是 `requireAdministrator`
- `README.md`：删掉未经验证的「运行内存 2~7MB」；技术栈表里删掉根本不存在的 `resvg/usvg` 依赖；
  `docs/structure.md` 去掉不存在的 `*.svg` 与「二维码 PNG 由 build.rs 生成」的错误说法
- `README.md` 头部改为双平台下载 + Gitee/GitHub 仓库链接（格式对齐 Pecia）

## [1.1.0 草稿] - 2026-08-18（未单独发布，随 1.1.0 一起出货）

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
- `tests/smoke.rs` — 冒烟测试
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
