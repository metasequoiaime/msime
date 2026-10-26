//! `msime-pack build`：编译一个脚本插件 crate，把模块放进插件包，再按 `validate` 的规则检查。
//!
//! crate 的约定就是 `msime-pack new` 生成的样子：根目录有 `Cargo.toml`，插件包文件夹是 `pack/`，模块的文件名取 `pack/plugin.toml` 的 `script.module`。cargo 在 crate 目录里运行，`.cargo/config.toml`（栈大小、release 配置）照常生效；编出的模块路径从 cargo 的 JSON 消息里取，不猜产物目录，`CARGO_TARGET_DIR` 和配置里的 `target-dir` 都不影响它。

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use msime_client_services::plugins::{validate_with, ReservedIds};

use crate::{reason, EXIT_INVALID, EXIT_OK, EXIT_USAGE};

/// 脚本插件的编译目标。
pub const TARGET: &str = "wasm32-unknown-unknown";
/// 清单不写 `memory_pages` 时的值，同 `script_pack`。
const DEFAULT_MEMORY_PAGES: u32 = 16;

pub fn command(directory: &Path, out: &mut impl Write, err: &mut impl Write) -> i32 {
    let pack = directory.join("pack");
    if !directory.join("Cargo.toml").is_file() || !pack.join("plugin.toml").is_file() {
        let _ = writeln!(
            err,
            "error: {} 不是插件 crate：要有 Cargo.toml 和 pack/plugin.toml（msime-pack new 生成的就是这样）",
            directory.display()
        );
        return EXIT_USAGE;
    }
    let (module_name, memory_pages) = match manifest_module(&pack.join("plugin.toml")) {
        Ok(found) => found,
        Err(message) => {
            let _ = writeln!(err, "error: pack/plugin.toml: {message}");
            return EXIT_INVALID;
        }
    };
    match target_installed(directory) {
        Ok(true) => {}
        Ok(false) => {
            let _ = writeln!(
                err,
                "error: 编译用的工具链没有 {TARGET} 目标，先在 {} 里运行 rustup target add {TARGET}",
                directory.display()
            );
            return EXIT_INVALID;
        }
        Err(message) => {
            let _ = writeln!(err, "error: {message}");
            return EXIT_INVALID;
        }
    }
    let module_path = match cargo_build(directory) {
        Ok(path) => path,
        Err(message) => {
            let _ = writeln!(err, "error: {message}");
            return EXIT_INVALID;
        }
    };
    let module = match fs::read(&module_path) {
        Ok(module) => module,
        Err(error) => {
            let _ = writeln!(err, "error: 读不出 {}：{error}", module_path.display());
            return EXIT_INVALID;
        }
    };
    if let Err(error) = fs::write(pack.join(&module_name), &module) {
        let _ = writeln!(err, "error: 写不进 pack/{module_name}：{error}");
        return EXIT_INVALID;
    }
    let pages = minimum_pages(&module);
    let needed = pages.map_or_else(|| "?".to_owned(), |pages| pages.to_string());
    let _ = writeln!(
        out,
        "built pack/{module_name}: module {} bytes, memory {needed}/{memory_pages} pages",
        module.len()
    );
    if let Some(pages) = pages.filter(|pages| *pages != memory_pages) {
        let _ = writeln!(
            err,
            "note: 模块实际需要 {pages} 页内存，把 pack/plugin.toml 的 memory_pages 改成 {pages}"
        );
        if pages > memory_pages && std::env::var_os("RUSTFLAGS").is_some() {
            let _ = writeln!(
                err,
                "note: 设了环境变量 RUSTFLAGS，.cargo/config.toml 里缩小栈的 rustflags 因此没有生效"
            );
        }
    }
    let line = match validate_with(&pack, ReservedIds::Refuse) {
        Ok(summary) => format!(
            "ok {} {} {}",
            summary.id,
            summary.kind().as_str(),
            summary.version
        ),
        Err(error) => format!("error {}: {}", pack.display(), reason(&error)),
    };
    let valid = line.starts_with("ok ");
    if writeln!(out, "{line}").is_err() || !valid {
        return EXIT_INVALID;
    }
    EXIT_OK
}

/// 清单里的 `script.module` 和 `script.memory_pages`（不写是 16）。其余的字段留给后面的校验。
fn manifest_module(path: &Path) -> Result<(String, u32), String> {
    let text = fs::read_to_string(path).map_err(|error| format!("读不出：{error}"))?;
    let manifest: toml::Table = text
        .parse()
        .map_err(|error| format!("不是合法的 TOML：{error}"))?;
    let script = manifest
        .get("script")
        .and_then(toml::Value::as_table)
        .ok_or("没有 [script] 表")?;
    let module = script
        .get("module")
        .and_then(toml::Value::as_str)
        .ok_or("[script] 没有 module")?;
    // 只是一个文件名：不能带路径，免得把模块写到包外面去。
    if module.is_empty() || module.starts_with('.') || module.contains(['/', '\\']) {
        return Err(format!("module {module:?} 必须是包里的一个文件名"));
    }
    let pages = match script.get("memory_pages") {
        None => DEFAULT_MEMORY_PAGES,
        Some(value) => value
            .as_integer()
            .and_then(|pages| u32::try_from(pages).ok())
            .ok_or("memory_pages 必须是正整数")?,
    };
    Ok((module.to_owned(), pages))
}

