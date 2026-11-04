//! 一句话开头的语气词：没有组字时按的回车记成上文里的换行，开头单打 `ma`、`ba` 时句尾语气词「吗」「吧」让给后面的单字；句中照常。

use super::*;

/// 「吗」的词频远高于「妈」，「吧」略高于「爸」，与锁定词库里的先后相同。
const PARTICLE_DICTIONARY: &str =
    "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_m VALUES('ma','m','吗',5664547);
INSERT INTO tbl_1_m VALUES('ma','m','妈',110515);
CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_b VALUES('ba','b','吧',8425271);
INSERT INTO tbl_1_b VALUES('ba','b','爸',6909598);
CREATE TABLE tbl_1_a(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_a VALUES('a','a','啊',3000000);
INSERT INTO tbl_1_a VALUES('a','a','阿',1000000);";

fn runtime() -> (tempfile::TempDir, Runtime) {
    runtime_with("")
}

fn runtime_with(extra_rows: &str) -> (tempfile::TempDir, Runtime) {
    let directory = tempfile::tempdir().unwrap();
    let options = real_engine_options(directory.path());
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch(&format!("{PARTICLE_DICTIONARY}\n{extra_rows}"))
        .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 9).unwrap();
    runtime.focus(true).unwrap();
    (directory, runtime)
}

fn first_after_typing(runtime: &mut Runtime, keys: &[u8]) -> String {
    for value in keys {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    let first = runtime.all_candidates().candidates[0].text.clone();
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    first
}

#[test]
fn a_return_with_nothing_composed_ends_the_sentence_in_the_context() {
    let (_directory, mut runtime) = runtime();
    runtime.seed_context("周末回家吃饭");
    let passed = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert!(!passed.handled);
    assert_eq!(runtime.ai_context, "周末回家吃饭\n");
}

#[test]
fn a_sentence_does_not_open_with_a_final_particle() {
    let (_directory, mut runtime) = runtime();
    // 上文为空：一句话的开头。
    assert_eq!(first_after_typing(&mut runtime, b"ma"), "妈");
    assert_eq!(first_after_typing(&mut runtime, b"ba"), "爸");
    // 「啊」也常作叹词放在句首，不让。
    assert_eq!(first_after_typing(&mut runtime, b"a"), "啊");
    // 回车之后又是开头。
    runtime.seed_context("好");
    assert_eq!(first_after_typing(&mut runtime, b"ma"), "吗");
    runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(first_after_typing(&mut runtime, b"ma"), "妈");
}

/// 给每段文字一个固定分数的打分器，没列出的文字一律 -20。
struct FixedScores(&'static [(&'static str, f32)]);

impl CandidateScorer for FixedScores {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        candidates
            .iter()
            .map(|text| {
                self.0
                    .iter()
                    .find(|(known, _)| known == text)
                    .map_or(-20.0, |(_, score)| *score)
            })
            .collect()
    }
}

/// 句首单字让模型挑时，比的是「字 + 停顿」：单看「马」它沾了「马上」的光，接上逗号后「妈，」才是更常见的一句开头。
#[test]
fn the_model_picks_the_opening_single_character_as_a_word_followed_by_a_pause() {
    let (_directory, mut runtime) =
        runtime_with("INSERT INTO tbl_1_m VALUES('ma','m','马',279925);");
    runtime.set_candidate_scorer(Some(Box::new(FixedScores(&[
        ("马", -1.0),
        ("妈", -3.0),
        ("马，", -9.0),
        ("妈，", -6.0),
    ]))));
    assert_eq!(first_after_typing(&mut runtime, b"ma"), "妈");
}
