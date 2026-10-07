//! `E` and `M` modes and the mixed emoji / kaomoji rows (emoji_query.cpp:59-140, kaomoji_query.cpp). Shuangpin also queries the code normalised to quanpin.

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
    source: CandidateSource,
    unavailable: &'static str,
    query_failed: &'static str,
}

const EMOJI: Catalog = Catalog {
    sql: "SELECT emoji,sort_order FROM emoji_pinyin WHERE key>=?1 AND key<?2 ORDER BY sort_order LIMIT ?3",
    source: CandidateSource::Emoji,
    unavailable: diagnostics::EMOJI_UNAVAILABLE,
    query_failed: diagnostics::EMOJI_QUERY_FAILED,
};

const KAOMOJI: Catalog = Catalog {
    sql: "SELECT kaomoji,sort_order FROM kaomoji WHERE (pinyin>=?1 AND pinyin<?2) OR (jianpin>=?1 AND jianpin<?2) ORDER BY sort_order LIMIT ?3",
    source: CandidateSource::Kaomoji,
    unavailable: diagnostics::KAOMOJI_UNAVAILABLE,
    query_failed: diagnostics::KAOMOJI_QUERY_FAILED,
};

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
    let mut prefixes = vec![lower.clone()];
    if scheme == SchemeType::Shuangpin {
        let quanpin = normalize_input(&lower, profile);
        if !quanpin.is_empty() && quanpin != lower {
            prefixes.push(quanpin);
        }
    }
    let Some(database) = open_local_database(others_db) else {
        return LocalQueryResult::failure(catalog.unavailable);
    };
    let entries = match read(&lock(&database), catalog.sql, &prefixes, limit) {
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
fn read(
    connection: &Connection,
    sql: &str,
    prefixes: &[String],
    limit: usize,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut statement = connection.prepare_cached(sql)?;
    let capacity = limit.saturating_mul(prefixes.len());
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
