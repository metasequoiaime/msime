use super::*;
use crate::ime::personal_rerank::allocations::count;

// 固定 8f011cb59c 的完整入列实现。
pub(super) fn frozen_push(
    candidates: &mut Vec<WordItem>,
    leading: &mut LeadingMap,
    candidate: WordItem,
    prefer_exact: bool,
    initials_lead: bool,
    grid: KeyGrid,
) {
    let key = rank_key(&candidate, prefer_exact, initials_lead, grid);
    let hash = leading_hash(&candidate.word);
    let existing = leading
        .get(&hash)
        .and_then(|slot| slot.find(candidates, &candidate.word));
    if let Some((slot, best)) = existing {
        if best <= key {
            return;
        }
        let candidate_index = candidates.len();
        candidates.push(candidate);
        leading
            .get_mut(&hash)
            .expect("leading hash entry remains present")
            .update(slot, candidate_index, key);
        return;
    }
    if leading.is_empty() {
        leading.reserve(CANDIDATE_LIMIT);
    }
    let candidate_index = candidates.len();
    candidates.push(candidate);
    let entry = LeadingEntry {
        candidate: candidate_index,
        key,
    };
    if let Some(slot) = leading.get_mut(&hash) {
        slot.insert(entry);
    } else {
        leading.insert(hash, LeadingSlot::One(entry));
    }
}

// 单次槽位查询未通过固定性能门槛，只保留测试候选。
pub(super) fn candidate_push(
    candidates: &mut Vec<WordItem>,
    leading: &mut LeadingMap,
    candidate: WordItem,
    prefer_exact: bool,
    initials_lead: bool,
    grid: KeyGrid,
) {
    let key = rank_key(&candidate, prefer_exact, initials_lead, grid);
    let hash = leading_hash(&candidate.word);
    if leading.is_empty() {
        leading.reserve(CANDIDATE_LIMIT);
    }
    match leading.entry(hash) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let existing = entry.get().find(candidates, &candidate.word);
            if existing.is_some_and(|(_, best)| best <= key) {
                return;
            }
            let index = candidates.len();
            candidates.push(candidate);
            if let Some((slot, _)) = existing {
                entry.get_mut().update(slot, index, key);
            } else {
                entry.get_mut().insert(LeadingEntry {
                    candidate: index,
                    key,
                });
            }
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            let index = candidates.len();
            candidates.push(candidate);
            entry.insert(LeadingSlot::One(LeadingEntry {
                candidate: index,
                key,
            }));
        }
    }
}

fn rows(width: usize, mode: &str) -> Vec<WordItem> {
    (0..width)
        .map(|index| {
            let weight = match mode {
                "improve" => 101,
                "worse" => 99,
                "mixed" if index % 3 == 0 => 101,
                "mixed" if index % 3 == 1 => 99,
                _ => 100,
            };
            let mut row = WordItem::new(
                "64426",
                format!("合成词{index}😀"),
                weight,
                CandidateSource::Database,
                "ni'hao",
            );
            row.word.shrink_to_fit();
            row.corrected_from = "synthetic".into();
            row.sentence_words = vec!["合成".to_owned(), "词段".to_owned()];
            row
        })
        .collect()
}

fn assert_state(actual: &[WordItem], expected: &[WordItem], a: &LeadingMap, b: &LeadingMap) {
    assert_eq!(actual, expected);
    assert_eq!(a.capacity(), b.capacity());
    assert_eq!(a.len(), b.len());
    for (row, old) in actual.iter().zip(expected) {
        for (field, old_field) in [
            (&row.word, &old.word),
            (&row.pinyin, &old.pinyin),
            (&row.canonical_pinyin, &old.canonical_pinyin),
            (&row.corrected_from, &old.corrected_from),
        ] {
            assert_eq!(field.capacity(), old_field.capacity());
        }
        assert_eq!(row.sentence_words.capacity(), old.sentence_words.capacity());
        for (part, old_part) in row.sentence_words.iter().zip(&old.sentence_words) {
            assert_eq!(part.capacity(), old_part.capacity());
        }
        assert_eq!(
            leading_rank(a, actual, &row.word),
            leading_rank(b, expected, &row.word)
        );
    }
    for (hash, slot) in a {
        let entries = |slot: &LeadingSlot| match slot {
            LeadingSlot::One(entry) => vec![(entry.candidate, entry.key)],
            LeadingSlot::Many(entries) => entries
                .iter()
                .map(|entry| (entry.candidate, entry.key))
                .collect(),
        };
        assert_eq!(entries(slot), entries(&b[hash]));
    }
}

