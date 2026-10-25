# Win32 实测库（PLATFORM_NOTES）

> **规则**：凡与平台 / 系统行为相关的结论，凭印象不算数——先写最小探针或真机测过，再进本文件。
> 每条格式：**现象 / 根因 / 判据（怎么验）/ 出处**。SDK 头文件（本机 `Windows Kits\10\Include\...`）
> 是常量名与值的最终依据。

---

## 1. `EN_KILLFOCUS` = `0x0200`，不是 `8`

- **现象**：主窗口「间隔 / 阈值」数字框改了值，不点别的控件就**不落盘**。
- **根因**：EDIT 控件的失焦通知 `EN_KILLFOCUS` 是 `0x0200`（512）。代码里写成 `notify == 8`，
  而 `8` 是组合框的 `CBN_CLOSEUP`——从组合框分支复制过来时串了味，条件恒假。
- **判据**：`WinUser.h` 里 `EN_SETFOCUS 0x0100 / EN_KILLFOCUS 0x0200 / EN_CHANGE 0x0300`；
  `CBN_CLOSEUP 8 / CBN_SELENDOK 9`。
- **出处**：v1.1.0 修复，见 `docs/changes.md`。

## 2. `SS_NOTIFY` = `0x0100`，`0x0200` 是 `SS_RIGHT`

- **现象**：关于对话框的仓库链接文字右偏、点击不灵。
- **根因**：静态控件要收鼠标通知必须 `SS_NOTIFY`（`0x0100`）；写成 `0x0200` 实际设的是 `SS_RIGHT`（右对齐）。
- **判据**：`WinUser.h` 中 `SS_CENTER 0x00000001`、`SS_NOTIFY 0x00000100`。
- **出处**：v1.1.0 修复。

## 3. `GetPerformanceInfo` 的数值字段单位是「页」

- **现象**：直接把它返回的 `SystemCache` 当字节数用，小了几千倍。
- **根因**：`PERFORMANCE_INFORMATION`（`Psapi.h`）里 `PhysicalTotal / SystemCache / CommitTotal` 等
  全是**页数**，须乘 `PageSize`。
- **判据**：本机实测 `SystemCache = 1,245,146 页 × 4096 = 4.75 GB`（占物理 30%）。
- **出处**：v1.1.0「系统缓存」读数修正。

## 4. 未文档化的 `SystemFileCacheInformation` 结构布局不可信

- **现象**：托盘 / 主窗口的「系统缓存」读数与文档口径差 **13 倍**（0.36 GB vs 4.75 GB），
  且拿「缓存峰值」当分母显示成「已用 / 总量」的样子。
- **根因**：`NtQuerySystemInformation(class 81)` 的结构体 SDK 头文件里根本没有，字段偏移靠猜；
  同一次调用里 `off16` 在不同时刻读出过指针样与计数样两种值，证明布局不稳。
- **判据**：`grep SYSTEM_FILE_CACHE_INFORMATION` 在 SDK `Include/` 下**无命中**（未文档化）。
  改用文档化 `GetPerformanceInfo().SystemCache` 后与 Task Manager「已缓存」量级一致。
- **出处**：v1.1.0，`core::mem::system_cache_bytes()`。

## 5. `requireAdministrator` + UIPI：跨完整性 `PostMessage` 静默失败

- **现象**：非提权脚本 `FindWindowW` 能找到主窗口，但 `PostMessage(WM_APP+4)` 开「关于」没反应，
  `PostMessage(WM_APP+3)` 退出也没反应。
- **根因**：窗口属 High 完整性进程；Medium 完整性进程的窗口消息被 UIPI **静默丢弃**（`PostMessage`
  返回 false，不抛错）。`FindWindow` 枚举不受限，所以「能找到 = 能发消息」是错觉。
- **判据**：把脚本用 `Start-Process -Verb RunAs` 提权重跑，同一 `PostMessage` 即返回 true、界面响应。
- **出处**：v1.1.0 像素验证期。

## 6. 清理效果必须在提权进程里量

- **现象**：非提权跑测试，`clear_system_file_cache()` / `clear_standby*()` 全返回 false，
  「系统缓存」读数纹丝不动，会误判成「清理没效果」。
- **根因**：这些原语依赖 `SeIncreaseQuotaPrivilege` / `SeProfileSingleProcessPrivilege`，非提权拿不到。
- **判据（提权实测，15.8 GB 机器，单位 GB）**：系统缓存 6.33 →（`clear_standby`）**0.25**；
  可用物理 8.34 →（标准清理）9.77 →（深度清理）**12.32**。
- **附带未决**：`SetSystemFileCacheSize(0,0)` 后**立刻**恢复不限，函数返回 true 但缓存读数 0 变化
  ——疑似没给缓存管理器回收时间，**未修**，下次动它先复测。

## 7. Explorer 重启后托盘图标消失

- **现象**：`explorer.exe` 重启（崩溃 / 手动重启）后托盘图标没了。
- **根因**：托盘图标随 Shell 生命周期失效。
- **判据**：`RegisterWindowMessageW("TaskbarCreated")` 收到即重新 `Shell_NotifyIconW(NIM_ADD)`。
- **出处**：v1.0.1 起已实现（`ui/tray.rs`）。

## 8. 含中文的 `.ps1` 被 PowerShell 5.1 按 GBK 解析

- **现象**：脚本里中文字符串导致整份 `ParserError: 字符串缺少终止符`。
- **根因**：无 BOM 的 UTF-8 `.ps1` 在 zh-CN 机器上按 GBK 读，某个多字节序列吞掉引号。
- **判据**：验证脚本**纯 ASCII**；要输出中文，写到文件再用 UTF-8 读回。
- **出处**：v1.1.0 像素验证脚本。
