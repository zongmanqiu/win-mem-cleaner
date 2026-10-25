//! 文档一致性门禁（学 CurSeen 的 `tests/docs_consistency.rs`）。
//!
//! 为什么要有这一文件：最常见的文档腐化是「代码挪了、说明还留在原处」，靠自觉核对不可靠。
//! 这里让 `cargo test` 替我们记住——而且只查**事实**（文件在不在、提到了没有、指向的文件
//! 真不真存在），不查文笔，所以误报率低、值得常驻。
//!
//! 覆盖六条可判定事实：
//! 1. 受控文档都存在且非空、有实质内容（空文档比没文档更糟，会在核对时给出「已检查」的错觉）；
//! 2. `docs/` 下每个 `.md` 都在 `DOCS_MANIFEST.md` 登记（防未受控文档溜进仓库）；
//! 3. 《docs/structure.md》点名了 `src/` 下每一个顶层模块，且它点名的每个 `.rs` 真实存在
//!    （「列全」+「列准」两半——删文件必须同步删文档条目）；
//! 4. `main/` 根目录没有白名单外的游离文件（一次性草稿 / 调试残留不进仓库）；
//! 5. `LICENSE` 是逐字的 GNU AGPL-3.0 全文、UTF-8、无 BOM，且不含本项目署名；
//! 6. 仓库任何文本文件不得泄漏本机绝对路径（换台电脑就失效，且泄露开发者目录）。

use std::path::{Path, PathBuf};

/// 仓库根 = `main/`（本 crate 的目录）。所有判据都相对它，克隆到任何机器都成立。
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(root: &Path, rel: &str) -> String {
    let p = root.join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("读 {rel} 失败：{e}"))
}

/// ① 受控文档存在、非空、有实质内容。
#[test]
fn controlled_docs_exist_and_are_substantive() {
    let root = repo_root();
    let listed = [
        ("README.md", "项目总说明"),
        ("AGENTS.md", "工程规则全集"),
        ("DOCS_MANIFEST.md", "文档清单"),
        ("history.md", "版本要点"),
        ("docs/architecture.md", "架构总览"),
        ("docs/structure.md", "目录结构说明"),
        ("docs/api.md", "API 参考"),
        ("docs/testing.md", "测试指南"),
        ("docs/platform_notes.md", "Win32 实测库"),
        ("docs/changes.md", "更新日志"),
    ];
    for (rel, why) in listed {
        let p = root.join(rel);
        assert!(p.is_file(), "{why}缺失：{rel}");
        let body = read(&root, rel);
        assert!(!body.trim().is_empty(), "{why}为空：{rel}");
        assert!(
            body.lines().count() > 10,
            "{why}内容过短（{} 行）：{rel}——空/占位文档会在核对时给出「已检查」的错觉",
            body.lines().count()
        );
    }
}

/// ② `docs/` 下每个 `.md` 必须在 `DOCS_MANIFEST.md` 登记——单一来源，防「随手一丢」。
#[test]
fn every_doc_is_registered_in_manifest() {
    let root = repo_root();
    let manifest = read(&root, "DOCS_MANIFEST.md");
    let docs = root.join("docs");
    let mut seen = 0usize;
    let mut unregistered: Vec<String> = Vec::new();
    for e in std::fs::read_dir(&docs)
        .unwrap_or_else(|_| panic!("读 docs/ 失败"))
        .flatten()
    {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        seen += 1;
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if !manifest.contains(name) {
            unregistered.push(format!("docs/{name}"));
        }
    }
    // 自证：docs/ 至少 6 份文档；扫不到说明目录结构变了，本门禁失去保护意义，必须同步改。
    assert!(
        seen >= 6,
        "docs/ 下只扫到 {seen} 份 .md（预期 >= 6）——目录结构变了，请同步修订本门禁基线"
    );
    assert!(
        unregistered.is_empty(),
        "docs/ 下出现未登记文档：{unregistered:?}——要么在 DOCS_MANIFEST.md 登记，要么移出 docs/"
    );
}

/// 递归收集 src 树里所有 `.rs` 的文件名（不含路径），供「列准」核对。
fn rs_basenames(dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_basenames(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                out.push(n.to_string());
            }
        }
    }
}

/// ③a 结构说明必须点名 `src/` 下每一个顶层模块，以及入口文件。
#[test]
fn structure_doc_covers_every_source_module() {
    let root = repo_root();
    let doc = read(&root, "docs/structure.md");
    let src = root.join("src");

    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&src)
        .expect("读 src/ 失败")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "src/ 下应有子目录");
    let mut missing: Vec<String> = Vec::new();
    for d in &dirs {
        let name = d.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !doc.contains(name) {
            missing.push(format!("src/{name}/"));
        }
    }
    assert!(
        missing.is_empty(),
        "以下 src 顶层模块未出现在 docs/structure.md：{missing:?}——新增模块请登记职责"
    );
    for f in ["main.rs", "lib.rs"] {
        assert!(doc.contains(f), "docs/structure.md 未提到入口 {f}");
    }
}

