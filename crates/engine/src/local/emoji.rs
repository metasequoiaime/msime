//! `E` and `M` modes and the mixed emoji / kaomoji rows (emoji_query.cpp:59-140, kaomoji_query.cpp). Shuangpin also queries the code normalised to quanpin.
//!
//! 九宫格混排用 `query_emoji_readings` / `query_kaomoji_readings`：数字串没有唯一的拼音，按几种可能的读法依次查。

use std::path::Path;

use rusqlite::Connection;

use super::database::{lock, open_local_database};
use super::LocalQueryResult;
use crate::diagnostics;
use crate::shuangpin::query::normalize_input;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{CandidateSource, SchemeType, WordItem};

pub const MODE_RESULT_LIMIT: usize = 10;
pub const MIXED_RESULT_LIMIT: usize = 3;

/// The one difference between the two files of the reference: the table, the source and the diagnostics.
struct Catalog {
    sql: &'static str,
    /// 按读法查（九宫格）用的语句：取回文字和命中的两列编码（emoji 只有 `key`，第二列为空），按目录顺序，不带 `LIMIT`，由调用方核对编码后取够为止。
    reading_sql: &'static str,
    source: CandidateSource,
    unavailable: &'static str,
    query_failed: &'static str,
}

const EMOJI: Catalog = Catalog {
    sql: "SELECT emoji,sort_order FROM emoji_pinyin WHERE key>=?1 AND key<?2 ORDER BY sort_order LIMIT ?3",
    reading_sql: "SELECT emoji,key,'' FROM emoji_pinyin WHERE key>=?1 AND key<?2 ORDER BY sort_order",
    source: CandidateSource::Emoji,
    unavailable: diagnostics::EMOJI_UNAVAILABLE,
    query_failed: diagnostics::EMOJI_QUERY_FAILED,
};

const KAOMOJI: Catalog = Catalog {
    sql: "SELECT kaomoji,sort_order FROM kaomoji WHERE (pinyin>=?1 AND pinyin<?2) OR (jianpin>=?1 AND jianpin<?2) ORDER BY sort_order LIMIT ?3",
    reading_sql: "SELECT kaomoji,pinyin,jianpin FROM kaomoji WHERE (pinyin>=?1 AND pinyin<?2) OR (jianpin>=?1 AND jianpin<?2) ORDER BY sort_order",
    source: CandidateSource::Kaomoji,
    unavailable: diagnostics::KAOMOJI_UNAVAILABLE,
    query_failed: diagnostics::KAOMOJI_QUERY_FAILED,
};

struct QueryPrefixes<'a> {
    lower: &'a str,
    normalized: Option<String>,
}

impl<'a> QueryPrefixes<'a> {
    fn new(lower: &'a str, scheme: SchemeType, profile: &ShuangpinProfile) -> Self {
        let normalized = if scheme == SchemeType::Shuangpin {
            let quanpin = normalize_input(lower, profile);
            (!quanpin.is_empty() && quanpin != lower).then_some(quanpin)
        } else {
            None
        };
        Self { lower, normalized }
    }

    fn len(&self) -> usize {
        1 + usize::from(self.normalized.is_some())
    }

    fn iter(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.lower).chain(self.normalized.iter().map(String::as_str))
    }
}

pub fn query_emoji(
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    limit: usize,
    profile: &ShuangpinProfile,
) -> LocalQueryResult {
    query(&EMOJI, code, scheme, others_db, limit, profile)
}

pub fn query_kaomoji(
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    limit: usize,
    profile: &ShuangpinProfile,
) -> LocalQueryResult {
    query(&KAOMOJI, code, scheme, others_db, limit, profile)
}

/// 九宫格混排：依次按 `readings`（小写全拼，可能性高的在前）查 emoji，见 `query_readings`。
pub fn query_emoji_readings(
    readings: &[String],
    others_db: &Path,
    limit: usize,
    accept: &dyn Fn(&str) -> bool,
) -> LocalQueryResult {
    query_readings(&EMOJI, readings, others_db, limit, accept)
}

