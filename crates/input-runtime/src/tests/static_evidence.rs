use super::*;
use msime_engine::ordering::{rerank_pick, OrderRow, ScoredOrderRows, StaticEvidence};

fn evidence(total: i64, characters: usize) -> Option<StaticEvidence> {
    Some(StaticEvidence {
        log_prob_milli_nats: total,
        characters,
    })
}

fn row(text: &str) -> OrderRow<'_> {
    OrderRow {
        text,
        source: LATTICE_SOURCE,
        answers_key: true,
        corrected: false,
    }
}

fn model() -> Reranker {
    Reranker::new(std::sync::Arc::new(favouring_model(&['甲', '乙'], &['乙'])))
}

#[test]
fn static_evidence_can_preserve_the_reading_the_model_alone_loses() {
    let rows = [row("甲"), row("乙")];
    let mut model = model();
    assert_eq!(rerank_pick(&mut model, "", 0, false, &rows), Some(1));
    let scores = [evidence(-1000, 1), evidence(-40000, 1)];
    assert_eq!(
        rerank_pick(
            &mut model,
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        None
    );
}

#[test]
fn incomplete_evidence_falls_back_to_the_existing_model() {
    let rows = [row("甲"), row("乙")];
    let mut model = model();
    for scores in [
        vec![],
        vec![evidence(-1000, 1)],
        vec![evidence(-1000, 1), None],
        vec![None, evidence(-40000, 1)],
        vec![evidence(-1000, 1), evidence(-40000, 0)],
    ] {
        assert_eq!(
            rerank_pick(
                &mut model,
                "",
                0,
                false,
                &ScoredOrderRows::new(&rows, &scores)
            ),
            Some(1),
            "{scores:?}"
        );
    }
}

#[test]
fn static_means_compare_complete_readings_of_different_lengths() {
    // 模型同样喜欢每个字；总分会选短读法，逐字平均必须让长读法胜出。
    let rows = [row("乙"), row("乙乙")];
    let scores = [evidence(-5000, 1), evidence(-6000, 2)];
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        Some(1)
    );
    let tied = [evidence(-3000, 1), evidence(-6000, 2)];
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &tied)
        ),
        None
    );
}

#[test]
fn static_evidence_preserves_dictionary_and_key_comparability_rules() {
    let mut rows = [row("甲"), row("乙")];
    let scores = [evidence(-40000, 1), evidence(-1000, 1)];
    rows[0].source = 0;
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        None
    );
    rows[0].source = LATTICE_SOURCE;
    rows[1].answers_key = false;
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        None
    );
    rows[1].answers_key = true;
    rows[1].corrected = true;
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            1,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        None
    );
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        Some(1)
    );
}

#[test]
fn static_evidence_ignores_nonreadings_and_keeps_the_nine_reading_budget() {
    let mut rows = vec![row("乙"); 10];
    let mut scores = vec![evidence(-5000, 1); 10];
    scores[9] = evidence(-1000, 1);
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        None,
        "第十条完整读法不扩充既有比较预算"
    );
    rows.insert(
        1,
        OrderRow {
            source: 3,
            ..row("乙")
        },
    );
    scores.insert(1, None);
    scores[2] = evidence(-3000, 1);
    assert_eq!(
        rerank_pick(
            &mut model(),
            "",
            0,
            false,
            &ScoredOrderRows::new(&rows, &scores)
        ),
        Some(2),
        "AI 行既不参与比较，也不因缺失证据阻止真实读法的混合"
    );
}

