use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 `f6fbf3ece1afb6853dd8a8a1a4007f8015cfacf8` 的完整词条方法，仅适配接收者和中文注释。
fn original_find_weight(database: &PinyinDatabase, key: &str, value: &str) -> Option<i64> {
    let connection = database.connection.as_ref()?;
    let table = build_table_name(&split_segments(key))?;
    // 缺表或首步失败沿用参考实现的未找到结果（QD:490-497）。
    let mut statement = connection.prepare_cached(&find_weight_sql(&table)).ok()?;
    let mut rows = statement.query((key, value)).ok()?;
    let row = rows.next().ok()??;
    column_i64(row, 0).ok()
}

fn original_insert_word(database: &PinyinDatabase, key: &str, value: &str) -> Result<()> {
    let connection = database.writable()?;
    let segments = split_segments(key);
    let table =
        build_table_name(&segments).ok_or_else(|| EngineError::invalid(INVALID_DICTIONARY_KEY))?;
    let jp = segments_to_jianpin(&segments);
    connection
        .prepare_cached(&insert_word_sql(&table))?
        .execute((key, jp.as_str(), value, INSERTED_WEIGHT))?;
    Ok(())
}

#[test]
fn weight_lookup_does_not_copy_key_segments() {
    let dir = tempfile::tempdir().unwrap();
    let path = pinyin_db(dir.path(), &[("tbl_2_n", "ni'hao", "合成甲", 17)]);
    let database = PinyinDatabase::open(&path);
    original_find_weight(&database, "ni'hao", "合成甲");
    database.find_weight("ni'hao", "合成甲");
    let (old, old_count) = count(|| original_find_weight(&database, "ni'hao", "合成甲"));
    let (new, new_count) = count(|| database.find_weight("ni'hao", "合成甲"));
    assert_eq!(new, old);
    eprintln!("lookup old={old_count} new={new_count}");
    assert_eq!(new_count + 3, old_count);
}

#[test]
fn word_insertion_does_not_copy_key_segments() {
    let old_dir = tempfile::tempdir().unwrap();
    let new_dir = tempfile::tempdir().unwrap();
    let seed = [("tbl_2_n", "ni'hao", "合成种子", 1)];
    let old_db = PinyinDatabase::open(&pinyin_db(old_dir.path(), &seed));
    let new_db = PinyinDatabase::open(&pinyin_db(new_dir.path(), &seed));
    original_insert_word(&old_db, "ni'hao", "合成甲").unwrap();
    new_db.insert_word("ni'hao", "合成甲").unwrap();
    let (old, old_count) = count(|| original_insert_word(&old_db, "ni'hao", "合成甲"));
    let (new, new_count) = count(|| new_db.insert_word("ni'hao", "合成甲"));
    old.unwrap();
    new.unwrap();
    eprintln!("insert old={old_count} new={new_count}");
    assert_eq!(new_count + 3, old_count);
}

fn split_cost(key: &str) -> usize {
    count(|| split_segments(key)).1
}

fn signature(result: &Result<()>) -> Option<(std::mem::Discriminant<EngineError>, String)> {
    result
        .as_ref()
        .err()
        .map(|error| (std::mem::discriminant(error), error.to_string()))
}

