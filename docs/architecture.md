# WinMemCleaner 架构总览

> 本文档描述源码的分层架构与数据流。

- 语言：Rust (edition 2021)
- UI：原生 Win32 API（无框架依赖）
- 构建：Cargo + winres（manifest/图标嵌入）
- 平台：仅 Windows x64

---

## 一、分层与依赖方向

```
core/  ←  ui/
```

- **`core/`（纯逻辑层）**：内存清理、配置持久化、自动调度、工具函数。禁止引用任何 UI 代码。
- **`ui/`（界面层）**：主窗口、系统托盘、关于对话框。唯一可以创建窗口 / 弹对话框的层。

### 依赖规则
- `core/` 禁止 `use crate::ui::*`
- `ui/` 可以 `use crate::core::*`
- `main.rs` 仅调用 `core::config::load()` 和 `ui::window::run()`

---

## 二、模块职责

### core/ — 纯逻辑层

| 模块 | 职责 |
|------|------|
| `mem.rs` | 内存清理核心：进程工作集修剪、系统缓存清理、Native API 动态加载、权限提升 |
| `config.rs` | 配置持久化：JSON 读写、默认值、sanity check |
| `scheduler.rs` | 自动调度：定时清理 + 全屏避让 |
| `util.rs` | 工具函数：UTF-8/UTF-16 转换 |

### ui/ — 界面层

| 模块 | 职责 |
|------|------|
| `window.rs` | 主窗口：UI 创建、窗口过程、控件、自绘进度条、开机启动 |
| `tray.rs` | 系统托盘：实时内存%图标、tooltip、右键菜单 |
| `about.rs` | 关于对话框：版本、作者、仓库链接、赞助二维码 |

### main.rs — 入口

精简入口，仅负责：
1. 单实例互斥（Mutex）
2. 配置加载
3. 权限提升
4. 调用 `ui::window::run()`

---

## 三、数据流

```
启动
 ├─ 单实例互斥 → 已有实例则激活其窗口并退出
 ├─ 配置加载（%APPDATA%/memclean/config.json）
 ├─ 权限提升（raise_privileges）
 └─ ui::window::run()
      ├─ 创建主窗口 + 控件
      ├─ 启动 scheduler（后台线程定时清理）
      ├─ 启动 tray（后台线程托盘图标）
      └─ Win32 消息循环
           ├─ WM_TIMER → 刷新内存信息
           ├─ 托盘消息 → 清理/显示/退出
           └─ 控件消息 → 保存配置/触发清理
```

---

## 四、清理策略

| 档位 | 操作 | 说明 |
|------|------|------|
| 标准 | 合并物理内存页 + 裁剪进程工作集 + 清理系统文件缓存 + 低优先级备用内存 | 温和，几乎不卡 |
| 深度 | 标准全部 + 系统级清空工作集 + 清空 Standby + 冲刷 Modified 页 | 强力，短暂卡顿 |

---

## 五、构建

```powershell
# Release 构建
.\build\build.ps1

# Debug 构建
.\build\build.ps1 -Debug

# 清理
.\build\build.ps1 -Clean
```

产物：`../build/WinMemCleaner-x64.exe`（约 0.4MB）
