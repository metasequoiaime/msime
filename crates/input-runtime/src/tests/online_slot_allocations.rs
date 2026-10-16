use super::allocations;
use super::*;

fn seated_runtime(rows: usize, source: u8, fixed: usize) -> Runtime<Fixture> {
    let mut sources = vec![0; rows];
    // 已经坐好的位置：云候选在第 2 格，AI 行在每页 5 个的第一页最后一格（#6888）。
    sources[if source == 2 { 1 } else { 4 }] = source;
    let mut positions = vec![0; rows];
    for &seat in [3, 7, 12, 20, 25].iter().take(fixed) {
        sources[seat] = 4;
        positions[seat] = (seat + 1) as u8;
    }
    Runtime::new(
        Fixture {
            words: (0..rows).map(|index| format!("合成候选{index}")).collect(),
            codes: (0..rows).map(|index| format!("k{index}")).collect(),
            sources,
            positions,
            text: "a".into(),
            ..Fixture::default()
        },
        5,
    )
    .unwrap()
}

#[test]
fn online_seating_skips_identity_storage_without_fixed_english_rows() {
    let mut measurements = Vec::new();
    for rows in [32, 1024, 4096] {
        for source in [2, 3, 12] {
            for fixed in [0, 1, 5] {
                let mut runtime = seated_runtime(rows, source, fixed);
                let (snapshot, snapshot_allocations, snapshot_bytes) =
                    allocations::count_bytes(|| runtime.engine.snapshot().unwrap());
                let (_, refresh_allocations, refresh_bytes) =
                    allocations::count_bytes(|| runtime.refresh().unwrap());
                // 列表已经坐好：没有固定英文行时不需要排列存储。
                assert_eq!(runtime.cached.candidates, snapshot.candidates);
                assert_eq!(runtime.cached.candidate_codes, snapshot.candidate_codes);
                assert_eq!(
                    runtime.cached.candidate_annotations,
                    snapshot.candidate_annotations
                );
                assert_eq!(runtime.cached.candidate_sources, snapshot.candidate_sources);
                assert_eq!(
                    runtime.cached.candidate_positions,
                    snapshot.candidate_positions
                );
                assert_eq!(
                    runtime.cached.candidate_corrected,
                    snapshot.candidate_corrected
                );
                assert_eq!(
                    runtime.cached.candidate_answers_key,
                    snapshot.candidate_answers_key
                );
                let budget = if fixed == 0 {
                    0
                } else {
                    (rows + fixed) * std::mem::size_of::<usize>()
                };
                let allocation_budget = if fixed == 0 { 0 } else { 2 };
                eprintln!("rows={rows} source={source} fixed={fixed}: extra_allocations={} extra_requested_bytes={}",
                    refresh_allocations.saturating_sub(snapshot_allocations),
                    refresh_bytes.saturating_sub(snapshot_bytes));
                measurements.push((
                    rows,
                    source,
                    fixed,
                    refresh_allocations.saturating_sub(snapshot_allocations),
                    refresh_bytes.saturating_sub(snapshot_bytes),
                    allocation_budget,
                    budget,
                ));
            }
        }
    }
    assert!(
        measurements
            .iter()
            .all(
                |(_, _, _, allocations, bytes, allocation_budget, budget)| allocations
                    <= allocation_budget
                    && bytes <= budget
            ),
        "已归位且无固定英文行时不应构造排列存储: {measurements:?}"
    );
}

