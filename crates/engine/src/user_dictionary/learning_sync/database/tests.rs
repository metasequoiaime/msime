use super::*;
use crate::user_dictionary::journal::test_support::{count, Dir};
use crate::user_dictionary::journal::{open_journal, release_thread_journal};
use crate::user_dictionary::learning_sync::{read_learning_sync, write_learning_sync};
use std::io::Cursor;

fn paths(dir: &Dir) -> RuntimePaths {
    let root = dir.root.path();
    RuntimePaths {
        resources: root.join("resources"),
        user_data: root.to_owned(),
        cache: root.join("cache"),
        dictionaries: root.to_owned(),
    }
}

fn fixture() -> Dir {
    let dir = Dir::new();
    dir.pinyin(&[("jia", "甲", 1), ("yi", "乙", 2)]);
    dir.english(&[("sample", "Sample", 1)]);
    open_journal(&dir.journal()).unwrap();
    dir
}

fn state(records: impl IntoIterator<Item = LearningSyncRecord>) -> LearningSyncState {
    let mut state = LearningSyncState::default();
    for record in records {
        state.insert(record).unwrap();
    }
    state
}

fn weight(frequency: u64, time: u64) -> LearningSyncRecord {
    LearningSyncRecord::Weight {
        dictionary: "english".into(),
        key: "sample".into(),
        value: "Sample".into(),
        weight: frequency,
        display: "Sample".into(),
        updated_at: time,
    }
}

fn fixed(key: &str, slot: u8, time: u64) -> LearningSyncRecord {
    LearningSyncRecord::Position {
        context: "synthetic".into(),
        key: key.into(),
        value: key.into(),
        position: slot,
        updated_at: time,
    }
}

fn pinned(value: &str, time: u64) -> LearningSyncRecord {
    LearningSyncRecord::Pin {
        context: "synthetic".into(),
        value: value.into(),
        updated_at: time,
    }
}

fn export(dir: &Dir) -> (LearningSyncState, LearningSyncExport) {
    let mut state = LearningSyncState::default();
    let info = stream_learning_sync_state(&paths(dir), &mut |record| {
        state.insert(record.clone()).unwrap();
        true
    })
    .unwrap();
    (state, info)
}

fn apply(dir: &Dir, remote: &LearningSyncState) -> LearningSyncMerge {
    merge_learning_sync_state(&paths(dir), true, remote, 0).unwrap()
}

fn query(dir: &Dir, sql: &str) -> i64 {
    count(&dir.journal(), sql)
}

#[test]
fn missing_and_corrupt_journals_are_never_created_or_replaced_by_export() {
    let dir = Dir::new();
    let mut emitted = 0;
    assert_eq!(
        stream_learning_sync_state(&paths(&dir), &mut |_| {
            emitted += 1;
            true
        })
        .unwrap(),
        LearningSyncExport::default()
    );
    assert_eq!(emitted, 0);
    assert!(!dir.journal().exists());
    std::fs::write(dir.journal(), b"synthetic invalid journal").unwrap();
    assert!(stream_learning_sync_state(&paths(&dir), &mut |_| true).is_err());
    assert_eq!(
        std::fs::read(dir.journal()).unwrap(),
        b"synthetic invalid journal"
    );
}

#[test]
fn export_preserves_actual_times_skips_owned_words_and_observes_unknown_kinds() {
    let dir = fixture();
    open_journal(&dir.journal()).unwrap().execute_batch(
        "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) VALUES\
         ('english','sample','Sample','upsert',23000000000,'Sample',0,9),\
         ('english','own','Own','upsert',5,'Own',1,10),\
         ('english','gone','Gone','delete',0,'',1,11),\
         ('future-kind','future','未来','upsert',1,'',0,99);\
         INSERT INTO pinned_candidates VALUES('synthetic','甲',12);\
         INSERT INTO fixed_candidate_position_state VALUES('synthetic','a','a',1,13),('synthetic','b','b',1,14);"
    ).unwrap();
    let (actual, info) = export(&dir);
    assert_eq!(
        info,
        LearningSyncExport {
            records: 5,
            skipped: 1,
            observed_max: 99
        }
    );
    assert_eq!(
        actual,
        state([
            weight(23_000_000_000, 9),
            LearningSyncRecord::Delete {
                dictionary: "english".into(),
                key: "gone".into(),
                value: "Gone".into(),
                updated_at: 11
            },
            pinned("甲", 12),
            fixed("a", 1, 13),
            fixed("b", 1, 14)
        ])
    );
    assert_eq!(
        query(&dir, "SELECT count(*) FROM fixed_candidate_positions"),
        0
    );
}

