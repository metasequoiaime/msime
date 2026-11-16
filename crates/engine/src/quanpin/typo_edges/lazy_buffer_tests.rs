use super::*;
use crate::ime::personal_rerank::allocations::{count, measure};
use crate::quanpin::fixture::Fixture;
use crate::types::autocorrect_type;

// 固定 ffb35a03751e1c2fdf139f13b142f83b0558f75b 的完整收集路径，加上之后的字母级纠错键、全拼罚分折扣和同键只查一次；规划与查库 helper 两侧共用。
fn legacy_collect_typo_edges(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    edit_cache: &mut EditWindowCache,
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<TypoEdge> {
    let mut planned = plan_keys(profile, segments, literal_best, autocorrect_types);
    plan_edit_keys(
        segments,
        literal_best,
        autocorrect_types,
        edit_cache,
        &mut planned,
    );
    for entry in &mut planned {
        entry.penalty *= QUANPIN_TYPO_PENALTY_SCALE;
    }

    let mut misses: Vec<String> = Vec::with_capacity(planned.len());
    for entry in &planned {
        if span_cache.get_ref(&entry.key).is_none() && !misses.contains(&entry.key) {
            misses.push(entry.key.clone());
        }
    }
    if !misses.is_empty() {
        let mut fetched = database.query_exact_keys_per_key(&misses, TYPO_ROWS_PER_KEY);
        for key in misses {
            let key_rows = fetched.remove(&key).unwrap_or_default();
            span_cache.insert(key, key_rows);
        }
    }

    let mut edges = Vec::with_capacity(planned.len() * TYPO_ROWS_PER_KEY);
    for entry in &planned {
        let Some(found) = span_cache.get_ref(&entry.key) else {
            continue;
        };
        edges.extend(found.iter().map(|row| TypoEdge {
            start: entry.start,
            end: entry.end,
            key: row.key.clone(),
            value: row.value.clone(),
            weight: row.weight,
            penalty: entry.penalty,
        }));
    }
    edges
}

/// 产品的完整规划：一换一的键加字母级纠错键，罚分打过全拼折扣。预热缓存和计算期望都要按它来，只按 `plan_keys` 预热时字母级的键仍会未命中。
fn plan_all(
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<PlannedKey> {
    let mut planned = plan_keys(profile, segments, literal_best, autocorrect_types);
    plan_edit_keys(
        segments,
        literal_best,
        autocorrect_types,
        &mut EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY),
        &mut planned,
    );
    for entry in &mut planned {
        entry.penalty *= QUANPIN_TYPO_PENALTY_SCALE;
    }
    planned
}

const TYPES: u32 = autocorrect_type::TRANSPOSITION
    | autocorrect_type::NEIGHBOR
    | autocorrect_type::MISSING_OR_EXTRA;

fn input(length: usize) -> (Vec<String>, SentencePath) {
    let segments = vec!["zhuang".to_owned(); length];
    let words = vec!["甲".to_owned(); length];
    let literal = SentencePath {
        sentence: words.concat(),
        key: segments.join("'"),
        log_prob: 0.0,
        words,
        typo_edges: 0,
    };
    (segments, literal)
}

#[test]
fn all_warm_empty_rows_skip_both_unused_buffers() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    let database = PinyinDatabase::open(&fixture.database());
    let (segments, literal) = input(6);
    let planned = plan_all(&profile, &segments, &literal, TYPES);
    assert!(!planned.is_empty());
    let mut legacy_cache = FifoCache::new(512);
    let mut legacy_edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
    let mut cache = FifoCache::new(512);
    let mut edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
    for entry in &planned {
        legacy_cache.insert(entry.key.clone(), vec![]);
        cache.insert(entry.key.clone(), vec![]);
    }
    // 全命中：窗口改法也先算好，计量区间里两侧都不再往缓存里写。
    plan_edit_keys(
        &segments,
        &literal,
        TYPES,
        &mut legacy_edit_cache,
        &mut Vec::new(),
    );
    plan_edit_keys(&segments, &literal, TYPES, &mut edit_cache, &mut Vec::new());
    let (legacy_edges, legacy_heap) = measure(|| {
        legacy_collect_typo_edges(
            &database,
            &mut legacy_cache,
            &mut legacy_edit_cache,
            &profile,
            &segments,
            &literal,
            TYPES,
        )
    });
    let (edges, heap) = measure(|| {
        collect_typo_edges(
            &database,
            &mut cache,
            &mut edit_cache,
            &profile,
            &segments,
            &literal,
            TYPES,
        )
    });
    assert_eq!(edges, legacy_edges);
    assert_eq!(
        heap.allocations + 2,
        legacy_heap.allocations,
        "全命中空结果仍预留了无用缓冲"
    );
    assert_eq!(edges.capacity(), 0);
    assert_eq!(heap.minimum_bytes, 0);
    assert_eq!(legacy_heap.minimum_bytes, 0);
    assert_eq!(heap.remaining_bytes, 0);
    assert!(legacy_heap.remaining_bytes > 0);
    // 峰值出在两侧共用的字母级规划上，查库阶段省下的缓冲压不低它；懒分配由上面的分配次数和返回存储断言盯着。
    assert!(heap.peak_bytes <= legacy_heap.peak_bytes);
    println!(
        "全热空查询：分配 {}→{}，峰值 {}→{}，返回存储 {}→{}",
        legacy_heap.allocations,
        heap.allocations,
        legacy_heap.peak_bytes,
        heap.peak_bytes,
        legacy_heap.remaining_bytes,
        heap.remaining_bytes
    );
}

