use super::*;
use crate::dictionary::fixtures::pinyin_db;

#[test]
fn initial_miss_keeps_no_dictionary_row_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_1_n", "ni", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    assert!(database.is_open());
    let result = database.query_initial("nu", 1000);
    assert!(result.is_empty());
    assert_eq!(result.capacity(), 0);
}

// 冻结 de6c73b254415f87e6a726335554ab71fed045f1 的两个完整查询正文。
fn original_rows(
    database: &PinyinDatabase,
    sql: &str,
    params: impl rusqlite::Params,
    capacity: Option<usize>,
) -> Vec<DictRow> {
    let Some(connection) = &database.connection else {
        return Vec::new();
    };
    let Ok(mut statement) = connection.prepare_cached(sql) else {
        return Vec::new();
    };
    let Ok(mut rows) = statement.query(params) else {
        return Vec::new();
    };
    let mut result = capacity.map_or_else(Vec::new, Vec::with_capacity);
    while let Ok(Some(row)) = rows.next() {
        match dict_row(row) {
            Ok(item) => result.push(item),
            Err(_) => break,
        }
    }
    result
}

fn original_initial(database: &PinyinDatabase, prefix: &str, limit: usize) -> Vec<DictRow> {
    let Some(&first) = prefix.as_bytes().first() else {
        return Vec::new();
    };
    if !first.is_ascii_lowercase() || limit == 0 {
        return Vec::new();
    }
    let sql = initial_sql(first, sql_limit(limit));
    // 保留原查询入口的上界计算。
    let upper_bound = key_prefix_upper_bound(prefix);
    original_rows(
        database,
        &sql,
        [prefix, upper_bound.as_str()],
        query_capacity(limit),
    )
}

use crate::ime::personal_rerank::allocations::{count, measure};

fn compare_rows_hot(database: &PinyinDatabase, sql: &str, capacity: Option<usize>, saved: usize) {
    // 热查询只测次数，缓存键替换会释放区间前已有的 Arc，不能据此称绝对堆峰值。
    drop(original_rows(database, sql, (), capacity));
    drop(database.rows(sql, (), capacity));
    let (old, old_count) = count(|| original_rows(database, sql, (), capacity));
    let (new, new_count) = count(|| database.rows(sql, (), capacity));
    assert_eq!(new, old, "sql={sql}, capacity={capacity:?}");
    assert_eq!(
        new_count + saved,
        old_count,
        "sql={sql}, capacity={capacity:?}"
    );
    if new.is_empty() {
        assert_eq!(new.capacity(), 0);
    } else {
        assert_eq!(new.capacity(), old.capacity());
    }
}

fn compare_initial_hot(database: &PinyinDatabase, prefix: &str, limit: usize, saved: usize) {
    drop(original_initial(database, prefix, limit));
    drop(database.query_initial(prefix, limit));
    let (old, old_count) = count(|| original_initial(database, prefix, limit));
    let (new, new_count) = count(|| database.query_initial(prefix, limit));
    assert_eq!(new, old, "prefix={prefix:?}, limit={limit}");
    assert_eq!(
        new_count + saved,
        old_count,
        "prefix={prefix:?}, limit={limit}"
    );
    if new.is_empty() {
        assert_eq!(new.capacity(), 0);
    } else {
        assert_eq!(new.capacity(), old.capacity());
    }
}

#[test]
fn row_hints_preserve_fields_order_and_natural_growth() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", -20),
            ("tbl_1_n", "nin", "合成乙", i64::from(i32::MAX) + 42),
            ("tbl_1_n", "niu", "合成丙", 20),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let sql = "SELECT key,value,weight FROM tbl_1_n ORDER BY weight DESC";
    for capacity in [None, Some(0), Some(1), Some(5), Some(1000)] {
        compare_rows_hot(&database, sql, capacity, 0);
        compare_rows_hot(
            &database,
            "SELECT key,value,weight FROM tbl_1_n WHERE key='missing'",
            capacity,
            usize::from(capacity.is_some_and(|value| value > 0)),
        );
    }
    let result = database.rows(sql, (), Some(1000));
    assert_eq!(
        result,
        [
            DictRow {
                key: "nin".into(),
                value: "合成乙".into(),
                weight: i64::from(i32::MAX) + 42
            },
            DictRow {
                key: "niu".into(),
                value: "合成丙".into(),
                weight: 20
            },
            DictRow {
                key: "ni".into(),
                value: "合成甲".into(),
                weight: -20
            },
        ]
    );
    for limit in [1, 2, 5, 1000] {
        compare_initial_hot(&database, "n", limit, 0);
        compare_initial_hot(&database, "nu", limit, 1);
    }
}

#[test]
fn dense_initial_pages_keep_the_original_reservation_count() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_1_n", "ni", "合成原点", 0)]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1024 {
        let suffix = [
            b'a' + (index / 676) as u8,
            b'a' + ((index / 26) % 26) as u8,
            b'a' + (index % 26) as u8,
        ];
        let key = format!("n{}", std::str::from_utf8(&suffix).unwrap());
        let value = format!("合成{index}");
        transaction
            .execute(
                "INSERT INTO tbl_1_n(key,jp,value,weight) VALUES(?1,'n',?2,?3)",
                (&key, &value, index as i64),
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    for limit in [1, 5, 1000] {
        compare_initial_hot(&database, "n", limit, 0);
        assert_eq!(database.query_initial("n", limit).len(), limit);
    }
    // 无界查询继续走 None，不把 SQL 的 i32 截断上限当候选容量。
    assert_eq!(
        query_capacity(i32::MAX as usize - 1),
        Some(i32::MAX as usize - 1)
    );
    for limit in [i32::MAX as usize, usize::MAX] {
        assert_eq!(query_capacity(limit), None);
        compare_initial_hot(&database, "n", limit, 0);
        compare_initial_hot(&database, "nu", limit, 0);
    }
}