#[test]
fn entry_preserves_order_slots_capacities_and_allocations() {
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    let shuangpin = crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap();
    for grid in [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(shuangpin),
    ] {
        for flags in 0..4 {
            for width in [0, 1, 16, 128, 224, 512, 896] {
                let mut a = LeadingMap::default();
                let mut b = LeadingMap::default();
                let mut current = Vec::new();
                let mut previous = Vec::new();
                for mode in ["unique", "equal", "improve", "worse", "mixed"] {
                    let mut input = rows(width, mode);
                    if mode == "mixed" {
                        for row in input.iter_mut().step_by(3) {
                            row.weight = 102;
                        }
                    }
                    let old_input = input.clone();
                    let (_, old_allocs) = count(|| {
                        for row in old_input {
                            frozen_push(
                                &mut previous,
                                &mut b,
                                row,
                                flags & 1 != 0,
                                flags & 2 != 0,
                                grid,
                            );
                        }
                    });
                    let (_, new_allocs) = count(|| {
                        for row in input {
                            candidate_push(
                                &mut current,
                                &mut a,
                                row,
                                flags & 1 != 0,
                                flags & 2 != 0,
                                grid,
                            );
                        }
                    });
                    assert_state(&current, &previous, &a, &b);
                    assert_eq!(current.capacity(), previous.capacity());
                    assert_eq!(new_allocs, old_allocs, "{grid:?}/{flags}/{width}/{mode}");
                }
            }
        }
    }
}

#[test]
fn entry_handles_colliding_words_and_updates_each_winner() {
    let mut a = LeadingMap::default();
    let mut b = LeadingMap::default();
    let mut current = rows(1, "equal");
    let mut previous = current.clone();
    let colliding = WordItem::new("64", "碰撞合成词", 10, CandidateSource::Database, "ni");
    let hash = leading_hash(&colliding.word);
    for leading in [&mut a, &mut b] {
        leading.reserve(CANDIDATE_LIMIT);
        leading.insert(
            hash,
            LeadingSlot::One(LeadingEntry {
                candidate: 0,
                key: rank_key(&current[0], false, false, KeyGrid::NineKey),
            }),
        );
    }
    for weight in [10, 10, 9, 11, 8, 12] {
        let mut row = colliding.clone();
        row.weight = weight;
        frozen_push(
            &mut previous,
            &mut b,
            row.clone(),
            false,
            false,
            KeyGrid::NineKey,
        );
        candidate_push(&mut current, &mut a, row, false, false, KeyGrid::NineKey);
        assert_state(&current, &previous, &a, &b);
    }
    let LeadingSlot::Many(entries) = &a[&hash] else {
        panic!("必须保留两个不同词的碰撞槽")
    };
    assert_eq!(entries.len(), 2);
    assert_eq!(current[entries[1].candidate].weight, 12);
}

