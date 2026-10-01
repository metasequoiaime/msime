//! `K` mode (quick_phrase_query.cpp:46-98). The table is really named `quick_parases`.

use std::path::Path;

use rusqlite::Connection;

use super::database::{lock, open_local_database};
use super::LocalQueryResult;
use crate::diagnostics;
use crate::types::{CandidateSource, WordItem};

pub const RESULT_LIMIT: usize = 100;

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
        let path = dir.join("msime.db");
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