#[test]
fn null_and_non_text_fields_keep_their_existing_conversions() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    let sql = "SELECT NULL,NULL,NULL UNION ALL SELECT 42,X'FF61',' 7 ' UNION ALL SELECT 3.5,'合成文本',4.9";
    compare_rows_hot(&database, sql, Some(5), 0);
    let result = database.rows(sql, (), Some(5));
    assert_eq!(
        result,
        [
            DictRow {
                key: "".into(),
                value: "".into(),
                weight: 0
            },
            DictRow {
                key: "42".into(),
                value: "�a".into(),
                weight: 7
            },
            DictRow {
                key: "3.5".into(),
                value: "合成文本".into(),
                weight: 4
            },
        ]
    );
}

#[test]
fn failures_before_binding_do_not_save_a_nonexistent_reservation() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    let prepare_failure = "SELECT key,value,weight FROM missing_table";
    assert!(database
        .connection
        .as_ref()
        .unwrap()
        .prepare_cached(prepare_failure)
        .is_err());
    compare_rows_hot(&database, prepare_failure, Some(1000), 0);
    let binding_failure = "SELECT 'ni','合成',10 WHERE ?1=1";
    assert!(database
        .connection
        .as_ref()
        .unwrap()
        .prepare_cached(binding_failure)
        .unwrap()
        .query(())
        .is_err());
    compare_rows_hot(&database, binding_failure, Some(1000), 0);
    for prefix in ["", "N", "合成"] {
        compare_initial_hot(&database, prefix, 1000, 0);
    }
    compare_initial_hot(&database, "n", 0, 0);
    let closed = PinyinDatabase::open(Path::new(""));
    compare_rows_hot(&closed, "SELECT 'ni','合成',10", Some(1000), 0);
    compare_initial_hot(&closed, "n", 1000, 0);
}

#[test]
fn first_step_and_first_parse_failures_skip_unused_row_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    let step_failure = "SELECT 'ni','合成',abs(-9223372036854775808)";
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(step_failure)
            .unwrap();
        let mut rows = statement.query(()).unwrap();
        assert!(rows.next().is_err());
    }
    let parse_failure = "SELECT 'ni','合成'";
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(parse_failure)
            .unwrap();
        let mut rows = statement.query(()).unwrap();
        assert!(dict_row(rows.next().unwrap().unwrap()).is_err());
    }
    for capacity in [None, Some(0), Some(1), Some(1000)] {
        let saved = usize::from(capacity.is_some_and(|value| value > 0));
        compare_rows_hot(&database, step_failure, capacity, saved);
        compare_rows_hot(&database, parse_failure, capacity, saved);
    }
}

#[test]
fn later_step_failure_preserves_the_partial_page() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    // 内置 abs 的整数溢出发生在第二个 UNION 分支，无需函数注册或故障注入特性。
    let sql = "SELECT 'ni','合成甲',17 UNION ALL SELECT 'nu','合成乙',abs(-9223372036854775808)";
    let expected = DictRow {
        key: "ni".into(),
        value: "合成甲".into(),
        weight: 17,
    };
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(sql)
            .unwrap();
        let mut rows = statement.query(()).unwrap();
        assert_eq!(dict_row(rows.next().unwrap().unwrap()).unwrap(), expected);
        assert!(rows.next().is_err());
    }
    for capacity in [None, Some(0), Some(1), Some(1000)] {
        compare_rows_hot(&database, sql, capacity, 0);
        assert_eq!(
            database.rows(sql, (), capacity).as_slice(),
            std::slice::from_ref(&expected)
        );
    }
}

#[test]
fn cold_initial_miss_has_no_limit_sized_returned_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_1_n", "ni", "合成甲", 10)]);
    // 打开、PRAGMA 缓存、主查询缓存和析构都在区间内；只借用外部 fixture 路径。
    let cold = |limit, original| {
        measure(|| {
            let database = PinyinDatabase::open(&path);
            let result = if original {
                original_initial(&database, "nu", limit)
            } else {
                database.query_initial("nu", limit)
            };
            drop(database);
            result
        })
    };
    let (old, old_heap) = cold(1000, true);
    let (new, new_heap) = cold(1000, false);
    assert_eq!(new, old);
    assert_eq!(new.capacity(), 0);
    assert_eq!(old_heap.minimum_bytes, 0);
    assert_eq!(new_heap.minimum_bytes, 0);
    assert_eq!(
        old_heap.remaining_bytes,
        (old.capacity() * size_of::<DictRow>()) as i128
    );
    assert_eq!(new_heap.remaining_bytes, 0);
    assert_eq!(new_heap.allocations + 1, old_heap.allocations);
    assert!(old_heap.peak_bytes > new_heap.peak_bytes);
    // SQL 上限使用同位数，避免把字面量位数变化算成行容器容量变化。
    let (_, larger_heap) = cold(9999, false);
    assert_eq!(larger_heap.minimum_bytes, 0);
    assert_eq!(larger_heap.remaining_bytes, 0);
    assert_eq!(larger_heap.peak_bytes, new_heap.peak_bytes);
    eprintln!(
        "DictRow={} old={old_heap:?} new={new_heap:?}",
        size_of::<DictRow>()
    );
}
