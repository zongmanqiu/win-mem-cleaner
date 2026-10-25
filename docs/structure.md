# WinMemCleaner 目录结构说明

> 接手项目必读。定义每个目录/文件的职责与规则。
> 本文件受门禁约束：`tests/docs.rs` 会核对它点名了每个 `src/` 顶层模块，且点名的 `.rs` 真实存在。

## 一、目录总览

```
main/
├── src/                    源码
│   ├── main.rs             程序入口（单实例 + 配置 + 提权 + 启动 UI）
│   ├── lib.rs              库入口（集成测试用）
│   ├── core/               核心纯逻辑层（禁止 UI 代码）
│   │   ├── mod.rs          模块聚合
│   │   ├── mem.rs          内存清理核心 + 内存信息采集
│   │   ├── config.rs       配置持久化（exe 同级 JSON）
│   │   ├── scheduler.rs    自动调度（定时 / 阈值）
│   │   └── util.rs         工具函数（UTF-8 ↔ UTF-16）
│   └── ui/                 界面层（窗口/控件/托盘）
│       ├── mod.rs          模块聚合
│       ├── window.rs       主窗口
│       ├── tray.rs         系统托盘
│       └── about.rs        关于对话框
├── tests/                  测试
│   ├── smoke.rs            冒烟测试（核心逻辑 + 配置便携性）
│   └── docs.rs             文档一致性门禁
├── docs/                   项目文档（见 DOCS_MANIFEST.md）
├── build/                  构建脚本（源码，非产物）
│   └── build.ps1           PowerShell 构建脚本
├── .cargo/config.toml      把编译产物钉到 ../.temp/target
├── image/                  图片资源（logo + 二维码）
├── Cargo.toml              项目清单（版本号唯一来源）
├── Cargo.lock              依赖锁定
├── build.rs                构建脚本（logo.png → 多尺寸 ICO + manifest 嵌入 + VERSIONINFO）
├── app.manifest            Windows 清单（DPI / requireAdministrator）
├── LICENSE                 AGPL-3.0 官方逐字全文
├── README.md               项目说明
├── AGENTS.md               工程规则全集
├── DOCS_MANIFEST.md        文档清单（受门禁约束）
├── history.md              版本要点
└── .gitignore              Git 排除规则
```

## 二、各目录职责

### src/core/ — 纯逻辑层（Model）

**规则：禁止包含任何 UI 代码。** 这是可测试性的根基。

| 文件 | 职责 |
|------|------|
| `mem.rs` | 内存清理：进程工作集修剪、系统缓存清理、Native API 动态加载、权限提升、内存信息采集（物理/提交/缓存） |
| `config.rs` | 配置：JSON 持久化（exe 同级 `WinMemCleaner.json`，便携）、默认值、sanity check |
| `scheduler.rs` | 调度：后台线程轮询，定时清理 / 内存阈值清理 + 全屏/游戏避让 |
| `util.rs` | 工具：UTF-8 ↔ UTF-16 转换 |

### src/ui/ — 界面层（View + Controller）

**规则：只有这里可以创建窗口 / 弹对话框。**

| 文件 | 职责 |
|------|------|
| `window.rs` | 主窗口：UI 创建、窗口过程、自绘进度条、控件交互、开机启动、字体设置 |
| `tray.rs` | 系统托盘：实时内存%图标（绿→黄→红）、tooltip 三项指标、右键菜单 |
| `about.rs` | 关于对话框：版本、作者、仓库链接（Gitee / GitHub）、赞助二维码 |

### tests/ — 测试

| 文件 | 职责 |
|------|------|
| `smoke.rs` | 冒烟测试：核心逻辑、配置默认值 / sanitize / JSON 往返 / 便携路径 |
| `docs.rs` | 文档一致性门禁：清单文件存在、结构文档覆盖 src 模块、LICENSE 逐字、目录纯净、无路径泄漏 |

### docs/ — 项目文档

六份受控文档，登记与更新时机见 `DOCS_MANIFEST.md`：`architecture` / `structure` / `api` /
`testing` / `changes` / `platform_notes`。

### build/ — 构建脚本

**这是源码的一部分（要上传），不是构建产物。**

| 文件 | 职责 |
|------|------|
| `build.ps1` | PowerShell 构建脚本（Release/Debug/Clean），产物拷进 `.build/` |

### image/ — 图片资源

| 文件 | 职责 |
|------|------|
| `logo.png` | 应用图标源图；`build.rs` 读它光栅化出多尺寸 `logo.ico`（exe / 窗口图标） |
| `WeChatPay.png` | 微信收款二维码（提交进仓库的静态资源，`about.rs` 用 `include_bytes!` 内嵌） |
| `ALiPay.png` | 支付宝收款二维码（同上） |

> 注：二维码是**提交的 PNG 源**，不由构建生成；`build.rs` 只生成 `logo.ico`。

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
5. **文档** → `docs/`（并在 `DOCS_MANIFEST.md` 登记）
6. **测试** → `tests/`

## 五、产物与工作区（不在源码树里）

| 位置 | 内容 | 入库 |
|------|------|------|
| `../.temp/target/` | 编译中间产物（由 `.cargo/config.toml` 钉死） | ❌ |
| `../.temp/` | 一次性验证脚本 / 实测转储 | ❌ |
| `../.build/` | 发布产物 `WinMemCleaner_x64.exe` | ❌ |

**对外发布时**：仅分发 `.build/WinMemCleaner_x64.exe`（+ 同级 `WinMemCleaner.json` 由程序自生成）。
