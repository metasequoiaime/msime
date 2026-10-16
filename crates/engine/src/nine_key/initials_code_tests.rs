use super::*;
use std::hint::black_box;
use std::time::Instant;

// 固定 4007628d74 的完整展开正文，只改函数名。
#[inline(never)]
fn reference(grid: KeyGrid, digits: &str, limit: usize) -> Option<Vec<String>> {
    let mut codes = vec![String::with_capacity(digits.len())];
    for digit in digits.bytes() {
        let count = initials_for_digit(grid, digit).count();
        if count == 0 || codes.len().saturating_mul(count) > limit {
            return None;
        }
        codes = codes
            .iter()
            .flat_map(|code| {
                initials_for_digit(grid, digit).map(move |letter| {
                    let mut next = String::with_capacity(digits.len());
                    next.push_str(code);
                    next.push(char::from(letter));
                    next
                })
            })
            .collect();
    }
    Some(codes)
}

#[inline(never)]
fn production(grid: KeyGrid, digits: &str, limit: usize) -> Option<Vec<String>> {
    initials_codes(grid, digits, limit)
}

#[test]
fn rejected_initials_do_not_allocate_partial_codes() {
    for digits in ["999999", "99999!", "99999é"] {
        let (result, count) = crate::ime::personal_rerank::allocations::count(|| {
            production(KeyGrid::NineKey, digits, 1024)
        });
        assert!(result.is_none());
        assert_eq!(count, 0, "拒绝输入仍物化中间码：{digits:?}");
    }
}

fn grids() -> Vec<KeyGrid> {
    let mut grids = vec![KeyGrid::NineKey, KeyGrid::FourteenKey];
    for kind in crate::types::NineKeyShuangpinKind::BUILTIN {
        let table = crate::nine_key_shuangpin::builtin_table(kind).expect("内置表");
        let table = crate::nine_key_shuangpin::nine_key_shuangpin(&table).expect("合法表");
        grids.push(KeyGrid::NineKeyShuangpin(table));
    }
    grids
}

#[test]
fn initials_preserve_order_limits_empty_invalid_and_long_single_choice() {
    for grid in grids() {
        let keys: Vec<u8> = (0..=127)
            .filter(|key| !grid.letters(*key).is_empty())
            .collect();
        let mut inputs = vec![
            String::new(),
            "!".to_owned(),
            "é".to_owned(),
            "1".to_owned(),
        ];
        for &a in &keys {
            inputs.push(char::from(a).to_string());
            for &b in &keys {
                inputs.push(format!("{}{}", char::from(a), char::from(b)));
                for &c in &keys {
                    inputs.push(format!(
                        "{}{}{}",
                        char::from(a),
                        char::from(b),
                        char::from(c)
                    ));
                }
            }
        }
        for input in &inputs {
            let count = input
                .bytes()
                .map(|digit| initials_for_digit(grid, digit).count())
                .product::<usize>();
            for limit in [
                0,
                1,
                count.saturating_sub(1),
                count,
                count + 1,
                1024,
                usize::MAX,
            ] {
                let (expected, old) = crate::ime::personal_rerank::allocations::count(|| {
                    reference(grid, input, limit)
                });
                let (actual, new) = crate::ime::personal_rerank::allocations::count(|| {
                    production(grid, input, limit)
                });
                assert!(
                    new <= old,
                    "分配回退：{grid:?} {input:?} {limit}: {old} -> {new}"
                );
                assert_eq!(actual, expected, "{grid:?} {input:?} {limit}");
                if actual.is_some() {
                    assert_eq!(new, old, "有效输入分配契约：{grid:?} {input:?}");
                } else {
                    assert_eq!(new, 0, "拒绝输入零分配：{grid:?} {input:?}");
                }
                if let (Some(actual), Some(expected)) = (actual, expected) {
                    assert_eq!(actual.capacity(), expected.capacity());
                    assert_eq!(
                        actual.iter().map(String::capacity).collect::<Vec<_>>(),
                        expected.iter().map(String::capacity).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    for input in [
        "99999".to_owned(),
        "999999".to_owned(),
        "8".repeat(4096),
        "8".repeat(4095) + "!",
    ] {
        for limit in [0, 1, 1023, 1024, 1025] {
            assert_eq!(
                production(KeyGrid::NineKey, &input, limit),
                reference(KeyGrid::NineKey, &input, limit)
            );
        }
    }
}

#[test]
#[ignore = "局部配对计时，使用 release 串行手动运行"]
fn paired_initials_timing() {
    let rounds = 31;
    let mut failures = Vec::new();
    for (grid_index, grid) in grids().into_iter().enumerate() {
        let keys: Vec<u8> = (0..=127)
            .filter(|key| initials_for_digit(grid, *key).count() > 0)
            .collect();
        let widest = *keys
            .iter()
            .max_by_key(|key| initials_for_digit(grid, **key).count())
            .unwrap();
        let wide = char::from(widest).to_string();
        let mut inputs = vec![
            String::new(),
            char::from(keys[0]).to_string(),
            format!(
                "{}{}",
                char::from(keys[0]),
                char::from(*keys.last().unwrap())
            ),
            wide.repeat(4),
            wide.repeat(5),
            wide.repeat(12),
            wide.repeat(5) + "!",
        ];
        if let Some(single) = keys
            .iter()
            .find(|key| initials_for_digit(grid, **key).count() == 1)
        {
            inputs.push(char::from(*single).to_string().repeat(16));
        }
        for input in inputs {
            let input = input.as_str();
            let mut iterations = 1;
            let mut ratios = Vec::new();
            let run = |f: fn(KeyGrid, &str, usize) -> Option<Vec<String>>, iterations| {
                let start = Instant::now();
                for _ in 0..iterations {
                    drop(black_box(f(grid, black_box(input), black_box(1024))));
                }
                start.elapsed().as_secs_f64()
            };
            while run(reference, iterations) < 0.050 {
                iterations *= 2;
            }
            for round in 0..rounds {
                let (old, new) = if round % 2 == 0 {
                    let old = run(reference, iterations);
                    (old, run(production, iterations))
                } else {
                    let new = run(production, iterations);
                    (run(reference, iterations), new)
                };
                ratios.push(new / old);
            }
            ratios.sort_by(f64::total_cmp);
            eprintln!("grid={grid_index} initials={input:?} repeats={iterations} new/old median {:.6}, p10 {:.6}, p90 {:.6}", ratios[rounds / 2], ratios[3], ratios[27]);
            if ratios[rounds / 2] > 1.05 {
                failures.push((grid_index, input.to_owned(), ratios[rounds / 2]));
            }
        }
    }
    assert!(failures.is_empty(), "局部耗时预算失败：{failures:?}");
}

#[test]
fn representative_initials_allocation_measurements() {
    for digits in ["2356", "99999", "999999", "99999!"] {
        let (expected, old) = crate::ime::personal_rerank::allocations::measure(|| {
            reference(KeyGrid::NineKey, digits, 1024)
        });
        let (actual, new) = crate::ime::personal_rerank::allocations::measure(|| {
            production(KeyGrid::NineKey, digits, 1024)
        });
        assert_eq!(actual, expected);
        assert!(new.allocations <= old.allocations);
        assert!(new.peak_bytes <= old.peak_bytes);
        assert!(new.remaining_bytes <= old.remaining_bytes);
        eprintln!(
            "initials {digits}: allocations {} -> {}, peak {} -> {}, retained {} -> {}",
            old.allocations,
            new.allocations,
            old.peak_bytes,
            new.peak_bytes,
            old.remaining_bytes,
            new.remaining_bytes
        );
    }
}
