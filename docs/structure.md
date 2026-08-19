# WinMemCleaner 目录结构说明

> 接手项目必读。定义每个目录/文件的职责与规则。

## 一、目录总览

```
.main/
├── src/                源码
│   ├── main.rs         程序入口（精简，仅初始化+启动）
│   ├── lib.rs          库入口（集成测试用）
│   ├── core/           核心纯逻辑层（禁止 UI 代码）
│   │   ├── mod.rs      模块聚合
│   │   ├── mem.rs      内存清理核心
│   │   ├── config.rs   配置持久化
│   │   ├── scheduler.rs 自动调度
│   │   └── util.rs     工具函数
│   └── ui/             界面层（窗口/控件/托盘）
│       ├── mod.rs      模块聚合
│       ├── window.rs   主窗口
│       ├── tray.rs     系统托盘
│       └── about.rs    关于对话框
├── build/              构建脚本（源码，非产物）
│   └── build.ps1       PowerShell 构建脚本
├── image/              ← 图片资源（logo + 二维码）
├── docs/               ← 项目文档
├── Cargo.toml          ← 项目清单
├── Cargo.lock          ← 依赖锁定
├── build.rs            ← 构建脚本（ICO 生成 + manifest 嵌入）
├── app.manifest        ← Windows 清单（DPI/权限）
├── lib.rs              ← 库入口（集成测试用）
├── main.rs             ← 程序入口
├── .gitignore          ← Git 排除规则
└── README.md           ← 项目说明
```

## 二、各目录职责

### src/core/ — 纯逻辑层（Model）

**规则：禁止包含任何 UI 代码。** 这是可测试性的根基。

| 文件 | 职责 |
|------|------|
| `mem.rs` | 内存清理：进程工作集修剪、系统缓存清理、Native API 动态加载、权限提升、内存信息采集 |
| `config.rs` | 配置：JSON 持久化（%APPDATA%/memclean/config.json）、默认值、sanity check |
| `scheduler.rs` | 调度：后台线程定时清理 + 全屏/游戏避让 |
| `util.rs` | 工具：UTF-8 ↔ UTF-16 转换 |

### src/ui/ — 界面层（View + Controller）

**规则：只有这里可以创建窗口 / 弹对话框。**

| 文件 | 职责 |
|------|------|
| `window.rs` | 主窗口：UI 创建、窗口过程、自绘进度条、控件交互、开机启动、字体设置 |
| `tray.rs` | 系统托盘：实时内存%图标（绿→黄→红）、tooltip 三项指标、右键菜单 |
| `about.rs` | 关于对话框：版本、作者、Gitee 仓库链接、赞助二维码 |

### build/ — 构建脚本

**这是源码的一部分（要上传），不是构建产物。**

| 文件 | 职责 |
|------|------|
| `build.ps1` | PowerShell 构建脚本（Release/Debug/Clean） |

### image/ — 图片资源

所有图片类资源集中于此。命名约定：`<用途>.{png,svg}`。

| 文件 | 职责 |
|------|------|
| `logo.png` | 应用图标 PNG 源图 |
| `logo.svg` | 应用图标 SVG 矢量源图 |
| `WeChatPay.svg` | 微信收款二维码 |
| `ALiPay.svg` | 支付宝收款二维码 |
| `WeChatPay.png` | 微信收款二维码（build.rs 生成） |
| `ALiPay.png` | 支付宝收款二维码（build.rs 生成） |

## 三、依赖方向（强制）

```
core ← ui
```

- `core/` 禁止 `use crate::ui::*`
- `ui/` 可以 `use crate::core::*`
- `main.rs` 仅调用 `core::config::load()` 和 `ui::window::run()`

## 四、新增代码规则

1. **纯逻辑（无 UI）** → `core/`
2. **窗口/对话框/控件** → `ui/`
3. **构建相关** → `build/`
4. **图片/图标资源** → `image/`
5. **文档** → `docs/`

## 五、文件管理规范

| 位置 | 内容 |
|------|------|
| `src/core/` | 核心逻辑源码 |
| `src/ui/` | 界面源码 |
| `src/main.rs` | 入口 |
| `build/` | 构建脚本 |
| `image/` | 图片资源 |
| `docs/` | 项目文档 |

**对外发布时**：仅分发 `.build/WinMemCleaner-x64.exe`。
