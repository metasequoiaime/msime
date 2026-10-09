use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 `fa60fdd5a6d0a3f6460318ea389c54bc89eaad1c` 的完整按键分组查询正文，仅转接数据库接收者。
fn original_query(
    database: &PinyinDatabase,
    keys: &[String],
    per_key_limit: usize,
) -> HashMap<String, Vec<DictRow>> {
    let mut result: HashMap<String, Vec<DictRow>> = HashMap::new();
    if database.connection.is_none() || keys.is_empty() || per_key_limit == 0 {
        return result;
    }
    let mut keys_by_table: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut seen = (keys.len() > SMALL_QUERY_KEY_BATCH).then(|| HashSet::with_capacity(keys.len()));
    for (index, key) in keys.iter().enumerate() {
        if !query_key_is_new(keys, index, &mut seen) {
            continue;
        }
        let Some((table, syllables)) = complete_key_table(key) else {
            continue;
        };
        if syllables == 1 {
            let rows = database.rows(
                &exact_sql(&table, sql_limit(per_key_limit)),
                [key.as_str()],
                query_capacity(per_key_limit),
            );
            if !rows.is_empty() {
                if result.capacity() == 0 {
                    result.reserve(keys.len());
                }
                result.insert(key.clone(), rows);
            }
            continue;
        }
        keys_by_table.entry(table).or_default().push(key.as_str());
    }
    for (table, table_keys) in &keys_by_table {
        let rows = database.batch_rows(table, table_keys, usize::MAX);
        if !rows.is_empty() && result.capacity() == 0 {
            result.reserve(keys.len());
        }
        // 按权重读取，每个键最先出现的是它的最高权重行。
        for row in rows {
            let slot = result
                .entry(row.key.clone())
                .or_insert_with(|| Vec::with_capacity(per_key_limit));
            if slot.len() < per_key_limit {
                slot.push(row);
            }
        }
    }
    result
}

#[test]
fn repeated_rows_skip_key_clones_after_first_slot_even_when_full() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_2_n", "ni'hao", "合成甲", 30),
            ("tbl_2_n", "ni'hao", "合成乙", 20),
            ("tbl_2_n", "ni'hao", "合成丙", -10),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let batch = vec!["ni'hao".to_owned()];
    drop(original_query(&database, &batch, 1));
    drop(database.query_exact_keys_per_key(&batch, 1));
    let (old, old_count) = count(|| original_query(&database, &batch, 1));
    let (new, new_count) = count(|| database.query_exact_keys_per_key(&batch, 1));
    assert_eq!(new, old);
    assert_eq!(new["ni'hao"].len(), 1);
    assert_eq!(new["ni'hao"][0].value, "合成甲");
    assert_eq!(new_count + 2, old_count);
}

