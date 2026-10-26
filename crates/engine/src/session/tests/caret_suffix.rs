//! 光标后编辑（`SessionOptions::caret_edit_suffix`）：光标前固定、首选整句、候选是光标后那段的，选中只消耗自己的音节。词库全部是合成的。

use super::*;
use crate::types::{fuzzy_rule, FuzzyPinyinOptions};

/// `wohaoxian|chihuoguo`：光标前只能转成「我好现」，光标后那段的整句是「吃火锅」，开头的词有吃货、痴货、尺蠖。
const SUFFIX_FIXTURE: &str = "CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
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
/// `wohaoxian` 之后。
const AFTER_XIAN: usize = 9;

fn suffix_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| options.caret_edit_suffix = true)
}

fn at_caret(session: &mut Session, typed: &str, caret: usize) {
    type_text(session, typed);
    session.set_caret(Some(caret));
}

/// 宿主看到的候选：首选前面接上固定的那段（runtime 在视图里做的事）。
fn shown(session: &Session) -> Vec<String> {
    let snapshot = session.snapshot();
    let mut rows = words(session);
    if let Some(first) = rows.first_mut() {
        first.insert_str(0, &snapshot.caret_edit_prefix);
    }
    rows
}

/// 光标后那段当作整段组字时的候选（选项关着、没有光标）。
fn typed_alone(fixture: &Fixture, text: &str) -> Vec<String> {
    let mut session = fixture.session();
    type_text(&mut session, text);
    shown(&session)
}

#[test]
fn the_prefix_is_fixed_and_the_first_candidate_is_the_whole_sentence() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    let snapshot = session.snapshot();
    assert!(snapshot.caret_edit_suffix);
    assert_eq!(snapshot.caret_edit_prefix, "我好现");
    // 行内仍是原文和光标，固定的那段没有上屏。
    assert_eq!(snapshot.editing_text, TYPED);
    assert_eq!(snapshot.caret_position, AFTER_XIAN);
    let rows = shown(&session);
    assert_eq!(rows[0], "我好现吃火锅", "{rows:?}");
    // 其余候选就是光标后那段当作整段组字时的候选，首选之外原样排列。
    let alone = typed_alone(&fixture, "chihuoguo");
    assert_eq!(alone[0], "吃火锅", "{alone:?}");
    assert_eq!(rows[1..], alone[1..]);
    for word in ["吃货", "痴货", "尺蠖", "吃"] {
        assert!(rows.contains(&word.to_owned()), "{word} missing: {rows:?}");
    }
}

#[test]
fn choosing_a_suffix_word_consumes_its_syllables_and_keeps_editing_the_rest() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);

    let picked = select_word(&mut session, "吃货");
    // 引擎把固定的那段和选中的词一起交出去；三个桌面宿主让 runtime 暂存它，组字完成时一起上屏。
    assert!(picked.handled);
    assert_eq!(picked.commit.as_deref(), Some("我好现吃货"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "guo");
    assert!(snapshot.caret_edit_suffix, "the mode continues on the rest");
    assert_eq!(snapshot.caret_edit_prefix, "我好现吃货");
    let rows = shown(&session);
    assert_eq!(rows[0], "我好现吃货锅", "{rows:?}");
    assert_eq!(rows[1..], typed_alone(&fixture, "guo")[1..]);

    let done = session.select(index_of(&session, "国"));
    assert_eq!(done.commit.as_deref(), Some("国"));
    let snapshot = session.snapshot();
    assert!(snapshot.editing_text.is_empty());
    assert!(!snapshot.caret_edit_suffix);
    assert!(snapshot.caret_edit_prefix.is_empty());

    // 首选：固定的部分已经交出去，只剩「锅」。
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    select_word(&mut session, "吃货");
    let done = session.select(0);
    assert_eq!(done.commit.as_deref(), Some("锅"));
    assert!(session.snapshot().editing_text.is_empty());
}

#[test]
fn the_first_candidate_commits_the_whole_sentence() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    for commit in [
        |session: &mut Session| session.select(0),
        |session: &mut Session| session.command(Command::CommitCandidate),
        |session: &mut Session| session.finish(0),
    ] {
        let mut session = suffix_session(&fixture);
        at_caret(&mut session, TYPED, AFTER_XIAN);
        let result = commit(&mut session);
        assert_eq!(result.commit.as_deref(), Some("我好现吃火锅"));
        let snapshot = session.snapshot();
        assert!(snapshot.editing_text.is_empty());
        assert!(!snapshot.caret_edit_suffix);
    }
}

