// 冻结 `2e79b2040406b4bfa4ee1eba21d99943be788261` 的完整聚合和直接跨度入口；仅转接接收者，移除注释并格式化。
use super::*;
use crate::dictionary::fixtures::pinyin_db;
use crate::ime::personal_rerank::allocations::{count, measure};

fn frozen_query_exact_segmentations_keyed_flat(
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

fn frozen_query_lattice_span(
    database: &PinyinDatabase,
    span: &[String],
    span_limit: usize,
) -> Vec<DictRow> {
    if database.connection.is_none() || span.is_empty() || span_limit == 0 {
        return Vec::new();
    }
    let normalized = span
        .iter()
        .map(|syllable| canonical_lattice_syllable(syllable));
    let intact = intact_pinyin_set();
    if !normalized.clone().all(|syllable| intact.contains(syllable)) {
        return Vec::new();
    }
    let initial = normalized
        .clone()
        .next()
        .and_then(|first| first.as_bytes().first());
    let Some(table) = initial.and_then(|first| quanpin_table(span.len(), *first)) else {
        return Vec::new();
    };
    let capacity = normalized.clone().map(str::len).sum::<usize>() + span.len() - 1;
    let mut key = String::with_capacity(capacity);
    for (index, syllable) in normalized.enumerate() {
        if index > 0 {
            key.push('\'');
        }
        key.push_str(syllable);
    }
    let page = database.batch_rows(&table, &[key.as_str()], span_limit);
    let mut rows = Vec::new();
    if !page.is_empty() {
        rows.reserve_exact(span_limit);
    }
    rows.extend(page);
    rows.sort_by_key(|row| std::cmp::Reverse(row.weight));
    rows
}

fn segments(key: &str) -> Vec<String> {
    key.split('\'').map(str::to_owned).collect()
}

#[test]
fn unbounded_exact_query_returns_real_sqlite_hits() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_p", "ping", "甲", 17),
            ("tbl_2_p", "ping'guo", "乙", -3),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let keys = [segments("ping"), segments("ping'guo")];
    let expected = frozen_query_exact_segmentations_keyed_flat(&database, &keys, 32);
    for limit in [i32::MAX as usize, isize::MAX as usize, usize::MAX] {
        assert_eq!(
            database.query_exact_segmentations_keyed_flat(&keys, limit),
            expected
        );
    }
}

#[test]
fn single_hit_page_reuses_storage_and_saves_exactly_one_allocation() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_2_p", "ping'guo", "合成甲", 50),
            ("tbl_2_p", "ping'guo", "合成乙", -20),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let key = segments("ping'guo");
    for limit in [1, 2, 32, 128, 1000] {
        for size in [1, 16, 17, 64, 65] {
            let mut keys = vec![key.clone(); size];
            keys.push(segments("p'guo"));
            drop(frozen_query_exact_segmentations_keyed_flat(
                &database, &keys, limit,
            ));
            drop(database.query_exact_segmentations_keyed_flat(&keys, limit));
            let (old, before) =
                count(|| frozen_query_exact_segmentations_keyed_flat(&database, &keys, limit));
            let (new, after) =
                count(|| database.query_exact_segmentations_keyed_flat(&keys, limit));
            assert_eq!(new, old);
            assert_eq!(after + 1, before);
            assert_eq!(new.capacity(), limit);
            assert_eq!(old.capacity(), keys.len() * limit);
        }
    }
}