#[test]
fn legacy_display_rows_export_at_one_without_creating_the_new_table() {
    let dir = fixture();
    {
        let connection = open_journal(&dir.journal()).unwrap();
        connection
            .execute_batch(
                "DROP TABLE fixed_candidate_position_state;\
            INSERT INTO fixed_candidate_positions VALUES('synthetic','a','a',2)",
            )
            .unwrap();
    }
    let (actual, _) = export(&dir);
    assert_eq!(actual, state([fixed("a", 2, 1)]));
    assert_eq!(
        query(
            &dir,
            "SELECT count(*) FROM sqlite_master WHERE name='fixed_candidate_position_state'"
        ),
        0
    );
}

#[test]
fn existing_full_state_and_new_display_only_identities_both_export() {
    let dir = fixture();
    open_journal(&dir.journal()).unwrap().execute_batch(
        "INSERT INTO fixed_candidate_position_state VALUES('synthetic','a','a',1,9);\
         INSERT INTO fixed_candidate_positions VALUES('synthetic','a','a',1),('synthetic','b','b',2)"
    ).unwrap();
    assert_eq!(export(&dir).0, state([fixed("a", 1, 9), fixed("b", 2, 1)]));
    assert_eq!(
        query(&dir, "SELECT count(*) FROM fixed_candidate_position_state"),
        1
    );
}

#[test]
fn merge_reopen_encode_and_export_preserve_hidden_candidates_and_times() {
    let dir = fixture();
    let expected = state([
        weight(23_000_000_000, 10),
        pinned("甲", 11),
        fixed("a", 1, 12),
        fixed("b", 1, 13),
    ]);
    let first = apply(&dir, &expected);
    assert_eq!((first.operations, first.pins, first.positions), (1, 1, 2));
    assert_eq!(
        query(
            &dir,
            "SELECT count(*) FROM fixed_candidate_positions WHERE entry_key='b'"
        ),
        1
    );
    assert_eq!(
        query(&dir, "SELECT count(*) FROM fixed_candidate_position_state"),
        2
    );
    release_thread_journal();
    let (actual, _) = export(&dir);
    assert_eq!(actual, expected);
    let mut bytes = Vec::new();
    write_learning_sync(&actual, "synthetic-content", &mut bytes).unwrap();
    let decoded = read_learning_sync(Cursor::new(bytes)).unwrap();
    let second = fixture();
    apply(&second, &decoded.state);
    assert_eq!(export(&second).0, expected);
    assert_eq!(
        apply(&second, &decoded.state),
        LearningSyncMerge {
            kept: 4,
            ..LearningSyncMerge::default()
        }
    );
}

#[test]
fn sqlite_merges_converge_for_all_three_device_orders() {
    let devices = [
        state([fixed("b", 2, 1), weight(100, 4), pinned("a", 4)]),
        state([fixed("a", 2, 1), weight(200, 4), pinned("b", 4)]),
        state([fixed("a", 1, 1), weight(1, 5), pinned("c", 3)]),
    ];
    let mut expected = LearningSyncState::default();
    for device in &devices {
        expected.combine(device);
    }
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let dir = fixture();
        for index in order {
            apply(&dir, &devices[index]);
            release_thread_journal();
        }
        assert_eq!(export(&dir).0, expected, "{order:?}");
        assert_eq!(
            query(
                &dir,
                "SELECT position FROM fixed_candidate_positions WHERE entry_key='a'"
            ),
            1
        );
        assert_eq!(
            query(
                &dir,
                "SELECT position FROM fixed_candidate_positions WHERE entry_key='b'"
            ),
            2
        );
    }
}