#[test]
fn fixed_english_seats_keep_stable_ties_clamping_and_the_ai_tail() {
    let mut runtime = Runtime::new(
        Fixture {
            words: [
                "本地甲",
                "英文甲",
                "英文乙",
                "英文丙",
                "本地乙",
                "云",
                "AI首行",
                "AI末行",
            ]
            .map(str::to_owned)
            .to_vec(),
            codes: (0..8).map(|index| format!("k{index}")).collect(),
            sources: vec![0, 4, 4, 4, 0, 2, 3, 3],
            positions: vec![0, 3, 3, 255, 0, 0, 0, 0],
            text: "a".into(),
            ..Fixture::default()
        },
        // 每页 5 个：第一条 AI 坐下标 4，固定英文在其余行里归位后它仍在那里。
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    assert_eq!(
        runtime.cached.candidates,
        [
            "本地甲",
            "云",
            "英文乙",
            "英文甲",
            "AI首行",
            "本地乙",
            "英文丙",
            "AI末行"
        ]
    );
    assert_eq!(
        runtime.cached.candidate_codes,
        ["k0", "k5", "k2", "k1", "k6", "k4", "k3", "k7"]
    );
    let english = runtime
        .view()
        .candidates
        .iter()
        .find(|candidate| candidate.text == "英文乙")
        .unwrap()
        .id;
    assert_eq!(
        runtime
            .dispatch(Action::Select(english))
            .unwrap()
            .commit
            .as_deref(),
        Some("英文乙")
    );
}

#[test]
fn online_seating_materializes_early_and_late_mismatches() {
    for rows in [8, 32, 128, 129, 4096] {
        for late in [false, true] {
            let mut sources = vec![0; rows];
            if late {
                // 大段云候选和第一页最后一格的 AI 行已经归位，只有末尾的本地行和第二条 AI 行需要互换。
                sources[1..rows - 2].fill(2);
                sources[4] = 3;
                sources[rows - 2] = 3;
            } else {
                // 引擎把 AI 行放在最后，它要挪到第一页最后一格（每页 5 个，下标 4）。
                sources[rows - 1] = 3;
            }
            let mut runtime = Runtime::new(
                Fixture {
                    words: (0..rows).map(|index| format!("合成候选{index}")).collect(),
                    codes: (0..rows).map(|index| format!("k{index}")).collect(),
                    sources,
                    text: "a".into(),
                    ..Fixture::default()
                },
                5,
            )
            .unwrap();
            runtime.refresh().unwrap();
            let mut expected: Vec<usize> = (0..rows).collect();
            if late {
                expected.swap(rows - 2, rows - 1);
            } else {
                expected.remove(rows - 1);
                expected.insert(4, rows - 1);
            }
            assert_eq!(runtime.engine_order, expected);
            for (seat, &row) in expected.iter().enumerate() {
                assert_eq!(runtime.cached.candidates[seat], format!("合成候选{row}"));
                assert_eq!(runtime.cached.candidate_codes[seat], format!("k{row}"));
                assert_eq!(
                    runtime.cached.candidate_annotations[seat],
                    format!("({row})")
                );
            }
            let (snapshot, snapshot_allocations, snapshot_bytes) =
                allocations::count_bytes(|| runtime.engine.snapshot().unwrap());
            let (_, refresh_allocations, refresh_bytes) =
                allocations::count_bytes(|| runtime.refresh().unwrap());
            // 第二次刷新复用引擎映射容量，保留一份排列和大列表的一份循环标记。
            let scratch = if rows > 128 { rows } else { 0 };
            assert!(refresh_allocations - snapshot_allocations <= 1 + usize::from(scratch > 0));
            assert!(
                refresh_bytes - snapshot_bytes <= rows * std::mem::size_of::<usize>() + scratch
            );
            assert_eq!(runtime.cached.candidates.len(), snapshot.candidates.len());
            assert_eq!(runtime.engine_order, expected);
            runtime.focused = true;
            let seat = if late { rows - 2 } else { 4 };
            let id = CandidateId {
                session: runtime.session,
                generation: runtime.generation,
                index: seat,
            };
            assert_eq!(
                runtime
                    .dispatch(Action::SelectAnyCandidate(id))
                    .unwrap()
                    .commit,
                Some(format!("合成候选{}", rows - 1))
            );
        }
    }
}

#[test]
fn seated_source_groups_need_no_order_allocation() {
    // 每页 5 个：第一条 AI 行坐下标 4，其余 AI 行在最后（#6888）。
    let cases = [
        (
            vec![0, 2, 2, 13, 3, 4, 6, 7, 0, 0, 4, 13, 6, 7, 3, 12],
            false,
        ),
        (vec![0, 13, 4, 6, 3, 7, 0, 4, 13, 6, 7, 12, 3], false),
        (vec![4, 13, 0, 2, 3, 6, 7, 0, 4, 13, 6, 7, 12], false),
        (vec![13, 0, 2, 4, 3, 6, 7, 0, 4, 13, 6, 7, 3], false),
        (vec![2, 2, 13, 4, 3, 6, 7, 4, 13, 6, 7, 3], false),
        (vec![0, 0, 2, 4, 3, 6, 7, 0, 4, 6, 7, 3], true),
    ];
    let mut measurements = Vec::new();
    for (sources, single_kana) in cases {
        let rows = sources.len();
        let mut runtime = Runtime::new(
            Fixture {
                words: (0..rows).map(|index| format!("合成候选{index}")).collect(),
                codes: (0..rows).map(|index| format!("k{index}")).collect(),
                sources,
                text: "a".into(),
                scheme: if single_kana {
                    msime_engine::SchemeType::JapaneseRomaji as u8
                } else {
                    0
                },
                reading: if single_kana {
                    "あ".into()
                } else {
                    String::new()
                },
                ..Fixture::default()
            },
            5,
        )
        .unwrap();
        let epochs = (runtime.composition_epoch, runtime.candidates_epoch);
        let (snapshot, snapshot_allocations, snapshot_bytes) =
            allocations::count_bytes(|| runtime.engine.snapshot().unwrap());
        let (_, refresh_allocations, refresh_bytes) =
            allocations::count_bytes(|| runtime.refresh().unwrap());
        assert_eq!(runtime.cached.candidates, snapshot.candidates);
        assert_eq!(runtime.cached.candidate_codes, snapshot.candidate_codes);
        assert_eq!(
            runtime.cached.candidate_annotations,
            snapshot.candidate_annotations
        );
        assert_eq!(runtime.cached.candidate_sources, snapshot.candidate_sources);
        assert_eq!(
            runtime.cached.candidate_positions,
            snapshot.candidate_positions
        );
        assert_eq!(
            runtime.cached.candidate_corrected,
            snapshot.candidate_corrected
        );
        assert_eq!(
            runtime.cached.candidate_answers_key,
            snapshot.candidate_answers_key
        );
        assert_eq!(
            (runtime.composition_epoch, runtime.candidates_epoch),
            epochs
        );
        assert!(runtime.engine_order.is_empty());
        measurements.push((
            rows,
            refresh_allocations - snapshot_allocations,
            refresh_bytes - snapshot_bytes,
        ));
    }
    assert!(
        measurements
            .iter()
            .all(|(_, count, bytes)| *count == 0 && *bytes == 0),
        "已归位的多来源列表不需要排列存储: {measurements:?}"
    );
}

#[test]
fn fixed_english_materializes_an_identity_prefix_before_the_ai_tail() {
    let mut runtime = Runtime::new(
        Fixture {
            words: ["本地", "云", "英文", "AI首行", "AI占位尾行"]
                .map(str::to_owned)
                .to_vec(),
            codes: (0..5).map(|index| format!("k{index}")).collect(),
            sources: vec![0, 2, 4, 3, 12],
            positions: vec![0, 0, 3, 0, 0],
            text: "a".into(),
            ..Fixture::default()
        },
        5,
    )
    .unwrap();
    let (snapshot, snapshot_allocations, snapshot_bytes) =
        allocations::count_bytes(|| runtime.engine.snapshot().unwrap());
    let (_, refresh_allocations, refresh_bytes) =
        allocations::count_bytes(|| runtime.refresh().unwrap());
    assert_eq!(runtime.cached.candidates, snapshot.candidates);
    assert_eq!(runtime.cached.candidate_codes, snapshot.candidate_codes);
    assert_eq!(runtime.cached.candidate_sources, snapshot.candidate_sources);
    assert_eq!(
        runtime.cached.candidate_positions,
        snapshot.candidate_positions
    );
    assert!(runtime.engine_order.is_empty());
    assert!(refresh_allocations - snapshot_allocations <= 2);
    assert!(refresh_bytes - snapshot_bytes <= 6 * std::mem::size_of::<usize>());
}
