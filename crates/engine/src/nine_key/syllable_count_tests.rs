use super::*;
use std::hint::black_box;
use std::time::Instant;

fn grids() -> [KeyGrid; 3] {
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .expect("内置九键双拼表");
    [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(
            crate::nine_key_shuangpin::nine_key_shuangpin(&table).expect("合法表"),
        ),
    ]
}

fn behavior_grids() -> Vec<KeyGrid> {
    let mut result = vec![KeyGrid::NineKey, KeyGrid::FourteenKey];
    for kind in crate::types::NineKeyShuangpinKind::BUILTIN {
        let table = crate::nine_key_shuangpin::builtin_table(kind).expect("内置表");
        result.push(KeyGrid::NineKeyShuangpin(
            crate::nine_key_shuangpin::nine_key_shuangpin(&table).expect("合法表"),
        ));
    }
    result
}

fn fixture(length: usize, mode: usize) -> Vec<WordItem> {
    (0..length)
        .map(|index| {
            let mixed = (index * 73 + 17) % 101;
            let word = if mode == 2 { index % 17 } else { index };
            let mut row = WordItem::new(
                if mode == 0 {
                    &"64426646"[..1 + mixed % 8]
                } else {
                    "626"
                },
                format!("样例{word}"),
                match mode {
                    0 => mixed as i64 * 31 - 1000,
                    1 => 100,
                    _ => index as i64,
                },
                if mode == 0 && index % 7 == 0 {
                    CandidateSource::Generated
                } else {
                    CandidateSource::Database
                },
                if mode == 0 {
                    ["ni'hao", "a", "ming'tian", "", "lü'e"][mixed % 5]
                } else {
                    "man"
                },
            );
            row.fixed_position = (index % 6) as i32;
            row.fuzzy = mode == 0 && index % 7 == 0;
            row.corrected_from = format!("synthetic-{index}").into();
            row.sentence_association = index % 2 == 0;
            row.sentence_words = vec![format!("part-{index}")];
            row
        })
        .collect()
}

// 按词面过滤，刻意删除前部下标；同词各行的去留保持一致。
fn filter(rows: &mut Vec<WordItem>) {
    rows.retain(|row| row.word.as_bytes().last().is_some_and(|byte| byte % 2 != 0));
}

type Pipeline = fn(&[WordItem], bool, bool, Option<&InitialsBoost>, KeyGrid, bool) -> Vec<WordItem>;

#[inline(never)]
fn reference(
    input: &[WordItem],
    exact: bool,
    initials: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
    filtering: bool,
) -> Vec<WordItem> {
    let mut candidates = Vec::new();
    let mut leading: HashMap<String, RankKey> = HashMap::new();
    for row in input {
        frozen::push_ranked(
            &mut candidates,
            &mut leading,
            row.clone(),
            exact,
            initials,
            grid,
        );
    }
    if filtering {
        filter(&mut candidates);
    }
    frozen::rank_candidates(&mut candidates, exact, initials, boost, grid);
    candidates
}

#[inline(never)]
fn production(
    input: &[WordItem],
    exact: bool,
    initials: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
    filtering: bool,
) -> Vec<WordItem> {
    let mut candidates = Vec::new();
    let mut leading = LeadingMap::default();
    for row in input {
        push_ranked(
            &mut candidates,
            &mut leading,
            row.clone(),
            exact,
            initials,
            grid,
        );
    }
    if filtering {
        filter(&mut candidates);
    }
    rank_candidates(&mut candidates, exact, initials, boost, grid);
    candidates
}

#[test]
fn full_pipeline_preserves_fields_capacity_ties_improvements_and_filters() {
    for length in [0, 1, 8, 32, 33, 64, 128, 512, 2048] {
        for mode in 0..3 {
            let input = fixture(length, mode);
            for grid in behavior_grids() {
                for exact in [false, true] {
                    for initials in [false, true] {
                        for filtering in [false, true] {
                            let expected =
                                reference(&input, exact, initials, None, grid, filtering);
                            let actual = production(&input, exact, initials, None, grid, filtering);
                            assert_eq!(actual, expected, "length={length}, mode={mode}, grid={grid:?}, exact={exact}, initials={initials}, filtering={filtering}");
                            assert_eq!(actual.capacity(), expected.capacity());
                        }
                    }
                }
            }
        }
    }
    // 同词同键拒绝、晚到改善与别词同分的稳定顺序。
    let mut input = fixture(64, 1);
    let mut duplicate = input[0].clone();
    duplicate.corrected_from = "same-key-late".into();
    input.push(duplicate);
    let mut improved = input[1].clone();
    improved.weight += 1;
    improved.corrected_from = "improved-late".into();
    input.push(improved);
    let mut tied = input[2].clone();
    tied.weight += 1;
    tied.word = "并列词".into();
    input.push(tied);
    let expected = reference(&input, true, true, None, KeyGrid::NineKey, false);
    let actual = production(&input, true, true, None, KeyGrid::NineKey, false);
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
}