#[test]
fn entry_matches_frozen_complete_refresh() {
    use crate::quanpin::fixture::Fixture;
    let fixture = Fixture::new();
    for (key, word, weight) in [
        ("ni", "你", 100),
        ("mi", "米", 80),
        ("hao", "好", 100),
        ("ni'hao", "你好", 500),
        ("ni'hao", "拟号", 100),
        ("ni'men'hao", "你们好", 800),
        ("zi'you", "自由", 400),
        ("zhi'you", "只有", 500),
        ("zi", "字", 80),
        ("zhi", "只", 100),
    ] {
        fixture.insert(key, word, weight);
    }
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    let shuangpin = crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap();
    for grid in [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(shuangpin),
    ] {
        for fuzzy in [0, crate::types::fuzzy_rule::Z_ZH] {
            for restriction in 0..4 {
                let open = || {
                    let mut session = NineKeySession::new(
                        &fixture.paths,
                        false,
                        FrequencyAdjustmentOptions {
                            mode: FrequencyAdjustmentMode::Disabled,
                            ..Default::default()
                        },
                        FuzzyPinyinOptions { rules: fuzzy },
                        EnglishInputOptions::default(),
                        true,
                        false,
                    );
                    session.grid = grid;
                    session.fuzzy.rules = fuzzy;
                    session.expressive = MixedExpressiveOptions::default();
                    session
                };
                let mut a = open();
                let mut b = open();
                for reading in ["ni", "ni'hao", "ni'men'hao", "zi'you", "ni'hao", ""] {
                    for session in [&mut a, &mut b] {
                        session.digits = grid.encode(reading);
                        session.locked.clear();
                        session.splits.clear();
                        session.initial = None;
                        if restriction == 1 && reading.starts_with("ni'") {
                            session.locked.push("ni".to_owned());
                        }
                        if restriction == 2 && session.digits.len() > 1 {
                            session.splits.push(1);
                        }
                        if restriction == 3 {
                            session.single_character_only = true;
                        }
                    }
                    for _ in 0..2 {
                        b.candidate_entry_refresh();
                        a.refresh();
                        assert_eq!(a.snapshot(), b.snapshot());
                        assert_eq!(a.candidates, b.candidates);
                        assert_eq!(a.candidates.capacity(), b.candidates.capacity());
                        for (row, old) in a.candidates.iter().zip(&b.candidates) {
                            // 九键给 `pinyin` 换上一份按长度新建的 `SharedString`，不再复用旧缓冲区，容量只能比内容。
                            assert_eq!(row.pinyin, old.pinyin);
                            assert_eq!(row.word.capacity(), old.word.capacity());
                            assert_eq!(
                                row.canonical_pinyin.capacity(),
                                old.canonical_pinyin.capacity()
                            );
                            assert_eq!(
                                row.sentence_words.capacity(),
                                old.sentence_words.capacity()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "手动运行普通分配器下的完整入列配对计时"]
fn entry_paired_latency() {
    use std::hint::black_box;
    use std::time::{Duration, Instant};
    fn batch(old: bool, width: usize, mode: &str, iterations: usize) -> Duration {
        let input = rows(width, mode);
        let mut batches: Vec<_> = (0..iterations)
            .map(|_| {
                let mut candidates = Vec::with_capacity(width * 2);
                let mut leading = LeadingMap::default();
                if mode != "unique" {
                    for row in rows(width, "equal") {
                        frozen_push(
                            &mut candidates,
                            &mut leading,
                            row,
                            false,
                            false,
                            KeyGrid::NineKey,
                        );
                    }
                } else {
                    leading.reserve(CANDIDATE_LIMIT.max(width));
                }
                (input.clone(), candidates, leading)
            })
            .collect();
        let start = Instant::now();
        for (rows, candidates, leading) in &mut batches {
            for row in rows.drain(..) {
                if old {
                    frozen_push(
                        black_box(candidates),
                        black_box(leading),
                        row,
                        false,
                        false,
                        KeyGrid::NineKey,
                    );
                } else {
                    candidate_push(
                        black_box(candidates),
                        black_box(leading),
                        row,
                        false,
                        false,
                        KeyGrid::NineKey,
                    );
                }
            }
            black_box((&*candidates, &*leading));
        }
        start.elapsed()
    }
    println!("ENTRY_CSV,width,mode,pair,iterations,old_ns,new_ns");
    let mut failed = Vec::new();
    for width in [16, 128, 512] {
        for mode in ["unique", "improve", "equal", "worse", "mixed"] {
            let mut iterations = 1;
            while batch(true, width, mode, iterations) < Duration::from_millis(2) {
                iterations *= 2;
            }
            let mut ratios = Vec::new();
            for pair in 0..40 {
                let (old, new) = if pair % 2 == 0 {
                    (
                        batch(true, width, mode, iterations),
                        batch(false, width, mode, iterations),
                    )
                } else {
                    let new = batch(false, width, mode, iterations);
                    (batch(true, width, mode, iterations), new)
                };
                println!(
                    "ENTRY_CSV,{width},{mode},{pair},{iterations},{},{},",
                    old.as_nanos(),
                    new.as_nanos()
                );
                ratios.push(new.as_secs_f64() / old.as_secs_f64());
            }
            ratios.sort_by(f64::total_cmp);
            let median = (ratios[19] + ratios[20]) / 2.;
            let threshold = if matches!(mode, "unique" | "improve") {
                0.98
            } else {
                1.05
            };
            println!("ENTRY_SUMMARY width={width} mode={mode} iterations={iterations} median={median:.6} p10={:.6} p90={:.6} threshold={threshold}", ratios[3], ratios[35]);
            if median >= threshold {
                failed.push((width, mode, median, threshold));
            }
        }
    }
    assert!(failed.is_empty(), "完整矩阵未通过：{failed:?}");
}
