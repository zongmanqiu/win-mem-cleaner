# WinMemCleaner 工程规则（AGENTS.md）

本文件对仓库根目录（`main/`，即本文件所在目录）及其全部子目录生效，是**规则全集**。
任何 AI 工具或接手者在此工作时须遵守。开发流程细节见 `docs/testing.md` 与 `DOCS_MANIFEST.md`；
Win32 实测结论单独归档在 `docs/platform_notes.md`（本文只引用、不复制，防两处漂移）。

> 这里每条「环境坑 / 陷阱」都是**踩过才知道**的，附现象与判据；改代码前先搜本文。

---

## 一、临时产物与目录纯净性（硬性）

| 位置 | 内容 | 入库 |
|---|---|---|
| `main/` | 手写源码（`src/`）、测试（`tests/`）、构建脚本（`build.rs` + `build/`）、文档（`*.md` + `docs/`）、图片资源（`image/`） | ✅ |
| 项目根 `.temp/` | 编译中间产物（`target/`）、一次性验证脚本、实测转储、计划/进度文件 | ❌ 可随时清理 |
| 项目根 `.build/` | 发布产物 `WinMemCleaner_x64.exe` | ❌ |

1. **`main/` 只放手写代码与受控文档**：日志、`.txt` 草稿、调试残留、编译产物一律不进。
   门禁 `tests/docs.rs::repo_root_has_no_stray_files` 钉住 `main/` 根文件白名单。
2. **编译中间产物落 `.temp/target`**：由 `main/.cargo/config.toml` 钉死，**不要手设 `CARGO_TARGET_DIR`**。
   本项目 debug+release 一轮约 590 MB，收工前确认 `.temp/target` 已清或磁盘有余量。
3. **`temp/` 是一次性区**：脚本随时可能被清理。「文档里写过」不等于「脚本还在」，照文档引用前先确认文件在。

---

## 二、分层与依赖方向（强制）

```
core/  ←  ui/        依赖方向（箭头右侧可依赖左侧，反向禁止）
```

- `core/` 是纯逻辑层：内存清理、配置、调度、工具。**禁止 `use crate::ui`**、禁止建窗口 / 弹对话框。
  这是可测试性的根基——`tests/` 能在无 GUI 下跑完核心逻辑。
- `ui/` 是界面层：主窗口、托盘、关于对话框。只有这里可以创建窗口。
- `main.rs` 仅做：单实例互斥 → 配置加载 → 提权 → `ui::window::run()`。

---

## 三、便携性铁律（硬性）

- **配置只落 exe 同级的 `WinMemCleaner.json`**（`core::config::config_path()` 由 `current_exe()` 推导）。
  取不到 exe 路径时返回 `None` ⇒ 宁可不存盘，**绝不**退回 `%APPDATA%` / `USERPROFILE` / 当前目录。