/// ③b 结构说明点名的每个 `.rs` 必须真实存在（删文件要同步删文档条目）。
#[test]
fn structure_doc_named_files_exist() {
    let root = repo_root();
    let doc = read(&root, "docs/structure.md");
    let mut present: Vec<String> = Vec::new();
    // 结构说明覆盖整个仓库树：源码、测试、构建脚本都要算「存在」。
    rs_basenames(&root.join("src"), &mut present);
    rs_basenames(&root.join("tests"), &mut present);
    present.push("build.rs".to_string());

    let mut stale: Vec<String> = Vec::new();
    for token in doc.split(['`', ' ', '|', '(', ')']) {
        let t = token.trim();
        let Some(name) = t.strip_suffix(".rs") else {
            continue;
        };
        // 只认「纯 ASCII 标识符.rs」——中文说明里紧跟的 build.rs 等会被这种切法误伤，
        // 用 is_ascii_alphanumeric 挡掉非文件名 token。
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            continue;
        }
        let fname = format!("{name}.rs");
        if !present.iter().any(|p| p == &fname) {
            stale.push(fname);
        }
    }
    assert!(
        stale.is_empty(),
        "docs/structure.md 列出了 src 树中不存在的源码文件：{stale:?}\n\
         ——删文件必须同步删掉文档那一行（别写成「已删除」，现行文档不写考古叙述）"
    );
}

/// ④ `main/` 根目录不得有白名单外的文件（源码/文档/清单之外的一次性件不进仓库）。
#[test]
fn repo_root_has_no_stray_files() {
    let root = repo_root();
    let allowed = [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "app.manifest",
        "LICENSE",
        ".gitignore",
        "README.md",
        "AGENTS.md",
        "DOCS_MANIFEST.md",
        "history.md",
    ];
    let mut stray: Vec<String> = Vec::new();
    for e in std::fs::read_dir(&root).expect("读 main/ 失败").flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if name == ".git" || !allowed.contains(&name) {
            // 目录（src/tests/docs/build/image/.cargo/.git）不在本判据范围，只看文件。
            if p.extension().is_none() && !name.contains('.') {
                continue;
            }
            stray.push(name.to_string());
        }
    }
    assert!(
        stray.is_empty(),
        "main/ 根出现白名单外的文件：{stray:?}\n\
         ——一次性草稿 / 调试残留 / 实测转储请放项目根 .temp/（不入库），别丢进源码树"
    );
}

/// ⑤ `LICENSE` 必须是逐字的 AGPL-3.0 全文（UTF-8、无 BOM、不含本项目署名）。
///
/// 用官方原文里最长的这段当指纹，任何删改 / 转录错误都会现形。
const AGPL3_FINGERPRINT: &str = "you convey, or propagate by procuring conveyance of";

#[test]
fn license_is_verbatim_agpl3() {
    let root = repo_root();
    let raw = std::fs::read(root.join("LICENSE")).expect("读 LICENSE 失败");
    assert!(
        !raw.starts_with(&[0xEF, 0xBB, 0xBF]),
        "LICENSE 带 UTF-8 BOM——许可文本应保持无 BOM 的纯文本"
    );
    // read_to_string 只接受合法 UTF-8：UTF-16（PowerShell 重定向默认产物）会在这里炸。
    let body = std::fs::read_to_string(root.join("LICENSE")).expect("LICENSE 不是合法 UTF-8");
    assert!(
        body.starts_with("                    GNU AFFERO GENERAL PUBLIC LICENSE\n"),
        "LICENSE 首行不是官方标题行——许可文本必须逐字，开头不加版权块或任何说明"
    );
    let flat: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "Version 3, 19 November 2007",
        "END OF TERMS AND CONDITIONS",
        AGPL3_FINGERPRINT,
    ] {
        assert!(
            flat.contains(needle),
            "LICENSE 缺少官方 AGPL-3.0 锚点 {needle:?}——许可被改写或不是 v3 全文"
        );
    }
    // 版权与署名写在 README / Cargo.toml 的 authors，不进 LICENSE（否则无法与官方逐字比对）。
    for forbidden in ["WinMemCleaner", "qiuzongman", "邱宗满"] {
        assert!(
            !body.contains(forbidden),
            "LICENSE 里出现了 {forbidden:?}——本项目署名应在 README，不在许可文本"
        );
    }
}

/// ⑥ 仓库文本文件不得泄漏本机绝对路径（换台电脑即失效，且泄露开发者目录）。
/// 占位示例（`C:\App`、`D:\素材`）不算；这里钉的是真实工作目录 / 真实用户名。
#[test]
fn no_local_absolute_paths_leak() {
    let root = repo_root();
    const TEXT_EXT: &[&str] = &["rs", "md", "toml", "ps1", "manifest", "txt", "json", "bat", "lock"];
    const NEEDLES: &[&str] = &["AI.Agent", "AI\\Agent", "Users\\user", "Users/user"];

    fn walk(dir: &Path, hits: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if matches!(name, ".git" | "target" | ".temp" | ".build" | "node_modules") {
                    continue;
                }
                walk(&p, hits);
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if !TEXT_EXT.contains(&ext) {
                continue;
            }
            // 本文件自身定义了上面的 NEEDLES，必然含这些串——豁免检测器本身，
            // 否则门禁一开机就扫到自己（CurSeen 同款处理）。
            if p.file_name().and_then(|n| n.to_str()) == Some("docs.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue; // 二进制 / 非 UTF-8 不扫
            };
            for (i, line) in text.lines().enumerate() {
                for n in NEEDLES {
                    if line.contains(n) {
                        hits.push(format!("{}:{}  含 {n:?}", file_name(&p), i + 1));
                    }
                }
            }
        }
    }
    fn file_name(p: &Path) -> String {
        p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string()
    }

    let mut hits = Vec::new();
    walk(&root, &mut hits);
    assert!(
        hits.is_empty(),
        "仓库里泄漏了本机绝对路径：\n  {}\n\
         ——路径一律用相对或 env!(\"CARGO_MANIFEST_DIR\") 派生，别把开发机目录写进仓库",
        hits.join("\n  ")
    );
}