#[test]
fn owned_words_are_protected_even_from_newer_remote_deletes() {
    let dir = fixture();
    open_journal(&dir.journal()).unwrap().execute_batch(
        "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at)\
         VALUES('english','sample','Sample','upsert',7,'Sample',1,1)"
    ).unwrap();
    let deletion = LearningSyncRecord::Delete {
        dictionary: "english".into(),
        key: "sample".into(),
        value: "Sample".into(),
        updated_at: 99,
    };
    for incoming in [weight(100, 98), deletion] {
        assert_eq!(apply(&dir, &state([incoming])).kept, 1);
        assert_eq!(
            query(&dir, "SELECT weight FROM user_dictionary_operations"),
            7
        );
        assert_eq!(
            count(
                &dir.english_db(),
                "SELECT weight FROM english_words WHERE word='sample'"
            ),
            1
        );
    }
}

#[test]
fn reset_floor_is_checked_again_at_application_including_hidden_positions() {
    let dir = fixture();
    let remote = state([
        weight(5, 10),
        pinned("甲", 10),
        fixed("a", 1, 10),
        fixed("b", 1, 11),
    ]);
    let result = merge_learning_sync_state(&paths(&dir), true, &remote, 10).unwrap();
    assert_eq!(result.filtered, 3);
    assert_eq!(result.positions, 1);
    assert_eq!(export(&dir).0, state([fixed("b", 1, 11)]));
}

#[test]
fn an_empty_or_fully_filtered_batch_does_not_create_storage() {
    let dir = Dir::new();
    for remote in [state([]), state([fixed("a", 1, 10)])] {
        let expected = remote.records().len();
        assert_eq!(
            merge_learning_sync_state(&paths(&dir), false, &remote, 10)
                .unwrap()
                .filtered,
            expected
        );
        assert!(!dir.journal().exists());
        assert!(!dir.english_db().exists());
    }
}

#[test]
fn remote_delete_removes_the_word_pin_and_every_ngram_reference() {
    let dir = fixture();
    open_journal(&dir.journal()).unwrap().execute_batch(
        "INSERT INTO pinned_candidates VALUES('synthetic','甲',1);\
         INSERT INTO personal_bigram VALUES('甲','乙',1),('乙','甲',1),('乙','丙',2);\
         INSERT INTO personal_trigram VALUES('甲','乙','丙',1),('乙','甲','丙',1),('乙','丙','甲',1)"
    ).unwrap();
    let remote = state([LearningSyncRecord::Delete {
        dictionary: "pinyin".into(),
        key: "jia".into(),
        value: "甲".into(),
        updated_at: 20,
    }]);
    assert_eq!(apply(&dir, &remote).operations, 1);
    assert_eq!(query(&dir, "SELECT count(*) FROM pinned_candidates"), 0);
    assert_eq!(query(&dir, "SELECT count(*) FROM personal_bigram"), 1);
    assert_eq!(query(&dir, "SELECT count(*) FROM personal_trigram"), 0);
    assert_eq!(
        count(
            &dir.main_db(),
            "SELECT count(*) FROM tbl_1_j WHERE value='甲'"
        ),
        0
    );
    // 删词仍清可见置顶与联想；同步导出保留隐藏胜者，避免改变到达次序后丢失比较历史。
    let mut expected = remote;
    expected.insert(pinned("甲", 1)).unwrap();
    assert_eq!(export(&dir).0, expected);
}

