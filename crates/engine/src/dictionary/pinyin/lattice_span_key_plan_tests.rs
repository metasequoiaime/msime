use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 `6db2285ff51cfca1580002026485b1ca25fef839` 的完整跨度与精确分段查询，仅转接接收者、旧查询调用和限额参数名。
fn original_query(database: &PinyinDatabase, span: &[String], limit: usize) -> Vec<DictRow> {
    let normalized: Vec<String> = span
        .iter()
        .map(|syllable| canonical_lattice_syllable(syllable).to_owned())
        .collect();
    original_exact_query(database, &[normalized], limit)
}

fn original_exact_query(
    database: &PinyinDatabase,
    segmentations: &[Vec<String>],
    limit: usize,
) -> Vec<DictRow> {
    if database.connection.is_none() || segmentations.is_empty() || limit == 0 {
        return Vec::new();
    }
    let keys_by_table = exact_segmentations_by_table(segmentations);
    let mut rows = Vec::new();
    for (table, keys) in &keys_by_table {
        let page = database.batch_rows(table, keys, limit);
        if !page.is_empty() && rows.capacity() == 0 {
            rows.reserve_exact(segmentations.len().saturating_mul(limit));
        }
        rows.extend(page);
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
    rows
}

fn span(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn complete_span_skips_owned_syllables_and_single_table_groups() {
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
    let input = span(&["ni", "hao"]);
    drop(original_query(&database, &input, 2));
    drop(database.query_lattice_span(&input, 2));
    let (old, old_count) = count(|| original_query(&database, &input, 2));
    let (new, new_count) = count(|| database.query_lattice_span(&input, 2));
    assert_eq!(new, old);
    assert_eq!(new.capacity(), old.capacity());
    assert_eq!(new_count + 6, old_count);
}

fn assert_results(new: &[DictRow], old: &[DictRow]) {
    assert_eq!(new, old);
    for (new, old) in new.iter().zip(old) {
        assert_eq!(new.key.capacity(), old.key.capacity());
        assert_eq!(new.value.capacity(), old.value.capacity());
    }
}

fn compare_hot(database: &PinyinDatabase, input: &[String], limit: usize, saved: usize) {
    drop(original_query(database, input, limit));
    drop(database.query_lattice_span(input, limit));
    let (old, old_count) = count(|| original_query(database, input, limit));
    let (new, new_count) = count(|| database.query_lattice_span(input, limit));
    assert_results(&new, &old);
    assert_eq!(new.capacity(), old.capacity());
    let saved = saved + usize::from(!new.is_empty());
    assert_eq!(
        new_count + saved,
        old_count,
        "input={input:?}, limit={limit}"
    );
}

#[test]
fn aliases_exact_keys_weights_and_table_boundaries_keep_fields_and_capacity() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_j", "ju", "合成甲", 10),
            ("tbl_1_q", "qu", "合成甲", 10),
            ("tbl_1_x", "xu", "合成甲", 10),
            ("tbl_1_y", "yu", "合成甲", 10),
            ("tbl_1_j", "jue", "合成甲", 10),
            ("tbl_1_q", "que", "合成甲", 10),
            ("tbl_1_x", "xue", "合成甲", 10),
            ("tbl_1_y", "yue", "合成甲", 10),
            ("tbl_1_l", "lve", "合成甲", 10),
            ("tbl_1_n", "nve", "合成甲", 10),
            ("tbl_2_g", "gun'qi", "合成大权", i64::from(i32::MAX) + 19),
            ("tbl_2_g", "gun'qi", "合成等权", 30),
            ("tbl_2_g", "gun'qi", "合成等权", 30),
            ("tbl_2_g", "gun'qi", "合成负权", -10),
            ("tbl_2_g", "gun'qiu", "合成邻键", 999_999),
            ("tbl_2_j", "ju'lve", "合成别名", 50),
            ("tbl_7_n", "ni'hao'ma'shi'jie'men'ma", "合成七段", 10),
            (
                "tbl_others_n",
                "ni'hao'ma'shi'jie'men'ma'ni",
                "合成八段",
                10,
            ),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for alias in [
        "jv", "qv", "xv", "yv", "jve", "qve", "xve", "yve", "lue", "nue",
    ] {
        for limit in [1, 2, 12, 1000] {
            compare_hot(&database, &span(&[alias]), limit, 4);
        }
    }
    for (input, saved) in [
        (span(&["gun", "qi"]), 5),
        (span(&["jv", "lue"]), 5),
        (span(&["ni", "hao", "ma", "shi", "jie", "men", "ma"]), 10),
        (
            span(&["ni", "hao", "ma", "shi", "jie", "men", "ma", "ni"]),
            11,
        ),
    ] {
        for limit in [1, 2, 12, 1000] {
            compare_hot(&database, &input, limit, saved);
        }
    }
    let input = span(&["gun", "qi"]);
    let result = database.query_lattice_span(&input, 12);
    drop(input);
    assert_eq!(result.len(), 4);
    assert_eq!(result[0].weight, i64::from(i32::MAX) + 19);
    assert_eq!(result[1].value, result[2].value);
    assert_eq!(result[3].weight, -10);
    assert!(result.iter().all(|row| row.key == "gun'qi"));
    let input = span(&["jv", "lue"]);
    let normalized_rows = database.query_lattice_span(&input, 12);
    drop(input);
    assert_eq!(normalized_rows[0].key, "ju'lve");
}

#[test]
fn guards_invalid_segments_and_empty_pages_keep_zero_return_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    let closed = PinyinDatabase::open(&directory.path().join("synthetic-missing.db"));
    compare_hot(&closed, &span(&["ni", "hao"]), 12, 3);
    compare_hot(&database, &span(&["ni", "hao"]), 0, 3);
    compare_hot(&database, &[], 12, 0);
    for (input, saved) in [
        (span(&[""]), 1),
        (span(&["", "ni"]), 2),
        (span(&["ni", ""]), 2),
        (span(&["ni", "h"]), 3),
        (span(&["Ni", "hao"]), 3),
        (span(&["合成", "hao"]), 3),
        (span(&["ni'hao"]), 2),
        (span(&["jvn"]), 2),
        (span(&["i"]), 2),
    ] {
        compare_hot(&database, &input, 12, saved);
        let (rows, allocations) = count(|| database.query_lattice_span(&input, 12));
        assert!(rows.is_empty());
        assert_eq!(rows.capacity(), 0);
        assert_eq!(allocations, 0);
    }
    for input in [
        span(&["ni", "ma"]),
        span(&["shi", "jie"]),
        span(&["ju"]),
        vec!["ni".to_owned(); 1025],
    ] {
        let saved = input.len() + 3;
        for limit in [1, 1000, usize::MAX] {
            compare_hot(&database, &input, limit, saved);
            assert_eq!(database.query_lattice_span(&input, limit).capacity(), 0);
        }
    }
}

