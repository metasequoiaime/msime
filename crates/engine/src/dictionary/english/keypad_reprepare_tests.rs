use super::*;
use rusqlite::StatementStatus;

#[test]
fn keypad_queries_reuse_the_prepared_program_after_binding_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = super::super::fixtures::english_db(
        directory.path(),
        &[
            ("alpha", "Alpha", 10),
            ("also", "Also", 8),
            ("beta", "Beta", 7),
        ],
        &[],
        &[],
    );
    let dictionary = EnglishDictionary::open(&path, None, None);
    let queries = [
        ("a", "[aAbBcC]*", 1),
        ("al", "[aAbBcC][jJkKlL]*", 2),
        ("be", "[aAbBcC][dDeEfF]*", 2),
        ("al", "[aAbBcC][jJkKlL][pPqQrRsS]*", 3),
    ];
    let mut candidates = Vec::new();
    for _ in 0..2 {
        for (prefix, pattern, typed) in queries {
            candidates.clear();
            dictionary.append_keypad_prefix(prefix, pattern, typed, 5, &mut candidates, 320);
        }
    }
    let connection = dictionary.connection.as_ref().unwrap();
    let sql = format!("{KEYPAD_PREFIX_SQL}5");
    let statement = connection.prepare_cached(&sql).unwrap();
    let runs = statement.get_status(StatementStatus::Run);
    let recompiles = statement.get_status(StatementStatus::RePrepare);
    eprintln!("KEYPAD_PROGRAM_REUSE runs={runs} recompiles={recompiles}");
    assert_eq!(runs, 8);
    assert_eq!(recompiles, 0, "已有显式索引范围的字符组模式不应重复规划");
    let current_steps = statement.get_status(StatementStatus::VmStep);
    drop(statement);
    for _ in 0..2 {
        for (prefix, pattern, typed) in queries {
            dictionary.legacy_query_keypad_prefix(prefix, pattern, typed, 5);
        }
    }
    let frozen_sql = format!("{}5", super::keypad_append_tests::FROZEN_KEYPAD_PREFIX_SQL);
    let frozen = connection.prepare_cached(&frozen_sql).unwrap();
    assert_eq!(frozen.get_status(StatementStatus::Run), 8);
    assert_eq!(frozen.get_status(StatementStatus::RePrepare), 8);
    eprintln!(
        "KEYPAD_PROGRAM_CONTROL old_recompiles=8 new_recompiles={recompiles} old_vm_steps={} new_vm_steps={current_steps}",
        frozen.get_status(StatementStatus::VmStep)
    );
}

#[test]
fn keypad_program_still_rebuilds_after_a_schema_change() {
    let directory = tempfile::tempdir().unwrap();
    let path =
        super::super::fixtures::english_db(directory.path(), &[("alpha", "Alpha", 10)], &[], &[]);
    let dictionary = EnglishDictionary::open(&path, None, None);
    let mut candidates = Vec::new();
    dictionary.append_keypad_prefix("al", "[aAbBcC][jJkKlL]*", 2, 5, &mut candidates, 320);
    dictionary
        .keypad_prefix_rows("al", "[aAbBcC][jJkKlL]*", 5)
        .unwrap();
    let connection = dictionary.connection.as_ref().unwrap();
    let sql = format!("{KEYPAD_PREFIX_SQL}5");
    let rows_sql = format!("{KEYPAD_PREFIX_ROWS_SQL}6");
    connection
        .prepare_cached(&rows_sql)
        .unwrap()
        .reset_status(StatementStatus::RePrepare);
    connection
        .prepare_cached(&sql)
        .unwrap()
        .reset_status(StatementStatus::RePrepare);
    let writer = Connection::open(path).unwrap();
    writer
        .execute_batch("CREATE TABLE schema_probe(value TEXT); INSERT INTO english_words VALUES('alpine','Alpine',99)")
        .unwrap();
    candidates.clear();
    dictionary.append_keypad_prefix("al", "[aAbBcC][jJkKlL]*", 2, 5, &mut candidates, 320);
    assert_eq!(candidates[0].word, "Alpine");
    let cached_rows = dictionary
        .keypad_prefix_rows("al", "[aAbBcC][jJkKlL]*", 5)
        .unwrap();
    assert_eq!(cached_rows[0].display, "Alpine");
    assert_eq!(
        connection
            .prepare_cached(&rows_sql)
            .unwrap()
            .get_status(StatementStatus::RePrepare),
        1
    );
    assert_eq!(
        connection
            .prepare_cached(&sql)
            .unwrap()
            .get_status(StatementStatus::RePrepare),
        1,
        "只消除模式绑定引起的重新编译，schema 变化仍须生效"
    );
}

