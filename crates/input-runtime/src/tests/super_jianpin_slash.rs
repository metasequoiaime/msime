//! 双拼组字末尾按 `/` 转超级简拼（#10860）经过 runtime 的部分：`/` 走标点入口也是转换，不先上屏首选；第二个 `/` 在引擎里一次上屏首选加 `/`，runtime 不再拿它当标点处理一遍。真实 Engine，合成词库。

use super::*;

const FIXTURE: &str = "CREATE TABLE tbl_3_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_3_n VALUES('ni''shi''shui','nss','你是谁',1000),('na''se''su','nss','那色素',900);\
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('nu','n','怒',500);\
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shi','s','是',1000);";

fn xiaohe_runtime(directory: &std::path::Path) -> Runtime {
    let mut options = real_engine_options(directory);
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