/// 九宫格混排：依次按 `readings` 查颜文字，见 `query_readings`。
pub fn query_kaomoji_readings(
    readings: &[String],
    others_db: &Path,
    limit: usize,
    accept: &dyn Fn(&str) -> bool,
) -> LocalQueryResult {
    query_readings(&KAOMOJI, readings, others_db, limit, accept)
}

/// 每种读法内按目录顺序，命中的编码（emoji 的 `key`，颜文字的 `pinyin` 或 `jianpin`）以这个读法开头、并且通过 `accept`，这一行才算；跨读法按文字去重，取够 `limit` 行就不再查后面的读法。行的 `pinyin` 是命中的读法，权重与 `query` 相同从 `limit` 往下数。只有一种读法、`accept` 全收时，结果与 `query` 按这个读法查的相同。
fn query_readings(
    catalog: &Catalog,
    readings: &[String],
    others_db: &Path,
    limit: usize,
    accept: &dyn Fn(&str) -> bool,
) -> LocalQueryResult {
    if readings.is_empty() || limit == 0 {
        return LocalQueryResult::default();
    }
    let Some(database) = open_local_database(others_db) else {
        return LocalQueryResult::failure(catalog.unavailable);
    };
    let entries = match read_readings(
        &lock(&database),
        catalog.reading_sql,
        readings,
        limit,
        accept,
    ) {
        Ok(entries) => entries,
        Err(_) => return LocalQueryResult::failure(catalog.query_failed),
    };
    let count = entries.len();
    let candidates = entries
        .into_iter()
        .enumerate()
        .map(|(index, (reading, text))| {
            WordItem::new(reading, text, (count - index) as i64, catalog.source, "")
        })
        .collect();
    LocalQueryResult {
        candidates,
        diagnostic: None,
    }
}

fn read_readings<'a>(
    connection: &Connection,
    sql: &str,
    readings: &'a [String],
    limit: usize,
    accept: &dyn Fn(&str) -> bool,
) -> rusqlite::Result<Vec<(&'a str, String)>> {
    let mut statement = connection.prepare_cached(sql)?;
    let mut entries: Vec<(&str, String)> = Vec::with_capacity(limit);
    for reading in readings {
        let upper_bound = prefix_upper_bound(reading);
        let mut rows = statement.query(rusqlite::params![reading, upper_bound])?;
        while let Some(row) = rows.next()? {
            let Some(text) = row.get::<_, Option<String>>(0)? else {
                continue;
            };
            // `OR` 的另一列可能才是命中的那一列，两列各自核对。
            let mut matched = false;
            for column in 1..=2 {
                let key = row.get::<_, Option<String>>(column)?.unwrap_or_default();
                if key.starts_with(reading.as_str()) && accept(&key) {
                    matched = true;
                    break;
                }
            }
            if !matched || entries.iter().any(|(_, entry)| *entry == text) {
                continue;
            }
            entries.push((reading.as_str(), text));
            if entries.len() == limit {
                return Ok(entries);
            }
        }
    }
    Ok(entries)
}

fn query(
    catalog: &Catalog,
    code: &str,
    scheme: SchemeType,
    others_db: &Path,
    limit: usize,
    profile: &ShuangpinProfile,
) -> LocalQueryResult {
    if code.is_empty()
        || limit == 0
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
    {
        return LocalQueryResult::default();
    }
    let lower = code.to_ascii_lowercase();
    let prefixes = QueryPrefixes::new(&lower, scheme, profile);
    let Some(database) = open_local_database(others_db) else {
        return LocalQueryResult::failure(catalog.unavailable);
    };
    let entries = match read(
        &lock(&database),
        catalog.sql,
        prefixes.iter(),
        prefixes.len(),
        limit,
    ) {
        Ok(entries) => entries,
        // Reported without SQLite's text, which could name the path or the typed code (emoji_query.cpp:98-127).
        Err(_) => return LocalQueryResult::failure(catalog.query_failed),
    };
    let count = entries.len().min(limit);
    let candidates = entries
        .into_iter()
        .take(count)
        .enumerate()
        .map(|(index, (text, _))| {
            WordItem::new(
                lower.clone(),
                text,
                (count - index) as i64,
                catalog.source,
                "",
            )
        })
        .collect();
    LocalQueryResult {
        candidates,
        diagnostic: None,
    }
}