#[test]
fn failed_projection_rolls_back_working_dictionaries_and_all_journal_changes() {
    let dir = fixture();
    open_journal(&dir.journal())
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_projection BEFORE INSERT ON fixed_candidate_positions \
         BEGIN SELECT RAISE(ABORT,'synthetic rejection'); END",
        )
        .unwrap();
    let remote = state([weight(99, 10), pinned("甲", 11), fixed("a", 1, 12)]);
    assert!(merge_learning_sync_state(&paths(&dir), true, &remote, 0).is_err());
    assert_eq!(
        count(
            &dir.english_db(),
            "SELECT weight FROM english_words WHERE word='sample'"
        ),
        1
    );
    assert_eq!(export(&dir).0, LearningSyncState::default());
    assert_eq!(
        query(&dir, "SELECT count(*) FROM fixed_candidate_positions"),
        0
    );
}

#[test]
fn missing_working_tables_are_skipped_but_other_sql_errors_abort_the_merge() {
    let dir = fixture();
    let remote = state([LearningSyncRecord::Weight {
        dictionary: "wubi".into(),
        key: "gg".into(),
        value: "甲".into(),
        weight: 5,
        display: "".into(),
        updated_at: 1,
    }]);
    assert_eq!(apply(&dir, &remote).skipped, 1);
    assert_eq!(export(&dir).0, LearningSyncState::default());
    Connection::open(dir.english_db()).unwrap().execute_batch(
        "CREATE TRIGGER reject_weight BEFORE UPDATE ON english_words BEGIN SELECT RAISE(ABORT,'synthetic rejection'); END"
    ).unwrap();
    assert!(merge_learning_sync_state(&paths(&dir), true, &state([weight(5, 2)]), 0).is_err());
    assert_eq!(
        query(&dir, "SELECT count(*) FROM user_dictionary_operations"),
        0
    );
}

#[test]
fn malformed_source_rows_are_rejected_without_exposing_their_contents() {
    let dir = fixture();
    for sql in [
        "INSERT INTO pinned_candidates VALUES('synthetic',CAST(x'ff' AS TEXT),1)",
        "INSERT INTO pinned_candidates VALUES('synthetic','synthetic private',0)",
        "INSERT INTO pinned_candidates VALUES('synthetic',replace(hex(zeroblob(4097)),'0','a'),1)",
    ] {
        let connection = open_journal(&dir.journal()).unwrap();
        connection
            .execute_batch("DELETE FROM pinned_candidates")
            .unwrap();
        connection.execute_batch(sql).unwrap();
        let error = stream_learning_sync_state(&paths(&dir), &mut |_| true).unwrap_err();
        assert_eq!(error.to_string(), "Invalid learning sync state");
    }
}

#[test]
fn export_reads_one_snapshot_even_when_a_writer_commits_during_the_callback() {
    let dir = fixture();
    let connection = open_journal(&dir.journal()).unwrap();
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL;\
        INSERT INTO pinned_candidates VALUES('a','甲',1),('b','乙',2)",
        )
        .unwrap();
    let mut seen = Vec::new();
    stream_learning_sync_state(&paths(&dir), &mut |record| {
        seen.push(record.clone());
        if seen.len() == 1 {
            connection
                .execute_batch("UPDATE pinned_candidates SET updated_at=9 WHERE context_key='b'")
                .unwrap();
        }
        true
    })
    .unwrap();
    assert_eq!(
        seen.iter()
            .map(LearningSyncRecord::updated_at)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        query(
            &dir,
            "SELECT updated_at FROM pinned_candidates WHERE context_key='b'"
        ),
        9
    );
    assert!(stream_learning_sync_state(&paths(&dir), &mut |_| false).is_err());
}

#[test]
fn full_sqlite_weight_range_survives_database_application_and_export() {
    for frequency in [0, 23_000_000_000, i64::MAX as u64] {
        let dir = fixture();
        let remote = state([weight(frequency, 10)]);
        apply(&dir, &remote);
        assert_eq!(
            count(
                &dir.english_db(),
                "SELECT weight FROM english_words WHERE word='sample'"
            ),
            frequency as i64
        );
        assert_eq!(export(&dir).0, remote);
    }
}

