use super::*;

fn assert_send<T: Send>() {}

fn lattice_runtime() -> Runtime<Fixture> {
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
    runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(favouring_model(
        &['甲', '乙'],
        &['乙'],
    )))));
    runtime
}

fn dictionary_runtime() -> Runtime<Fixture> {
    let mut runtime = Runtime::new(
        Fixture {
            words: vec!["甲".into(), "乙".into()],
            codes: vec!["a".into(), "a".into()],
            text: "a".into(),
            ..Fixture::default()
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(favouring_model(
        &['甲', '乙'],
        &['乙'],
    )))));
    runtime
}

#[test]
fn settled_rerank_pieces_are_send() {
    assert_send::<Reranker>();
    assert_send::<SettledRerankRequest>();
    assert_send::<SettledRerankScore>();
}

#[test]
fn async_score_matches_the_synchronous_settled_rerank() {
    let mut synchronous = lattice_runtime();
    let mut asynchronous = lattice_runtime();
    assert!(synchronous.rerank_settled());

    let request = asynchronous.begin_settled_rerank().unwrap();
    let score = std::thread::spawn(move || score_settled_rerank(request))
        .join()
        .unwrap();
    let applied = asynchronous.apply_settled_rerank(score);

    assert!(applied.current && applied.moved);
    assert_eq!(
        asynchronous.cached.candidates,
        synchronous.cached.candidates
    );
    assert_eq!(asynchronous.engine_order, synchronous.engine_order);
    assert_eq!(asynchronous.generation, synchronous.generation);
    assert!(asynchronous.settled_reranker.is_some());
    assert_eq!(asynchronous.cached.candidates, ["乙", "甲"]);
    assert_eq!(asynchronous.engine_order, [1, 0]);
}

#[test]
fn async_score_keeps_static_evidence_on_the_promoted_row() {
    let mut synchronous = lattice_runtime();
    let mut asynchronous = lattice_runtime();
    let evidence = |total: i64| {
        Some(msime_engine::ordering::StaticEvidence {
            log_prob_milli_nats: total,
            characters: 1,
        })
    };
    synchronous.cached.candidate_static_evidence = vec![evidence(-40000), evidence(-1000)];
    asynchronous.cached.candidate_static_evidence = vec![evidence(-40000), evidence(-1000)];
    assert!(synchronous.rerank_settled());
    let request = asynchronous.begin_settled_rerank().unwrap();
    let applied = asynchronous.apply_settled_rerank(score_settled_rerank(request));
    assert!(applied.current && applied.moved);
    assert_eq!(
        asynchronous.cached.candidate_static_evidence,
        synchronous.cached.candidate_static_evidence
    );
    assert_eq!(
        asynchronous.cached.candidates,
        synchronous.cached.candidates
    );
}

#[test]
fn a_key_during_scoring_discards_the_promotion_and_returns_the_model() {
    let mut runtime = lattice_runtime();
    let request = runtime.begin_settled_rerank().unwrap();
    assert!(runtime.settled_reranker.is_none());
    let score = std::thread::spawn(move || score_settled_rerank(request))
        .join()
        .unwrap();
    type_key(&mut runtime);
    let applied = runtime.apply_settled_rerank(score);
    assert!(!applied.current && !applied.moved);
    assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
    assert!(runtime.settled_reranker.is_some());

    let request = runtime.begin_settled_rerank().unwrap();
    let applied = runtime.apply_settled_rerank(score_settled_rerank(request));
    assert!(applied.current && applied.moved);
    assert_eq!(runtime.cached.candidates, ["乙", "甲"]);
}

#[test]
fn a_cancelled_score_restores_the_model_without_reordering() {
    let mut runtime = lattice_runtime();
    let request = runtime.begin_settled_rerank().unwrap();
    runtime.discard_settled_rerank(score_settled_rerank(request));
    assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
    assert!(runtime.settled_reranker.is_some());
    assert!(runtime.rerank_settled());
    assert_eq!(runtime.cached.candidates, ["乙", "甲"]);
}

#[test]
fn a_trusted_dictionary_leader_stays_on_the_session_thread() {
    let mut runtime = dictionary_runtime();
    assert!(runtime.begin_settled_rerank().is_none());
    assert!(runtime.settled_reranker.is_some());
    assert!(!runtime.rerank_settled());
    assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
}

#[test]
fn idle_and_disabled_settled_rerank_leave_the_model_in_place() {
    let mut runtime = Runtime::new(
        Fixture {
            words: vec!["甲".into(), "乙".into()],
            codes: vec!["a".into(), "a".into()],
            sources: vec![LATTICE_SOURCE; 2],
            ..Fixture::default()
        },
        5,
    )
    .unwrap();
    runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(favouring_model(
        &['甲', '乙'],
        &['乙'],
    )))));
    assert!(runtime.begin_settled_rerank().is_none());
    assert!(runtime.settled_reranker.is_some());

    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.set_settled_rerank_enabled(false);
    assert!(runtime.begin_settled_rerank().is_none());
    assert!(runtime.settled_reranker.is_some());
    assert!(!runtime.rerank_settled());
}