#[test]
fn multi_table_pages_preserve_fields_stable_order_and_per_table_limits() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_p", "ping", "合成同值", 50),
            ("tbl_1_p", "ping", "合成负权", -20),
            ("tbl_2_p", "ping'guo", "合成同值", 50),
            ("tbl_2_p", "ping'guo", "合成大权", i64::from(i32::MAX) + 7),
            ("tbl_3_p", "ping'guo'shu", "合成等权", 50),
            ("tbl_3_p", "ping'guo'shu", "合成尾页", -20),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for size in [3, 16, 17, 64, 65] {
        let mut keys = vec![segments("ping"); size];
        keys.extend([
            segments("ping'guo"),
            segments("ping'guo'shu"),
            segments("nan"),
            segments("p'guo"),
            Vec::new(),
        ]);
        for limit in [1, 2, 3, 32, 128] {
            drop(frozen_query_exact_segmentations_keyed_flat(
                &database, &keys, limit,
            ));
            drop(database.query_exact_segmentations_keyed_flat(&keys, limit));
            let (old, before) =
                count(|| frozen_query_exact_segmentations_keyed_flat(&database, &keys, limit));
            let (new, after) =
                count(|| database.query_exact_segmentations_keyed_flat(&keys, limit));
            assert_eq!(new, old);
            assert_eq!(new.len(), 3 * limit.min(2));
            assert!(new.capacity() <= 3 * limit);
            assert!(after <= before + 1);
            if limit <= 2 {
                assert_eq!(after, before, "密集三表不增加分配");
                assert_eq!(new.capacity(), 3 * limit);
            }
        }
    }
    let keys = [
        segments("ping"),
        segments("ping'guo"),
        segments("ping'guo'shu"),
    ];
    let rows = database.query_exact_segmentations_keyed_flat(&keys, 32);
    assert_eq!(
        rows.iter().map(|r| r.value.as_str()).collect::<Vec<_>>(),
        [
            "合成大权",
            "合成同值",
            "合成同值",
            "合成等权",
            "合成负权",
            "合成尾页"
        ]
    );
}

#[test]
fn late_hit_short_then_full_and_empty_guards_preserve_results() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_p", "ping", "甲", 50),
            ("tbl_2_p", "ping'guo", "乙", 50),
            ("tbl_2_p", "ping'guo", "丙", 50),
        ],
    );
    let database = PinyinDatabase::open(&path);
    for keys in [
        vec![],
        vec![segments("nan")],
        vec![segments("p'guo")],
        vec![segments("nan"), segments("ping'guo")],
        vec![segments("ping"), segments("ping'guo")],
    ] {
        for limit in [0, 1, 2, 32] {
            let old = frozen_query_exact_segmentations_keyed_flat(&database, &keys, limit);
            let new = database.query_exact_segmentations_keyed_flat(&keys, limit);
            assert_eq!(new, old);
            if new.is_empty() {
                assert_eq!(new.capacity(), 0);
            }
        }
    }
    let closed = PinyinDatabase::open(Path::new(""));
    let keys = [segments("ping")];
    let (rows, allocations) = count(|| closed.query_exact_segmentations_keyed_flat(&keys, 32));
    assert!(rows.is_empty());
    assert_eq!(allocations, 0);
}

#[test]
fn cold_hit_measurement_reduces_aggregate_storage_without_negative_boundary() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_2_p", "ping'guo", "合成甲", 50)]);
    let key = segments("ping'guo");
    assert!(has_only_complete_pinyin_segments(&key));
    let keys = vec![key; 65];
    let cold = |frozen| {
        measure(|| {
            let database = PinyinDatabase::open(&path);
            let result = if frozen {
                frozen_query_exact_segmentations_keyed_flat(&database, &keys, 128)
            } else {
                database.query_exact_segmentations_keyed_flat(&keys, 128)
            };
            drop(database);
            result
        })
    };
    let (old, before) = cold(true);
    let (new, after) = cold(false);
    assert_eq!(new, old);
    assert_eq!(before.minimum_bytes, 0);
    assert_eq!(after.minimum_bytes, 0);
    assert_eq!(after.allocations + 1, before.allocations);
    assert!(after.remaining_bytes < before.remaining_bytes);
    assert!(after.peak_bytes < before.peak_bytes);
    eprintln!("重复65切分冷查询 before={before:?} after={after:?}");
}

#[test]
fn real_lattice_merge_saves_one_allocation_per_nonempty_lookup() {
    use crate::lattice::decode::LatticeOptions;
    use crate::lattice::merge::merge_lattice_candidates;
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_p", "ping", "甲", 50),
            ("tbl_1_g", "guo", "乙", 50),
            ("tbl_2_p", "ping'guo", "甲乙", 50),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let key = segments("ping'guo");
    let options = LatticeOptions::default();
    let run = |frozen| {
        let mut hits = 0;
        let mut lookup = |span: &[String]| {
            let rows = if frozen {
                frozen_query_lattice_span(&database, span, options.span_limit)
            } else {
                database.query_lattice_span(span, options.span_limit)
            };
            hits += usize::from(!rows.is_empty());
            rows
        };
        let mut candidates = Vec::new();
        let typo = merge_lattice_candidates(
            &mut candidates,
            &key,
            &mut lookup,
            "pingguo",
            &options,
            None,
            &mut [],
            "",
        );
        (candidates, typo, hits)
    };
    drop(run(true));
    drop(run(false));
    let (old, before) = count(|| run(true));
    let (new, after) = count(|| run(false));
    assert_eq!(new, old);
    assert!(!new.0.is_empty());
    assert_eq!(new.2, 3);
    assert_eq!(after + new.2, before);
    eprintln!("真实词网格合并 分配 {before}→{after} 非空查库 {}", new.2);
}

