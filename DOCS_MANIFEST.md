# 文档清单（DOCS_MANIFEST）

> 列出全部**随代码变化**的文档、何时更新、由谁验证。每次改动收尾逐项核对本表。
> `tests/docs.rs` 会验证「清单里的文件真实存在且非空、docs/ 全部登记、结构文档覆盖每个 src 模块」——
> 改文档后必须保持全绿。
>
> **两分法**：有长期价值的文档进 `main/docs/` 入库并在本表登记；一次性草稿 / 实测转储放项目根
> `.temp/`，不入库（`repo_root_has_no_stray_files` 门禁核对 `main/` 根无游离文件）。

## 一、必须随代码同步的文档（受门禁约束）

| # | 文件 | 何时必须更新 | 由谁验证 |
|---|---|---|---|
| 1 | `AGENTS.md` | 项目规则变化；发现新的环境坑 / 源码陷阱；新增硬约束 | `controlled_docs_exist_and_are_substantive` |
| 2 | `README.md` | 顶层目录树 / 技术栈 / 构建 / 运行环境 / 功能描述变化 | `controlled_docs_exist_and_are_substantive` |
| 3 | `DOCS_MANIFEST.md`（本文件） | 清单本身变化时 | `every_doc_is_registered_in_manifest`（自证） |
| 4 | `history.md` | 发新版本：顶部按主题追加**要点**，旧内容不改 | `controlled_docs_exist_and_are_substantive` |
| 5 | `docs/changes.md` | 每个版本的「新增 / 改动 / 修复 / 文档同步」明细 | `controlled_docs_exist_and_are_substantive` |
| 6 | `docs/architecture.md` | 分层架构、模块职责、数据流、清理策略、触发方式变化 | `controlled_docs_exist_and_are_substantive` |
| 7 | `docs/structure.md` | 增删源码文件 / 目录；改变某文件职责；分层变动 | `structure_doc_covers_every_source_module` + `structure_doc_named_files_exist` |
| 8 | `docs/api.md` | 新增 / 修改 / 删除任何 `pub` 函数或结构体 | `controlled_docs_exist_and_are_substantive` |
| 9 | `docs/testing.md` | 新增 / 删除测试、测试框架、UI 手动清单变化 | `controlled_docs_exist_and_are_substantive` |
| 10 | `docs/platform_notes.md` | **新测出来的 Win32 行为**（凭印象下结论必须先探针测过） | `controlled_docs_exist_and_are_substantive` |

> **`docs/` 只收这六份**：`architecture` / `structure` / `api` / `testing` / `changes` / `platform_notes`。
> 新增其它文档要先在本表登记，否则 `every_doc_is_registered_in_manifest` 会红。

## 二、由别的机制覆盖的文件

| 文件 | 说明 | 由谁验证 |
|---|---|---|
| `LICENSE` | AGPL-3.0 官方**逐字**全文（UTF-8、无 BOM、不含本项目署名）；版权写在 README | `license_is_verbatim_agpl3` |
| `Cargo.toml` | 版本号唯一权威来源（`[package].version`，关于页 / 资源跟随） | `smoke` 无关；发版核对 `AGENTS.md` §八 |
| `.cargo/config.toml` | 把编译产物钉到 `../temp/target`（不在源码树里） | 环境约定，见 `AGENTS.md` §一 |
| `build/build.ps1` | 构建入口；产物落 `.build/`，中间产物落 `.temp/` | 改了同步 `AGENTS.md` §一 |
| `main/` 根文件 | 白名单外的文件不得存在 | `repo_root_has_no_stray_files` |
| 全部文本文件 | 不得泄漏本机绝对路径 | `no_local_absolute_paths_leak` |

## 三、改动后的核对流程

完整交付清单见 `AGENTS.md` §八（唯一正本，本节不重复以防漂移）。本清单只负责回答
「哪些文件受门禁约束、何时必须更新」；逐项核对在每次改动收尾时执行，命中就更新，
并保证 `cargo test`（含 `tests/docs.rs`）全绿。
