//! 香港官方街道、地方名称的双方案补充表。仅插入缺失条目，不借地名导入抬高已有词频。

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;

use crate::{cantonese, cantonese_curation, msime, sources::Sources, text};

pub const DIRECTORY: &str = "sources/hong-kong";
pub const TABLE: &str = "sources/hong-kong/landsd-readings.tsv";
const MANIFEST: &str = "sources/hong-kong/sources.json";
pub const LICENSE: &str = "CC-BY-4.0 AND LicenseRef-HK-CSDI";
const WEIGHT: i64 = 10;

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub traditional: String,
    pub simplified: String,
    pub cantonese: String,
    pub pinyin: String,
}

#[derive(Deserialize)]
struct Manifest {
    generated: Generated,
}

#[derive(Deserialize)]
struct Generated {
    size: u64,
    sha256: String,
}

/// 旧词库 checkout 没有这整个目录时保持兼容；目录已存在却缺表或摘要不符时拒绝发布。
pub fn load(sources: &Sources) -> Result<Vec<Entry>> {
    let Some(directory) = sources.checkout_file(DIRECTORY) else {
        return Ok(Vec::new());
    };
    match std::fs::symlink_metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        _ => {}
    }
    let manifest: Manifest = serde_json::from_str(&text::read(&sources.pinned(MANIFEST)?)?)?;
    let path = sources.pinned(TABLE)?;
    if std::fs::metadata(&path)?.len() != manifest.generated.size
        || crate::sources::sha256_file(&path)? != manifest.generated.sha256
    {
        bail!("{TABLE}: generated table does not match {MANIFEST}");
    }
    parse(&text::read(&path)?)
}

pub fn parse(source: &str) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, line) in source.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        let [traditional, simplified, cantonese, pinyin] = fields.as_slice() else {
            bail!("{TABLE}:{}: expected four columns", index + 1);
        };
        for (word, key, separator) in [
            (*traditional, *cantonese, ' '),
            (*simplified, *pinyin, '\''),
        ] {
            let syllables: Vec<_> = key.split(separator).collect();
            if word.chars().count() < 2
                || !word.chars().all(
                    |c| matches!(c as u32, 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0x20000..=0x323af),
                )
                || syllables.len() != word.chars().count()
                || syllables
                    .iter()
                    .any(|s| s.is_empty() || !s.bytes().all(|byte| byte.is_ascii_lowercase()))
            {
                bail!("{TABLE}:{}: invalid word or syllables", index + 1);
            }
        }
        if msime::pinyin_table(pinyin).is_none()
            || !seen.insert((
                traditional.to_string(),
                cantonese.to_string(),
                pinyin.to_string(),
            ))
        {
            bail!(
                "{TABLE}:{}: duplicate entry or invalid pinyin table",
                index + 1
            );
        }
        entries.push(Entry {
            traditional: traditional.to_string(),
            simplified: simplified.to_string(),
            cantonese: cantonese.to_string(),
            pinyin: pinyin.to_string(),
        });
    }
    Ok(entries)
}

/// 先取导入前每个键的最低权重，让同键新增地名之间的顺序不依赖输入行序。
fn ceilings(entries: &BTreeMap<(String, String), i64>) -> BTreeMap<&str, i64> {
    let mut result = BTreeMap::new();
    for ((key, _), weight) in entries {
        let value = result.entry(key.as_str()).or_insert(WEIGHT);
        *value = (*value).min(weight.saturating_sub(1).max(0));
    }
    result
}

pub fn add_cantonese(
    dictionary: &mut cantonese::Dictionary,
    entries: &[Entry],
    vetoes: &cantonese_curation::WordVetoes,
) -> usize {
    let limits: BTreeMap<String, i64> = ceilings(&dictionary.entries)
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();
    let mut added = 0;
    for entry in entries {
        let key = (entry.cantonese.clone(), entry.traditional.clone());
        if vetoes.rejects(&key.0, &key.1) || dictionary.entries.contains_key(&key) {
            continue;
        }
        let weight = limits.get(&entry.cantonese).copied().unwrap_or(WEIGHT);
        dictionary.entries.insert(key, weight);
        dictionary
            .syllables
            .extend(entry.cantonese.split(' ').map(str::to_owned));
        added += 1;
    }
    dictionary.words += added;
    dictionary.hong_kong_places = !entries.is_empty();
    added
}

