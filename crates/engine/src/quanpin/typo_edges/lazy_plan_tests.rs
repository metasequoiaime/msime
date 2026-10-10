use super::*;
use crate::ime::personal_rerank::allocations::{count, measure};
use crate::quanpin::fixture::Fixture;
use crate::types::autocorrect_type;

// 固定 84917234ac7d7fd4c7a9f8ae1124b95841deb7b1 的完整规划路径；仅改函数名并翻译注释。
fn legacy_plan_keys(
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<PlannedKey> {
    let n = segments.len();
    // 最优字面路径中的单字可能来自被纠错拆开的词，因此优先尝试；字词宽度按字符数计算，未完整覆盖输入时不标记弱位置。
    let mut weak = WeakPositions::new(n);
    let mut covered = 0;
    for word in &literal_best.words {
        let width = count_utf8_chars(word);
        if width == 0 || covered + width > n {
            covered = n + 1;
            break;
        }
        if width == 1 {
            weak.set(covered);
        }
        covered += width;
    }
    if covered != n {
        weak.clear();
    }
    let positions = (0..n)
        .filter(|&i| weak.get(i))
        .chain((0..n).filter(|&i| !weak.get(i)));

    let mut planned = Vec::with_capacity(TYPO_KEY_BUDGET);
    let mut variants = Vec::new();
    'positions: for position in positions {
        if planned.len() >= TYPO_KEY_BUDGET {
            break;
        }
        let typed = &segments[position];
        let typos = syllable_typos(typed);
        variants.clear();
        variants.extend(
            typos
                .iter()
                .filter(|typo| autocorrect_types & autocorrect_bit(typo.kind) != 0)
                .map(|typo| (typo, profile.accepted(typed, &typo.syllable))),
        );
        // 变体种类已按成本升序排列，按个人次数稳定排序保留相同次数时的成本优先级。
        variants.sort_by_key(|variant| std::cmp::Reverse(variant.1));
        for &(typo, accepted) in &variants {
            let penalty = discounted(base_cost(typo.kind), accepted);
            for length in MIN_TYPO_SPAN_SYLLABLES..=MAX_TYPO_SPAN_SYLLABLES.min(n) {
                let first = (position + 1).saturating_sub(length);
                for start in first..=position {
                    if start + length > n {
                        break;
                    }
                    let key = typo_span_key(
                        segments,
                        start,
                        start + length,
                        position - start,
                        &typo.syllable,
                    );
                    if planned_key_seen(&planned, &key) {
                        continue;
                    }
                    planned.push(PlannedKey {
                        start,
                        end: start + length,
                        key,
                        penalty,
                    });
                    if planned.len() >= TYPO_KEY_BUDGET {
                        break 'positions;
                    }
                }
            }
        }
    }
    planned
}

const TYPES: u32 = autocorrect_type::TRANSPOSITION
    | autocorrect_type::NEIGHBOR
    | autocorrect_type::MISSING_OR_EXTRA;

fn literal(segments: &[String], words: Vec<String>) -> SentencePath {
    SentencePath {
        sentence: words.concat(),
        key: segments.join("'"),
        log_prob: 0.0,
        words,
        typo_edges: 0,
    }
}

fn assert_same_plan(planned: &[PlannedKey], legacy: &[PlannedKey]) {
    assert_eq!(planned.len(), legacy.len());
    for (entry, old) in planned.iter().zip(legacy) {
        assert_eq!(
            (entry.start, entry.end, &entry.key),
            (old.start, old.end, &old.key)
        );
        assert_eq!(entry.penalty.to_bits(), old.penalty.to_bits());
    }
}

#[test]
fn empty_plans_do_not_allocate_the_unused_budget_buffer() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    for (length, syllable, types) in [
        (3, "a", autocorrect_type::TRANSPOSITION),
        (0, "zhuang", TYPES),
        (1, "zhuang", TYPES),
        (6, "zhuang", 0),
        (65, "zhuang", 0),
        (6, "zhuang", 1 << 31),
        (2, "!", TYPES),
        (64, "!", TYPES),
        (65, "!", TYPES),
    ] {
        let segments = vec![syllable.to_owned(); length];
        let best = literal(&segments, vec!["甲".to_owned(); length]);
        // 在测量前初始化静态变体表，profile 与输入只借用，不在区间内析构。
        drop(legacy_plan_keys(&profile, &segments, &best, types));
        let (legacy, old_heap) = measure(|| legacy_plan_keys(&profile, &segments, &best, types));
        let (planned, heap) = measure(|| plan_keys(&profile, &segments, &best, types));
        assert_same_plan(&planned, &legacy);
        assert!(planned.is_empty());
        assert_eq!(
            heap.allocations + 1,
            old_heap.allocations,
            "空计划仍分配预算缓冲：length={length}, types={types}"
        );
        assert_eq!(planned.capacity(), 0);
        assert_eq!(heap.minimum_bytes, 0);
        assert_eq!(old_heap.minimum_bytes, 0);
        let bytes = TYPO_KEY_BUDGET * std::mem::size_of::<PlannedKey>();
        assert_eq!(heap.remaining_bytes, 0);
        assert_eq!(old_heap.remaining_bytes, bytes as i128);
        assert_eq!(heap.peak_bytes + bytes, old_heap.peak_bytes);
        println!(
            "空规划 length={length}, types={types}：分配 {}→{}，峰值 {}→{}，返回存储 {}→{}",
            old_heap.allocations,
            heap.allocations,
            old_heap.peak_bytes,
            heap.peak_bytes,
            old_heap.remaining_bytes,
            heap.remaining_bytes
        );
    }
}

