//! 完整偏好文档命令；不扩展 MCP 工具对助手开放的字段集合。

use crate::{bounded, config::Config};
use msime_client_core::preferences::PreferencesStore;
use serde_json::{json, Value};
use std::path::Path;
use std::process::ExitCode;

const FILE_LIMIT: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Operation {
    Revision,
    Validate {
        file: String,
    },
    Apply {
        file: String,
        expected_revision: u64,
    },
}

/// 生成键表无需安装输入法、运行时选项或用户目录。
pub fn run_schema() -> ExitCode {
    println!(
        "{}",
        serde_json::to_string_pretty(&msime_client_core::preferences::schema::json_schema())
            .expect("schema is JSON")
    );
    ExitCode::SUCCESS
}

/// 此命令在进入 MCP 服务器之前执行；所有错误只报告类别，不回显偏好值或文件内容。
pub fn run(config: &Config, operation: Operation) -> ExitCode {
    match execute(config, operation) {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("result is JSON")
            );
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{}: {message}", crate::config::program());
            ExitCode::FAILURE
        }
    }
}

fn execute(config: &Config, operation: Operation) -> Result<Value, &'static str> {
    if matches!(operation, Operation::Apply { .. }) && !config.allow_write {
        return Err("config apply needs --allow-write outside a terminal");
    }
    // 显式状态目录支持未安装设置应用的 dotfiles 场景；版本仍由该目录的记录决定。
    let store = if let Some(directory) = &config.state_dir {
        PreferencesStore::new(directory)
    } else {
        let options = config
            .read_options()
            .map_err(|_| "cannot read runtime options; use --state-dir")?;
        PreferencesStore::for_edition(
            config
                .state_dir(&options)
                .map_err(|_| "cannot resolve preferences directory")?,
            config
                .edition(&options)
                .map_err(|_| "unknown input method edition")?,
        )
    };
    match operation {
        Operation::Revision => store
            .document_revision()
            .map(|revision| json!({ "revision": revision }))
            .map_err(|_| "cannot read preferences document"),
        Operation::Validate { file } => {
            let patch = read_file(&file)?;
            store
                .preview_patch(&patch)
                .map(|snapshot| json!({ "valid": true, "revision": snapshot.revision }))
                .map_err(|_| "invalid preferences patch or existing document")
        }
        Operation::Apply {
            file,
            expected_revision,
        } => {
            let patch = read_file(&file)?;
            store
                .apply_patch(expected_revision, &patch)
                .map(|snapshot| json!({ "applied": true, "revision": snapshot.revision }))
                .map_err(|error| match error {
                    msime_client_core::preferences::PreferencesError::Conflict => {
                        "preferences revision conflict; validate again before applying"
                    }
                    _ => "invalid preferences patch or unwritable document",
                })
        }
    }
}

fn read_file(path: &str) -> Result<Value, &'static str> {
    let file =
        bounded::open_private(Path::new(path)).map_err(|_| "cannot open preferences patch")?;
    let bytes = bounded::read(file, FILE_LIMIT)
        .map_err(|_| "cannot read preferences patch (limit 1 MiB)")?;
    serde_json::from_slice(&bytes).map_err(|_| "preferences patch must be JSON")
}
