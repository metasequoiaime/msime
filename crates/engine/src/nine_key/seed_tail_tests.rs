use super::*;
use std::hint::black_box;
use std::time::Instant;

// 固定 8ef7f5a4d4 的完整切分与种子追加正文，仅改名、接收者和内部调用，移除注释。
fn reference_split_paths(
    table: &SpellingTable,
    digits: &str,
    splits: &[usize],
    prior: &SyllablePrior,
) -> Vec<Path> {
    let length = digits.len();
    let mut suffix: Vec<Vec<SpellingPath<'_>>> = vec![Vec::new(); length + 1];
    suffix[length].push(SpellingPath {
        score: 0.0,
        complete: false,
        piece: "",
        next: (length, 0),
        length: 0,
    });
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
                for (row, tail) in suffix[end].iter().enumerate() {
                    if result.is_empty() {
                        result.reserve_exact(PATH_LIMIT);
                    }
                    result.push(SpellingPath {
                        score: score + tail.score,
                        complete: if tail.length == 0 {
                            *complete
                        } else {
                            tail.complete
                        },
                        piece,
                        next: (end, row),
                        length: tail.length + 1,
                    });
                }
            }
        }
        result.sort_by(|a, b| {
            a.length
                .cmp(&b.length)
                .then_with(|| b.complete.cmp(&a.complete))
                .then_with(|| b.score.total_cmp(&a.score))
                .then_with(|| a.pieces(&suffix).cmp(b.pieces(&suffix)))
        });
        result.truncate(PATH_LIMIT);
        suffix[offset] = result;
    }
    std::mem::take(&mut suffix[0])
        .into_iter()
        .map(|root| {
            let mut path = Vec::with_capacity(root.length);
            path.extend(root.pieces(&suffix).map(str::to_owned));
            path
        })
        .collect()
}

fn reference_seeded_paths(
    alternatives: &mut Vec<Path>,
    seeds: &[String],
    remaining: &str,
    table: &SpellingTable,
    prior: &SyllablePrior,
) {
    for seed in seeds {
        let (kept, covered, _) = fitting_syllables(table.grid, seed_prefix(seed), remaining);
        if kept == 0 {
            continue;
        }
        let tail = &remaining[covered..];
        let tails = if tail.is_empty() {
            vec![Vec::new()]
        } else {
            reference_split_paths(table, tail, &[], prior)
        };
        for tail_path in tails.into_iter().take(SEED_TAIL_LIMIT) {
            let mut path: Path = Vec::with_capacity(kept + tail_path.len());
            path.extend(seed.split('\'').take(kept).map(str::to_owned));
            path.extend(tail_path);
            if !alternatives.contains(&path) {
                alternatives.push(path);
            }
        }
    }
}

fn grids() -> [KeyGrid; 3] {
    let table =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap()),
    ]
}

fn prior(table: &SpellingTable, weighted: bool) -> SyllablePrior<'_> {
    if !weighted {
        return SyllablePrior::default();
    }
    let weights = table
        .syllables
        .iter()
        .enumerate()
        .map(|(index, (text, _))| (text.clone(), ((index * 137) % 1021 + 1) as i64))
        .collect();
    SyllablePrior::from_weights(&table.syllables, &weights)
}

fn cases() -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("", vec![]),
        ("ni", vec!["hao'ma".into()]),
        ("ni", vec!["ni'hao".into()]),
        ("ni'hao", vec!["ni'hao".into()]),
        ("ni'hao'ma", vec!["ni'hao".into()]),
        ("wo'jin'tian'qu'shang'ban", vec!["wo'jin'tian".into()]),
        (
            "yi'dian'yi'dian",
            vec!["yi'dian".into(), "yi'dian'yi'dian".into()],
        ),
        (
            "wo'jin'tian'qu'shang'ban",
            vec![
                "wo'jin'tian".into(),
                "wo'jin'tian".into(),
                "wo'jin'tian'qu'shang'ban".into(),
            ],
        ),
    ]
}

#[test]
fn seeded_tail_preserves_frozen_output_and_allocation_budget() {
    for grid in grids() {
        let table = grid.spelling_table();
        for weighted in [false, true] {
            let prior = prior(table, weighted);
            for (reading, seeds) in cases() {
                let digits = grid.encode(reading);
                for populated in [false, true] {
                    let mut old = if populated {
                        table.split_paths(&digits, &[], &prior)
                    } else {
                        Vec::new()
                    };
                    let mut new = old.clone();
                    let (_, before) = crate::ime::personal_rerank::allocations::count(|| {
                        reference_seeded_paths(&mut old, &seeds, &digits, table, &prior)
                    });
                    let (_, after) = crate::ime::personal_rerank::allocations::count(|| {
                        seeded_paths(&mut new, &seeds, &digits, table, &prior)
                    });
                    assert_eq!(new, old, "{grid:?}/{reading}/{weighted}/{populated}");
                    assert!(after <= before, "{grid:?}/{reading}: {after}>{before}");
                }
            }
        }
    }
}

