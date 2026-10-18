//! 把用户确认的新词按两版五笔单字全码取码，并与回放日志一起保存。

use rusqlite::{params, Connection, TransactionBehavior};

use super::journal::{ensure_user_database, open_dictionary_for_writing};
use super::replay::attach;
use crate::assets;
use crate::error::Result;
use crate::paths::RuntimePaths;
use crate::types::PersonalDictionaryKind;
use crate::wubi::code::phrase_code;

/// 自动生成的词不挤占已有同码候选；显式调频仍由原来的学习规则负责。
const MAX_INITIAL_WEIGHT: i64 = 9;

fn prefix(connection: &Connection, table: &str, character: char, length: usize) -> Option<String> {
    let mut statement = connection
        .prepare_cached(&format!(
            "SELECT key FROM {table} WHERE value=?1 ORDER BY length(key) DESC"
        ))
        .ok()?;
    let mut rows = statement.query([character.to_string()]).ok()?;
    let first: String = rows.next().ok()??.get(0).ok()?;
    let result = first.get(..length)?;
    if first.len() > 4 || !first.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return None;
    }
    while let Some(row) = rows.next().ok()? {
        let code: String = row.get(0).ok()?;
        if code.len() < first.len() {
            break;
        }
        if code.get(..length) != Some(result) {
            return None;
        }
    }
    Some(result.to_owned())
}