#[test]
fn same_time_operations_use_delete_then_weight_and_display_priority() {
    let dir = fixture();
    apply(&dir, &state([weight(100, 10)]));
    assert_eq!(apply(&dir, &state([weight(99, 10)])).kept, 1);
    assert_eq!(apply(&dir, &state([weight(101, 10)])).operations, 1);
    let deletion = LearningSyncRecord::Delete {
        dictionary: "english".into(),
        key: "sample".into(),
        value: "Sample".into(),
        updated_at: 10,
    };
    assert_eq!(apply(&dir, &state([deletion.clone()])).operations, 1);
    assert_eq!(apply(&dir, &state([weight(1000, 10)])).kept, 1);
    assert_eq!(export(&dir).0, state([deletion]));
    assert_eq!(apply(&dir, &state([weight(1, 11)])).operations, 1);
    assert_eq!(
        count(
            &dir.english_db(),
            "SELECT weight FROM english_words WHERE word='sample'"
        ),
        1
    );
}

#[test]
fn stale_zero_pinyin_weights_are_not_applied_or_exported_as_live_learning() {
    let dir = fixture();
    open_journal(&dir.journal()).unwrap().execute_batch(
        "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,updated_at) \
         VALUES('pinyin','jia','甲','upsert',0,1)"
    ).unwrap();
    let (exported, info) = export(&dir);
    assert_eq!(exported, LearningSyncState::default());
    assert_eq!(info.skipped, 1);
    assert_eq!(info.observed_max, 1);
    let remote = state([LearningSyncRecord::Weight {
        dictionary: "pinyin".into(),
        key: "jia".into(),
        value: "甲".into(),
        weight: 0,
        display: "".into(),
        updated_at: 2,
    }]);
    assert_eq!(apply(&dir, &remote).skipped, 1);
    assert_eq!(
        count(
            &dir.main_db(),
            "SELECT weight FROM tbl_1_j WHERE value='甲'"
        ),
        1
    );
}

#[test]
fn existing_maximum_length_quick_phrases_round_trip_without_truncation() {
    let dir = fixture();
    Connection::open(dir.main_db())
        .unwrap()
        .execute_batch("CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER)")
        .unwrap();
    let remote = state([LearningSyncRecord::Weight {
        dictionary: "quick".into(),
        key: "synthetic".into(),
        value: "a".repeat(4096),
        weight: 1,
        display: "".into(),
        updated_at: 5,
    }]);
    assert_eq!(apply(&dir, &remote).operations, 1);
    assert_eq!(export(&dir).0, remote);
    let mut bytes = Vec::new();
    write_learning_sync(&remote, "synthetic-content", &mut bytes).unwrap();
    assert_eq!(
        read_learning_sync(Cursor::new(bytes)).unwrap().state,
        remote
    );
}

#[test]
fn a_later_failure_restores_deleted_words_pins_and_ngram_rows() {
    let dir = fixture();
    open_journal(&dir.journal())
        .unwrap()
        .execute_batch(
            "INSERT INTO pinned_candidates VALUES('synthetic','甲',1);\
         INSERT INTO personal_bigram VALUES('甲','乙',1);\
         CREATE TRIGGER reject_projection BEFORE INSERT ON fixed_candidate_positions \
         BEGIN SELECT RAISE(ABORT,'synthetic rejection'); END",
        )
        .unwrap();
    let remote = state([
        LearningSyncRecord::Delete {
            dictionary: "pinyin".into(),
            key: "jia".into(),
            value: "甲".into(),
            updated_at: 5,
        },
        fixed("a", 1, 6),
    ]);
    assert!(merge_learning_sync_state(&paths(&dir), true, &remote, 0).is_err());
    assert_eq!(
        query(
            &dir,
            "SELECT count(*) FROM pinned_candidates WHERE value='甲'"
        ),
        1
    );
    assert_eq!(query(&dir, "SELECT count(*) FROM personal_bigram"), 1);
    assert_eq!(
        query(&dir, "SELECT count(*) FROM user_dictionary_operations"),
        0
    );
    assert_eq!(
        count(
            &dir.main_db(),
            "SELECT count(*) FROM tbl_1_j WHERE value='甲'"
        ),
        1
    );
}

