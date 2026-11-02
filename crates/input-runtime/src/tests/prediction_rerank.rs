//! 上屏后联想经逐键打分器排序：只在挂了排一池仍在逐键预算之内的打分器（`scores_prediction_pool_within_keystroke_budget`）时多取候选池、排过之后仍是 8 行，座位就是引擎的下标，选中哪一行上屏的就是哪一行；不报这一条的打分器（随包模型、退到 6x256 的 ONNX 后端）挂着时联想与没挂时逐行相同。

use super::*;
use std::sync::Mutex;

/// 「吃」之后 11 条词库续写，按词频是 饭 好 喝 药 苦 亏 惊 力 素 醋 东西。
const POOL_DICTIONARY: &str = "CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_c VALUES('chi','c','吃',1000);
CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_c VALUES('chi''fan','cf','吃饭',100);
INSERT INTO tbl_2_c VALUES('chi''hao','ch','吃好',95);
INSERT INTO tbl_2_c VALUES('chi''he','ch','吃喝',90);
INSERT INTO tbl_2_c VALUES('chi''yao','cy','吃药',85);
INSERT INTO tbl_2_c VALUES('chi''ku','ck','吃苦',80);
INSERT INTO tbl_2_c VALUES('chi''kui','ck','吃亏',75);
INSERT INTO tbl_2_c VALUES('chi''jing','cj','吃惊',70);
INSERT INTO tbl_2_c VALUES('chi''li','cl','吃力',65);
INSERT INTO tbl_2_c VALUES('chi''su','cs','吃素',60);
INSERT INTO tbl_2_c VALUES('chi''cu','cc','吃醋',55);
CREATE TABLE tbl_3_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_3_c VALUES('chi''dong''xi','cdx','吃东西',50);
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_w VALUES('wo','w','我',1000);";

const BY_WEIGHT: [&str; 8] = ["饭", "好", "喝", "药", "苦", "亏", "惊", "力"];

type Calls = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// 逐字平均分照表给，表外 -5。按键重排也会调它，所以记下每一次的上文和候选，测试按候选认出联想的那一次。`fast` 是它报的 `scores_prediction_pool_within_keystroke_budget`。
struct Stub {
    means: Vec<(&'static str, f32)>,
    calls: Calls,
    fast: bool,
}

impl CandidateScorer for Stub {
    fn context_length(&self) -> usize {
        64
    }