#[test]
fn full_pipeline_preserves_active_interleaving_and_boost() {
    let input: Vec<_> = (0..160)
        .map(|index| {
            let initials = index >= 136;
            WordItem::new(
                "626",
                if initials {
                    format!("简拼{}", index - 136)
                } else {
                    format!("音节{index}")
                },
                if initials { 2000 + index } else { 1000 - index },
                CandidateSource::Database,
                if initials { "ni'hao'ma" } else { "man" },
            )
        })
        .collect();
    let boost = InitialsBoost {
        used: [("简拼0".into(), 100)].into(),
        frequency: FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Linear,
            trigger_count: 1,
            linear_step: 2,
        },
    };
    for grid in behavior_grids() {
        for exact in [false, true] {
            for initials in [false, true] {
                let raw = reference(&input, exact, initials, None, grid, false);
                let boosted = reference(&input, exact, initials, Some(&boost), grid, false);
                assert_ne!(raw, boosted, "样本必须触发调频");
                if !initials {
                    assert!(
                        raw.iter().any(|row| row.word.starts_with("简拼")),
                        "低于原始截断线的简拼必须实际交错到可见范围"
                    );
                }
                for (boost, expected) in [(None, &raw), (Some(&boost), &boosted)] {
                    let actual = production(&input, exact, initials, boost, grid, false);
                    assert_eq!(&actual, expected);
                    assert_eq!(actual.capacity(), expected.capacity());
                }
            }
        }
    }
}

#[test]
fn full_pipeline_does_not_increase_allocation_count_or_peak_bytes() {
    for length in [0, 1, 8, 32, 33, 64, 128, 512, 2048] {
        for mode in 0..3 {
            let input = fixture(length, mode);
            let measure = |pipeline: Pipeline| {
                crate::ime::personal_rerank::allocations::measure(|| {
                    let rows = pipeline(&input, true, false, None, KeyGrid::NineKey, false);
                    black_box(rows);
                })
                .1
            };
            let old = measure(reference);
            let new = measure(production);
            assert_eq!(old.remaining_bytes, 0);
            assert_eq!(new.remaining_bytes, 0);
            assert!(
                new.allocations <= old.allocations,
                "生产路径增加了申请：{old:?} -> {new:?}"
            );
            assert!(
                new.peak_bytes <= old.peak_bytes,
                "生产路径增加了峰值：{old:?} -> {new:?}"
            );
            println!(
                "heap {length} {mode} {} {} {} {}",
                old.allocations, new.allocations, old.peak_bytes, new.peak_bytes
            );
        }
    }
}

