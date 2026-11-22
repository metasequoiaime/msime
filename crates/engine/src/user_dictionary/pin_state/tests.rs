use super::*;

#[test]
fn failed_migration_rolls_back_the_table_and_can_retry() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE pinned_candidates(context_key TEXT PRIMARY KEY,value TEXT); INSERT INTO pinned_candidates VALUES('synthetic','甲')").unwrap();
    assert!(ensure_schema(&db).is_err());
    let tables: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='pinned_candidate_state'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0);
    db.execute_batch("ALTER TABLE pinned_candidates ADD COLUMN updated_at INTEGER DEFAULT 123")
        .unwrap();
    ensure_schema(&db).unwrap();
    assert_eq!(
        db.query_row("SELECT updated_at FROM pinned_candidate_state", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        123
    );
}

#[test]
fn local_replacement_advances_hidden_time_and_cancel_does_not_revive_it() {
    let db = Connection::open_in_memory().unwrap();
    crate::user_dictionary::journal::ensure_schema(&db).unwrap();
    db.execute_batch("INSERT INTO pinned_candidate_state VALUES('synthetic','甲',1000000000000)")
        .unwrap();
    set(&db, "synthetic", "乙").unwrap();
    assert_eq!(
        db.query_row("SELECT updated_at FROM pinned_candidates", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1000000000001
    );
    clear(&db, "synthetic", "甲").unwrap();
    assert_eq!(
        db.query_row("SELECT value FROM pinned_candidates", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "乙"
    );
    clear(&db, "synthetic", "乙").unwrap();
    project(&db, "main").unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM pinned_candidates", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM pinned_candidate_state", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
