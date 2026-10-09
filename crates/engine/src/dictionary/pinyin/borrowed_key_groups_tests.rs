use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 `4a2343f01bca460f97f352dd2448f6e6f243f125` 的完整按键分组查询正文，仅转接数据库接收者。
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
fn unique_multi_syllable_keys_skip_group_key_clones() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    let batch: Vec<String> = ["ni'hao", "nan'hao", "nu'hao"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    drop(original_query(&database, &batch, 2));
    drop(database.query_exact_keys_per_key(&batch, 2));
    let (old, old_count) = count(|| original_query(&database, &batch, 2));
    let (new, new_count) = count(|| database.query_exact_keys_per_key(&batch, 2));
    assert_eq!(new, old);
    assert_eq!(new.len(), 1);
    assert_eq!(new_count + 3, old_count);
}

fn assert_results(new: &HashMap<String, Vec<DictRow>>, old: &HashMap<String, Vec<DictRow>>) {
    // `HashMap` 的迭代顺序未定义，逐键比较完整行字段、顺序与容量。
    assert_eq!(new, old);
    assert_eq!(new.capacity(), old.capacity());
    for (key, rows) in new {
        assert_eq!(rows.capacity(), old[key].capacity());
    }
}

fn compare_hot(database: &PinyinDatabase, batch: &[String], limit: usize, saved: usize) {
    let saved =
        saved + result_key_reuse_tests::repeated_row_key_allocations(database, batch, limit);
    // 热缓存会替换区间前已拥有的查询键，此边界只比较次数。
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

fn keys(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn many_keys() -> Vec<String> {
    let mut batch = Vec::new();
    for first in ["ni", "nan", "nu", "nin", "nai", "nei", "na", "neng", "nang"] {
        for second in [
            "hao", "men", "ma", "mo", "min", "miao", "mi", "mai", "mu", "man", "mei", "meng",
            "mie", "ming", "mian",
        ] {
            batch.push(format!("{first}'{second}"));
        }
    }
    batch
}

#[test]
fn mixed_valid_invalid_repeated_keys_keep_results_and_limits() {
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
            ("tbl_7_n", "ni'hao'ma'shi'jie'men'ma", "合成七段", 60),
            (
                "tbl_others_n",
                "ni'hao'ma'shi'jie'men'ma'ni",
                "合成八段",
                40,
            ),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for batch in [
        keys(&["nu", "ni"]),
        keys(&["nu", "ni'hao"]),
        keys(&["nan'hao", "shi'jie"]),
        keys(&[
            "", "ni''hao", "Ni", "ni", "ni'hao", "ni'hao", "shi'jie", "n'h",
        ]),
        keys(&["ni'hao'ma'shi'jie'men'ma", "ni'hao'ma'shi'jie'men'ma'ni"]),
    ] {
        let saved = owned_group_allocations(&batch);
        for limit in [1, 2, 5, 1000] {
            compare_hot(&database, &batch, limit, saved);
        }
    }
    let long = std::iter::repeat_n("ni", 1025)
        .collect::<Vec<_>>()
        .join("'");
    compare_hot(&database, &[long], 2, 1);
    for length in [1, 64, 65, 96, 129] {
        let batch: Vec<String> = (0..length)
            .map(|index| ["ni", "ni'hao", "Ni", "", "ni''hao", "shi'jie"][index % 6].to_owned())
            .collect();
        compare_hot(&database, &batch, 2, owned_group_allocations(&batch));
    }
    let result = database.query_exact_keys_per_key(&keys(&["ni", "ni'hao", "shi'jie"]), 2);
    assert_eq!(result["ni"][0].value, "合成甲");
    assert_eq!(result["ni"][1].weight, -20);
    assert_eq!(result["ni'hao"][0].weight, i64::from(i32::MAX) + 17);
    assert_eq!(result["ni'hao"][1].value, "合成等权甲");
    assert_eq!(result["shi'jie"][0].value, "合成跨表");
}

#[test]
fn unique_dense_batches_save_group_key_clones_at_batch_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成原点", 0)]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    let batch = many_keys();
    for (index, key) in batch.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO tbl_2_n(key,jp,value,weight) VALUES(?1,'nh',?2,?3)",
                (key, format!("合成键{index}"), index as i64),
            )
            .unwrap();
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
            compare_hot(&database, &batch[..length], limit, length);
            let result = database.query_exact_keys_per_key(&batch[..length], limit);
            assert_eq!(result.len(), length);
            assert_eq!(result["ni'hao"].len(), limit);
        }
    }
    compare_hot(&database, &batch[..1], 1000, 1);
    assert_eq!(
        database.query_exact_keys_per_key(&batch[..1], 1000)["ni'hao"].len(),
        1000
    );
}

#[test]
fn parameter_guards_keep_allocation_count_and_unbounded_single_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 10),
            ("tbl_1_n", "ni", "合成乙", 5),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let closed = PinyinDatabase::open(Path::new(""));
    for length in [0, 1, 64, 65, 96, 129] {
        let batch = vec!["ni'hao".to_owned(); length];
        compare_hot(&database, &batch, 0, 0);
        compare_hot(&closed, &batch, 2, 0);
    }
    compare_hot(&database, &[], 2, 0);
    let batch = keys(&["ni", "ni"]);
    for limit in [i32::MAX as usize, usize::MAX] {
        compare_hot(&database, &batch, limit, 0);
        assert_eq!(
            database.query_exact_keys_per_key(&batch, limit)["ni"].len(),
            2
        );
    }
}

