use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 `b3934a95ee5b550f6c503d9841ea76aea65ddb00` 的两个完整入口，仅转接接收者并移除文档注释。
fn frozen_query(database: &PinyinDatabase, codes: &[String], limit: usize) -> Vec<DictRow> {
    let mut rows = frozen_query_per_table(database, codes, limit);
    rows.truncate(limit);
    rows
}

fn frozen_query_per_table(
    database: &PinyinDatabase,
    codes: &[String],
    table_limit: usize,
) -> Vec<DictRow> {
    if database.connection.is_none() || codes.is_empty() || table_limit == 0 {
        return Vec::new();
    }
    let mut codes_by_table: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for code in codes {
        let Some(&first) = code.as_bytes().first() else {
            continue;
        };
        let Some(table) = quanpin_table(code.len(), first) else {
            continue;
        };
        let table_codes = codes_by_table.entry(table).or_default();
        if !table_codes.contains(&code.as_str()) {
            table_codes.push(code.as_str());
        }
    }
    let mut rows = Vec::with_capacity(table_limit.min(128));
    for (table, table_codes) in &codes_by_table {
        let sql = jianpin_batch_sql(table, table_codes.len(), sql_limit(table_limit));
        rows.extend(database.rows(
            &sql,
            params_from_iter(table_codes),
            query_capacity(table_limit),
        ));
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
    deduplicate_by_value(&mut rows);
    rows
}

fn codes(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn empty_public_jianpin_queries_keep_zero_row_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    for input in [codes(&["nz"]), codes(&["ih"]), codes(&["", "NH", "合成"])] {
        for limit in [1, 64, 128, 512, usize::MAX] {
            for rows in [
                database.query_jianpin_codes(&input, limit),
                database.query_jianpin_codes_per_table(&input, limit),
            ] {
                assert!(rows.is_empty());
                assert_eq!(rows.capacity(), 0);
            }
        }
    }
}

#[test]
fn sparse_single_table_saves_page_allocation_without_larger_result_capacity() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_2_n", "ni'hao", "合成甲", 50),
            ("tbl_2_n", "ni'hao", "合成乙", -20),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for length in [1, 16, 65, 1024] {
        let input = vec!["nh".to_owned(); length];
        for limit in [1, 2, 64, 128, 512, usize::MAX] {
            drop(frozen_query_per_table(&database, &input, limit));
            drop(database.query_jianpin_codes_per_table(&input, limit));
            let (old, before) = count(|| frozen_query_per_table(&database, &input, limit));
            let (new, after) = count(|| database.query_jianpin_codes_per_table(&input, limit));
            assert_eq!(new, old);
            assert_eq!(new.capacity(), old.capacity());
            assert_eq!(after + 1, before);
        }
    }
}

#[test]
fn multiple_tables_keep_weight_order_value_dedup_and_distinct_limits() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_2_a", "an'hao", "合成未命中", 80),
            ("tbl_1_n", "ni", "合成单字", 50),
            ("tbl_2_n", "ni'hao", "合成同值", 50),
            ("tbl_2_n", "ni'hao", "合成负权", -20),
            ("tbl_2_n", "ni'men", "合成大权", i64::MAX),
            ("tbl_2_s", "shi'jie", "合成同值", 50),
            ("tbl_2_s", "shi'jie", "合成等权", 50),
            ("tbl_3_z", "zi'hao'ma", "合成跨长度", 50),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let input = codes(&["az", "nh", "nm", "sj", "zhm", "n", "nh", "ih", "", "NH"]);
    for limit in [0, 1, 2, 64, 128, 512, usize::MAX] {
        assert_eq!(
            database.query_jianpin_codes_per_table(&input, limit),
            frozen_query_per_table(&database, &input, limit)
        );
        assert_eq!(
            database.query_jianpin_codes(&input, limit),
            frozen_query(&database, &input, limit)
        );
    }
    let per_table = database.query_jianpin_codes_per_table(&input, 1);
    assert_eq!(per_table.len(), 4);
    assert_eq!(database.query_jianpin_codes(&input, 1), per_table[..1]);
    assert_eq!(per_table[0].weight, i64::MAX);
    assert_eq!(per_table[1].key, "ni");
}

