use super::*;
use crate::ime::personal_rerank::allocations::{count, measure};
use crate::quanpin::fixture::Fixture;

// 固定改动前的查询路径，逐次比较真实查库、缓存命中和分配量。
fn legacy_cached_lattice_span(
    database: &PinyinDatabase,
    cache: &mut FifoCache<String, Vec<DictRow>>,
    span: &[String],
    span_limit: usize,
) -> Vec<DictRow> {
    let mut key = span_limit.to_string();
    for syllable in span {
        key.push('\'');
        key.push_str(syllable);
    }
    if let Some(rows) = cache.get_ref(&key) {
        return rows.clone();
    }
    let rows = database.query_lattice_span(span, span_limit);
    cache.insert(key, rows.clone());
    rows
}

#[test]
fn warm_empty_hits_save_all_lookup_key_allocations() {
    let fixture = Fixture::new();
    let database = PinyinDatabase::open(&fixture.database());
    let span = ["ni", "hao", "ma"].map(str::to_owned);
    let mut legacy_cache = FifoCache::new(2);
    let mut cache = FifoCache::new(2);
    assert!(legacy_cached_lattice_span(&database, &mut legacy_cache, &span, 32).is_empty());
    assert!(cached_lattice_span(&database, &mut cache, &span, 32).is_empty());

    let (legacy_rows, legacy_allocations) =
        count(|| legacy_cached_lattice_span(&database, &mut legacy_cache, &span, 32));
    let (rows, allocations) = count(|| cached_lattice_span(&database, &mut cache, &span, 32));
    assert_eq!(rows, legacy_rows);
    assert!(legacy_allocations > 0);
    assert_eq!(allocations, 0, "命中空结果仍分配了查询键");
}

#[test]
fn cold_and_warm_queries_keep_rows_and_limits() {
    let fixture = Fixture::new();
    fixture
        .insert("ni'hao", "甲乙", 100)
        .insert("ni'hao", "丙丁", 100)
        .insert("ni'hao", "戊己", -7)
        .insert("ju", "庚", i64::MAX)
        .insert("lve", "辛", 12)
        .insert("ni", "壬", 20);
    let database = PinyinDatabase::open(&fixture.database());
    let mut legacy_cache = FifoCache::new(3);
    let mut cache = FifoCache::new(3);
    for syllables in [
        vec!["ni", "hao"],
        vec!["jv"],
        vec!["lue"],
        vec!["ni"],
        vec!["zzzz"],
        vec![],
        vec![""],
        vec!["ni", ""],
        vec!["ni", "hao", "ma"],
    ] {
        let span: Vec<String> = syllables.into_iter().map(str::to_owned).collect();
        for limit in [0, 1, 2, 32, 512] {
            let expected = database.query_lattice_span(&span, limit);
            for _ in 0..2 {
                let (legacy_rows, legacy_allocations) = count(|| {
                    legacy_cached_lattice_span(&database, &mut legacy_cache, &span, limit)
                });
                let (rows, allocations) =
                    count(|| cached_lattice_span(&database, &mut cache, &span, limit));
                assert_eq!(rows, expected, "跨度 {span:?}，上限 {limit}");
                assert_eq!(rows, legacy_rows);
                assert_eq!(rows.capacity(), legacy_rows.capacity());
                assert!(allocations <= legacy_allocations);
            }
        }
    }
}

#[test]
fn a_wrong_entry_at_the_same_hash_requeries_instead_of_returning_rows() {
    let fixture = Fixture::new();
    fixture
        .insert("ni'hao", "甲乙", 100)
        .insert("ni'hao", "丙丁", 99);
    let database = PinyinDatabase::open(&fixture.database());
    let span = ["ni", "hao"].map(str::to_owned);
    let expected = database.query_lattice_span(&span, 32);
    assert_eq!(expected.len(), 2);
    for (wrong_span, wrong_limit) in [
        ("'ni'he", 32),
        ("'ni'hao", 1),
        ("'ni'hao'extra", 32),
        ("'niha'o", 32),
        ("", 32),
    ] {
        let mut cache = FifoCache::new(2);
        cache.insert(
            lattice_span_cache_hash(32, &span),
            CachedLatticeSpan {
                span_limit: wrong_limit,
                span: wrong_span.to_owned(),
                rows: vec![DictRow {
                    key: "zzzz".to_owned(),
                    value: "错误哨兵".to_owned(),
                    weight: i64::MAX,
                }],
            },
        );
        for _ in 0..2 {
            assert_eq!(
                cached_lattice_span(&database, &mut cache, &span, 32),
                expected
            );
        }
    }
}

