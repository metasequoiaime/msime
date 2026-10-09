use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

// 冻结 96fd6b7f9ca636078250e5cc1b8e528b7f4986a9 的两个完整聚合查询正文，仅转接数据库接收者。
fn original_query_longer_phrases(
    database: &PinyinDatabase,
    segments: &[String],
    extra_syllables: usize,
    limit: usize,
) -> Vec<DictRow> {
    if database.connection.is_none()
        || segments.len() < 2
        || extra_syllables == 0
        || limit == 0
        || !has_only_complete_pinyin_segments(segments)
    {
        return Vec::new();
    }
    // 保留原查询的完整音节续接前缀。
    let prefix = longer_phrase_prefix(segments);
    let upper_bound = key_prefix_upper_bound(&prefix);
    let initial = segments[0].as_bytes()[0];
    let mut rows = Vec::with_capacity(extra_syllables.saturating_mul(limit));
    for extra in 1..=extra_syllables {
        let Some(table) = quanpin_table(segments.len() + extra, initial) else {
            continue;
        };
        rows.extend(database.rows(
            &range_sql(&table, sql_limit(limit)),
            [prefix.as_str(), upper_bound.as_str()],
            query_capacity(limit),
        ));
    }
    deduplicate_by_value(&mut rows);
    rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
    rows.truncate(limit);
    rows
}

fn original_query_exact_segmentations_keyed_flat(
    database: &PinyinDatabase,
    segmentations: &[Vec<String>],
    limit: usize,
) -> Vec<DictRow> {
    if database.connection.is_none() || segmentations.is_empty() || limit == 0 {
        return Vec::new();
    }
    let keys_by_table = exact_segmentations_by_table(segmentations);
    let mut rows = Vec::with_capacity(segmentations.len().saturating_mul(limit));
    for (table, keys) in &keys_by_table {
        rows.extend(database.batch_rows(table, keys, limit));
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
    rows
}

fn segments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

#[test]
fn longer_empty_aggregate_keeps_no_row_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[("tbl_3_p", "ping'guo'shu", "合成甲", 10)],
    );
    let database = PinyinDatabase::open(&path);
    let rows = database.query_longer_phrases(&segments(&["ping", "gan"]), 3, 1000);
    assert!(rows.is_empty());
    assert_eq!(rows.capacity(), 0);
}

#[test]
fn exact_empty_aggregate_keeps_no_row_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_p", "ping'guo", "合成甲", 10)]);
    let database = PinyinDatabase::open(&path);
    let rows = database.query_exact_segmentations_keyed_flat(&[segments(&["ping", "gan"])], 1000);
    assert!(rows.is_empty());
    assert_eq!(rows.capacity(), 0);
}

fn compare_hot(
    original: impl Fn() -> Vec<DictRow>,
    current: impl Fn() -> Vec<DictRow>,
    saved: usize,
) {
    // 缓存键替换的析构跨越热测量边界，热查询仅比较分配次数。
    drop(original());
    drop(current());
    let (old, old_count) = count(original);
    let (new, new_count) = count(current);
    assert_eq!(new, old);
    assert_eq!(new_count + saved, old_count);
    if new.is_empty() {
        assert_eq!(new.capacity(), 0);
    } else {
        assert_eq!(new.capacity(), old.capacity());
    }
}

fn compare_longer(
    database: &PinyinDatabase,
    key: &[String],
    extra: usize,
    limit: usize,
    saved: usize,
) {
    drop(original_query_longer_phrases(database, key, extra, limit));
    drop(database.query_longer_phrases(key, extra, limit));
    let (old, old_count) = count(|| original_query_longer_phrases(database, key, extra, limit));
    let (new, new_count) = count(|| database.query_longer_phrases(key, extra, limit));
    assert_eq!(new, old);
    assert!(
        new_count + saved <= old_count + extra,
        "按页扩容的分配次数应有界：新={}，旧={}，extra={extra}",
        new_count,
        old_count
    );
    if new.is_empty() {
        assert_eq!(new.capacity(), 0);
    } else {
        assert!(
            new.capacity() <= old.capacity(),
            "按页预留不得超过旧上界：新={}，旧={}，extra={extra}，limit={limit}",
            new.capacity(),
            old.capacity()
        );
    }
}