#[test]
fn a_score_in_flight_keeps_the_fast_reranker_and_refuses_a_second_model() {
    let mut runtime = lattice_runtime();
    runtime.set_reranker(Some(Reranker::new(std::sync::Arc::new(favouring_model(
        &['甲', '乙'],
        &['甲'],
    )))));
    let request = runtime.begin_settled_rerank().unwrap();
    assert!(runtime.has_reranker());
    assert!(runtime.begin_settled_rerank().is_none());
    assert!(runtime.settled_reranker.is_none());
    type_key(&mut runtime);
    assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
    let applied = runtime.apply_settled_rerank(score_settled_rerank(request));
    assert!(!applied.current);
    assert!(runtime.settled_reranker.is_some());
    assert!(runtime.has_reranker());
}

#[test]
fn disabling_during_scoring_rejects_the_result_but_keeps_the_model() {
    let mut runtime = lattice_runtime();
    let request = runtime.begin_settled_rerank().unwrap();
    let generation = runtime.generation;
    runtime.set_settled_rerank_enabled(false);
    let applied = runtime.apply_settled_rerank(score_settled_rerank(request));
    assert!(!applied.current && !applied.moved);
    assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
    assert_eq!(runtime.generation, generation);
    assert!(runtime.settled_reranker.is_some());
    assert!(!runtime.rerank_settled());
    runtime.set_settled_rerank_enabled(true);
    assert!(runtime.rerank_settled());
    assert_eq!(runtime.cached.candidates, ["乙", "甲"]);
}

#[test]
fn toggling_off_and_back_on_does_not_revalidate_an_old_score() {
    let mut runtime = lattice_runtime();
    let request = runtime.begin_settled_rerank().unwrap();
    runtime.set_settled_rerank_enabled(false);
    runtime.set_settled_rerank_enabled(true);
    let applied = runtime.apply_settled_rerank(score_settled_rerank(request));
    assert!(!applied.current && !applied.moved);
    assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
    assert!(runtime.rerank_settled());
}

#[test]
fn replacing_the_model_rejects_the_old_score_and_keeps_the_new_model() {
    for publish in [false, true] {
        let mut runtime = lattice_runtime();
        let request = runtime.begin_settled_rerank().unwrap();
        let generation = runtime.generation;
        runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(favouring_model(
            &['甲', '乙'],
            &['甲'],
        )))));
        let score = score_settled_rerank(request);
        if publish {
            let applied = runtime.apply_settled_rerank(score);
            assert!(!applied.current && !applied.moved);
        } else {
            runtime.discard_settled_rerank(score);
        }
        assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
        assert_eq!(runtime.generation, generation);
        assert!(runtime.settled_reranker.is_some());
        assert!(!runtime.rerank_settled());
        assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
    }
}

#[test]
fn removing_the_model_during_scoring_never_restores_it() {
    for publish in [false, true] {
        let mut runtime = lattice_runtime();
        let request = runtime.begin_settled_rerank().unwrap();
        let generation = runtime.generation;
        runtime.set_settled_reranker(None);
        let score = score_settled_rerank(request);
        if publish {
            let applied = runtime.apply_settled_rerank(score);
            assert!(!applied.current && !applied.moved);
        } else {
            runtime.discard_settled_rerank(score);
        }
        assert_eq!(runtime.cached.candidates, ["甲", "乙"]);
        assert_eq!(runtime.generation, generation);
        assert!(runtime.settled_reranker.is_none());
        assert!(runtime.begin_settled_rerank().is_none());
    }
}

#[test]
fn reapplying_the_same_switch_value_keeps_the_in_flight_score_valid() {
    let mut runtime = lattice_runtime();
    let request = runtime.begin_settled_rerank().unwrap();
    runtime.set_settled_rerank_enabled(true);
    let applied = runtime.apply_settled_rerank(score_settled_rerank(request));
    assert!(applied.current && applied.moved);
    assert_eq!(runtime.cached.candidates, ["乙", "甲"]);
}

#[test]
fn a_highlight_moved_during_scoring_stays_on_the_selected_word() {
    let mut runtime = Runtime::new(
        Fixture {
            words: vec!["甲".into(), "乙".into(), "丙".into()],
            codes: vec!["a".into(); 3],
            sources: vec![LATTICE_SOURCE; 3],
            text: "a".into(),
            ..Fixture::default()
        },
        5,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(favouring_model(
        &['甲', '乙', '丙'],
        &['乙'],
    )))));
    let request = runtime.begin_settled_rerank().unwrap();
    runtime.dispatch(Action::NextCandidate).unwrap();
    runtime.dispatch(Action::NextCandidate).unwrap();
    assert_eq!(runtime.cached.candidates[runtime.highlighted], "丙");
    let applied = runtime.apply_settled_rerank(score_settled_rerank(request));
    // 候选导航也推进代次，旧评分先丢弃；随后评分仍须保留导航后的高亮。
    assert!(!applied.current && !applied.moved);
    assert_eq!(runtime.cached.candidates, ["甲", "乙", "丙"]);
    assert_eq!(runtime.cached.candidates[runtime.highlighted], "丙");
    let request = runtime.begin_settled_rerank().unwrap();
    let applied = runtime.apply_settled_rerank(score_settled_rerank(request));
    assert!(applied.current && applied.moved);
    assert_eq!(runtime.cached.candidates[0], "乙");
    assert_eq!(runtime.cached.candidates[runtime.highlighted], "丙");
}
