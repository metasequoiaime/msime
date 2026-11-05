//! 双拼组字末尾按 `/` 转超级简拼（#10860）经过 runtime 的部分：`/` 走标点入口也是转换，不先上屏首选；第二个 `/` 撤回转换，从转换前宿主高亮的那一行上屏再出 `/`，退格撤回时高亮也回到那一行，与以前单按 `/` 相同。真实 Engine，合成词库。

use super::*;

const FIXTURE: &str = "CREATE TABLE tbl_3_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_3_n VALUES('ni''shi''shui','nss','你是谁',1000),('na''se''su','nss','那色素',900);\
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('nu','n','怒',500);\
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('nu''shi','ns','怒视',900),('nu''shi','ns','努实',800);\
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shi','s','是',1000);";

fn xiaohe_runtime(directory: &std::path::Path) -> Runtime {
    xiaohe_runtime_with(directory, true)
}

/// `slash` 是 `local_modes.super_jianpin_slash`；关掉时 `/` 照以前那样是标点键，拿来对照撤回后的结果。
fn xiaohe_runtime_with(directory: &std::path::Path, slash: bool) -> Runtime {
    let mut options = real_engine_options(directory);
    options.local_super_jianpin_slash = slash;
    options.scheme = 1;
    options.shuangpin_profile = 0;
    rusqlite::Connection::open(
        std::path::Path::new(&options.dictionaries).join(msime_engine::assets::MAIN_DICTIONARY),
    )
    .unwrap()
    .execute_batch(FIXTURE)
    .unwrap();
    let session = msime_engine::host::Session::new(&options).unwrap();
    let mut runtime = Runtime::new(session, 5).unwrap();
    runtime.focus(true).unwrap();
    runtime
}

#[test]
fn a_slash_after_shuangpin_letters_converts_and_a_second_one_escapes() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = xiaohe_runtime(directory.path());
    for value in *b"nuu" {
        character(&mut runtime, value);
    }
    let first = runtime.view().candidates[0].text.clone();
    assert!(runtime.view().spelling_symbols.contains('/'));

    let converted = runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    assert!(converted.handled && converted.commit.is_none());
    assert_eq!(converted.view.local_mode, "super_jianpin");
    assert_eq!(converted.view.editing_text, "Jnuu");
    assert_eq!(converted.view.candidates[0].text, "你是谁");

    let escaped = runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    assert!(escaped.handled);
    assert_eq!(escaped.commit, Some(format!("{first}/")));
    assert_eq!(escaped.view.local_mode, "none");
    assert!(escaped.view.editing_text.is_empty());
}

#[test]
fn the_ascii_punctuation_route_leaves_the_slash_literal() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = xiaohe_runtime(directory.path());
    for value in *b"nuu" {
        character(&mut runtime, value);
    }
    let finished = runtime.dispatch(Action::PunctuationAscii(b'/')).unwrap();
    let commit = finished.commit.unwrap();
    assert!(commit.ends_with('/'), "{commit:?}");
    assert_eq!(finished.view.local_mode, "none");
}

fn highlighted_seat(view: &View) -> Option<usize> {
    view.candidates
        .iter()
        .position(|candidate| candidate.highlighted)
}

/// 打 `nuu` 并把高亮移到第二行，返回首行和第二行的文字。
fn type_and_highlight_second(runtime: &mut Runtime) -> (String, String) {
    for value in *b"nuu" {
        character(runtime, value);
    }
    assert!(
        runtime.view().candidates.len() > 1,
        "{:?}",
        runtime.view().candidates
    );
    let moved = runtime.dispatch(Action::NextCandidate).unwrap();
    assert_eq!(highlighted_seat(&moved.view), Some(1));
    (
        moved.view.candidates[0].text.clone(),
        moved.view.candidates[1].text.clone(),
    )
}

/// 对照：关掉后置简拼时，高亮第二行再按 `/` 上屏的是什么（从第二行结束组字，剩下的读音用各自首选，再接 `/`）。
fn slash_on_the_second_row_without_conversion() -> String {
    let directory = tempfile::tempdir().unwrap();
    let mut before = xiaohe_runtime_with(directory.path(), false);
    let (first, second) = type_and_highlight_second(&mut before);
    let commit = before
        .dispatch(Action::Punctuation(b'/'))
        .unwrap()
        .commit
        .unwrap();
    assert!(
        commit.starts_with(&second) && commit.ends_with('/'),
        "{commit:?}"
    );
    assert_ne!(commit, format!("{first}/"));
    commit
}

#[test]
fn a_second_slash_commits_the_row_highlighted_before_the_conversion() {
    let expected = slash_on_the_second_row_without_conversion();
    // 标点、字符两条入口的 `//` 都得到同样的结果，而不是首选加 `/`。
    for punctuation_route in [true, false] {
        let slash = || {
            if punctuation_route {
                Action::Punctuation(b'/')
            } else {
                Action::Character {
                    value: b'/',
                    shift: false,
                }
            }
        };
        let directory = tempfile::tempdir().unwrap();
        let mut runtime = xiaohe_runtime(directory.path());
        type_and_highlight_second(&mut runtime);
        let converted = runtime.dispatch(slash()).unwrap();
        assert_eq!(converted.view.local_mode, "super_jianpin");
        let escaped = runtime.dispatch(slash()).unwrap();
        assert!(escaped.handled);
        assert_eq!(escaped.commit.as_deref(), Some(expected.as_str()));
        assert_eq!(escaped.view.local_mode, "none");
        assert!(escaped.view.editing_text.is_empty());
    }
}

#[test]
fn backspace_after_the_conversion_puts_the_highlight_back() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = xiaohe_runtime(directory.path());
    let (_, second) = type_and_highlight_second(&mut runtime);
    runtime.dispatch(Action::Punctuation(b'/')).unwrap();

    let reverted = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(reverted.view.local_mode, "none");
    assert_eq!(reverted.view.editing_text, "nuu");
    assert_eq!(highlighted_seat(&reverted.view), Some(1));
    assert_eq!(reverted.view.candidates[1].text, second);

    // 再转一次、再撤回，用的是这时的高亮。
    runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    let escaped = runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    assert_eq!(
        escaped.commit,
        Some(slash_on_the_second_row_without_conversion())
    );
}

#[test]
fn an_edited_jianpin_code_forgets_the_highlight() {
    let directory = tempfile::tempdir().unwrap();
    let mut runtime = xiaohe_runtime(directory.path());
    type_and_highlight_second(&mut runtime);
    runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    // 简拼码改过以后退格只是删字，不撤回；删回原文时又能撤回，高亮仍是转换前那一行。
    character(&mut runtime, b's');
    let edited = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(edited.view.local_mode, "super_jianpin");
    assert_eq!(edited.view.editing_text, "Jnuu");
    let reverted = runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(reverted.view.editing_text, "nuu");
    assert_eq!(highlighted_seat(&reverted.view), Some(1));

    // Esc 离开超级简拼之后，记下的高亮不再用于下一次组字。
    runtime.dispatch(Action::Punctuation(b'/')).unwrap();
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    assert!(runtime.jianpin_slash.is_none());
}