#[test]
fn first_step_errors_and_coerced_fields_keep_existing_sqlite_semantics() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE VIEW tbl_2_n AS SELECT 'ni' || char(39) || 'hao' AS key,'nh' AS jp,'合成错误' AS value,abs(-9223372036854775808) AS weight; CREATE VIEW tbl_2_j AS SELECT 'ju' || char(39) || 'lve' AS key,'jl' AS jp,'合成高权' AS value,30 AS weight UNION ALL SELECT 'ju' || char(39) || 'lve','jl',NULL,NULL;").unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    {
        let mut statement = database
            .connection
            .as_ref()
            .unwrap()
            .prepare_cached(&batch_sql("tbl_2_n", 1, sql_limit(12)))
            .unwrap();
        assert!(statement.query(["ni'hao"]).unwrap().next().is_err());
    }
    for limit in [1, 2, 12] {
        compare_hot(&database, &span(&["ni", "hao"]), limit, 5);
        compare_hot(&database, &span(&["jv", "lue"]), limit, 5);
    }
    let rows = database.query_lattice_span(&span(&["jv", "lue"]), 12);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].value, "");
    assert_eq!(rows[1].weight, 0);
}

#[test]
fn cold_queries_reduce_planning_storage_without_changing_return_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_n", "ni'hao", "合成原点", 0)]);
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..128 {
        transaction.execute("INSERT INTO tbl_2_n(key,jp,value,weight) VALUES('ni' || char(39) || 'hao','nh',?1,?2)",
            (format!("合成行{index}"), 1000 + index as i64)).unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    // 区间外预热进程级音节表和当前线程随机状态；数据库与返回结果均在区间内构造。
    let _ = intact_pinyin_set();
    drop(HashMap::<String, Vec<DictRow>>::new());
    for (case, input, saved) in [
        ("dense-129", span(&["ni", "hao"]), 5),
        ("empty-page", span(&["ni", "ma"]), 5),
        ("missing-table", span(&["shi", "jie"]), 5),
        ("invalid", span(&["ni", "h"]), 3),
        ("empty", Vec::new(), 0),
    ] {
        for limit in [1, 12, 1000] {
            let cold = |original| {
                measure(|| {
                    let database = PinyinDatabase::open(&path);
                    let rows = if original {
                        original_query(&database, &input, limit)
                    } else {
                        database.query_lattice_span(&input, limit)
                    };
                    drop(database);
                    rows
                })
            };
            let (old, old_heap) = cold(true);
            let (new, new_heap) = cold(false);
            assert_results(&new, &old);
            assert_eq!(new.capacity(), old.capacity());
            assert_eq!(
                new_heap.allocations + saved + usize::from(!new.is_empty()),
                old_heap.allocations
            );
            assert_eq!(new_heap.remaining_bytes, old_heap.remaining_bytes);
            assert_eq!(
                new_heap.remaining_bytes,
                (new.capacity() * size_of::<DictRow>()
                    + new
                        .iter()
                        .map(|row| row.key.capacity() + row.value.capacity())
                        .sum::<usize>()) as i128
            );
            assert_eq!(old_heap.minimum_bytes, 0);
            assert_eq!(new_heap.minimum_bytes, 0);
            assert!(new_heap.peak_bytes <= old_heap.peak_bytes);
            eprintln!("case={case} limit={limit} old={old_heap:?} new={new_heap:?}");
        }
    }
}
