use super::*;
use crate::assets;
use crate::paths::RuntimePaths;
use crate::types::SchemeSet;
use crate::user_dictionary::journal::{open_journal, release_thread_journal, test_support::Dir};
use crate::user_dictionary::positions::{clear_fixed_position, set_fixed_position};
use crate::user_dictionary::state::{
    merge_dictionary_state, stage_dictionary_state_for, DictionaryStateRecord,
};

fn legacy() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(
        "CREATE TABLE fixed_candidate_positions(context_key TEXT,entry_key TEXT,value TEXT,position INTEGER);\
         INSERT INTO fixed_candidate_positions VALUES('synthetic','a','甲',1);",
    ).unwrap();
    connection
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn rows(connection: &Connection) -> Vec<(String, String, i32)> {
    connection
        .prepare("SELECT entry_key,value,position FROM fixed_candidate_positions ORDER BY context_key,position")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn full(connection: &Connection, key: &str, value: &str, position: i32, timestamp: u64) {
    connection
        .execute(
            "INSERT INTO fixed_candidate_position_state VALUES('synthetic',?1,?2,?3,?4)",
            params![key, value, position, i64::try_from(timestamp).unwrap()],
        )
        .unwrap();
}

fn backup(key: &str, value: &str, position: i32) -> DictionaryStateRecord {
    DictionaryStateRecord::Position {
        context: "synthetic".into(),
        key: key.into(),
        value: value.into(),
        position: position.into(),
    }
}

fn paths(dir: &Dir) -> RuntimePaths {
    dir.english(&[]);
    let root = dir.root.path();
    RuntimePaths {
        resources: root.join("resources"),
        user_data: root.to_owned(),
        cache: root.join("cache"),
        dictionaries: root.to_owned(),
    }
}

#[test]
fn legacy_migration_is_once_only_and_uses_timestamp_one() {
    let connection = legacy();
    ensure_schema(&connection).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT updated_at FROM fixed_candidate_position_state WHERE entry_key='a'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    connection
        .execute("DELETE FROM fixed_candidate_position_state", [])
        .unwrap();
    ensure_schema(&connection).unwrap();
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 0);
    assert_eq!(count(&connection, "fixed_candidate_positions"), 1);
}

#[test]
fn failed_migration_rolls_back_its_table_and_can_retry() {
    let connection = legacy();
    connection
        .execute("UPDATE fixed_candidate_positions SET position=6", [])
        .unwrap();
    assert!(ensure_schema(&connection).is_err());
    assert!(connection.is_autocommit());
    assert!(!exists(&connection).unwrap());
    connection
        .execute("UPDATE fixed_candidate_positions SET position=1", [])
        .unwrap();
    ensure_schema(&connection).unwrap();
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 1);
}

#[test]
fn migration_joins_an_outer_transaction_without_committing_it() {
    let connection = legacy();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    ensure_schema(&connection).unwrap();
    assert!(!connection.is_autocommit());
    connection.execute_batch("ROLLBACK").unwrap();
    assert!(!exists(&connection).unwrap());
    assert_eq!(count(&connection, "fixed_candidate_positions"), 1);
}

#[test]
fn local_replacement_then_cancel_never_revives_the_previous_owner() {
    let dir = Dir::new();
    let file = dir.journal();
    set_fixed_position(&file, "synthetic", "a", "甲", 1).unwrap();
    set_fixed_position(&file, "synthetic", "b", "乙", 1).unwrap();
    clear_fixed_position(&file, "synthetic", "b", "乙").unwrap();
    release_thread_journal();
    let connection = open_journal(&file).unwrap();
    assert!(rows(&connection).is_empty());
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 0);
}

#[test]
fn display_projection_preserves_hidden_candidates_across_reopen_and_cancel() {
    let dir = Dir::new();
    let file = dir.journal();
    {
        let connection = open_journal(&file).unwrap();
        full(&connection, "a", "甲", 1, 5);
        full(&connection, "b", "乙", 1, 6);
        full(&connection, "c", "丙", 2, 9);
        full(&connection, "d", "丁", 2, 9);
        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        project(&connection, "synthetic").unwrap();
        connection.execute_batch("COMMIT").unwrap();
        assert_eq!(
            rows(&connection),
            [("b".into(), "乙".into(), 1), ("c".into(), "丙".into(), 2)]
        );
        assert_eq!(count(&connection, "fixed_candidate_position_state"), 4);
    }
    release_thread_journal();
    {
        let connection = open_journal(&file).unwrap();
        assert_eq!(count(&connection, "fixed_candidate_position_state"), 4);
    }
    clear_fixed_position(&file, "synthetic", "b", "乙").unwrap();
    let connection = open_journal(&file).unwrap();
    assert_eq!(
        rows(&connection),
        [("a".into(), "甲".into(), 1), ("c".into(), "丙".into(), 2)]
    );
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 3);
}

