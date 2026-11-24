use super::*;
use crate::paths::RuntimePaths;
use crate::types::{
    FrequencyAdjustmentMode, PersonalDictionaryEntry, WubiPhrasePlacement, WubiProfileKind,
};
use crate::user_dictionary::bundled::edit_bundled_dictionary_entry;
use crate::user_dictionary::journal::{
    open_journal, record_user_insert, release_thread_journal, test_support::item,
    test_support::Dir, tombstone_sql, write_upsert,
};
use crate::user_dictionary::personal::edit_personal_dictionary;
use crate::user_dictionary::positions::{record_pinned_candidate, set_fixed_position};
use crate::user_dictionary::ranking::{adjust_candidate_ranking, RankingRequest};
use crate::user_dictionary::wubi_learning;

const FUTURE: i64 = 10_000_000_000;

fn floor(dir: &Dir, value: u64) {
    crate::advance_learning_sync_clock(&paths(dir), value).unwrap();
}

fn paths(dir: &Dir) -> RuntimePaths {
    let root = dir.root.path();
    RuntimePaths {
        resources: root.join("resources"),
        user_data: root.to_owned(),
        cache: root.join("cache"),
        dictionaries: root.to_owned(),
    }
}

fn entry(
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
    weight: i64,
) -> PersonalDictionaryEntry {
    PersonalDictionaryEntry {
        kind,
        key: key.into(),
        value: value.into(),
        weight,
    }
}

fn stamp(dir: &Dir, key: &str, value: &str) -> i64 {
    open_journal(&dir.journal())
        .unwrap()
        .query_row(
            "SELECT updated_at FROM user_dictionary_operations WHERE key=?1 AND value=?2",
            (key, value),
            |row| row.get(0),
        )
        .unwrap()
}

fn future(dir: &Dir, key: &str, value: &str, timestamp: i64) {
    open_journal(&dir.journal())
        .unwrap()
        .execute(
            "UPDATE user_dictionary_operations SET updated_at=?3 WHERE key=?1 AND value=?2",
            params![key, value, timestamp],
        )
        .unwrap();
}

