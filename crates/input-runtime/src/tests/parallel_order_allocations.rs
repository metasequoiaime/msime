use super::allocations;
use super::*;

fn runtime_with_rows(rows: usize) -> Runtime<Fixture> {
    Runtime::new(
        Fixture {
            words: (0..rows).map(|index| format!("合成候选{index}")).collect(),
            codes: (0..rows).map(|index| format!("k{index}")).collect(),
            text: "a".into(),
            ..Fixture::default()
        },
        5,
    )
    .unwrap()
}

#[test]
fn parallel_candidate_order_uses_one_cycle_buffer() {
    let mut measurements = Vec::new();
    for rows in [32, 127, 128, 129, 1024, 4096] {
        for reverse in [false, true] {
            let mut runtime = runtime_with_rows(rows);
            let mut order: Vec<usize> = (0..rows).collect();
            if reverse {
                order.reverse();
            } else {
                order[1..].rotate_left(1);
            }
            let (_, count, bytes) = allocations::count_bytes(|| {
                runtime.apply_candidate_order(&order);
            });
            let scratch = if rows > 128 { rows } else { 0 };
            let budget = rows * std::mem::size_of::<usize>() + scratch;
            let allocation_budget = 1 + usize::from(scratch > 0);
            eprintln!("rows={rows} reverse={reverse}: allocations={count} requested_bytes={bytes}");
            measurements.push((rows, reverse, count, bytes, allocation_budget, budget));
            assert_eq!(runtime.engine_order, order);
            for (seat, &row) in order.iter().enumerate() {
                assert_eq!(runtime.cached.candidates[seat], format!("合成候选{row}"));
                assert_eq!(runtime.cached.candidate_codes[seat], format!("k{row}"));
                assert_eq!(
                    runtime.cached.candidate_annotations[seat],
                    format!("({row})")
                );
            }
        }
    }
    assert!(
        measurements
            .iter()
            .all(|(_, _, count, bytes, allocation_budget, budget)| {
                count <= allocation_budget && bytes <= budget
            }),
        "候选元数据和引擎座位映射只应共用一份循环访问缓冲: {measurements:?}"
    );
}

#[test]
fn parallel_candidate_order_keeps_all_metadata_and_composes_engine_seats() {
    for rows in [0, 1, 127, 128, 129, 4096] {
        let mut runtime = runtime_with_rows(rows);
        runtime.focus(true).unwrap();
        type_key(&mut runtime);
        // 使用不同的合成字段值，避免不变的布尔或来源掩盖漏交换。
        runtime.cached.candidate_static_evidence = (0..rows)
            .map(|row| {
                (row % 2 == 0).then_some(msime_engine::ordering::StaticEvidence {
                    log_prob_milli_nats: -(row as i64),
                    characters: row + 1,
                })
            })
            .collect();
        for row in 0..rows {
            runtime.cached.candidate_sources[row] = (row % 14) as u8;
            runtime.cached.candidate_positions[row] = (row % 256) as u8;
            runtime.cached.candidate_corrected[row] = row % 2 == 0;
            runtime.cached.candidate_answers_key[row] = row % 3 == 0;
        }
        let original = runtime.cached.clone();
        let pointer = runtime.cached.candidates.as_ptr();
        let mut order: Vec<usize> = (0..rows).collect();
        if rows > 1 {
            order[1..].rotate_left(1);
        }
        runtime.apply_candidate_order(&order);
        let reverse: Vec<usize> = (0..rows).rev().collect();
        runtime.apply_candidate_order(&reverse);
        let expected: Vec<usize> = reverse.iter().map(|&seat| order[seat]).collect();
        assert_eq!(runtime.engine_order, expected);
        assert_eq!(runtime.cached.candidates.as_ptr(), pointer);
        for (seat, &row) in expected.iter().enumerate() {
            assert_eq!(runtime.cached.candidates[seat], original.candidates[row]);
            assert_eq!(
                runtime.cached.candidate_codes[seat],
                original.candidate_codes[row]
            );
            assert_eq!(
                runtime.cached.candidate_annotations[seat],
                original.candidate_annotations[row]
            );
            assert_eq!(
                runtime.cached.candidate_sources[seat],
                original.candidate_sources[row]
            );
            assert_eq!(
                runtime.cached.candidate_positions[seat],
                original.candidate_positions[row]
            );
            assert_eq!(
                runtime.cached.candidate_corrected[seat],
                original.candidate_corrected[row]
            );
            assert_eq!(
                runtime.cached.candidate_answers_key[seat],
                original.candidate_answers_key[row]
            );
            assert_eq!(
                runtime.cached.candidate_static_evidence[seat],
                original.candidate_static_evidence[row]
            );
        }
        let identity: Vec<usize> = (0..rows).collect();
        let (_, allocations) = allocations::count(|| runtime.apply_candidate_order(&identity));
        assert_eq!(allocations, 0);
        assert_eq!(runtime.engine_order, expected);
        if rows > 0 {
            let candidate = runtime.view().candidates[0].clone();
            assert_eq!(
                runtime
                    .dispatch(Action::Select(candidate.id))
                    .unwrap()
                    .commit,
                Some(candidate.text)
            );
        }
    }
}
