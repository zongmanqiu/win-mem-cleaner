# WinMemCleaner 测试指南

> 参照 Pecia 的测试体系设计。

## 一、测试策略

| 层级 | 测试类型 | 工具 | 覆盖目标 |
|------|----------|------|----------|
| 单元测试 | `#[test]` 函数 | 内置 `cargo test` | core 模块纯逻辑 |
| 集成测试 | `tests/` 目录 | `cargo test` | 模块间交互 |
| 冒烟测试 | `tests/smoke.rs` | `cargo test` | 基本功能不 panic + 配置便携性 |
| 文档门禁 | `tests/docs.rs` | `cargo test` | 文档即被 CI 读：清单/结构/许可/目录纯净/路径泄漏 |
| 安全门禁 | `tests/safety.rs` | `cargo test` | 不具备结束/注入其它进程的能力（本程序以管理员运行，这条是用户敢用的前提） |
| 手动测试 | 运行 exe | 人工 | UI 交互、托盘、清理效果（UI 轮次须出像素） |

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
| `smoke_config_load` | 配置默认值正确（含两种方式各自的阈值与强度） |
| `smoke_config_path_next_to_exe` | 配置路径在 exe 同级、不落 `%APPDATA%`（便携） |
| `smoke_config_save_next_to_exe` | `save()` 真的写在 exe 同级，`load()` 能读回 |
| `smoke_config_sanitize` | 配置校验逻辑正确 |
| `smoke_config边界值` | 配置边界值处理（间隔 1440、阈值 99） |
| `smoke_util_roundtrip` | UTF-8 ↔ UTF-16 往返一致性 |
| `smoke_util_empty` | 空字符串处理 |
| `smoke_util_ascii` | 纯 ASCII 处理 |
| `smoke_util_chinese` | 中文字符处理 |
| `smoke_format_bytes` | 字节格式化（B/KB/MB/GB） |
| `smoke_mem_info` | 内存信息可读取（Windows） |
| `smoke_physical_usage` | 物理内存使用率在合理范围（Windows） |
| `smoke_memory_snapshot` | 内存快照数据有效（Windows） |
| `smoke_is_admin` | 管理员检测函数可调用（Windows） |
| `smoke_clean_standard` | 标准清理不 panic（Windows） |
| `smoke_config_json_忽略已删除的旧键` | 老配置残留 `fullscreen_avoid` / `interval_level` / `threshold_level` / `level` 不导致读失败，用户设的值原样保留；且重新存盘后这四个键不再出现 |
| `smoke_last_run_starts_empty` | 「上次自动清理」初始为 None ⇒ 界面显示 `--:--` |
| `smoke_config_json_roundtrip` | 配置 JSON 序列化往返 |
| `smoke_config_json_unknown_fields` | JSON 未知字段容错 |
| `smoke_config_json_missing_fields` | JSON 缺失字段用默认值 |

### `tests/docs.rs` — 文档一致性门禁

让 `cargo test` 替我们记住「代码挪了、说明还留在原处」这类腐化。只查事实、不查文笔：

| 测试函数 | 钉住的事实 |
|----------|-----------|
| `controlled_docs_exist_and_are_substantive` | 受控文档都存在、非空、有实质内容（>10 行） |
| `every_doc_is_registered_in_manifest` | `docs/` 每个 `.md` 都登记在 DOCS_MANIFEST |
| `structure_doc_covers_every_source_module` | 结构说明点名了每个 `src/` 顶层模块 + 入口文件 |
| `structure_doc_named_files_exist` | 结构说明点名的每个 `.rs` 真实存在（删文件要删条目） |
| `repo_root_has_no_stray_files` | `main/` 根无白名单外文件（拦调试残留 / 草稿） |
| `license_is_verbatim_agpl3` | LICENSE 是逐字 AGPL-3.0、UTF-8、无 BOM、无本项目署名 |
| `no_local_absolute_paths_leak` | 任何文本文件不得出现**盘符绝对路径**（结构性判据：字母+冒号+单斜杠；`://` 协议前缀和时间戳不算）。判据不点名具体目录，所以**检测器自身也在扫描范围内**，且带四枚自检钉子（两种分隔符各认一次、三种误报各否一次） |

### `tests/safety.rs` — 安全门禁

用户的一句话判据：**「我们只是释放内存，不要关闭电脑上的其它软件」**。本程序是
`requireAdministrator`，所以这条不能靠"我看过代码了"，必须机器把守。

| 测试函数 | 钉住的事实 |
|----------|-----------|
| `我们不具备结束或注入其它进程的能力` | 扫 `main/` 下全部 `.rs`/`.ps1`/`.bat`/`.toml`/`.manifest`/`.json`，任何「结束进程 / 注入 / 操作别的窗口 / 借外部命令杀进程」的 API 名出现即红 |
| `门禁自身会红` | 四枚钉子：真词元判红、`EmptyWorkingSet` 不误报、现有合法 `OpenProcess` 掩码不误报、每条规则都能匹配到自己 |

