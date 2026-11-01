use std::{env, fs, path::Path, process::Command};

fn git(root: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "MSIME_BUILD_REVISION",
        "MSIME_BUILD_RUN",
        "GITHUB_RUN_ID",
        "GITHUB_RUN_ATTEMPT",
        "MSIME_TEST_TELEMETRY_ORIGIN",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    // 源码包由打包脚本显式传身份；只在确实属于本源码树时读取 Git，不能误取外层打包仓库的提交。
    let own_git = git(&root, &["rev-parse", "--show-toplevel"])
        .and_then(|path| std::fs::canonicalize(path).ok())
        == std::fs::canonicalize(&root).ok();
    if own_git {
        // 提交身份只取决于 HEAD 指向和分支引用；reflog 在 reset、同提交 checkout 等不换提交的操作上也会追加，不能拿来触发重建。
        for name in ["HEAD", "packed-refs"] {
            watch_git_path(&root, name);
        }
        if let Some(branch) = git(&root, &["symbolic-ref", "-q", "HEAD"]) {
            watch_git_path(&root, &branch);
        }
    }
    let revision = env::var("MSIME_BUILD_REVISION")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            own_git
                .then(|| git(&root, &["rev-parse", "HEAD"]))
                .flatten()
        })
        .or_else(|| {
            let file = root.join("platforms/linux/build-revision.txt");
            let revision = fs::read_to_string(&file).ok()?;
            println!("cargo:rerun-if-changed={}", file.display());
            Some(revision.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let run = env::var("MSIME_BUILD_RUN")
        .or_else(|_| env::var("GITHUB_RUN_ID"))
        .unwrap_or_default();
    let attempt = env::var("GITHUB_RUN_ATTEMPT").unwrap_or_default();
    let architecture = env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo sets target architecture");
    for (name, value) in [
        ("REVISION", revision),
        ("RUN", run),
        ("ATTEMPT", attempt),
        ("ARCH", architecture),
    ] {
        assert!(
            value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte)),
            "invalid crash build identity: {name}"
        );
        println!("cargo:rustc-env=MSIME_CRASH_{name}={value}");
    }
}

fn watch_git_path(root: &Path, name: &str) {
    if let Some(path) = git(root, &["rev-parse", "--git-path", name]) {
        let path = root.join(path);
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
