//! `K` mode (quick_phrase_query.cpp:46-98). The table is really named `quick_parases`.

use std::path::Path;

use rusqlite::Connection;

use super::database::{lock, open_local_database};
use super::LocalQueryResult;
use crate::diagnostics;
use crate::types::{CandidateSource, QuickPhraseEntry, WordItem};

pub const RESULT_LIMIT: usize = 100;
/// 宿主短语表最多保留的行数，多出的按启用顺序丢弃。
pub const TABLE_LIMIT: usize = 8192;
/// 宿主短语的编码长度上限。
pub const KEY_LIMIT: usize = 32;
/// 宿主短语文本的 UTF-16 长度上限，与 Windows 候选管道的文本字段一致（`CandidateTextMaxLength`）。
pub const TEXT_UTF16_LIMIT: usize = 199;

const SQL: &str = "SELECT key,value,weight FROM quick_parases WHERE key>=?1 AND key<?2 ORDER BY weight DESC,key,value LIMIT ?3";

/// `SELECT key,value,weight FROM quick_parases WHERE key>=?1 AND key<?2 ORDER BY weight DESC,key,value LIMIT ?3`, upper bound `prefix + "\x7f"`; QuickPhrase rows.
pub fn query_quick_phrases(prefix: &str, main_db: &Path) -> LocalQueryResult {
    query_quick_phrases_with_limit(prefix, main_db, RESULT_LIMIT)
}

/// `query_quick_phrases` with the reference's explicit limit; zero answers nothing.
pub fn query_quick_phrases_with_limit(
    prefix: &str,
    main_db: &Path,
    limit: usize,
) -> LocalQueryResult {
    if prefix.is_empty() || !prefix.bytes().all(|byte| byte.is_ascii_lowercase()) || limit == 0 {
        return LocalQueryResult::default();
    }
    let Some(database) = open_local_database(main_db) else {
        return LocalQueryResult::failure(diagnostics::QUICK_PHRASE_UNAVAILABLE);
    };
    let rows = read(&lock(&database), prefix, limit);
    match rows {
        Ok(candidates) => LocalQueryResult {
            candidates,
            diagnostic: None,
        },
        // A corrupt or schema-less file is reported without SQLite's text, which could name the path or the typed code (quick_phrase_query.cpp:78-96).
        Err(_) => LocalQueryResult::failure(diagnostics::QUICK_PHRASE_QUERY_FAILED),
    }
}

fn read(connection: &Connection, prefix: &str, limit: usize) -> rusqlite::Result<Vec<WordItem>> {
    let mut statement = connection.prepare_cached(SQL)?;
    let upper_bound = prefix_upper_bound(prefix);
    let rows = statement.query_map(
        rusqlite::params![prefix, upper_bound, super::sql_limit(limit)],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        },
    )?;
    let mut candidates = Vec::with_capacity(limit);
    for row in rows {
        // A NULL key or value is skipped and a NULL weight reads as 0, as `sqlite3_column_*` did.
        if let (Some(key), Some(value), weight) = row? {
            candidates.push(WordItem::new(
                key,
                value,
                weight.unwrap_or(0),
                CandidateSource::QuickPhrase,
                "",
            ));
        }
    }
    Ok(candidates)
}

/// 宿主短语表里 Engine 能用的行：编码是 1..=`KEY_LIMIT` 个小写字母，文本非空白、不超过 `TEXT_UTF16_LIMIT` 个 UTF-16 单元、不含控制字符；按启用顺序最多保留 `TABLE_LIMIT` 行，再按编码稳定排序，同一编码内保持启用顺序。
pub fn usable_quick_phrase_table(table: &[QuickPhraseEntry]) -> Vec<QuickPhraseEntry> {
    let mut usable = Vec::with_capacity(table.len().min(TABLE_LIMIT));
    usable.extend(
        table
            .iter()
            .filter(|entry| usable_entry(entry))
            .take(TABLE_LIMIT)
            .cloned(),
    );
    usable.sort_by(|a, b| a.key.cmp(&b.key));
    usable
}

fn usable_entry(entry: &QuickPhraseEntry) -> bool {
    (1..=KEY_LIMIT).contains(&entry.key.len())
        && entry.key.bytes().all(|byte| byte.is_ascii_lowercase())
        && !entry.text.trim().is_empty()
        && entry.text.encode_utf16().count() <= TEXT_UTF16_LIMIT
        && !entry.text.chars().any(char::is_control)
}