#[test]
fn first_step_failure_keeps_empty_result_and_skips_group_key_clones() {
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
    compare_hot(&database, &keys(&["ni'hao", "ni'hao"]), 2, 1);
    assert!(database
        .query_exact_keys_per_key(&keys(&["ni'hao"]), 2)
        .is_empty());
}

#[test]
fn cold_unique_queries_keep_result_storage_and_save_group_key_clones() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成甲", 10),
            ("tbl_2_n", "ni'hao", "合成乙", 20),
        ],
    );
    // 区间外预热全局音节表与线程随机状态；路径与输入只借用，区间内拥有数据库全部状态。
    let _ = intact_pinyin_set();
    drop(HashMap::<String, Vec<DictRow>>::new());
    let single: Vec<String> = crate::pinyin::syllables::intact_pinyin_list()
        .iter()
        .take(96)
        .map(|value| (*value).to_owned())
        .collect();
    let many = many_keys();
    for (label, batch, saved) in [
        ("single-unique-96", single, 0),
        ("multi-unique-96", many[..96].to_vec(), 96),
        ("single-hit", keys(&["ni"]), 0),
        ("multi-hit", keys(&["ni'hao"]), 1),
        ("invalid-unique", keys(&["Ni", "ni''hao", "'ni", "合成"]), 0),
    ] {
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
            new_heap.allocations + saved,
            old_heap.allocations,
            "{label}"
        );
        assert_eq!(
            new_heap.remaining_bytes, old_heap.remaining_bytes,
            "{label}"
        );
        assert_eq!(new_heap.minimum_bytes, 0);
        assert_eq!(old_heap.minimum_bytes, 0);
        assert!(new_heap.peak_bytes <= old_heap.peak_bytes, "{label}");
        eprintln!("case={label} old={old_heap:?} new={new_heap:?}");
    }
}

// 独立用旧校验识别首次合法多音节键，区间外计量每个完整键克隆，保留各片冻结正文。
pub(super) fn owned_group_allocations(keys: &[String]) -> usize {
    keys.iter()
        .enumerate()
        .filter(|(index, key)| !keys[..*index].contains(key))
        .filter(|(_, key)| {
            let segments = split_segments(key);
            segments.len() > 1
                && has_only_complete_pinyin_segments(&segments)
                && build_table_name(&segments).is_some()
        })
        .map(|(_, key)| count(|| key.clone()).1)
        .sum()
}

// 冻结同一基线的私有批量读行正文，以原来的 `&String` 参数绑定验证泛型桥接。
fn original_batch_rows(
    database: &PinyinDatabase,
    table: &str,
    keys: &[String],
    limit: usize,
) -> Vec<DictRow> {
    if table.is_empty() || keys.is_empty() || limit == 0 {
        return Vec::new();
    }
    let sql = batch_sql(table, keys.len(), sql_limit(limit));
    database.rows(&sql, params_from_iter(keys), query_capacity(limit))
}

#[test]
fn owned_and_borrowed_batch_parameters_keep_rows_capacity_and_allocations() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_2_n", "ni'hao", "合成甲", 30),
            ("tbl_2_n", "ni'hao", "合成乙", -10),
            ("tbl_2_n", "nan'hao", "合成丙", 20),
            ("tbl_2_n", "ni\0hao", "合成零字节", 15),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for batch in [
        vec![],
        keys(&["ni'hao"]),
        keys(&["ni'hao", "nan'hao", "ni\0hao"]),
        keys(&["不存在"]),
    ] {
        let borrowed: Vec<&str> = batch.iter().map(String::as_str).collect();
        for table in ["", "tbl_2_n", "tbl_missing"] {
            for limit in [0, 1, 2, i32::MAX as usize, usize::MAX] {
                drop(original_batch_rows(&database, table, &batch, limit));
                drop(database.batch_rows(table, &batch, limit));
                drop(database.batch_rows(table, &borrowed, limit));
                let (old, old_count) =
                    count(|| original_batch_rows(&database, table, &batch, limit));
                let (owned, owned_count) = count(|| database.batch_rows(table, &batch, limit));
                let (new, new_count) = count(|| database.batch_rows(table, &borrowed, limit));
                assert_eq!(owned, old);
                assert_eq!(new, old);
                assert_eq!(owned.capacity(), old.capacity());
                assert_eq!(new.capacity(), old.capacity());
                assert_eq!(owned_count, old_count);
                assert_eq!(new_count, old_count);
            }
        }
    }
    let batch = keys(&["ni'hao", "nan'hao"]);
    let result = database.query_exact_keys_per_key(&batch, 2);
    drop(batch);
    assert_eq!(result["ni'hao"][0].value, "合成甲");
    assert_eq!(result["ni'hao"][1].weight, -10);
    assert_eq!(result["nan'hao"][0].value, "合成丙");
}
