use super::*;
use crate::ime::personal_rerank::allocations::count;

// 冻结 9878ab5a12 的完整输出缓冲切分函数，仅更改名称与接收者。
fn frozen_into(
    table: &SpellingTable,
    digits: &str,
    splits: &[usize],
    prior: &SyllablePrior,
    output_limit: usize,
    destination: &mut Vec<Path>,
) {
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
        // 按码分组访问音节；比较项与原拥有路径相同，只在需要字典序时回溯音节。
        let compare = |a: &SpellingPath<'_>, b: &SpellingPath<'_>| {
            a.length
                .cmp(&b.length)
                .then_with(|| b.complete.cmp(&a.complete))
                .then_with(|| b.score.total_cmp(&a.score))
                .then_with(|| a.pieces(&suffix).cmp(b.pieces(&suffix)))
        };
        let keep = if offset == 0 {
            output_limit.min(PATH_LIMIT)
        } else {
            PATH_LIMIT
        };
        // 根页没有前驱消费者；同分同读法的路径输出相同，只选择最终采用的前几条。
        if keep < PATH_LIMIT && result.len() > keep {
            result.select_nth_unstable_by(keep, compare);
            result.truncate(keep);
        }
        result.sort_by(compare);
        result.truncate(keep);
        suffix[offset] = result;
    }
    // 后继一定在更大偏移，取出根页不影响回溯；只有最终保留路径才物化字符串。
    let roots = std::mem::take(&mut suffix[0]);
    if destination.is_empty() {
        *destination = roots
            .into_iter()
            .take(output_limit)
            .map(|root| {
                let mut path = Vec::with_capacity(root.length);
                path.extend(root.pieces(&suffix).map(str::to_owned));
                path
            })
            .collect();
        return;
    }
    let output_len = roots.len().min(output_limit);
    destination.reserve(output_len.saturating_sub(destination.len()));
    destination.truncate(output_len);
    for (index, root) in roots.into_iter().take(output_limit).enumerate() {
        let path = if let Some(path) = destination.get_mut(index) {
            path
        } else {
            destination.push(Vec::with_capacity(root.length));
            destination.last_mut().expect("new path")
        };
        path.truncate(root.length);
        path.reserve(root.length.saturating_sub(path.len()));
        for (piece_index, piece) in root.pieces(&suffix).enumerate() {
            if piece_index == path.len() {
                path.push(String::new());
            }
            path[piece_index].clear();
            path[piece_index].push_str(piece);
        }
        path.truncate(root.length);
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

fn weighted_prior(table: &SpellingTable) -> SyllablePrior<'_> {
    let weights = table
        .syllables
        .iter()
        .enumerate()
        .map(|(i, (text, _))| (text.clone(), (i * 137 % 1021 + 1) as i64))
        .collect();
    SyllablePrior::from_weights(&table.syllables, &weights)
}

#[test]
fn warm_path_search_reuses_page_storage() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = weighted_prior(table);
    let digits = "666666666666";
    let mut old = Vec::new();
    let mut new = Vec::new();
    let mut scratch = PathScratch::default();
    frozen_into(table, digits, &[], &prior, PATH_LIMIT, &mut old);
    table.split_paths_with_scratch(digits, &[], &prior, PATH_LIMIT, &mut scratch, &mut new);
    let (_, before) = count(|| frozen_into(table, digits, &[], &prior, PATH_LIMIT, &mut old));
    let (_, after) = count(|| {
        table.split_paths_with_scratch(digits, &[], &prior, PATH_LIMIT, &mut scratch, &mut new)
    });
    let (_, steady) = count(|| {
        table.split_paths_with_scratch(digits, &[], &prior, PATH_LIMIT, &mut scratch, &mut new)
    });
    assert_eq!(new, old);
    eprintln!("密集路径热切分：{before} → 首次热态 {after} → 稳定热态 {steady}");
    assert!(steady <= 5, "页复用后仍有额外容器申请：{steady}");
    let retained = scratch.pages.capacity() * std::mem::size_of::<Vec<SpellingPath>>()
        + scratch
            .pages
            .iter()
            .map(|page| page.capacity() * std::mem::size_of::<SpellingPath>())
            .sum::<usize>();
    eprintln!("密集样例保留页槽位字节：{retained}");
    assert!(
        after < before,
        "热切分仍重新申请页容器：{after} >= {before}"
    );
}