fn rows(entry: &PlannedKey, length: usize) -> Vec<DictRow> {
    (0..length)
        .map(|index| DictRow {
            key: entry.key.clone(),
            value: ["甲", "乙", "丙", "丁", "戊"][index % 5].repeat(entry.end - entry.start),
            weight: [100, 100, -7, i64::MAX, 1][index % 5],
        })
        .collect()
}

fn assert_same_edges(edges: &[TypoEdge], legacy_edges: &[TypoEdge]) {
    assert_eq!(edges, legacy_edges);
    for (edge, legacy_edge) in edges.iter().zip(legacy_edges) {
        assert_eq!(edge.penalty.to_bits(), legacy_edge.penalty.to_bits());
    }
}

#[test]
fn warm_nonempty_rows_keep_order_penalties_and_exact_capacity() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    let first_typo = &syllable_typos("zhuang")[0];
    profile
        .record_accepted(&[("zhuang".to_owned(), first_typo.syllable.clone())])
        .unwrap();
    let database = PinyinDatabase::open(&fixture.database());
    let (segments, literal) = input(6);
    let planned = plan_all(&profile, &segments, &literal, TYPES);
    assert!(!planned.is_empty());
    for (length, only_last) in [(1, false), (4, false), (5, false), (1, true)] {
        let mut legacy_cache = FifoCache::new(512);
        let mut legacy_edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
        let mut cache = FifoCache::new(512);
        let mut edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
        for (index, entry) in planned.iter().enumerate() {
            let values = if only_last && index + 1 != planned.len() {
                vec![]
            } else {
                rows(entry, length)
            };
            legacy_cache.insert(entry.key.clone(), values.clone());
            cache.insert(entry.key.clone(), values);
        }
        let (legacy_edges, legacy_allocations) = count(|| {
            legacy_collect_typo_edges(
                &database,
                &mut legacy_cache,
                &mut legacy_edit_cache,
                &profile,
                &segments,
                &literal,
                TYPES,
            )
        });
        let (edges, allocations) = count(|| {
            collect_typo_edges(
                &database,
                &mut cache,
                &mut edit_cache,
                &profile,
                &segments,
                &literal,
                TYPES,
            )
        });
        assert!(!edges.is_empty());
        assert_same_edges(&edges, &legacy_edges);
        assert_eq!(edges.capacity(), edges.len());
        assert!(edges.capacity() <= legacy_edges.capacity());
        assert!(allocations <= legacy_allocations);
    }
}

