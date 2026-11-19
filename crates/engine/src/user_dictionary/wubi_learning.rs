//! 把用户确认的新词按各版五笔单字全码取码，并与回放日志一起保存。

use rusqlite::{params, Connection, TransactionBehavior};
use std::path::Path;

use super::journal::{ensure_user_database, open_dictionary_for_writing, tombstone_sql};
use super::ranking::MANAGED_WEIGHT_CEILING;
use super::replay::attach;
use crate::assets;
use crate::error::Result;
use crate::paths::RuntimePaths;
use crate::types::{PersonalDictionaryKind, WubiPhrasePlacement, WubiProfileKind};
use crate::wubi::code::phrase_code;

/// 自动生成的词不挤占已有同码候选；显式调频仍由原来的学习规则负责。
const MAX_INITIAL_WEIGHT: i64 = 9;
/// `Top`（置顶）档在同码没有任何行时给的初始权重，与手动加词的默认同级。
const TOP_NEW_WORD_WEIGHT: i64 = 10;
/// 本模块在回放日志上用的 attach 别名；每个连接独立 attach，与 `learn` 共用同名不冲突。
const JOURNAL_ALIAS: &str = "wubi_phrase_journal";

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

/// 造词对三版都写：即使当前是 86/98 方案，也按新世纪单字全码推导并写 `wubixsj` 表和日志行，
/// 换到新世纪时以前造的词直接可用；降级兼容的取舍见仓库根目录的
/// `.agents/notes/implemented/feature/2026-11-19-wubi-phrase-learning-all-three-kinds.md`。
fn learned_kinds() -> &'static [PersonalDictionaryKind] {
    &[
        PersonalDictionaryKind::Wubi,
        PersonalDictionaryKind::Wubi98,
        PersonalDictionaryKind::WubiXsj,
    ]
}

/// 只补缺失词，不覆盖已有编码、权重或删除决定。`profile` 保留在接口里但不再决定写哪版
/// （三版都学，见 [`learned_kinds`]）；`placement` 决定初始权重档（[`MAX_INITIAL_WEIGHT`] 沉底 / [`TOP_NEW_WORD_WEIGHT`]
/// 与同码已有行最大权重 + 1 置顶）。返回是否改变了工作词库。
pub(crate) fn learn(
    paths: &RuntimePaths,
    word: &str,
    _profile: WubiProfileKind,
    placement: WubiPhrasePlacement,
) -> Result<bool> {
    if word.chars().count() < 2 || !word.chars().all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
        return Ok(false);
    }
    let mut connection = open_dictionary_for_writing(&paths.dictionary(assets::MAIN_DICTIONARY))?;
    let mut entries = Vec::new();
    for &kind in learned_kinds() {
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
        let weight = match placement {
            WubiPhrasePlacement::Bottom => {
                let minimum: Option<i64> = transaction.query_row(
                    &format!("SELECT MIN(weight) FROM {table} WHERE key=?1"),
                    [&code],
                    |row| row.get(0),
                )?;
                minimum.map_or(MAX_INITIAL_WEIGHT, |minimum| {
                    minimum.saturating_sub(1).clamp(0, MAX_INITIAL_WEIGHT)
                })
            }
            // 置顶：各表独立取同码已有行的最大权重 + 1，只管入库这一次，之后照常参与调频（不是永远浮顶）。
            WubiPhrasePlacement::Top => {
                let maximum: Option<i64> = transaction.query_row(
                    &format!("SELECT MAX(weight) FROM {table} WHERE key=?1"),
                    [&code],
                    |row| row.get(0),
                )?;
                maximum.map_or(TOP_NEW_WORD_WEIGHT, |maximum| {
                    maximum.saturating_add(1).min(MANAGED_WEIGHT_CEILING)
                })
            }
        };
        transaction.execute(
            &format!("INSERT INTO {table}(key,value,weight) VALUES(?1,?2,?3)"),
            params![code, word, weight],
        )?;
        transaction.execute(
            "INSERT INTO wubi_phrase_journal.user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) VALUES(?1,?2,?3,'upsert',?4,'',1,min(max(unixepoch(),COALESCE((SELECT reset_floor FROM wubi_phrase_journal.learning_sync_clock WHERE singleton=1),0)+1),1099511627776)) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='upsert',weight=excluded.weight,user_inserted=1,updated_at=min(max(excluded.updated_at,min(updated_at,1099511627776)+1),1099511627776)",
            params![kind.journal_name(), code, word, weight],
        )?;
        changed = true;
    }
    transaction.commit()?;
    Ok(changed)
}

