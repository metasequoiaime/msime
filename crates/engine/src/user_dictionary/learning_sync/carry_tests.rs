use super::*;
use crate::user_dictionary::journal::{open_journal, test_support::Dir};
use rusqlite::Connection;

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
    dir.pinyin(&[("jia", "甲", 1)]);
    dir.english(&[("sample", "Sample", 1)]);
    open_journal(&dir.journal()).unwrap();
    dir
}

#[test]
fn carry_preserves_exact_rows_and_hidden_positions_without_copying_user_words() {
    let active = fixture();
    let staged = fixture();
    let source = Connection::open(active.journal()).unwrap();
    source.execute_batch(
        "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) VALUES
         ('english','sample','Sample','upsert',23000000000,'Sample',0,123),
         ('pinyin','yi','乙','delete',0,'',1,124),
         ('pinyin','bing','丙','upsert',99,'',1,125),
         ('pinyin','ding','丁','upsert',0,'',0,126);
         INSERT INTO fixed_candidate_position_state VALUES
         ('synthetic','a','甲',2,127),('synthetic','b','乙',2,128);
         INSERT INTO fixed_candidate_positions VALUES('synthetic','b','乙',2);",
    ).unwrap();
    let before = std::fs::read(active.journal()).unwrap();
    assert_eq!(
        carry_learning_sync_state(&paths(&active), &paths(&staged), true).unwrap(),
        4
    );
    let target = Connection::open(staged.journal()).unwrap();
    assert_eq!(target.query_row(
        "SELECT weight,display,updated_at FROM user_dictionary_operations WHERE dictionary='english'",
        [], |row| Ok((row.get::<_,i64>(0)?, row.get::<_,String>(1)?, row.get::<_,i64>(2)?)),
    ).unwrap(), (23000000000, "Sample".into(), 123));
    assert_eq!(target.query_row(
        "SELECT user_inserted,updated_at FROM user_dictionary_operations WHERE operation='delete'",
        [], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?)),
    ).unwrap(), (1,124));
    assert_eq!(
        target
            .query_row(
                "SELECT count(*) FROM user_dictionary_operations",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(
        target
            .query_row(
                "SELECT count(*) FROM fixed_candidate_position_state",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(
        target
            .query_row("SELECT value FROM fixed_candidate_positions", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "乙"
    );
    assert_eq!(std::fs::read(active.journal()).unwrap(), before);
    assert_eq!(
        carry_learning_sync_state(&paths(&active), &paths(&staged), true).unwrap(),
        0
    );
}

#[test]
fn staged_cloud_identity_wins_even_when_the_old_local_row_is_newer() {
    let active = fixture();
    let staged = fixture();
    for (dir, frequency, timestamp, user) in [(&active, 20, 200, 0), (&staged, 10, 100, 1)] {
        Connection::open(dir.journal()).unwrap().execute(
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) VALUES('english','sample','Sample','upsert',?1,'Sample',?2,?3)",
            params![frequency,user,timestamp],
        ).unwrap();
    }
    assert_eq!(
        carry_learning_sync_state(&paths(&active), &paths(&staged), true).unwrap(),
        0
    );
    assert_eq!(
        Connection::open(staged.journal())
            .unwrap()
            .query_row(
                "SELECT weight,updated_at FROM user_dictionary_operations",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap(),
        (10, 100)
    );
}

#[test]
fn missing_or_invalid_source_does_not_open_the_target() {
    let active = Dir::new();
    let staged = Dir::new();
    assert_eq!(
        carry_learning_sync_state(&paths(&active), &paths(&staged), true).unwrap(),
        0
    );
    assert!(!staged.journal().exists());
    std::fs::write(active.journal(), b"synthetic invalid journal").unwrap();
    assert!(carry_learning_sync_state(&paths(&active), &paths(&staged), true).is_err());
    assert!(!staged.journal().exists());
}

#[test]
fn failed_projection_rolls_back_all_fixed_candidates() {
    let active = fixture();
    let staged = fixture();
    Connection::open(active.journal()).unwrap().execute_batch(
        "INSERT INTO fixed_candidate_position_state VALUES('synthetic','a','甲',2,127),('synthetic','b','乙',2,128);",
    ).unwrap();
    let target = Connection::open(staged.journal()).unwrap();
    target.execute_batch(
        "CREATE TRIGGER reject_projection BEFORE INSERT ON fixed_candidate_positions BEGIN SELECT RAISE(ABORT,'synthetic write failure'); END;",
    ).unwrap();
    assert!(carry_learning_sync_state(&paths(&active), &paths(&staged), true).is_err());
    assert_eq!(
        target
            .query_row(
                "SELECT count(*) FROM fixed_candidate_position_state",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn carry_preserves_hidden_pin_and_its_comparison_history() {
    let active = fixture();
    let staged = fixture();
    Connection::open(active.journal()).unwrap().execute_batch(
        "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) VALUES('pinyin','jia','甲','delete',0,'',0,20); INSERT INTO pinned_candidate_state VALUES('synthetic','甲',10);"
    ).unwrap();
    carry_learning_sync_state(&paths(&active), &paths(&staged), true).unwrap();
    let target = Connection::open(staged.journal()).unwrap();
    assert_eq!(
        target
            .query_row(
                "SELECT value,updated_at FROM pinned_candidate_state",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            )
            .unwrap(),
        ("甲".into(), 10)
    );
    assert_eq!(
        target
            .query_row("SELECT count(*) FROM pinned_candidates", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let mut older = super::super::LearningSyncState::default();
    older
        .insert(LearningSyncRecord::Pin {
            context: "synthetic".into(),
            value: "乙".into(),
            updated_at: 9,
        })
        .unwrap();
    super::super::merge_learning_sync_state(&paths(&staged), true, &older, 0).unwrap();
    assert_eq!(
        target
            .query_row("SELECT count(*) FROM pinned_candidates", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
