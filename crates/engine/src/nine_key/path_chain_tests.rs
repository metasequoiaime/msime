use super::*;
use std::hint::black_box;
use std::time::Instant;

// 固定 465204211a 的完整切分正文，只更换接收者、移除注释。
#[inline(never)]
fn reference(
    table: &SpellingTable,
    digits: &str,
    splits: &[usize],
    prior: &SyllablePrior,
) -> Vec<Path> {
    let length = digits.len();
    let mut suffix: Vec<Vec<(f64, bool, Path)>> = vec![Vec::new(); length + 1];
    suffix[length].push((0.0, false, Vec::new()));
    for offset in (0..length).rev() {
        let mut result = Vec::new();
        for end in offset + 1..=length.min(offset + table.longest_code) {
            if splits.iter().any(|&split| offset < split && split < end) {
                break;
            }
            let Some(pieces) = table.by_code.get(&digits[offset..end]) else {
                continue;
            };
            let must_complete = end != length || splits.contains(&end);
            for (piece, complete) in pieces {
                if must_complete && !complete {
                    continue;
                }
                let score = prior.score(piece);
                for (tail_score, tail_complete, tail) in &suffix[end] {
                    let mut path = Vec::with_capacity(tail.len() + 1);
                    path.push(piece.clone());
                    path.extend(tail.iter().cloned());
                    if result.is_empty() {
                        result.reserve_exact(PATH_LIMIT);
                    }
                    let last_complete = if tail.is_empty() {
                        *complete
                    } else {
                        *tail_complete
                    };
                    result.push((score + tail_score, last_complete, path));
                }
            }
        }
        result.sort_by(|(a_score, a_complete, a), (b_score, b_complete, b)| {
            a.len()
                .cmp(&b.len())
                .then_with(|| b_complete.cmp(a_complete))
                .then_with(|| b_score.total_cmp(a_score))
                .then_with(|| a.cmp(b))
        });
        result.truncate(PATH_LIMIT);
        suffix[offset] = result;
    }
    suffix
        .swap_remove(0)
        .into_iter()
        .map(|(_, _, path)| path)
        .collect()
}

#[inline(never)]
fn production(
    table: &SpellingTable,
    digits: &str,
    splits: &[usize],
    prior: &SyllablePrior,
) -> Vec<Path> {
    table.split_paths(digits, splits, prior)
}

fn grids() -> [KeyGrid; 3] {
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    let shuangpin = crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap();
    [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(shuangpin),
    ]
}

fn weighted_prior(table: &SpellingTable) -> SyllablePrior<'_> {
    let weights = table
        .syllables
        .iter()
        .enumerate()
        .map(|(index, (syllable, _))| (syllable.clone(), ((index * 137) % 1021 + 1) as i64))
        .collect();
    SyllablePrior::from_weights(&table.syllables, &weights)
}