#[test]
fn unbounded_lattice_query_returns_real_sqlite_hits() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(directory.path(), &[("tbl_1_j", "ju", "合成甲", 17)]);
    let database = PinyinDatabase::open(&path);
    let key = segments("jv");
    assert_eq!(
        database.query_lattice_span(&key, usize::MAX),
        frozen_query_lattice_span(&database, &key, 32)
    );
}

#[test]
fn dense_large_pages_keep_original_allocation_budget() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_p", "ping", "合成原点", 0),
            ("tbl_2_p", "ping'guo", "合成原点", 0),
            ("tbl_3_p", "ping'guo'shu", "合成原点", 0),
        ],
    );
    let mut connection = Connection::open(&path).unwrap();
    let transaction = connection.transaction().unwrap();
    for (table, key) in [
        ("tbl_1_p", "ping"),
        ("tbl_2_p", "ping'guo"),
        ("tbl_3_p", "ping'guo'shu"),
    ] {
        for index in 0..128 {
            transaction
                .execute(
                    &format!("INSERT INTO {table}(key,jp,value,weight) VALUES(?1,'p',?2,?3)"),
                    (key, format!("合成{index}"), index as i64),
                )
                .unwrap();
        }
    }
    transaction.commit().unwrap();
    drop(connection);
    let database = PinyinDatabase::open(&path);
    let keys = [
        segments("ping"),
        segments("ping'guo"),
        segments("ping'guo'shu"),
    ];
    for limit in [32, 128] {
        drop(frozen_query_exact_segmentations_keyed_flat(
            &database, &keys, limit,
        ));
        drop(database.query_exact_segmentations_keyed_flat(&keys, limit));
        let (old, before) =
            count(|| frozen_query_exact_segmentations_keyed_flat(&database, &keys, limit));
        let (new, after) = count(|| database.query_exact_segmentations_keyed_flat(&keys, limit));
        assert_eq!(new, old);
        assert_eq!(new.len(), 3 * limit);
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(after, before);
    }
}

#[test]
fn sparse_overflow_grows_only_with_actual_rows_and_same_table_keys_share_limit() {
    let directory = tempfile::tempdir().unwrap();
    let path = pinyin_db(
        directory.path(),
        &[
            ("tbl_1_p", "ping", "甲", 50),
            ("tbl_2_p", "ping'guo", "乙", 50),
            ("tbl_3_p", "ping'guo'shu", "丙", 50),
            ("tbl_4_p", "ping'guo'shu'ye", "丁", 50),
            ("tbl_5_p", "ping'guo'shu'ye'zi", "戊", 50),
            ("tbl_2_p", "ping'gan", "己", 50),
        ],
    );
    let database = PinyinDatabase::open(&path);
    let keys = [
        segments("ping"),
        segments("ping'guo"),
        segments("ping'guo'shu"),
        segments("ping'guo'shu'ye"),
        segments("ping'guo'shu'ye'zi"),
    ];
    drop(frozen_query_exact_segmentations_keyed_flat(
        &database, &keys, 2,
    ));
    drop(database.query_exact_segmentations_keyed_flat(&keys, 2));
    let (old, before) = count(|| frozen_query_exact_segmentations_keyed_flat(&database, &keys, 2));
    let (new, after) = count(|| database.query_exact_segmentations_keyed_flat(&keys, 2));
    assert_eq!(new, old);
    assert_eq!(new.capacity(), 5);
    assert_eq!(old.capacity(), 10);
    assert_eq!(after, before + 2, "五张连续短页明确多两次扩容");
    let same_table = [segments("ping'guo"), segments("ping'gan")];
    let rows = database.query_exact_segmentations_keyed_flat(&same_table, 1);
    assert_eq!(
        rows,
        frozen_query_exact_segmentations_keyed_flat(&database, &same_table, 1)
    );
    assert_eq!(rows.len(), 1, "同表不同键共享 SQL 页上限");
    assert_eq!(rows.capacity(), 1);
}
