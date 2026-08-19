# WinMemCleaner x64

[**📦 立即下载 v1.0.0**](https://gitee.com/qiuzongman/win-mem-cleaner/releases/download/1.0.0/WinMemCleaner-x64_1.0.0.zip)

轻量级 Windows x64 内存清理工具，纯 Rust + 原生 Win32 API 实现。

> 仅支持 Windows 10/11 64 位。极简设计，无技术术语，把用户当傻瓜。

## 功能

- **两级内存清理**：标准（温和）/ 深度（强力，短暂卡顿）
- **实时监控**：进度条 + 托盘图标显示物理/虚拟/缓存内存
- **自动清理**：可设定时清理间隔，支持全屏避让
- **开机启动**：注册表静默自启
- **单实例**：第二次启动自动激活已有窗口
- **极小体积**：exe 约 0.4MB，运行内存 2~7MB

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

- Windows 10/11 x64
- [Rust 工具链](https://rustup.rs/)（≥ 1.70）

### 构建

```powershell
cd .main

# 方式一：构建脚本（推荐）
.\build\build.ps1            # Release
.\build\build.ps1 -Debug     # Debug
.\build\build.ps1 -Clean     # 清理缓存

# 方式二：手动
$env:CARGO_HOME = "..\.build\cargo"
$env:CARGO_TARGET_DIR = "..\.build\target"
cargo build --release
```

产物：`../build/WinMemCleaner-x64.exe`（约 0.4MB）

### 测试

```powershell
cargo test --test smoke    # 运行 18 个冒烟测试
cargo test                 # 运行全部测试
```

## 目录结构

```
.main/
├── src/
│   ├── main.rs          入口（单实例 + 配置 + 提权 + 启动 UI）
│   ├── lib.rs           库入口（集成测试用）
│   ├── core/            纯逻辑层
│   │   ├── mem.rs       内存清理核心
│   │   ├── config.rs    配置持久化（JSON）
│   │   ├── scheduler.rs 自动调度
│   │   └── util.rs      工具函数
│   └── ui/              界面层
│       ├── window.rs    主窗口
│       ├── tray.rs      系统托盘
│       └── about.rs     关于对话框
├── tests/smoke.rs       冒烟测试
├── build/build.ps1      构建脚本
├── image/               图片资源
├── docs/                项目文档
├── Cargo.toml
├── build.rs             构建脚本（ICO + manifest）
└── app.manifest         Windows 清单
```

## 文档

| 文件 | 说明 |
|------|------|
| [docs/architecture.md](docs/architecture.md) | 架构总览 |
| [docs/structure.md](docs/structure.md) | 目录结构说明 |
| [docs/api.md](docs/api.md) | API 参考 |
| [docs/testing.md](docs/testing.md) | 测试指南 |
| [docs/changes.md](docs/changes.md) | 更新日志 |
| [DOCS_MANIFEST.md](DOCS_MANIFEST.md) | 文档清单 |

## 技术栈

| 组件 | 说明 |
|------|------|
| Rust | 主语言（edition 2021） |
| Win32 API | 原生 UI + 内存清理（via `windows` crate） |
| serde/serde_json | 配置持久化 |
| winres | Windows 资源嵌入（manifest + 图标） |
| resvg/usvg | SVG 二维码转 PNG（构建时） |

## 清理策略

| 档位 | 操作 | 说明 |
|------|------|------|
| 标准 | 合并物理内存页 + 裁剪进程工作集 + 清理系统缓存 + 低优先级备用内存 | 温和，几乎不卡 |
| 深度 | 标准全部 + 系统级清空工作集 + 清空 Standby + 冲刷 Modified 页 | 强力，短暂卡顿 |

## 许可证

[AGPL-3.0](LICENSE)

Copyright (C) 2026 邱宗满
