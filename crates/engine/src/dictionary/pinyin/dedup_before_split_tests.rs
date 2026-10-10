use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 `41bb27db0e6473b09fdda3e383782c00e07fc530` 的完整按键分组查询正文，仅转接数据库接收者。
fn original_query(
    database: &PinyinDatabase,
    keys: &[String],
    per_key_limit: usize,
) -> HashMap<String, Vec<DictRow>> {
    let mut result: HashMap<String, Vec<DictRow>> = HashMap::new();
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
                if result.capacity() == 0 {
                    result.reserve(keys.len());
                }
                result.insert(key.clone(), rows);
            }
            continue;
        }
        keys_by_table.entry(table).or_default().push(key.clone());
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
fn duplicate_single_syllables_skip_owned_segment_allocations() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_1_n", "ni", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    let batch = vec!["nu".to_owned(); 96];
    drop(original_query(&database, &batch, 2));
    drop(database.query_exact_keys_per_key(&batch, 2));
    let (old, old_count) = count(|| original_query(&database, &batch, 2));
    let (new, new_count) = count(|| database.query_exact_keys_per_key(&batch, 2));
    assert_eq!(new, old);
    assert!(new.is_empty());
    assert_eq!(
        new_count + 190 + unique_split_allocations(&batch),
        old_count
    );
}

fn keys(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn assert_results(new: &HashMap<String, Vec<DictRow>>, old: &HashMap<String, Vec<DictRow>>) {
    // `HashMap` 无固定迭代顺序；逐键比较行字段、顺序与存储容量。
    assert_eq!(new, old);
    assert_eq!(new.capacity(), old.capacity());
    for (key, rows) in new {
        assert_eq!(rows.capacity(), old[key].capacity());
    }
}

// 旧去重基线仍为每个首次键创建音节，单独计入后续借用规划节省，保留冻结正文。
fn unique_split_allocations(keys: &[String]) -> usize {
    keys.iter()
        .enumerate()
        .filter(|(index, key)| !keys[..*index].contains(key))
        .map(|(_, key)| count(|| split_segments(key)).1)
        .sum()
}

fn compare_hot(database: &PinyinDatabase, batch: &[String], limit: usize, saved: usize) {
    let saved =
        saved + result_key_reuse_tests::repeated_row_key_allocations(database, batch, limit);
    let saved = saved
        + if database.connection.is_some() && limit > 0 {
            unique_split_allocations(batch)
                + borrowed_key_groups_tests::owned_group_allocations(batch)
        } else {
            0
        };
    // 缓存会释放区间前的键，此边界只比较分配次数。
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
fn duplicate_valid_invalid_and_empty_keys_preserve_boundary_batches() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 300),
            ("tbl_1_n", "ni", "合成负权", -20),
            ("tbl_2_n", "ni'hao", "合成大权", i64::from(i32::MAX) + 17),
            ("tbl_2_n", "ni'hao", "合成等权甲", 50),
            ("tbl_2_n", "ni'hao", "合成等权乙", 50),
            ("tbl_2_s", "shi'jie", "合成跨表", 70),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for length in [1, 64, 65, 96, 129] {
        for (key, per_duplicate) in [
            ("ni", 2),
            ("nu", 2),
            ("ni'hao", 3),
            ("shi'jie", 3),
            ("nan'hao", 3),
            ("qi'hao", 3),
            ("ni'hao'ma'shi'jie", 6),
            ("", 0),
            ("Ni", 2),
            ("合成", 2),
            ("ni''hao", 3),
            ("n'h", 3),
            ("'", 1),
            ("'ni'", 2),
        ] {
            let batch = vec![key.to_owned(); length];
            for limit in [1, 2, 5, 1000] {
                compare_hot(&database, &batch, limit, (length - 1) * per_duplicate);
            }
        }
    }
    let batch = keys(&[
        "ni'hao", "Ni", "nu", "ni", "ni'hao", "shi'jie", "n'h", "ni", "", "Ni", "n'h",
    ]);
    compare_hot(&database, &batch, 2, 10);
    let result = database.query_exact_keys_per_key(&batch, 2);
    assert_eq!(result["ni"][0].value, "合成甲");
    assert_eq!(result["ni"][1].weight, -20);
    assert_eq!(result["ni'hao"][0].weight, i64::from(i32::MAX) + 17);
    assert_eq!(result["ni'hao"][1].value, "合成等权甲");
    assert_eq!(result["shi'jie"][0].value, "合成跨表");
}

