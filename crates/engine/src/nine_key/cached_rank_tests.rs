use super::*;
use std::hint::black_box;
use std::time::Instant;

#[cfg(debug_assertions)]
std::thread_local! {
    pub(super) static KEY_EVALUATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// 固定 1e12fd8466 的完整排名正文；共用未改动的键、去重、交错和个人调频函数。
#[inline(never)]
fn reference(
    candidates: &mut Vec<WordItem>,
    prefer_exact: bool,
    initials_lead: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
) {
    candidates.sort_by_key(|item| rank_key(item, prefer_exact, initials_lead, grid));
    retain_unique_words(candidates);
    if !initials_lead {
        interleave_initials(candidates);
    }
    if let Some(boost) = boost {
        boost_used_initials(candidates, initials_lead, boost);
    }
    candidates.truncate(CANDIDATE_LIMIT);
}

#[inline(never)]
fn production(
    candidates: &mut Vec<WordItem>,
    prefer_exact: bool,
    initials_lead: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
) {
    rank_candidates(candidates, prefer_exact, initials_lead, boost, grid);
}

fn fixture(length: usize, tied: bool) -> Vec<WordItem> {
    let sources = [
        CandidateSource::Database,
        CandidateSource::UserDatabase,
        CandidateSource::Generated,
        CandidateSource::Fallback,
        CandidateSource::NeuralKeyboard,
        CandidateSource::EnglishDictionary,
    ];
    let readings = [
        "ni'hao",
        "ming'tian",
        "zhong'guo",
        "wo'yao'qu'na'li",
        "a",
        "",
        "lü'e",
        "'xi''an'",
    ];
    (0..length)
        .map(|index| {
            let mixed = (index * 73 + 17) % 101;
            let word = if index % 5 == 0 { index % 17 } else { index };
            let mut row = WordItem::new(
                if tied {
                    "64426"
                } else {
                    &"64426646"[..1 + mixed % 8]
                },
                format!("样例{word}"),
                if tied { 100 } else { mixed as i64 * 31 - 1000 },
                if tied {
                    CandidateSource::Database
                } else {
                    sources[mixed % sources.len()]
                },
                if tied {
                    "ni'hao'ma"
                } else {
                    readings[mixed % readings.len()]
                },
            );
            row.fixed_position = (index % 6) as i32;
            row.fuzzy = !tied && index % 7 == 0;
            row.corrected_from = format!("synthetic-{index}").into();
            row.sentence_association = index % 2 == 0;
            row.sentence_words = vec![format!("part-{index}")];
            row
        })
        .collect()
}

#[test]
#[cfg(debug_assertions)]
fn large_ranking_evaluates_each_key_at_most_once() {
    let mut rows = fixture(128, false);
    let length = rows.len();
    KEY_EVALUATIONS.with(|count| count.set(0));
    production(&mut rows, true, false, None, KeyGrid::NineKey);
    let count = KEY_EVALUATIONS.with(std::cell::Cell::get);
    assert!(count <= length, "排名重复求键：{count} 次，{length} 条候选");
}

#[test]
fn ranking_preserves_all_fields_ties_duplicates_and_boosts() {
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .expect("内置九键双拼表");
    let shuangpin = crate::nine_key_shuangpin::nine_key_shuangpin(&table).expect("合法表");
    let boost = InitialsBoost {
        used: (0..17)
            .map(|index| (format!("样例{index}").into(), 100))
            .collect(),
        frequency: FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Linear,
            trigger_count: 1,
            linear_step: 2,
        },
    };
    for length in [0, 1, 8, 32, 33, 64, 128, 512, 2048] {
        for tied in [false, true] {
            let input = fixture(length, tied);
            for grid in [
                KeyGrid::NineKey,
                KeyGrid::FourteenKey,
                KeyGrid::NineKeyShuangpin(shuangpin),
            ] {
                for prefer_exact in [false, true] {
                    for initials_lead in [false, true] {
                        for boost in [None, Some(&boost)] {
                            let mut expected = input.clone();
                            let mut actual = input.clone();
                            reference(&mut expected, prefer_exact, initials_lead, boost, grid);
                            production(&mut actual, prefer_exact, initials_lead, boost, grid);
                            assert_eq!(
                                actual, expected,
                                "length={length}, tied={tied}, grid={grid:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn ranking_preserves_active_interleaving_and_personal_boost() {
    let input: Vec<_> = (0..64)
        .map(|index| {
            let initials = index >= 48;
            let mut row = WordItem::new(
                "626",
                if initials {
                    format!("简拼{}", index - 48)
                } else {
                    format!("音节{index}")
                },
                if initials { 2000 + index } else { 1000 - index },
                CandidateSource::Database,
                if initials { "ni'hao'ma" } else { "man" },
            );
            row.fixed_position = (index % 6) as i32;
            row.corrected_from = format!("synthetic-{index}").into();
            row.sentence_words = vec![format!("part-{index}")];
            row
        })
        .collect();
    let boost = InitialsBoost {
        used: [(SharedString::from("简拼0"), 100)].into(),
        frequency: FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Linear,
            trigger_count: 1,
            linear_step: 2,
        },
    };
    for grid in [KeyGrid::NineKey, KeyGrid::FourteenKey] {
        for prefer_exact in [false, true] {
            for initials_lead in [false, true] {
                let mut raw = input.clone();
                raw.sort_by_key(|row| rank_key(row, prefer_exact, initials_lead, grid));
                let mut unboosted = input.clone();
                reference(&mut unboosted, prefer_exact, initials_lead, None, grid);
                if !initials_lead {
                    assert_ne!(raw, unboosted, "样本必须实际触发简拼交错");
                    assert!(!is_initials_row(
                        &unboosted[SYLLABLE_ROWS_BEFORE_INITIALS - 1]
                    ));
                    assert!(is_initials_row(&unboosted[SYLLABLE_ROWS_BEFORE_INITIALS]));
                }
                let mut boosted = input.clone();
                reference(
                    &mut boosted,
                    prefer_exact,
                    initials_lead,
                    Some(&boost),
                    grid,
                );
                assert_ne!(unboosted, boosted, "样本必须实际触发个人调频");
                let position =
                    |rows: &[WordItem]| rows.iter().position(|row| row.word == "简拼0").unwrap();
                assert!(position(&boosted) < position(&unboosted));
                for (boost, expected) in [(None, &unboosted), (Some(&boost), &boosted)] {
                    let mut actual = input.clone();
                    production(&mut actual, prefer_exact, initials_lead, boost, grid);
                    assert_eq!(&actual, expected);
                }
            }
        }
    }
}

#[test]
fn short_ranking_keeps_the_existing_allocation_budget() {
    for length in [0, 1, 8, 32] {
        let mut expected = fixture(length, false);
        let mut actual = expected.clone();
        let (_, old_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            reference(&mut expected, true, false, None, KeyGrid::NineKey);
        });
        let (_, new_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            production(&mut actual, true, false, None, KeyGrid::NineKey);
        });
        assert_eq!(actual, expected);
        assert_eq!(new_allocations, old_allocations, "length={length}");
    }
}

type Rank = fn(&mut Vec<WordItem>, bool, bool, Option<&InitialsBoost>, KeyGrid);

fn elapsed(rank: Rank, input: &[WordItem], repeats: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..repeats {
        // 两臂均包含输入复制、完整排名与结果析构，计时不只覆盖局部排序。
        let mut rows = black_box(input).to_vec();
        rank(black_box(&mut rows), true, false, None, KeyGrid::NineKey);
        black_box(rows);
    }
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "手动配对完整九键排名，不与构建、采样或其它计时并行"]
fn full_ranking_paired_budget() {
    let mut failures = Vec::new();
    println!("length tied repeats paired_ratio_p50");
    for length in [8, 32, 33, 64, 128, 512, 2048] {
        for tied in [false, true] {
            let input = fixture(length, tied);
            let mut expected = input.clone();
            let mut actual = input.clone();
            reference(&mut expected, true, false, None, KeyGrid::NineKey);
            production(&mut actual, true, false, None, KeyGrid::NineKey);
            assert_eq!(actual, expected);
            let mut repeats = 1;
            while elapsed(reference, &input, repeats) < 0.010 {
                repeats *= 2;
            }
            let mut ratios = Vec::new();
            for pair in 0..40 {
                let (old, new) = if pair % 2 == 0 {
                    let old = elapsed(reference, &input, repeats);
                    (old, elapsed(production, &input, repeats))
                } else {
                    let new = elapsed(production, &input, repeats);
                    (elapsed(reference, &input, repeats), new)
                };
                ratios.push(new / old);
            }
            ratios.sort_by(f64::total_cmp);
            let ratio = ratios[19];
            println!("{length} {tied} {repeats} {ratio:.4}");
            let budget = if length >= 64 { 0.98 } else { 1.05 };
            if ratio >= budget {
                failures.push((length, tied, ratio, budget));
            }
        }
    }
    assert!(failures.is_empty(), "完整排名配对预算失败：{failures:?}");
}
