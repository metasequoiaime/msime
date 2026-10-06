//! 网页引擎的 NOTICE 必须列出每个链接进 wasm 的 crate。
//!
//! 依赖图来自 `cargo tree -p msime-engine-wasm --target wasm32-unknown-unknown -e normal,no-proc-macro`，和 `scripts/web-engine-notice.sh` 生成 NOTICE 时用的是同一条命令；新增或升级一个依赖而没有重新生成 NOTICE，这里就会失败。没有安装 wasm32-unknown-unknown 目标的机器上打印 skipped 后通过。
#![cfg(not(target_family = "wasm"))]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET: &str = "wasm32-unknown-unknown";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn wasm_target_installed() -> bool {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let Ok(output) = Command::new(rustc).args(["--print", "sysroot"]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let sysroot = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Path::new(&sysroot)
        .join("lib/rustlib")
        .join(TARGET)
        .is_dir()
}

/// `cargo tree` 的 `{p}` 是 `name vX.Y.Z`，后面可能跟来源和 `(*)`；返回去重后的 (name, version)。
fn linked_crates(root: &Path) -> BTreeSet<(String, String)> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(root)
        .args([
            "tree",
            "--locked",
            "-p",
            "msime-engine-wasm",
            "--target",
            TARGET,
            "-e",
            "normal,no-proc-macro",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()
        .expect("run cargo tree");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let name = fields.next()?;
            let version = fields.next()?.strip_prefix('v')?;
            Some((name.to_owned(), version.to_owned()))
        })
        .collect()
}

#[test]
fn notice_lists_every_linked_crate() {
    if !wasm_target_installed() {
        println!("skipped: the {TARGET} target is not installed");
        return;
    }
    let root = repo_root();
    let notice = std::fs::read_to_string(root.join("resources/licenses/web-engine-NOTICE.md"))
        .expect("read web-engine-NOTICE.md");
    let crates = linked_crates(&root);
    assert!(
        crates.iter().any(|(name, _)| name == "msime-engine"),
        "cargo tree did not list msime-engine: {crates:?}"
    );
    let missing: Vec<String> = crates
        .iter()
        .filter(|(name, version)| !notice.contains(&format!("| `{name}` | {version} |")))
        .map(|(name, version)| format!("{name} {version}"))
        .collect();
    assert!(
        missing.is_empty(),
        "resources/licenses/web-engine-NOTICE.md does not list these linked crates; run scripts/web-engine-notice.sh: {missing:?}"
    );
}
