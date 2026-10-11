//! 作者 SDK（`docs/script-plugin-template/sdk/` 下的 C 头文件和 Rust 绑定）与运行时的 ABI 逐项一致：版本、返回码、钩子、字段编号、枚举取值、日志级别和 import 白名单。SDK 是给作者复制走的独立文件，不能直接引用运行时的常量，所以靠这里守着。

#![cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use msime_script_runtime::abi::{
    self, candidate_field, commit_field, composition_field, event_field, log_level,
    scalar_values as values, status,
};
use msime_script_runtime::Hook;

fn sdk(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/script-plugin-template/sdk")
            .join(name),
    )
    .unwrap()
}

/// 每个常量：C 里的宏名、Rust 绑定里的路径、值。
fn expected() -> Vec<(String, String, i64)> {
    let mut all = vec![(
        "MSIME_ABI_VERSION".to_owned(),
        "ABI_VERSION".to_owned(),
        i64::from(abi::ABI_VERSION),
    )];
    let mut group = |c_prefix: &str, rust_module: &str, entries: &[(&str, i64)]| {
        for (name, value) in entries {
            all.push((
                format!("{c_prefix}{name}"),
                format!("{rust_module}::{name}"),
                *value,
            ));
        }
    };
    group(
        "MSIME_",
        "status",
        &[
            ("DENIED", status::DENIED.into()),
            ("ABSENT", status::ABSENT.into()),
            ("INVALID", status::INVALID.into()),
            ("LIMIT", status::LIMIT.into()),
        ],
    );
    let hooks: Vec<(String, i64)> = Hook::ALL
        .iter()
        .map(|hook| (hook.as_str().to_uppercase(), hook.code().into()))
        .collect();
    let hooks: Vec<(&str, i64)> = hooks
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    group("MSIME_HOOK_", "hook", &hooks);
    group(
        "MSIME_EVENT_",
        "event",
        &[
            ("KIND", event_field::KIND.into()),
            ("ORIGIN", event_field::ORIGIN.into()),
            ("HOW", event_field::HOW.into()),
            ("OUTCOME", event_field::OUTCOME.into()),
            ("REASON", event_field::REASON.into()),
            ("LOCAL_MODE", event_field::LOCAL_MODE.into()),
            ("INPUT_CLASS", event_field::INPUT_CLASS.into()),
            ("RETRACTION", event_field::RETRACTION.into()),
            ("REWRITABLE", event_field::REWRITABLE.into()),
            ("CONTINUES", event_field::CONTINUES.into()),
            ("GENERATED", event_field::GENERATED.into()),
        ],
    );
    group(
        "MSIME_COMPOSITION_",
        "composition",
        &[
            ("PREEDIT", composition_field::PREEDIT.into()),
            ("READING", composition_field::READING.into()),
            ("INPUT_CHAR", composition_field::INPUT_CHAR.into()),
            ("CARET", composition_field::CARET.into()),
        ],
    );
    group(
        "MSIME_COMMIT_",
        "commit",
        &[
            ("SELECTED", commit_field::SELECTED.into()),
            ("COMMIT", commit_field::COMMIT.into()),
            ("COMMIT_BODY", commit_field::COMMIT_BODY.into()),
            ("SEAT", commit_field::SEAT.into()),
        ],
    );
    group(
        "MSIME_CANDIDATE_",
        "candidates",
        &[
            ("SOURCE", candidate_field::SOURCE.into()),
            ("TEXT_LEN", candidate_field::TEXT_LEN.into()),
            ("CODE_LEN", candidate_field::CODE_LEN.into()),
            ("ANNOTATION_LEN", candidate_field::ANNOTATION_LEN.into()),
            ("PAGE", candidate_field::PAGE.into()),
            ("HIGHLIGHTED", candidate_field::HIGHLIGHTED.into()),
        ],
    );
    group(
        "MSIME_LOG_",
        "log",
        &[
            ("DEBUG", log_level::DEBUG.into()),
            ("INFO", log_level::INFO.into()),
            ("WARN", log_level::WARN.into()),
            ("ERROR", log_level::ERROR.into()),
        ],
    );
    for (c_prefix, rust_module, table) in [
        ("MSIME_ORIGIN_", "origin", values::ORIGIN),
        ("MSIME_HOW_", "how", values::HOW),
        ("MSIME_OUTCOME_", "outcome", values::OUTCOME),
        ("MSIME_REASON_", "reason", values::REASON),
        ("MSIME_LOCAL_MODE_", "local_mode", values::LOCAL_MODE),
        ("MSIME_INPUT_CLASS_", "input_class", values::INPUT_CLASS),
        ("MSIME_RETRACTION_", "retraction", values::RETRACTION),
    ] {
        let entries: Vec<(String, i64)> = table
            .iter()
            .map(|(name, value)| (name.to_uppercase(), i64::from(*value)))
            .collect();
        let entries: Vec<(&str, i64)> = entries
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();
        group(c_prefix, rust_module, &entries);
    }
    all
}

/// 头文件里的整数宏：`#define MSIME_X 1` 或 `#define MSIME_X (-1)`。
fn c_constants(header: &str) -> BTreeMap<String, i64> {
    header
        .lines()
        .filter_map(|line| {
            let mut parts = line.strip_prefix("#define ")?.split_whitespace();
            let name = parts.next()?;
            let value = parts.next()?.trim_start_matches('(').trim_end_matches(')');
            Some((name.to_owned(), value.parse().ok()?))
        })
        .collect()
}

/// Rust 绑定里的常量，按所在的 `pub mod` 加上路径：`pub const NAME: T = VALUE;`。
fn rust_constants(binding: &str) -> BTreeMap<String, i64> {
    let mut module: Option<&str> = None;
    let mut constants = BTreeMap::new();
    for line in binding.lines() {
        if let Some(name) = line
            .strip_prefix("pub mod ")
            .and_then(|rest| rest.strip_suffix(" {"))
        {
            module = Some(name);
        } else if line == "}" {
            module = None;
        }
        let Some(rest) = line.trim_start().strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(':') else {
            continue;
        };
        let Some(value) = rest
            .split_once('=')
            .and_then(|(_, value)| value.trim().strip_suffix(';'))
            .and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let path = match module {
            Some(module) => format!("{module}::{name}"),
            None => name.to_owned(),
        };
        constants.insert(path, value);
    }
    constants
}

#[test]
fn the_sdk_constants_are_the_runtime_abi() {
    let expected = expected();
    let c: BTreeMap<String, i64> = expected
        .iter()
        .map(|(name, _, value)| (name.clone(), *value))
        .collect();
    let rust: BTreeMap<String, i64> = expected
        .iter()
        .map(|(_, path, value)| (path.clone(), *value))
        .collect();
    // 两边都不多不少：SDK 里有 ABI 没有的常量，同样是漂移。
    assert_eq!(c_constants(&sdk("msime_script.h")), c);
    assert_eq!(rust_constants(&sdk("msime_script.rs")), rust);
}

#[test]
fn the_sdk_declares_exactly_the_import_whitelist() {
    let whitelist: Vec<&str> = abi::imports().map(|(name, _)| name).collect();
    let header = sdk("msime_script.h");
    let declared: Vec<&str> = header
        .lines()
        .filter_map(|line| line.strip_prefix("MSIME_IMPORT(")?.split_once(')'))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(declared, whitelist);

    let binding = sdk("msime_script.rs");
    let start = binding.find("unsafe extern \"C\" {").unwrap();
    let block = &binding[start..start + binding[start..].find("\n    }").unwrap()];
    let declared: Vec<&str> = block
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("pub fn ")?.split_once('('))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(declared, whitelist);
}