fn keys(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn assert_results(new: &HashMap<String, Vec<DictRow>>, old: &HashMap<String, Vec<DictRow>>) {
    assert_eq!(new, old);
    assert_eq!(new.capacity(), old.capacity());
    for (key, rows) in new {
        assert_eq!(rows.capacity(), old[key].capacity());
    }
}

// 用旧校验和未改动的 SQL 读取全部行，在区间外独立计量首次结果槽之后的键克隆。
pub(super) fn repeated_row_key_allocations(
    database: &PinyinDatabase,
    batch: &[String],
    limit: usize,
) -> usize {
    if database.connection.is_none() || batch.is_empty() || limit == 0 {
        return 0;
    }
    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut present = HashSet::new();
    for (index, key) in batch.iter().enumerate() {
        if batch[..index].contains(key) {
            continue;
        }
        let segments = split_segments(key);
        if !has_only_complete_pinyin_segments(&segments) {
            continue;
        }
        let Some(table) = build_table_name(&segments) else {
            continue;
        };
        if segments.len() == 1 {
            if !database
                .rows(
                    &exact_sql(&table, sql_limit(limit)),
                    [key.as_str()],
                    query_capacity(limit),
                )
                .is_empty()
            {
                present.insert(key.clone());
            }
        } else {
            groups.entry(table).or_default().push(key.as_str());
        }
    }
    let mut saved = 0;
    for (table, table_keys) in groups {
        for row in database.batch_rows(&table, &table_keys, usize::MAX) {
            if !present.insert(row.key.clone()) {
                saved += count(|| row.key.clone()).1;
            }
        }
    }
    saved
}

fn compare_hot(database: &PinyinDatabase, batch: &[String], limit: usize, saved: usize) {
    assert_eq!(repeated_row_key_allocations(database, batch, limit), saved);
    drop(original_query(database, batch, limit));
    drop(database.query_exact_keys_per_key(batch, limit));
    let (old, old_count) = count(|| original_query(database, batch, limit));
    let (new, new_count) = count(|| database.query_exact_keys_per_key(batch, limit));
    assert_results(&new, &old);
    assert_eq!(
        new_count + saved,
        old_count,
        "keys={} limit={limit}",
        batch.len()
    );
}

#[test]
fn mixed_tables_weights_and_duplicate_inputs_keep_all_rows_and_limits() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成单甲", 30),
            ("tbl_1_n", "ni", "合成单乙", -10),
            ("tbl_2_n", "ni'hao", "合成大权", i64::from(i32::MAX) + 17),
            ("tbl_2_n", "ni'hao", "合成等权甲", 50),
            ("tbl_2_n", "ni'hao", "合成等权乙", 50),
            ("tbl_2_s", "shi'jie", "合成跨表甲", 10),
            ("tbl_2_s", "shi'jie", "合成跨表乙", -20),
            ("tbl_7_n", "ni'hao'ma'shi'jie'men'ma", "合成七段甲", 10),
            ("tbl_7_n", "ni'hao'ma'shi'jie'men'ma", "合成七段乙", 5),
            (
                "tbl_others_n",
                "ni'hao'ma'shi'jie'men'ma'ni",
                "合成八段甲",
                10,
            ),
            (
                "tbl_others_n",
                "ni'hao'ma'shi'jie'men'ma'ni",
                "合成八段乙",
                5,
            ),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for (batch, saved) in [
        (keys(&["ni", "ni"]), 0),
        (keys(&["ni'hao", "ni'hao", "shi'jie"]), 3),
        (
            keys(&[
                "", "Ni", "ni''hao", "合成", "ni", "ni'hao", "shi'jie", "n'h",
            ]),
            3,
        ),
        (
            keys(&["ni'hao'ma'shi'jie'men'ma", "ni'hao'ma'shi'jie'men'ma'ni"]),
            2,
        ),
        (keys(&["nan'hao", "ni'hao"]), 2),
    ] {
        for limit in [1, 2, 5, 1000] {
            compare_hot(&database, &batch, limit, saved);
        }
    }
    let batch = keys(&["ni'hao", "shi'jie"]);
    let result = database.query_exact_keys_per_key(&batch, 3);
    drop(batch);
    assert_eq!(result["ni'hao"][0].weight, i64::from(i32::MAX) + 17);
    assert_eq!(result["ni'hao"][1].value, "合成等权甲");
    assert_eq!(result["ni'hao"][2].value, "合成等权乙");
    assert_eq!(result["shi'jie"][1].weight, -20);
}

#[test]
fn dense_pages_and_input_batch_boundaries_clone_only_new_result_keys() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成原点", 0)]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1025 {
        transaction.execute("INSERT INTO tbl_2_n(key,jp,value,weight) VALUES('ni' || char(39) || 'hao','nh',?1,?2)", (format!("合成行{index}"), 10000+index as i64)).unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    for length in [1, 64, 65, 96, 129] {
        let batch: Vec<String> = (0..length)
            .map(|index| ["ni'hao", "nan'hao", "ni", "Ni", "", "ni''hao"][index % 6].to_owned())
            .collect();
        for limit in [1, 2, 12, 1000] {
            compare_hot(&database, &batch, limit, 1025);
            assert_eq!(
                database.query_exact_keys_per_key(&batch, limit)["ni'hao"].len(),
                limit
            );
        }
    }
}

#[test]
fn guards_missing_tables_and_first_step_error_keep_allocation_boundary() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成单甲", 10),
            ("tbl_1_n", "ni", "合成单乙", 5),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let closed = PinyinDatabase::open(Path::new(""));
    for length in [0, 1, 64, 65, 96, 129] {
        let batch = vec!["ni'hao".to_owned(); length];
        compare_hot(&database, &batch, 0, 0);
        compare_hot(&closed, &batch, 2, 0);
        compare_hot(&database, &batch, 2, 0);
    }
    for limit in [i32::MAX as usize, usize::MAX] {
        compare_hot(&database, &keys(&["ni", "ni"]), limit, 0);
    }
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE VIEW tbl_2_n AS SELECT 'ni' || char(39) || 'hao' AS key,'nh' AS jp,'合成故障' AS value,abs(-9223372036854775808) AS weight;").unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(&batch_sql("tbl_2_n", 1, sql_limit(usize::MAX)))
            .unwrap();
        let mut rows = statement.query(["ni'hao"]).unwrap();
        assert!(rows.next().is_err());
    }
    compare_hot(&database, &keys(&["ni'hao", "ni'hao"]), 2, 0);
}