    fn scores_prediction_pool_within_keystroke_budget(&self) -> bool {
        self.fast
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        self.calls.lock().unwrap().push((
            context.to_owned(),
            candidates.iter().map(|text| (*text).to_owned()).collect(),
        ));
        candidates
            .iter()
            .map(|text| {
                self.means
                    .iter()
                    .find(|(row, _)| row == text)
                    .map_or(-5.0, |(_, mean)| *mean)
            })
            .collect()
    }
}

/// 在逐键预算之内、会排联想的打分器，像过了自测预算的 ONNX 大模型。
fn stub(means: &[(&'static str, f32)]) -> (Box<dyn CandidateScorer>, Calls) {
    stub_with_speed(means, true)
}

/// 不在预算之内、只做逐键重排的打分器，像随包模型或退到 6x256 的 ONNX 后端。
fn slow_stub(means: &[(&'static str, f32)]) -> (Box<dyn CandidateScorer>, Calls) {
    stub_with_speed(means, false)
}

fn stub_with_speed(means: &[(&'static str, f32)], fast: bool) -> (Box<dyn CandidateScorer>, Calls) {
    let calls = Calls::default();
    (
        Box::new(Stub {
            means: means.to_vec(),
            calls: calls.clone(),
            fast,
        }),
        calls,
    )
}

/// 联想那一次打分（候选里有 东西）读到的上文。
fn prediction_contexts(calls: &Calls) -> Vec<String> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, candidates)| candidates.iter().any(|text| text == "东西"))
        .map(|(context, _)| context.clone())
        .collect()
}

fn engine(directory: &std::path::Path) -> msime_engine::host::Session {
    let mut options = real_engine_options(directory);
    options.next_word_prediction = true;
    options.learning = false;
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    if !database.exists() {
        rusqlite::Connection::open(database)
            .unwrap()
            .execute_batch(POOL_DICTIONARY)
            .unwrap();
    }
    msime_engine::host::Session::new(&options).unwrap()
}

fn runtime(scorer: Option<Box<dyn CandidateScorer>>) -> (tempfile::TempDir, Runtime) {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = Runtime::new(engine(directory.path()), 9).unwrap();
    runtime.focus(true).unwrap();
    runtime.set_candidate_scorer(scorer);
    runtime.seed_context("今天");
    (directory, runtime)
}

fn type_keys(runtime: &mut Runtime, keys: &[u8]) {
    for value in keys {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
}

fn select_text(runtime: &mut Runtime, text: &str) -> Transition {
    let id = runtime
        .view()
        .candidates
        .iter()
        .find(|row| row.text == text)
        .unwrap_or_else(|| panic!("{text} is not among the candidates"))
        .id;
    runtime.dispatch(Action::Select(id)).unwrap()
}

fn commit_chi(runtime: &mut Runtime) {
    type_keys(runtime, b"chi");
    assert_eq!(select_text(runtime, "吃").commit.as_deref(), Some("吃"));
    assert!(runtime.view().prediction);
}

fn texts(runtime: &Runtime) -> Vec<String> {
    runtime
        .view()
        .candidates
        .iter()
        .map(|row| row.text.clone())
        .collect()
}

#[test]
fn without_a_scorer_prediction_keeps_the_dictionary_order() {
    let (_directory, mut runtime) = runtime(None);
    commit_chi(&mut runtime);
    assert_eq!(texts(&runtime), BY_WEIGHT);
}

#[test]
fn a_scorer_orders_the_dictionary_pool_against_the_committed_text() {
    let (scorer, calls) = stub(&[("东西", -1.0), ("醋", -1.5)]);
    let (_directory, mut runtime) = runtime(Some(scorer));
    commit_chi(&mut runtime);
    assert_eq!(
        texts(&runtime),
        ["醋", "东西", "饭", "好", "喝", "药", "苦", "亏"]
    );
    // 一次上屏只打一次分，上文是之前写下的 今天 接上刚上屏的 吃。
    assert_eq!(prediction_contexts(&calls), ["今天吃"]);
    // 之后不改变联想的刷新（翻译回包、在线候选）不再打分。
    runtime.refresh().unwrap();
    assert_eq!(prediction_contexts(&calls).len(), 1);
}

#[test]
fn every_reordered_seat_commits_the_row_it_shows() {
    let means = [("东西", -1.0), ("醋", -1.5), ("素", -1.6)];
    let (scorer, _) = stub(&means);
    let (_directory, mut shown) = runtime(Some(scorer));
    commit_chi(&mut shown);
    let rows = texts(&shown);
    assert_eq!(rows[..3], ["醋", "素", "东西"]);
    for (seat, text) in rows.iter().enumerate() {
        let (scorer, _) = stub(&means);
        let (_directory, mut runtime) = runtime(Some(scorer));
        commit_chi(&mut runtime);
        let id = runtime.view().candidates[seat].id;
        let transition = runtime.dispatch(Action::Select(id)).unwrap();
        assert_eq!(
            transition.commit.as_deref(),
            Some(text.as_str()),
            "seat {seat}"
        );
    }
}

#[test]
fn an_id_from_the_reordered_view_goes_stale_with_the_next_key() {
    let (scorer, _) = stub(&[("东西", 0.0)]);
    let (_directory, mut runtime) = runtime(Some(scorer));
    commit_chi(&mut runtime);
    let id = runtime.view().candidates[0].id;
    type_keys(&mut runtime, b"w");
    assert!(matches!(
        runtime.dispatch(Action::Select(id)),
        Err(RuntimeError::StaleCandidate)
    ));
}

#[test]
fn detaching_the_scorer_returns_to_the_dictionary_order() {
    let (scorer, calls) = stub(&[("东西", 0.0)]);
    let (_directory, mut runtime) = runtime(Some(scorer));
    runtime.set_candidate_scorer(None);
    commit_chi(&mut runtime);
    assert_eq!(texts(&runtime), BY_WEIGHT);
    assert!(prediction_contexts(&calls).is_empty());
}

#[test]
fn a_replaced_engine_still_collects_the_pool() {
    let (scorer, _) = stub(&[("东西", 0.0)]);
    let (directory, mut runtime) = runtime(Some(scorer));
    runtime.replace_engine(engine(directory.path()), 9).unwrap();
    commit_chi(&mut runtime);
    assert_eq!(texts(&runtime)[0], "东西");
}

#[test]
fn the_privacy_gate_still_suppresses_prediction_and_its_scoring() {
    let (scorer, calls) = stub(&[("东西", 0.0)]);
    let (_directory, mut runtime) = runtime(Some(scorer));
    runtime.set_prediction_allowed(false).unwrap();
    type_keys(&mut runtime, b"chi");
    assert_eq!(
        select_text(&mut runtime, "吃").commit.as_deref(),
        Some("吃")
    );
    assert!(runtime.view().candidates.is_empty());
    assert!(prediction_contexts(&calls).is_empty());
}

#[test]
fn a_held_phrase_piece_is_part_of_the_context() {
    let (scorer, calls) = stub(&[("东西", 0.0)]);
    let (_directory, mut runtime) = runtime(Some(scorer));
    runtime.set_phrase_preedit(true);
    type_keys(&mut runtime, b"wochi");
    assert!(select_text(&mut runtime, "我").commit.is_none());
    assert_eq!(runtime.view().phrase_prefix, "我");
    assert_eq!(
        select_text(&mut runtime, "吃").commit.as_deref(),
        Some("我吃")
    );
    assert_eq!(texts(&runtime)[0], "东西");
    assert_eq!(prediction_contexts(&calls), ["今天我吃"]);
}

/// 联想的每一行连同编码、附注和来源，供逐行比较。
fn rows(runtime: &Runtime) -> Vec<(String, String, String, u8)> {
    runtime
        .view()
        .candidates
        .iter()
        .map(|row| {
            (
                row.text.clone(),
                row.code.clone(),
                row.annotation.clone(),
                row.source,
            )
        })
        .collect()
}

#[test]
fn a_scorer_too_slow_for_a_commit_leaves_prediction_as_without_one() {
    let (_directory, mut plain) = runtime(None);
    commit_chi(&mut plain);
    let (scorer, calls) = slow_stub(&[("东西", -1.0), ("醋", -1.5)]);
    let (_directory, mut slow) = runtime(Some(scorer));
    commit_chi(&mut slow);
    assert_eq!(texts(&slow), BY_WEIGHT);
    assert_eq!(rows(&slow), rows(&plain));
    // 词库没多取，也没给联想打分。
    assert!(prediction_contexts(&calls).is_empty());
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .all(|(_, candidates)| !candidates.iter().any(|text| text == "饭")));
}

/// 同一个随包模型，只是报自己够快：对照用，说明下面那一条不排是速度的门禁，不是模型给不出顺序。
struct FastBundled(Reranker);

impl CandidateScorer for FastBundled {
    fn context_length(&self) -> usize {
        CandidateScorer::context_length(&self.0)
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        self.0.log_probabilities(context, candidates)
    }

    fn scores_prediction_pool_within_keystroke_budget(&self) -> bool {
        true
    }
}

#[test]
fn the_bundled_model_does_not_order_prediction() {
    let model = std::sync::Arc::new(favouring_model(&['吃', '东', '西', '醋'], &['醋']));
    let (_directory, mut bundled) = runtime(None);
    bundled.set_reranker(Some(Reranker::new(model.clone())));
    commit_chi(&mut bundled);
    assert_eq!(texts(&bundled), BY_WEIGHT);
    let (_directory, mut fast) = runtime(Some(Box::new(FastBundled(Reranker::new(model)))));
    commit_chi(&mut fast);
    assert_eq!(texts(&fast)[0], "醋");
}

#[test]
fn swapping_in_a_slow_scorer_closes_the_pool() {
    let (fast, _) = stub(&[("东西", 0.0)]);
    let (_directory, mut runtime) = runtime(Some(fast));
    let (slow, calls) = slow_stub(&[("东西", 0.0)]);
    runtime.set_candidate_scorer(Some(slow));
    commit_chi(&mut runtime);
    assert_eq!(texts(&runtime), BY_WEIGHT);
    assert!(prediction_contexts(&calls).is_empty());
    let (fast, calls) = stub(&[("东西", 0.0)]);
    runtime.set_candidate_scorer(Some(fast));
    commit_chi(&mut runtime);
    assert_eq!(texts(&runtime)[0], "东西");
    assert_eq!(prediction_contexts(&calls).len(), 1);
}

#[test]
fn a_replaced_engine_keeps_the_pool_closed_for_a_slow_scorer() {
    let (scorer, calls) = slow_stub(&[("东西", 0.0)]);
    let (directory, mut runtime) = runtime(Some(scorer));
    runtime.replace_engine(engine(directory.path()), 9).unwrap();
    commit_chi(&mut runtime);
    assert_eq!(texts(&runtime), BY_WEIGHT);
    assert!(prediction_contexts(&calls).is_empty());
}
