//! 14／18 键全拼的横滑精确字母（`nine_key::grid_exact`）：合成词库上，大写精确码只留对应字母的读法，与点按混用，并且在编辑、锁定、选词和兜底上屏里保持。

use super::*;

/// 14 键里「你好」「不好」都是 `bugao`（BN UI GH AS OP），十八键里「你好」「波好」都是 `bihai`（BN IO H A IO）。
const SQL: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_n VALUES('ni','n','你',100);
CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_b VALUES('bu','b','不',95),('bi','b','比',90),('bo','b','波',85);
CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_l VALUES('li','l','里',99);
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_h VALUES('hao','h','好',100);
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_g VALUES('gao','g','高',90);
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);
CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_b VALUES('bu''hao','bh','不好',1100),('bo''hao','bh','波好',1050);";

/// 一种全拼网格上的几串组码：`group` 是「你好」的整组码，`exact` 把首字母换成精确的 `N`，`middle` 在第二位放一个排除「你好」的精确码、留下对手，`rival` 是同一串整组码上的对手词。
struct Layout {
    grid: KeyGrid,
    group: &'static str,
    exact: &'static str,
    middle: &'static str,
    rival: &'static str,
    /// 「你」的两位：`N` 加上 i 所在组的组码。
    ni: &'static str,
    /// 简拼「你好」：精确的 `N` 加上 h 所在组的组码。
    initials: &'static str,
    /// 同键的另一个字母，也不起头任何词：无候选时上屏用。
    lone: u8,
}

const LAYOUTS: [Layout; 2] = [
    Layout {
        grid: KeyGrid::FourteenKey,
        group: "bugao",
        exact: "Nugao",
        middle: "bUgao",
        rival: "不好",
        ni: "Nu",
        initials: "Ng",
        lone: b'W',
    },
    Layout {
        grid: KeyGrid::EighteenKey,
        group: "bihai",
        exact: "Nihai",
        middle: "bOhai",
        rival: "波好",
        ni: "Ni",
        initials: "Nh",
        lone: b'W',
    },
];

fn session(fixture: &Fixture, layout: &Layout) -> Session {
    session_with(fixture, layout, |_| {})
}

fn session_with(
    fixture: &Fixture,
    layout: &Layout,
    configure: impl FnOnce(&mut SessionOptions),
) -> Session {
    let mut session = fixture.session_with(|options| {
        options.learning = false;
        options.english.mixed_candidates = false;
        configure(options);
    });
    session.set_key_grid(Some(layout.grid));
    session
}

fn type_grid(session: &mut Session, codes: &str) {
    for code in codes.bytes() {
        assert!(session.grid_key(code).handled, "{codes}");
    }
}

#[test]
fn exact_letters_narrow_the_readings_and_mix_with_taps() {
    let fixture = Fixture::new(SQL);
    for layout in &LAYOUTS {
        let grid = layout.grid;
        let mut session = session(&fixture, layout);
        type_grid(&mut session, layout.group);
        let both = words(&session);
        assert!(
            both.contains(&"你好".to_owned()) && both.contains(&layout.rival.to_owned()),
            "{grid:?} {both:?}"
        );
        session.command(Command::Cancel);

        type_grid(&mut session, layout.exact);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.editing_text, layout.exact, "{grid:?}");
        let narrowed = words(&session);
        assert!(
            narrowed.contains(&"你好".to_owned()),
            "{grid:?} {narrowed:?}"
        );
        assert!(
            !narrowed.contains(&layout.rival.to_owned()),
            "{grid:?} {narrowed:?}"
        );
        assert!(
            snapshot
                .nine_key_spellings
                .iter()
                .all(|spelling| !spelling.starts_with('b')),
            "{grid:?} {:?}",
            snapshot.nine_key_spellings
        );
        assert_eq!(snapshot.nine_key_reading, "ni'hao", "{grid:?}");
        assert_eq!(
            select_word(&mut session, "你好").commit.as_deref(),
            Some("你好"),
            "{grid:?}"
        );

        // 中间一位的精确码排除「你好」，留下对手。
        type_grid(&mut session, layout.middle);
        let middle = words(&session);
        assert!(
            middle.contains(&layout.rival.to_owned()),
            "{grid:?} {middle:?}"
        );
        assert!(!middle.contains(&"你好".to_owned()), "{grid:?} {middle:?}");
        session.command(Command::Cancel);

        // 末尾一位：左列的首字母选项只剩这个字母。
        type_grid(&mut session, "b");
        assert!(session
            .snapshot()
            .nine_key_spellings
            .contains(&"N".to_owned()));
        session.command(Command::Cancel);
        type_grid(&mut session, "N");
        let letters: Vec<String> = session
            .snapshot()
            .nine_key_spellings
            .into_iter()
            .filter(|spelling| spelling.len() == 1 && spelling.as_bytes()[0].is_ascii_uppercase())
            .collect();
        assert_eq!(letters, ["N"], "{grid:?}");
        session.command(Command::Cancel);
    }
}