pub fn add_pinyin(connection: &mut Connection, entries: &[Entry]) -> Result<usize> {
    let transaction = connection.transaction()?;
    let mut limits = BTreeMap::new();
    for entry in entries {
        if limits.contains_key(&entry.pinyin) {
            continue;
        }
        let table = msime::pinyin_table(&entry.pinyin).context("place without a pinyin table")?;
        let lowest: Option<i64> = transaction.query_row(
            &format!("SELECT MIN(CAST(weight AS INTEGER)) FROM {table} WHERE key = ?"),
            [&entry.pinyin],
            |row| row.get(0),
        )?;
        limits.insert(
            entry.pinyin.clone(),
            lowest.map_or(WEIGHT, |w| w.saturating_sub(1).clamp(0, WEIGHT)),
        );
    }
    let mut added = 0;
    for entry in entries {
        let table = msime::pinyin_table(&entry.pinyin).context("place without a pinyin table")?;
        let existing: Option<i64> = transaction
            .query_row(
                &format!("SELECT 1 FROM {table} WHERE key = ? AND value = ? LIMIT 1"),
                params![entry.pinyin, entry.simplified],
                |row| row.get(0),
            )
            .optional()?;
        if existing.is_some() {
            continue;
        }
        let jp: String = entry
            .pinyin
            .split('\'')
            .filter_map(|s| s.chars().next())
            .collect();
        transaction.execute(
            &format!("INSERT INTO {table} (key, jp, value, weight) VALUES (?, ?, ?, ?)"),
            params![entry.pinyin, jp, entry.simplified, limits[&entry.pinyin]],
        )?;
        added += 1;
    }
    transaction.commit()?;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        "示例路\t示例路\tsi lai lou\tshi'li'lu\n示例街\t示例街\tsi lai gaai\tshi'li'jie\n";

    #[test]
    fn old_checkout_is_supported_but_partial_or_corrupt_data_fails() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(
            root.join("upstream.lock.json"),
            r#"{"version":1,"upstreams":{},"files":[]}"#,
        )
        .unwrap();
        let lock: crate::sources::Lock = serde_json::from_str(r#"{"references":{},"mozc":{"repository":"https://example.invalid/source","commit":"0000000000000000000000000000000000000000"},"files":[]}"#).unwrap();
        let dictionary = crate::sources::Dictionary::open(root.to_path_buf(), &lock).unwrap();
        let sources = Sources {
            lock,
            repository_inputs: root.into(),
            cache: root.into(),
            offline: true,
            dictionary: Some(dictionary),
        };
        assert!(load(&sources).unwrap().is_empty());
        std::fs::create_dir_all(root.join(DIRECTORY)).unwrap();
        assert!(load(&sources).is_err());
        std::fs::write(root.join(TABLE), FIXTURE).unwrap();
        let digest = crate::sources::sha256_file(&root.join(TABLE)).unwrap();
        std::fs::write(
            root.join(MANIFEST),
            serde_json::json!({"generated":{"size":FIXTURE.len(),"sha256":digest}}).to_string(),
        )
        .unwrap();
        assert_eq!(load(&sources).unwrap().len(), 2);
        std::fs::write(root.join(TABLE), FIXTURE.replace("路", "道")).unwrap();
        assert!(load(&sources)
            .unwrap_err()
            .to_string()
            .contains("does not match"));
    }

    #[test]
    fn rejects_partial_or_ambiguous_records() {
        assert_eq!(parse(FIXTURE).unwrap().len(), 2);
        for invalid in [
            "示例路\t示例路\tsi lai\tshi'li'lu",
            "示例路\t示例路\tsi lai lou\tshi'li'",
            "示例Ａ\t示例Ａ\tsi lai a\tshi'li'a",
            "示例路\t示例路\tsi6 lai6 lou6\tshi'li'lu",
        ] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
        assert!(parse(&format!("{FIXTURE}{FIXTURE}")).is_err());
    }

    #[test]
    fn cantonese_preserves_existing_weights_and_vetoes() {
        let entries = parse(FIXTURE).unwrap();
        let mut dictionary = cantonese::Dictionary::default();
        dictionary
            .entries
            .insert(("si lai lou".into(), "示例路".into()), 2);
        dictionary
            .entries
            .insert(("si lai gaai".into(), "示例界".into()), 7);
        assert_eq!(
            add_cantonese(&mut dictionary, &entries, &Default::default()),
            1
        );
        assert_eq!(
            dictionary.entries[&("si lai lou".into(), "示例路".into())],
            2
        );
        assert_eq!(
            dictionary.entries[&("si lai gaai".into(), "示例街".into())],
            6
        );
        assert!(dictionary.syllables.contains("gaai"));
        assert_eq!(
            add_cantonese(&mut dictionary, &entries, &Default::default()),
            0
        );
        let mut empty = cantonese::Dictionary::default();
        let vetoes = cantonese_curation::WordVetoes::parse("示例街\n").unwrap();
        assert_eq!(add_cantonese(&mut empty, &entries, &vetoes), 1);
    }

    #[test]
    fn pinyin_preserves_existing_rows_and_caps_new_competitors() {
        let mut db = Connection::open_in_memory().unwrap();
        for table in msime::quanpin_tables() {
            db.execute_batch(&format!(
                "CREATE TABLE {table} (key TEXT, jp TEXT, value TEXT, weight INTEGER)"
            ))
            .unwrap();
        }
        let entries = parse(FIXTURE).unwrap();
        let table = msime::pinyin_table("shi'li'lu").unwrap();
        db.execute(
            &format!("INSERT INTO {table} VALUES (?, ?, ?, ?)"),
            params!["shi'li'lu", "sll", "示例路", 2],
        )
        .unwrap();
        db.execute(
            &format!("INSERT INTO {table} VALUES (?, ?, ?, ?)"),
            params!["shi'li'jie", "slj", "示例界", 7],
        )
        .unwrap();
        assert_eq!(add_pinyin(&mut db, &entries).unwrap(), 1);
        let weights: Vec<i64> = db
            .prepare(&format!("SELECT weight FROM {table} ORDER BY rowid"))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(weights, [2, 7, 6]);
        assert_eq!(add_pinyin(&mut db, &entries).unwrap(), 0);
    }
}