#[test]
fn cold_queries_save_only_repeated_result_key_clones() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成单甲", 30),
            ("tbl_1_n", "ni", "合成单乙", 20),
            ("tbl_2_n", "ni'hao", "合成多甲", 30),
            ("tbl_2_n", "ni'hao", "合成多乙", 20),
            ("tbl_2_n", "ni'hao", "合成多丙", -10),
            ("tbl_2_s", "shi'jie", "合成跨表甲", 30),
            ("tbl_2_s", "shi'jie", "合成跨表乙", 20),
        ],
    );
    let _ = intact_pinyin_set();
    drop(HashMap::<String, Vec<DictRow>>::new());
    for (label, batch, saved) in [
        ("single-two-rows", keys(&["ni"]), 0),
        ("multi-three-rows", keys(&["ni'hao"]), 2),
        ("cross-table-five-rows", keys(&["ni'hao", "shi'jie"]), 3),
        ("invalid", keys(&["Ni", "ni''hao", "'ni", "合成"]), 0),
        ("missing-table", keys(&["nan'hao'men"]), 0),
    ] {
        for limit in [1, 2, 5] {
            let cold = |original| {
                measure(|| {
                    let database = PinyinDatabase::open(&path);
                    let result = if original {
                        original_query(&database, &batch, limit)
                    } else {
                        database.query_exact_keys_per_key(&batch, limit)
                    };
                    drop(database);
                    result
                })
            };
            let (old, old_heap) = cold(true);
            let (new, new_heap) = cold(false);
            assert_results(&new, &old);
            assert_eq!(new_heap.allocations + saved, old_heap.allocations);
            assert_eq!(new_heap.remaining_bytes, old_heap.remaining_bytes);
            assert_eq!(old_heap.minimum_bytes, 0);
            assert_eq!(new_heap.minimum_bytes, 0);
            assert!(new_heap.peak_bytes <= old_heap.peak_bytes);
            eprintln!("case={label} limit={limit} old={old_heap:?} new={new_heap:?}");
        }
    }
}

#[test]
fn cold_dense_page_keeps_return_storage_and_skips_all_repeated_key_copies() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成原点", 0)]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1025 {
        transaction.execute("INSERT INTO tbl_2_n(key,jp,value,weight) VALUES('ni' || char(39) || 'hao','nh',?1,?2)",(format!("合成行{index}"),10000+index as i64)).unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let batch = keys(&["ni'hao"]);
    let _ = intact_pinyin_set();
    drop(HashMap::<String, Vec<DictRow>>::new());
    for limit in [1, 2, 1000] {
        let cold = |original| {
            measure(|| {
                let database = PinyinDatabase::open(&path);
                let result = if original {
                    original_query(&database, &batch, limit)
                } else {
                    database.query_exact_keys_per_key(&batch, limit)
                };
                drop(database);
                result
            })
        };
        let (old, old_heap) = cold(true);
        let (new, new_heap) = cold(false);
        assert_results(&new, &old);
        assert_eq!(new["ni'hao"].len(), limit);
        assert_eq!(new_heap.allocations + 1025, old_heap.allocations);
        assert_eq!(new_heap.remaining_bytes, old_heap.remaining_bytes);
        assert_eq!(old_heap.minimum_bytes, 0);
        assert_eq!(new_heap.minimum_bytes, 0);
        assert!(new_heap.peak_bytes <= old_heap.peak_bytes);
        eprintln!("case=dense-1026 limit={limit} old={old_heap:?} new={new_heap:?}");
    }
}

#[test]
fn coerced_weight_rows_keep_fields_and_repeated_key_savings() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE VIEW tbl_2_n AS SELECT 'ni' || char(39) || 'hao' AS key,'nh' AS jp,'合成高权' AS value,30 AS weight UNION ALL SELECT 'ni' || char(39) || 'hao','nh','合成低权',20 UNION ALL SELECT 'ni' || char(39) || 'hao','nh','合成零权',NULL;").unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(&batch_sql("tbl_2_n", 1, sql_limit(usize::MAX)))
            .unwrap();
        let mut rows = statement.query(["ni'hao"]).unwrap();
        assert_eq!(rows.next().unwrap().unwrap().get::<_, i64>(2).unwrap(), 30);
        assert_eq!(rows.next().unwrap().unwrap().get::<_, i64>(2).unwrap(), 20);
        assert!(rows.next().unwrap().unwrap().get::<_, i64>(2).is_err());
    }
    for limit in [1, 2, 5] {
        compare_hot(&database, &keys(&["ni'hao"]), limit, 2);
    }
    let result = database.query_exact_keys_per_key(&keys(&["ni'hao"]), 5);
    assert_eq!(result["ni'hao"].len(), 3);
    assert_eq!(result["ni'hao"][1].value, "合成低权");
    assert_eq!(result["ni'hao"][2].value, "合成零权");
    assert_eq!(result["ni'hao"][2].weight, 0);
}
