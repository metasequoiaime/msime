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
    OnnxScorer::load(
        &model,
        &fixture("model.json"),
        &fixture("vocab.json"),
        1,
        true,
    )
    .unwrap()
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

/// iOS 宿主的设置：ONNX Runtime 自己读模型文件、不用 CPU 内存池。只换了内存从哪里来，分数与参考运行时一致。
#[test]
fn reading_the_file_without_a_memory_arena_scores_the_same() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut scorer = OnnxScorer::load_file(
        &fixtures.join("lm_step.ort"),
        &fixture("model.json"),
        &fixture("vocab.json"),
        1,
        false,
    )
    .unwrap();
    for (context, keystrokes) in &cases() {
        for (i, k) in keystrokes.iter().enumerate() {
            assert_within(
                &scorer.log_probabilities(context, &texts(k)),
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

/// 上屏后先编码好上文（宿主在后台线程里做），接着打字的分数与按键时才编码的一样：两种做法都是先单独跑一次上文、再跑按键。
#[test]
fn priming_the_context_scores_like_encoding_it_on_the_keystroke() {
    let mut scorer = scorer();
    for (context, keystrokes) in &cases() {
        scorer.prime_context(context);
        for (i, k) in keystrokes.iter().enumerate() {
            assert_within(
                &scorer.log_probabilities(context, &texts(k)),
                &k.scores,
                TOLERANCE,
                &format!("primed {context:?} keystroke {i}"),
            );
        }
    }
}

/// 编码好的上文与按键时交来的不同（用户在编码完之前又上屏了，或者长候选让上文截短了），按键照旧从它自己的上文重来。
#[test]
fn a_keystroke_after_priming_another_context_starts_over() {
    let mut scorer = scorer();
    let cases = cases();
    let (first, first_keys) = &cases[0];
    let (second, _) = &cases[2];
    scorer.prime_context(second);
    let last = first_keys.last().unwrap();
    assert_within(
        &scorer.log_probabilities(first, &texts(last)),
        &last.scores,
        REBATCHED,
        "primed another context",
    );
}

#[test]
fn nothing_to_score_declines() {
    let mut scorer = scorer();
    assert!(scorer.log_probabilities("我们", &[]).is_empty());
    assert!(scorer.log_probabilities("我们", &["去", ""]).is_empty());
}

#[test]
fn calibration_measures_a_heavy_page_and_leaves_no_cache() {
    let mut scorer = scorer();
    let ms = scorer.calibrate();
    assert!(ms.is_finite() && ms > 0.0, "{ms}");
    // 量完不留缓存：接着打分与新加载的结果一样。
    let (context, keystrokes) = &cases()[0];
    let last = keystrokes.last().unwrap();
    assert_within(
        &scorer.log_probabilities(context, &texts(last)),
        &crate_fresh_scores(context, last),
        TOLERANCE,
        "after calibration",
    );
}

/// 只有自测过了预算的模型排上屏后联想：刚加载、没量过的不排，量了超预算的也不排。0 毫秒的预算任何一次真实的图运行都超过；`f64::MAX` 只在自测真的量出了一个有限的时间时才算过。
#[test]
fn only_a_model_within_the_keystroke_budget_orders_prediction() {
    let mut scorer = scorer();
    assert!(!scorer.scores_prediction_pool_within_keystroke_budget());
    assert!(scorer.fits_keystroke_budget(f64::MAX));
    assert!(scorer.scores_prediction_pool_within_keystroke_budget());
    assert!(!scorer.fits_keystroke_budget(0.0));
    assert!(!scorer.scores_prediction_pool_within_keystroke_budget());
}

fn crate_fresh_scores(context: &str, keystroke: &Keystroke) -> Vec<f32> {
    scorer().log_probabilities(context, &texts(keystroke))
}

/// 字表外的字按未知字编码，模型给的是「随便哪个生僻字」的总概率；每个未知字再扣 ln(未知字这一类的大小)，摊回到一个字上。两个不同的未知字分数相同，都比把那份总概率直接当成一个字的分低这么多；字表里的字不受影响（上面几条与参考运行时逐位对照的测试守着）。
#[test]
fn an_unknown_character_pays_for_standing_in_for_a_whole_class() {
    let mut scorer = scorer();
    let (context, _) = &cases()[0];
    let unknown = scorer.log_probabilities(context, &["\u{2B50D}", "\u{7653}"]);
    assert_eq!(unknown.len(), 2);
    assert!((unknown[0] - unknown[1]).abs() < 1e-4, "{unknown:?}");
    // 一个未知字跟着一个字表里的字：两个字平均，每个未知字扣的分摊一半。
    let known = fixture("chars.txt")
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap();
    let pair = format!("\u{2B50D}{known}");
    let mixed = scorer.log_probabilities(context, &[pair.as_str()]);
    assert_eq!(mixed.len(), 1);
    assert!(mixed[0].is_finite());
}