fn stored_rows(database: &PinyinDatabase, table: &str) -> Vec<(String, String, String, i64)> {
    let mut statement = database
        .connection
        .as_ref()
        .unwrap()
        .prepare(&format!(
            "SELECT key,jp,value,weight FROM {table} ORDER BY rowid"
        ))
        .unwrap();
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn word_methods_preserve_raw_keys_empty_parts_unicode_and_overflow_tables() {
    let old_dir = tempfile::tempdir().unwrap();
    let new_dir = tempfile::tempdir().unwrap();
    let mut keys: Vec<String> = [
        "ni",
        "ni'hao",
        "ni''hao",
        "ni'",
        "ni'''",
        "n'猫'🦊",
        "n猫",
        "ni'h",
        "ia",
        "i'v",
        "n'合成'",
        "ni'quote;--",
        "",
        "'ni",
        "Ni'hao",
        "猫'ni",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    keys.extend([
        ["ni"; 7].join("'"),
        ["ni"; 8].join("'"),
        vec!["ni"; 1025].join("'"),
    ]);
    let tables: Vec<_> = keys
        .iter()
        .map(|key| build_table_name(&split_segments(key)))
        .collect();
    let seed: Vec<_> = keys
        .iter()
        .zip(&tables)
        .filter_map(|(key, table)| {
            table
                .as_ref()
                .map(|table| (table.as_str(), key.as_str(), "合成种子", 17))
        })
        .collect();
    let old_db = PinyinDatabase::open(&pinyin_db(old_dir.path(), &seed));
    let new_db = PinyinDatabase::open(&pinyin_db(new_dir.path(), &seed));
    for (key, table) in keys.iter().zip(&tables) {
        for value in ["合成种子", "合成缺词"] {
            original_find_weight(&old_db, key, value);
            new_db.find_weight(key, value);
            let (old, old_count) = count(|| original_find_weight(&old_db, key, value));
            let (new, new_count) = count(|| new_db.find_weight(key, value));
            assert_eq!(new, old, "key={key:?}");
            assert_eq!(new_count + split_cost(key), old_count, "key={key:?}");
        }
        for _ in 0..2 {
            let old = original_insert_word(&old_db, key, "合成'词🦊");
            let new = new_db.insert_word(key, "合成'词🦊");
            assert_eq!(signature(&new), signature(&old), "key={key:?}");
        }
        let (old, old_count) = count(|| original_insert_word(&old_db, key, "合成'词🦊"));
        let (new, new_count) = count(|| new_db.insert_word(key, "合成'词🦊"));
        assert_eq!(signature(&new), signature(&old));
        assert_eq!(new_count + split_cost(key), old_count, "key={key:?}");
        if table.is_some() {
            assert!(new.is_ok());
        }
    }
    let mut tables: Vec<_> = tables.into_iter().flatten().collect();
    tables.sort();
    tables.dedup();
    for table in tables {
        assert_eq!(stored_rows(&new_db, &table), stored_rows(&old_db, &table));
    }
    let rows = stored_rows(&new_db, "tbl_3_n");
    assert!(rows
        .iter()
        .any(|row| row.0 == "n'猫'🦊" && row.1 == "n猫🦊"));
    assert!(rows
        .iter()
        .any(|row| row.0 == "ni''hao" && row.1 == "nh" && row.3 == INSERTED_WEIGHT));
    assert!(stored_rows(&new_db, "tbl_others_n")
        .iter()
        .any(|row| row.0 == keys[keys.len() - 1]));
}

#[test]
fn database_guards_and_sqlite_failures_keep_result_and_error_order() {
    let old_dir = tempfile::tempdir().unwrap();
    let new_dir = tempfile::tempdir().unwrap();
    let seed = [("tbl_2_n", "ni'hao", "合成种子", 1)];
    let old_db = PinyinDatabase::open(&pinyin_db(old_dir.path(), &seed));
    let new_db = PinyinDatabase::open(&pinyin_db(new_dir.path(), &seed));
    for (key, expected_invalid) in [
        ("", true),
        ("'ni", true),
        ("Ni'hao", true),
        ("shi'jie", false),
        ("ia", false),
    ] {
        let old = original_insert_word(&old_db, key, "合成甲");
        let new = new_db.insert_word(key, "合成甲");
        assert_eq!(signature(&new), signature(&old));
        assert!(matches!(new, Err(EngineError::InvalidArgument(_))) == expected_invalid);
        assert!(new.is_err());
        assert_eq!(
            new_db.find_weight(key, "合成甲"),
            original_find_weight(&old_db, key, "合成甲")
        );
    }
    for database in [&old_db, &new_db] {
        database
            .connection
            .as_ref()
            .unwrap()
            .execute_batch("PRAGMA query_only=ON")
            .unwrap();
    }
    original_insert_word(&old_db, "ni'hao", "合成甲").unwrap_err();
    new_db.insert_word("ni'hao", "合成甲").unwrap_err();
    let (old, old_count) = count(|| original_insert_word(&old_db, "ni'hao", "合成甲"));
    let (new, new_count) = count(|| new_db.insert_word("ni'hao", "合成甲"));
    assert_eq!(signature(&new), signature(&old));
    assert!(matches!(new, Err(EngineError::Sqlite(_))));
    assert_eq!(new_count + 3, old_count);
    assert_eq!(
        stored_rows(&new_db, "tbl_2_n"),
        stored_rows(&old_db, "tbl_2_n")
    );
    let closed = PinyinDatabase::open(&new_dir.path().join("synthetic-missing.db"));
    assert!(closed.connection.is_none());
    for key in ["ni'hao", "", "Ni"] {
        assert_eq!(closed.find_weight(key, "合成甲"), None);
        let (old, old_count) = count(|| original_insert_word(&closed, key, "合成甲"));
        let (new, new_count) = count(|| closed.insert_word(key, "合成甲"));
        assert_eq!(signature(&new), signature(&old));
        assert!(matches!(new, Err(EngineError::Failed(_))));
        assert_eq!(new_count, old_count);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = pinyin_db(dir.path(), &[]);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE VIEW tbl_2_n AS SELECT 'ni' || char(39) || 'hao' AS key,'合成甲' AS value,abs(-9223372036854775808) AS weight; CREATE VIEW tbl_1_n AS SELECT 'ni' AS key,'合成甲' AS value,NULL AS weight UNION ALL SELECT 'nu','合成乙','37';").unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(&find_weight_sql("tbl_2_n"))
            .unwrap();
        assert!(statement
            .query(("ni'hao", "合成甲"))
            .unwrap()
            .next()
            .is_err());
    }
    for (key, value) in [("ni'hao", "合成甲"), ("ni", "合成甲"), ("nu", "合成乙")] {
        assert_eq!(
            database.find_weight(key, value),
            original_find_weight(&database, key, value)
        );
    }
    assert_eq!(database.find_weight("ni'hao", "合成甲"), None);
    assert_eq!(database.find_weight("ni", "合成甲"), Some(0));
    assert_eq!(database.find_weight("nu", "合成乙"), Some(37));
}

#[test]
fn cold_weight_lookup_reduces_planning_storage_and_releases_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = pinyin_db(dir.path(), &[("tbl_2_n", "ni'hao", "合成甲", 17)]);
    // 区间外初始化线程随机状态；打开、查询和显式析构都在计量区间内。
    drop(HashMap::<String, Vec<DictRow>>::new());
    for (case, key, value) in [
        ("hit", "ni'hao", "合成甲"),
        ("missing-value", "ni'hao", "合成乙"),
        ("missing-table", "shi'jie", "合成甲"),
        ("empty-part", "ni''hao", "合成甲"),
        ("unicode", "n'猫", "合成甲"),
        ("invalid-initial", "Ni'hao", "合成甲"),
        ("empty", "", "合成甲"),
    ] {
        let cold = |old| {
            measure(|| {
                let database = PinyinDatabase::open(&path);
                let result = if old {
                    original_find_weight(&database, key, value)
                } else {
                    database.find_weight(key, value)
                };
                drop(database);
                result
            })
        };
        let (old, old_heap) = cold(true);
        let (new, new_heap) = cold(false);
        assert_eq!(new, old);
        assert_eq!(new_heap.allocations + split_cost(key), old_heap.allocations);
        assert_eq!(old_heap.remaining_bytes, 0);
        assert_eq!(new_heap.remaining_bytes, 0);
        assert_eq!(old_heap.minimum_bytes, 0);
        assert_eq!(new_heap.minimum_bytes, 0);
        assert!(new_heap.peak_bytes <= old_heap.peak_bytes);
        eprintln!("case={case} old={old_heap:?} new={new_heap:?}");
    }
}
