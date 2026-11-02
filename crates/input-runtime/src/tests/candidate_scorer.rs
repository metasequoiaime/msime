use super::*;

/// 只提供分数的打分器，挑选走 [`CandidateScorer::best_where`] 的默认规则；原生模型后端就是这样接入的。
struct ScoresOnly(Reranker);

impl CandidateScorer for ScoresOnly {
    fn context_length(&self) -> usize {
        CandidateScorer::context_length(&self.0)
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        self.0.log_probabilities(context, candidates)
    }
}

const CHARACTERS: [char; 12] = [
    '甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸', '子', '丑',
];

/// 默认规则与随包模型自带的 `best_where` 逐条一致：首行可信或不回答按键时不动，只比较前九条回答按键的候选，少于两条不动。
#[test]
fn the_default_pick_matches_the_bundled_reranker() {
    let mut cases = 0;
    for count in 1..=CHARACTERS.len() {
        let texts: Vec<String> = CHARACTERS[..count].iter().map(char::to_string).collect();
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        for &favoured in &CHARACTERS[..count] {
            let model = || {
                Reranker::new(std::sync::Arc::new(favouring_model(
                    &CHARACTERS,
                    &[favoured],
                )))
            };
            let (mut bundled, mut default) = (model(), ScoresOnly(model()));
            // 每一位是一条候选是否回答按键；超过 9 条时第十条以后由最后一位决定，覆盖「只比较前九条」的截断。
            for answers in 0u32..(1 << count.min(10)) {
                for trusted in [false, true] {
                    let facts = |index: usize| CandidateFacts {
                        answers_key: answers >> index.min(9) & 1 == 1,
                        trusted_dictionary_hit: index == 0 && trusted,
                    };
                    assert_eq!(
                        CandidateScorer::best_where(&mut default, "", &texts, &facts),
                        bundled.best_where("", &texts, facts),
                        "{count} candidates, favoured {favoured}, answers {answers:b}, trusted {trusted}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert!(cases > 10_000, "{cases}");
}

/// 经 `set_candidate_scorer` 挂上的打分器和经 `set_reranker` 挂上的随包模型给出同样的首选。
#[test]
fn a_boxed_scorer_reranks_like_the_bundled_model() {
    let texts = ["甲", "乙", "丙"].map(String::from);
    let sources = [LATTICE_SOURCE; 3];
    let answers = [true; 3];
    let corrected = [false; 3];
    let rows =
        msime_engine::ordering::ParallelOrderRows::new(&texts, &sources, &answers, &corrected);
    for favoured in ['甲', '乙', '丙'] {
        let model = || {
            Reranker::new(std::sync::Arc::new(favouring_model(
                &CHARACTERS,
                &[favoured],
            )))
        };
        let mut boxed: Box<dyn CandidateScorer> = Box::new(ScoresOnly(model()));
        assert_eq!(
            msime_engine::ordering::rerank_pick(boxed.as_mut(), "", 0, false, &rows),
            msime_engine::ordering::rerank_pick(&mut model(), "", 0, false, &rows),
            "favoured {favoured}"
        );
    }
}

/// 记下 runtime 每次告诉它上文变了时交来的上文。
struct ContextRecorder(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl CandidateScorer for ContextRecorder {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        vec![-1.0; candidates.len()]
    }

    fn context_changed(&mut self, context: &str) {
        self.0.lock().unwrap().push(context.to_owned());
    }
}

/// 每次上屏和宿主送来上文之后，挂着的打分器都拿到此后排序所依据的全部上文，好在下一次按键之前把它编码好；失焦清空上文时不通知。
#[test]
fn the_scorer_hears_each_new_context_after_a_commit() {
    let directory = tempfile::tempdir().unwrap();
    let options = real_engine_options(directory.path());
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_1_t VALUES('ta','t','他',1000);",
        )
        .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 9).unwrap();
    runtime.focus(true).unwrap();
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    runtime.set_candidate_scorer(Some(Box::new(ContextRecorder(std::sync::Arc::clone(
        &heard,
    )))));

    runtime.seed_context("我们");
    for value in b"ta" {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    let committed = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(committed.commit.as_deref(), Some("他"));
    runtime.focus(false).unwrap();

    assert_eq!(*heard.lock().unwrap(), ["我们", "我们他"]);
}
