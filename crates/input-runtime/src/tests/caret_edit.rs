//! 光标后编辑经过 runtime 的部分：暂存交出去的文字、视图里的两个字段、候选身份、不重排首选。真实 Engine，合成词库。

use super::*;

const FIXTURE: &str = "CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wo','w','我',1000);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',1000),('huo','h','火',300);\
CREATE TABLE tbl_1_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_x VALUES('xian','x','现',1000);\
CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_c VALUES('chi','c','吃',1000),('chi','c','痴',200),('chi','c','尺',100);\
CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_c VALUES('chi''huo','ch','吃货',500),('chi''huo','ch','痴货',300),('chi''huo','ch','尺蠖',200);\
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_g VALUES('guo','g','锅',1000),('guo','g','国',500),('guo','g','过',300);\
CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_h VALUES('huo''guo','hg','火锅',5000);";

const TYPED: &str = "wohaoxianchihuoguo";

fn caret_edit_runtime(root: &std::path::Path, enabled: bool) -> Runtime {
    let mut options = real_engine_options(root);
    options.caret_edit_suffix = enabled;
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch(FIXTURE)
        .unwrap();
    let engine = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(engine, 6).unwrap();
    // 三个桌面宿主都请求暂存词段。
    runtime.set_phrase_preedit(true);
    runtime.focus(true).unwrap();
    runtime
}

fn id_of(view: &View, text: &str) -> CandidateId {
    view.candidates
        .iter()
        .find(|row| row.text == text)
        .unwrap_or_else(|| panic!("{text} is not on the page: {:?}", texts(view)))
        .id
}

fn at_caret(runtime: &mut Runtime, caret: usize) -> View {
    type_characters(runtime, TYPED);
    runtime.set_caret(Some(caret)).unwrap();
    runtime.view()
}

#[test]
fn the_fixed_prefix_and_chosen_parts_commit_together() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = caret_edit_runtime(directory.path(), true);
    let view = at_caret(&mut runtime, 9);
    assert!(view.caret_edit_suffix);
    assert_eq!(view.caret_edit_prefix, "我好现");
    assert_eq!(view.candidates[0].text, "我好现吃火锅");
    assert_eq!(view.candidates[0].id.index, 0);
    assert_eq!(view.editing_text, TYPED);
    assert_eq!(view.caret_position, 9);

    let picked = runtime
        .dispatch(Action::Select(id_of(&view, "吃货")))
        .unwrap();
    // 交出去的文字暂存在组字区里，不上屏。
    assert!(picked.handled && picked.commit.is_none(), "{picked:?}");
    assert_eq!(picked.view.phrase_prefix, "我好现吃货");
    assert_eq!(picked.view.editing_text, "guo");
    assert!(picked.view.caret_edit_suffix);
    assert_eq!(picked.view.caret_edit_prefix, "我好现吃货");
    assert_eq!(picked.view.candidates[0].text, "我好现吃货锅");
    let fact = runtime.facts().selection.expect("the pick is recorded");
    assert_eq!(fact.outcome, SelectionOutcome::Held);

    let done = runtime
        .dispatch(Action::Select(picked.view.candidates[0].id))
        .unwrap();
    assert_eq!(done.commit.as_deref(), Some("我好现吃货锅"));
    assert!(done.view.phrase_prefix.is_empty() && done.view.editing_text.is_empty());
    assert!(!done.view.caret_edit_suffix && done.view.caret_edit_prefix.is_empty());
    assert!(runtime.is_idle());

    // 首选直接上屏整句。
    at_caret(&mut runtime, 9);
    let done = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(done.commit.as_deref(), Some("我好现吃火锅"));
    assert!(runtime.is_idle());
}