/// 在 `directory` 里生效的那个 rustc 有没有 wasm 目标：它的 sysroot 下有没有这个目标的标准库。在 crate 目录里问，`rust-toolchain.toml` 和 rustup 的目录覆盖同样算数；不依赖 rustup，自己装的工具链也一样。
fn target_installed(directory: &Path) -> Result<bool, String> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .current_dir(directory)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("运行不了 rustc：{error}。先装 Rust：https://rustup.rs"))?;
    if !output.status.success() {
        return Err("rustc --print sysroot 失败".into());
    }
    let sysroot = String::from_utf8_lossy(&output.stdout);
    Ok(Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(TARGET)
        .is_dir())
}

/// `cargo build --release --target wasm32-unknown-unknown`，返回编出的 `.wasm`。cargo 的进度和错误照常打在标准错误上。
fn cargo_build(directory: &Path) -> Result<PathBuf, String> {
    let manifest = fs::canonicalize(directory.join("Cargo.toml"))
        .map_err(|error| format!("读不出 Cargo.toml：{error}"))?;
    // 不用 `CARGO` 环境变量：在 `cargo run`、`cargo test` 底下它指向仓库那套工具链里的 cargo，绕过 rustup 按插件目录选工具链，和上面 `target_installed` 问到的 rustc 可能不是同一套。
    let mut child = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--target",
            TARGET,
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("运行不了 cargo：{error}。先装 Rust：https://rustup.rs"))?;
    let mut module = None;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            let line = line.map_err(|error| format!("读不出 cargo 的输出：{error}"))?;
            if let Some(path) = artifact(&line, &manifest) {
                module = Some(path);
            }
        }
    }
    let status = child
        .wait()
        .map_err(|error| format!("cargo 没有正常结束：{error}"))?;
    if !status.success() {
        return Err("cargo build 失败，原因见上面 cargo 的输出".into());
    }
    module.ok_or_else(|| {
        "cargo 没有编出 .wasm：Cargo.toml 的 [lib] 要写 crate-type = [\"cdylib\"]".into()
    })
}

/// cargo 的一行 JSON 消息：是这个 crate 的 cdylib 产物时，返回其中的 `.wasm`。
fn artifact(line: &str, manifest: &Path) -> Option<PathBuf> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message.get("reason")?.as_str()? != "compiler-artifact" {
        return None;
    }
    let ours = message
        .get("manifest_path")
        .and_then(serde_json::Value::as_str)
        .and_then(|path| fs::canonicalize(path).ok())
        .is_some_and(|path| path == manifest);
    let cdylib = message
        .pointer("/target/crate_types")?
        .as_array()?
        .iter()
        .any(|kind| kind.as_str() == Some("cdylib"));
    if !ours || !cdylib {
        return None;
    }
    message
        .get("filenames")?
        .as_array()?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .find(|name| name.ends_with(".wasm"))
        .map(PathBuf::from)
}

/// 模块自己定义的那块线性内存的最小页数（memory 段，编号 5）。不是 wasm 模块或者没有定义内存时是 `None`，交给后面的校验报错。
pub fn minimum_pages(module: &[u8]) -> Option<u32> {
    let mut offset = 8;
    if module.get(..offset)? != b"\0asm\x01\0\0\0" {
        return None;
    }
    while offset < module.len() {
        let section = *module.get(offset)?;
        let (size, start) = leb128(module, offset + 1)?;
        if section == 5 {
            let (count, cursor) = leb128(module, start)?;
            if count != 1 {
                return None;
            }
            // limits 的标志字节之后就是最小页数。
            let (pages, _) = leb128(module, cursor + 1)?;
            return u32::try_from(pages).ok();
        }
        offset = start.checked_add(usize::try_from(size).ok()?)?;
    }
    None
}

/// 从 `offset` 读一个无符号 LEB128，返回值和它后面的位置。
fn leb128(data: &[u8], mut offset: usize) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *data.get(offset)?;
        offset += 1;
        value |= u64::from(byte & 0x7F) << shift;
        if byte < 0x80 {
            return Some((value, offset));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::minimum_pages;

    #[test]
    fn the_memory_minimum_is_read_from_the_memory_section() {
        // 类型段（编号 1，空），然后是 memory 段：一块内存，flags 0，最小 3 页。
        let module = b"\0asm\x01\0\0\0\x01\x01\x00\x05\x03\x01\x00\x03";
        assert_eq!(minimum_pages(module), Some(3));
        // 最小页数超过一个字节的 LEB128：130 = 0x82 0x01。
        let module = b"\0asm\x01\0\0\0\x05\x04\x01\x00\x82\x01";
        assert_eq!(minimum_pages(module), Some(130));
        assert_eq!(minimum_pages(b"\0asm\x01\0\0\0"), None);
        assert_eq!(minimum_pages(b"not wasm"), None);
        // 段长度越界时不会读到切片外面。
        assert_eq!(minimum_pages(b"\0asm\x01\0\0\0\x05\x7f\x01"), None);
    }
}