/// 只补缺失词，不覆盖已有编码、权重或删除决定。返回是否改变了工作词库。
pub(crate) fn learn(paths: &RuntimePaths, word: &str) -> Result<bool> {
    if word.chars().count() < 2 || !word.chars().all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
        return Ok(false);
    }
    let mut connection = open_dictionary_for_writing(&paths.dictionary(assets::MAIN_DICTIONARY))?;
    let mut entries = Vec::new();
    for kind in [PersonalDictionaryKind::Wubi, PersonalDictionaryKind::Wubi98] {
        let table = kind.wubi_table().expect("五笔表");
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            continue;
        }
        if let Some(Ok(code)) = phrase_code(word, |c, length| prefix(&connection, table, c, length))
        {
            entries.push((kind, table, code));
        }
    }
    if entries.is_empty() {
        return Ok(false);
    }
    let journal = paths.user(assets::USER_JOURNAL);
    ensure_user_database(&journal)?;
    attach(&connection, &journal, "wubi_phrase_journal")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut changed = false;
    for (kind, table, code) in entries {
        let existing: bool = transaction.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE value=?1)"),
            [word],
            |row| row.get(0),
        )?;
        let deleted: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM wubi_phrase_journal.user_dictionary_operations WHERE dictionary=?1 AND value=?2 AND operation='delete')",
            (kind.journal_name(), word),
            |row| row.get(0),
        )?;
        if existing || deleted {
            continue;
        }
        let minimum: Option<i64> = transaction.query_row(
            &format!("SELECT MIN(weight) FROM {table} WHERE key=?1"),
            [&code],
            |row| row.get(0),
        )?;
        let weight = minimum.map_or(MAX_INITIAL_WEIGHT, |minimum| {
            minimum.saturating_sub(1).clamp(0, MAX_INITIAL_WEIGHT)
        });
        transaction.execute(
            &format!("INSERT INTO {table}(key,value,weight) VALUES(?1,?2,?3)"),
            params![code, word, weight],
        )?;
        transaction.execute(
            "INSERT INTO wubi_phrase_journal.user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,'upsert',?4,'',1) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='upsert',weight=excluded.weight,user_inserted=1,updated_at=unixepoch()",
            params![kind.journal_name(), code, word, weight],
        )?;
        changed = true;
    }
    transaction.commit()?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLES: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
        INSERT INTO wubi86 VALUES('abcd','甲',100),('a','甲',200),('efgh','乙',100),('abef','占位',2);
        CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
        INSERT INTO wubi98 VALUES('ijkl','甲',100),('mnop','乙',100);";

    fn fixture() -> (tempfile::TempDir, RuntimePaths) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let paths = RuntimePaths {
            resources: root.clone(),
            user_data: root.clone(),
            cache: root.clone(),
            dictionaries: root,
        };
        Connection::open(paths.dictionary(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch(TABLES)
            .unwrap();
        (directory, paths)
    }

    #[test]
    fn derived_wubi_words_keep_native_rank_and_survive_replay() {
        let (_directory, paths) = fixture();
        assert!(learn(&paths, "甲乙").unwrap());
        assert!(!learn(&paths, "甲乙").unwrap());
        let db = Connection::open(paths.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT weight FROM wubi86 WHERE key='abef' AND value='甲乙'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT key FROM wubi98 WHERE value='甲乙'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "ijmn"
        );
        let fresh = paths.cache.join("fresh.db");
        Connection::open(&fresh)
            .unwrap()
            .execute_batch(TABLES)
            .unwrap();
        crate::ensure_english_schema(&paths.dictionary(assets::ENGLISH_DICTIONARY)).unwrap();
        let replay = crate::replay_user_dictionary(
            &paths.user(assets::USER_JOURNAL),
            &fresh,
            &paths.dictionary(assets::ENGLISH_DICTIONARY),
        );
        assert_eq!(replay.failed, 0, "{replay:?}");
        assert_eq!(replay.applied, 2);
        assert_eq!(
            Connection::open(fresh)
                .unwrap()
                .query_row("SELECT key FROM wubi98 WHERE value='甲乙'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            "ijmn"
        );
    }

    #[test]
    fn derived_wubi_words_preserve_existing_rows_and_deletions() {
        let (_directory, paths) = fixture();
        let db = Connection::open(paths.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        db.execute("INSERT INTO wubi86 VALUES('zzzz','甲乙',700)", [])
            .unwrap();
        super::super::journal::record_user_insert(
            &paths.user(assets::USER_JOURNAL),
            PersonalDictionaryKind::Wubi98,
            "ijmn",
            "甲乙",
            0,
            "",
        )
        .unwrap();
        Connection::open(paths.user(assets::USER_JOURNAL))
            .unwrap()
            .execute(
                "UPDATE user_dictionary_operations SET operation='delete'",
                [],
            )
            .unwrap();
        assert!(!learn(&paths, "甲乙").unwrap());
        assert_eq!(
            db.query_row("SELECT weight FROM wubi86 WHERE value='甲乙'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
            700
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM wubi98 WHERE value='甲乙'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn derived_wubi_words_skip_ambiguous_full_codes_and_missing_characters() {
        let (_directory, paths) = fixture();
        let db = Connection::open(paths.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        db.execute("INSERT INTO wubi86 VALUES('axcd','甲',100)", [])
            .unwrap();
        assert!(learn(&paths, "甲乙").unwrap());
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM wubi86 WHERE value='甲乙'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        for word in ["甲", "甲丙", "甲乙丙丁乙", "甲A乙", "甲㐀乙"] {
            assert!(!learn(&paths, word).unwrap(), "{word}");
        }
    }

    #[test]
    fn derived_wubi_words_rollback_both_profiles_if_journaling_fails() {
        let (_directory, paths) = fixture();
        ensure_user_database(&paths.user(assets::USER_JOURNAL)).unwrap();
        Connection::open(paths.user(assets::USER_JOURNAL)).unwrap().execute_batch("CREATE TRIGGER reject_second BEFORE INSERT ON user_dictionary_operations WHEN NEW.dictionary='wubi98' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(learn(&paths, "甲乙").is_err());
        let db = Connection::open(paths.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        for table in ["wubi86", "wubi98"] {
            assert_eq!(
                db.query_row(
                    &format!("SELECT count(*) FROM {table} WHERE value='甲乙'"),
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
        }
    }
}
