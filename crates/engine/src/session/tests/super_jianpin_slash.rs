//! 双拼组字末尾按 `/` 转超级简拼（#10860）：触发条件、让位、撤回（退格和 `//`）、回车与兜底行不带 `J`，以及全拼和出厂设置下 `/` 原来的行为不变。词库全部是合成的。

use std::collections::BTreeMap;

use super::*;
use crate::shuangpin::profile::profile;

/// `(全拼读音, 词, 权重)` 写成 `msime-pinyin.db` 的表：表名按音节数和首字母，`jp` 是各音节的首字母。
fn dictionary(rows: &[(&str, &str, i64)]) -> String {
    let mut tables: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, value, weight) in rows {
        let syllables: Vec<&str> = key.split('\'').collect();
        let table = format!("tbl_{}_{}", syllables.len(), &key[..1]);
        let jp: String = syllables.iter().map(|syllable| &syllable[..1]).collect();
        tables.entry(table).or_default().push(format!(
            "('{}','{jp}','{value}',{weight})",
            key.replace('\'', "''")
        ));
    }
    tables
        .iter()
        .map(|(table, values)| {
            format!(
                "CREATE TABLE {table}(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO {table} VALUES{};",
                values.join(",")
            )
        })
        .collect()
}

/// 「你是谁」和「那色素」的 `jp` 都是 `nss`：只有按完整声母过滤，小鹤的 `nuu`（n'sh'sh）和 `nss`（n's's）才分得开。
const ROWS: &[(&str, &str, i64)] = &[
    ("ni'shi'shui", "你是谁", 1000),
    ("na'se'su", "那色素", 900),
    ("ni'hao", "你好", 1000),
    ("ni", "你", 1000),
    ("nu", "怒", 500),
    ("na", "那", 800),
    ("shi", "是", 1000),
    ("shui", "谁", 500),
    ("se", "色", 300),
    ("su", "素", 300),
    ("hao", "好", 1000),
    ("de", "的", 1000),
    ("xing", "行", 800),
    ("de'xing", "德行", 500),
    // `qqq` 没有匹配：表在，只是没有 jp 为 `qqq` 的行。
    ("qian'qiu'ye", "千秋业", 10),
];

fn fixture() -> Fixture {
    Fixture::new(&dictionary(ROWS))
}

fn shuangpin_with(
    fixture: &Fixture,
    kind: ShuangpinProfileKind,
    configure: impl FnOnce(&mut SessionOptions),
) -> Session {
    fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = kind;
        configure(options);
    })
}

fn xiaohe_with(fixture: &Fixture, configure: impl FnOnce(&mut SessionOptions)) -> Session {
    shuangpin_with(fixture, ShuangpinProfileKind::Xiaohe, configure)
}

fn xiaohe(fixture: &Fixture) -> Session {
    xiaohe_with(fixture, |_| {})
}

fn publishes_slash(session: &Session) -> bool {
    session.snapshot().spelling_symbols.contains('/')
}

/// 组字原文按下 `/` 之后进了超级简拼。
fn assert_converted(session: &Session, editing: &str) {
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::SuperJianpin);
    assert_eq!(snapshot.editing_text, editing);
}

/// `/` 没有被抢：不发布、字符入口不处理、组字原样。
fn assert_slash_left_alone(session: &mut Session, editing: &str) {
    assert!(!publishes_slash(session), "{editing}");
    let result = session.character(b'/', false);
    assert!(!result.handled && result.commit.is_none(), "{editing}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None, "{editing}");
    assert_eq!(snapshot.editing_text, editing);
}

#[test]
fn slash_after_three_letters_turns_the_composition_into_super_jianpin() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nuu");
    assert!(publishes_slash(&session));
    let result = session.character(b'/', false);
    assert!(result.handled && result.commit.is_none());
    assert_converted(&session, "Jnuu");
    let listed = words(&session);
    assert!(listed.contains(&"你是谁".to_owned()), "{listed:?}");
    assert!(!listed.contains(&"那色素".to_owned()), "{listed:?}");
    // 选中照常上屏。
    let at = index_of(&session, "你是谁");
    assert_eq!(session.select(at).commit.as_deref(), Some("你是谁"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
}

#[test]
fn the_punctuation_route_converts_the_same_way() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nuu");
    let result = session.punctuation(b'/');
    assert!(result.handled && result.commit.is_none());
    assert_converted(&session, "Jnuu");
    assert!(words(&session).contains(&"你是谁".to_owned()));
}

#[test]
fn shuangpin_keys_are_read_back_as_full_initials() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nss");
    assert!(session.character(b'/', false).handled);
    assert_converted(&session, "Jnss");
    let listed = words(&session);
    assert!(listed.contains(&"那色素".to_owned()), "{listed:?}");
    assert!(!listed.contains(&"你是谁".to_owned()), "{listed:?}");
}