/// 对称删除已学的五笔词：三版（[`learned_kinds`]）里除 `keep` 外的版本，按各自
/// 码表独立推导四码，删行并写墓碑；推导失败的版本本来也学不进，直接跳过。返回删除的行数。
/// `profile` 保留在接口里但不再决定删哪版。词库与日志在同一个事务里提交（与 [`learn`] 同一模式）。
pub(crate) fn delete_other_learned_kinds(
    main_db: &Path,
    user_db: &Path,
    keep: PersonalDictionaryKind,
    _profile: WubiProfileKind,
    word: &str,
) -> Result<usize> {
    let mut connection = open_dictionary_for_writing(main_db)?;
    ensure_user_database(user_db)?;
    attach(&connection, user_db, JOURNAL_ALIAS)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut removed = 0;
    for &kind in learned_kinds() {
        if kind == keep {
            continue;
        }
        let table = kind.wubi_table().expect("五笔表");
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            continue;
        }
        let Some(Ok(code)) = phrase_code(word, |c, length| prefix(&transaction, table, c, length))
        else {
            continue;
        };
        // 行不存在（这一版本来就没学进）不算错，墓碑照写：这个用户已经删过这个词，之后也不该再学进这一版。
        let deleted = transaction.execute(
            &format!("DELETE FROM {table} WHERE key=?1 AND value=?2"),
            params![code, word],
        )?;
        removed += deleted;
        transaction.execute(
            &tombstone_sql(JOURNAL_ALIAS),
            params![kind.journal_name(), code, word],
        )?;
    }
    transaction.commit()?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLES: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
        INSERT INTO wubi86 VALUES('abcd','甲',100),('a','甲',200),('efgh','乙',100),('abef','占位',2);
        CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
        INSERT INTO wubi98 VALUES('ijkl','甲',100),('mnop','乙',100);
        CREATE TABLE wubixsj(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
        INSERT INTO wubixsj VALUES('qrst','甲',100),('uvwx','乙',100);";

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
        assert!(learn(
            &paths,
            "甲乙",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
        assert!(!learn(
            &paths,
            "甲乙",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
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
        assert_eq!(replay.applied, 3);
        assert_eq!(
            Connection::open(&fresh)
                .unwrap()
                .query_row("SELECT key FROM wubi98 WHERE value='甲乙'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            "ijmn"
        );
        assert_eq!(
            Connection::open(&fresh)
                .unwrap()
                .query_row("SELECT key FROM wubixsj WHERE value='甲乙'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            "qruv"
        );
    }

    /// 造词对三版都学：86、98 方案也写 `wubixsj` 表和日志行，换到新世纪时以前造的词直接可用；旧词库没有新世纪表时只跳过这一版。
    #[test]
    fn the_new_century_table_is_learned_in_every_profile() {
        let count = |paths: &RuntimePaths, sql: &str| -> i64 {
            Connection::open(paths.dictionary(assets::MAIN_DICTIONARY))
                .unwrap()
                .query_row(sql, [], |r| r.get(0))
                .unwrap()
        };
        let journal = |paths: &RuntimePaths| -> i64 {
            Connection::open(paths.user(assets::USER_JOURNAL))
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubixsj'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };
        for profile in [WubiProfileKind::Wubi86, WubiProfileKind::Wubi98] {
            let (_directory, paths) = fixture();
            assert!(learn(&paths, "甲乙", profile, WubiPhrasePlacement::Bottom).unwrap());
            assert_eq!(
                count(&paths, "SELECT count(*) FROM wubixsj WHERE value='甲乙'"),
                1,
                "{profile:?}"
            );
            // 新世纪表按自己的单字全码取码（甲 qrst + 乙 uvwx → qruv），不能拿 86 的码充数。
            assert_eq!(
                Connection::open(paths.dictionary(assets::MAIN_DICTIONARY))
                    .unwrap()
                    .query_row("SELECT key FROM wubixsj WHERE value='甲乙'", [], |r| {
                        r.get::<_, String>(0)
                    })
                    .unwrap(),
                "qruv",
                "{profile:?}"
            );
            assert_eq!(
                count(&paths, "SELECT count(*) FROM wubi98 WHERE value='甲乙'"),
                1
            );
            assert_eq!(journal(&paths), 1, "{profile:?}");
        }
        let (_directory, paths) = fixture();
        assert!(learn(
            &paths,
            "甲乙",
            WubiProfileKind::WubiXsj,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
        assert_eq!(
            Connection::open(paths.dictionary(assets::MAIN_DICTIONARY))
                .unwrap()
                .query_row("SELECT key FROM wubixsj WHERE value='甲乙'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            "qruv"
        );
        assert_eq!(
            count(&paths, "SELECT count(*) FROM wubi86 WHERE value='甲乙'"),
            1
        );
        assert_eq!(journal(&paths), 1);
        // 没有新世纪表的词库（旧的发布）照样学另两版。
        let (_directory, paths) = fixture();
        Connection::open(paths.dictionary(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch("DROP TABLE wubixsj")
            .unwrap();
        assert!(learn(
            &paths,
            "甲乙",
            WubiProfileKind::WubiXsj,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
        assert_eq!(journal(&paths), 0);
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
        // 86 的已有行与 98 的墓碑都不动；新世纪表没有前置行和墓碑，照常写入。
        assert!(learn(
            &paths,
            "甲乙",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
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
        assert_eq!(
            db.query_row(
                "SELECT key FROM wubixsj WHERE value='甲乙'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "qruv"
        );
    }

    #[test]
    fn derived_wubi_words_skip_ambiguous_full_codes_and_missing_characters() {
        let (_directory, paths) = fixture();
        let db = Connection::open(paths.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        db.execute("INSERT INTO wubi86 VALUES('axcd','甲',100)", [])
            .unwrap();
        assert!(learn(
            &paths,
            "甲乙",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
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
            assert!(
                !learn(
                    &paths,
                    word,
                    WubiProfileKind::Wubi86,
                    WubiPhrasePlacement::Bottom
                )
                .unwrap(),
                "{word}"
            );
        }
    }

    #[test]
    fn derived_wubi_words_rollback_both_profiles_if_journaling_fails() {
        let (_directory, paths) = fixture();
        ensure_user_database(&paths.user(assets::USER_JOURNAL)).unwrap();
        Connection::open(paths.user(assets::USER_JOURNAL)).unwrap().execute_batch("CREATE TRIGGER reject_second BEFORE INSERT ON user_dictionary_operations WHEN NEW.dictionary='wubi98' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(learn(
            &paths,
            "甲乙",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .is_err());
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

    /// 对称删除：学词时三版同时插行，删除也要三版都删。先删当前版（显示行 + 墓碑），
    /// 再走 `delete_other_learned_kinds` 删另两版；重放零失败，`learn` 不复活。
    #[test]
    fn deleting_a_wubi_candidate_removes_all_learned_versions() {
        let (_directory, paths) = fixture();
        let main = paths.dictionary(assets::MAIN_DICTIONARY);
        let journal = paths.user(assets::USER_JOURNAL);
        assert!(learn(
            &paths,
            "甲乙",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
        // 当前版（86）先走会话同一条删除：显示行 + 墓碑。
        crate::user_dictionary::removal::delete_dictionary_candidate(
            &main,
            &journal,
            PersonalDictionaryKind::Wubi,
            "abef",
            "甲乙",
        )
        .unwrap();
        // 对称删另两版（98、新世纪）。
        assert_eq!(
            crate::user_dictionary::wubi_learning::delete_other_learned_kinds(
                &main,
                &journal,
                PersonalDictionaryKind::Wubi,
                WubiProfileKind::Wubi86,
                "甲乙",
            )
            .unwrap(),
            2
        );
        let db = Connection::open(&main).unwrap();
        for table in ["wubi86", "wubi98", "wubixsj"] {
            assert_eq!(
                db.query_row(
                    &format!("SELECT count(*) FROM {table} WHERE value='甲乙'"),
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0,
                "{table}"
            );
        }
        let log = Connection::open(&journal).unwrap();
        for (dictionary, key) in [
            ("wubi", "abef"),
            ("wubi98", "ijmn"),
            ("wubixsj", "qruv"),
        ] {
            assert_eq!(
                log.query_row(
                    "SELECT count(*) FROM user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value='甲乙' AND operation='delete'",
                    rusqlite::params![dictionary, key],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                1,
                "{dictionary}"
            );
        }
        // 删过的词再凑不复活。
        assert!(!learn(
            &paths,
            "甲乙",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
        // 重放零失败。
        let fresh = paths.cache.join("fresh.db");
        Connection::open(&fresh)
            .unwrap()
            .execute_batch(TABLES)
            .unwrap();
        crate::ensure_english_schema(&paths.dictionary(assets::ENGLISH_DICTIONARY)).unwrap();
        let replay = crate::replay_user_dictionary(
            &journal,
            &fresh,
            &paths.dictionary(assets::ENGLISH_DICTIONARY),
        );
        assert_eq!(replay.failed, 0, "{replay:?}");
    }

    /// 推导不出码的版本学不进也就删不到：无行、无墓碑，也不算失败。
    #[test]
    fn unlearnable_codes_are_skipped_without_tombstones() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let paths = RuntimePaths {
            resources: root.clone(),
            user_data: root.clone(),
            cache: root.clone(),
            dictionaries: root,
        };
        // 戊只在 86 表里有码，98 表里没有。
        Connection::open(paths.dictionary(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch(
                "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
                 INSERT INTO wubi86 VALUES('abcd','甲',100),('eeee','戊',100);
                 CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
                 INSERT INTO wubi98 VALUES('ijkl','甲',100);",
            )
            .unwrap();
        let main = paths.dictionary(assets::MAIN_DICTIONARY);
        let journal = paths.user(assets::USER_JOURNAL);
        // 甲戊：86 推得出 aeee 并学入；98 推不出，没学。
        assert!(learn(
            &paths,
            "甲戊",
            WubiProfileKind::Wubi86,
            WubiPhrasePlacement::Bottom
        )
        .unwrap());
        assert_eq!(
            Connection::open(&main)
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM wubi86 WHERE value='甲戊'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        // keep=86：另一版（98）推不出码，无行亦无墓碑，什么也不删。
        assert_eq!(
            crate::user_dictionary::wubi_learning::delete_other_learned_kinds(
                &main,
                &journal,
                PersonalDictionaryKind::Wubi,
                WubiProfileKind::Wubi86,
                "甲戊",
            )
            .unwrap(),
            0
        );
        assert_eq!(
            Connection::open(&main)
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM wubi86 WHERE value='甲戊'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            Connection::open(&journal)
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi98'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
}