#[test]
fn moving_a_word_to_another_slot_leaves_one_fixed_record_and_cancel_removes_it() {
    let dir = Dir::new();
    let file = dir.journal();
    set_fixed_position(&file, "synthetic", "aa", "甲", 1).unwrap();
    set_fixed_position(&file, "synthetic", "aad", "甲", 3).unwrap();
    release_thread_journal();
    {
        let connection = open_journal(&file).unwrap();
        assert_eq!(rows(&connection), [("aad".into(), "甲".into(), 3)]);
        assert_eq!(count(&connection, "fixed_candidate_position_state"), 1);
    }
    clear_fixed_position(&file, "synthetic", "aad", "甲").unwrap();
    release_thread_journal();
    let connection = open_journal(&file).unwrap();
    assert!(rows(&connection).is_empty());
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 0);
}

#[test]
fn moving_a_future_dated_candidate_advances_its_clock_and_caps_it() {
    let dir = Dir::new();
    let file = dir.journal();
    let original = MAX_LEARNING_SYNC_TIMESTAMP - 2;
    {
        let connection = open_journal(&file).unwrap();
        full(&connection, "a", "甲", 1, original);
    }
    for (position, expected) in [(2, original + 1), (3, original + 2), (4, original + 2)] {
        set_fixed_position(&file, "synthetic", "a", "甲", position).unwrap();
        let connection = open_journal(&file).unwrap();
        let timestamp: i64 = connection
            .query_row(
                "SELECT updated_at FROM fixed_candidate_position_state",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(timestamp, i64::try_from(expected).unwrap());
        assert_eq!(rows(&connection), [("a".into(), "甲".into(), position)]);
    }
}

#[test]
fn a_projection_failure_rolls_back_full_state_and_display() {
    let dir = Dir::new();
    let file = dir.journal();
    set_fixed_position(&file, "synthetic", "a", "甲", 1).unwrap();
    {
        let connection = open_journal(&file).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_projection BEFORE INSERT ON fixed_candidate_positions \
             WHEN NEW.entry_key='b' BEGIN SELECT RAISE(ABORT,'synthetic rejection'); END;",
            )
            .unwrap();
    }
    assert!(set_fixed_position(&file, "synthetic", "b", "乙", 1).is_err());
    let connection = open_journal(&file).unwrap();
    assert!(connection.is_autocommit());
    assert_eq!(rows(&connection), [("a".into(), "甲".into(), 1)]);
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 1);
    assert_eq!(
        connection
            .query_row(
                "SELECT entry_key FROM fixed_candidate_position_state",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "a"
    );
}

#[test]
fn an_existing_display_only_writer_is_adopted_before_local_edits() {
    let dir = Dir::new();
    let file = dir.journal();
    {
        let connection = open_journal(&file).unwrap();
        connection
            .execute_batch("INSERT INTO fixed_candidate_positions VALUES('synthetic','a','甲',1)")
            .unwrap();
    }
    set_fixed_position(&file, "synthetic", "b", "乙", 2).unwrap();
    let connection = open_journal(&file).unwrap();
    assert_eq!(
        rows(&connection),
        [("a".into(), "甲".into(), 1), ("b".into(), "乙".into(), 2)]
    );
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 2);
}

#[test]
fn backup_merge_keeps_hidden_local_identity_and_records_new_positions() {
    let dir = Dir::new();
    let runtime = paths(&dir);
    {
        let connection = open_journal(&dir.journal()).unwrap();
        full(&connection, "a", "甲", 1, 10);
        full(&connection, "b", "乙", 1, 9);
        project(&connection, "synthetic").unwrap();
    }
    let result = merge_dictionary_state(
        &runtime,
        false,
        &mut [Ok(backup("b", "乙", 2)), Ok(backup("c", "丙", 2))].into_iter(),
        10,
    )
    .unwrap();
    assert_eq!(result.positions, 1);
    assert_eq!(result.kept, 1);
    let connection = open_journal(&dir.journal()).unwrap();
    assert_eq!(
        rows(&connection),
        [("a".into(), "甲".into(), 1), ("c".into(), "丙".into(), 2)]
    );
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 3);
    drop(connection);
    clear_fixed_position(&dir.journal(), "synthetic", "c", "丙").unwrap();
    let connection = open_journal(&dir.journal()).unwrap();
    assert_eq!(rows(&connection), [("a".into(), "甲".into(), 1)]);
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 2);
}

#[test]
fn staging_a_backup_seeds_state_before_the_first_local_edit() {
    let dir = Dir::new();
    dir.english(&[]);
    let resources = dir.root.path().join("resources");
    std::fs::create_dir(&resources).unwrap();
    std::fs::copy(dir.english_db(), resources.join(assets::ENGLISH_DICTIONARY)).unwrap();
    let generation = dir.root.path().join("staged");
    let runtime = stage_dictionary_state_for(
        &resources,
        &generation,
        "synthetic",
        &mut [Ok(backup("a", "甲", 1)), Ok(backup("b", "乙", 2))].into_iter(),
        10,
        SchemeSet::EMPTY,
    )
    .unwrap();
    let file = runtime.user(assets::USER_JOURNAL);
    let connection = open_journal(&file).unwrap();
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 2);
    assert_eq!(
        connection
            .query_row(
                "SELECT min(updated_at) FROM fixed_candidate_position_state",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
    drop(connection);
    clear_fixed_position(&file, "synthetic", "a", "甲").unwrap();
    release_thread_journal();
    let connection = open_journal(&file).unwrap();
    assert_eq!(rows(&connection), [("b".into(), "乙".into(), 2)]);
    assert_eq!(count(&connection, "fixed_candidate_position_state"), 1);
}