fn compare_exact(database: &PinyinDatabase, keys: &[Vec<String>], limit: usize, saved: usize) {
    compare_hot(
        || original_query_exact_segmentations_keyed_flat(database, keys, limit),
        || database.query_exact_segmentations_keyed_flat(keys, limit),
        saved,
    );
}

#[test]
fn sparse_longer_pages_do_not_reserve_all_future_limit_slots() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[("tbl_3_p", "ping'guo'shu", "稀疏一行", 10)],
    );
    let database = PinyinDatabase::open(&path);
    let rows = database.query_longer_phrases(&segments(&["ping", "guo"]), 3, 1000);
    assert_eq!(
        rows.iter()
            .map(|row| row.value.as_str())
            .collect::<Vec<_>>(),
        ["稀疏一行"]
    );
    assert!(
        rows.capacity() <= 3,
        "稀疏页不应预留 3000 个槽位：{}",
        rows.capacity()
    );

    let key = segments(&["ping", "guo"]);
    let (old, old_heap) = measure(|| original_query_longer_phrases(&database, &key, 3, 1000));
    let (new, new_heap) = measure(|| database.query_longer_phrases(&key, 3, 1000));
    assert_eq!(new, old);
    assert_eq!(old.capacity(), 3000);
    assert!(new.capacity() <= 3);
    assert!(new_heap.peak_bytes < old_heap.peak_bytes);
    assert_eq!(new_heap.minimum_bytes, 0);
    assert_eq!(old_heap.minimum_bytes, 0);
}

#[test]
fn aggregate_hits_keep_fields_order_capacity_and_reservation_count() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_p", "ping", "合成同值", 50),
            ("tbl_2_p", "ping'guo", "合成同值", 50),
            ("tbl_2_p", "ping'guo", "合成低权", -20),
            ("tbl_3_p", "ping'guo'shu", "合成首次", 50),
            ("tbl_3_p", "ping'guo'yuan", "合成等权", 50),
            ("tbl_4_p", "ping'guo'shu'ye", "合成首次", 900),
            (
                "tbl_4_p",
                "ping'guo'shou'ji",
                "合成最高",
                i64::from(i32::MAX) + 7,
            ),
            ("tbl_5_p", "ping'guo'shou'ji'ke", "合成尾页", -20),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let key = segments(&["ping", "guo"]);
    let keys = [
        segments(&["nan"]),
        segments(&["ping"]),
        key.clone(),
        key.clone(),
        segments(&["p", "guo"]),
    ];
    for limit in [1, 2, 12, 1000] {
        for extra in [1, 2, 3] {
            compare_longer(&database, &key, extra, limit, 0);
        }
        compare_exact(&database, &keys, limit, 0);
    }
    let rows = database.query_longer_phrases(&key, 3, 12);
    let values: Vec<_> = rows.iter().map(|row| row.value.as_str()).collect();
    assert_eq!(values, ["合成最高", "合成首次", "合成等权", "合成尾页"]);
    assert_eq!(rows[1].key, "ping'guo'shu");
    assert_eq!(rows[1].weight, 50);
    let rows = database.query_exact_segmentations_keyed_flat(&keys, 12);
    let keyed: Vec<_> = rows
        .iter()
        .map(|row| (row.key.as_str(), row.value.as_str()))
        .collect();
    assert_eq!(
        keyed,
        [
            ("ping", "合成同值"),
            ("ping'guo", "合成同值"),
            ("ping'guo", "合成低权")
        ]
    );
}