#[test]
fn a_caret_at_either_end_or_the_option_off_keeps_caret_prefix_decoding() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    for caret in [0, 1, TYPED.len()] {
        let mut on = suffix_session(&fixture);
        let mut off = fixture.session();
        at_caret(&mut on, TYPED, caret);
        at_caret(&mut off, TYPED, caret);
        assert_eq!(on.snapshot(), off.snapshot(), "caret {caret}");
        assert!(!on.snapshot().caret_edit_suffix);
    }
    // 关着时就是光标前缀解码：候选是光标前那段的。
    let mut off = fixture.session();
    at_caret(&mut off, TYPED, AFTER_XIAN);
    let snapshot = off.snapshot();
    assert!(!snapshot.caret_edit_suffix && snapshot.caret_edit_prefix.is_empty());
    assert_eq!(words(&off), typed_alone(&fixture, "wohaoxian"));
    let picked = off.select(0);
    assert_eq!(picked.commit.as_deref(), Some("我好现"));
    assert_eq!(off.snapshot().editing_text, "chihuoguo");
}

#[test]
fn moving_the_caret_again_fixes_a_different_prefix() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    assert_eq!(session.snapshot().caret_edit_prefix, "我好现");
    // `wohaoxianchi|huoguo`
    session.set_caret(Some(12));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.caret_edit_prefix, "我好现吃");
    assert_eq!(shown(&session)[0], "我好现吃火锅");
    assert_eq!(shown(&session)[1..], typed_alone(&fixture, "huoguo")[1..]);
    // 光标在音节中间时落到它前面的音节边界，与光标前缀解码相同。
    session.set_caret(Some(10));
    assert_eq!(session.snapshot().caret_edit_prefix, "我好现");
    session.command(Command::MoveEnd);
    assert!(!session.snapshot().caret_edit_suffix);
    assert_eq!(shown(&session), typed_alone(&fixture, TYPED));
}

#[test]
fn shuangpin_fixes_the_prefix_by_its_two_key_units() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let xiaohe = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
    };
    let mut session = fixture.session_with(|options| {
        xiaohe(options);
        options.caret_edit_suffix = true;
    });
    // wo hc xm | ii ho go
    at_caret(&mut session, "wohcxmiihogo", 6);
    let snapshot = session.snapshot();
    assert!(snapshot.caret_edit_suffix);
    assert_eq!(snapshot.caret_edit_prefix, "我好现");
    assert_eq!(shown(&session)[0], "我好现吃火锅");
    // 落在双拼的两键中间时退到前一个音节的边界。
    session.set_caret(Some(7));
    assert_eq!(session.snapshot().caret_edit_prefix, "我好现");

    let picked = select_word(&mut session, "吃货");
    assert_eq!(picked.commit.as_deref(), Some("我好现吃货"));
    assert_eq!(session.snapshot().editing_text, "go");
    assert_eq!(shown(&session)[0], "我好现吃货锅");
    assert_eq!(session.select(0).commit.as_deref(), Some("锅"));
}

#[test]
fn backspace_escape_and_enter_act_on_the_composition_as_usual() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    // 退格删光标前的字母：固定的那段跟着重新转换。
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "wohaoxiachihuoguo");
    assert_eq!(snapshot.caret_position, AFTER_XIAN - 1);

    // Esc 丢掉整段，什么都不上屏。
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    let cancelled = session.command(Command::Cancel);
    assert!(cancelled.handled && cancelled.commit.is_none());
    assert!(!session.snapshot().caret_edit_suffix);
    assert!(session.snapshot().editing_text.is_empty());

    // 回车上屏原文。
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some(TYPED)
    );

    // 选过一个词之后，退格删的是剩下的拼写；删空时模式随组字一起结束。
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    select_word(&mut session, "吃货");
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().editing_text, "gu");
    assert!(session.snapshot().caret_edit_suffix);
    session.command(Command::Backspace);
    session.command(Command::Backspace);
    let snapshot = session.snapshot();
    assert!(snapshot.editing_text.is_empty());
    assert!(!snapshot.caret_edit_suffix && snapshot.caret_edit_prefix.is_empty());
    // 下一段组字从头开始，不带着上一段暂存的文字。
    type_text(&mut session, "guo");
    assert!(!session.snapshot().caret_edit_suffix);
    assert_eq!(shown(&session), typed_alone(&fixture, "guo"));
}

#[test]
fn edge_selection_keeps_the_fixed_prefix() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    let index = index_of(&session, "吃货");
    let result = session.select_edge(index, CandidateEdge::LastHan);
    assert_eq!(result.commit.as_deref(), Some("我好现货"));
    assert!(session.snapshot().editing_text.is_empty());
}

