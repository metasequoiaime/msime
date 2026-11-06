//! 十四／十八键网格辅助码在 runtime 一侧：动作交给引擎、视图转述它的字段，筛过的列表不再被呈现排序挪动。

use super::*;

const CODES: &[(&str, &str)] = &[("你", "ab"), ("拟", "cd"), ("逆", "ea"), ("好", "ef")];

fn grid_runtime(directory: &std::path::Path) -> Runtime<msime_engine::host::Session> {
    let mut options = real_engine_options(directory);
    options.helpcode = true;
    options.grid_helpcode = true;
    options.helpcode_table = Some(std::sync::Arc::new(
        msime_engine::HelpcodeKeymap::from_codes(
            CODES
                .iter()
                .map(|(word, code)| ((*word).to_owned(), (*code).to_owned()))
                .collect(),
        ),
    ));
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000),('ni''hao','nh','拟好',800),('ni''hao','nh','逆好',600);",
        )
        .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 9).unwrap();
    runtime.focus(true).unwrap();
    runtime.set_key_grid(Some(KeyGrid::FourteenKey)).unwrap();
    for letter in KeyGrid::FourteenKey.encode("nihao").bytes() {
        assert!(runtime.dispatch(Action::GridKey(letter)).unwrap().handled);
    }
    runtime
}

fn texts(view: &View) -> Vec<String> {
    view.candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect()
}

/// 动作经 runtime 到引擎，视图转述可用性、显示和整串解释；整串解释沿用拼音选择的代次校验。
#[test]
fn grid_helpcode_actions_reach_the_engine_and_its_choices_check_the_generation() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = grid_runtime(directory.path());
    let plain = runtime.view();
    assert!(plain.grid_helpcode_available);
    assert!(plain.grid_helpcode_display.is_empty());

    let begun = runtime
        .dispatch(Action::GridHelpcode(GridHelpcodeOp::Begin))
        .unwrap();
    assert!(begun.handled);
    assert_eq!(begun.view.grid_helpcode_display, "|");
    assert_eq!(texts(&begun.view), texts(&plain));
    assert!(begun.view.nine_key_spellings.is_empty());

    assert!(!runtime.dispatch(Action::GridKey(b'a')).unwrap().handled);
    runtime
        .dispatch(Action::GridHelpcode(GridHelpcodeOp::Group(b'a')))
        .unwrap();
    let view = runtime
        .dispatch(Action::GridHelpcode(GridHelpcodeOp::Group(b'e')))
        .unwrap()
        .view;
    assert_eq!(view.grid_helpcode_display, "|[AS][ER]");
    assert_eq!(view.nine_key_spellings, ["AE", "AR", "SE", "SR"]);
    assert_eq!(texts(&view), ["你好"]);

    let stale = NineKeySpellingId {
        session: view.session,
        generation: view.generation - 1,
        index: 0,
    };
    assert!(matches!(
        runtime.dispatch(Action::ChooseNineKeySpelling(stale)),
        Err(RuntimeError::StaleNineKeySpelling)
    ));
    let chosen = runtime
        .dispatch(Action::ChooseNineKeySpelling(NineKeySpellingId {
            session: view.session,
            generation: view.generation,
            index: 0,
        }))
        .unwrap();
    assert!(chosen.handled && chosen.commit.is_none());
    assert_eq!(chosen.view.grid_helpcode_display, "|AE");
    assert!(chosen.view.nine_key_spellings.is_empty());

    let refused = runtime
        .dispatch(Action::GridHelpcode(GridHelpcodeOp::Exact(b'1')))
        .unwrap();
    assert!(!refused.handled);
    let cleared = runtime
        .dispatch(Action::GridHelpcode(GridHelpcodeOp::Clear))
        .unwrap();
    assert!(cleared.view.grid_helpcode_display.is_empty());
    assert_eq!(texts(&cleared.view), texts(&plain));
}

/// 落定模型偏向最后一行：普通列表里它把那一行提上首选；网格辅助码筛过的列表不动，代次也不变。
#[test]
fn a_constrained_list_is_left_out_of_the_settled_rerank() {
    let words = ["甲", "乙", "丙"];
    for constrained in [false, true] {
        let mut runtime = Runtime::new(
            Fixture {
                local_mode: "none".into(),
                words: words.iter().map(|word| (*word).to_owned()).collect(),
                codes: (0..words.len()).map(|n| format!("c{n}")).collect(),
                sources: vec![msime_engine::ordering::LATTICE_SOURCE; words.len()],
                grid_helpcode_constrained: constrained,
                ..Fixture::default()
            },
            5,
        )
        .unwrap();
        runtime.focus(true).unwrap();
        type_key(&mut runtime);
        let model = favouring_model(&['甲', '乙', '丙'], &['丙']);
        runtime.set_settled_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
        let generation = runtime.view().generation;
        assert_eq!(runtime.rerank_settled(), !constrained);
        let view = runtime.view();
        if constrained {
            assert_eq!(texts(&view), words);
            assert_eq!(view.generation, generation);
            assert!(runtime.begin_settled_rerank().is_none());
        } else {
            assert_eq!(view.candidates[0].text, "丙");
        }
    }
}