#[test]
fn local_journal_changes_advance_future_times_after_reopening_and_saturate() {
    let dir = Dir::new();
    let kind = PersonalDictionaryKind::English;
    let ceiling = MAX_LEARNING_SYNC_TIMESTAMP as i64;
    record_user_insert(&dir.journal(), kind, "sample", "Sample", 1, "Sample").unwrap();
    assert!((1..=ceiling).contains(&stamp(&dir, "sample", "Sample")));
    future(&dir, "sample", "Sample", FUTURE);
    release_thread_journal();
    {
        let journal = open_journal(&dir.journal()).unwrap();
        write_upsert(&journal, kind, "sample", "Sample", 2, "Sample").unwrap();
    }
    assert_eq!(stamp(&dir, "sample", "Sample"), FUTURE + 1);
    record_user_insert(&dir.journal(), kind, "sample", "Sample", 3, "Sample").unwrap();
    assert_eq!(stamp(&dir, "sample", "Sample"), FUTURE + 2);
    {
        let journal = open_journal(&dir.journal()).unwrap();
        journal
            .execute(
                &tombstone_sql("main"),
                params!["english", "sample", "Sample"],
            )
            .unwrap();
        let (operation, inserted): (String, i64) = journal
            .query_row(
                "SELECT operation,user_inserted FROM user_dictionary_operations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((operation.as_str(), inserted), ("delete", 1));
    }
    assert_eq!(stamp(&dir, "sample", "Sample"), FUTURE + 3);
    for previous in [ceiling - 1, ceiling, i64::MAX] {
        future(&dir, "sample", "Sample", previous);
        record_user_insert(&dir.journal(), kind, "sample", "Sample", 4, "Sample").unwrap();
        assert_eq!(stamp(&dir, "sample", "Sample"), ceiling);
    }
}

#[test]
fn personal_edits_and_deletes_advance_the_same_identity_clock() {
    let dir = Dir::new();
    dir.english(&[]);
    let paths = paths(&dir);
    let initial = entry(PersonalDictionaryKind::English, "sample", "Sample", 1);
    let edited = entry(PersonalDictionaryKind::English, "sample", "Sample", 2);
    edit_personal_dictionary(&paths, None, Some(&initial), "").unwrap();
    future(&dir, "sample", "Sample", FUTURE);
    edit_personal_dictionary(&paths, Some(&initial), Some(&edited), "").unwrap();
    // 同身份编辑先删旧行再写新行，两次日志修改各推进一次。
    assert_eq!(stamp(&dir, "sample", "Sample"), FUTURE + 2);
    edit_personal_dictionary(&paths, Some(&edited), None, "").unwrap();
    assert_eq!(stamp(&dir, "sample", "Sample"), FUTURE + 3);
}

#[test]
fn bundled_reweight_and_delete_use_the_attached_journal_clock() {
    let dir = Dir::new();
    dir.english(&[("sample", "Sample", 1)]);
    let paths = paths(&dir);
    let initial = entry(PersonalDictionaryKind::English, "sample", "Sample", 1);
    let edited = entry(PersonalDictionaryKind::English, "sample", "Sample", 2);
    edit_bundled_dictionary_entry(&paths, &initial, Some(2), "").unwrap();
    future(&dir, "sample", "Sample", FUTURE);
    edit_bundled_dictionary_entry(&paths, &edited, Some(3), "").unwrap();
    assert_eq!(stamp(&dir, "sample", "Sample"), FUTURE + 1);
    let edited = entry(PersonalDictionaryKind::English, "sample", "Sample", 3);
    edit_bundled_dictionary_entry(&paths, &edited, None, "").unwrap();
    assert_eq!(stamp(&dir, "sample", "Sample"), FUTURE + 2);
}

#[test]
fn learned_wubi_phrase_advances_an_existing_journal_identity() {
    let dir = Dir::new();
    dir.wubi(&[("abcd", "甲", 100), ("efgh", "乙", 100)]);
    {
        let journal = open_journal(&dir.journal()).unwrap();
        write_upsert(
            &journal,
            PersonalDictionaryKind::Wubi,
            "abef",
            "甲乙",
            1,
            "",
        )
        .unwrap();
    }
    future(&dir, "abef", "甲乙", FUTURE);
    assert!(wubi_learning::learn(
        &paths(&dir),
        "甲乙",
        WubiProfileKind::Wubi86,
        WubiPhrasePlacement::Bottom
    )
    .unwrap());
    assert_eq!(stamp(&dir, "abef", "甲乙"), FUTURE + 1);
}

#[test]
fn pin_and_position_keep_future_times_and_the_protocol_ceiling() {
    let dir = Dir::new();
    let read = || {
        let journal = open_journal(&dir.journal()).unwrap();
        let pin: i64 = journal
            .query_row("SELECT updated_at FROM pinned_candidates", [], |row| {
                row.get(0)
            })
            .unwrap();
        let position: i64 = journal
            .query_row(
                "SELECT updated_at FROM fixed_candidate_position_state",
                [],
                |row| row.get(0),
            )
            .unwrap();
        (pin, position)
    };
    record_pinned_candidate(&dir.journal(), "synthetic", "甲").unwrap();
    set_fixed_position(&dir.journal(), "synthetic", "a", "甲", 1).unwrap();
    for previous in [FUTURE, MAX_LEARNING_SYNC_TIMESTAMP as i64] {
        open_journal(&dir.journal())
            .unwrap()
            .execute_batch(&format!(
                "UPDATE pinned_candidates SET updated_at={previous}; \
                 UPDATE fixed_candidate_position_state SET updated_at={previous};"
            ))
            .unwrap();
        release_thread_journal();
        record_pinned_candidate(&dir.journal(), "synthetic", "乙").unwrap();
        set_fixed_position(&dir.journal(), "synthetic", "a", "甲", 2).unwrap();
        let expected = (previous + 1).min(MAX_LEARNING_SYNC_TIMESTAMP as i64);
        assert_eq!(read(), (expected, expected));
    }
}

#[test]
fn ranking_staircase_and_selection_share_one_clock_and_ignore_untouched_rows() {
    for kind in [PersonalDictionaryKind::Pinyin, PersonalDictionaryKind::Wubi] {
        for previous in [FUTURE, MAX_LEARNING_SYNC_TIMESTAMP as i64] {
            let dir = Dir::new();
            let key = if kind.is_wubi() { "abcd" } else { "ni" };
            let words = ["甲", "乙", "丙", "丁", "戊"];
            let rows: Vec<_> = words.iter().map(|word| (key, *word, 5000)).collect();
            if kind.is_wubi() {
                dir.wubi(&rows);
            } else {
                dir.pinyin(&rows);
            }
            {
                let journal = open_journal(&dir.journal()).unwrap();
                for word in words {
                    write_upsert(&journal, kind, key, word, 5000, "").unwrap();
                }
            }
            future(&dir, key, "甲", MAX_LEARNING_SYNC_TIMESTAMP as i64);
            future(&dir, key, "丁", previous);
            future(&dir, key, "戊", 1);
            let ordered: Vec<_> = words.iter().map(|word| item(key, word, 5000)).collect();
            let main_db = dir.main_db();
            let user_db = dir.journal();
            assert!(adjust_candidate_ranking(&RankingRequest {
                main_db: &main_db,
                user_db: &user_db,
                context_key: key,
                ordered: &ordered,
                entry_key: key,
                value: "戊",
                mode: FrequencyAdjustmentMode::Promote,
                linear_step: 1,
                trigger_count: 1,
                force_top: false,
                kind,
            })
            .unwrap());
            let expected = (previous + 1).min(MAX_LEARNING_SYNC_TIMESTAMP as i64);
            assert_eq!(stamp(&dir, key, "丁"), expected);
            assert_eq!(stamp(&dir, key, "戊"), expected);
            assert_eq!(stamp(&dir, key, "甲"), MAX_LEARNING_SYNC_TIMESTAMP as i64);
        }
    }
}

#[test]
fn a_failed_group_write_rolls_back_dictionary_and_journal_together() {
    let dir = Dir::new();
    let words = ["甲", "乙", "丙", "丁", "戊"];
    let rows: Vec<_> = words.iter().map(|word| ("ni", *word, 5000)).collect();
    dir.pinyin(&rows);
    open_journal(&dir.journal())
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER synthetic_failure BEFORE INSERT ON user_dictionary_operations \
         WHEN NEW.value='戊' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;",
        )
        .unwrap();
    let ordered: Vec<_> = words.iter().map(|word| item("ni", word, 5000)).collect();
    assert!(adjust_candidate_ranking(&RankingRequest {
        main_db: &dir.main_db(),
        user_db: &dir.journal(),
        context_key: "ni",
        ordered: &ordered,
        entry_key: "ni",
        value: "戊",
        mode: FrequencyAdjustmentMode::Promote,
        linear_step: 1,
        trigger_count: 1,
        force_top: false,
        kind: PersonalDictionaryKind::Pinyin,
    })
    .is_err());
    assert_eq!(dir.weight("ni", "丁"), Some(5000));
    assert_eq!(dir.weight("ni", "戊"), Some(5000));
    assert_eq!(
        open_journal(&dir.journal())
            .unwrap()
            .query_row(
                "SELECT count(*) FROM user_dictionary_operations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}

#[test]
fn new_and_existing_local_rows_read_the_persistent_floor_after_reopening() {
    let dir = Dir::new();
    let kind = PersonalDictionaryKind::English;
    record_user_insert(&dir.journal(), kind, "existing", "Existing", 1, "").unwrap();
    let cached = open_journal(&dir.journal()).unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| floor(&dir, FUTURE as u64)).join().unwrap();
    });
    // 连接在另一线程推进屏障之前已经缓存，写入仍须读到新下限。
    write_upsert(&cached, kind, "existing", "Existing", 2, "").unwrap();
    write_upsert(&cached, kind, "fresh", "Fresh", 1, "").unwrap();
    assert_eq!(stamp(&dir, "existing", "Existing"), FUTURE + 1);
    assert_eq!(stamp(&dir, "fresh", "Fresh"), FUTURE + 1);
    drop(cached);
    release_thread_journal();
    record_user_insert(&dir.journal(), kind, "user", "User", 1, "").unwrap();
    assert_eq!(stamp(&dir, "user", "User"), FUTURE + 1);
    open_journal(&dir.journal())
        .unwrap()
        .execute(
            &tombstone_sql("main"),
            params!["english", "deleted", "Deleted"],
        )
        .unwrap();
    assert_eq!(stamp(&dir, "deleted", "Deleted"), FUTURE + 1);
    floor(&dir, 1);
    record_user_insert(&dir.journal(), kind, "later", "Later", 1, "").unwrap();
    assert_eq!(stamp(&dir, "later", "Later"), FUTURE + 1);
}

