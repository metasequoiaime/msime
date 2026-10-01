//! `J` mode, super jianpin (jianpin_query.cpp:138-219): one letter per syllable initial, shuangpin keys mapped back to their initials.

use std::path::Path;

use rusqlite::Connection;

use super::database::{lock, open_local_database};
use super::LocalQueryResult;
use crate::diagnostics;
use crate::format;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{CandidateSource, SchemeType, WordItem};

/// 24 for a single letter, else 100.
pub fn result_limit(code: &str) -> usize {
    if code.len() == 1 {
        24
    } else {
        100
    }
}

/// Database rows with `pinyin` = the joined expanded initials and `canonical_pinyin` = the dictionary key.
pub fn query_jianpin(
    code: &str,
    scheme: SchemeType,
    main_db: &Path,
    limit: usize,
    profile: &ShuangpinProfile,
) -> LocalQueryResult {
    if limit == 0 {
        return LocalQueryResult::default();
    }
    let Some(initials) = expand_code(code, scheme, profile) else {
        return LocalQueryResult::default();
    };
    let Some(table) = format::build_table_name(&initials) else {
        return LocalQueryResult::default();
    };
    let Some(database) = open_local_database(main_db) else {
        return LocalQueryResult::failure(diagnostics::SUPER_JIANPIN_UNAVAILABLE);
    };
    // A shuangpin key can expand to zh/ch/sh, but `jp` keeps only first letters, so Xiaohe `nu` (n'sh) and `ns` (n's) both select `jp = 'ns'`. Rows are filtered by their full initials after an oversized scan (jianpin_query.cpp:122-129, 188-189).
    let filter_initials = scheme == SchemeType::Shuangpin;
    let scan_limit = if filter_initials {
        limit.saturating_mul(32).max(512)
    } else {
        limit
    };
    let rows = read(
        &lock(&database),
        &table,
        &initials,
        scan_limit,
        limit,
        filter_initials,
    );
    match rows {
        Ok(candidates) => LocalQueryResult {
            candidates,
            diagnostic: None,
        },
        // A missing table included: the reference prepared the statement and reported any failure (jianpin_query.cpp:190-195).
        Err(_) => LocalQueryResult::failure(diagnostics::SUPER_JIANPIN_QUERY_FAILED),
    }
}

/// The ranking and fixed-position context for a jianpin code: the joined expanded initials.
pub fn jianpin_ranking_context(
    code: &str,
    scheme: SchemeType,
    profile: &ShuangpinProfile,
) -> String {
    expand_code(code, scheme, profile)
        .map(|initials| initials.join("'"))
        .unwrap_or_default()
}

/// One initial per letter, uppercase folded; `None` for an empty code or any non-letter.
fn expand_code(code: &str, scheme: SchemeType, profile: &ShuangpinProfile) -> Option<Vec<String>> {
    if code.is_empty() || !code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    Some(
        code.bytes()
            .map(|byte| {
                let key = byte.to_ascii_lowercase();
                if scheme == SchemeType::Shuangpin {
                    if let Some((initial, _)) = profile
                        .initials
                        .iter()
                        .find(|(_, mapped)| mapped.as_bytes() == [key])
                    {
                        return (*initial).to_owned();
                    }
                }
                char::from(key).to_string()
            })
            .collect(),
    )
}

fn read(
    connection: &Connection,
    table: &str,
    initials: &[String],
    scan_limit: usize,
    limit: usize,
    filter_initials: bool,
) -> rusqlite::Result<Vec<WordItem>> {
    let sql = jianpin_sql(table);
    let jianpin: String = initials
        .iter()
        .filter_map(|initial| initial.get(..1))
        .collect();
    let matched_code = initials.join("'");
    let mut statement = connection.prepare_cached(&sql)?;
    let mut rows = statement.query(rusqlite::params![jianpin, super::sql_limit(scan_limit)])?;
    let mut candidates = Vec::with_capacity(limit);
    while let Some(row) = rows.next()? {
        let Some(value) = row.get::<_, Option<String>>(1)? else {
            continue;
        };
        let canonical = row.get::<_, Option<String>>(0)?.unwrap_or_default();
        if filter_initials && !key_matches_initials(&canonical, initials) {
            continue;
        }
        candidates.push(WordItem::new(
            matched_code.clone(),
            value,
            row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            CandidateSource::Database,
            canonical,
        ));
        if candidates.len() >= limit {
            break;
        }
    }
    Ok(candidates)
}