#[test]
fn exact_letters_survive_splits_locks_and_caret_edits() {
    let fixture = Fixture::new(SQL);
    for layout in &LAYOUTS {
        let grid = layout.grid;
        let mut session = session(&fixture, layout);
        // 切分在精确码之后：读法照旧受限。
        type_grid(&mut session, layout.ni);
        assert!(session.character(b'\'', false).handled, "{grid:?}");
        type_grid(&mut session, &layout.group[2..]);
        let split = words(&session);
        assert!(split.contains(&"你好".to_owned()), "{grid:?} {split:?}");
        assert!(
            !split.contains(&layout.rival.to_owned()),
            "{grid:?} {split:?}"
        );
        session.command(Command::Cancel);

        // 锁定拼写后组码串里仍是精确码；退格撤销锁定后也是。
        type_grid(&mut session, layout.ni);
        let ni = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|spelling| spelling == "ni")
            .unwrap_or_else(|| panic!("{grid:?}"));
        assert!(session.choose_nine_key_spelling(ni).handled);
        assert_eq!(session.snapshot().editing_text, layout.ni, "{grid:?}");
        assert_eq!(session.snapshot().nine_key_reading, "ni", "{grid:?}");
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, layout.ni, "{grid:?}");
        assert!(
            !session
                .snapshot()
                .nine_key_spellings
                .iter()
                .any(|spelling| spelling == "bi" || spelling == "bu"),
            "{grid:?}"
        );

        // 再锁一次，把光标移进锁定的拼写里插一键：锁定解除，精确码还在，不会放宽回整组。
        let ni = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|spelling| spelling == "ni")
            .unwrap();
        assert!(session.choose_nine_key_spelling(ni).handled);
        session.command(Command::MoveLeft);
        type_grid(&mut session, "H");
        assert_eq!(
            session.snapshot().editing_text,
            format!("NH{}", &layout.ni[1..]),
            "{grid:?}"
        );
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, layout.ni, "{grid:?}");
        session.command(Command::DeleteForward);
        assert_eq!(session.snapshot().editing_text, "N", "{grid:?}");
        session.command(Command::Cancel);
    }
}

#[test]
fn exact_letters_narrow_initials_and_keep_fuzzy_readings_of_the_typed_letter() {
    let fixture = Fixture::new(SQL);
    for layout in &LAYOUTS {
        let grid = layout.grid;
        let mut session = session(&fixture, layout);
        // 简拼：`bg`／`bh` 两种整组都查到对手，精确的 `N` 只留 n 开头的词。
        let group_initials = format!("b{}", &layout.initials[1..]);
        type_grid(&mut session, &group_initials);
        assert!(
            words(&session).contains(&layout.rival.to_owned()),
            "{grid:?}"
        );
        session.command(Command::Cancel);
        type_grid(&mut session, layout.initials);
        let initials = words(&session);
        assert!(
            initials.contains(&"你好".to_owned()),
            "{grid:?} {initials:?}"
        );
        assert!(
            !initials.contains(&layout.rival.to_owned()),
            "{grid:?} {initials:?}"
        );
        session.command(Command::Cancel);

        // 模糊音按打出的那一边核对：精确的 n 在 n/l 模糊音下照样出 l 的读法，没有模糊音时不出。
        for (rules, fuzzy) in [(crate::types::fuzzy_rule::N_L, true), (0, false)] {
            let mut session = session_with(&fixture, layout, |options| {
                options.fuzzy_pinyin.rules = rules
            });
            type_grid(&mut session, layout.ni);
            let readings = words(&session);
            assert!(readings.contains(&"你".to_owned()), "{grid:?} {readings:?}");
            assert_eq!(
                readings.contains(&"里".to_owned()),
                fuzzy,
                "{grid:?} {readings:?}"
            );
            assert!(
                !readings.contains(&"比".to_owned()),
                "{grid:?} {readings:?}"
            );
        }
    }
}

/// 无候选时原样上屏、结束组字和逐段结束后的余码都上屏小写字母，不泄漏内部的大写。
#[test]
fn exact_markers_do_not_escape_into_committed_text() {
    let fixture = Fixture::new(SQL);
    for layout in &LAYOUTS {
        let grid = layout.grid;
        for finish in [false, true] {
            let mut session = session(&fixture, layout);
            assert!(session.grid_key(layout.lone).handled, "{grid:?}");
            assert!(session.snapshot().candidates.is_empty(), "{grid:?}");
            let result = if finish {
                session.finish(0)
            } else {
                session.command(Command::CommitRaw)
            };
            let lone = char::from(layout.lone.to_ascii_lowercase()).to_string();
            assert_eq!(result.commit.as_deref(), Some(lone.as_str()), "{grid:?}");
            assert!(session.snapshot().editing_text.is_empty(), "{grid:?}");
        }
        let mut session = session(&fixture, layout);
        type_grid(&mut session, layout.ni);
        assert!(session.grid_key(layout.lone).handled);
        assert_eq!(session.snapshot().candidates[0].word, "你", "{grid:?}");
        assert_eq!(session.finish(0).commit.as_deref(), Some("你w"), "{grid:?}");
    }
}

#[test]
fn hardware_letters_dedicated_english_and_wubi_take_no_exact_letters() {
    let fixture = Fixture::new(SQL);
    for layout in &LAYOUTS {
        let grid = layout.grid;
        let mut session = session(&fixture, layout);
        assert!(session.character(b'n', false).handled);
        assert!(!session.grid_composing(), "{grid:?}");
        assert!(!session.grid_key(b'N').handled, "{grid:?}");
        session.command(Command::Cancel);
        session.set_dedicated_english(true);
        assert!(!session.grid_key(b'N').handled, "{grid:?}");
    }
    let mut wubi = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    wubi.set_key_grid(Some(KeyGrid::FourteenKey));
    let before = wubi.snapshot();
    assert!(!wubi.grid_key(b'Q').handled);
    assert_eq!(wubi.snapshot().editing_text, before.editing_text);
    assert!(wubi.grid_key(b'q').handled);
}