#[test]
fn attached_journal_edits_deletes_and_wubi_creation_exceed_the_floor() {
    let dir = Dir::new();
    dir.english(&[("bundled", "Bundled", 1)]);
    dir.wubi(&[("abcd", "甲", 100), ("efgh", "乙", 100)]);
    floor(&dir, FUTURE as u64);
    let paths = paths(&dir);
    let personal = entry(PersonalDictionaryKind::English, "user", "User", 1);
    edit_personal_dictionary(&paths, None, Some(&personal), "").unwrap();
    assert_eq!(stamp(&dir, "user", "User"), FUTURE + 1);
    edit_personal_dictionary(&paths, Some(&personal), None, "").unwrap();
    assert_eq!(stamp(&dir, "user", "User"), FUTURE + 2);
    let bundled = entry(PersonalDictionaryKind::English, "bundled", "Bundled", 1);
    edit_bundled_dictionary_entry(&paths, &bundled, Some(2), "").unwrap();
    assert_eq!(stamp(&dir, "bundled", "Bundled"), FUTURE + 1);
    let bundled = entry(PersonalDictionaryKind::English, "bundled", "Bundled", 2);
    edit_bundled_dictionary_entry(&paths, &bundled, None, "").unwrap();
    assert_eq!(stamp(&dir, "bundled", "Bundled"), FUTURE + 2);
    assert!(wubi_learning::learn(
        &paths,
        "甲乙",
        WubiProfileKind::Wubi86,
        WubiPhrasePlacement::Bottom
    )
    .unwrap());
    assert_eq!(stamp(&dir, "abef", "甲乙"), FUTURE + 1);
}

