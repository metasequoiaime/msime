use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 c6c9bc4b06970c13dad1c9a0f28d22a796f028f9 的完整按键分组查询正文，仅转接数据库接收者。
fn original_query(
    database: &PinyinDatabase,
    keys: &[String],
    per_key_limit: usize,
) -> HashMap<String, Vec<DictRow>> {
    let mut result: HashMap<String, Vec<DictRow>> = HashMap::with_capacity(keys.len());
    if database.connection.is_none() || keys.is_empty() || per_key_limit == 0 {
        return result;
    }
    let mut keys_by_table: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut seen = (keys.len() > SMALL_QUERY_KEY_BATCH).then(|| HashSet::with_capacity(keys.len()));
    for (index, key) in keys.iter().enumerate() {
        let segments = split_segments(key);
        if !query_key_is_new(keys, index, &mut seen)
            || !has_only_complete_pinyin_segments(&segments)
        {
            continue;
        }
        let Some(table) = build_table_name(&segments) else {
            continue;
        };
        if segments.len() == 1 {
            let rows = database.rows(
                &exact_sql(&table, sql_limit(per_key_limit)),
                [key.as_str()],
                query_capacity(per_key_limit),
            );
            if !rows.is_empty() {
                result.insert(key.clone(), rows);
            }
            continue;
        }
        keys_by_table.entry(table).or_default().push(key.clone());
    }
    for (table, table_keys) in &keys_by_table {
        // 按权重读取，每个键最先出现的是它的最高权重行。
        for row in database.batch_rows(table, table_keys, usize::MAX) {
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
fn missed_key_batch_keeps_no_result_map_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_1_n", "ni", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    let keys = vec!["nu".to_owned(); 96];
    let result = database.query_exact_keys_per_key(&keys, 2);
    assert!(result.is_empty());
    assert_eq!(result.capacity(), 0);
}

fn keys(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

// 当前生产先去重再拆分；旧空表基线仍在每次重复键上创建音节，单独计这部分新增收益。
fn duplicate_split_allocations(keys: &[String]) -> usize {
    keys.iter()
        .enumerate()
        .filter(|(index, key)| keys[..*index].contains(key))
        .map(|(_, key)| count(|| split_segments(key)).1)
        .sum()
}

fn compare_hot(database: &PinyinDatabase, keys: &[String], limit: usize, saved: usize) {
    let saved = saved
        + if database.connection.is_some() && limit > 0 {
            duplicate_split_allocations(keys)
        } else {
            0
        };
    // 热缓存会替换并释放区间前的键，仅比较次数，不称绝对堆峰值。
    drop(original_query(database, keys, limit));
    drop(database.query_exact_keys_per_key(keys, limit));
    let (old, old_count) = count(|| original_query(database, keys, limit));
    let (new, new_count) = count(|| database.query_exact_keys_per_key(keys, limit));
    // `HashMap` 的迭代顺序未定义，逐键比较内容及各键行的顺序。
    assert_eq!(new, old);
    assert_eq!(new_count + saved, old_count);
    if new.is_empty() {
        assert_eq!(new.capacity(), 0);
    } else {
        assert_eq!(new.capacity(), old.capacity());
        for (key, rows) in &new {
            assert_eq!(rows.capacity(), old[key].capacity());
        }
    }
}

#[test]
fn single_and_multi_syllable_hits_preserve_fields_order_capacity_and_count() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 300),
            ("tbl_1_n", "ni", "合成乙", -20),
            ("tbl_2_n", "ni'hao", "合成大权", i64::from(i32::MAX) + 17),
            ("tbl_2_n", "ni'hao", "合成等权甲", 50),
            ("tbl_2_n", "ni'hao", "合成等权乙", 50),
            ("tbl_2_s", "shi'jie", "合成跨表", 70),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for batch in [
        keys(&["nu", "ni"]),
        keys(&["nu", "ni'hao"]),
        keys(&["nan'hao", "shi'jie"]),
        keys(&["nu", "ni", "ni'hao", "ni'hao", "shi'jie", "n'h", "", "Ni"]),
    ] {
        for limit in [1, 2, 5, 1000] {
            compare_hot(&database, &batch, limit, 0);
        }
    }
    let result = database.query_exact_keys_per_key(&keys(&["ni", "ni'hao", "shi'jie"]), 2);
    assert_eq!(result["ni"][0].value, "合成甲");
    assert_eq!(result["ni"][1].weight, -20);
    assert_eq!(result["ni'hao"][0].key, "ni'hao");
    assert_eq!(result["ni'hao"][0].weight, i64::from(i32::MAX) + 17);
    assert_eq!(result["ni'hao"][1].value, "合成等权甲");
    assert_eq!(result["shi'jie"][0].value, "合成跨表");
}

#[test]
fn closed_zero_limit_empty_and_invalid_batches_keep_no_result_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    let closed = PinyinDatabase::open(Path::new(""));
    for length in [0, 1, 64, 65, 96, 129] {
        let batch = vec!["ni".to_owned(); length];
        let saved = usize::from(length > 0);
        compare_hot(&database, &batch, 0, saved);
        compare_hot(&closed, &batch, 2, saved);
    }
    for batch in [Vec::new(), keys(&["", "Ni", "n'h", "合成"])] {
        let saved = usize::from(!batch.is_empty());
        compare_hot(&database, &batch, 2, saved);
    }
}

