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

## 2. 静态控件的 `0x0200` 是 `SS_CENTERIMAGE`，不是 `SS_RIGHT`

- **现象**：关于对话框的仓库链接点击不灵（当时还记为"文字右偏"）。
- **根因**：静态控件要收到鼠标通知必须带 `SS_NOTIFY`（`0x0100`）。当年写成 `0x0200`，
  那一位是 `SS_CENTERIMAGE`（单行文字垂直居中），既没打开通知、也不改变水平对齐。
- **勘误**：本条原文把 `0x0200` 说成 `SS_RIGHT`，**是错的**——`SS_RIGHT = 0x00000002`。
  所以"右偏"这个现象当时就没有被解释对；真正被证实并修好的只有"点了没反应"。
- **判据**：`WinUser.h` 的 `Static Control Constants` 一整段：`SS_LEFT 0x0 / SS_CENTER 0x1 /
  SS_RIGHT 0x2 / SS_NOTIFY 0x100 / SS_CENTERIMAGE 0x200 / SS_RIGHTJUST 0x400 /
  SS_EDITCONTROL 0x2000`。⇒ **常量一律查头文件，不要背。** 本项目 `lbl()` 用的 `0x200`
  是有意要垂直居中。
- **出处**：v1.1.0 修复（常量改对，成因写错）；v1.1.2 复核 `WinUser.h` 时勘误。

## 3. `GetPerformanceInfo` 的数值字段单位是「页」

- **现象**：直接把它返回的 `SystemCache` 当字节数用，小了几千倍。
- **根因**：`PERFORMANCE_INFORMATION`（`Psapi.h`）里 `PhysicalTotal / SystemCache / CommitTotal` 等
  全是**页数**，须乘 `PageSize`。
- **判据**：本机实测 `SystemCache = 1,245,146 页 × 4096 = 4.75 GB`（占物理 30%）。
- **补（v1.1.2 实测）**：这台 **10.0.26200** 上 `GetPerformanceInfo` 只接受 `cb = 104`，传 88 直接
  `ERROR_BAD_LENGTH(24)`。差的 16 字节是 `CommitPeak` 和末尾的 `ThreadCount`——`windows` crate 0.58
  的定义里有，手写复刻时漏掉就会"API 怎么都失败"。**自己拼 Win32 结构体前先跟头文件逐字段核对。**
- **出处**：v1.1.0「系统缓存」读数修正；v1.1.2 采样器踩 `cb` 长度。

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
- **v1.1.3 补**：收到 `TaskbarCreated` 的那一次重加本身也可能失败（Shell 刚起来还没就绪）。
  现在的做法是失败不清线程，交给 1 秒定时器继续重试，见第 14 条。
- **出处**：v1.0.1 起已实现（`ui/tray.rs`）。

## 8. 含中文的 `.ps1` 被 PowerShell 5.1 按 GBK 解析

- **现象**：脚本里中文字符串导致整份 `ParserError: 字符串缺少终止符`。
- **根因**：无 BOM 的 UTF-8 `.ps1` 在 zh-CN 机器上按 GBK 读，某个多字节序列吞掉引号。
- **判据**：验证脚本**纯 ASCII**；要输出中文，写到文件再用 UTF-8 读回。
- **另一副面孔（v1.1.1 实测）**：不报错也能坏——`build/build.ps1` 注释行末的全角 `）`（UTF-8 `EF BC 89`）
  被 GBK 解码吞掉换行，下一行 `$env:CARGO_TARGET_DIR = ...` 被并进注释、从未执行；脚本直到
  「拷 exe 到 .build」才报 `Join-Path ... Path ... null`。修法：给 `.ps1` 存成**带 BOM 的 UTF-8**
  （PowerShell 5.1 只认 BOM），或全脚本纯 ASCII。
- **判据**：`[System.IO.File]::ReadAllBytes()` 前 3 字节为 `EF BB BF`；同一脚本加 BOM 后构建成功。
- **出处**：v1.1.0 像素验证脚本；v1.1.1 修 `build/build.ps1`。

## 9. `QUNS_BUSY` 是「系统里存在全屏窗口」，不是「前台在全屏」

- **现象**：物理内存到 95% 也不触发阈值清理；`is_game_or_fullscreen()` 恒为 true。
- **根因**：`SHQueryUserNotificationState()` 在普通桌面返回 `QUNS_BUSY(2)`——机器上有一个后台常驻的
  全屏覆盖层（实测 `CurSeen Overlay`，`WindUiWindowClass`，可见、未 cloaked、`0,0,1920,1080`）。
  `QUNS_BUSY` 的语义是「有全屏应用在跑」，**不要求在前台**；调度器每轮都走避让分支，一次都不清。
