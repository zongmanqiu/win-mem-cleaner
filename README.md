# WinMemCleaner — 轻量 Windows 内存清理工具

**[⬇️ 下载 1.1.1 (Gitee)](https://gitee.com/qiuzongman/win-mem-cleaner/releases/download/1.1.1/WinMemCleaner_x64_1.1.1.zip)**
 ｜ **[⬇️ 下载 1.1.1 (GitHub)](https://github.com/zongmanqiu/win-mem-cleaner/releases/download/1.1.1/WinMemCleaner_x64_1.1.1.zip)**
 ｜ [Gitee主仓](https://gitee.com/qiuzongman/win-mem-cleaner)
 ｜ [GitHub镜像](https://github.com/zongmanqiu/win-mem-cleaner)

纯 Rust + 原生 Win32 API 实现，无运行库依赖，单文件 exe 约 0.4 MB。托盘实时显示内存占用，
一键标准 / 深度清理，可按时钟或内存阈值自动清理。

> 仅支持 Windows 10 及以上（x64），详见[运行环境](#运行环境)。极简设计，无技术术语，把用户当傻瓜。

## 功能

- **两级内存清理**：标准（温和）/ 深度（强力，短暂卡顿）
- **实时监控**：进度条 + 托盘图标显示物理/虚拟/缓存内存
- **自动清理**：两种方式可各自配置、同时生效——每 N 分钟清理一次 / 内存占用超过 X% 时清理，各自选择强度；支持全屏避让
- **开机启动**：注册表静默自启
- **便携**：配置文件 `WinMemCleaner.json` 与 exe 同级，不往用户目录写文件，拷走即带走全部设置
- **单实例**：第二次启动自动激活已有窗口
- **极小体积**：单文件 exe 约 0.4 MB，无运行库依赖

## 运行环境

| 项 | 要求 |
|----|------|
| 操作系统 | **Windows 10 及以上，仅 x64**（含 Windows 11） |
| 权限 | 管理员（清单 `requireAdministrator`；清空系统工作集等操作需要） |
| 运行库 | 无（Rust 静态链接，单文件 exe） |
| 配置位置 | 与 exe 同级的 `WinMemCleaner.json`（便携，不写用户目录） |

- **不支持 Win7 / 8 / 8.1**：这不是本项目自己设的门槛——Rust 官方把 `x86_64-pc-windows-msvc` 目标的底线定在 Windows 10（Rust 1.78 起，1.77.2 是最后一个能在 Win7 上跑的版本），本项目工具链是 1.97。
- **Win10 建议 1703 及以上**：逐屏 DPI（`PerMonitorV2`）从 1703 才被识别，更早的 Win10 退回系统级 DPI 感知，高分屏上文字可能被拉伸；清理功能本身各 Win10 版本一致。
- 清单里没有 `<compatibility>/<supportedOS>` 声明，进程会以 Vista 兼容上下文运行（任务管理器「操作系统上下文」列可见）。功能不受影响，属已知取舍。
- **内存阈值方式只判物理内存占用**：界面上的「虚拟内存」是提交量 / 提交上限，「系统缓存」是实时缓存占物理内存的比例，两者都不表示"内存不够用"，而且清空工作集与缓存也降不下提交量，所以都不参与触发。

## 架构

```
core/  ←  ui/       依赖方向（强制）
```

| 层 | 职责 |
|----|------|
| `core/` | 纯逻辑：内存清理、配置、调度（禁止 UI 代码） |
| `ui/` | 界面：主窗口、托盘、关于对话框 |

详见 [docs/architecture.md](docs/architecture.md)。

## 快速开始

### 环境要求

- 构建机：Windows 10/11 x64
- [Rust 工具链](https://rustup.rs/) **≥ 1.80**（代码用到 c-string 字面量＝1.77 与预置 `size_of`＝1.80；本机 1.97 构建通过）

### 构建

```powershell
cd main

# 方式一：构建脚本（推荐）
.\build\build.ps1            # Release
.\build\build.ps1 -Debug     # Debug
.\build\build.ps1 -Clean     # 清理缓存

# 方式二：手动（中间产物同样归到 .temp/）
$env:CARGO_TARGET_DIR = "..\.temp\target"
cargo build --release
```

产物：`../.build/WinMemCleaner.exe`（约 0.4MB）

### 测试

```powershell
cargo test --test smoke    # 21 个冒烟测试（核心逻辑 + 便携配置）
cargo test --test docs     # 7 条文档一致性门禁（文档即被 CI 读）
cargo test                 # 运行全部测试
```

## 目录结构

> 每个目录/文件的职责与规则见 [docs/structure.md](docs/structure.md)（受门禁核对），这里只给顶层。

```
main/
├── src/            源码：core/（纯逻辑）+ ui/（窗口/托盘/关于）
├── tests/          smoke.rs（冒烟）+ docs.rs（文档门禁）
├── docs/           项目文档
├── build/          构建脚本（源码，非产物）
├── image/          图片资源（logo + 二维码）
├── .cargo/         config.toml：编译产物钉到 ../.temp/target
├── src/main.rs     程序入口 · src/lib.rs 库入口（测试用）
├── Cargo.toml      版本号唯一来源 · build.rs 生成 ICO + 嵌 manifest
├── app.manifest    Windows 清单（DPI / requireAdministrator）
├── AGENTS.md       工程规则全集 · DOCS_MANIFEST.md 文档清单
├── history.md      版本要点 · LICENSE  AGPL-3.0 全文
└── README.md       本文档
```

## 文档

| 文件 | 说明 |
|------|------|
| [AGENTS.md](AGENTS.md) | 工程规则全集（临时产物 / 分层 / 便携铁律 / 环境坑 / 交付清单） |
| [docs/architecture.md](docs/architecture.md) | 架构总览 |
| [docs/structure.md](docs/structure.md) | 目录结构说明 |
| [docs/api.md](docs/api.md) | API 参考 |
| [docs/testing.md](docs/testing.md) | 测试指南 |
| [docs/platform_notes.md](docs/platform_notes.md) | Win32 实测库（每条带判据） |
| [docs/changes.md](docs/changes.md) | 更新日志 |
| [history.md](history.md) | 版本要点 |
| [DOCS_MANIFEST.md](DOCS_MANIFEST.md) | 文档清单（谁验证 / 何时更新） |

## 技术栈

| 组件 | 说明 |
|------|------|
| Rust | 主语言（edition 2021） |
| Win32 API | 原生 UI + 内存清理（via `windows` crate） |
| serde/serde_json | 配置持久化 |
| winres | Windows 资源嵌入（manifest + VERSIONINFO） |
| image / ico | `build.rs` 从 `logo.png` 生成多尺寸 ICO（exe/窗口图标） |

## 清理策略

| 档位 | 操作 | 说明 |
|------|------|------|
| 标准 | 合并物理内存页 + 裁剪进程工作集 + 清理系统缓存 + 低优先级备用内存 | 温和，几乎不卡 |
| 深度 | 标准全部 + 系统级清空工作集 + 清空 Standby + 冲刷 Modified 页 | 强力，短暂卡顿 |

## 许可证

[AGPL-3.0](LICENSE)

Copyright (C) 2026 邱宗满