#[test]
fn populated_plans_keep_capacity_fields_order_and_budget() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    let intended = syllable_typos("zhuang").last().unwrap().syllable.clone();
    profile
        .record_accepted(&[("zhuang".to_owned(), intended)])
        .unwrap();
    let syllables = [
        "zhuang", "shuang", "chang", "liang", "jiang", "xiang", "chuang", "huang", "shang", "guang",
    ];
    let mut saw_full_budget = false;
    for length in [2, 6, 64, 65, 128] {
        let segments = (0..length)
            .map(|index| syllables[index % syllables.len()].to_owned())
            .collect::<Vec<_>>();
        for words in [
            vec!["甲".to_owned(); length],
            vec!["甲甲".to_owned(); length / 2],
            vec![String::new()],
            vec!["甲".repeat(length + 1)],
        ] {
            let best = literal(&segments, words);
            for types in [
                TYPES,
                autocorrect_type::TRANSPOSITION,
                autocorrect_type::NEIGHBOR,
                autocorrect_type::MISSING_OR_EXTRA,
            ] {
                drop(legacy_plan_keys(&profile, &segments, &best, types));
                let (legacy, old_allocations) =
                    count(|| legacy_plan_keys(&profile, &segments, &best, types));
                let (planned, allocations) = count(|| plan_keys(&profile, &segments, &best, types));
                assert_same_plan(&planned, &legacy);
                assert!(planned.len() <= TYPO_KEY_BUDGET);
                if planned.is_empty() {
                    assert_eq!(allocations + 1, old_allocations);
                    assert_eq!(planned.capacity(), 0);
                } else {
                    assert_eq!(allocations, old_allocations);
                    assert_eq!(planned.capacity(), legacy.capacity());
                }
                saw_full_budget |= planned.len() == TYPO_KEY_BUDGET;
            }
        }
    }
    assert!(saw_full_budget, "输入矩阵必须实际触达预算截断");
}

#[test]
fn last_position_can_create_the_first_plan_with_the_original_capacity() {
    let fixture = Fixture::new();
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    for length in [3, 64, 65] {
        let mut segments = vec!["!".to_owned(); length];
        segments[length - 1] = "zhuang".to_owned();
        // 只有末位为弱位置，强制读到位集第 63 位或堆向量第 64 项。
        let best = literal(&segments, vec!["甲".repeat(length - 1), "乙".to_owned()]);
        drop(legacy_plan_keys(&profile, &segments, &best, TYPES));
        let (legacy, old_allocations) =
            count(|| legacy_plan_keys(&profile, &segments, &best, TYPES));
        let (planned, allocations) = count(|| plan_keys(&profile, &segments, &best, TYPES));
        assert!(!planned.is_empty());
        assert_same_plan(&planned, &legacy);
        assert_eq!(allocations, old_allocations);
        assert_eq!(planned.capacity(), legacy.capacity());
        assert!(planned.iter().all(|entry| entry.end == segments.len()));
    }
}

#[test]
fn legal_empty_plan_is_reached_by_the_sentence_merge_source() {
    let fixture = Fixture::new();
    fixture.insert("a", "阿", 100);
    let profile = PersonalTypoProfile::shared(&fixture.journal());
    let database = PinyinDatabase::open(&fixture.database());
    let segments = vec!["a".to_owned(); 3];
    let best = literal(&segments, vec!["阿".to_owned(); 3]);
    let types = autocorrect_type::TRANSPOSITION;
    drop(legacy_plan_keys(&profile, &segments, &best, types));
    let mut cache = FifoCache::new(512);
    let visited = std::cell::Cell::new(false);
    let source_allocations = std::cell::Cell::new(usize::MAX);
    let mut source = |best: &SentencePath| {
        visited.set(true);
        // 只测真实合并调用的纠错收集区间，不把字面词网格查库分配归入本片。
        let (edges, allocations) =
            count(|| collect_typo_edges(&database, &mut cache, &profile, &segments, best, types));
        source_allocations.set(allocations);
        assert!(edges.is_empty());
        assert_eq!(edges.capacity(), 0);
        edges
    };
    let mut lookup = |span: &[String]| database.query_lattice_span(span, 32);
    let mut candidates = Vec::new();
    let typo = crate::lattice::merge::merge_lattice_candidates(
        &mut candidates,
        &segments,
        &mut lookup,
        "aaa",
        &crate::lattice::decode::LatticeOptions::default(),
        Some(&mut source),
        &mut [],
        "",
    );
    assert!(visited.get(), "真实句子合并必须进入纠错源");
    assert!(typo.is_none());
    assert!(candidates.iter().any(|item| item.word == "阿阿阿"));
    assert_eq!(source_allocations.get(), 0);
}
