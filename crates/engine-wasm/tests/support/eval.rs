//! 需要真实资源（`MSIME_EVAL_RESOURCES`）的测试共用的部分：读评测集、把资源目录暂存成一份工作副本、把按键串转成 `Key`。

#![allow(dead_code, reason = "parity.rs 和 rerank_routing.rs 各用其中一部分")]

use std::path::{Path, PathBuf};

use msime_engine_wasm::host::Key;

/// `resources/eval` 下的四个评测集，和 convert_eval 门禁用的一样。
pub const SETS: [&str; 4] = [
    "sentences-v1.tsv",
    "sentences-neutral-v1.tsv",
    "sentences-v2.tsv",
    "quanpin-words-v1.tsv",
];

pub struct Case {
    pub id: String,
    pub input: String,
    pub context: String,
}

/// 没有设置 `MSIME_EVAL_RESOURCES` 时返回 None，调用方打印 skipped 后直接通过。
pub fn resources() -> Option<PathBuf> {
    std::env::var_os("MSIME_EVAL_RESOURCES")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn eval_set(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resources/eval")
        .join(name)
}

/// 和 convert_eval 的 `load` 同样的规则：跳过注释、空行和表头；第 6 列是上文。
pub fn load(path: &Path) -> Vec<Case> {
    let text = std::fs::read_to_string(path).expect("eval set");
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty() && !line.starts_with("id\t"))
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert!(
                fields.len() >= 4,
                "{}: malformed row: {line}",
                path.display()
            );
            Case {
                id: fields[0].to_owned(),
                input: fields[1].to_owned(),
                context: fields.get(5).copied().unwrap_or("").to_owned(),
            }
        })
        .collect()
}

/// 把资源目录暂存成 `state/user/dictionaries/<id>` 下的工作副本（`prepare_runtime_paths`，convert_eval 经 `prepare_options` 做的同一件事），返回那个目录。资源目录本身只读，从不写入。
pub fn stage(resources: &Path, state: &Path) -> PathBuf {
    msime_engine::prepare_runtime_paths(
        resources,
        &state.join("user"),
        &state.join("cache"),
        "web-engine-eval",
    )
    .expect("staged generation")
    .dictionaries
}

/// 评测集的按键串：小写字母，以及当作音节分隔符的撇号。
pub fn typed(input: &str) -> Vec<Key> {
    input
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' => Key::Letter(byte),
            b'A'..=b'Z' => Key::ShiftLetter(byte),
            b'0'..=b'9' => Key::Digit(byte),
            b' ' => Key::Space,
            _ => Key::Punct(byte),
        })
        .collect()
}