#[test]
fn seeded_tail_does_not_materialize_discarded_paths() {
    let grid = KeyGrid::NineKey;
    let table = grid.spelling_table();
    let prior = SyllablePrior::default();
    let digits = grid.encode("wo'jin'tian'qu'shang'ban");
    let seeds = vec!["wo'jin'tian".into()];
    let mut old = Vec::new();
    let mut new = Vec::new();
    let (_, before) = crate::ime::personal_rerank::allocations::count(|| {
        reference_seeded_paths(&mut old, &seeds, &digits, table, &prior)
    });
    let (_, after) = crate::ime::personal_rerank::allocations::count(|| {
        seeded_paths(&mut new, &seeds, &digits, table, &prior)
    });
    assert_eq!(new, old);
    let (_, covered, _) = fitting_syllables(grid, seed_prefix(&seeds[0]), &digits);
    let discarded = reference_split_paths(table, &digits[covered..], &[], &prior);
    let savings: usize = discarded
        .iter()
        .skip(SEED_TAIL_LIMIT)
        .map(|path| 1 + path.len())
        .sum();
    assert!(
        after + savings <= before,
        "至少消除未采用路径的向量与音节申请"
    );
    println!(
        "种子尾段分配：旧 {before}，新 {after}，未物化字段预算 {savings}，共省 {}",
        before - after
    );
    assert!(
        after + 100 <= before,
        "尾段多余物化未消除：旧 {before}，新 {after}"
    );
}

type Append = fn(&mut Vec<Path>, &[String], &str, &SpellingTable, &SyllablePrior);
fn elapsed(
    append: Append,
    seeds: &[String],
    digits: &str,
    table: &SpellingTable,
    prior: &SyllablePrior,
    repeats: usize,
) -> f64 {
    let start = Instant::now();
    for _ in 0..repeats {
        let mut paths = Vec::new();
        append(
            &mut paths,
            black_box(seeds),
            black_box(digits),
            black_box(table),
            black_box(prior),
        );
        black_box(paths);
    }
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "手动普通系统分配器完整配对，不与构建和采样并行"]
fn seeded_tail_paired_budget() {
    let mut failures = Vec::new();
    println!("grid weighted shape pair old_seconds new_seconds ratio");
    for grid in grids() {
        let table = grid.spelling_table();
        for weighted in [false, true] {
            let prior = prior(table, weighted);
            for (shape, (reading, seeds)) in cases().into_iter().enumerate() {
                let digits = grid.encode(reading);
                let mut old = Vec::new();
                let mut new = Vec::new();
                reference_seeded_paths(&mut old, &seeds, &digits, table, &prior);
                seeded_paths(&mut new, &seeds, &digits, table, &prior);
                assert_eq!(old, new);
                let mut repeats = 1;
                while elapsed(
                    reference_seeded_paths,
                    &seeds,
                    &digits,
                    table,
                    &prior,
                    repeats,
                ) < 0.010
                {
                    repeats *= 2;
                }
                let mut ratios = Vec::new();
                for pair in 0..40 {
                    let (old, new) = if pair % 2 == 0 {
                        let old = elapsed(
                            reference_seeded_paths,
                            &seeds,
                            &digits,
                            table,
                            &prior,
                            repeats,
                        );
                        (
                            old,
                            elapsed(seeded_paths, &seeds, &digits, table, &prior, repeats),
                        )
                    } else {
                        let new = elapsed(seeded_paths, &seeds, &digits, table, &prior, repeats);
                        (
                            elapsed(
                                reference_seeded_paths,
                                &seeds,
                                &digits,
                                table,
                                &prior,
                                repeats,
                            ),
                            new,
                        )
                    };
                    println!(
                        "{grid:?} {weighted} {shape} {pair} {old:.9} {new:.9} {:.6}",
                        new / old
                    );
                    ratios.push(new / old);
                }
                ratios.sort_by(f64::total_cmp);
                let median = (ratios[19] + ratios[20]) / 2.0;
                let budget = if shape >= 4 { 0.98 } else { 1.05 };
                println!("SUMMARY {grid:?} {weighted} {shape} {median:.6} budget {budget}");
                if median >= budget {
                    failures.push(format!("{grid:?}/{weighted}/{shape}: {median}>={budget}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn final_limit_keeps_full_search_order_and_independent_paths() {
    for grid in grids() {
        let table = grid.spelling_table();
        for weighted in [false, true] {
            let prior = prior(table, weighted);
            let mut inputs: Vec<_> = [
                "",
                "ni",
                "ni'hao",
                "wo'jin'tian'qu'shang'ban",
                "yi'dian'yi'dian",
            ]
            .into_iter()
            .map(|reading| grid.encode(reading))
            .collect();
            let mut alphabet: Vec<_> = table
                .syllables
                .iter()
                .flat_map(|(_, code)| code.chars())
                .collect();
            alphabet.sort_unstable();
            alphabet.dedup();
            let mut state = 17usize;
            for length in [1, 2, 4, 8, 16, 32, 42] {
                for _ in 0..4 {
                    inputs.push(
                        (0..length)
                            .map(|_| {
                                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                                alphabet[state % alphabet.len()]
                            })
                            .collect(),
                    );
                }
            }
            for digits in inputs {
                for splits in [
                    Vec::new(),
                    vec![digits.len() / 2],
                    (1..digits.len()).collect(),
                ] {
                    let expected = reference_split_paths(table, &digits, &splits, &prior);
                    for limit in [0, 1, 3, PATH_LIMIT, PATH_LIMIT + 1] {
                        let mut actual = table.split_paths_limited(&digits, &splits, &prior, limit);
                        assert_eq!(
                            actual,
                            expected.iter().take(limit).cloned().collect::<Vec<_>>()
                        );
                        if let Some(first) = actual.first_mut().and_then(|path| path.first_mut()) {
                            first.push_str("独立副本");
                        }
                        assert_eq!(table.split_paths(&digits, &splits, &prior), expected);
                    }
                }
            }
        }
    }
}