#[test]
fn dense_real_caller_limits_do_not_increase_allocation_count() {
    let directory = tempfile::tempdir().unwrap();
    let entries = [
        ("tbl_2_m", "ma'hao", "mh"),
        ("tbl_2_n", "ni'hao", "nh"),
        ("tbl_2_s", "shi'jie", "sj"),
        ("tbl_2_z", "zi'hao", "zh"),
    ];
    let path = pinyin_db(directory.path(), &[]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for (table, key, code) in entries {
        transaction
            .execute_batch(&format!(
                "CREATE TABLE {table}(key TEXT,jp TEXT,value TEXT,weight INTEGER)"
            ))
            .unwrap();
        for index in 0..512 {
            transaction
                .execute(
                    &format!("INSERT INTO {table} VALUES(?1,?2,?3,?4)"),
                    (
                        key,
                        code,
                        format!("合成{table}-{}", index / 2),
                        index as i64,
                    ),
                )
                .unwrap();
        }
    }
    transaction.commit().unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    let input = codes(&["mh", "nh", "sj", "zh"]);
    for limit in [64, 128, 512] {
        drop(frozen_query_per_table(&database, &input, limit));
        drop(database.query_jianpin_codes_per_table(&input, limit));
        let (old, before) = count(|| frozen_query_per_table(&database, &input, limit));
        let (new, after) = count(|| database.query_jianpin_codes_per_table(&input, limit));
        assert_eq!(new, old);
        assert_eq!(new.len(), 2 * limit);
        assert_eq!(new.capacity(), old.capacity());
        assert!(after <= before);
        if limit == 64 {
            assert!(after < before);
        }
        eprintln!(
            "四表密集简拼 limit={limit}: 分配 {before}→{after}, 容量 {}",
            new.capacity()
        );
    }
}

#[test]
fn uneven_pages_keep_capacity_and_heap_budget() {
    let _ = intact_pinyin_set();
    for lengths in [
        vec![129],
        vec![200],
        vec![257],
        vec![383],
        vec![200, 257, 1],
        vec![257, 257, 257, 257],
        vec![171, 257, 383],
        vec![65, 129, 1],
        vec![1, 64, 257],
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = pinyin_db(directory.path(), &[]);
        let mut connection = Connection::open(&path).unwrap();
        let transaction = connection.transaction().unwrap();
        let entries = [
            ("tbl_2_m", "ma'hao", "mh"),
            ("tbl_2_n", "ni'hao", "nh"),
            ("tbl_2_s", "shi'jie", "sj"),
            ("tbl_2_z", "zi'hao", "zh"),
        ];
        for ((table, key, code), length) in entries.iter().zip(&lengths) {
            transaction
                .execute_batch(&format!(
                    "CREATE TABLE {table}(key TEXT,jp TEXT,value TEXT,weight INTEGER)"
                ))
                .unwrap();
            for index in 0..*length {
                transaction
                    .execute(
                        &format!("INSERT INTO {table} VALUES(?1,?2,?3,?4)"),
                        (key, code, format!("合成{table}-{index}"), index as i64),
                    )
                    .unwrap();
            }
        }
        transaction.commit().unwrap();
        drop(connection);
        let input: Vec<_> = entries[..lengths.len()]
            .iter()
            .map(|entry| entry.2.to_owned())
            .collect();
        for limit in [64, 512, usize::MAX] {
            let run = |original: bool| {
                measure(|| {
                    let database = PinyinDatabase::open(&path);
                    let result = if original {
                        frozen_query_per_table(&database, &input, limit)
                    } else {
                        database.query_jianpin_codes_per_table(&input, limit)
                    };
                    drop(database);
                    result
                })
            };
            let (old, before) = run(true);
            let (new, after) = run(false);
            eprintln!(
                "不均匀简拼 {lengths:?} limit={limit}: 容量 {}→{}, 旧 {before:?}, 新 {after:?}",
                old.capacity(),
                new.capacity()
            );
            assert_eq!(new, old);
            assert_eq!(new.capacity(), old.capacity());
            assert!(after.allocations <= before.allocations);
            assert!(after.peak_bytes <= before.peak_bytes);
            assert_eq!(after.remaining_bytes, before.remaining_bytes);
            assert_eq!(before.minimum_bytes, 0);
            assert_eq!(after.minimum_bytes, 0);
        }
    }
}

#[test]
fn cold_sparse_and_empty_queries_have_lower_peak_and_same_owned_output() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成甲", 10)]);
    let _ = intact_pinyin_set();
    for input in [codes(&["nh"]), codes(&["nz"])] {
        let run = |original: bool| {
            measure(|| {
                let database = PinyinDatabase::open(&path);
                let result = if original {
                    frozen_query_per_table(&database, &input, 512)
                } else {
                    database.query_jianpin_codes_per_table(&input, 512)
                };
                drop(database);
                result
            })
        };
        let (old, before) = run(true);
        let (new, after) = run(false);
        assert_eq!(new, old);
        assert_eq!(after.allocations + 1, before.allocations);
        assert!(after.peak_bytes < before.peak_bytes);
        assert_eq!(before.minimum_bytes, 0);
        assert_eq!(after.minimum_bytes, 0);
        if new.is_empty() {
            assert_eq!(after.remaining_bytes, 0);
        } else {
            assert_eq!(after.remaining_bytes, before.remaining_bytes);
        }
        eprintln!("简拼冷查询命中 {}: 旧 {before:?}, 新 {after:?}", new.len());
    }
}