#[test]
fn new_pin_position_and_ranking_group_exceed_the_floor_and_saturate() {
    for previous in [FUTURE as u64, MAX_LEARNING_SYNC_TIMESTAMP] {
        let dir = Dir::new();
        let words = ["甲", "乙", "丙", "丁", "戊"];
        let rows: Vec<_> = words.iter().map(|word| ("ni", *word, 5000)).collect();
        dir.pinyin(&rows);
        floor(&dir, previous);
        let expected = (previous + 1).min(MAX_LEARNING_SYNC_TIMESTAMP) as i64;
        record_pinned_candidate(&dir.journal(), "synthetic", "甲").unwrap();
        set_fixed_position(&dir.journal(), "synthetic", "a", "甲", 1).unwrap();
        let journal = open_journal(&dir.journal()).unwrap();
        for table in ["pinned_candidate_state", "fixed_candidate_position_state"] {
            assert_eq!(
                journal
                    .query_row(&format!("SELECT updated_at FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                expected
            );
        }
        drop(journal);
        let ordered: Vec<_> = words.iter().map(|word| item("ni", word, 5000)).collect();
        assert!(adjust_candidate_ranking(&RankingRequest {
            main_db: &dir.main_db(),
            user_db: &dir.journal(),
            context_key: "ni",
            ordered: &ordered,
            entry_key: "ni",
            value: "戊",
            mode: FrequencyAdjustmentMode::Promote,
            linear_step: 1,
            trigger_count: 1,
            force_top: false,
            kind: PersonalDictionaryKind::Pinyin,
        })
        .unwrap());
        assert_eq!(stamp(&dir, "ni", "丁"), expected);
        assert_eq!(stamp(&dir, "ni", "戊"), expected);
    }
}
