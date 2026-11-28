//! 全拼的部件辅助段（`session/component_filter.rs`，#11804）：反引号打开、退格删回、筛选、上屏、学词与云候选都不带部件段；选项关着时反引号和以前一样。

use super::*;

/// 合成词库：`cheng` 下有同音的 蛏（虫 + 圣）和 城（土 + 成），另有 `shi` 与 `cheng'shi`。
const COMPONENT_FIXTURE: &str =
    "CREATE TABLE tbl_1_c(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_c VALUES('cheng', 'c', '成', 500);\
INSERT INTO tbl_1_c VALUES('cheng', 'c', '城', 400);\
INSERT INTO tbl_1_c VALUES('cheng', 'c', '程', 300);\
INSERT INTO tbl_1_c VALUES('cheng', 'c', '蛏', 10);\
CREATE TABLE tbl_1_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shi', 's', '是', 500);\
INSERT INTO tbl_1_s VALUES('shi', 's', '市', 400);\
CREATE TABLE tbl_2_c(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_2_c VALUES('cheng''shi', 'cs', '城市', 300);";

fn component_session(fixture: &Fixture, enabled: bool) -> Session {
    fixture.session_with(|options| {
        options.helpcode = false;
        options.component_filter_backtick = enabled;
    })
}

fn preedit(session: &Session) -> String {
    session.snapshot().preedit
}

#[test]
fn the_option_is_off_by_default() {
    let fixture = Fixture::new(COMPONENT_FIXTURE);
    assert!(!SessionOptions::new(fixture.paths()).component_filter_backtick);
    assert!(!fixture.options().component_filter_backtick);
}

/// 选项关着时反引号和没有这个功能时一样：不发布成拼写符号，字符入口不收，标点入口按首选结束组字再出反引号对应的标点。打开选项但反引号没被接收（拼音不完整）时走同一条旧路径，结果逐字段相同；组字本身的候选和预编辑也不受选项影响。
#[test]
fn backtick_is_punctuation_while_the_option_is_off() {
    let fixture = Fixture::new(COMPONENT_FIXTURE);
    let mut off = component_session(&fixture, false);
    type_text(&mut off, "cheng");
    assert!(!off.snapshot().spelling_symbols.contains('`'));
    assert!(!off.character(b'`', false).handled);
    assert_eq!(preedit(&off), "cheng");
    let mut on = component_session(&fixture, true);
    type_text(&mut on, "cheng");
    assert_eq!(words(&on), words(&off));
    assert_eq!(preedit(&on), preedit(&off));
    let committed = off.punctuation(b'`');
    assert!(committed.handled);
    assert!(committed
        .commit
        .as_deref()
        .is_some_and(|text| text.starts_with('成')));
    assert!(preedit(&off).is_empty());

    for enabled in [false, true] {
        let mut session = component_session(&fixture, enabled);
        type_text(&mut session, "c");
        let mut reference = component_session(&fixture, false);
        type_text(&mut reference, "c");
        assert_eq!(
            session.punctuation(b'`'),
            reference.punctuation(b'`'),
            "{enabled}"
        );
        assert_eq!(session.snapshot().preedit, reference.snapshot().preedit);
    }
}

#[test]
fn backtick_opens_the_segment_and_backspace_removes_it() {
    let fixture = Fixture::new(COMPONENT_FIXTURE);
    let mut session = component_session(&fixture, true);
    type_text(&mut session, "cheng");
    let unfiltered = words(&session);
    assert!(session.snapshot().spelling_symbols.contains('`'));

    assert!(session.character(b'`', false).handled);
    assert_eq!(preedit(&session), "cheng`");
    assert_eq!(session.snapshot().editing_text, "cheng`");
    // 段里还没有字母时不筛。
    assert_eq!(words(&session), unfiltered);
    // 一段组字只有一个反引号。
    assert!(!session.snapshot().spelling_symbols.contains('`'));

    type_text(&mut session, "chong");
    assert_eq!(preedit(&session), "cheng`chong");
    assert_eq!(words(&session), ["蛏"]);
    assert!(session.snapshot().raw_segmentation.ends_with("`chong"));

    for _ in 0..5 {
        assert!(session.command(Command::Backspace).handled);
    }
    assert_eq!(preedit(&session), "cheng`");
    assert_eq!(words(&session), unfiltered);
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(preedit(&session), "cheng");
    assert_eq!(words(&session), unfiltered);
    assert!(session.snapshot().spelling_symbols.contains('`'));
}