/// K 模式的结果：先是数据库行（内置与用户自己的，顺序不变），再接上编码以 `prefix` 开头的宿主短语行；文本已经出现过的跳过，总数不超过 `RESULT_LIMIT`。数据库不可用时照样给出宿主短语行，并保留原来的诊断。`table` 必须先经过 [`usable_quick_phrase_table`]。
pub fn merge_quick_phrases(
    prefix: &str,
    database: LocalQueryResult,
    table: &[QuickPhraseEntry],
) -> LocalQueryResult {
    if table.is_empty() || prefix.is_empty() || !prefix.bytes().all(|b| b.is_ascii_lowercase()) {
        return database;
    }
    let mut result = database;
    let start = table.partition_point(|entry| entry.key.as_str() < prefix);
    for entry in table[start..]
        .iter()
        .take_while(|entry| entry.key.starts_with(prefix))
    {
        if result.candidates.len() >= RESULT_LIMIT {
            break;
        }
        if result
            .candidates
            .iter()
            .any(|candidate| candidate.word == entry.text)
        {
            continue;
        }
        result.candidates.push(WordItem::new(
            entry.key.clone(),
            entry.text.clone(),
            0,
            CandidateSource::QuickPhrase,
            "",
        ));
    }
    result
}

fn prefix_upper_bound(prefix: &str) -> String {
    let mut result = String::with_capacity(prefix.len() + 1);
    result.push_str(prefix);
    result.push('\x7f');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_upper_bound_allocates_only_result_bytes() {
        let prefix = "secret";
        let result = prefix_upper_bound(prefix);
        assert_eq!(result, "secret\x7f");
        assert_eq!(result.capacity(), result.len());
    }

    fn fixture(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("msime-pinyin.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                 INSERT INTO quick_parases VALUES('ab','highest weight',20);
                 INSERT INTO quick_parases VALUES('aa','tie b',10);
                 INSERT INTO quick_parases VALUES('aa','tie a',10);
                 INSERT INTO quick_parases VALUES('b','outside prefix',100);",
            )
            .unwrap();
        path
    }

    /// test_local_modes.cpp:190-243.
    #[test]
    fn prefix_order_limit_and_validation() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());

        let rows = query_quick_phrases_with_limit("a", &path, 10);
        assert_eq!(rows.diagnostic, None);
        let words: Vec<_> = rows
            .candidates
            .iter()
            .map(|row| (row.pinyin.as_str(), row.word.as_str()))
            .collect();
        assert_eq!(
            words,
            [("ab", "highest weight"), ("aa", "tie a"), ("aa", "tie b")]
        );
        assert_eq!(rows.candidates[0].source, CandidateSource::QuickPhrase);
        assert_eq!(rows.candidates[0].weight, 20);

        let limited = query_quick_phrases_with_limit("a", &path, 2);
        assert_eq!(limited.diagnostic, None);
        assert_eq!(limited.candidates.len(), 2);

        assert_eq!(query_quick_phrases("a", &path).candidates.len(), 3);
        for prefix in ["", "A", "a1", "a'"] {
            assert_eq!(
                query_quick_phrases(prefix, &path),
                LocalQueryResult::default(),
                "{prefix:?}"
            );
        }
        assert_eq!(
            query_quick_phrases_with_limit("a", &path, 0),
            LocalQueryResult::default()
        );
    }

    fn phrase(key: &str, text: &str) -> QuickPhraseEntry {
        QuickPhraseEntry {
            key: key.into(),
            text: text.into(),
        }
    }

    #[test]
    fn usable_tables_drop_bad_rows_and_sort_by_key_keeping_the_enabled_order() {
        let long = "字".repeat(TEXT_UTF16_LIMIT + 1);
        let table = usable_quick_phrase_table(&[
            phrase("dh", "电话"),
            phrase("", "空编码"),
            phrase("Dh", "大写"),
            phrase("d1", "数字"),
            phrase(&"a".repeat(KEY_LIMIT + 1), "太长的编码"),
            phrase("ab", "  "),
            phrase("ab", "换\n行"),
            phrase("ab", &long),
            phrase("ab", "地址"),
            phrase("dh", "电话号码"),
        ]);
        let rows: Vec<_> = table
            .iter()
            .map(|entry| (entry.key.as_str(), entry.text.as_str()))
            .collect();
        assert_eq!(rows, [("ab", "地址"), ("dh", "电话"), ("dh", "电话号码")]);

        // 多出的行按启用顺序丢弃：留下的是前 TABLE_LIMIT 行。
        let many: Vec<_> = (0..TABLE_LIMIT + 5)
            .map(|index| phrase("zz", &index.to_string()))
            .collect();
        let kept = usable_quick_phrase_table(&many);
        assert_eq!(kept.len(), TABLE_LIMIT);
        assert_eq!(kept.last().unwrap().text, (TABLE_LIMIT - 1).to_string());
    }

    #[test]
    fn usable_table_reserves_the_capped_input_capacity() {
        let table = ["aa", "ab", "ac"]
            .into_iter()
            .map(|key| phrase(key, "短语"))
            .collect::<Vec<_>>();
        let usable = usable_quick_phrase_table(&table);
        assert_eq!(usable.capacity(), table.len().min(TABLE_LIMIT));
    }

    #[test]
    fn plugin_rows_follow_the_database_rows_without_repeating_a_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let table = usable_quick_phrase_table(&[
            phrase("ac", "插件甲"),
            phrase("aa", "tie a"),
            phrase("b", "不匹配"),
            phrase("aa", "插件乙"),
        ]);
        let merged = merge_quick_phrases("a", query_quick_phrases("a", &path), &table);
        assert_eq!(merged.diagnostic, None);
        let words: Vec<_> = merged
            .candidates
            .iter()
            .map(|row| (row.pinyin.as_str(), row.word.as_str()))
            .collect();
        assert_eq!(
            words,
            [
                ("ab", "highest weight"),
                ("aa", "tie a"),
                ("aa", "tie b"),
                ("aa", "插件乙"),
                ("ac", "插件甲"),
            ]
        );
        assert!(merged
            .candidates
            .iter()
            .all(|row| row.source == CandidateSource::QuickPhrase));

        // 数据库不可用：插件行照样给出，诊断保留。
        let missing = dir.path().join("missing.db");
        let merged = merge_quick_phrases("a", query_quick_phrases("a", &missing), &table);
        assert_eq!(
            merged.diagnostic.as_deref(),
            Some(diagnostics::QUICK_PHRASE_UNAVAILABLE)
        );
        assert_eq!(merged.candidates.len(), 3);

        // 无效的前缀不给插件行。
        for prefix in ["", "A", "a1"] {
            assert!(
                merge_quick_phrases(prefix, LocalQueryResult::default(), &table)
                    .candidates
                    .is_empty()
            );
        }

        // 总数不超过 RESULT_LIMIT。
        let many: Vec<_> = (0..RESULT_LIMIT + 10)
            .map(|index| phrase("ad", &format!("行{index}")))
            .collect();
        let many = usable_quick_phrase_table(&many);
        let merged = merge_quick_phrases("a", query_quick_phrases("a", &path), &many);
        assert_eq!(merged.candidates.len(), RESULT_LIMIT);
    }

    #[test]
    fn missing_and_corrupt_databases_have_private_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let missing = query_quick_phrases("secret", &dir.path().join("private-missing.db"));
        assert!(missing.candidates.is_empty());
        assert_eq!(
            missing.diagnostic.as_deref(),
            Some(diagnostics::QUICK_PHRASE_UNAVAILABLE)
        );
        assert!(!dir.path().join("private-missing.db").exists());

        let corrupt_path = dir.path().join("private-corrupt.db");
        std::fs::write(&corrupt_path, "not a sqlite database").unwrap();
        let corrupt = query_quick_phrases("secret", &corrupt_path);
        assert!(corrupt.candidates.is_empty());
        assert_eq!(
            corrupt.diagnostic.as_deref(),
            Some(diagnostics::QUICK_PHRASE_QUERY_FAILED)
        );

        assert_eq!(
            query_quick_phrases("a", Path::new(""))
                .diagnostic
                .as_deref(),
            Some(diagnostics::QUICK_PHRASE_UNAVAILABLE)
        );
    }
}
