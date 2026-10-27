//! 安全门禁：本程序**永远不得**具备结束/注入/操作其它进程的能力。
//!
//! 为什么需要这道门禁而不是"我看过代码了"：本程序是 `requireAdministrator`，
//! 一旦哪天有人加进一个「直接结束别的进程」的调用，它就能要掉用户机器上任何进程的命——
//! 而用户装它只是想释放内存。所以这条约束必须是机器把守的，且改它要显式改这里。
//!
//! 判据用「拼起来的词元」而不是字面量：门禁文件本身也在扫描范围内，
//! 不需要给自己开豁免口子（这条教训来自 `no_local_absolute_paths_leak`）。

use std::path::{Path, PathBuf};

/// 禁止出现的词元：分段的字符串，运行时拼接，源码文本里不留完整字面量。
const FORBIDDEN: &[&[&str]] = &[
    // —— 结束进程 ——
    &["Terminate", "Process"],
    &["PROCESS_", "TERMINATE"],
    &["Terminate", "JobObject"],
    &["Terminate", "Thread"],
    &["ExitWindows", "Ex"],
    &["InitiateSystem", "Shutdown"],
    // —— 提权后能碰到别人的一切 ——
    &["PROCESS_", "ALL_ACCESS"],
    &["WriteProcess", "Memory"],
    &["VirtualAlloc", "Ex"],
    &["ReadProcess", "Memory"],
    &["DebugActive", "Process"],
    &["Suspend", "Thread"],
    &["SetWindowsHook", "Ex"],
    &["CreateRemote", "Thread"],
    // —— 往别的窗口发东西 ——
    &["Enum", "Windows"],
    &["GetWindowThread", "ProcessId"],
    &["AttachThread", "Input"],
    &["Send", "Input"],
    &["keybd_", "event"],
    // —— 借外部命令杀进程 ——
    &["task", "kill"],
    &["Stop-", "Process"],
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 扫一个文件，返回命中的「规则名:行号」。
fn scan(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new(); // 二进制 / 非 UTF-8 不扫
    };
    let mut hits = Vec::new();
    for (i, line) in text.lines().enumerate() {
        for parts in FORBIDDEN {
            let needle = parts.concat();
            if line.contains(&needle) {
                hits.push(format!("{}:{}  含 `{needle}`", file_name(path), i + 1));
            }
        }
    }
    hits
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_string()
}

fn walk(dir: &Path, exts: &[&str], hits: &mut Vec<String>) {
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
            walk(&p, exts, hits);
            continue;
        }
        if exts.contains(&p.extension().and_then(|x| x.to_str()).unwrap_or("")) {
            hits.extend(scan(&p));
        }
    }
}

#[test]
fn 我们不具备结束或注入其它进程的能力() {
    let root = repo_root();
    // 只扫代码与脚本，不扫文档（文档里要解释"为什么禁止"，必然提到这些名字）
    let mut hits = Vec::new();
    walk(&root, &["rs", "ps1", "bat", "toml", "manifest", "json"], &mut hits);
    assert!(
        hits.is_empty(),
        "代码/脚本里出现了越界能力，本程序只能释放内存、不能动别人的进程：\n  {}",
        hits.join("\n  ")
    );
}

/// 自检钉子：证明这把尺子会红。少了这一步，上面那条绿可能只是扫描根本没跑起来。
#[test]
fn 门禁自身会红() {
    let joined = FORBIDDEN[0].concat();
    assert!(
        scan_line(&format!("unsafe {{ {joined}(handle, 0); }}")),
        "钉子失效：{joined} 没被判为越界"
    );
    assert!(
        !scan_line("let _ = EmptyWorkingSet(handle);"),
        "误报：清工作集被判为越界"
    );
    assert!(
        !scan_line("OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION, false, pid)"),
        "误报：现有合法掩码被判为越界"
    );
    // 规则表不能空转：每条都得真的能匹配到自己
    for parts in FORBIDDEN {
        let needle = parts.concat();
        assert!(scan_line(&format!("x {needle} y")), "规则 {needle} 匹配不到自己");
    }
}

fn scan_line(line: &str) -> bool {
    FORBIDDEN.iter().any(|parts| line.contains(&parts.concat()))
}