#[test]
fn borrowed_key_check_preserves_delimiters_and_the_full_span() {
    for syllables in [vec![], vec![""], vec!["ni", "hao"], vec!["ni", ""]] {
        let span: Vec<String> = syllables.into_iter().map(str::to_owned).collect();
        let mut key = String::new();
        for syllable in &span {
            key.push('\'');
            key.push_str(syllable);
        }
        let cached = CachedLatticeSpan {
            span_limit: 32,
            span: key,
            rows: vec![],
        };
        let (matches, allocations) = count(|| lattice_span_cache_key_matches(&cached, 32, &span));
        assert!(matches);
        assert_eq!(allocations, 0);
        assert!(!lattice_span_cache_key_matches(&cached, 1, &span));
        let mut longer = span.clone();
        longer.push(String::new());
        assert!(!lattice_span_cache_key_matches(&cached, 32, &longer));
        if !span.is_empty() {
            assert!(!lattice_span_cache_key_matches(
                &cached,
                32,
                &span[..span.len() - 1]
            ));
        }
    }
}

#[test]
fn reads_keep_fifo_age_and_clear_drops_every_entry() {
    let fixture = Fixture::new();
    let database = PinyinDatabase::open(&fixture.database().with_file_name("absent.db"));
    assert!(!database.is_open());
    let ni = ["ni".to_owned()];
    let ni_hash = lattice_span_cache_hash(32, &ni);
    // 其它槽使用确定不同的数值索引，不假设不同音节的哈希一定不同。
    let hao_hash = ni_hash.wrapping_add(1);
    let ma_hash = ni_hash.wrapping_add(2);
    let mut cache = FifoCache::new(2);
    assert!(cached_lattice_span(&database, &mut cache, &ni, 32).is_empty());
    cache.insert(
        hao_hash,
        CachedLatticeSpan {
            span_limit: 32,
            span: "'hao".to_owned(),
            rows: vec![],
        },
    );
    assert!(cached_lattice_span(&database, &mut cache, &ni, 32).is_empty());
    cache.insert(
        ma_hash,
        CachedLatticeSpan {
            span_limit: 32,
            span: "'ma".to_owned(),
            rows: vec![],
        },
    );
    assert!(!cache.contains(&ni_hash));
    assert!(cache.contains(&hao_hash));
    assert!(cache.contains(&ma_hash));
    cache.clear();
    assert!(!cache.contains(&hao_hash));
    assert!(!cache.contains(&ma_hash));
}

#[test]
fn cold_cache_measurement_includes_creation_and_drop() {
    let fixture = Fixture::new();
    fixture
        .insert("ni'hao", "甲乙", 100)
        .insert("ni'hao", "丙丁", 99);
    let path = fixture.database();
    for syllables in [vec!["ni", "hao"], vec!["zzzz"], vec!["ni", "hao", "ma"]] {
        let span: Vec<String> = syllables.into_iter().map(str::to_owned).collect();
        // 不可变音节表在测量区间外预热；数据库、缓存和全部临时存储在区间内创建并析构。
        let _ = has_only_complete_pinyin_segments(&span);
        let (legacy_rows, legacy_heap) = measure(|| {
            let database = PinyinDatabase::open(&path);
            let mut cache = FifoCache::new(2);
            let rows = legacy_cached_lattice_span(&database, &mut cache, &span, 32);
            drop(cache);
            drop(database);
            rows
        });
        let (rows, heap) = measure(|| {
            let database = PinyinDatabase::open(&path);
            let mut cache = FifoCache::new(2);
            let rows = cached_lattice_span(&database, &mut cache, &span, 32);
            drop(cache);
            drop(database);
            rows
        });
        assert_eq!(rows, legacy_rows);
        assert_eq!(rows.capacity(), legacy_rows.capacity());
        assert_eq!(heap.remaining_bytes, legacy_heap.remaining_bytes);
        assert_eq!(heap.minimum_bytes, 0);
        assert_eq!(legacy_heap.minimum_bytes, 0);
        assert!(heap.allocations <= legacy_heap.allocations);
        println!(
            "跨度 {span:?}：分配 {}→{}，峰值 {}→{}，返回存储 {}",
            legacy_heap.allocations,
            heap.allocations,
            legacy_heap.peak_bytes,
            heap.peak_bytes,
            heap.remaining_bytes
        );
    }
}