- **判据**：前台是普通最大化窗口（`Chrome_WidgetWin_1`，`-8,-8,1928,1040`，任务栏可见）时 QUNS 仍为 2；
  枚举铺满监视器的窗口只有该覆盖层 + `TextInputHost`（DWM cloaked=2）+ `Progman`（桌面）。
  改用「前台窗口是否铺满其所在显示器」后，同一时刻返回 false；用全屏窗口占住前台则返回 true。
- **出处**：v1.1.1 修复（改为只看前台窗口），见 `docs/changes.md`。
  **v1.1.2 进一步把「全屏避让」整个开关删掉了**——它带来的静默失效比偶尔一次卡顿更伤；
  取而代之的是清理后的「回弹退避」。本条作为 Win32 事实保留，防止将来重犯。

## 10. `requireAdministrator` 的程序不能只靠 `HKCU\...\Run` 常驻

- **现象（实测）**：用户报告"设了 80% 阈值清理却没效果"。查下来：机器连续开机 **6 天 10 小时**、
  托盘程序**无进程**；注册表 `Run` 项 `memclean` 指向**运行目录**那份 `WinMemCleaner.exe`（1.1.1），
  而开发产物在仓库侧的 `.build`（1.1.2）——**改的和你开机跑的不是同一个文件**。
  那份配置里 `threshold_percent: 80`、`autostart: true` 都是对的，设置从来没丢过。
- **机制推断（本机未验证：6 天没重启，复现不了登录场景）**：清单为 `requireAdministrator` 的程序放在
  `HKCU\Run`，登录时会弹 UAC 同意框；没人点 → 进程不启动 → 这段时间里"自动清理"根本不存在。
- **判据 / 解法**：开机自动提权要用**计划任务**（`schtasks /sc ONLOGON /rl HIGHEST`），不是 Run 键。
  1.1.2 起：首选计划任务、建不了退回 Run 键、两者只留一条，且**每次启动把启动项刷成当前 exe 路径**（自愈）。
- **怎么查**：`reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run"`（输出是 UTF-16，
  直接管道给 grep 会被判成二进制，先转码）+ `schtasks /query /tn WinMemCleaner`。
- **出处**：v1.1.2。★ 2026-10-30 真机重启已证实：计划任务确实在登录时刻（10:30:05）拉起了程序，
  上次结果 1 = 用户在任务管理器里结束了它。那次「托盘没有」是另一条缺陷（见第 14 条），不是启动失败。

## 11. 子控件**不继承**父窗口字体，`WM_SETFONT` 得逐个发

- **现象（实测）**：把 `apply_ui_font()` 从"窗口创建后"挪到"布局之前"之后，主界面所有文字变宽，
  末位字符被裁——`15.8 GB` 画成 `15.8 G`、`--:--` 画成 `--:-`。
- **根因**：`STATIC` / `BUTTON` / `EDIT` 都**不继承**父窗口的字体。原实现靠
  `apply_ui_font` 末尾那次 `EnumChildWindows` 给每个子控件补 `WM_SETFONT`；一旦它在子控件存在
  之前运行，就没有任何东西可补，控件全部退回系统默认 GUI 字体（比消息字体宽）。
- **判据 / 解法**：字体是控件自己的事——`make_child` / `edt` 在 `CreateWindowExW` 之后立刻
  `SendMessageW(child, WM_SETFONT, KEEP_FONT, TRUE)`。这样"先设字体再布局"和"先布局再设字体"
  两种顺序都成立，`measure_text_width` 量到的和画出来的才是同一个字体。
- **顺带**：本项目是 Per-MonitorV2，窗口先建在 `(-100,-100)` 再移到工作区，所以布局期用的 DC
  与最终落位可能不同上下文——字体对齐之后这条不再有可见影响，但改窗口尺寸逻辑时别指望它。
- **出处**：v1.1.2（自己改出来的回归，像素比对抓回来的）。

## 12. 外框尺寸 ≠ 客户区尺寸：`SetWindowPos` 要的是**外框**

- **现象（实测）**：「自动清理」分组框的右竖线画在窗口边框上，右边距 0；左右不对称（左边距 14）。
- **根因**：`create_ui` 里所有控件坐标都是**客户区**坐标，但 `box_w = win_w - MARGIN*2` 的 `win_w`
  是直接交给 `SetWindowPos` 的**外框**宽度。两者差一整圈非客户边框，凡是贴右边算的东西
  （分组框宽、右对齐的时刻列）都被推出客户区。标题栏高度同理——原来还硬编码了 `+31`。