#[test]
fn unique_batches_keep_allocation_count_and_result_capacity() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成原点", 0)]);
    let connection = Connection::open(&path).unwrap();
    let mut batch = Vec::new();
    for first in ["ni", "nan", "nu", "nin", "nai", "nei", "na", "neng", "nang"] {
        for second in [
            "hao", "men", "ma", "mo", "min", "miao", "mi", "mai", "mu", "man", "mei", "meng",
            "mie", "ming", "mian",
        ] {
            let key = format!("{first}'{second}");
            connection
                .execute(
                    "INSERT INTO tbl_2_n(key,jp,value,weight) VALUES(?1,'nh',?2,?3)",
                    (&key, format!("合成键{}", batch.len()), batch.len() as i64),
                )
                .unwrap();
            batch.push(key);
        }
    }
    drop(connection);
    let database = PinyinDatabase::open(&path);
    for length in [1, 64, 65, 96, 129] {
        for limit in [1, 2, 5] {
            compare_hot(&database, &batch[..length], limit, 0);
            assert_eq!(
                database
                    .query_exact_keys_per_key(&batch[..length], limit)
                    .len(),
                length
            );
        }
    }
    let invalid: Vec<String> = (0..129).map(|index| format!("合成{index}")).collect();
    compare_hot(&database, &invalid, 2, 0);
    compare_hot(&database, &keys(&["ni", "nu", "nan", "nin", "nai"]), 2, 0);
}

#[test]
fn parameter_guards_still_skip_all_key_work() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    let closed = PinyinDatabase::open(Path::new(""));
    for length in [0, 1, 64, 65, 96, 129] {
        let batch = vec!["ni'hao".to_owned(); length];
        compare_hot(&database, &batch, 0, 0);
        compare_hot(&closed, &batch, 2, 0);
    }
    compare_hot(&database, &[], 2, 0);
}

#[test]
fn duplicate_single_syllables_keep_unbounded_row_limits() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 10),
            ("tbl_1_n", "ni", "合成乙", 5),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let batch = vec!["ni".to_owned(); 65];
    for limit in [i32::MAX as usize, usize::MAX] {
        compare_hot(&database, &batch, limit, 128);
        assert_eq!(
            database.query_exact_keys_per_key(&batch, limit)["ni"].len(),
            2
        );
    }
}

#[test]
fn duplicate_first_step_errors_preserve_empty_results() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
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
    for length in [64, 65, 96] {
        let batch = vec!["ni'hao".to_owned(); length];
        compare_hot(&database, &batch, 2, (length - 1) * 3);
        assert!(database.query_exact_keys_per_key(&batch, 2).is_empty());
    }
}

#[test]
fn cold_queries_save_only_duplicate_segment_allocations() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 10),
            ("tbl_2_n", "ni'hao", "合成乙", 20),
        ],
    );
    // 区间外预热进程级音节表与线程级随机哈希状态；输入与路径仅借用。
    let _ = intact_pinyin_set();
    drop(HashMap::<String, Vec<DictRow>>::new());
    for (key, saved) in [("nu", 190), ("ni", 190), ("nan'hao", 285), ("ni'hao", 285)] {
        let batch = vec![key.to_owned(); 96];
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
        assert_results(&new, &old);
        assert_eq!(
            new_heap.allocations
                + saved
                + unique_split_allocations(&batch)
                + borrowed_key_groups_tests::owned_group_allocations(&batch),
            old_heap.allocations
        );
        assert_eq!(new_heap.remaining_bytes, old_heap.remaining_bytes);
        assert_eq!(new_heap.minimum_bytes, 0);
        assert_eq!(old_heap.minimum_bytes, 0);
        assert!(new_heap.peak_bytes <= old_heap.peak_bytes);
        eprintln!("key={key} old={old_heap:?} new={new_heap:?}");
    }
}