fn elapsed(
    pipeline: Pipeline,
    input: &[WordItem],
    grid: KeyGrid,
    filtering: bool,
    repeats: usize,
) -> f64 {
    let start = Instant::now();
    for _ in 0..repeats {
        // 整个入列与排名：输入复制、map/工作区申请、后处理及全部析构均在区间内。
        black_box(pipeline(
            black_box(input),
            true,
            false,
            None,
            grid,
            filtering,
        ));
    }
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "手动完整入列加排名126形状配对；仅在普通分配器构建中运行"]
fn full_pipeline_paired_budget() {
    let mut failures = Vec::new();
    println!("pair,length,mode,grid,filtering,repeats,pair,old_seconds,new_seconds");
    for length in [8, 32, 33, 64, 128, 512, 2048] {
        for mode in 0..3 {
            let input = fixture(length, mode);
            for (grid_index, grid) in grids().into_iter().enumerate() {
                for filtering in [false, true] {
                    assert_eq!(
                        production(&input, true, false, None, grid, filtering),
                        reference(&input, true, false, None, grid, filtering)
                    );
                    let mut repeats = 1;
                    while elapsed(reference, &input, grid, filtering, repeats) < 0.010 {
                        repeats *= 2;
                    }
                    let mut ratios = Vec::new();
                    for pair in 0..21 {
                        let (old, new) = if pair % 2 == 0 {
                            let old = elapsed(reference, &input, grid, filtering, repeats);
                            (old, elapsed(production, &input, grid, filtering, repeats))
                        } else {
                            let new = elapsed(production, &input, grid, filtering, repeats);
                            (elapsed(reference, &input, grid, filtering, repeats), new)
                        };
                        println!("pair,{length},{mode},{grid_index},{filtering},{repeats},{pair},{old:.9},{new:.9}");
                        ratios.push(new / old);
                    }
                    ratios.sort_by(f64::total_cmp);
                    let ratio = ratios[10];
                    let budget = if length >= 64 && !filtering {
                        0.98
                    } else {
                        1.05
                    };
                    println!(
                        "result,{length},{mode},{grid_index},{filtering},{ratio:.6},{budget:.2}"
                    );
                    if ratio >= budget {
                        failures.push((length, mode, grid_index, filtering, ratio, budget));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "完整流水线配对预算失败：{failures:?}");
}

#[test]
fn full_pipeline_preserves_winner_changes_across_rank_dimensions() {
    for grid in behavior_grids() {
        for exact in [false, true] {
            for initials in [false, true] {
                for dimension in 0..6 {
                    // 每个维度独立建同词序列，避免更长覆盖遮住后续维度。
                    let mut first =
                        WordItem::new("6266", "目标维度", 100, CandidateSource::Database, "man");
                    let mut second = first.clone();
                    match dimension {
                        0 => second.pinyin.push('6'),
                        1 => first.source = CandidateSource::Generated,
                        2 => {
                            first.pinyin = "626".into();
                            second.pinyin = "626".into();
                            first.canonical_pinyin = "ni'hao'ma".into();
                        }
                        3 => first.canonical_pinyin = "ni'hao'ma".into(),
                        4 => first.fuzzy = true,
                        _ => second.weight += 1,
                    }
                    let first_key = rank_key(&first, exact, initials, grid);
                    let second_key = rank_key(&second, exact, initials, grid);
                    if dimension != 3 || exact {
                        assert_ne!(
                            first_key, second_key,
                            "维度必须实际改变键：{dimension}, {grid:?}"
                        );
                    }
                    let (mut worse, mut better) = if first_key < second_key {
                        (second, first)
                    } else {
                        (first, second)
                    };
                    worse.corrected_from = "first-worse".into();
                    better.corrected_from = "late-winner".into();
                    let mut same = better.clone();
                    same.corrected_from = "later-same-key".into();
                    let mut input = fixture(64, 0);
                    input.extend([worse, better.clone(), same]);
                    for filtering in [false, true] {
                        let expected = reference(&input, exact, initials, None, grid, filtering);
                        if !filtering && first_key != second_key {
                            assert_eq!(
                                expected.iter().find(|row| row.word == "目标维度"),
                                Some(&better)
                            );
                        }
                        let actual = production(&input, exact, initials, None, grid, filtering);
                        assert_eq!(actual, expected);
                        assert_eq!(actual.capacity(), expected.capacity());
                    }
                }
            }
        }
    }
}
#[test]
fn reading_counts_and_rank_keys_preserve_empty_segments_and_unicode() {
    let readings = [
        "",
        "'",
        "''",
        "'xi''an'",
        "ni'hao",
        "lü'e",
        "Nǐ'Hǎo",
        "中'文",
        "’ni’hao",
        "ni＇hao",
        "ni\0'hao",
    ];
    let sources = [
        CandidateSource::Database,
        CandidateSource::UserDatabase,
        CandidateSource::Generated,
        CandidateSource::Fallback,
        CandidateSource::NeuralKeyboard,
        CandidateSource::EnglishDictionary,
    ];
    let mut samples: Vec<String> = readings.iter().map(|s| s.to_string()).collect();
    let alphabet = ['a', '\'', '中', 'ü', 'A', '’'];
    for number in 0..4096 {
        let mut number = number;
        let mut value = String::new();
        for _ in 0..6 {
            value.push(alphabet[number % alphabet.len()]);
            number /= alphabet.len();
        }
        samples.push(value);
    }
    samples.push("ni''中'hao'".repeat(128));
    for reading in samples {
        assert_eq!(
            reading_syllable_count(&reading),
            reading.split('\'').count()
        );
        for full_len in [0, 1, 2, 3, 4, 8, 64, reading_syllable_count(&reading)] {
            assert_eq!(
                reading_syllable_count(&reading) > full_len,
                reading.matches('\'').count() >= full_len
            );
        }
        for grid in behavior_grids() {
            assert_eq!(
                grid.reading_code_len(&reading),
                frozen::reading_code_len(grid, &reading)
            );
            for source in sources {
                for covered in [0, 1, 2, 3, 4, 8, 64] {
                    let mut row = WordItem::new("", "合成词", -1234, source, &reading);
                    row.fuzzy = covered % 2 == 0;
                    assert_eq!(
                        comparable_weight_for_reading(&row, &reading),
                        frozen::comparable_weight_for_reading(&row, &reading)
                    );
                    assert_eq!(
                        is_initials_reading(&row, covered, &reading),
                        frozen::is_initials_reading(&row, covered, &reading)
                    );
                    for exact in [false, true] {
                        for initials in [false, true] {
                            assert_eq!(
                                rank_key_for_reading(
                                    &row, covered, &reading, exact, initials, grid
                                ),
                                frozen::rank_key_for_reading(
                                    &row, covered, &reading, exact, initials, grid
                                )
                            );
                        }
                    }
                }
            }
        }
    }
}

// 固定b3ba3480b0的完整旧入列、排名与传递调用，不与生产共用本片修改的扫描。
mod frozen {
    use super::*;
    pub(super) fn reading_code_len(grid: KeyGrid, canonical_pinyin: &str) -> usize {
        match grid {
            KeyGrid::NineKeyShuangpin(_) => 2 * canonical_pinyin.split('\'').count(),
            _ => canonical_pinyin
                .bytes()
                .filter(u8::is_ascii_lowercase)
                .count(),
        }
    }

    pub(super) fn rank_key(
        item: &WordItem,
        prefer_exact: bool,
        initials_lead: bool,
        grid: KeyGrid,
    ) -> RankKey {
        rank_key_for_reading(
            item,
            item.pinyin.len(),
            &item.canonical_pinyin,
            prefer_exact,
            initials_lead,
            grid,
        )
    }

    pub(super) fn rank_key_for_reading(
        item: &WordItem,
        covered: usize,
        canonical: &str,
        prefer_exact: bool,
        initials_lead: bool,
        grid: KeyGrid,
    ) -> RankKey {
        #[cfg(all(test, debug_assertions))]
        cached_rank_tests::KEY_EVALUATIONS.with(|count| count.set(count.get() + 1));
        let completion = prefer_exact && reading_code_len(grid, canonical) > covered;
        let initials = is_initials_reading(item, covered, canonical);
        (
            Reverse(covered),
            item.source.is_generated_or_fallback(),
            initials != initials_lead,
            completion,
            item.fuzzy,
            Reverse(comparable_weight_for_reading(item, canonical)),
        )
    }

    pub(super) fn comparable_weight_for_reading(item: &WordItem, canonical: &str) -> i64 {
        if item.source != CandidateSource::Generated || canonical.is_empty() {
            return item.weight;
        }
        let syllables = canonical.split('\'').count();
        // 每词插入代价已在共享词网格解码中扣除；这里只消去跨切分比较的音节奖励。
        item.weight - (PHRASE_LENGTH_BONUS * 1000.0 * syllables as f64) as i64
    }

    pub(super) fn is_initials_row(item: &WordItem) -> bool {
        is_initials_reading(item, item.pinyin.len(), &item.canonical_pinyin)
    }

    pub(super) fn is_initials_reading(item: &WordItem, digits: usize, canonical: &str) -> bool {
        digits >= 2
            && !item.source.is_generated_or_fallback()
            && canonical.split('\'').count() == digits
            && canonical.bytes().filter(u8::is_ascii_lowercase).count() > digits
    }

    pub(super) fn push_ranked(
        candidates: &mut Vec<WordItem>,
        leading: &mut HashMap<String, RankKey>,
        candidate: WordItem,
        prefer_exact: bool,
        initials_lead: bool,
        grid: KeyGrid,
    ) {
        let key = rank_key(&candidate, prefer_exact, initials_lead, grid);
        match leading.get_mut(candidate.word.as_str()) {
            Some(best) if *best <= key => return,
            Some(best) => *best = key,
            None => {
                if leading.is_empty() {
                    leading.reserve(CANDIDATE_LIMIT);
                }
                leading.insert(candidate.word.to_string(), key);
            }
        }
        candidates.push(candidate);
    }

    pub(super) fn rank_candidates(
        candidates: &mut Vec<WordItem>,
        prefer_exact: bool,
        initials_lead: bool,
        boost: Option<&InitialsBoost>,
        grid: KeyGrid,
    ) {
        if candidates.len() <= SMALL_CANDIDATE_SORT {
            candidates.sort_by_key(|item| rank_key(item, prefer_exact, initials_lead, grid));
        } else {
            candidates.sort_by_cached_key(|item| rank_key(item, prefer_exact, initials_lead, grid));
        }
        retain_unique_words(candidates);
        if !initials_lead {
            interleave_initials(candidates);
        }
        if let Some(boost) = boost {
            boost_used_initials(candidates, initials_lead, boost);
        }
        candidates.truncate(CANDIDATE_LIMIT);
    }

    pub(super) fn interleave_initials(candidates: &mut Vec<WordItem>) {
        let mut start = 0;
        while start < candidates.len() {
            let coverage = candidates[start].pinyin.len();
            let generated = candidates[start].source.is_generated_or_fallback();
            let end = start
                + candidates[start..]
                    .iter()
                    .take_while(|item| {
                        item.pinyin.len() == coverage
                            && item.source.is_generated_or_fallback() == generated
                    })
                    .count();
            if !generated {
                // 这一段里音节行在前、简拼行在后（`rank_key` 的顺序）。
                let middle = start
                    + candidates[start..end]
                        .iter()
                        .take_while(|item| !is_initials_row(item))
                        .count();
                let kept = start + SYLLABLE_ROWS_BEFORE_INITIALS;
                if kept < middle && middle < end {
                    let mut syllables: Vec<WordItem> = candidates.drain(kept..end).collect();
                    let initials = syllables.split_off(middle - kept);
                    let mut syllables = syllables.into_iter().peekable();
                    let mut initials = initials.into_iter().peekable();
                    let mut merged = Vec::with_capacity(end - kept);
                    loop {
                        let take_initial = match (syllables.peek(), initials.peek()) {
                            (Some(syllable), Some(initial)) => initial.weight > syllable.weight,
                            (None, Some(_)) => true,
                            (_, None) => false,
                        };
                        let next = if take_initial {
                            initials.next()
                        } else {
                            syllables.next()
                        };
                        match next {
                            Some(item) => merged.push(item),
                            None => break,
                        }
                    }
                    candidates.splice(kept..kept, merged);
                }
            }
            start = end;
        }
    }

    pub(super) fn boost_used_initials(
        candidates: &mut Vec<WordItem>,
        initials_lead: bool,
        boost: &InitialsBoost,
    ) {
        let Some(first) = candidates.first() else {
            return;
        };
        let coverage = first.pinyin.len();
        let end = candidates
            .iter()
            .take_while(|item| {
                item.pinyin.len() == coverage && !item.source.is_generated_or_fallback()
            })
            .count();
        let floor = if initials_lead {
            0
        } else {
            candidates[..end]
                .iter()
                .take(SYLLABLE_ROWS_BEFORE_INITIALS)
                .take_while(|item| !is_initials_row(item))
                .count()
        };
        let trigger_count = u32::try_from(boost.frequency.trigger_count.clamp(1, 10)).unwrap_or(1);
        let per_trigger = PERSONAL_PICK_TIMES * trigger_count;
        // (目标位置, 现在的位置)
        let mut moves = Vec::new();
        for (index, item) in candidates[..end].iter().enumerate() {
            if !is_initials_row(item) {
                continue;
            }
            let Some(&count) = boost.used.get(&item.word) else {
                continue;
            };
            let mut rank = index;
            // 每次触发至少挪一位（步长为 0 的线性模式除外，它一位也不挪），所以最多算 `index` 次。
            let triggers = usize::try_from(count / per_trigger).unwrap_or(usize::MAX);
            for _ in 0..triggers.min(index) {
                if rank == 0 {
                    break;
                }
                rank = ranking::ranking_target(
                    rank,
                    boost.frequency.mode,
                    boost.frequency.linear_step,
                    false,
                );
            }
            let target = rank.max(floor);
            if target < index {
                moves.push((target, index));
            }
        }
        if moves.is_empty() {
            return;
        }
        // 先从后往前取出要挪的行（前面的下标不受影响），再按目标位置从前往后放回；同一个位置先放原来靠前的，后来的顺延一位。
        let mut rows: Vec<(usize, usize, WordItem)> = moves
            .iter()
            .rev()
            .map(|&(target, index)| (target, index, candidates.remove(index)))
            .collect();
        rows.sort_by_key(|&(target, index, _)| (target, index));
        let mut next = 0;
        for (target, _, row) in rows {
            let at = target.max(next);
            candidates.insert(at, row);
            next = at + 1;
        }
    }
}