#[test]
fn sparse_first_hit_reserves_only_the_rows_it_has() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    let database = PinyinDatabase::open(&fixture.database());
    let (segments, literal) = input(6);
    let planned = plan_all(&profile, &segments, &literal, TYPES);
    assert!(planned.len() > TYPO_ROWS_PER_KEY);
    let mut cache = FifoCache::new(512);
    let mut edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
    // 字母级纠错键里同一个键可能有几个跨度；每个键只写一次，后写的空结果不能盖掉第一个键的那一行。
    for (index, entry) in planned.iter().enumerate() {
        if cache.get_ref(&entry.key).is_some() {
            continue;
        }
        cache.insert(
            entry.key.clone(),
            if index == 0 { rows(entry, 1) } else { vec![] },
        );
    }

    let edges = collect_typo_edges(
        &database,
        &mut cache,
        &mut edit_cache,
        &profile,
        &segments,
        &literal,
        TYPES,
    );

    // 那一行词在这个键出现的每个跨度上各成一条边。
    let spans = planned
        .iter()
        .filter(|entry| entry.key == planned[0].key)
        .count();
    assert_eq!(edges.len(), spans);
    assert_eq!(edges.capacity(), spans);
}

#[test]
fn real_queries_and_partial_hits_keep_batch_fill_and_fifo_eviction() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    let (segments, literal) = input(6);
    let planned = plan_all(&profile, &segments, &literal, TYPES);
    assert!(planned.len() > 3);
    for (index, entry) in planned.iter().enumerate() {
        if index % 3 == 0 || index + 1 == planned.len() {
            for row in rows(entry, 5) {
                fixture.insert(&row.key, &row.value, row.weight);
            }
        }
    }
    for capacity in [2, 3, 512] {
        for partial in [false, true] {
            let legacy_database = PinyinDatabase::open(&fixture.database());
            let database = PinyinDatabase::open(&fixture.database());
            let mut legacy_cache = FifoCache::new(capacity);
            let mut cache = FifoCache::new(capacity);
            let mut legacy_edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
            let mut edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
            if partial {
                for (index, entry) in planned.iter().enumerate() {
                    if index % 2 == 0 {
                        let values = if index % 3 == 0 {
                            rows(entry, 1)
                        } else {
                            vec![]
                        };
                        legacy_cache.insert(entry.key.clone(), values.clone());
                        cache.insert(entry.key.clone(), values);
                    }
                }
            }
            for _ in 0..2 {
                let had_misses = planned.iter().any(|entry| !cache.contains(&entry.key));
                let (legacy_edges, legacy_allocations) = count(|| {
                    legacy_collect_typo_edges(
                        &legacy_database,
                        &mut legacy_cache,
                        &mut legacy_edit_cache,
                        &profile,
                        &segments,
                        &literal,
                        TYPES,
                    )
                });
                let (edges, allocations) = count(|| {
                    collect_typo_edges(
                        &database,
                        &mut cache,
                        &mut edit_cache,
                        &profile,
                        &segments,
                        &literal,
                        TYPES,
                    )
                });
                assert_same_edges(&edges, &legacy_edges);
                if edges.is_empty() {
                    assert_eq!(edges.capacity(), 0);
                } else {
                    assert_eq!(edges.capacity(), edges.len());
                    assert!(edges.capacity() <= legacy_edges.capacity());
                }
                let saved = usize::from(!had_misses) + usize::from(edges.is_empty());
                assert_eq!(allocations + saved, legacy_allocations);
                for entry in &planned {
                    assert_eq!(cache.get_ref(&entry.key), legacy_cache.get_ref(&entry.key));
                    assert_eq!(
                        cache.get_ref(&entry.key).map(Vec::capacity),
                        legacy_cache.get_ref(&entry.key).map(Vec::capacity)
                    );
                }
            }
        }
    }
}

