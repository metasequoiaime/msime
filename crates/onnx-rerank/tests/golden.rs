//! 小模型逐键跑出的分数与 ONNX Runtime 的 Python 包一致（见 `fixtures/README.md`），跨按键复用缓存与从头算的结果只差在量化误差以内。
#![cfg(feature = "runtime")]

use msime_engine::ordering::CandidateScorer;
use msime_onnx_rerank::OnnxScorer;
use std::path::Path;

/// 同一份模型在不同 CPU 的整数内核上会有末位差别；随机小模型的分数在 -5 附近，这个容差远小于候选之间的差。
const TOLERANCE: f32 = 1e-3;

/// 同一页换一种分批方式算（一次从头算完，或者分几次按键、复用缓存）时的容差。权重是动态量化的：每次运行按这一批节点的取值范围定激活的量化比例，同一个前缀和不同的节点一起算，结果就差在末几位，实测约 0.005；fp32 下两种分批完全相同。
const REBATCHED: f32 = 2e-2;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn scorer() -> OnnxScorer {
    let model =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lm_step.ort"))
            .unwrap();
    OnnxScorer::load(&model, &fixture("model.json"), &fixture("vocab.json"), 1).unwrap()
}

struct Keystroke {
    candidates: Vec<String>,
    scores: Vec<f32>,
}

fn cases() -> Vec<(String, Vec<Keystroke>)> {
    fixture("golden.jsonl")
        .lines()
        .map(|line| {
            let case: serde_json::Value = serde_json::from_str(line).unwrap();
            let keystrokes = case["keystrokes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| Keystroke {
                    candidates: k["candidates"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| c.as_str().unwrap().to_owned())
                        .collect(),
                    scores: k["scores"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_f64().unwrap() as f32)
                        .collect(),
                })
                .collect();
            (case["context"].as_str().unwrap().to_owned(), keystrokes)
        })
        .collect()
}

fn texts(k: &Keystroke) -> Vec<&str> {
    k.candidates.iter().map(String::as_str).collect()
}

fn assert_within(got: &[f32], want: &[f32], tolerance: f32, what: &str) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < tolerance, "{what}: {got:?} vs {want:?}");
    }
}

#[test]
fn typing_matches_the_reference_runtime() {
    let mut scorer = scorer();
    let cases = cases();
    assert_eq!(cases.len(), 10);
    for (context, keystrokes) in &cases {
        for (i, k) in keystrokes.iter().enumerate() {
            let texts: Vec<&str> = k.candidates.iter().map(String::as_str).collect();
            assert_within(
                &scorer.log_probabilities(context, &texts),
                &k.scores,
                TOLERANCE,
                &format!("{context:?} keystroke {i}"),
            );
        }
    }
}

#[test]
fn a_fresh_scorer_agrees_with_one_that_kept_its_cache() {
    for (context, keystrokes) in &cases() {
        let last = keystrokes.last().unwrap();
        let texts: Vec<&str> = last.candidates.iter().map(String::as_str).collect();
        assert_within(
            &scorer().log_probabilities(context, &texts),
            &last.scores,
            REBATCHED,
            context,
        );
    }
}

#[test]
fn going_back_to_an_earlier_context_starts_over() {
    let mut scorer = scorer();
    let cases = cases();
    let (first, first_keys) = &cases[0];
    let (second, second_keys) = &cases[2];
    scorer.log_probabilities(first, &texts(&first_keys[0]));
    scorer.log_probabilities(second, &texts(&second_keys[0]));
    let back = first_keys.last().unwrap();
    assert_within(
        &scorer.log_probabilities(first, &texts(back)),
        &back.scores,
        REBATCHED,
        "back to the first context",
    );
}

#[test]
fn nothing_to_score_declines() {
    let mut scorer = scorer();
    assert!(scorer.log_probabilities("我们", &[]).is_empty());
    assert!(scorer.log_probabilities("我们", &["去", ""]).is_empty());
}
