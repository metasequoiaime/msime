use super::*;
use crate::ime::personal_rerank::allocations::count;

// 冻结 dd0b58ea16 的种子追加函数，只修改名称；末音节锚定的那条尾段是之后有意新增的语义，按同样规则补上。
fn frozen_seeded_paths(
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
        let mut tails: Vec<Path> = if tail.is_empty() {
            vec![Vec::new()]
        } else {
            table
                .split_paths_limited(tail, &[], prior, SEED_TAIL_LIMIT)
                .into_iter()
                .take(SEED_TAIL_LIMIT)
                .collect()
        };
        let (_, full_covered, full_fits) = fitting_syllables(table.grid, seed, remaining);
        if !tail.is_empty() && full_fits {
            let rest = &remaining[full_covered..];
            let rest_path = if rest.is_empty() {
                Some(Vec::new())
            } else {
                table
                    .split_paths_limited(rest, &[], prior, 1)
                    .into_iter()
                    .next()
            };
            if let Some(rest_path) = rest_path {
                let mut anchored = vec![seed.rsplit('\'').next().unwrap().to_owned()];
                anchored.extend(rest_path);
                tails.push(anchored);
            }
        }
        for tail_path in tails {
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
    let profile =
        crate::nine_key_shuangpin::builtin_table(crate::types::NineKeyShuangpinKind::Migala2)
            .unwrap();
    [
        KeyGrid::NineKey,
        KeyGrid::FourteenKey,
        KeyGrid::NineKeyShuangpin(crate::nine_key_shuangpin::nine_key_shuangpin(&profile).unwrap()),
    ]
}

#[test]
fn complete_duplicate_seed_needs_no_path_allocations() {
    for grid in grids() {
        let table = grid.spelling_table();
        let digits = grid.encode("ni'hao");
        let seeds = vec!["ni'hao'ma".to_owned()];
        let initial = vec![vec!["ni".to_owned(), "hao".to_owned()]];
        let mut old = initial.clone();
        let mut new = initial;
        let (_, before) = count(|| {
            frozen_seeded_paths(&mut old, &seeds, &digits, table, &SyllablePrior::default())
        });
        let (_, after) =
            count(|| seeded_paths(&mut new, &seeds, &digits, table, &SyllablePrior::default()));
        assert_eq!(new, old);
        eprintln!("完整重复种子 {grid:?}：{before} → {after}");
        assert_eq!(after, 0, "重复种子不能先分配完整路径再丢弃");
    }
}

#[test]
fn duplicate_seed_tails_skip_prefix_materialization() {
    for grid in grids() {
        let table = grid.spelling_table();
        let prior = SyllablePrior::default();
        let digits = grid.encode("wo'jin'tian'qu'shang'ban");
        let seeds = vec!["wo'jin'tian".to_owned()];
        let mut initial = Vec::new();
        frozen_seeded_paths(&mut initial, &seeds, &digits, table, &prior);
        assert!(!initial.is_empty());
        let savings = initial.len() * 3;
        let mut old = initial.clone();
        let mut new = initial;
        let (_, before) = count(|| frozen_seeded_paths(&mut old, &seeds, &digits, table, &prior));
        let (_, after) = count(|| seeded_paths(&mut new, &seeds, &digits, table, &prior));
        assert_eq!(new, old);
        eprintln!("重复尾段 {grid:?}：{before} → {after}，至少省 {savings}");
        assert!(
            after + savings <= before,
            "重复尾段不能复制前缀和建立组合路径"
        );
    }
}

#[test]
fn seeded_dedup_matches_frozen_order_and_allocation_budget() {
    for grid in grids() {
        let table = grid.spelling_table();
        let weights = table
            .syllables
            .iter()
            .enumerate()
            .map(|(i, (text, _))| (text.clone(), (i * 137 % 1021 + 1) as i64))
            .collect();
        for prior in [
            SyllablePrior::default(),
            SyllablePrior::from_weights(&table.syllables, &weights),
        ] {
            for reading in [
                "",
                "ni",
                "ni'hao",
                "ni'hao'ma",
                "yi'dian'yi'dian",
                "wo'jin'tian'qu'shang'ban",
            ] {
                let digits = grid.encode(reading);
                for seeds in [
                    vec![],
                    vec!["ni".to_owned()],
                    vec!["hao'ma".to_owned()],
                    vec!["ni'hao'ma".to_owned()],
                    vec![
                        "wo'jin'tian".to_owned(),
                        "wo'jin'tian".to_owned(),
                        "wo'jin'tian'qu'shang'ban'ma".to_owned(),
                    ],
                    vec![
                        "yi'dian'yi'dian".to_owned(),
                        "yi'dian'yi'dian'ma".to_owned(),
                        "ni'hao".to_owned(),
                    ],
                    vec!["".to_owned(), "ni''hao".to_owned(), "invalid'ma".to_owned()],
                ] {
                    for populated in 0..3 {
                        let initial = if populated == 0 {
                            Vec::new()
                        } else {
                            table.split_paths(&digits, &[], &prior)
                        };
                        let mut old = initial.clone();
                        if populated == 2 {
                            old.reverse();
                        }
                        let mut new = old.clone();
                        for _ in 0..2 {
                            let (_, before) = count(|| {
                                frozen_seeded_paths(&mut old, &seeds, &digits, table, &prior)
                            });
                            let (_, after) =
                                count(|| seeded_paths(&mut new, &seeds, &digits, table, &prior));
                            assert_eq!(new, old, "{grid:?}/{reading}/{seeds:?}/{populated}");
                            assert!(
                                after <= before,
                                "{grid:?}/{reading}/{seeds:?}/{populated}: {after}>{before}"
                            );
                        }
                    }
                }
            }
        }
    }
}

type Append = fn(&mut Vec<Path>, &[String], &str, &SpellingTable, &SyllablePrior);

fn elapsed(
    append: Append,
    initial: &[Path],
    seeds: &[String],
    digits: &str,
    table: &SpellingTable,
    prior: &SyllablePrior,
    repeats: usize,
) -> f64 {
    let mut paths = initial.to_vec();
    let start = std::time::Instant::now();
    for _ in 0..repeats {
        append(
            &mut paths,
            std::hint::black_box(seeds),
            std::hint::black_box(digits),
            std::hint::black_box(table),
            std::hint::black_box(prior),
        );
        std::hint::black_box(&paths);
        paths.truncate(initial.len());
    }
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "手动 release 配对；不与构建或其他性能测试同时运行"]
fn seeded_dedup_paired_budget() {
    let mut failures = Vec::new();
    for (grid_index, grid) in grids().into_iter().enumerate() {
        let table = grid.spelling_table();
        let weights = table
            .syllables
            .iter()
            .enumerate()
            .map(|(i, (text, _))| (text.clone(), (i * 137 % 1021 + 1) as i64))
            .collect();
        for (weighted, prior) in [
            SyllablePrior::default(),
            SyllablePrior::from_weights(&table.syllables, &weights),
        ]
        .into_iter()
        .enumerate()
        {
            for (shape, (reading, seed)) in [
                ("ni", "hao'ma"),
                ("ni", "ni'hao"),
                ("ni'hao", "ni'hao"),
                ("ni'hao'ma", "ni'hao"),
                ("wo'jin'tian'qu'shang'ban", "wo'jin'tian"),
                ("yi'dian'yi'dian", "yi'dian'yi'dian'ma"),
            ]
            .into_iter()
            .enumerate()
            {
                let digits = grid.encode(reading);
                let seeds = vec![seed.to_owned()];
                for populated in [false, true] {
                    let initial = if populated {
                        table.split_paths(&digits, &[], &prior)
                    } else {
                        Vec::new()
                    };
                    let mut expected = initial.clone();
                    frozen_seeded_paths(&mut expected, &seeds, &digits, table, &prior);
                    let mut actual = initial.clone();
                    seeded_paths(&mut actual, &seeds, &digits, table, &prior);
                    assert_eq!(actual, expected);
                    let mut repeats = 1;
                    while elapsed(
                        frozen_seeded_paths,
                        &initial,
                        &seeds,
                        &digits,
                        table,
                        &prior,
                        repeats,
                    ) < 0.005
                    {
                        repeats *= 2;
                    }
                    let mut ratios = Vec::new();
                    for pair in 0..40 {
                        let old = || {
                            elapsed(
                                frozen_seeded_paths,
                                &initial,
                                &seeds,
                                &digits,
                                table,
                                &prior,
                                repeats,
                            )
                        };
                        let new = || {
                            elapsed(
                                seeded_paths,
                                &initial,
                                &seeds,
                                &digits,
                                table,
                                &prior,
                                repeats,
                            )
                        };
                        let (before, after) = if pair % 2 == 0 {
                            (old(), new())
                        } else {
                            let after = new();
                            (old(), after)
                        };
                        let ratio = after / before;
                        ratios.push(ratio);
                        println!("pair {grid_index} {weighted} {shape} {populated} {pair} {before:.9} {after:.9} {ratio:.6}");
                    }
                    ratios.sort_by(f64::total_cmp);
                    let median = (ratios[19] + ratios[20]) / 2.0;
                    let budget = if populated && (shape == 1 || shape == 5) {
                        0.98
                    } else {
                        1.05
                    };
                    println!("median {grid_index} {weighted} {shape} {populated} {median:.6} budget {budget}");
                    if median >= budget {
                        failures.push(format!(
                            "{grid_index}/{weighted}/{shape}/{populated}: {median:.6} >= {budget}"
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn seeded_dedup_preserves_prefixes_beyond_the_session_digit_limit() {
    for syllables in [
        DIGIT_LIMIT - 1,
        DIGIT_LIMIT,
        DIGIT_LIMIT + 1,
        DIGIT_LIMIT * 2,
    ] {
        let reading = std::iter::repeat_n("ni", syllables)
            .collect::<Vec<_>>()
            .join("'");
        let seeds = vec![format!("{reading}'hao")];
        for grid in grids() {
            let table = grid.spelling_table();
            let digits = grid.encode(&reading);
            let mut old = Vec::new();
            let mut new = Vec::new();
            for _ in 0..2 {
                frozen_seeded_paths(&mut old, &seeds, &digits, table, &SyllablePrior::default());
                seeded_paths(&mut new, &seeds, &digits, table, &SyllablePrior::default());
                assert_eq!(new, old);
            }
            assert_eq!(new.len(), 1);
            assert_eq!(new[0].len(), syllables);
        }
    }
}
