use super::*;
use msime_engine::ordering::{
    rerank_needs_model, rerank_pick, ParallelOrderRows, CONTEST_NONE, CONTEST_RECENCY,
    CONTEST_RUNNER_UP,
};

/// 两行都是回答整个按键的词典命中（来源 0），第一行是可信首选。
fn dictionary_rows(texts: &[&str]) -> (Vec<String>, Vec<u8>, Vec<bool>, Vec<bool>) {
    (
        texts.iter().map(|text| text.to_string()).collect(),
        vec![0; texts.len()],
        vec![true; texts.len()],
        vec![false; texts.len()],
    )
}

fn model(favoured: &[char]) -> Reranker {
    Reranker::new(std::sync::Arc::new(favouring_model(
        &['甲', '乙', '丙'],
        favoured,
    )))
}

#[test]
fn a_trusted_leader_without_a_contest_is_left_alone() {
    let (texts, sources, answers, corrected) = dictionary_rows(&["甲", "乙"]);
    let rows = ParallelOrderRows::new(&texts, &sources, &answers, &corrected);
    assert!(!rerank_needs_model(0, false, &rows));
    assert_eq!(rerank_pick(&mut model(&['乙']), "", 0, false, &rows), None);
}

#[test]
fn a_recency_contest_lets_the_model_seat_the_dictionary_leader() {
    let (texts, sources, answers, corrected) = dictionary_rows(&["甲", "乙", "丙"]);
    let contests = [CONTEST_RECENCY, CONTEST_RECENCY, CONTEST_NONE];
    let rows =
        ParallelOrderRows::new(&texts, &sources, &answers, &corrected).with_contests(&contests);
    assert!(rerank_needs_model(0, false, &rows));
    assert_eq!(
        rerank_pick(&mut model(&['乙']), "", 0, false, &rows),
        Some(1)
    );
    // 只比被标出的两行：模型更喜欢没有标记的 丙 也不把它提上来。
    assert_eq!(rerank_pick(&mut model(&['丙']), "", 0, false, &rows), None);
    // 模型站在最近选择一边，或者两行打平（差距不过 margin）时，最近选择留在首位。
    assert_eq!(rerank_pick(&mut model(&['甲']), "", 0, false, &rows), None);
    assert_eq!(
        rerank_pick(&mut model(&['甲', '乙']), "", 0, false, &rows),
        None
    );
}

#[test]
fn a_contest_needs_the_leader_marked_and_two_rows_that_answer_the_key() {
    let (texts, sources, mut answers, corrected) = dictionary_rows(&["甲", "乙"]);
    let rival_only = [CONTEST_NONE, CONTEST_RECENCY];
    let rows =
        ParallelOrderRows::new(&texts, &sources, &answers, &corrected).with_contests(&rival_only);
    assert_eq!(rerank_pick(&mut model(&['乙']), "", 0, false, &rows), None);

    answers[1] = false;
    let both = [CONTEST_RECENCY, CONTEST_RECENCY];
    let rows = ParallelOrderRows::new(&texts, &sources, &answers, &corrected).with_contests(&both);
    assert!(!rerank_needs_model(0, false, &rows));
    assert_eq!(rerank_pick(&mut model(&['乙']), "", 0, false, &rows), None);
}

#[test]
fn contest_marks_of_the_wrong_length_are_ignored() {
    let (texts, sources, answers, corrected) = dictionary_rows(&["甲", "乙"]);
    let short = [CONTEST_RECENCY];
    let rows = ParallelOrderRows::new(&texts, &sources, &answers, &corrected).with_contests(&short);
    assert_eq!(rerank_pick(&mut model(&['乙']), "", 0, false, &rows), None);
}

/// 韩文汉字表这类不重排的方案，标记也不起作用。
#[test]
fn a_contest_respects_the_schemes_that_never_reorder() {
    let (texts, sources, answers, corrected) = dictionary_rows(&["甲", "乙"]);
    let both = [CONTEST_RECENCY, CONTEST_RECENCY];
    let rows = ParallelOrderRows::new(&texts, &sources, &answers, &corrected).with_contests(&both);
    let hanja = (0..=u8::MAX)
        .find(|&scheme| !msime_engine::ordering::reorders_candidates(scheme))
        .expect("some scheme never reorders its candidates");
    assert_eq!(
        rerank_pick(&mut model(&['乙']), "", hanja, false, &rows),
        None
    );
}

