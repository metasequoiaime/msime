use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 `4981ab7481f7e49bfc1cd038c1fa1fdd54345636` 的完整入口，仅转接接收者。
fn frozen_query(
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
        // 按权重读取，每个键最先出现的是它的最高权重行；已有槽只借用键查找。
        for row in rows {
            if let Some(slot) = result.get_mut(row.key.as_str()) {
                if slot.len() < per_key_limit {
                    slot.push(row);
                }
            } else {
                let key = row.key.clone();
                let mut slot = Vec::with_capacity(per_key_limit);
                slot.push(row);
                result.insert(key, slot);
            }
        }
    }
    result
}

fn keys(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn multi_syllable_unbounded_limit_returns_all_real_rows() {
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
    for limit in [i32::MAX as usize, usize::MAX] {
        let result = database.query_exact_keys_per_key(&keys(&["ni'hao"]), limit);
        assert_eq!(
            result["ni'hao"]
                .iter()
                .map(|row| row.value.as_str())
                .collect::<Vec<_>>(),
            ["合成甲", "合成乙", "合成丙"]
        );
    }
}

#[test]
fn finite_limits_keep_slot_capacity_while_unbounded_slots_follow_actual_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_2_n", "ni'hao", "合成甲", 30),
            ("tbl_2_n", "ni'hao", "合成乙", 20),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let finite = database.query_exact_keys_per_key(&keys(&["ni'hao"]), 5);
    assert_eq!(finite["ni'hao"].len(), 2);
    assert_eq!(finite["ni'hao"].capacity(), 5);
    let unbounded = database.query_exact_keys_per_key(&keys(&["ni'hao"]), usize::MAX);
    assert_eq!(unbounded["ni'hao"].len(), 2);
    assert!(unbounded["ni'hao"].capacity() < 5);
}

#[test]
fn unbounded_multi_key_results_preserve_keys_and_rows_after_input_is_dropped() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_2_n", "ni'hao", "合成甲", 30),
            ("tbl_2_n", "ni'men", "合成乙", 20),
            ("tbl_2_s", "shi'jie", "合成丙", 10),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let input = keys(&["ni'hao", "ni'men", "shi'jie"]);
    let result = database.query_exact_keys_per_key(&input, usize::MAX);
    drop(input);
    assert_eq!(result.len(), 3);
    assert_eq!(result["ni'hao"][0].key, "ni'hao");
    assert_eq!(result["ni'men"][0].value, "合成乙");
    assert_eq!(result["shi'jie"][0].weight, 10);
}

#[test]
fn empty_and_invalid_batches_keep_zero_result_storage_for_unbounded_limits() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    for input in [keys(&[]), keys(&["nan'hao"]), keys(&["Ni'hao"])] {
        let result = database.query_exact_keys_per_key(&input, usize::MAX);
        assert!(result.is_empty());
        assert_eq!(result.capacity(), 0);
    }
}

#[test]
fn unbounded_slot_cold_measurement_has_no_impossible_reservation() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成甲", 10)]);
    let input = keys(&["ni'hao"]);
    let _ = intact_pinyin_set();
    drop(HashMap::<String, Vec<DictRow>>::new());
    let cold = || {
        measure(|| {
            let database = PinyinDatabase::open(&path);
            let result = database.query_exact_keys_per_key(&input, usize::MAX);
            drop(database);
            result
        })
    };
    let (result, usage) = cold();
    assert_eq!(result["ni'hao"].len(), 1);
    assert!(usage.remaining_bytes > 0);
    assert_eq!(usage.minimum_bytes, 0);
    eprintln!("无界多音节槽冷查询: {usage:?}");
}

#[test]
fn finite_queries_keep_frozen_fields_slot_capacity_and_allocation_count() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_n", "ni", "合成单音节", 30),
            ("tbl_2_n", "ni'hao", "合成同值", 50),
            ("tbl_2_n", "ni'hao", "合成同值", 50),
            ("tbl_2_n", "ni'hao", "合成负权", -20),
            ("tbl_2_n", "ni'men", "合成大权", i64::MAX),
            ("tbl_2_s", "shi'jie", "合成跨表", 50),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for length in [1, 64, 65, 96, 129] {
        let mut input = vec!["ni'hao".to_owned(); length];
        input.extend(keys(&["ni", "ni'men", "shi'jie", "nan'hao", "n'h", ""]));
        for limit in [0, 1, 2, 5, 128, 1000] {
            drop(frozen_query(&database, &input, limit));
            drop(database.query_exact_keys_per_key(&input, limit));
            let (old, before) = count(|| frozen_query(&database, &input, limit));
            let (new, after) = count(|| database.query_exact_keys_per_key(&input, limit));
            assert_eq!(new, old);
            assert_eq!(new.capacity(), old.capacity());
            assert_eq!(after, before);
            for (key, rows) in &new {
                assert_eq!(rows.capacity(), old[key].capacity());
            }
        }
    }
}

#[test]
fn dense_unbounded_groups_preserve_all_rows_without_hidden_truncation() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成原点", -10)]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1025 {
        transaction
            .execute(
                "INSERT INTO tbl_2_n(key,jp,value,weight) VALUES(?1,'nh',?2,?3)",
                ("ni'hao", format!("合成{}", index / 2), index as i64),
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    let input = keys(&["ni'hao", "ni'hao"]);
    let expected = frozen_query(&database, &input, 2048);
    for limit in [i32::MAX as usize, usize::MAX] {
        let rows = database.query_exact_keys_per_key(&input, limit);
        assert_eq!(rows, expected);
        assert_eq!(rows["ni'hao"].len(), 1026);
        assert!(rows["ni'hao"].capacity() <= 2048);
    }
}

#[test]
fn sqlite_first_step_error_keeps_empty_unbounded_result() {
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
            .prepare_cached(&batch_sql("tbl_2_n", 1, sql_limit(usize::MAX)))
            .unwrap();
        assert!(statement.query(["ni'hao"]).unwrap().next().is_err());
    }
    let rows = database.query_exact_keys_per_key(&keys(&["ni'hao"]), usize::MAX);
    assert!(rows.is_empty());
    assert_eq!(rows.capacity(), 0);
}