#[test]
fn paths_preserve_old_order_with_ties_priors_splits_and_grids() {
    for grid in grids() {
        let table = grid.spelling_table();
        let mut inputs: Vec<_> = [
            "",
            "o",
            "ni",
            "ni'hao",
            "xi'an",
            "zi'cong",
            "yi'dian'yi'dian",
            "wo'jin'tian'qu'shang'ban",
            "zhe'ci'kao'shi'de'ti'mu'you'dian'nan",
            "shi'jie",
            "lü'e",
            "zhong'guo",
        ]
        .into_iter()
        .map(|reading| grid.encode(reading))
        .collect();
        inputs.push("1".repeat(32));
        let mut alphabet: Vec<_> = table
            .syllables
            .iter()
            .flat_map(|(_, code)| code.chars())
            .collect();
        alphabet.sort_unstable();
        alphabet.dedup();
        let mut seed = 17usize;
        for length in [1, 2, 3, 4, 8, 16, 32, 42] {
            for _ in 0..2 {
                inputs.push(
                    (0..length)
                        .map(|_| {
                            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                            alphabet[seed % alphabet.len()]
                        })
                        .collect(),
                );
            }
        }
        for prior in [SyllablePrior::default(), weighted_prior(table)] {
            for digits in &inputs {
                let splits = [
                    Vec::new(),
                    vec![digits.len() / 2],
                    (1..digits.len()).collect(),
                    vec![digits.len()],
                ];
                for splits in &splits {
                    let expected = reference(table, digits, splits, &prior);
                    let actual = production(table, digits, splits, &prior);
                    assert_eq!(
                        actual, expected,
                        "grid={grid:?}, digits={digits}, splits={splits:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn intermediate_paths_do_not_materialize_owned_suffixes() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::default();
    let (expected, old) = crate::ime::personal_rerank::allocations::count(|| {
        reference(table, "666666666666", &[], &prior)
    });
    let (actual, new) = crate::ime::personal_rerank::allocations::count(|| {
        production(table, "666666666666", &[], &prior)
    });
    assert_eq!(actual, expected);
    assert!(new * 2 < old, "中间路径申请未减半：旧 {old}，新 {new}");
}

#[test]
fn empty_dead_and_short_paths_keep_the_allocation_budget() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::default();
    for digits in ["", "1", "11111111111111111111111111111111", "6", "64"] {
        let (expected, old) = crate::ime::personal_rerank::allocations::count(|| {
            reference(table, digits, &[], &prior)
        });
        let (actual, new) = crate::ime::personal_rerank::allocations::count(|| {
            production(table, digits, &[], &prior)
        });
        assert_eq!(actual, expected);
        assert!(new <= old, "短路径申请回退：{digits}，旧 {old}，新 {new}");
    }
}

type SplitPaths = fn(&SpellingTable, &str, &[usize], &SyllablePrior) -> Vec<Path>;

fn elapsed(
    rank: SplitPaths,
    table: &SpellingTable,
    digits: &str,
    splits: &[usize],
    prior: &SyllablePrior,
    repeats: usize,
) -> f64 {
    let start = Instant::now();
    for _ in 0..repeats {
        // 两臂都包括完整路径构造、排序、逐偏移截断、最终物化和结果析构。
        black_box(rank(
            black_box(table),
            black_box(digits),
            black_box(splits),
            black_box(prior),
        ));
    }
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "手动配对完整切分函数，不与构建、采样或其它计时并行"]
fn complete_split_paths_paired_budget() {
    let [nine, fourteen, shuangpin] = grids();
    let cases = [
        ("one", nine, "6".to_string(), Vec::new(), false),
        ("two", nine, "64".to_string(), Vec::new(), false),
        ("word", nine, "64426".to_string(), Vec::new(), true),
        ("repeat", nine, "943426943426".to_string(), Vec::new(), true),
        ("dense", nine, "666666666666".to_string(), Vec::new(), true),
        ("split", nine, "64426".to_string(), vec![2], true),
        (
            "fourteen",
            fourteen,
            fourteen.encode("wo'jin'tian'qu'shang'ban"),
            Vec::new(),
            true,
        ),
        (
            "shuangpin",
            shuangpin,
            shuangpin.encode("wo'jin'tian'qu'shang'ban"),
            Vec::new(),
            true,
        ),
    ];
    let mut failures = Vec::new();
    println!("shape weighted repeats paired_ratio_p50");
    for (name, grid, digits, splits, positive) in cases {
        let table = grid.spelling_table();
        for (weighted, prior) in [SyllablePrior::default(), weighted_prior(table)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                production(table, &digits, &splits, &prior),
                reference(table, &digits, &splits, &prior)
            );
            let mut repeats = 1;
            while elapsed(reference, table, &digits, &splits, &prior, repeats) < 0.010 {
                repeats *= 2;
            }
            let mut ratios = Vec::new();
            for pair in 0..40 {
                let (old, new) = if pair % 2 == 0 {
                    let old = elapsed(reference, table, &digits, &splits, &prior, repeats);
                    (
                        old,
                        elapsed(production, table, &digits, &splits, &prior, repeats),
                    )
                } else {
                    let new = elapsed(production, table, &digits, &splits, &prior, repeats);
                    (
                        elapsed(reference, table, &digits, &splits, &prior, repeats),
                        new,
                    )
                };
                ratios.push(new / old);
            }
            ratios.sort_by(f64::total_cmp);
            let ratio = ratios[19];
            println!("{name} {} {repeats} {ratio:.4}", weighted != 0);
            let budget = if positive { 0.95 } else { 1.05 };
            if ratio >= budget {
                failures.push((name, weighted, ratio, budget));
            }
        }
    }
    assert!(failures.is_empty(), "完整切分配对预算失败：{failures:?}");
}

#[test]
#[ignore = "四轮端到端结果有漂移，单独配对评测集全部实际按键前缀"]
fn evaluation_prefixes_complete_split_paths_paired_budget() {
    let grid = KeyGrid::NineKey;
    let table = grid.spelling_table();
    let inputs: Vec<String> = include_str!("../../../../resources/eval/sentences-nine-key-v1.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty() && !line.starts_with("id\t"))
        .flat_map(|line| {
            let digits = grid.encode(line.split('\t').nth(1).unwrap());
            (1..=digits.len())
                .map(|end| digits[..end].to_owned())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(inputs.len(), 1791);
    let elapsed_batch = |split: SplitPaths, prior: &SyllablePrior, repeats: usize| {
        let start = Instant::now();
        for _ in 0..repeats {
            for digits in &inputs {
                // 全部实际按键前缀均计入，包含最终结果的析构。
                black_box(split(
                    black_box(table),
                    black_box(digits),
                    black_box(&[]),
                    black_box(prior),
                ));
            }
        }
        start.elapsed().as_secs_f64()
    };
    let mut failures = Vec::new();
    for (weighted, prior) in [SyllablePrior::default(), weighted_prior(table)]
        .into_iter()
        .enumerate()
    {
        for digits in &inputs {
            assert_eq!(
                production(table, digits, &[], &prior),
                reference(table, digits, &[], &prior)
            );
        }
        let mut repeats = 1;
        while elapsed_batch(reference, &prior, repeats) < 0.010 {
            repeats *= 2;
        }
        let mut ratios = Vec::new();
        for pair in 0..40 {
            let (old, new) = if pair % 2 == 0 {
                let old = elapsed_batch(reference, &prior, repeats);
                (old, elapsed_batch(production, &prior, repeats))
            } else {
                let new = elapsed_batch(production, &prior, repeats);
                (elapsed_batch(reference, &prior, repeats), new)
            };
            ratios.push(new / old);
        }
        ratios.sort_by(f64::total_cmp);
        let ratio = ratios[19];
        println!(
            "evaluation_prefixes weighted={} count={} repeats={repeats} paired_ratio_p50={ratio:.4}",
            weighted != 0,
            inputs.len()
        );
        if ratio >= 1.0 {
            failures.push((weighted, ratio));
        }
    }
    assert!(
        failures.is_empty(),
        "实际按键前缀完整切分回退：{failures:?}"
    );
}