禁止词元**分段存储、运行时拼接**——所以门禁文件自身也在扫描范围内，不需要给自己开豁免口子
（这条形态直接继承自 `no_local_absolute_paths_leak` 的教训：判据自身不得成为泄漏源）。

现存代码的越界面为零，靠的是设计上就没要这些权限：打开其它进程只请求
`PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION`，唯一的写操作是 `EmptyWorkingSet`；
唯一的子进程是 `schtasks.exe`（开机启动项）。

### Windows 特定测试

标记 `#[cfg(windows)]` 的测试仅在 Windows 上运行，涉及真实 Win32 API 调用。

## 四、UI 手动测试清单

由于 Win32 GUI 无法自动化测试，以下 UI 功能需手动验证：

### 主窗口
- [ ] 启动后窗口显示在屏幕右下角
- [ ] 标题栏固定显示「WinMemCleaner」（不带权限后缀）
- [ ] 三个进度条正确显示物理/虚拟/缓存内存
- [ ] 进度条颜色随占用变化（绿→黄→红）
- [ ] 进度条内百分比数字正确
- [ ] 窗口宽度自动适应文字长度
- [ ] **分组框左右边距对称**（都等于 `MARGIN`=14），右竖线**不压**窗口边框——v1.1.2 之前右边距实测 0
- [ ] **三行内存文本末字完整**（读到 `…/ 15.8 GB` 而不是 `…/ 15.8 G`），时刻列 `--:--` 五字符齐全
      —— 这两条是"控件字体没拿到"的判据，见 `platform_notes.md` 第 11 条

### 控件
- [ ] 「自动清理」分组内两行，列位置对齐：条件 / 数值 / 单位 / 动作（档位固定，没有"强度"列）
- [ ] 方式一「每 [x] 分钟 标准清理」：x 仅允许整数，档位固定为标准（无下拉）
- [ ] 方式二「内存 > [x] % 深度清理」：x 仅允许整数，档位固定为深度（无下拉）
- [ ] 默认显示：每 5 分钟 / 内存 > 85%
- [ ] 输入框失焦后自动保存到 exe 同级 `WinMemCleaner.json`
- [ ] **敲完数字直接点 X 关窗，值不丢**（关窗前会存盘）——重开窗口应显示刚才那个数
- [ ] 两行右侧各显示「上次自动清理」`hh:mm`；本次运行没自动清过就是 `--:--`
- [ ] 便携：换目录/换机器运行，用户目录 `%APPDATA%\memclean\` 不再生成或更新文件
- [ ] 开机启动复选框可勾选/取消

### 托盘
- [ ] 托盘图标显示内存占用百分比
- [ ] 托盘图标颜色随占用变化
- [ ] 鼠标悬停显示 tooltip（物理/虚拟/缓存三项 + 「已运行 X」，用于确认常驻进程在岗）
- [ ] 右键菜单从上到下依次为：窗口 / 标准清理 / 深度清理 / 关于 / 退出
- [ ] 双击托盘图标显示主窗口
- [ ] 点击「窗口」显示主窗口
- [ ] 点击「标准清理」执行清理
- [ ] 点击「深度清理」执行清理
- [ ] 点击「退出」退出程序

### 关于对话框
- [ ] 显示版本号「WinMemCleaner 1.1.0」
- [ ] 显示作者和邮箱
- [ ] 一行并排显示「Gitee仓库 | GitHub仓库」，中间用竖线间隔
- [ ] 两条链接各自可点击，分别打开 Gitee / GitHub 仓库
- [ ] 链接悬停变红色
- [ ] 微信/支付宝二维码正确显示
- [ ] 对话框居中显示

### 自动清理
- [ ] 方式一：设置间隔后，等待对应时间自动触发清理（该行右侧 `hh:mm` 随之更新）
- [ ] 方式二：物理内存占用超过阈值时自动触发清理（该行右侧 `hh:mm` 随之更新）
- [ ] 改完设置**不重启也生效**（最迟 5 秒：调度器每轮重读配置）
- [ ] 阈值触发后进入静默期（初始 60 秒），期间占用持续超阈值也不会反复清理
- [ ] 回弹退避：清理后 30 秒占用又越过阈值 ⇒ 静默期翻倍（上限 15 分钟）；复查通过则回到 60 秒
- [ ] 阈值触发的那一轮会把定时那一轮顺延，避免 5 秒内连清两次
- [ ] **没有任何"避让"会静默阻止清理**：后台有全屏覆盖层、前台是全屏窗口，到点照样清
      （1.1.2 删掉了全屏避让开关——它的判据 `QUNS_BUSY` 会被常驻覆盖层长期置真）

### 开机启动
- [ ] 勾选后创建计划任务 `WinMemCleaner`（登录触发 + 最高权限），并**删除**旧的 Run 值
- [ ] 取消勾选后计划任务与 Run 值都被删除
- [ ] 自愈：从哪个目录启动，启动项就指向哪个目录（改名/搬家不留死项）
- [ ] 非管理员组、建任务失败时退回 `HKCU\...\Run` 键（只留一条，不会开机拉两个实例）
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