#[test]
fn successive_reranks_keep_static_evidence_on_its_engine_row() {
    let mut runtime = Runtime::new(
        Fixture {
            words: vec!["甲".into(), "乙".into()],
            codes: vec!["a".into(), "a".into()],
            sources: vec![LATTICE_SOURCE; 2],
            text: "a".into(),
            ..Fixture::default()
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.cached.candidate_static_evidence = vec![evidence(-40000, 1), evidence(-1000, 1)];
    runtime.set_settled_reranker(Some(model()));
    let before = runtime.cached.clone();
    runtime.set_settled_rerank_enabled(false);
    assert!(!runtime.rerank_settled());
    assert_eq!(runtime.cached, before, "静态证据不绕过关闭的模型开关");
    runtime.set_settled_rerank_enabled(true);
    assert!(runtime.rerank_settled());
    assert_eq!(runtime.cached.candidates, ["乙", "甲"]);
    assert_eq!(runtime.engine_order, [1, 0]);
    assert_eq!(
        runtime.cached.candidate_static_evidence,
        [evidence(-1000, 1), evidence(-40000, 1)]
    );
    assert!(
        !runtime.rerank_settled(),
        "同一行的证据重复排序后仍保持领先"
    );
    let first = runtime.view().candidates[0].clone();
    assert_eq!(
        runtime.dispatch(Action::Select(first.id)).unwrap().commit,
        Some("乙".into())
    );
}

#[test]
fn static_rerank_large_lists_do_not_allocate_a_full_text_projection() {
    for compared in [2, 9] {
        for interleaved in [false, true] {
            let mut prefix = Vec::new();
            let mut scores = Vec::new();
            for index in 0..compared {
                if interleaved && index > 0 {
                    prefix.push(OrderRow {
                        source: 3,
                        ..row("乙")
                    });
                    scores.push(None);
                }
                prefix.push(row(if index == 0 { "甲" } else { "乙" }));
                scores.push(evidence(
                    if index + 1 == compared { -1000 } else { -40000 },
                    1,
                ));
            }
            let expected = Some(prefix.len() - 1);
            for warmed in [false, true] {
                let run = |count| {
                    let mut rows = prefix.clone();
                    let mut scores = scores.clone();
                    rows.resize(
                        count,
                        OrderRow {
                            source: 3,
                            ..row("乙")
                        },
                    );
                    scores.resize(count, None);
                    let scored = ScoredOrderRows::new(&rows, &scores);
                    let mut model = model();
                    if warmed {
                        assert_eq!(rerank_pick(&mut model, "甲", 0, false, &scored), expected);
                    }
                    let (picked, count, bytes) = allocations::count_bytes(|| {
                        rerank_pick(&mut model, "甲", 0, false, &scored)
                    });
                    assert_eq!(picked, expected);
                    (count, bytes)
                };
                let baseline = run(prefix.len());
                for count in [128, 129, 1024] {
                    assert_eq!(run(count), baseline, "compared={compared} interleaved={interleaved} warmed={warmed} rows={count}");
                }
            }
        }
    }
}

#[test]
fn large_static_rerank_lists_preserve_fallback_and_key_rules() {
    for count in [2, 128, 129, 1024] {
        for mode in 0..7 {
            let mut rows = vec![
                OrderRow {
                    source: 3,
                    ..row("乙")
                };
                count
            ];
            rows[0] = row("甲");
            rows[1] = row("乙");
            let mut scores = vec![None; count];
            scores[0] = evidence(-1000, 1);
            scores[1] = evidence(-40000, 1);
            let mut scheme = 0;
            let expected = match mode {
                0 => {
                    scores[1] = None;
                    Some(1)
                }
                1 => {
                    scores[1] = evidence(-40000, 0);
                    Some(1)
                }
                2 => {
                    scores.pop();
                    Some(1)
                }
                3 => {
                    rows[0].source = 0;
                    None
                }
                4 => {
                    rows[0].source = 0;
                    rows[1].corrected = true;
                    scores[1] = evidence(-1000, 1);
                    scores[0] = evidence(-40000, 1);
                    Some(1)
                }
                5 => {
                    scheme = 1;
                    rows[1].corrected = true;
                    None
                }
                _ => {
                    rows[0].answers_key = false;
                    None
                }
            };
            assert_eq!(
                rerank_pick(
                    &mut model(),
                    "甲",
                    scheme,
                    false,
                    &ScoredOrderRows::new(&rows, &scores)
                ),
                expected,
                "rows={count} mode={mode}"
            );
        }
    }
}