- 我们是便携程序：**不往用户目录写任何文件**。历史上配置曾在 `%APPDATA%\memclean\`，已废弃且不迁移
  （便携语义下偷读用户目录 = 换台机器带错设置）。
- **唯一被接受的外部状态** = 开机启动写的注册表 `HKCU\...\Run` 值（自启只能靠它）；它记 exe 全路径，
  换目录后需重勾一次。
- 门禁：`tests/smoke.rs::smoke_config_path_next_to_exe` + `smoke_config_save_next_to_exe`（已用旧
  APPDATA 实现验过会红）。

---

## 四、内存判据（已定案，勿重开）

- **自动清理的内存阈值只判物理内存占用**（`physical_usage_percent()`）。
- 界面上另外两行**不参与触发**，因为它们的定义不适合当触发条件：
  - 「虚拟内存」= 提交量 / 提交上限（`ullTotalPageFile` 是 物理+页面文件）；清空工作集降不下提交量。
  - 「系统缓存」= 实时缓存占物理内存比；缓存本就可回收，它高 ≠ 该清。
- 用户提过「三个任一达到就触发」，实测三指标后**否决**（见 `docs/platform_notes.md`）。
- 版本要求：**仅 Windows 10 及以上 x64**——根因是 Rust 工具链底线即 Win10，非本项目自设门槛。
  清单不加 `supportedOS`，靠 README 说明（也是定案）。

---

## 五、这台机器的执行环境（踩过的坑，硬性）

| 坑 | 对策 |
|---|---|
| exe 是 `requireAdministrator`：**非提权进程能 `FindWindowW` 但 `PostMessageW` 被 UIPI 静默丢弃**（返回 false 不报错） | 要驱动 / 退出 / 开「关于」，把脚本用 `Start-Process -Verb RunAs` 提权跑；提权子进程 stdout 回不来 ⇒ 结果写 `.temp/*.log` 再读 |
| **含中文的 `.ps1` 被 PowerShell 5.1 按 GBK 读**，吞掉引号导致整份不解析 | 验证脚本**纯 ASCII**；中文输出用 UTF-8 读回 |
| PowerShell 前台 stdout 常取不到 | 要读输出就写文件再 `Read` |
| `cargo test` 里调清理原语时**非提权**，`clear_system_file_cache` / `clear_standby*` 全返回 false 且读数不动 | 测清理效果必须**提权跑测试二进制**，并把每个原语返回值一起打印，否则会把「没权限」误读成「没效果」 |
| `build.rs` 的 `rerun-if-changed` 指向不存在路径 ⇒ cargo 判「永远脏」每次全量重编 | 新增 `rerun-if-changed` 前先确认目标真实存在 |
| 脚本化改文件：`utf-8-sig` 读写会**悄悄加 BOM**；转义层可能吃掉路径里的字母 | 改完数一遍 BOM 与 CRLF/LF；路径锚点用 `chr(92)` |

---

## 六、源码内已知陷阱（改代码前必读）

Win32 的**实测事实**（常量值、结构布局、静默失败行为）集中在 `docs/platform_notes.md`，一处一条、
带判据。当前几条要命的：

- **`EN_KILLFOCUS` 是 `0x0200`，不是 `8`**（`8` 是 `CBN_CLOSEUP`）：EDIT 失焦保存判定用错常量会导致
  「只改数字不落盘」。
- **`SS_NOTIFY` 是 `0x0100`，不是 `0x0200`**（`0x0200` 是 `SS_RIGHT`）：静态控件链接写错会右偏 + 收不到点击。
- **`GetPerformanceInfo` 的每个数值字段单位是「页」**，要乘 `PageSize`。
- **未文档化的 `SystemFileCacheInformation` 结构布局不可靠**（SDK 里没有它）：本项目曾据此把「系统缓存」
  读错 13 倍，改用文档化 API 后修正。

---

## 七、编码纪律

1. **`unsafe` 块要写 `// SAFETY:`**（义务与证据，不是复述操作）。本项目大量裸 Win32 调用。
2. **注释写「为什么」不写「做了什么」**；每条非常规取舍都留「踩过才知道」的理由。
3. **不 panic 在交付路径**：`main` 启动早期可 `unwrap`，其余用 `unwrap_or_default` / `let-else` 兜底；
   托盘 / 窗口过程不得 panic。
4. **单一来源 + 护栏**：同一常量 / 规则只在一处定义（版本号在 `Cargo.toml`、流程在本文与 testing.md），
   其它引用它；关键不变量用测试钉死。
5. **禁用 `#![allow(dead_code)]` 当遮羞布**：全仓已摘除该属性（`do_clean_all` 就是被它养烂的——
   挂着「未文档化 API 全套实现」却无人调用）。新加私有项若没人用，编译器会报，**删掉它而不是压掉警告**。
6. **先实测再下结论**：凡与平台 / 系统行为相关的判断，先用最小探针验证，再写进代码或文档。
7. **UI 轮次要出像素**：改了界面就真机截图量化（`GetWindowRect` 定矩形、屏幕区域捕获，两通道一致才算数），
   默认开启、给用户看见，别只改代码说「完成了」。
8. **文档同步**：改了源码 / API / 目录，按 `DOCS_MANIFEST.md` 逐项核对；`tests/docs.rs` 会读文档并把
   漏改变红。新增文档必须登记，否则「未受控文档进仓库」门禁红。

---

## 八、交付前核查清单（硬性）

按顺序全部通过方可答复「完成了」：

| # | 项 | 命令 | 通过判据 |
|---|---|---|---|
| 1 | 测试 | `cargo test` | 全绿，数量不少于上一版（含 `tests/docs.rs` 文档门禁） |
| 2 | 静态 | `cargo build` | **第一方源码零 warning** |
| 3 | 构建产物 | `build/build.ps1`（或 `cargo build --release`） | exe 拷进 `.build/`，`VERSIONINFO` 与 `Cargo.toml` 版本一致 |
| 4 | 像素 | 提权跑一次界面 | UI 轮次：主窗口 / 托盘 / 关于截图符合预期 |
| 5 | 文档 | 逐项核对 `DOCS_MANIFEST.md` | 清单里文件都存在且描述成立 |
| 6 | 卫生 | 见 `tests/docs.rs` | 无游离文件、无本机绝对路径、LICENSE 逐字 |

> 本清单只检查、不提交。git 提交须先取得用户明确许可。