#[test]
fn sqlite_first_step_failure_keeps_no_aggregate_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE VIEW tbl_2_n AS SELECT 'ni' || char(39) || 'hao' AS key,'nh' AS jp,'合成错误' AS value,abs(-9223372036854775808) AS weight;").unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(&jianpin_batch_sql("tbl_2_n", 1, 64))
            .unwrap();
        assert!(statement.query(["nh"]).unwrap().next().is_err());
    }
    let rows = database.query_jianpin_codes_per_table(&codes(&["nh"]), 64);
    assert!(rows.is_empty());
    assert_eq!(rows.capacity(), 0);
}

#[test]
fn buffered_rows_preserve_sqlite_field_conversion_and_output_ownership() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE tbl_2_n(key,jp,value,weight); INSERT INTO tbl_2_n VALUES('合成键','nh',NULL,NULL), (123,'nh',x'ff',1.5), (NULL,'nh',456,-5);").unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    let input = codes(&["nh"]);
    let rows = database.query_jianpin_codes_per_table(&input, usize::MAX);
    assert_eq!(rows, frozen_query_per_table(&database, &input, usize::MAX));
    drop(input);
    drop(database);
    assert_eq!(rows[0].key, "123");
    assert_eq!(rows[0].value, "�");
    assert_eq!(rows[0].weight, 1);
    assert_eq!(rows[1].value, "");
    assert_eq!(rows[2].key, "");
    assert_eq!(rows[2].value, "456");
}

#[test]
fn early_returns_keep_zero_capacity_and_original_allocation_budget() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    let closed = PinyinDatabase::open(&directory.path().join("missing.db"));
    for (database, input, limit) in [
        (&database, codes(&[]), 64),
        (&database, codes(&["nh"]), 0),
        (&closed, codes(&["nh"]), 64),
    ] {
        let (old, before) = count(|| frozen_query_per_table(database, &input, limit));
        let (new, after) = count(|| database.query_jianpin_codes_per_table(&input, limit));
        assert_eq!(new, old);
        assert_eq!(new.capacity(), 0);
        assert_eq!(after, before);
    }
}