/// 宿主把反引号当标点送来时同样进组字（runtime 按 `spelling_symbols` 转给字符入口，直接调用方走这条）。
#[test]
fn backtick_through_the_punctuation_entry_opens_the_segment_too() {
    let fixture = Fixture::new(COMPONENT_FIXTURE);
    let mut session = component_session(&fixture, true);
    type_text(&mut session, "cheng");
    let result = session.punctuation(b'`');
    assert!(result.handled);
    assert_eq!(result.commit, None);
    type_text(&mut session, "tu");
    assert_eq!(words(&session), ["城"]);
}

/// 拼音不完整、光标不在末尾、已经有大写辅助码时反引号不打开辅助段。
#[test]
fn backtick_needs_complete_pinyin_with_the_caret_at_the_end() {
    let fixture = Fixture::new(COMPONENT_FIXTURE);
    let mut session = component_session(&fixture, true);
    type_text(&mut session, "ch");
    assert!(!session.snapshot().spelling_symbols.contains('`'));
    assert!(!session.character(b'`', false).handled);
    assert_eq!(preedit(&session), "ch");
    session.command(Command::Cancel);

    type_text(&mut session, "cheng");
    session.set_caret(Some(2));
    assert!(!session.character(b'`', false).handled);
    session.set_caret(None);
    assert!(session.character(b'`', false).handled);
    session.command(Command::Cancel);

    // 五笔、双拼不收。
    let mut shuangpin = fixture.session_with(|options| {
        options.helpcode = false;
        options.component_filter_backtick = true;
        options.scheme = SchemeType::Shuangpin;
    });
    type_text(&mut shuangpin, "ig");
    assert!(!shuangpin.character(b'`', false).handled);
}

/// 选中筛出来的字只上屏那个字；回车原样上屏和筛空后的结束组字只上屏反引号前的拼音。
#[test]
fn commits_never_include_the_component_segment() {
    let fixture = Fixture::new(COMPONENT_FIXTURE);
    let mut session = component_session(&fixture, true);
    type_text(&mut session, "cheng`chong");
    assert_eq!(session.select(0).commit.as_deref(), Some("蛏"));
    assert!(preedit(&session).is_empty());

    type_text(&mut session, "cheng`chong");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("cheng")
    );
    assert!(preedit(&session).is_empty());

    type_text(&mut session, "cheng`xyz");
    assert!(words(&session).is_empty());
    assert_eq!(session.finish(0).commit.as_deref(), Some("cheng"));
    assert!(preedit(&session).is_empty());
}

/// 多字的组字按末字筛；选走一个字之后剩下的拼音不再带部件段，接着组成的词按拼音学进词库，学习日志里没有部件串。
#[test]
fn learning_records_only_the_pinyin() {
    // 词库里没有 城市，组完的词才会作为新词学进去。
    let fixture = Fixture::new(&COMPONENT_FIXTURE.replace(
        "INSERT INTO tbl_2_c VALUES('cheng''shi', 'cs', '城市', 300);",
        "",
    ));
    let mut session = fixture.session_with(|options| {
        options.helpcode = false;
        options.component_filter_backtick = true;
        options.learning = true;
    });
    type_text(&mut session, "chengshi`tu");
    assert_eq!(words(&session), ["城"]);
    // 选走一部分时先上屏这一段，剩下的拼音接着组字。
    assert_eq!(
        select_word(&mut session, "城").commit.as_deref(),
        Some("城")
    );
    assert_eq!(preedit(&session), "shi");
    assert_eq!(
        select_word(&mut session, "市").commit.as_deref(),
        Some("市")
    );
    let leaked =
        "SELECT count(*) FROM user_dictionary_operations WHERE key LIKE '%`%' OR key LIKE '%tu%'";
    assert_eq!(count(&fixture.journal(), leaked), 0);
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='pinyin' AND key='cheng''shi' AND value='城市'"
        ),
        1
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM tbl_2_c WHERE key LIKE '%`%' OR key LIKE '%tu%'"
        ),
        0
    );
}

/// 云候选只问反引号前的拼音：段打开但还没有部件时照常问 `cheng`，有部件在筛时不再发起查询。
#[test]
fn cloud_requests_never_include_the_component_segment() {
    let fixture = Fixture::new(COMPONENT_FIXTURE);
    let mut session = component_session(&fixture, true);
    type_text(&mut session, "cheng");
    let plain = session.online_query().expect("全拼查询");
    assert!(session.character(b'`', false).handled);
    let open = session.online_query().expect("全拼查询");
    assert_eq!(open.query_text, plain.query_text);
    assert_eq!(open.cache_key, plain.cache_key);
    assert!(!open.query_text.contains('`'));
    type_text(&mut session, "chong");
    assert!(session.online_query().is_none_or(
        |query| !query.query_text.contains("chong") && !query.cache_key.contains("chong")
    ));
}
