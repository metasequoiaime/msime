//! `msime-pack pack`：把插件包文件夹打成确定的 `.zip`，再按 `validate` 的规则检查这个压缩包。
//!
//! 和 `plugins/script/tools/assemble.py` 给仓库里的插件打的包是同样的性质：条目按名字排序、放在顶层、时间固定为 1980-01-01、权限 0644、不压缩（deflate 的输出随实现和版本变化，插件包只有几百 KiB 以内，不值得为此失去逐字节可复现），同样的文件得到逐字节相同的压缩包。字节不和 assemble.py 的相同：两边用的 zip 实现不同，头部字段各有各的写法。

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use msime_client_services::plugins::{validate_with, ReservedIds};

use crate::{reason, EXIT_INVALID, EXIT_OK};

/// `source` 是插件包文件夹（里面直接有 `plugin.toml`），或者 `msime-pack new` 生成的 crate（包在它的 `pack/` 里）。`output` 不写时是包文件夹旁边的 `<id>-<version>.zip`。
pub fn command(
    source: &Path,
    output: Option<&Path>,
    reserved: ReservedIds,
    out: &mut impl Write,
    err: &mut impl Write,
) -> i32 {
    let folder = if source.join("plugin.toml").is_file() {
        source.to_path_buf()
    } else {
        source.join("pack")
    };
    let summary = match validate_with(&folder, reserved) {
        Ok(summary) => summary,
        Err(error) => {
            let _ = writeln!(out, "error {}: {}", folder.display(), reason(&error));
            return EXIT_INVALID;
        }
    };
    let output = match output {
        Some(output) => output.to_path_buf(),
        None => folder
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("{}-{}.zip", summary.id, summary.version)),
    };
    // 放进包文件夹里的压缩包，下一次打包时会被当成包里的文件。
    if inside(&output, &folder) {
        let _ = writeln!(
            err,
            "error: 压缩包不能放在插件包文件夹 {} 里",
            folder.display()
        );
        return EXIT_INVALID;
    }
    let archive = match write_archive(&folder) {
        Ok(archive) => archive,
        Err(message) => {
            let _ = writeln!(err, "error: {message}");
            return EXIT_INVALID;
        }
    };
    if let Err(error) = fs::write(&output, &archive) {
        let _ = writeln!(err, "error: 写不进 {}：{error}", output.display());
        return EXIT_INVALID;
    }
    let line = match validate_with(&output, reserved) {
        Ok(summary) => format!(
            "ok {} {} {}",
            summary.id,
            summary.kind().as_str(),
            summary.version
        ),
        Err(error) => format!("error {}: {}", output.display(), reason(&error)),
    };
    let valid = line.starts_with("ok ");
    let _ = writeln!(out, "packed {}: {} bytes", output.display(), archive.len());
    if writeln!(out, "{line}").is_err() || !valid {
        return EXIT_INVALID;
    }
    EXIT_OK
}

/// `path` 是否落在 `folder` 里（按规范化之后的路径比较；`path` 还不存在时看它的上级目录）。
fn inside(path: &Path, folder: &Path) -> bool {
    let Ok(folder) = fs::canonicalize(folder) else {
        return false;
    };
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::canonicalize(parent).is_ok_and(|parent| parent.starts_with(&folder))
}

/// 包文件夹里的文件（隐藏文件不算，和导入相同；`validate` 已经确认没有子文件夹和符号链接），按名字排序打成不压缩的 zip。
fn write_archive(folder: &Path) -> Result<Vec<u8>, String> {
    let unreadable = |error: std::io::Error| format!("读不出 {}：{error}", folder.display());
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(folder).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let Ok(name) = entry.file_name().into_string() else {
            return Err("有文件名不是 UTF-8 编码".into());
        };
        if !name.starts_with('.') {
            files.push((name, entry.path()));
        }
    }
    files.sort();
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, path) in files {
        let bytes = fs::read(&path).map_err(|error| format!("读不出 {name}：{error}"))?;
        writer
            .start_file(name.as_str(), options)
            .map_err(|error| format!("写压缩包失败：{error}"))?;
        writer
            .write_all(&bytes)
            .map_err(|error| format!("写压缩包失败：{error}"))?;
    }
    writer
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| format!("写压缩包失败：{error}"))
}