fn jianpin_sql(table: &str) -> String {
    let mut sql = String::with_capacity(table.len() + 83);
    sql.push_str("SELECT \"key\",\"value\",\"weight\" FROM \"");
    sql.push_str(table);
    sql.push_str("\" WHERE \"jp\"=?1 ORDER BY \"weight\" DESC LIMIT ?2");
    sql
}

fn key_matches_initials(key: &str, initials: &[String]) -> bool {
    let mut syllables = key.split('\'');
    initials.iter().all(|initial| {
        syllables
            .next()
            .is_some_and(|syllable| syllable_initial(syllable) == initial)
    }) && syllables.next().is_none()
}

fn syllable_initial(syllable: &str) -> &str {
    match syllable.get(..2) {
        Some(prefix @ ("zh" | "ch" | "sh")) => prefix,
        _ => syllable.get(..1).unwrap_or(""),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::types::ShuangpinProfileKind;

    const XIAOHE: ShuangpinProfile = ShuangpinProfile {
        kind: ShuangpinProfileKind::Xiaohe,
        initials: &[("sh", "u"), ("ch", "i"), ("zh", "v")],
        zero_initials: &[],
        finals: &[],
    };
    const ZIRANMA: ShuangpinProfile = ShuangpinProfile {
        kind: ShuangpinProfileKind::Ziranma,
        initials: &[("sh", "u"), ("ch", "i"), ("zh", "v")],
        zero_initials: &[],
        finals: &[],
    };
    const SHOUDAO: ShuangpinProfile = ShuangpinProfile {
        kind: ShuangpinProfileKind::Shoudao,
        initials: &[("sh", "e"), ("ch", "i"), ("zh", "v")],
        zero_initials: &[],
        finals: &[],
    };
    const MICROSOFT: ShuangpinProfile = ShuangpinProfile {
        kind: ShuangpinProfileKind::Microsoft,
        initials: &[("sh", "u"), ("ch", "i"), ("zh", "v")],
        zero_initials: &[],
        finals: &[],
    };

    #[test]
    fn jianpin_sql_writes_the_lookup_statement_directly() {
        let sql = jianpin_sql("tbl_2_n");
        assert_eq!(
            sql,
            "SELECT \"key\",\"value\",\"weight\" FROM \"tbl_2_n\" WHERE \"jp\"=?1 ORDER BY \"weight\" DESC LIMIT ?2"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    fn fixture(dir: &Path) -> PathBuf {
        let path = dir.join("msime.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "BEGIN;
                 CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_1_n VALUES('ni','n','你',300);
                 INSERT INTO tbl_1_n VALUES('na','n','拿',200);
                 CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',300);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','拟好',200);
                 INSERT INTO tbl_2_n VALUES('na''han','nh','呐喊',100);
                 INSERT INTO tbl_2_n VALUES('ni''shuo','ns','你说',290);
                 INSERT INTO tbl_2_n VALUES('ni''si','ns','你思',280);
                 INSERT INTO tbl_2_n VALUES('ni''u','nu','你屋',100);
                 CREATE TABLE tbl_2_a(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_a VALUES('ai''ni','an','爱你',250);
                 CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_z VALUES('zhi''chi','zc','知耻',240);
                 COMMIT;",
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

    /// test_jianpin_input_session.cpp:125-143.
    #[test]
    fn quanpin_initials() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());

        let quanpin = query_jianpin("nh", SchemeType::Quanpin, &path, 50, &XIAOHE);
        assert_eq!(quanpin.diagnostic, None);
        assert_eq!(words(&quanpin), ["你好", "拟好", "呐喊"]);
        for row in &quanpin.candidates {
            assert_eq!(row.source, CandidateSource::Database);
            assert_eq!(row.pinyin, "n'h");
            assert!(!row.canonical_pinyin.is_empty());
        }
        assert_eq!(quanpin.candidates[0].canonical_pinyin, "ni'hao");
        assert_eq!(quanpin.candidates[0].weight, 300);

        let uppercase = query_jianpin("NH", SchemeType::Quanpin, &path, 1, &XIAOHE);
        assert_eq!(words(&uppercase), ["你好"]);

        let two_initials = query_jianpin("an", SchemeType::Quanpin, &path, 50, &XIAOHE);
        assert_eq!(words(&two_initials), ["爱你"]);

        assert_eq!(
            words(&query_jianpin("n", SchemeType::Quanpin, &path, 24, &XIAOHE)),
            ["你", "拿"]
        );
        for code in ["n'", "", "n1", "你"] {
            assert_eq!(
                query_jianpin(code, SchemeType::Quanpin, &path, 50, &XIAOHE),
                LocalQueryResult::default()
            );
        }
        assert_eq!(
            query_jianpin("nh", SchemeType::Quanpin, &path, 0, &XIAOHE),
            LocalQueryResult::default()
        );
    }

    /// test_jianpin_input_session.cpp:145-170.
    #[test]
    fn shuangpin_initials() {
        let cases = [
            ("nh", SchemeType::Quanpin, &XIAOHE, "n'h"),
            ("nu", SchemeType::Shuangpin, &XIAOHE, "n'sh"),
            ("ns", SchemeType::Shuangpin, &XIAOHE, "n's"),
            ("vi", SchemeType::Shuangpin, &XIAOHE, "zh'ch"),
            ("ne", SchemeType::Shuangpin, &SHOUDAO, "n'sh"),
            ("nu", SchemeType::Shuangpin, &SHOUDAO, "n'u"),
            ("nu", SchemeType::Shuangpin, &ZIRANMA, "n'sh"),
            ("nu", SchemeType::Shuangpin, &MICROSOFT, "n'sh"),
            ("NU", SchemeType::Shuangpin, &XIAOHE, "n'sh"),
            ("nu", SchemeType::Quanpin, &XIAOHE, "n'u"),
            ("n'", SchemeType::Quanpin, &XIAOHE, ""),
        ];
        for (code, scheme, profile, expected) in cases {
            assert_eq!(
                jianpin_ranking_context(code, scheme, profile),
                expected,
                "{code} {}",
                profile.kind.name()
            );
        }

        let dir = tempfile::tempdir().unwrap();
        let path = fixture(dir.path());
        let xiaohe_nu = query_jianpin("nu", SchemeType::Shuangpin, &path, 50, &XIAOHE);
        assert_eq!(words(&xiaohe_nu), ["你说"]);
        assert_eq!(xiaohe_nu.candidates[0].pinyin, "n'sh");
        assert_eq!(xiaohe_nu.candidates[0].canonical_pinyin, "ni'shuo");
        assert_eq!(
            words(&query_jianpin(
                "ns",
                SchemeType::Shuangpin,
                &path,
                50,
                &XIAOHE
            )),
            ["你思"]
        );
        assert_eq!(
            words(&query_jianpin(
                "ne",
                SchemeType::Shuangpin,
                &path,
                50,
                &SHOUDAO
            )),
            ["你说"]
        );
        assert_eq!(
            words(&query_jianpin(
                "nu",
                SchemeType::Shuangpin,
                &path,
                50,
                &SHOUDAO
            )),
            ["你屋"]
        );
        assert_eq!(
            words(&query_jianpin(
                "vi",
                SchemeType::Shuangpin,
                &path,
                50,
                &XIAOHE
            )),
            ["知耻"]
        );
        assert_eq!(
            query_jianpin("ns", SchemeType::Shuangpin, &path, 1, &XIAOHE)
                .candidates
                .len(),
            1
        );
    }

    #[test]
    fn missing_corrupt_and_tableless_databases() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing").join("msime.db");
        let result = query_jianpin("nh", SchemeType::Quanpin, &missing, 50, &XIAOHE);
        assert!(result.candidates.is_empty());
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::SUPER_JIANPIN_UNAVAILABLE)
        );
        assert!(!missing.exists());

        let corrupt = dir.path().join("corrupt.db");
        std::fs::write(&corrupt, "not a sqlite database").unwrap();
        let result = query_jianpin("nh", SchemeType::Quanpin, &corrupt, 50, &XIAOHE);
        assert!(result.candidates.is_empty());
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::SUPER_JIANPIN_QUERY_FAILED)
        );

        let path = fixture(dir.path());
        let result = query_jianpin("xy", SchemeType::Quanpin, &path, 50, &XIAOHE);
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::SUPER_JIANPIN_QUERY_FAILED)
        );
    }

    #[test]
    fn initials_of_keys() {
        assert_eq!(syllable_initial("zhi"), "zh");
        assert_eq!(syllable_initial("shuo"), "sh");
        assert_eq!(syllable_initial("si"), "s");
        assert_eq!(syllable_initial("a"), "a");
        assert_eq!(syllable_initial(""), "");
        assert!(key_matches_initials("zhi'chi", &["zh".into(), "ch".into()]));
        assert!(!key_matches_initials("zhi'chi", &["z".into(), "c".into()]));
        assert!(!key_matches_initials("ni", &["n".into(), "h".into()]));
    }

    #[test]
    fn limits() {
        assert_eq!(result_limit("n"), 24);
        assert_eq!(result_limit("nh"), 100);
    }
}