#[test]
fn delete_and_pin_converge_across_every_arrival_order_and_restart() {
    let deletion = LearningSyncRecord::Delete {
        dictionary: "pinyin".into(),
        key: "jia".into(),
        value: "甲".into(),
        updated_at: 20,
    };
    let records = [
        state([deletion]),
        state([pinned("甲", 10)]),
        state([pinned("乙", 9)]),
    ];
    let mut expected = LearningSyncState::default();
    for record in &records {
        expected.combine(record);
    }
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let dir = fixture();
        for i in order {
            apply(&dir, &records[i]);
            release_thread_journal();
        }
        assert_eq!(export(&dir).0, expected, "{order:?}");
        assert_eq!(
            query(&dir, "SELECT count(*) FROM pinned_candidates"),
            0,
            "{order:?}"
        );
        let encoded = {
            let mut bytes = Vec::new();
            write_learning_sync(&export(&dir).0, "synthetic-content", &mut bytes).unwrap();
            bytes
        };
        let decoded = read_learning_sync(Cursor::new(encoded)).unwrap();
        let restored = fixture();
        apply(&restored, &decoded.state);
        assert_eq!(export(&restored).0, expected);
        assert_eq!(
            query(&restored, "SELECT count(*) FROM pinned_candidates"),
            0
        );
        apply(&dir, &records[2]);
        assert_eq!(
            query(&dir, "SELECT count(*) FROM pinned_candidates"),
            0,
            "较旧的另一个词不能因胜者隐藏而上位"
        );
    }
}

#[test]
fn a_later_weight_reprojects_the_same_pin_in_every_order() {
    let deletion = state([LearningSyncRecord::Delete {
        dictionary: "pinyin".into(),
        key: "jia".into(),
        value: "甲".into(),
        updated_at: 20,
    }]);
    let restore = state([LearningSyncRecord::Weight {
        dictionary: "pinyin".into(),
        key: "jia".into(),
        value: "甲".into(),
        weight: 9,
        display: "".into(),
        updated_at: 21,
    }]);
    let pin = state([pinned("甲", 10)]);
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let dir = fixture();
        let states = [&deletion, &restore, &pin];
        for i in order {
            apply(&dir, states[i]);
        }
        assert_eq!(
            query(
                &dir,
                "SELECT count(*) FROM pinned_candidates WHERE value='甲'"
            ),
            1,
            "{order:?}"
        );
    }
}

#[test]
fn local_delete_keeps_the_hidden_pin_for_later_sync_comparison() {
    let dir = fixture();
    crate::user_dictionary::positions::record_pinned_candidate(&dir.journal(), "synthetic", "甲")
        .unwrap();
    crate::user_dictionary::removal::delete_dictionary_candidate(
        &dir.main_db(),
        &dir.journal(),
        PersonalDictionaryKind::Pinyin,
        "jia",
        "甲",
    )
    .unwrap();
    assert_eq!(query(&dir, "SELECT count(*) FROM pinned_candidates"), 0);
    let (exported, _) = export(&dir);
    assert!(exported
        .records()
        .any(|r| matches!(r,LearningSyncRecord::Pin{value,..} if value=="甲")));
    let other = fixture();
    apply(&other, &exported);
    assert_eq!(export(&other).0, exported);
    assert_eq!(query(&other, "SELECT count(*) FROM pinned_candidates"), 0);
}

#[test]
fn failed_pin_projection_rolls_back_the_comparison_state() {
    let dir = fixture();
    open_journal(&dir.journal()).unwrap().execute_batch(
        "CREATE TRIGGER reject_pin BEFORE INSERT ON pinned_candidates BEGIN SELECT RAISE(ABORT,'synthetic rejection'); END;"
    ).unwrap();
    assert!(merge_learning_sync_state(&paths(&dir), true, &state([pinned("甲", 10)]), 0).is_err());
    assert_eq!(
        query(&dir, "SELECT count(*) FROM pinned_candidate_state"),
        0
    );
    assert_eq!(query(&dir, "SELECT count(*) FROM pinned_candidates"), 0);
}