#[test]
fn valid_misses_and_missing_tables_save_one_map_at_batch_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 10),
            ("tbl_2_n", "ni'hao", "合成乙", 20),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for length in [1, 64, 65, 96, 129] {
        for missed in ["nu", "nan'hao", "qi'hao"] {
            let batch = vec![missed.to_owned(); length];
            compare_hot(&database, &batch, 1, 1);
            compare_hot(&database, &batch, 1000, 1);
        }
    }
}

#[test]
fn single_syllable_unbounded_limits_keep_natural_row_growth() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 10),
            ("tbl_1_n", "ni", "合成乙", 5),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for limit in [i32::MAX as usize, usize::MAX] {
        compare_hot(&database, &keys(&["ni", "ni"]), limit, 0);
        compare_hot(&database, &keys(&["nu"]), limit, 1);
        assert_eq!(
            database.query_exact_keys_per_key(&keys(&["ni"]), limit)["ni"].len(),
            2
        );
    }
}

#[test]
fn dense_many_key_pages_preserve_reservation_and_per_key_limit() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成原点", 0)]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    let mut batch = Vec::new();
    for first in ["ni", "nan", "nu", "nin", "nai", "nei", "na", "neng", "nang"] {
        for second in [
            "hao", "men", "ma", "mo", "min", "miao", "mi", "mai", "mu", "man", "mei", "meng",
            "mie", "ming", "mian",
        ] {
            let key = format!("{first}'{second}");
            transaction
                .execute(
                    "INSERT INTO tbl_2_n(key,jp,value,weight) VALUES(?1,'nh',?2,?3)",
                    (&key, format!("合成键{}", batch.len()), batch.len() as i64),
                )
                .unwrap();
            batch.push(key);
        }
    }
    for index in 0..1024 {
        transaction.execute("INSERT INTO tbl_2_n(key,jp,value,weight) VALUES('ni' || char(39) || 'hao','nh',?1,?2)",
            (format!("合成行{index}"), 10000 + index as i64)).unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    for length in [1, 64, 65, 96, 129] {
        for limit in [1, 2, 12] {
            compare_hot(&database, &batch[..length], limit, 0);
            let result = database.query_exact_keys_per_key(&batch[..length], limit);
            assert_eq!(result.len(), length);
            assert_eq!(result["ni'hao"].len(), limit);
        }
    }
    compare_hot(&database, &batch[..1], 1000, 0);
    assert_eq!(
        database.query_exact_keys_per_key(&batch[..1], 1000)["ni'hao"].len(),
        1000
    );
}

#[test]
fn first_step_error_returns_an_empty_unallocated_map() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE VIEW tbl_2_n AS SELECT 'ni' || char(39) || 'hao' AS key,'nh' AS jp,'合成故障' AS value,abs(-9223372036854775808) AS weight;").unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    let batch = keys(&["ni'hao"]);
    {
        let sql = batch_sql("tbl_2_n", 1, sql_limit(usize::MAX));
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(&sql)
            .unwrap();
        let mut rows = statement.query(params_from_iter(&batch)).unwrap();
        assert!(rows.next().is_err());
    }
    compare_hot(&database, &batch, 2, 1);
}

#[test]
fn cold_missed_batch_returns_no_reserved_hash_table() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_1_n", "ni", "合成甲", 10)]);
    let batch = vec!["nu".to_owned(); 96];
    // 区间外预热进程级音节表与线程级随机哈希状态，冷边界只覆盖数据库和本次查询。
    let _ = intact_pinyin_set();
    drop(HashMap::<String, Vec<DictRow>>::new());
    let cold = |original| {
        measure(|| {
            let database = PinyinDatabase::open(&path);
            let result = if original {
                original_query(&database, &batch, 2)
            } else {
                database.query_exact_keys_per_key(&batch, 2)
            };
            drop(database);
            result
        })
    };
    let (old, old_heap) = cold(true);
    let (new, new_heap) = cold(false);
    assert_eq!(new, old);
    assert_eq!(new.capacity(), 0);
    assert_eq!(old_heap.minimum_bytes, 0);
    assert_eq!(new_heap.minimum_bytes, 0);
    assert_eq!(new_heap.remaining_bytes, 0);
    assert_eq!(
        new_heap.allocations + 1 + duplicate_split_allocations(&batch),
        old_heap.allocations
    );
    assert!(new_heap.peak_bytes < old_heap.peak_bytes);
    // 表包含桶和控制字节，直接计量同类型预留，不用容量乘元素大小猜物理布局。
    let (reserved, reserved_heap) =
        measure(|| HashMap::<String, Vec<DictRow>>::with_capacity(batch.len()));
    assert_eq!(reserved.capacity(), old.capacity());
    assert_eq!(reserved_heap.allocations, 1);
    assert_eq!(reserved_heap.minimum_bytes, 0);
    assert_eq!(old_heap.remaining_bytes, reserved_heap.remaining_bytes);
    assert!(old_heap.remaining_bytes > 0);
    eprintln!("old={old_heap:?} new={new_heap:?} map={reserved_heap:?}");
}
