use super::*;
use crate::types::PersonalDictionaryKind;
use crate::user_dictionary::journal::{record_user_insert, test_support::Dir};

const FLOOR: u64 = 10_000_000_000;

fn paths(dir: &Dir) -> RuntimePaths {
    RuntimePaths {
        user_data: dir.root.path().to_owned(),
        dictionaries: dir.root.path().to_owned(),
        resources: dir.root.path().join("resources"),
        cache: dir.root.path().join("cache"),
    }
}

#[test]
fn empty_and_legacy_journals_have_no_floor_and_reads_do_not_migrate_them() {
    let dir = Dir::new();
    assert_eq!(read_path(&dir.journal()).unwrap(), 0);
    assert!(!dir.journal().exists());
    let legacy = Connection::open(dir.journal()).unwrap();
    legacy
        .execute_batch("CREATE TABLE synthetic_legacy(value INTEGER)")
        .unwrap();
    drop(legacy);
    let before = std::fs::read(dir.journal()).unwrap();
    assert_eq!(read_path(&dir.journal()).unwrap(), 0);
    assert_eq!(std::fs::read(dir.journal()).unwrap(), before);
    assert!(advance_learning_sync_clock(&paths(&dir), MAX_LEARNING_SYNC_TIMESTAMP + 1).is_err());
    assert_eq!(std::fs::read(dir.journal()).unwrap(), before);
}

#[test]
fn resetting_learning_twice_keeps_the_clock_but_clears_learning() {
    let dir = Dir::new();
    let paths = paths(&dir);
    std::fs::create_dir(&paths.resources).unwrap();
    Connection::open(paths.resource(assets::ENGLISH_DICTIONARY))
        .unwrap()
        .execute_batch("CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);")
        .unwrap();
    advance_learning_sync_clock(&paths, FLOOR).unwrap();
    record_user_insert(
        &dir.journal(),
        PersonalDictionaryKind::English,
        "sample",
        "Sample",
        1,
        "",
    )
    .unwrap();
    for _ in 0..2 {
        crate::user_dictionary::reset::reset_learned_data_with(&paths, false).unwrap();
        assert_eq!(read_path(&dir.journal()).unwrap(), FLOOR);
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
    record_user_insert(
        &dir.journal(),
        PersonalDictionaryKind::English,
        "new",
        "New",
        1,
        "",
    )
    .unwrap();
    assert_eq!(
        open_journal(&dir.journal())
            .unwrap()
            .query_row(
                "SELECT updated_at FROM user_dictionary_operations",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        FLOOR as i64 + 1
    );
}

#[test]
fn carrying_only_the_clock_preserves_the_larger_local_floor_without_touching_source() {
    let active = Dir::new();
    let staged = Dir::new();
    advance_learning_sync_clock(&paths(&active), FLOOR).unwrap();
    carry_learning_sync_clock(&paths(&active), &paths(&staged)).unwrap();
    assert_eq!(read_path(&staged.journal()).unwrap(), FLOOR);
    advance_learning_sync_clock(&paths(&staged), FLOOR + 10).unwrap();
    carry_learning_sync_clock(&paths(&active), &paths(&staged)).unwrap();
    assert_eq!(read_path(&staged.journal()).unwrap(), FLOOR + 10);
    assert_eq!(read_path(&active.journal()).unwrap(), FLOOR);
    assert_eq!(
        open_journal(&staged.journal())
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
fn malformed_clock_cannot_replace_a_valid_target() {
    let active = Dir::new();
    let staged = Dir::new();
    Connection::open(active.journal())
        .unwrap()
        .execute_batch(
            "CREATE TABLE learning_sync_clock(singleton INTEGER,reset_floor INTEGER); \
         INSERT INTO learning_sync_clock VALUES(1,-1);",
        )
        .unwrap();
    advance_learning_sync_clock(&paths(&staged), FLOOR).unwrap();
    assert!(carry_learning_sync_clock(&paths(&active), &paths(&staged)).is_err());
    assert_eq!(read_path(&staged.journal()).unwrap(), FLOOR);
}
