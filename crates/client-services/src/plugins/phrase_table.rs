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
use std::path::Path;
use toml::Value;

use super::{only_keys, PluginKind};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["phrases"];

/// 一个包的短语行数上限。
pub const MAX_PHRASES: usize = 2000;
/// 编码字母数上限。
pub const MAX_KEY_BYTES: usize = 32;
/// 文本的 UTF-16 单元上限：Windows 候选管道的文本字段，即 Engine 的 `TEXT_UTF16_LIMIT`。
pub const MAX_TEXT_UTF16: usize = 199;
/// 普通候选中可配置的最大名次。
pub const MAX_CANDIDATE_POSITION: u8 = 100;
const MAX_POSITIONED_TABLE_BYTES: u64 = 256 * 1024;
/// 所有启用的短语表合起来交给 Engine 的行数上限，即 Engine 的 `local::quick_phrase::TABLE_LIMIT`。
pub const MAX_ENABLED_PHRASES: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhraseRow {
    /// K 之后输入的小写 ASCII 字母。
    pub key: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u8>,
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
        only_keys(row, &["key", "text", "position"], "a phrase")?;
        let field = |key: &str| {
            row.get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("每条短语都需要 {key}"))
        };
        let phrase = PhraseRow {
            key: field("key")?,
            text: field("text")?,
            position: row
                .get("position")
                .map(|position| {
                    position
                        .as_integer()
                        .and_then(|value| u8::try_from(value).ok())
                        .filter(|value| (1..=MAX_CANDIDATE_POSITION).contains(value))
                        .ok_or("短语候选位置须为 1 到 100 的整数")
                })
                .transpose()?,
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

/// 将选中的 UTF-8 文件装成短语表。每行按「编码 文字 位置」读取，文字可含空格。
pub fn import_positioned_table(
    root: &Path,
    source: &Path,
) -> Result<super::PluginSummary, super::PluginError> {
    use super::authoring::{self, AuthoredKind, AuthoredPack, AuthoredPhrase};
    use super::PluginError;

    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(PluginError::UnsupportedSource)?;
    let parent = source.parent().ok_or(PluginError::UnsupportedSource)?;
    let bytes = super::read_file(parent, file_name, MAX_POSITIONED_TABLE_BYTES)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| PluginError::Invalid("码表须为 UTF-8 文本".into()))?;
    let mut phrases = Vec::new();
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let invalid = |reason: &str| PluginError::Invalid(format!("第 {} 行：{reason}", index + 1));
        let (key, rest) = line
            .split_once(char::is_whitespace)
            .ok_or_else(|| invalid("请按「编码 文字 位置」填写"))?;
        let (value, position) = rest
            .trim_start()
            .rsplit_once(char::is_whitespace)
            .ok_or_else(|| invalid("请按「编码 文字 位置」填写"))?;
        let position = position
            .parse::<u8>()
            .ok()
            .filter(|value| (1..=MAX_CANDIDATE_POSITION).contains(value))
            .ok_or_else(|| invalid("候选位置须为 1 到 100 的整数"))?;
        let row = PhraseRow {
            key: key.into(),
            text: value.trim_end().into(),
            position: Some(position),
        };
        validate(&row).map_err(|reason| invalid(&reason))?;
        phrases.push(AuthoredPhrase {
            key: row.key,
            text: row.text,
            position: row.position,
        });
        if phrases.len() > MAX_PHRASES {
            return Err(invalid("一个短语表最多 2000 条"));
        }
    }
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let mut name: String = stem
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    name = name.trim().to_owned();
    let mut end = name.len().min(80);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name.truncate(end);
    if name.is_empty() {
        name = "自定义编码".into();
    }
    authoring::save(
        root,
        &AuthoredPack {
            kind: AuthoredKind::PhraseTable,
            id: None,
            name,
            version: "1.0.0".into(),
            license: "NOASSERTION".into(),
            author: None,
            description: None,
            phrases: Some(phrases),
            commands: None,
            groups: None,
            words: None,
        },
        false,
    )
}

/// 按 `enabled` 顺序启用的短语表的全部行，最多 `MAX_ENABLED_PHRASES` 行：宿主交给 Engine 的表。已安装的包在 `root` 下，官方短语表（`super::BUILTIN_PHRASE_TABLES`）在内置目录 `builtin_packs` 下。缺失或载入失败的包不贡献任何行，设置页会报告它。
pub fn enabled_phrases(
    root: &std::path::Path,
    builtin_packs: Option<&std::path::Path>,
    enabled: &[String],
) -> Vec<PhraseRow> {
    let mut rows = Vec::new();
    for id in enabled {
        let Ok(package) = super::load_package(
            root,
            super::BuiltinDirs::packs(builtin_packs),
            PluginKind::PhraseTable,
            id,
        ) else {
            continue;
        };
        let super::PluginContent::PhraseTable(table) = package.content else {
            continue;
        };
        let count = table.phrases.len().min(MAX_ENABLED_PHRASES - rows.len());
        if count == 0 {
            return rows;
        }
        if rows.capacity() - rows.len() < count {
            let required = rows.len() + count;
            let target = rows
                .capacity()
                .saturating_mul(2)
                .max(required)
                .min(MAX_ENABLED_PHRASES);
            rows.reserve_exact(target - rows.len());
        }
        for row in table.phrases.into_iter().take(count) {
            rows.push(row);
        }
    }
    rows
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::plugins::{authoring, PluginContent, PluginError};
    use std::fs;

    #[test]
    fn macos_positioned_text_becomes_an_editable_phrase_table() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("plugins");
        let source = directory.path().join("合成.txt");
        fs::write(
            &source,
            "\u{feff}# 合成\r\nka か 7\r\nka\t카\t8\r\nka 混合 文本 6\r\n",
        )
        .unwrap();
        let summary = import_positioned_table(&root, &source).unwrap();
        let PluginContent::PhraseTable(table) = &summary.content else {
            panic!("应导入短语表");
        };
        assert_eq!(summary.name, "合成");
        assert_eq!(
            table
                .phrases
                .iter()
                .map(|row| (row.text.as_str(), row.position))
                .collect::<Vec<_>>(),
            [("か", Some(7)), ("카", Some(8)), ("混合 文本", Some(6))]
        );

        let mut edit = authoring::load(&root, "phrase_table", &summary.id).unwrap();
        edit.phrases.as_mut().unwrap()[0].position = None;
        edit.name = "改名".into();
        authoring::save(&root, &edit, true).unwrap();
        let saved = authoring::load(&root, "phrase_table", &summary.id).unwrap();
        assert_eq!(saved.name, "改名");
        assert_eq!(saved.phrases.unwrap()[0].position, Some(7));
    }

    #[test]
    fn bad_position_rejects_the_whole_text_import() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("合成.txt");
        let root = directory.path().join("plugins");
        fs::write(&source, "ka か 7\nka 카 101\n").unwrap();
        assert!(matches!(import_positioned_table(&root, &source),
            Err(PluginError::Invalid(reason)) if reason.contains("第 2 行")));
        assert!(!root.exists());
    }
}