- **判据 / 解法**：布局一律用客户区尺寸，最后一步换算：`frame_delta()` 用
  `GetWindowRect − GetClientRect` **实测**边框差再相加。边框宽度与标题栏高度都随系统版本 /
  主题 / DPI 变，写死数字迟早错。
- **出处**：v1.1.2。

## 13. `GetTextExtentPoint32W` 的字符数：windows-rs 传的是**切片长度**

- **现象**：`measure_text_width` 用 `util::wide(text)` 的结果去量宽。
- **根因**：`util::wide` 返回**带结尾 NUL** 的 `Vec<u16>`；windows-rs 的绑定把
  `lpstring.len()` 当作 Win32 的 `c`（字符数）参数传下去 ⇒ 多算一枚 NUL 字模的推进宽度。
  方向是"量多了"，所以不会裁字，但会让窗口比必要的宽。
- **判据**：`windows-0.58.0` 的绑定体 `GetTextExtentPoint32W(hdc, transmute(lpstring.as_ptr()),
  lpstring.len(), psizl)`——`c` 就是切片长度。
- **解法**：量之前把结尾 NUL 切掉：`&wide[..wide.len()-1]`。
- **出处**：v1.1.2 复核布局时顺手修。

## 14. 登录自启时 `Shell_NotifyIcon(NIM_ADD)` 会失败——失败不能丢线程

- **现象**（用户报，真机）：开机后托盘没有图标；进程在（任务管理器里结束的就是它）、界面能打开；
  结束进程再手动启动，图标正常。任务计划的「上次运行时间」= 登录时刻 ⇒ 自启本身是成功的。
- **根因**：登录瞬间 Explorer 的任务栏还没就绪，`NIM_ADD` 失败。原实现 `if !add_tray_icon(hwnd) { return; }`
  让托盘线程连同隐藏窗口、消息循环一起结束——之后既不重试，也收不到 `TaskbarCreated`。
  主窗口与进程由主线程维持，于是表现为「进程在、没有入口」。
- **判据（无人值守 A/B，本机）**：`CreateDesktopW` 建一个没有任务栏的桌面，用
  `STARTUPINFO.lpDesktop` 把程序拉上去，等 9 秒后 `EnumDesktopWindows` 数窗口类：
  - 旧代码：`MemCleanerTrayWindow` **不存在**（线程已退出），`MemCleanerMainWindow` 仍在、进程存活；
  - 新代码：托盘窗口**仍在**，`Shell_NotifyIconGetRect` 返回 `E_FAIL`（图标确实没进通知区）
    ⇒ 线程活着、在重试。
  - 反向校验：正常桌面下同一程序 `Shell_NotifyIconGetRect` 返回 `S_OK`，矩形落在底右任务栏区
    （实测 `1583,1032,1615,1080`，1920x1080）——「查不到」不是 API 失灵。
- **修法**：`TrayState.added` + 1 秒定时器重试；`TaskbarCreated` 那一次重加失败同样交给定时器。
- **出处**：v1.1.3。

## 15. `schtasks /create` 默认带 72 小时执行上限与电池限制

- **现象**：任务的「如果运行了 X 小时 X 分钟，停止任务」显示 `72:00:00`——常驻程序满三天被任务计划
  结束（托盘消失、自动清理也停），而且没有任何提示。默认还有「不用电池启动 / 切电池就停止」，
  笔记本上等于开机常驻随时下线。
- **根因**：`schtasks.exe` 命令行选项里没有执行时间上限和电池设置，建出来的任务吃 Task Scheduler 默认值。
- **判据 / 解法**：`schtasks /create /xml <file>` 显式写 `<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>`
  （不设上限）与 `DisallowStartIfOnBatteries=false` / `StopIfGoingOnBatteries=false`。
  本机实测：用同样的 XML 设置块建一次性探针任务，`schtasks /query /v` 显示「停止任务: **已禁用**」、
  「电源管理」一栏**为空**（默认任务那里写的是「在电池模式停止, 不用电池启动」）。
  XML 必须是 Unicode（UTF-16LE + BOM），中文路径才不会乱码；`<LogonTrigger>` 的任务需要管理员
  （本程序清单就是 `requireAdministrator`）。
- **出处**：v1.1.3。