#[test]
fn guard_inputs_and_long_plans_keep_empty_results_without_output_storage() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    let database = PinyinDatabase::open(&fixture.database().with_file_name("absent.db"));
    assert!(!database.is_open());
    for length in [0, 1, 2, 6, 64, 65] {
        let (segments, mut literal) = input(length);
        for types in [0, TYPES] {
            // 不完整覆盖的字词路径不标记弱位置，两侧仍遵循相同规划顺序。
            literal.words = vec!["甲甲".to_owned(); length];
            let planned = plan_all(&profile, &segments, &literal, types);
            assert!(planned.len() <= TYPO_KEY_BUDGET + EDIT_KEY_BUDGET);
            let mut legacy_cache = FifoCache::new(512);
            let mut legacy_edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
            let mut cache = FifoCache::new(512);
            let mut edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
            for entry in &planned {
                legacy_cache.insert(entry.key.clone(), vec![]);
                cache.insert(entry.key.clone(), vec![]);
            }
            let (legacy_edges, legacy_allocations) = count(|| {
                legacy_collect_typo_edges(
                    &database,
                    &mut legacy_cache,
                    &mut legacy_edit_cache,
                    &profile,
                    &segments,
                    &literal,
                    types,
                )
            });
            let (edges, allocations) = count(|| {
                collect_typo_edges(
                    &database,
                    &mut cache,
                    &mut edit_cache,
                    &profile,
                    &segments,
                    &literal,
                    types,
                )
            });
            assert_same_edges(&edges, &legacy_edges);
            assert!(edges.is_empty());
            assert_eq!(edges.capacity(), 0);
            let saved = if planned.is_empty() { 0 } else { 2 };
            assert_eq!(allocations + saved, legacy_allocations);
        }
    }
}

#[test]
fn cold_measurement_creates_and_drops_database_and_cache_inside_the_scope() {
    for with_rows in [false, true] {
        let fixture = Fixture::new();
        let profile = PersonalTypoProfile::shared(&fixture.journal());
        let (segments, literal) = input(6);
        let planned = plan_all(&profile, &segments, &literal, TYPES);
        assert!(!planned.is_empty());
        if with_rows {
            let entry = planned.last().unwrap();
            for row in rows(entry, 2) {
                fixture.insert(&row.key, &row.value, row.weight);
            }
        }
        let path = fixture.database();
        // 区间外完成静态表和线程级状态初始化；该连接、缓存及结果随后全部析构。
        let prewarm_database = PinyinDatabase::open(&path);
        let mut prewarm_cache = FifoCache::new(512);
        let mut prewarm_edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
        drop(legacy_collect_typo_edges(
            &prewarm_database,
            &mut prewarm_cache,
            &mut prewarm_edit_cache,
            &profile,
            &segments,
            &literal,
            TYPES,
        ));
        drop(prewarm_cache);
        drop(prewarm_database);
        let (legacy_edges, legacy_heap) = measure(|| {
            let database = PinyinDatabase::open(&path);
            let mut cache = FifoCache::new(512);
            let mut edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
            let edges = legacy_collect_typo_edges(
                &database,
                &mut cache,
                &mut edit_cache,
                &profile,
                &segments,
                &literal,
                TYPES,
            );
            drop(cache);
            drop(database);
            edges
        });
        let (edges, heap) = measure(|| {
            let database = PinyinDatabase::open(&path);
            let mut cache = FifoCache::new(512);
            let mut edit_cache = EditWindowCache::new(EDIT_WINDOW_CACHE_CAPACITY);
            let edges = collect_typo_edges(
                &database,
                &mut cache,
                &mut edit_cache,
                &profile,
                &segments,
                &literal,
                TYPES,
            );
            drop(cache);
            drop(database);
            edges
        });
        assert_same_edges(&edges, &legacy_edges);
        assert_eq!(heap.minimum_bytes, 0);
        assert_eq!(legacy_heap.minimum_bytes, 0);
        if edges.is_empty() {
            assert_eq!(heap.allocations + 1, legacy_heap.allocations);
            assert_eq!(edges.capacity(), 0);
            assert_eq!(heap.remaining_bytes, 0);
            assert!(legacy_heap.remaining_bytes > 0);
            // 同上：峰值出在共用的字母级规划上。
            assert!(heap.peak_bytes <= legacy_heap.peak_bytes);
        } else {
            assert_eq!(heap.allocations, legacy_heap.allocations);
            assert_eq!(edges.capacity(), edges.len());
            assert!(heap.remaining_bytes < legacy_heap.remaining_bytes);
            // 峰值出在两侧共用的字母级规划上。
            assert!(heap.peak_bytes <= legacy_heap.peak_bytes);
        }
        println!(
            "冷查询 with_rows={with_rows}：分配 {}→{}，峰值 {}→{}，返回存储 {}→{}",
            legacy_heap.allocations,
            heap.allocations,
            legacy_heap.peak_bytes,
            heap.peak_bytes,
            legacy_heap.remaining_bytes,
            heap.remaining_bytes
        );
    }
}