#[test]
fn scratch_preserves_frozen_output_and_allocation_budget() {
    for grid in grids() {
        let table = grid.spelling_table();
        let mut inputs: Vec<String> = [
            "",
            "o",
            "ni",
            "ni'hao",
            "xi'an",
            "yi'dian'yi'dian",
            "wo'jin'tian'qu'shang'ban",
        ]
        .into_iter()
        .map(|reading| grid.encode(reading))
        .collect();
        inputs.extend(["666666666666".to_owned(), "1".repeat(32), "6!".to_owned()]);
        let alphabet: Vec<u8> = table
            .syllables
            .iter()
            .flat_map(|(_, code)| code.bytes())
            .collect();
        let mut state = 17usize;
        for length in [1, 2, 3, 4, 8, 16, 32, 42] {
            for _ in 0..4 {
                inputs.push(
                    (0..length)
                        .map(|_| {
                            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                            alphabet[state % alphabet.len()] as char
                        })
                        .collect(),
                );
            }
        }
        for prior in [SyllablePrior::default(), weighted_prior(table)] {
            let mut old = Vec::new();
            let mut new = Vec::new();
            let mut scratch = PathScratch::default();
            for digits in &inputs {
                for splits in [
                    vec![],
                    vec![digits.len() / 2],
                    vec![digits.len()],
                    (1..digits.len()).collect(),
                ] {
                    for limit in [0, 1, 3, PATH_LIMIT, PATH_LIMIT + 1] {
                        for _ in 0..2 {
                            let (_, before) = count(|| {
                                frozen_into(table, digits, &splits, &prior, limit, &mut old)
                            });
                            let (_, after) = count(|| {
                                table.split_paths_with_scratch(
                                    digits,
                                    &splits,
                                    &prior,
                                    limit,
                                    &mut scratch,
                                    &mut new,
                                )
                            });
                            assert_eq!(new, old, "{grid:?}/{digits}/{splits:?}/{limit}");
                            assert_eq!(new.capacity(), old.capacity());
                            assert!(
                                after <= before,
                                "{grid:?}/{digits}/{limit}: {after}>{before}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn scratch_recomputes_pages_after_grid_prior_and_length_changes() {
    let grids = grids();
    let mut scratch = PathScratch::default();
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    for step in 0..48 {
        let grid = grids[step % grids.len()];
        let table = grid.spelling_table();
        let prior = if step % 2 == 0 {
            SyllablePrior::default()
        } else {
            weighted_prior(table)
        };
        let reading = ["ni'hao", "o", "yi'dian'yi'dian", "wo'jin'tian'qu'shang'ban"][step % 4];
        let digits = grid.encode(reading);
        let splits = if step % 3 == 0 {
            vec![digits.len()]
        } else {
            vec![]
        };
        let limit = [PATH_LIMIT, 3, 0, PATH_LIMIT + 1][step % 4];
        frozen_into(table, &digits, &splits, &prior, limit, &mut expected);
        table.split_paths_with_scratch(&digits, &splits, &prior, limit, &mut scratch, &mut actual);
        assert_eq!(actual, expected, "跨方案调用 {step}");
        assert_eq!(scratch.pages.len(), digits.len() + 1);
    }
}

type Split = fn(
    &'static SpellingTable,
    &str,
    &[usize],
    &SyllablePrior,
    usize,
    &mut PathScratch<'static>,
    &mut Vec<Path>,
);

fn before(
    table: &'static SpellingTable,
    digits: &str,
    splits: &[usize],
    prior: &SyllablePrior,
    limit: usize,
    _scratch: &mut PathScratch<'static>,
    paths: &mut Vec<Path>,
) {
    frozen_into(table, digits, splits, prior, limit, paths);
}

struct Case {
    table: &'static SpellingTable,
    digits: String,
    splits: Vec<usize>,
}

fn elapsed(
    split: Split,
    case: &Case,
    prior: &SyllablePrior,
    limit: usize,
    warm: bool,
    repeats: usize,
) -> f64 {
    let mut paths = Vec::new();
    let mut scratch = PathScratch::default();
    if warm {
        for _ in 0..2 {
            split(
                case.table,
                &case.digits,
                &case.splits,
                prior,
                limit,
                &mut scratch,
                &mut paths,
            );
        }
    }
    let start = std::time::Instant::now();
    for _ in 0..repeats {
        split(
            std::hint::black_box(case.table),
            std::hint::black_box(&case.digits),
            std::hint::black_box(&case.splits),
            std::hint::black_box(prior),
            limit,
            &mut scratch,
            &mut paths,
        );
        std::hint::black_box(&paths);
        if !warm {
            drop(std::mem::take(&mut paths));
            scratch = PathScratch::default();
        }
    }
    drop(paths);
    drop(scratch);
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "手动普通System分配器冷热配对，不与构建或其它计时并行"]
fn path_scratch_paired_budget() {
    let mut failures = Vec::new();
    for (grid_index, grid) in grids().into_iter().enumerate() {
        let table = grid.spelling_table();
        for (weighted, prior) in [SyllablePrior::default(), weighted_prior(table)]
            .into_iter()
            .enumerate()
        {
            for (shape, (digits, splits)) in [
                (grid.encode("o"), vec![]),
                (grid.encode("ni"), vec![]),
                (grid.encode("ni'hao"), vec![]),
                (grid.encode("yi'dian'yi'dian"), vec![]),
                ("666666666666".to_owned(), vec![]),
                (grid.encode("wo'jin'tian'qu'shang'ban"), vec![]),
                (grid.encode("ni'hao"), vec![2]),
            ]
            .into_iter()
            .enumerate()
            {
                let case = Case {
                    table,
                    digits,
                    splits,
                };
                for limit in [3, PATH_LIMIT] {
                    for warm in [false, true] {
                        let mut expected = Vec::new();
                        let mut actual = Vec::new();
                        frozen_into(
                            table,
                            &case.digits,
                            &case.splits,
                            &prior,
                            limit,
                            &mut expected,
                        );
                        table.split_paths_with_scratch(
                            &case.digits,
                            &case.splits,
                            &prior,
                            limit,
                            &mut PathScratch::default(),
                            &mut actual,
                        );
                        assert_eq!(actual, expected);
                        let mut repeats = 1;
                        while elapsed(before, &case, &prior, limit, warm, repeats) < 0.005 {
                            repeats *= 2;
                        }
                        let mut ratios = Vec::new();
                        for pair in 0..40 {
                            let old = || elapsed(before, &case, &prior, limit, warm, repeats);
                            let new = || {
                                elapsed(
                                    SpellingTable::split_paths_with_scratch,
                                    &case,
                                    &prior,
                                    limit,
                                    warm,
                                    repeats,
                                )
                            };
                            let (old_time, new_time) = if pair % 2 == 0 {
                                (old(), new())
                            } else {
                                let new_time = new();
                                (old(), new_time)
                            };
                            ratios.push(new_time / old_time);
                            println!("pair {grid_index} {weighted} {shape} {limit} {warm} {pair} {old_time:.9} {new_time:.9} {:.6}",new_time/old_time);
                        }
                        ratios.sort_by(f64::total_cmp);
                        let median = (ratios[19] + ratios[20]) / 2.0;
                        let budget = if warm && shape == 4 { 0.98 } else { 1.05 };
                        println!("median {grid_index} {weighted} {shape} {limit} {warm} {median:.6} budget {budget}");
                        if median >= budget {
                            failures.push(format!("{grid_index}/{weighted}/{shape}/{limit}/{warm}: {median:.6} >= {budget}"));
                        }
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "手动普通System分配器逐句复用工作区，完整配对1791个按键前缀"]
fn evaluation_prefixes_path_scratch_paired_budget() {
    let grid = KeyGrid::NineKey;
    let table = grid.spelling_table();
    let inputs: Vec<Vec<String>> =
        include_str!("../../../../resources/eval/sentences-nine-key-v1.tsv")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty() && !line.starts_with("id\t"))
            .map(|line| {
                let digits = grid.encode(line.split('\t').nth(1).unwrap());
                (1..=digits.len())
                    .map(|end| digits[..end].to_owned())
                    .collect()
            })
            .collect();
    assert_eq!(inputs.len(), 81);
    assert_eq!(inputs.iter().map(Vec::len).sum::<usize>(), 1791);
    let batch = |split: Split, prior: &SyllablePrior, repeats: usize| {
        let start = std::time::Instant::now();
        for _ in 0..repeats {
            for case in &inputs {
                // 一句内逐键复用页与输出，两臂都在句末释放全部状态。
                let mut paths = Vec::new();
                let mut scratch = PathScratch::default();
                for digits in case {
                    split(
                        std::hint::black_box(table),
                        std::hint::black_box(digits),
                        &[],
                        std::hint::black_box(prior),
                        PATH_LIMIT,
                        &mut scratch,
                        &mut paths,
                    );
                    std::hint::black_box(&paths);
                }
            }
        }
        start.elapsed().as_secs_f64()
    };
    let mut failures = Vec::new();
    for (weighted, prior) in [SyllablePrior::default(), weighted_prior(table)]
        .into_iter()
        .enumerate()
    {
        for case in &inputs {
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            let mut scratch = PathScratch::default();
            for digits in case {
                frozen_into(table, digits, &[], &prior, PATH_LIMIT, &mut expected);
                table.split_paths_with_scratch(
                    digits,
                    &[],
                    &prior,
                    PATH_LIMIT,
                    &mut scratch,
                    &mut actual,
                );
                assert_eq!(actual, expected);
            }
        }
        let mut repeats = 1;
        while batch(before, &prior, repeats) < 0.010 {
            repeats *= 2;
        }
        let mut ratios = Vec::new();
        for pair in 0..40 {
            let (old_time, new_time) = if pair % 2 == 0 {
                let old_time = batch(before, &prior, repeats);
                (
                    old_time,
                    batch(SpellingTable::split_paths_with_scratch, &prior, repeats),
                )
            } else {
                let new_time = batch(SpellingTable::split_paths_with_scratch, &prior, repeats);
                (batch(before, &prior, repeats), new_time)
            };
            ratios.push(new_time / old_time);
            println!(
                "prefix_pair {weighted} {pair} {old_time:.9} {new_time:.9} {:.6}",
                new_time / old_time
            );
        }
        ratios.sort_by(f64::total_cmp);
        let median = (ratios[19] + ratios[20]) / 2.0;
        println!("prefix_median {weighted} {median:.6}");
        if median >= 0.98 {
            failures.push((weighted, median));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
