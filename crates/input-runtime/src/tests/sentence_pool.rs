//! 整句读法池：挂着报了 `sentence_pool` 的逐键打分器时，引擎每条切分解满一次比较的条数，排在默认 6 条之后的读法也能被模型挑上首位；不报的打分器（随包模型）和不挂打分器时，候选与以前逐行相同。

use super::*;

/// ta 三个字、shuo 三个字，没有词组也没有二元表：九种读法按两字词频之积排，第 7 到第 9 条是 他朔 她朔 它朔。
const POOL_DICTIONARY: &str = "CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_t VALUES('ta','t','他',1000);
INSERT INTO tbl_1_t VALUES('ta','t','她',900);
INSERT INTO tbl_1_t VALUES('ta','t','它',800);
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_s VALUES('shuo','s','说',1000);
INSERT INTO tbl_1_s VALUES('shuo','s','硕',100);
INSERT INTO tbl_1_s VALUES('shuo','s','朔',50);";

/// 按词频排在第九的读法。
const NINTH: &str = "它朔";

/// 只给 `NINTH` 高分，其余 -5；`pool` 是它报的 `sentence_pool`。
struct Favouring {
    pool: usize,
}

impl CandidateScorer for Favouring {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        candidates
            .iter()
            .map(|text| if *text == NINTH { 0.0 } else { -5.0 })
            .collect()
    }

    fn sentence_pool(&self) -> usize {
        self.pool
    }
}

fn runtime(scorer: Option<Box<dyn CandidateScorer>>) -> (tempfile::TempDir, Runtime) {
    let directory = tempfile::tempdir().unwrap();
    let options = real_engine_options(directory.path());
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch(POOL_DICTIONARY)
        .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 9).unwrap();
    runtime.focus(true).unwrap();
    runtime.set_candidate_scorer(scorer);
    (directory, runtime)
}

fn sentences_after_typing(runtime: &mut Runtime) -> Vec<String> {
    for value in b"tashuo" {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    runtime
        .all_candidates()
        .candidates
        .iter()
        .filter(|row| row.text.chars().count() == 2 && !row.text.is_ascii())
        .map(|row| row.text.clone())
        .collect()
}

#[test]
fn without_a_pool_the_lattice_keeps_six_readings() {
    let (_directory, mut runtime) = runtime(None);
    let sentences = sentences_after_typing(&mut runtime);
    assert_eq!(sentences.len(), 6, "{sentences:?}");
    assert!(!sentences.iter().any(|text| text == NINTH));
}

#[test]
fn a_scorer_without_a_pool_sees_the_same_readings() {
    let (_directory, mut plain) = runtime(None);
    let (_directory, mut scored) = runtime(Some(Box::new(Favouring { pool: 0 })));
    assert_eq!(
        sentences_after_typing(&mut scored),
        sentences_after_typing(&mut plain)
    );
}

#[test]
fn a_pooled_scorer_can_promote_a_reading_past_the_sixth() {
    let (_directory, mut runtime) = runtime(Some(Box::new(Favouring {
        pool: msime_engine::ordering::SENTENCE_POOL,
    })));
    let sentences = sentences_after_typing(&mut runtime);
    assert_eq!(sentences.len(), 9, "{sentences:?}");
    assert_eq!(sentences[0], NINTH);
}

#[test]
fn removing_the_scorer_shrinks_the_pool_again() {
    let (_directory, mut runtime) = runtime(Some(Box::new(Favouring {
        pool: msime_engine::ordering::SENTENCE_POOL,
    })));
    runtime.set_candidate_scorer(None);
    let sentences = sentences_after_typing(&mut runtime);
    assert_eq!(sentences.len(), 6, "{sentences:?}");
}
