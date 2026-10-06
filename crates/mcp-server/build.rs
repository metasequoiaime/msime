//! 让 `--version` 和 MCP 握手报告输入法本身的版本（如 macOS 的 0.52.0），而不是这个 crate 在 workspace 里的 0.1.0。
//!
//! 来源依次是：发布脚本显式给的 `MSIME_VERSION`（`package-release.sh`、`package-container.sh`、`Build-Client.ps1` 手上的发布版本）；所编平台的 `platforms/<os>/version.txt`，与发布 workflow 打标签用的是同一个文件；都没有时（iOS、Android 等不带 `msime-mcp` 的平台，或拿不到仓库的构建）退回 crate 版本。

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=MSIME_VERSION");
    let explicit = std::env::var("MSIME_VERSION")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let version = explicit.or_else(platform_version).unwrap_or_else(|| {
        std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION")
    });
    println!("cargo:rustc-env=MSIME_APP_VERSION={version}");
}

/// 所编平台的 `version.txt`。只对带 `msime-mcp` 的三个桌面平台有意义。
fn platform_version() -> Option<String> {
    let platform = match std::env::var("CARGO_CFG_TARGET_OS").ok()?.as_str() {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let file = Path::new(&manifest)
        .join("../../platforms")
        .join(platform)
        .join("version.txt");
    let text = std::fs::read_to_string(&file).ok()?;
    // 只在文件存在时登记：登记一个不存在的文件会让 cargo 每次都重跑这个脚本。
    println!("cargo:rerun-if-changed={}", file.display());
    Some(text.trim().to_owned()).filter(|value| !value.is_empty())
}