/// 每一种内置双拼都按自己的声母键位转：打 n、sh、sh 三个声母键，查出来的是 n'sh'sh 的「你是谁」。
#[test]
fn every_shuangpin_profile_converts_by_its_own_initials() {
    let fixture = fixture();
    for kind in [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
        ShuangpinProfileKind::SmartAbc,
    ] {
        let keys = profile(kind).expect("built-in profile").initials;
        let key = |initial: &str| {
            keys.iter()
                .find(|(full, _)| *full == initial)
                .map_or_else(|| initial.to_owned(), |(_, key)| (*key).to_owned())
        };
        let typed = format!("n{}{}", key("sh"), key("sh"));
        let mut session = shuangpin_with(&fixture, kind, |_| {});
        type_text(&mut session, &typed);
        assert!(publishes_slash(&session), "{kind:?} {typed}");
        assert!(session.character(b'/', false).handled, "{kind:?}");
        assert_converted(&session, &format!("J{typed}"));
        assert!(
            words(&session).contains(&"你是谁".to_owned()),
            "{kind:?} {typed}: {:?}",
            words(&session)
        );
    }
}

#[test]
fn two_letters_keep_the_slash_as_punctuation() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nu");
    assert_slash_left_alone(&mut session, "nu");
    // runtime 先上屏首选再要标点，和全拼的旧用例一样。
    assert!(session.finish(0).commit.is_some());
    assert!(!session.punctuation(b'/').handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
}

#[test]
fn either_switch_off_leaves_the_slash_alone() {
    let fixture = fixture();
    for configure in [
        (|options: &mut SessionOptions| options.local_modes.super_jianpin_slash = false)
            as fn(&mut SessionOptions),
        |options: &mut SessionOptions| options.local_modes.super_jianpin = false,
    ] {
        let mut session = xiaohe_with(&fixture, configure);
        type_text(&mut session, "nuu");
        assert_slash_left_alone(&mut session, "nuu");
    }
    // 「Shift+字母」总开关不管它。
    let mut session = xiaohe_with(&fixture, |options| {
        options.local_modes.shift_letter_entry = false;
    });
    type_text(&mut session, "nuu");
    assert!(session.character(b'/', false).handled);
    assert_converted(&session, "Jnuu");
}

#[test]
fn a_slash_owned_by_another_feature_keeps_that_meaning() {
    let fixture = fixture();

    // 辅助码前导键是 `/`：前导键先接走。
    let mut session = xiaohe_with(&fixture, |options| {
        options.helpcode_trigger_key = Some(b'/');
    });
    type_text(&mut session, "nuu");
    assert!(session.character(b'/', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.editing_text, "nuu");

    // 万象式的 `/` 结束键：`/` 写进原文当标记。
    let mut session = xiaohe_with(&fixture, |options| {
        options.shuangpin_helpcode_modes.direct_helpcode = true;
        options.shuangpin_helpcode_modes.direct_helpcode_slash = true;
    });
    type_text(&mut session, "nuu");
    assert!(session.character(b'/', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert!(snapshot.editing_text.contains('/'), "{snapshot:?}");

    // `/` 出顿号：上屏首选之后 `/` 还是顿号。
    let mut session = xiaohe_with(&fixture, |options| {
        options.punctuation_marks.enumeration_comma_slash = true;
    });
    type_text(&mut session, "nuu");
    assert_slash_left_alone(&mut session, "nuu");
    assert!(session.finish(0).commit.is_some());
    assert_eq!(session.punctuation(b'/').commit.as_deref(), Some("、"));
}

#[test]
fn ascii_punctuation_keeps_the_slash_literal() {
    let fixture = fixture();
    for configure in [
        (|options: &mut SessionOptions| options.chinese_punctuation = false)
            as fn(&mut SessionOptions),
        |options: &mut SessionOptions| options.punctuation_lock = 2,
    ] {
        let mut session = xiaohe_with(&fixture, configure);
        type_text(&mut session, "nuu");
        assert_slash_left_alone(&mut session, "nuu");
    }
}

#[test]
fn compositions_that_are_not_plain_letters_at_the_end_are_left_alone() {
    let fixture = fixture();

    // 大写辅码字母。
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nu");
    assert!(session.character(b'U', false).handled);
    assert!(!publishes_slash(&session));
    assert!(!session.character(b'/', false).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // 音节分隔符。
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nu'u");
    assert_slash_left_alone(&mut session, "nu'u");

    // 微软双拼的 `;` 是韵母 ing。
    let mut session = shuangpin_with(&fixture, ShuangpinProfileKind::Microsoft, |_| {});
    type_text(&mut session, "dex;");
    assert_slash_left_alone(&mut session, "dex;");

    // 光标在组字中间。
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nuu");
    session.set_caret(Some(1));
    assert!(!publishes_slash(&session));
    session.character(b'/', false);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // 已经选了一段（你），还剩「是谁」在组字里。
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "niuiuv");
    let at = index_of(&session, "你");
    assert!(session.select(at).handled);
    assert_eq!(session.snapshot().editing_text, "uiuv");
    assert!(!publishes_slash(&session));
    session.character(b'/', false);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
}

#[test]
fn backspace_takes_the_conversion_back() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nuu");
    let before = words(&session);
    session.character(b'/', false);
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.editing_text, "nuu");
    assert_eq!(words(&session), before);
    // 回来之后 `/` 还能再转一次。
    assert!(publishes_slash(&session));

    // 多打的字母先一个个删掉，删回原文后再退格才撤回。
    session.character(b'/', false);
    type_text(&mut session, "a");
    assert_converted(&session, "Jnuua");
    session.command(Command::Backspace);
    assert_converted(&session, "Jnuu");
    session.command(Command::Backspace);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.editing_text, "nuu");
}

