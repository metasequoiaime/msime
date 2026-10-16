use super::*;
use std::hint::black_box;
use std::time::Instant;

// 固定 c2d2d0c78c 的候选组码非空、双向前缀判定和覆盖长度。
#[inline(never)]
fn reference(grid: KeyGrid, reading: &str, digits: &str) -> Option<usize> {
    let code = grid.encode(reading);
    if code.is_empty() || (!code.starts_with(digits) && !digits.starts_with(&code)) {
        None
    } else {
        Some(code.len().min(digits.len()))
    }
}

#[inline(never)]
fn production(grid: KeyGrid, reading: &str, digits: &str) -> Option<usize> {
    grid.matching_code_prefix_len(reading, digits)
}

fn grids() -> Vec<KeyGrid> {
    let mut grids = vec![KeyGrid::NineKey, KeyGrid::FourteenKey];
    for kind in crate::types::NineKeyShuangpinKind::BUILTIN {
        let table = crate::nine_key_shuangpin::builtin_table(kind).unwrap();
        grids.push(KeyGrid::NineKeyShuangpin(
            crate::nine_key_shuangpin::nine_key_shuangpin(&table).unwrap(),
        ));
    }
    grids
}

#[test]
fn compatible_coverage_matches_owned_encoding() {
    for grid in grids() {
        let mut readings: Vec<String> = grid
            .spelling_table()
            .syllables
            .iter()
            .map(|(syllable, _)| syllable.clone())
            .collect();
        readings.extend(
            [
                "",
                "'",
                "123",
                "你好",
                "NI'HAO",
                "Ni'hao",
                "lü'e",
                "ni'hao",
                "ni'",
                "'ni",
                "ni''hao",
                "ni'你好",
                "ni'badtail",
                "abcdefghijklmnopqrstuvwxyz",
            ]
            .map(str::to_owned),
        );
        let mut seed = 29usize;
        let alphabet = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'0123456789";
        for length in [1, 2, 3, 8, 16, 32, 65] {
            for _ in 0..8 {
                readings.push(
                    (0..length)
                        .map(|_| {
                            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                            char::from(alphabet[seed % alphabet.len()])
                        })
                        .collect(),
                );
            }
        }
        for reading in readings {
            let code = grid.encode(&reading);
            let mut digits = vec!["".to_owned(), "1".repeat(32), "你好".to_owned()];
            digits.extend((0..=code.len()).map(|end| code[..end].to_owned()));
            digits.push(format!("{code}2"));
            digits.push(format!("{code}?"));
            if !code.is_empty() {
                digits.push(format!("?{}", &code[1..]));
                digits.push(format!("{}?", &code[..code.len() - 1]));
            }
            for digits in digits {
                assert_eq!(
                    production(grid, &reading, &digits),
                    reference(grid, &reading, &digits),
                    "grid={grid:?}, reading={reading:?}, digits={digits:?}"
                );
            }
        }
    }
}

#[test]
fn letter_grid_checks_do_not_allocate() {
    for grid in [KeyGrid::NineKey, KeyGrid::FourteenKey] {
        for reading in [
            "ni'hao",
            "ni'hao'ni'hao'ni'hao'ni'hao",
            "Ni'你hao",
            "",
            "你好",
        ] {
            let code = grid.encode(reading);
            for digits in [&code[..], &code[..code.len().min(1)], "?", ""] {
                let (expected, _) = crate::ime::personal_rerank::allocations::count(|| {
                    reference(grid, reading, digits)
                });
                let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    production(grid, reading, digits)
                });
                assert_eq!(actual, expected);
                assert_eq!(allocations, 0, "候选组码判定仍申请：{grid:?}, {reading:?}");
            }
        }
    }
}

#[test]
fn shuangpin_checks_keep_the_original_allocation_budget() {
    for grid in grids().into_iter().skip(2) {
        for reading in ["ni'hao", "n'h", "ni'badtail", "ni'你", ""] {
            let code = grid.encode(reading);
            for digits in [&code[..], &code[..code.len().min(1)], "?", ""] {
                let (expected, old) = crate::ime::personal_rerank::allocations::count(|| {
                    reference(grid, reading, digits)
                });
                let (actual, new) = crate::ime::personal_rerank::allocations::count(|| {
                    production(grid, reading, digits)
                });
                assert_eq!(actual, expected);
                assert!(new <= old, "九键双拼判定申请回退：旧 {old}，新 {new}");
            }
        }
    }
}

type Check = fn(KeyGrid, &str, &str) -> Option<usize>;

fn elapsed(check: Check, grid: KeyGrid, reading: &str, digits: &str, repeats: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..repeats {
        black_box(check(
            black_box(grid),
            black_box(reading),
            black_box(digits),
        ));
    }
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "独占配对完整组码判定，包含编码构造及析构"]
fn complete_code_prefix_paired_budget() {
    let mut failures = Vec::new();
    for (grid_index, grid) in grids().into_iter().enumerate() {
        let reading = "wo'jin'tian'qu'shang'ban";
        let code = grid.encode(reading);
        let cases = [
            ("full", reading, code.clone(), true),
            ("typed_prefix", reading, code[..1].to_owned(), true),
            (
                "code_prefix",
                "ni",
                format!("{}222", grid.encode("ni")),
                true,
            ),
            ("reject_first", reading, format!("?{}", &code[1..]), true),
            (
                "reject_last",
                reading,
                format!("{}?", &code[..code.len() - 1]),
                true,
            ),
            ("ignored_bytes", "Ni'你hao", grid.encode("Ni'你hao"), true),
            ("empty_reading", "", code.clone(), false),
            ("empty_digits", reading, String::new(), true),
        ];
        for (name, reading, digits, positive) in cases {
            assert_eq!(
                production(grid, reading, &digits),
                reference(grid, reading, &digits)
            );
            let mut repeats = 1;
            while elapsed(reference, grid, reading, &digits, repeats) < 0.010 {
                repeats *= 2;
            }
            let mut ratios = Vec::new();
            for pair in 0..40 {
                let (old, new) = if pair % 2 == 0 {
                    let old = elapsed(reference, grid, reading, &digits, repeats);
                    (old, elapsed(production, grid, reading, &digits, repeats))
                } else {
                    let new = elapsed(production, grid, reading, &digits, repeats);
                    (elapsed(reference, grid, reading, &digits, repeats), new)
                };
                ratios.push(new / old);
            }
            ratios.sort_by(f64::total_cmp);
            let ratio = ratios[19];
            let budget = if matches!(grid, KeyGrid::NineKeyShuangpin(_)) {
                1.05
            } else if positive {
                0.90
            } else {
                1.10
            };
            println!("grid={grid_index} shape={name} repeats={repeats} paired_ratio_p50={ratio:.4} budget={budget:.2}");
            if ratio >= budget {
                failures.push((grid_index, name, ratio, budget));
            }
        }
    }
    assert!(failures.is_empty(), "完整组码判定预算失败：{failures:?}");
}
