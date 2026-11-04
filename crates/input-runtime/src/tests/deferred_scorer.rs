//! 延后到停顿的逐键打分器（`Runtime::set_deferred_scorer`）：按键时不挂，引擎照没有模型时那样解码；停顿时 `settle_deferred_scorer` 挂上它重解一遍，一次只花一片模型时间，没解完的那一片不改屏幕上的候选，打过的分留在引擎的备忘里接着用。

use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// ta 三个字、shuo 三个字：九种读法按两字词频之积排，没有模型时第九条 `它朔` 不在候选里（见 `sentence_pool.rs`）。
const POOL_DICTIONARY: &str = "CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_t VALUES('ta','t','他',1000);
INSERT INTO tbl_1_t VALUES('ta','t','她',900);
INSERT INTO tbl_1_t VALUES('ta','t','它',800);
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_s VALUES('shuo','s','说',1000);
INSERT INTO tbl_1_s VALUES('shuo','s','硕',100);
INSERT INTO tbl_1_s VALUES('shuo','s','朔',50);";

const NINTH: &str = "它朔";

/// 束搜索里的模型：只给 `NINTH` 和它的前缀高分；记下每一次被要求打分的文字，每次调用先睡 `delay`，好让一片的时限在测试里也会用完。
struct Recording {
    delay: Duration,
    scored: Mutex<Vec<String>>,
}

impl msime_engine::ordering::SentenceScorer for Recording {
    fn log_probabilities(&self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        std::thread::sleep(self.delay);
        self.scored
            .lock()
            .unwrap()
            .extend(candidates.iter().map(|text| (*text).to_owned()));
        candidates
            .iter()
            .map(|text| if NINTH.starts_with(text) { 0.0 } else { -5.0 })
            .collect()
    }
}

/// 把 `Recording` 交给引擎的逐键打分器；自己的重排分对谁都一样，顺序只由束搜索决定。
struct Lends(Arc<Recording>);

impl CandidateScorer for Lends {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        vec![-5.0; candidates.len()]
    }

    fn sentence_scorer(&self) -> Option<Arc<dyn msime_engine::ordering::SentenceScorer>> {
        Some(self.0.clone())
    }
}

fn runtime(delay: Duration) -> (tempfile::TempDir, Runtime, Arc<Recording>) {
    let directory = tempfile::tempdir().unwrap();
    let options = real_engine_options(directory.path());
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch(POOL_DICTIONARY)
        .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 9).unwrap();
    runtime.focus(true).unwrap();
    let model = Arc::new(Recording {
        delay,
        scored: Mutex::default(),
    });
    runtime.set_deferred_scorer(Some(Box::new(Lends(model.clone()))));
    (directory, runtime, model)
}

fn type_tashuo(runtime: &mut Runtime) {
    for value in b"tashuo" {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
}

fn texts(runtime: &mut Runtime) -> Vec<String> {
    runtime
        .all_candidates()
        .candidates
        .into_iter()
        .map(|row| row.text)
        .collect()
}

fn first(runtime: &mut Runtime) -> String {
    runtime.all_candidates().candidates[0].text.clone()
}

#[test]
fn a_deferred_scorer_stays_off_the_keys() {
    let (_directory, mut runtime, model) = runtime(Duration::ZERO);
    type_tashuo(&mut runtime);
    assert!(model.scored.lock().unwrap().is_empty());
    assert_ne!(first(&mut runtime), NINTH);
    assert!(runtime.has_reranker() && runtime.has_deferred_scorer());
}

#[test]
fn settling_decodes_again_with_the_model_and_moves_to_a_new_generation() {
    let (_directory, mut runtime, model) = runtime(Duration::ZERO);
    type_tashuo(&mut runtime);
    let before = runtime.generation();
    assert_eq!(
        runtime
            .settle_deferred_scorer(Duration::from_secs(60))
            .unwrap(),
        DeferredSettle::Done { moved: true }
    );
    assert_eq!(first(&mut runtime), NINTH);
    assert!(runtime.generation() > before);
    assert!(!model.scored.lock().unwrap().is_empty());
    // 停顿之后的按键又不带模型：退格之后模型一次也没再被调用。
    let calls = model.scored.lock().unwrap().len();
    runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(model.scored.lock().unwrap().len(), calls);
}

#[test]
fn a_slice_that_runs_out_leaves_the_view_alone_and_the_next_one_carries_on() {
    let (_directory, mut runtime, model) = runtime(Duration::from_millis(5));
    type_tashuo(&mut runtime);
    let shown = texts(&mut runtime);
    let mut slices = 0;
    loop {
        slices += 1;
        assert!(slices < 100, "the settle never finished");
        match runtime
            .settle_deferred_scorer(Duration::from_millis(1))
            .unwrap()
        {
            DeferredSettle::Continue { moved } => {
                assert!(!moved);
                assert_eq!(texts(&mut runtime), shown);
            }
            DeferredSettle::Done { moved } => {
                assert!(moved);
                break;
            }
        }
    }
    assert!(
        slices > 1,
        "a 1 ms slice against a 5 ms model call should run out"
    );
    assert_eq!(first(&mut runtime), NINTH);
    // 每一片都接着前一片打过的分：同一段文字只交给模型一次。
    let scored = model.scored.lock().unwrap();
    let distinct: std::collections::HashSet<&String> = scored.iter().collect();
    assert_eq!(distinct.len(), scored.len(), "{scored:?}");
}

#[test]
fn a_second_pause_reuses_the_scores_of_the_first() {
    let (_directory, mut runtime, model) = runtime(Duration::ZERO);
    type_tashuo(&mut runtime);
    runtime
        .settle_deferred_scorer(Duration::from_secs(60))
        .unwrap();
    let calls = model.scored.lock().unwrap().len();
    runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    runtime
        .dispatch(Action::Character {
            value: b'o',
            shift: false,
        })
        .unwrap();
    runtime
        .settle_deferred_scorer(Duration::from_secs(60))
        .unwrap();
    assert_eq!(model.scored.lock().unwrap().len(), calls);
    assert_eq!(first(&mut runtime), NINTH);
}

#[test]
fn switching_the_keystroke_scorer_off_drops_the_deferred_one() {
    let (_directory, mut runtime, _model) = runtime(Duration::ZERO);
    runtime.set_candidate_scorer(None);
    assert!(!runtime.has_reranker() && !runtime.has_deferred_scorer());
    type_tashuo(&mut runtime);
    assert_eq!(
        runtime
            .settle_deferred_scorer(Duration::from_secs(60))
            .unwrap(),
        DeferredSettle::Done { moved: false }
    );
    assert_ne!(first(&mut runtime), NINTH);
}

#[test]
fn nothing_is_settled_without_a_composition() {
    let (_directory, mut runtime, model) = runtime(Duration::ZERO);
    assert_eq!(
        runtime
            .settle_deferred_scorer(Duration::from_secs(60))
            .unwrap(),
        DeferredSettle::Done { moved: false }
    );
    assert!(model.scored.lock().unwrap().is_empty());
}
