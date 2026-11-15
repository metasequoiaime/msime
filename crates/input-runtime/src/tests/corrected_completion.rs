use super::*;

struct PreferCompletion;

impl CandidateScorer for PreferCompletion {
    fn context_length(&self) -> usize {
        64
    }
    fn log_probabilities(&mut self, _: &str, candidates: &[&str]) -> Vec<f32> {
        candidates
            .iter()
            .map(|word| {
                if *word == "应用程序" {
                    100.0
                } else {
                    -10.0
                }
            })
            .collect()
    }
}

fn runtime(root: &std::path::Path) -> Runtime {
    let options = real_engine_options(root);
    rusqlite::Connection::open(std::path::Path::new(&options.dictionaries).join("msime-pinyin.db"))
        .unwrap()
        .execute_batch(
            r#"
            CREATE TABLE tbl_2_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);
            INSERT INTO tbl_2_y VALUES('ying''yong','yy','应用',1000);
            INSERT INTO tbl_2_y VALUES('ying''yong','yy','英勇',50);
            CREATE TABLE tbl_3_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);
            INSERT INTO tbl_3_y VALUES('ying''yong''de','yyd','应用的',500);
            CREATE TABLE tbl_4_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);
            INSERT INTO tbl_4_y VALUES('ying''yong''cheng''xu','yycx','应用程序',100);
            "#,
        )
        .unwrap();
    Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 6).unwrap()
}

#[test]
fn corrected_completion_cannot_lead_in_keystroke_or_settled_reranking() {
    for (settled, typed) in [
        (false, b"yigyong".as_slice()),
        (true, b"yigyong".as_slice()),
        (false, b"yingyong".as_slice()),
        (true, b"yingyong".as_slice()),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut runtime = runtime(directory.path());
        runtime.focus(true).unwrap();
        if settled {
            runtime.set_settled_candidate_scorer(Some(Box::new(PreferCompletion)));
        } else {
            runtime.set_candidate_scorer(Some(Box::new(PreferCompletion)));
        }
        for value in typed {
            runtime
                .dispatch(Action::Character {
                    value: *value,
                    shift: false,
                })
                .unwrap();
        }
        if settled {
            runtime.rerank_settled();
        }
        let view = runtime.view();
        let words: Vec<_> = view
            .candidates
            .iter()
            .map(|row| row.text.as_str())
            .collect();
        assert_eq!(words[0], "应用", "settled={settled}: {words:?}");
        assert!(words.contains(&"应用程序"), "{words:?}");
        let whole = runtime
            .cached
            .candidates
            .iter()
            .position(|word| word == "应用")
            .unwrap();
        assert!(runtime.cached.candidate_answers_key[whole]);
        assert_eq!(
            runtime.cached.candidate_corrected[whole],
            typed == b"yigyong"
        );
        let index = runtime
            .cached
            .candidates
            .iter()
            .position(|word| word == "应用程序")
            .unwrap();
        assert!(!runtime.cached.candidate_answers_key[index]);
        let completion = runtime
            .view()
            .candidates
            .iter()
            .find(|row| row.text == "应用程序")
            .unwrap()
            .id;
        let selected = runtime.dispatch(Action::Select(completion)).unwrap();
        assert_eq!(selected.commit.as_deref(), Some("应用程序"));
        assert!(selected.view.editing_text.is_empty());
    }
}

struct PreferAnotherWholeWord;

impl CandidateScorer for PreferAnotherWholeWord {
    fn context_length(&self) -> usize {
        64
    }
    fn log_probabilities(&mut self, _: &str, candidates: &[&str]) -> Vec<f32> {
        candidates
            .iter()
            .map(|word| match *word {
                "应用程序" => 100.0,
                "英勇" => 50.0,
                _ => -10.0,
            })
            .collect()
    }
}

#[test]
fn corrected_whole_words_still_compete_and_selected_identity_follows_the_order() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = runtime(directory.path());
    runtime.focus(true).unwrap();
    runtime.set_candidate_scorer(Some(Box::new(PreferAnotherWholeWord)));
    for value in b"yigyong" {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    let view = runtime.view();
    assert_eq!(view.candidates[0].text, "英勇");
    let chosen = runtime
        .dispatch(Action::Select(view.candidates[0].id))
        .unwrap();
    assert_eq!(chosen.commit.as_deref(), Some("英勇"));
    assert!(chosen.view.editing_text.is_empty());
}