#[test]
fn empty_pages_missing_tables_and_invalid_batches_save_only_the_aggregate() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[("tbl_3_p", "ping'guo'shu", "合成甲", 10)],
    );
    let database = PinyinDatabase::open(&path);
    for limit in [1, 12, 1000] {
        compare_longer(&database, &segments(&["ping", "gan"]), 3, limit, 1);
        compare_longer(&database, &segments(&["nan", "guo"]), 3, limit, 1);
        compare_exact(&database, &[segments(&["ping", "gan", "shu"])], limit, 1);
        compare_exact(&database, &[segments(&["nan", "guo"])], limit, 1);
        compare_exact(&database, &[segments(&["p", "guo"]), Vec::new()], limit, 1);
    }
    // 第一层/第一张表为空，随后命中时仍预留原聚合容量一次。
    let late = segments(&["ping", "guo", "shu"]);
    compare_exact(&database, &[segments(&["nan"]), late], 12, 0);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE tbl_4_p(key TEXT,jp TEXT,value TEXT,weight INTEGER); INSERT INTO tbl_4_p VALUES('ping' || char(39) || 'gan' || char(39) || 'shou' || char(39) || 'ji','pgsj','合成后页',17);").unwrap();
    drop(connection);
    compare_longer(&database, &segments(&["ping", "gan"]), 3, 12, 0);
}

#[test]
fn guards_keep_the_original_zero_allocation_boundary() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[]);
    let database = PinyinDatabase::open(&path);
    for key in [Vec::new(), segments(&["ping"]), segments(&["p", "guo"])] {
        compare_longer(&database, &key, 3, 1000, 0);
    }
    let key = segments(&["ping", "guo"]);
    compare_longer(&database, &key, 0, 1000, 0);
    compare_longer(&database, &key, 3, 0, 0);
    compare_exact(&database, &[], 1000, 0);
    compare_exact(&database, std::slice::from_ref(&key), 0, 0);
    let closed = PinyinDatabase::open(Path::new(""));
    compare_longer(&closed, &key, 3, 1000, 0);
    compare_exact(&closed, &[key], 1000, 0);
}

#[test]
fn dense_aggregate_pages_preserve_sort_dedup_and_capacity() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[("tbl_3_p", "ping'guo'shu", "合成原点", 0)],
    );
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..1024 {
        transaction.execute(
            "INSERT INTO tbl_3_p(key,jp,value,weight) VALUES('ping' || char(39) || 'guo' || char(39) || 'shu','pgs',?1,?2)",
            (format!("合成{}", index / 2), index as i64),
        ).unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    let key = segments(&["ping", "guo"]);
    let exact = segments(&["ping", "guo", "shu"]);
    for limit in [1, 12, 1000] {
        compare_longer(&database, &key, 3, limit, 0);
        compare_exact(&database, std::slice::from_ref(&exact), limit, 0);
        assert_eq!(
            database
                .query_exact_segmentations_keyed_flat(std::slice::from_ref(&exact), limit)
                .len(),
            limit
        );
    }
}

#[test]
fn cold_misses_remove_the_limit_sized_aggregate_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_3_p", "ping'guo'shu", "合成甲", 10),
            ("tbl_4_p", "ping'guo'shou'ji", "合成乙", 20),
            ("tbl_5_p", "ping'guo'shou'ji'ke", "合成丙", 30),
        ],
    );
    let key = segments(&["ping", "gan"]);
    let keys = [
        segments(&["ping", "gan", "shu"]),
        segments(&["ping", "gan", "shou", "ji"]),
    ];
    // 进程级不可变音节表在区间外预热，冷边界只覆盖数据库及本次查询。
    assert!(has_only_complete_pinyin_segments(&key));
    for longer in [true, false] {
        // 打开、PRAGMA 缓存、各表主查询与析构均在区间内，输入路径和键只借用。
        let cold = |limit, original| {
            measure(|| {
                let database = PinyinDatabase::open(&path);
                let result = match (longer, original) {
                    (true, true) => original_query_longer_phrases(&database, &key, 3, limit),
                    (true, false) => database.query_longer_phrases(&key, 3, limit),
                    (false, true) => {
                        original_query_exact_segmentations_keyed_flat(&database, &keys, limit)
                    }
                    (false, false) => database.query_exact_segmentations_keyed_flat(&keys, limit),
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
        assert!(new_heap.peak_bytes < old_heap.peak_bytes);
        let (_, larger_heap) = cold(9999, false);
        assert_eq!(larger_heap.minimum_bytes, 0);
        assert_eq!(larger_heap.remaining_bytes, 0);
        assert_eq!(larger_heap.peak_bytes, new_heap.peak_bytes);
        eprintln!("longer={longer} old={old_heap:?} new={new_heap:?}");
    }
}