#[test]
fn mixed_rows_and_online_queries_stay_out_of_the_mode() {
    let fixture = Fixture::new(SUFFIX_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('chihuoguos','chihuoguos',100);");
    let options = |options: &mut SessionOptions| options.english.mixed_candidates = true;
    // 光标后那段单独打时会混进英文。
    let mut alone = fixture.session_with(options);
    type_text(&mut alone, "chihuoguo");
    assert!(words(&alone).contains(&"chihuoguos".to_owned()));

    let mut session = fixture.session_with(|value| {
        options(value);
        value.caret_edit_suffix = true;
    });
    session.set_cloud_candidates_enabled(true);
    type_text(&mut session, TYPED);
    assert!(session.online_query().is_some());
    let query = session.online_query().unwrap();
    session.set_caret(Some(AFTER_XIAN));
    let rows = shown(&session);
    assert!(!rows.contains(&"chihuoguos".to_owned()), "{rows:?}");
    assert!(
        session
            .snapshot()
            .candidates
            .iter()
            .all(|row| !row.source.is_online() && row.source != CandidateSource::CloudPending),
        "{rows:?}"
    );
    assert!(session.online_query().is_none());
    // 进入模式之前发出的查询，回包也不接。
    assert!(!session.apply_online_candidate(&query, "云端整句", CandidateSource::CloudSuggestion));
    assert_eq!(shown(&session), rows);
}

#[test]
fn fuzzy_pinyin_applies_to_the_suffix() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.caret_edit_suffix = true;
        options.fuzzy_pinyin = FuzzyPinyinOptions {
            rules: fuzzy_rule::C_CH,
        };
    });
    at_caret(&mut session, "wohaoxiancihuoguo", AFTER_XIAN);
    let rows = shown(&session);
    let mut alone = fixture.session_with(|options| {
        options.fuzzy_pinyin = FuzzyPinyinOptions {
            rules: fuzzy_rule::C_CH,
        };
    });
    type_text(&mut alone, "cihuoguo");
    let alone = words(&alone);
    assert!(alone.contains(&"吃货".to_owned()), "{alone:?}");
    // 首选是固定的那段接上光标后那段的首选，后者不一定覆盖光标后的全部音节（这里模糊音只查到了词，没有整句）。
    assert_eq!(rows[0], format!("我好现{}", alone[0]), "{rows:?}");
    assert_eq!(rows[1..], alone[1..]);
    // 选它就和选中光标后那段的首选一样：只消耗它自己的音节，剩下的接着编辑。
    let first = session.select(0);
    assert_eq!(first.commit.as_deref(), Some(&*rows[0]));
    if !session.snapshot().editing_text.is_empty() {
        assert!(session.snapshot().caret_edit_suffix);
        assert!(shown(&session)[0].starts_with(&rows[0]));
    }
}

#[test]
fn a_one_letter_suffix_expands_its_own_list() {
    let rows: Vec<String> = (0..30)
        .map(|i| {
            format!(
                "('ni','n','{}',{})",
                char::from_u32(0x4e00 + i).unwrap(),
                1000 - i
            )
        })
        .collect();
    let fixture = Fixture::new(&format!(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES{};\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',100);",
        rows.join(",")
    ));
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, "haon", 3);
    assert_eq!(session.snapshot().caret_edit_prefix, "好");
    assert_eq!(shown(&session).len(), 24);
    assert!(session.expand_initial_candidates());
    let widened = shown(&session);
    assert_eq!(widened.len(), 30, "{widened:?}");
    assert_eq!(widened[0], "好一");
    let picked = session.select(29);
    assert_eq!(
        picked.commit.as_deref(),
        Some(&*format!("好{}", widened[29]))
    );
}

#[test]
fn choosing_parts_learns_like_partial_selections() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let mut session = suffix_session(&fixture);
    at_caret(&mut session, TYPED, AFTER_XIAN);
    select_word(&mut session, "吃货");
    assert_eq!(session.select(0).commit.as_deref(), Some("锅"));
    // 各段按造词规则存成个人词条，与逐段选词相同。
    let mut again = fixture.session();
    type_text(&mut again, TYPED);
    assert!(
        words(&again).contains(&"我好现吃货锅".to_owned()),
        "{:?}",
        words(&again)
    );

    // 学习关着时什么都不存。
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.caret_edit_suffix = true;
        options.learning = false;
    });
    at_caret(&mut session, TYPED, AFTER_XIAN);
    select_word(&mut session, "吃货");
    session.select(0);
    let mut again = fixture.session();
    type_text(&mut again, TYPED);
    assert!(!words(&again).contains(&"我好现吃货锅".to_owned()));
}

#[test]
fn choosing_a_later_suffix_word_adjusts_its_frequency_under_the_suffix() {
    let fixture = Fixture::new(SUFFIX_FIXTURE);
    let promote = FrequencyAdjustmentOptions {
        mode: FrequencyAdjustmentMode::Promote,
        trigger_count: 1,
        linear_step: 1,
    };
    let mut session = fixture.session_with(|options| {
        options.caret_edit_suffix = true;
        options.frequency = promote;
    });
    at_caret(&mut session, TYPED, AFTER_XIAN);
    select_word(&mut session, "痴货");
    session.command(Command::Cancel);
    // 调频记在光标后那段的读音下：单独打 chihuo 时痴货排到吃货前面。
    let mut again = fixture.session_with(|options| options.frequency = promote);
    type_text(&mut again, "chihuo");
    let rows = words(&again);
    let chi = rows.iter().position(|word| word == "痴货").unwrap();
    let eat = rows.iter().position(|word| word == "吃货").unwrap();
    assert!(chi < eat, "{rows:?}");
}
