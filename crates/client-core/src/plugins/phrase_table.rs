//! `kind = "phrase_table"`：K 模式的短语，每行一个编码和一段文本，全部写在清单里，没有数据文件。
//!
//! ```toml
//! [[phrases]]
//! key = "dh"
//! text = "电话"
//! ```
//!
//! 编码是 1 到 32 个小写 ASCII 字母，即 K 之后输入的字母；文本非空白、1 到 199 个 UTF-16 单元，不含任何控制字符（包括换行和制表符：候选只显示一行，换行会不声不响地进到应用里）。同一编码可以对应多段文本，但同一对编码和文本不能重复。一个包 1 到 2000 行。Engine 用同样的规则跳过它用不了的行（`crates/engine/src/local/quick_phrase.rs`），这里重复一遍，让包带着原因被拒绝，而不是丢了行却没人知道。

use serde::Serialize;
use std::collections::HashSet;
use toml::Value;

use super::{only_keys, PluginKind};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["phrases"];

/// 一个包的短语行数上限。
pub const MAX_PHRASES: usize = 2000;
/// 编码字母数上限。
pub const MAX_KEY_BYTES: usize = 32;
/// 文本的 UTF-16 单元上限：Windows 候选管道的文本字段，即 Engine 的 `TEXT_UTF16_LIMIT`。
pub const MAX_TEXT_UTF16: usize = 199;
/// 所有启用的短语表合起来交给 Engine 的行数上限，即 Engine 的 `local::quick_phrase::TABLE_LIMIT`。
pub const MAX_ENABLED_PHRASES: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhraseRow {
    /// K 之后输入的小写 ASCII 字母。
    pub key: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhraseTable {
    pub phrases: Vec<PhraseRow>,
}

pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<PhraseTable, String> {
    let items = table
        .get("phrases")
        .and_then(Value::as_array)
        .ok_or("短语表缺少 phrases")?;
    if items.is_empty() || items.len() > MAX_PHRASES {
        return Err("短语表的条数不在允许范围内".into());
    }
    let mut phrases = Vec::with_capacity(items.len());
    let mut seen = HashSet::with_capacity(items.len());
    for item in items {
        let row = item.as_table().ok_or("每条短语都必须是一个表")?;
        only_keys(row, &["key", "text"], "a phrase")?;
        let field = |key: &str| {
            row.get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("每条短语都需要 {key}"))
        };
        let phrase = PhraseRow {
            key: field("key")?,
            text: field("text")?,
        };
        validate(&phrase)?;
        if !seen.insert((phrase.key.clone(), phrase.text.clone())) {
            return Err(format!("短语 {} 的「{}」重复了", phrase.key, phrase.text));
        }
        phrases.push(phrase);
    }
    Ok(PhraseTable { phrases })
}

/// Engine 用不了 `phrase` 时的原因。
pub fn validate(phrase: &PhraseRow) -> Result<(), String> {
    let key = &phrase.key;
    if key.is_empty()
        || key.len() > MAX_KEY_BYTES
        || !key.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        return Err(format!("短语编码 {key} 必须是 1 到 32 个小写字母"));
    }
    let text = &phrase.text;
    if text.trim().is_empty() || !crate::text::is_bounded_utf16(text, MAX_TEXT_UTF16) {
        return Err(format!("短语 {key} 的文本为空或太长"));
    }
    if crate::text::has_disallowed_control_with_allowed(text, &[]) {
        return Err(format!("短语 {key} 的文本含有换行、制表符等控制字符"));
    }
    Ok(())
}

/// `root` 下按 `enabled` 顺序启用的短语表的全部行，最多 `MAX_ENABLED_PHRASES` 行：宿主交给 Engine 的表。缺失或载入失败的包不贡献任何行，设置页会报告它。
pub fn enabled_phrases(root: &std::path::Path, enabled: &[String]) -> Vec<PhraseRow> {
    let mut rows = Vec::new();
    for id in enabled {
        let Ok(package) = super::load_package(root, None, PluginKind::PhraseTable, id) else {
            continue;
        };
        let super::PluginContent::PhraseTable(table) = package.content else {
            continue;
        };
        for row in table.phrases {
            if rows.len() == MAX_ENABLED_PHRASES {
                return rows;
            }
            if rows.is_empty() {
                rows.reserve_exact(MAX_ENABLED_PHRASES);
            }
            rows.push(row);
        }
    }
    rows
}
