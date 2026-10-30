# 更新日志

## [1.1.3] - 2026-10-30

### 修复
- **开机后托盘没有图标、「自己结束了再开就有」（用户报）**：不是开机启动没生效。
  真机证据链：计划任务 `\WinMemCleaner` 的「上次运行时间」= **10:30:05**，`quser` 的登录时间也是
  10:30；「上次结果 = 1」正是进程被强制结束的退出码（实测 `taskkill /f` 也是 1），即用户在任务管理器
  里结束的就是它；任务定义在取消/重新勾选后被**重建**（`<Date>11:04:48`）且命令、权限、触发器都对。
  根因在 `ui/tray.rs`：托盘线程第一次 `Shell_NotifyIconW(NIM_ADD)` 失败就 `return`，隐藏窗口与消息
  循环随之消失——之后没有重试，连 `TaskbarCreated` 也没人收。登录时 Explorer 的任务栏还没建好，
  这正是唯一能同时解释「开机没有、手动启动有」的机制。修法：`added` 状态 + 1 秒定时器重试，
  `TaskbarCreated` 重加失败也交给定时器
- **计划任务的 72 小时执行上限 / 电池限制**：`schtasks /create` 命令行没有暴露这些设置，建出来的任务
  是默认值——「如果运行了 72 小时，停止任务: 72:00:00」会在三天后把常驻进程结束掉；默认「不用电池
  启动 / 切电池就停止」让笔记本上等于随时下线。改用 `schtasks /create /xml` 显式写
  `ExecutionTimeLimit=PT0S` 与电池两项（UTF-16LE + BOM 的临时 XML 放 exe 同级、建完即删；
  写不下时退回命令行方式，仍能开机启动）

### 实测
- **红绿对照（无人值守）**：新建一个没有任务栏的桌面，把程序拉上去跑 9 秒——
  HEAD（旧代码）：托盘窗口 `MemCleanerTrayWindow` **不存在**（线程已退出），主窗口仍在、进程存活，
  与用户看到的「进程在、能开界面、没托盘」一致；修复版：同条件下托盘窗口**仍在**，
  `Shell_NotifyIconGetRect` 返回 `E_FAIL`（图标确实没加上）⇒ 线程在重试
- **反向校验**：正常桌面（Explorer 在岗）下同一程序 `Shell_NotifyIconGetRect` 返回 `S_OK`
  （底右任务栏区域有矩形），证明 A/B 里「查不到图标」不是 API 失灵

### 文档
- `docs/platform_notes.md` 新增第 14 条（登录早期 `NIM_ADD` 失败，失败不能丢线程）、第 15 条
  （`schtasks` 默认 72 小时上限与电池限制）；第 7 条补「重加也可能失败，交给重试」，第 10 条的
  推断用本次真机重启证实并改写
- `docs/testing.md` 手动清单补「登录后托盘在岗」「任务设置：执行时间上限已禁用、电源管理为空」
- 版本 1.1.2 → 1.1.3；README 下载链接同步

## [1.1.2] - 2026-10-26

### 删除
- **「全屏避让」整个开关移除**：配置字段、复选框、`is_game_or_fullscreen()` 及其前台判定实现一并删。
  1.1.1 只是把判据改准，但这个功能的本质是"用一个容易误判的条件**静默**阻止清理"——用户完全看不出
  它没在跑。删掉后没有任何路径能让阈值清理不触发。老配置里残留的 `fullscreen_avoid` 键被安全忽略
  （`smoke_config_json_忽略已删除的旧键` 钉住）
- **两个清理强度下拉**：见下方「档位固定」

### 新增
- **回弹退避**：阈值清理后 30 秒复查，占用若又越过阈值 ⇒ 静默期翻倍（60 秒起，上限 15 分钟），
  复查通过则回到 60 秒。活跃构建负载撑起来的占用是清不动的，别再每分钟卡用户一次
- **可观测**：主窗口两行右侧各显示该方式「上次自动清理」的 `hh:mm`，本次运行没自动清过是 `--:--`。
  手动托盘清理不记录（那是用户自己按的，不属于"自动清理没生效"）
- **安全门禁 `tests/safety.rs`**：钉住「本程序只释放内存，永远不具备结束 / 注入 / 其它进程的能力」。
  起因是用户指令「我们只是释放内存，不要关闭电脑上的其它软件啊…你要确保这一点，否则我不敢用你」。
  审计结果本就干净（全仓 0 处结束进程 API，打开别的进程只要 `PROCESS_SET_QUOTA | QUERY_LIMITED`，
  唯一子进程是 `schtasks.exe`），但"我看过代码"不是保证——本程序以管理员运行，所以这条改成了机器把守：
  禁止词元**分段存储、运行时拼接**，门禁文件自身也在扫描范围内（不开豁免口子），另配四枚自检钉子
  证明它会红。见 `AGENTS.md` §〇