/// Every prefix's rows, the first occurrence of a text kept, then stably ordered by `sort_order` so the raw and normalised shuangpin matches interleave in catalog order.
fn read<'a, I>(
    connection: &Connection,
    sql: &str,
    prefixes: I,
    prefix_count: usize,
    limit: usize,
) -> rusqlite::Result<Vec<(String, i64)>>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut statement = connection.prepare_cached(sql)?;
    let capacity = limit.saturating_mul(prefix_count);
    let mut entries = Vec::with_capacity(capacity);
    for prefix in prefixes {
        let upper_bound = prefix_upper_bound(prefix);
        let rows = statement.query_map(
            rusqlite::params![prefix, upper_bound, super::sql_limit(limit)],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                ))
            },
        )?;
        for row in rows {
            if let (Some(text), sort_order) = row? {
                // The result page is deliberately small (normally ten rows, at most one
                // page per spelling). Scanning the collected rows avoids allocating a
                // second owned copy of every emoji/kaomoji just to deduplicate it.
                if !entries.iter().any(|(entry, _)| entry == &text) {
                    entries.push((text, sort_order.unwrap_or(0)));
                }
            }
        }
    }
    entries.sort_by_key(|(_, sort_order)| *sort_order);
    Ok(entries)
}

fn prefix_upper_bound(prefix: &str) -> String {
    let mut upper_bound = String::with_capacity(prefix.len() + 1);
    upper_bound.push_str(prefix);
    upper_bound.push('\x7f');
    upper_bound
}