/// 两种裁决点互不混比：首行标的是哪一种，就只和同种标记的行比。
#[test]
fn a_contest_compares_only_rows_of_the_leaders_kind() {
    let (texts, sources, answers, corrected) = dictionary_rows(&["甲", "乙", "丙"]);
    let contests = [CONTEST_RUNNER_UP, CONTEST_RECENCY, CONTEST_RUNNER_UP];
    let rows =
        ParallelOrderRows::new(&texts, &sources, &answers, &corrected).with_contests(&contests);
    assert_eq!(rerank_pick(&mut model(&['乙']), "", 0, false, &rows), None);
    assert_eq!(
        rerank_pick(&mut model(&['丙']), "", 0, false, &rows),
        Some(2)
    );
}

/// 裁决标记只交给逐键模型。同一个更喜欢 乙 的模型挂成逐键模型时 乙 坐上首位；只挂成落定模型时，停顿重排照旧不越过可信的词典首选，也不为它把模型移出会话线程：裁决点的 margin 是在随包的键盘模型上调的，落定那一层没有量过。
#[test]
fn only_the_keystroke_model_settles_a_contest() {
    let typed = |keystroke: bool| {
        let mut runtime = Runtime::new(
            Fixture {
                local_mode: "none".into(),
                words: vec!["甲".into(), "乙".into()],
                codes: vec!["k".into(), "k".into()],
                contests: vec![CONTEST_RECENCY, CONTEST_RECENCY],
                ..Fixture::default()
            },
            5,
        )
        .unwrap();
        if keystroke {
            runtime.set_reranker(Some(model(&['乙'])));
        } else {
            runtime.set_settled_reranker(Some(model(&['乙'])));
            runtime.set_settled_rerank_enabled(true);
        }
        runtime.focus(true).unwrap();
        runtime
            .dispatch(Action::Character {
                value: b'k',
                shift: false,
            })
            .unwrap();
        runtime
    };
    let texts = |runtime: &Runtime<Fixture>| -> Vec<String> {
        runtime
            .view()
            .candidates
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect()
    };
    assert_eq!(texts(&typed(true)), ["乙", "甲"]);

    let mut settled = typed(false);
    assert_eq!(texts(&settled), ["甲", "乙"]);
    assert!(settled.begin_settled_rerank().is_none());
    settled.rerank_settled();
    assert_eq!(texts(&settled), ["甲", "乙"]);
}

/// 真引擎的九宫格，「增强」挂着一个偏爱 恁 的逐键模型：自动记下的最近选择 能 照旧让模型把首位还给 恁；用户手动置顶（置顶、固定到第 1 位）的 能 不交给模型，留在首位。
#[test]
fn a_manual_pin_stays_first_when_the_keystroke_model_prefers_another_word() {
    type Engine = msime_engine::host::Session;
    let id_of = |runtime: &Runtime<Engine>, text: &str| {
        runtime
            .view()
            .candidates
            .iter()
            .find(|candidate| candidate.text == text)
            .unwrap_or_else(|| panic!("{text} is on the first page"))
            .id
    };
    let leader_after = |manage: &dyn Fn(&mut Runtime<Engine>, CandidateId)| -> String {
        let directory = tempfile::tempdir().unwrap();
        let mut options = real_engine_options(directory.path());
        options.learning = true;
        let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
        rusqlite::Connection::open(database)
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
                 INSERT INTO tbl_1_n VALUES('neng','n','能',1),('neng','n','恁',100),('neng','n','嫩',90);",
            )
            .unwrap();
        let mut runtime = Runtime::new(Engine::new(&options).unwrap(), 5).unwrap();
        runtime.set_reranker(Some(Reranker::new(std::sync::Arc::new(favouring_model(
            &['能', '恁', '嫩'],
            &['恁'],
        )))));
        runtime.set_key_grid(Some(KeyGrid::NineKey)).unwrap();
        runtime.focus(true).unwrap();
        type_characters(&mut runtime, "6364");
        assert_eq!(runtime.view().candidates[0].text, "恁");
        let id = id_of(&runtime, "能");
        manage(&mut runtime, id);
        runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
        type_characters(&mut runtime, "6364");
        runtime.view().candidates[0].text.clone()
    };

    let select = |runtime: &mut Runtime<Engine>, id| {
        let selected = runtime.dispatch(Action::Select(id)).unwrap();
        assert_eq!(selected.commit.as_deref(), Some("能"));
    };
    assert_eq!(leader_after(&select), "恁");

    let pin = |runtime: &mut Runtime<Engine>, id| {
        runtime.dispatch(Action::PinCandidate(id)).unwrap();
        assert_eq!(runtime.view().candidates[0].text, "能");
    };
    assert_eq!(leader_after(&pin), "能");

    let fix = |runtime: &mut Runtime<Engine>, id| {
        runtime
            .dispatch(Action::FixCandidatePosition(id, 1))
            .unwrap();
        assert_eq!(runtime.view().candidates[0].text, "能");
    };
    assert_eq!(leader_after(&fix), "能");
}