- 阈值触发的那一轮把定时那一轮**顺延**，避免 5 秒内连清两次
- **两种方式档位固定**：定时＝标准、阈值＝深度，界面文字直接写成「每 x 分钟 **标准清理**」/
  「内存 > x % **深度清理**」，规则印在界面上。删掉 `interval_level` / `threshold_level` 两个配置字段
  与 `combo_*` 全套下拉辅助函数。理由：两个非默认组合都更差——定时+深度＝周期性主动卡顿；
  阈值+标准＝实测几乎清不动（标准档连缓存读数都不变，深度档才把 6.3 GB 打到 0.25 GB）。
  想手动选档走托盘右键菜单，那里本来就有一级入口

### 修复
- **敲完数字直接点 X 会丢设置**：`WM_CLOSE` 隐藏前与托盘「退出」前各补一次存盘。
  ★ 更正一条我先前写错的因果：我一度判断"用户设的 80 没落盘"，因为只在开发目录和系统目录搜了配置——
  实际它一直好好地存在**运行目录**的 `WinMemCleaner.json` 里（`threshold_percent: 80`）。
  **丢设置不是用户症状的原因**，程序没在跑才是（见下方「常驻」）。本条作为健壮性修复保留。
- **框线范围异常（用户报的）**：「自动清理」分组框的右竖线画在窗口边框上，右边距实测 **0**
  （左边距 14）。根因是 `create_ui` 用客户区坐标摆控件，却把 `SetWindowPos` 要的**外框**宽度当
  客户区宽来算框宽——差一整圈非客户边框，凡贴右边算的东西都被推出去。改成布局全程用客户区尺寸、
  最后一步加**实测**的 `frame_delta()`（`GetWindowRect − GetClientRect`），顺带删掉硬编码的
  `+31` 标题栏高度。修完左右边距 14/14 对称，像素已核
- **控件字体改为创建时自己拿**（`make_child` / `edt` 发 `WM_SETFONT`）：上一条修完出现的新裁字
  ——`15.8 GB` 画成 `15.8 G`。是我把 `apply_ui_font` 挪到布局之前导致的：那时 `EnumChildWindows`
  还没有子控件可补，控件全退回系统默认字体（更宽）。子控件本来就不继承父窗口字体，
  字体该由控件自己在 `CreateWindowExW` 之后拿。见 `docs/platform_notes.md` 第 11 条
- `measure_text_width` 不再多量一枚 NUL：`util::wide` 带结尾 0，而 windows-rs 把切片长度当
  Win32 的字符数传下去（`platform_notes.md` 第 13 条）
- **勘误一条文档事实**：`AGENTS.md` / `platform_notes.md` 原写「`0x0200` 是 `SS_RIGHT`」是**错的**。
  `WinUser.h` 里 `SS_RIGHT = 0x2`、`0x200 = SS_CENTERIMAGE`（垂直居中）。所以 v1.1.0 那次
  "链接右偏"的成因当时就没解释对；真正修好的只有"点了没反应"（缺 `SS_NOTIFY`）。常量一律查头文件
- 设置**改完即生效**：调度器每轮重读配置，最迟 5 秒，不需要重启程序
- **文档门禁的泄漏判据换掉**：`no_local_absolute_paths_leak` 原来靠四个写死的字符串（开发机目录名、
  用户名）当判据——判据本身就是把本机路径写进仓库，而且没点名的形态会漏：本次 `docs/` 里 5 处
  「运行目录 + 盘符」的真实路径，门禁一路报绿。改成结构性判据（字母 + 冒号 + 单斜杠即算，
  `://` 协议前缀与时间戳不算），**取消对检测器自身的豁免**，并配四枚自检钉子。
  换完当场就红了三次，包括它自己新写的注释——这才叫在跑

### 常驻（查"为什么没效果"时查出来的真问题）
- **开机启动改用计划任务**（登录触发 + 最高权限），建不了才退回 `HKCU\...\Run`，两者只留一条。
  本程序是 `requireAdministrator`，只靠 Run 键会在登录时弹 UAC、没人点就不启动
- **启动自愈**：勾了开机启动时，每次启动把启动项刷成**当前 exe 的路径**——改名/搬家不再留死项
- **在岗可见**：托盘 tooltip 增加「已运行 X天X时」，一眼看出常驻进程在不在
- `build/build.ps1 -Deploy <运行目录>`：构建后直接拷到实际运行的目录。
  实测教训：注册表里注册的是**运行目录**那份 `WinMemCleaner.exe`（1.1.1），而产物一直只更新到
  开发侧的 `.build`——**改了没人执行**

### 版本
- 1.1.1 → 1.1.2；README 下载链接同步

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
- 关于对话框链接控件改用 `SS_NOTIFY`(0x0100)：此前误写 `0x0200`，点击收不到通知。
  ★ v1.1.2 勘误：本条原把 `0x0200` 说成 `SS_RIGHT` 并归因"文字右偏"，**是错的**——
  `SS_RIGHT = 0x2`，`0x200` 是 `SS_CENTERIMAGE`（垂直居中），右偏当时就没有被解释对
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