#[cfg(test)]
fn contains_text(entries: &[(String, i64)], text: &str) -> bool {
    entries.iter().any(|(entry, _)| entry == text)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::types::ShuangpinProfileKind;

    const QUANPIN_ONLY: ShuangpinProfile = ShuangpinProfile {
        kind: ShuangpinProfileKind::Xiaohe,
        initials: &[],
        zero_initials: &[],
        finals: &[],
    };

    fn fixture(dir: &Path) -> PathBuf {
        let path = dir.join("msime-others.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE emoji_pinyin(key TEXT,emoji TEXT,sort_order INTEGER);
                 INSERT INTO emoji_pinyin VALUES('xiaolian','😀',10);
                 INSERT INTO emoji_pinyin VALUES('xiaolian','😄',20);
                 INSERT INTO emoji_pinyin VALUES('xiaolian','😀',30);
                 INSERT INTO emoji_pinyin VALUES('xl','😀',10);
                 INSERT INTO emoji_pinyin VALUES('xiao''lian','😄',20);
                 INSERT INTO emoji_pinyin VALUES('laugh','😀',10);
                 INSERT INTO emoji_pinyin VALUES('xnlm','raw shuangpin match',40);
                 CREATE TABLE kaomoji(pinyin TEXT,jianpin TEXT,kaomoji TEXT,sort_order INTEGER);
                 INSERT INTO kaomoji VALUES('haixiu','hx','(*/ω＼*)',10);
                 INSERT INTO kaomoji VALUES('haixiu','hx','(^_^)',20);
                 INSERT INTO kaomoji VALUES('haixiu','hx','(*/ω＼*)',30);
                 INSERT INTO kaomoji VALUES('kiss','','( ˘ ³˘)♥',40);
                 INSERT INTO kaomoji VALUES('kind','','single prefix',50);",
            )
            .unwrap();
        path
    }

    fn words(result: &LocalQueryResult) -> Vec<&str> {
        result
            .candidates
            .iter()
            .map(|row| row.word.as_str())
            .collect()
    }

    #[test]
    fn duplicate_text_lookup_scans_existing_entries() {
        let entries = vec![("😀".to_owned(), 1)];
        assert!(contains_text(&entries, "😀"));
        assert!(!contains_text(&entries, "😄"));
    }

    #[test]
    fn quanpin_prefixes_borrow_the_lowercase_input_without_allocating() {
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            QueryPrefixes::new("ni", SchemeType::Quanpin, &QUANPIN_ONLY)
        });
        assert_eq!(allocations, 0);
    }

    #[test]
    fn prefix_upper_bound_allocates_only_result_bytes() {
        let prefix = "xiao'lian";
        let upper_bound = prefix_upper_bound(prefix);
        assert_eq!(upper_bound, "xiao'lian\x7f");
        assert_eq!(upper_bound.capacity(), upper_bound.len());
    }

    /// test_local_modes.cpp:269-296, the quanpin half.
    #[test]
    fn emoji_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let emoji = query_emoji("XIAOLIAN", SchemeType::Quanpin, &path, 10, &QUANPIN_ONLY);
        assert_eq!(emoji.diagnostic, None);
        assert_eq!(words(&emoji), ["😀", "😄"]);
        assert_eq!(emoji.candidates[0].pinyin, "xiaolian");
        assert_eq!(emoji.candidates[0].source, CandidateSource::Emoji);
        assert_eq!(emoji.candidates[0].weight, 2);
        assert_eq!(emoji.candidates[1].weight, 1);

        let quanpin = SchemeType::Quanpin;
        // The local_emoji_kaomoji golden types the apostrophe, which is part of the key.
        let separated = query_emoji("xiao'lian", quanpin, &path, 10, &QUANPIN_ONLY);
        assert_eq!(words(&separated), ["😄"]);
        assert_eq!(separated.candidates[0].pinyin, "xiao'lian");
        assert_eq!(
            words(&query_emoji("xl", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "😀"
        );
        assert_eq!(
            words(&query_emoji("laugh", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "😀"
        );
        assert_eq!(
            words(&query_emoji("xiao", quanpin, &path, 10, &QUANPIN_ONLY)),
            ["😀", "😄"]
        );
        assert_eq!(
            query_emoji("xiaolian", quanpin, &path, 1, &QUANPIN_ONLY)
                .candidates
                .len(),
            1
        );
        for code in ["", "bad1", "xiao lian", "表情"] {
            assert_eq!(
                query_emoji(code, quanpin, &path, 10, &QUANPIN_ONLY),
                LocalQueryResult::default()
            );
        }
        assert_eq!(
            query_emoji("x", quanpin, &path, 0, &QUANPIN_ONLY),
            LocalQueryResult::default()
        );
    }

    /// test_local_modes.cpp:298-317, the quanpin half.
    #[test]
    fn kaomoji_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let quanpin = SchemeType::Quanpin;
        let kaomoji = query_kaomoji("HAIXIU", quanpin, &path, 10, &QUANPIN_ONLY);
        assert_eq!(kaomoji.diagnostic, None);
        assert_eq!(words(&kaomoji), ["(*/ω＼*)", "(^_^)"]);
        assert_eq!(kaomoji.candidates[0].pinyin, "haixiu");
        assert_eq!(kaomoji.candidates[0].source, CandidateSource::Kaomoji);
        assert_eq!(
            words(&query_kaomoji("hx", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "(*/ω＼*)"
        );
        assert_eq!(
            words(&query_kaomoji("kiss", quanpin, &path, 10, &QUANPIN_ONLY))[0],
            "( ˘ ³˘)♥"
        );
        assert_eq!(
            words(&query_kaomoji("k", quanpin, &path, 10, &QUANPIN_ONLY)),
            ["( ˘ ³˘)♥", "single prefix"]
        );
    }

    /// test_local_modes.cpp:285-291, 311-314: the raw code and its Xiaohe normalisation are merged in catalog order.
    #[test]
    fn shuangpin_merges_both_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let xiaohe = crate::shuangpin::profile::profile(ShuangpinProfileKind::Xiaohe);
        let emoji = query_emoji("xnlm", SchemeType::Shuangpin, &path, 10, xiaohe);
        assert_eq!(words(&emoji), ["😀", "😄", "raw shuangpin match"]);
        assert!(emoji.candidates.iter().all(|row| row.pinyin == "xnlm"));
        let kaomoji = query_kaomoji("hx", SchemeType::Shuangpin, &path, 10, xiaohe);
        assert_eq!(words(&kaomoji), ["(*/ω＼*)", "(^_^)"]);
    }

    fn readings(list: &[&str]) -> Vec<String> {
        list.iter().map(|reading| (*reading).to_owned()).collect()
    }

    /// 九宫格的读法查询：读法按给定的先后查，读法内按目录顺序；跨读法去重，取够就停；行的 `pinyin` 是命中的读法。
    #[test]
    fn reading_rows_follow_the_reading_order_then_the_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let all = |_: &str| true;
        let emoji = query_emoji_readings(&readings(&["laugh", "xiao"]), &path, 10, &all);
        assert_eq!(emoji.diagnostic, None);
        assert_eq!(words(&emoji), ["😀", "😄"]);
        assert_eq!(emoji.candidates[0].pinyin, "laugh");
        assert_eq!(emoji.candidates[1].pinyin, "xiao");
        assert_eq!(emoji.candidates[0].source, CandidateSource::Emoji);
        assert_eq!(emoji.candidates[0].weight, 2);
        assert_eq!(emoji.candidates[1].weight, 1);
        // 只有一种读法、全收时与 `query_emoji` 相同。
        assert_eq!(
            query_emoji_readings(&readings(&["xiao"]), &path, 10, &all),
            query_emoji("xiao", SchemeType::Quanpin, &path, 10, &QUANPIN_ONLY)
        );
        // 取够 `limit` 行就不再看后面的读法。
        let first = query_emoji_readings(&readings(&["xiao", "laugh"]), &path, 1, &all);
        assert_eq!(words(&first), ["😀"]);
        assert_eq!(first.candidates[0].pinyin, "xiao");
        assert_eq!(
            query_emoji_readings(&[], &path, 10, &all),
            LocalQueryResult::default()
        );
    }

    /// `accept` 核对的是命中的那一列编码；不通过的行跳过，不占名额。
    #[test]
    fn reading_rows_are_checked_against_the_matched_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let not_xiaolian = |key: &str| key != "xiaolian";
        let emoji = query_emoji_readings(&readings(&["xiao"]), &path, 1, &not_xiaolian);
        // `xiaolian` 的两行被拒，`xiao'lian` 的 😄 还在。
        assert_eq!(words(&emoji), ["😄"]);

        // 颜文字的 `jianpin` 列命中时核对 `jianpin`。
        let only_jianpin = |key: &str| key == "hx";
        let kaomoji = query_kaomoji_readings(&readings(&["hx"]), &path, 10, &only_jianpin);
        assert_eq!(words(&kaomoji), ["(*/ω＼*)", "(^_^)"]);
        assert_eq!(kaomoji.candidates[0].source, CandidateSource::Kaomoji);
        let none = |_: &str| false;
        assert!(query_kaomoji_readings(&readings(&["hx"]), &path, 10, &none)
            .candidates
            .is_empty());

        let missing = dir.path().join("private-others-missing.db");
        assert_eq!(
            query_emoji_readings(&readings(&["xiao"]), &missing, 10, &none).diagnostic,
            Some(diagnostics::EMOJI_UNAVAILABLE.to_owned())
        );
    }

    /// test_local_modes.cpp:319-331.
    #[test]
    fn missing_and_corrupt_databases_have_private_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("private-others-missing.db");
        let result = query_emoji(
            "privatecode",
            SchemeType::Quanpin,
            &missing,
            10,
            &QUANPIN_ONLY,
        );
        assert!(result.candidates.is_empty());
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::EMOJI_UNAVAILABLE)
        );
        let result = query_kaomoji(
            "privatecode",
            SchemeType::Quanpin,
            &missing,
            10,
            &QUANPIN_ONLY,
        );
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::KAOMOJI_UNAVAILABLE)
        );

        let corrupt = dir.path().join("private-corrupt.db");
        std::fs::write(&corrupt, "not a sqlite database").unwrap();
        let result = query_kaomoji(
            "privatecode",
            SchemeType::Quanpin,
            &corrupt,
            10,
            &QUANPIN_ONLY,
        );
        assert!(result.candidates.is_empty());
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::KAOMOJI_QUERY_FAILED)
        );
        let result = query_emoji(
            "privatecode",
            SchemeType::Quanpin,
            &corrupt,
            10,
            &QUANPIN_ONLY,
        );
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::EMOJI_QUERY_FAILED)
        );
    }
}