#[test]
fn a_second_slash_commits_the_first_candidate_and_the_slash() {
    let fixture = fixture();
    for route in [false, true] {
        let mut session = xiaohe(&fixture);
        type_text(&mut session, "nuu");
        let first = words(&session)[0].clone();
        session.character(b'/', false);
        assert!(publishes_slash(&session));
        let result = if route {
            session.punctuation(b'/')
        } else {
            session.character(b'/', false)
        };
        assert!(result.handled);
        assert_eq!(result.commit, Some(format!("{first}/")));
        let snapshot = session.snapshot();
        assert_eq!(snapshot.local_mode, LocalInputMode::None);
        assert!(snapshot.editing_text.is_empty());
    }
}

#[test]
fn once_the_code_changed_the_slash_is_no_longer_an_escape() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nuu");
    session.character(b'/', false);
    type_text(&mut session, "a");
    assert!(!publishes_slash(&session));
}

#[test]
fn enter_commits_what_was_typed_without_the_j() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nuu");
    session.character(b'/', false);
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("nuu")
    );

    type_text(&mut session, "nuu");
    session.character(b'/', false);
    type_text(&mut session, "a");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("nuua")
    );

    // Shift+J 进入的超级简拼照旧上屏带 J 的原文（#7616）。
    session.character(b'J', true);
    type_text(&mut session, "nuu");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("Jnuu")
    );
}

#[test]
fn an_unmatched_code_falls_back_to_the_typed_letters() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "qqq");
    session.character(b'/', false);
    assert_converted(&session, "Jqqq");
    assert_eq!(words(&session), ["qqq"]);
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("qqq")
    );
}

#[test]
fn escape_forgets_the_conversion() {
    let fixture = fixture();
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "nuu");
    session.character(b'/', false);
    session.command(Command::Cancel);
    assert!(session.snapshot().editing_text.is_empty());
    session.character(b'J', true);
    type_text(&mut session, "nuu");
    // Shift+J 进入的不是 `/` 转出来的：退格只删字母，回车带 J。
    assert!(!publishes_slash(&session));
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("Jnuu")
    );
}

#[test]
fn quanpin_keeps_the_slash_as_punctuation() {
    let fixture = fixture();
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    assert_slash_left_alone(&mut session, "nihao");
}

/// 上一个上屏的词是这段组字的上下文：转成简拼再撤回（退格或 `//`）时它还在，首选和 `//` 上屏的词与没按过 `/` 时相同。
#[test]
fn taking_the_conversion_back_keeps_the_previous_word_as_context() {
    let fixture = Fixture::new(&dictionary(&[
        ("hao", "子", 100),
        ("ni'hao", "甲甲", 100),
        ("ni'hao", "乙乙", 90),
    ]));
    let journal = crate::user_dictionary::journal::open_journal(&fixture.journal()).unwrap();
    journal
        .execute(
            "INSERT INTO personal_bigram(previous,word,count) VALUES('子','乙乙',32)",
            [],
        )
        .unwrap();
    drop(journal);
    let mut session = xiaohe(&fixture);
    type_text(&mut session, "hc");
    select_word(&mut session, "子");
    type_text(&mut session, "nihc");
    let before = words(&session);
    assert_eq!(before[0], "乙乙", "{before:?}");

    assert!(session.character(b'/', false).handled);
    assert_converted(&session, "Jnihc");
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().editing_text, "nihc");
    assert_eq!(words(&session), before);

    assert!(session.character(b'/', false).handled);
    assert_eq!(
        session.character(b'/', false).commit.as_deref(),
        Some("乙乙/")
    );
}