#[test]
fn moving_the_caret_invalidates_the_previous_suffix_candidates() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = caret_edit_runtime(directory.path(), true);
    let before = at_caret(&mut runtime, 9);
    let stale = before.candidates[1].id;
    runtime.set_caret(Some(12)).unwrap();
    let after = runtime.view();
    assert_eq!(after.caret_edit_prefix, "我好现吃");
    assert_eq!(after.candidates[0].text, "我好现吃火锅");
    assert!(after.generation > before.generation);
    assert!(matches!(
        runtime.dispatch(Action::Select(stale)),
        Err(RuntimeError::StaleCandidate)
    ));
    // 光标命令同样推进代次。
    let moved = runtime
        .dispatch(Action::Command(Command::MoveLeft))
        .unwrap();
    assert!(matches!(
        runtime.dispatch(Action::Select(after.candidates[0].id)),
        Err(RuntimeError::StaleCandidate)
    ));
    assert!(moved.view.caret_edit_suffix);
}

#[test]
fn backspace_escape_and_enter_follow_the_held_phrase_rules() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = caret_edit_runtime(directory.path(), true);
    // 退格删到最后一个字母时撤回上一次选择：读音还回来，暂存的词段清掉。
    let view = at_caret(&mut runtime, 9);
    runtime
        .dispatch(Action::Select(id_of(&view, "吃货")))
        .unwrap();
    runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    let restored = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert!(restored.commit.is_none());
    assert!(restored.view.phrase_prefix.is_empty());
    assert_eq!(restored.view.editing_text, "wohaoxianchihuo");
    assert!(!restored.view.caret_edit_suffix);
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();

    // Esc 丢掉固定的那段、已选的词和剩下的拼写。
    let view = at_caret(&mut runtime, 9);
    runtime
        .dispatch(Action::Select(id_of(&view, "吃货")))
        .unwrap();
    let cancelled = runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(cancelled.commit.is_none());
    assert!(runtime.is_idle());

    // 回车：暂存的文字接上剩下的原文。
    let view = at_caret(&mut runtime, 9);
    runtime
        .dispatch(Action::Select(id_of(&view, "吃货")))
        .unwrap();
    let raw = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(raw.commit.as_deref(), Some("我好现吃货guo"));
    // 还没选过时回车上屏整段原文。
    at_caret(&mut runtime, 9);
    let raw = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(raw.commit.as_deref(), Some(TYPED));
}

#[test]
fn the_view_fields_are_absent_when_the_mode_is_not_active() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = caret_edit_runtime(directory.path(), true);
    at_caret(&mut runtime, 9);
    let json = serde_json::to_value(runtime.view()).unwrap();
    assert_eq!(json["caret_edit_suffix"], true);
    assert_eq!(json["caret_edit_prefix"], "我好现");
    runtime.set_caret(None).unwrap();
    let json = serde_json::to_value(runtime.view()).unwrap();
    assert!(json.get("caret_edit_suffix").is_none(), "{json}");
    assert!(json.get("caret_edit_prefix").is_none(), "{json}");

    // 选项关着：与光标前缀解码相同，视图不带这两项。
    let directory = tempfile::tempdir().unwrap();
    let mut off = caret_edit_runtime(directory.path(), false);
    let view = at_caret(&mut off, 9);
    assert!(!view.caret_edit_suffix && view.caret_edit_prefix.is_empty());
    assert_eq!(view.candidates[0].text, "我好现");
    let json = serde_json::to_value(&view).unwrap();
    assert!(json.get("caret_edit_suffix").is_none() && json.get("caret_edit_prefix").is_none());
}

/// 首选接着固定的那段，重排模型不能把别的行提到它前面。
#[test]
fn the_reranker_leaves_the_caret_edit_list_in_engine_order() {
    let typed = |caret_edit_suffix: bool| {
        let mut runtime = Runtime::new(
            WubiMixedEngine {
                scheme: 0,
                answered_by_pinyin_fallback: false,
                reading: String::new(),
                caret_edit_suffix,
            },
            5,
        )
        .unwrap();
        let model = favouring_model(&['态', '太', '快', '顿'], &['太', '快']);
        runtime.set_reranker(Some(Reranker::new(std::sync::Arc::new(model))));
        runtime.focus(true).unwrap();
        for value in *b"dyn" {
            runtime
                .dispatch(Action::Character {
                    value,
                    shift: false,
                })
                .unwrap();
        }
        texts(&runtime.view())
    };
    assert_eq!(typed(false), ["太快", "态", "顿"]);
    assert_eq!(typed(true), ["态", "太快", "顿"]);
}