#[test]
fn keypad_pattern_expression_preserves_the_explicit_range_plan() {
    let directory = tempfile::tempdir().unwrap();
    let path = super::super::fixtures::english_db(
        directory.path(),
        &[
            ("alpha", "Alpha", 10),
            ("also", "Also", 8),
            ("beta", "Beta", 7),
        ],
        &[],
        &[],
    );
    let dictionary = EnglishDictionary::open(&path, None, None);
    let connection = dictionary.connection.as_ref().unwrap();
    for limit in [1, 5, 32] {
        for (prefix, pattern, typed) in [
            ("a", "[aAbBcC]*", 1),
            ("al", "[aAbBcC][jJkKlL]*", 2),
            ("be", "[aAbBcC][dDeEfF]*", 2),
            ("zz", "[wWxXyYzZ][wWxXyYzZ]*", 2),
        ] {
            let upper = prefix_upper_bound(prefix);
            let parameters = rusqlite::params![prefix, upper, pattern, typed];
            let plan = |sql: &str| {
                connection
                    .prepare(&format!("EXPLAIN QUERY PLAN {sql}{limit}"))
                    .unwrap()
                    .query_map(parameters, |row| row.get::<_, String>(3))
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            };
            let expected = plan(super::keypad_append_tests::FROZEN_KEYPAD_PREFIX_SQL);
            let actual = plan(KEYPAD_PREFIX_SQL);
            assert_eq!(actual, expected);
            assert!(actual.iter().any(|line| line.contains("word>? AND word<?")));
        }
    }
}

#[test]
fn keypad_cached_rows_reuse_the_program_and_match_the_frozen_query() {
    // 固定前缀缓存合入时的查询；独立对照不能跟随生产常量变化。
    const FROZEN_ROWS_SQL: &str = "SELECT word,display,weight,length(CAST(display AS BLOB)),weight>0 FROM english_words WHERE word >= ?1 AND word < ?2 AND word GLOB ?3 AND word NOT GLOB '*[^a-zA-Z]*' AND instr(word,char(0))=0 AND display IS NOT NULL ORDER BY weight DESC,length(CAST(display AS BLOB)),CASE WHEN word=?1 THEN 0 ELSE 1 END,length(word),word,display LIMIT 6;";
    let directory = tempfile::tempdir().unwrap();
    let path = super::super::fixtures::english_db(
        directory.path(),
        &[
            ("alpha", "Alpha", 10),
            ("also", "Also", 8),
            ("beta", "Beta", 7),
        ],
        &[],
        &[],
    );
    let dictionary = EnglishDictionary::open(&path, None, None);
    let connection = dictionary.connection.as_ref().unwrap();
    for _ in 0..2 {
        for (prefix, pattern) in [
            ("a", "[aAbBcC]*"),
            ("al", "[aAbBcC][jJkKlL]*"),
            ("be", "[aAbBcC][dDeEfF]*"),
            ("al", "[aAbBcC][jJkKlL][pPqQrRsS]*"),
        ] {
            let actual: Vec<_> = dictionary
                .keypad_prefix_rows(prefix, pattern, 5)
                .unwrap()
                .into_iter()
                .map(|row| {
                    (
                        row.word,
                        row.display,
                        row.weight,
                        row.display_bytes,
                        row.positive,
                    )
                })
                .collect();
            let expected = connection
                .prepare_cached(FROZEN_ROWS_SQL)
                .unwrap()
                .query_map(
                    rusqlite::params![prefix, prefix_upper_bound(prefix), pattern],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            usize::try_from(row.get::<_, i64>(3)?).unwrap(),
                            row.get::<_, bool>(4)?,
                        ))
                    },
                )
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(actual, expected);
        }
    }
    let statement = connection
        .prepare_cached(&format!("{KEYPAD_PREFIX_ROWS_SQL}6"))
        .unwrap();
    let frozen = connection.prepare_cached(FROZEN_ROWS_SQL).unwrap();
    assert_eq!(statement.get_status(StatementStatus::Run), 8);
    assert_eq!(frozen.get_status(StatementStatus::Run), 8);
    assert_eq!(frozen.get_status(StatementStatus::RePrepare), 8);
    let recompiles = statement.get_status(StatementStatus::RePrepare);
    eprintln!("KEYPAD_ROWS_PROGRAM_REUSE runs=8 recompiles={recompiles} old_recompiles=8");
    assert_eq!(recompiles, 0, "前缀缓存填充也不应因字符组模式重新规划");
}
