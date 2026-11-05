//! 十四／十八键网格辅助码（`nine_key::grid_helpcode`）：合成词库上覆盖四种键盘的状态、匹配和收尾。

use super::*;
use crate::helpcode::HelpcodeKeymap;
use crate::{
    resolve_eighteen_key_shuangpin, resolve_fourteen_key_shuangpin, GridHelpcodeOp as Op,
    SessionSnapshot,
};

const SQL: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_n VALUES('ni','n','你',100),('ni','n','拟',80),('ni','n','逆',60);
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_h VALUES('hao','h','好',100),('hao','h','号',80);
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000),('ni''hao','nh','拟好',800),('ni''hao','nh','逆好',600);";

fn layouts() -> [KeyGrid; 4] {
    [
        KeyGrid::FourteenKey,
        KeyGrid::EighteenKey,
        KeyGrid::FourteenKeyShuangpin(
            resolve_fourteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None).unwrap(),
        ),
        KeyGrid::EighteenKeyShuangpin(
            resolve_eighteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None).unwrap(),
        ),
    ]
}

fn shuangpin(grid: KeyGrid) -> bool {
    matches!(
        grid,
        KeyGrid::FourteenKeyShuangpin(_) | KeyGrid::EighteenKeyShuangpin(_)
    )
}

fn eighteen(grid: KeyGrid) -> bool {
    matches!(
        grid,
        KeyGrid::EighteenKey | KeyGrid::EighteenKeyShuangpin(_)
    )
}

fn keymap(codes: &[(&str, &str)]) -> crate::helpcode::SharedKeymap {
    Arc::new(HelpcodeKeymap::from_codes(
        codes
            .iter()
            .map(|(word, code)| ((*word).to_owned(), (*code).to_owned()))
            .collect(),
    ))
}

const CODES: &[(&str, &str)] = &[
    ("你", "ab"),
    ("拟", "cd"),
    ("逆", "ea"),
    ("好", "ef"),
    ("号", "gh"),
];

fn session_with(
    fixture: &Fixture,
    grid: KeyGrid,
    configure: impl FnOnce(&mut SessionOptions),
) -> Session {
    let mut session = fixture.session_with(|options| {
        options.scheme = if shuangpin(grid) {
            SchemeType::Shuangpin
        } else {
            SchemeType::Quanpin
        };
        options.learning = false;
        options.helpcode_table = Some(keymap(CODES));
        options.grid_helpcode = true;
        configure(options);
    });
    session.set_key_grid(Some(grid));
    session
}

fn type_reading(session: &mut Session, grid: KeyGrid, reading: &str) {
    let spelled = if shuangpin(grid) {
        reading.to_owned()
    } else {
        reading.replace('\'', "")
    };
    for key in grid.encode(&spelled).bytes() {
        assert!(session.grid_key(key).handled, "{grid:?} {reading}");
    }
}

/// 打好 `ni'hao` 的会话。
fn typed(fixture: &Fixture, grid: KeyGrid, strict: bool) -> Session {
    let mut session = session_with(fixture, grid, |options| {
        options.strict_quanpin_helpcode = strict;
    });
    type_reading(&mut session, grid, "ni'hao");
    session
}

fn apply(session: &mut Session, op: Op) {
    let result = session.grid_helpcode(op);
    assert!(
        result.handled && result.commit.is_none() && result.diagnostic.is_none(),
        "{op:?}: {result:?}"
    );
}

/// 覆盖全部组码的行。
fn whole_words(session: &Session) -> Vec<String> {
    let snapshot = session.snapshot();
    let length = snapshot.editing_text.len();
    snapshot
        .candidates
        .iter()
        .filter(|row| row.pinyin.len() == length)
        .map(|row| row.word.to_string())
        .collect()
}

/// 组字与候选的可见部分，比较两次快照用。
fn composition(snapshot: &SessionSnapshot) -> (Vec<String>, String, String, String) {
    (
        snapshot
            .candidates
            .iter()
            .map(|row| row.word.to_string())
            .collect(),
        snapshot.preedit.clone(),
        snapshot.editing_text.clone(),
        snapshot.nine_key_reading.clone(),
    )
}

fn shown(grid: KeyGrid, letters: &str) -> String {
    letters
        .chars()
        .enumerate()
        .map(|(index, letter)| {
            if shuangpin(grid) && index == 0 {
                letter
            } else {
                letter.to_ascii_uppercase()
            }
        })
        .collect()
}

#[test]
fn the_separator_alone_keeps_the_ordinary_list() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        let before = session.snapshot();
        assert!(before.grid_helpcode_available, "{grid:?}");
        assert!(before.grid_helpcode_display.is_empty(), "{grid:?}");
        apply(&mut session, Op::Begin);
        let after = session.snapshot();
        assert_eq!(after.grid_helpcode_display, "|", "{grid:?}");
        assert!(!after.grid_helpcode_constrained, "{grid:?}");
        assert!(after.nine_key_spellings.is_empty(), "{grid:?}");
        assert_eq!(composition(&before), composition(&after), "{grid:?}");
        let revision = session.candidate_revision();
        apply(&mut session, Op::Begin);
        assert_eq!(session.candidate_revision(), revision, "{grid:?}");
        assert!(session.command(Command::Backspace).handled);
        let back = session.snapshot();
        assert!(back.grid_helpcode_display.is_empty(), "{grid:?}");
        assert_eq!(
            back.nine_key_spellings, before.nine_key_spellings,
            "{grid:?}"
        );
        assert_eq!(composition(&before), composition(&back), "{grid:?}");
        let revision = session.candidate_revision();
        assert!(!session.grid_helpcode(Op::Clear).handled, "{grid:?}");
        assert_eq!(session.candidate_revision(), revision, "{grid:?}");
    }
}

#[test]
fn the_switch_alone_changes_nothing_in_ordinary_typing() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let on = typed(&fixture, grid, false);
        let mut off = session_with(&fixture, grid, |options| options.grid_helpcode = false);
        type_reading(&mut off, grid, "ni'hao");
        let (on, off) = (on.snapshot(), off.snapshot());
        assert_eq!(composition(&on), composition(&off), "{grid:?}");
        assert_eq!(on.nine_key_spellings, off.nine_key_spellings, "{grid:?}");
        assert_eq!(
            on.candidate_annotations, off.candidate_annotations,
            "{grid:?}"
        );
        assert!(on.grid_helpcode_available && !off.grid_helpcode_available);
    }
}

#[test]
fn key_groups_fill_slots_and_a_whole_choice_locks_them() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Group(b'a'));
        apply(&mut session, Op::Group(b'e'));
        let snapshot = session.snapshot();
        assert!(snapshot.grid_helpcode_constrained, "{grid:?}");
        // 十四键 A 与 S、E 与 R 同键；十八键 A 单独一键，E 与 W 同键。
        let (display, count) = match (eighteen(grid), shuangpin(grid)) {
            (false, false) => ("|[AS][ER]", 4),
            (false, true) => ("|[as][ER]", 4),
            (true, false) => ("|A[WE]", 2),
            (true, true) => ("|a[WE]", 2),
        };
        assert_eq!(snapshot.grid_helpcode_display, display, "{grid:?}");
        assert_eq!(snapshot.nine_key_spellings.len(), count, "{grid:?}");
        assert_eq!(whole_words(&session), ["你好"], "{grid:?}");
        let index = snapshot
            .nine_key_spellings
            .iter()
            .position(|choice| choice.eq_ignore_ascii_case("ae"))
            .expect("A E 的整串解释");
        assert_eq!(snapshot.nine_key_spellings[index], shown(grid, "ae"));
        let result = session.choose_nine_key_spelling(index);
        assert!(result.handled && result.commit.is_none(), "{grid:?}");
        let locked = session.snapshot();
        assert_eq!(
            locked.grid_helpcode_display,
            format!("|{}", shown(grid, "ae"))
        );
        assert!(locked.nine_key_spellings.is_empty(), "{grid:?}");
        assert_eq!(whole_words(&session), ["你好"], "{grid:?}");
        assert!(!session.choose_nine_key_spelling(0).handled, "{grid:?}");
    }
}

/// 放下分隔符后组码不再变：`grid_key` 一律不收，辅助码只从 `Group`、`Exact` 进，二者也只收小写，大写留给与 26 键一致的含义（双拼一码大写是尾字优先）。分隔符之前十八键双拼的大写照旧是主码的确定字母。
#[test]
fn grid_keys_and_uppercase_letters_are_refused_after_the_separator() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        for slots in [0, 1, 2] {
            let before = session.snapshot();
            for letter in *b"aewAEW" {
                assert!(!session.grid_key(letter).handled, "{grid:?} {slots}");
                let after = session.snapshot();
                assert_eq!(composition(&before), composition(&after), "{grid:?}");
                assert_eq!(before.grid_helpcode_display, after.grid_helpcode_display);
                assert_eq!(before.nine_key_spellings, after.nine_key_spellings);
            }
            for op in [Op::Group(b'E'), Op::Exact(b'E'), Op::Group(b'1')] {
                assert!(!session.grid_helpcode(op).handled, "{grid:?} {op:?}");
            }
            if slots < 2 {
                apply(&mut session, Op::Group(b'a'));
            }
        }
    }
    let grid = layouts()[3];
    let fixture = Fixture::new(SQL);
    let mut session = typed(&fixture, grid, false);
    let length = session.snapshot().editing_text.len();
    assert!(session.grid_key(b'H').handled);
    assert_eq!(session.snapshot().editing_text.len(), length + 1);
}

#[test]
fn a_third_letter_and_repeated_entry_change_nothing() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'a'));
        apply(&mut session, Op::Exact(b'e'));
        let full = session.snapshot();
        assert_eq!(
            full.grid_helpcode_display,
            format!("|{}", shown(grid, "ae"))
        );
        let revision = session.candidate_revision();
        apply(&mut session, Op::Group(b'q'));
        apply(&mut session, Op::Exact(b'z'));
        apply(&mut session, Op::Begin);
        assert_eq!(session.candidate_revision(), revision, "{grid:?}");
        let after = session.snapshot();
        assert_eq!(composition(&full), composition(&after), "{grid:?}");
        assert_eq!(full.grid_helpcode_display, after.grid_helpcode_display);
        assert!(!session.grid_helpcode(Op::Exact(b'1')).handled, "{grid:?}");
    }
}

/// 一码：首码命中（或首尾都中）、只有尾码命中、都不中依次排；严格时全拼只筛不排，双拼仍让首码命中的在前，与 26 键相同。
#[test]
fn one_code_ranks_like_the_26_key_rules() {
    for grid in layouts() {
        for strict in [false, true] {
            let fixture = Fixture::new(SQL);
            let mut session = typed(&fixture, grid, strict);
            apply(&mut session, Op::Begin);
            apply(&mut session, Op::Exact(b'e'));
            // 逆好首尾都是 e，你好、拟好只有尾字好是 e。
            let expected: &[&str] = if strict && !shuangpin(grid) {
                &["你好", "拟好", "逆好"]
            } else {
                &["逆好", "你好", "拟好"]
            };
            assert_eq!(whole_words(&session), expected, "{grid:?} strict={strict}");
            assert!(session.command(Command::Backspace).handled);
            apply(&mut session, Op::Exact(b'a'));
            let expected: &[&str] = if strict {
                &["你好"]
            } else {
                &["你好", "拟好", "逆好"]
            };
            assert_eq!(whole_words(&session), expected, "{grid:?} strict={strict}");
        }
    }
}

/// 两码按码序严格匹配，与双拼显示成首码小写、次码大写无关；单字取前两码。
#[test]
fn two_codes_match_in_order() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'e'));
        apply(&mut session, Op::Exact(b'a'));
        assert!(whole_words(&session).is_empty(), "{grid:?}");
        assert_eq!(words(&session), ["逆"], "{grid:?}");
    }
}

#[test]
fn backspace_takes_slots_then_the_separator_before_the_code() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        let plain = session.snapshot();
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'a'));
        apply(&mut session, Op::Exact(b'e'));
        for display in [shown(grid, "a"), String::new()] {
            assert!(session.command(Command::Backspace).handled);
            let snapshot = session.snapshot();
            assert_eq!(snapshot.grid_helpcode_display, format!("|{display}"));
            assert_eq!(snapshot.editing_text, plain.editing_text, "{grid:?}");
        }
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(composition(&plain), composition(&session.snapshot()));
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(
            session.snapshot().editing_text.len() + 1,
            plain.editing_text.len(),
            "{grid:?}"
        );
    }
}

#[test]
fn the_split_key_clears_the_helpcode_and_then_splits() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        let mut control = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'a'));
        assert!(session.character(b'\'', false).handled);
        assert!(control.character(b'\'', false).handled);
        let (snapshot, control) = (session.snapshot(), control.snapshot());
        assert!(snapshot.grid_helpcode_display.is_empty(), "{grid:?}");
        assert_eq!(composition(&snapshot), composition(&control), "{grid:?}");
        assert_eq!(snapshot.nine_key_spellings, control.nine_key_spellings);
    }
}

#[test]
fn choosing_a_word_clears_the_helpcode() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'a'));
        let index = words(&session)
            .iter()
            .position(|word| word == "你")
            .expect("部分选择的单字");
        let result = session.select(index);
        assert_eq!(result.commit.as_deref(), Some("你"), "{grid:?}");
        let snapshot = session.snapshot();
        assert!(snapshot.grid_helpcode_display.is_empty(), "{grid:?}");
        assert!(!snapshot.grid_helpcode_constrained, "{grid:?}");
        assert_eq!(whole_words(&session)[0], "好", "{grid:?}");
    }
}

/// 筛空时会结束组字的键都不上屏，组字留着；有候选时数字照旧交给 runtime 按页选词。
#[test]
fn an_empty_filter_holds_keys_that_would_end_the_composition() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'a'));
        assert!(!session.character(b'1', false).handled, "{grid:?}");
        apply(&mut session, Op::Exact(b'z'));
        let held = session.snapshot();
        assert!(held.candidates.is_empty(), "{grid:?}");
        let results = [
            session.command(Command::CommitCandidate),
            session.punctuation(b','),
            session.character(b' ', false),
            session.character(b'1', false),
        ];
        for result in results {
            assert!(
                result.handled && result.commit.is_none(),
                "{grid:?}: {result:?}"
            );
        }
        let after = session.snapshot();
        assert_eq!(composition(&held), composition(&after), "{grid:?}");
        assert_eq!(held.grid_helpcode_display, after.grid_helpcode_display);
    }
}

/// 筛空时宿主结束组字（原样上屏、结束组字）丢掉这次组字、什么也不上屏，组码字母不会被当原文上屏；有候选时原样上屏照旧按首选结束。
#[test]
fn host_ends_drop_an_empty_filtered_composition() {
    for grid in layouts() {
        for end in [
            |session: &mut Session| session.command(Command::CommitRaw),
            |session: &mut Session| session.finish(0),
        ] {
            let fixture = Fixture::new(SQL);
            let mut session = typed(&fixture, grid, false);
            apply(&mut session, Op::Begin);
            apply(&mut session, Op::Exact(b'z'));
            apply(&mut session, Op::Exact(b'z'));
            assert!(session.snapshot().candidates.is_empty(), "{grid:?}");
            let result = end(&mut session);
            assert!(
                result.handled && result.commit.is_none(),
                "{grid:?}: {result:?}"
            );
            let after = session.snapshot();
            assert!(
                after.editing_text.is_empty() && after.candidates.is_empty(),
                "{grid:?}"
            );
            assert!(after.grid_helpcode_display.is_empty(), "{grid:?}");
        }
        let fixture = Fixture::new(SQL);
        let mut session = typed(&fixture, grid, false);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'e'));
        let first = session.snapshot().candidates[0].word.to_string();
        assert_eq!(first, "逆好", "{grid:?}");
        let result = session.command(Command::CommitRaw);
        assert_eq!(result.commit.as_deref(), Some("逆好"), "{grid:?}");
    }
}

/// 单字筛选与辅助码并存；笔画筛选与辅助码互斥，后启用的一方清掉另一方。
#[test]
fn single_characters_combine_and_strokes_exclude() {
    for grid in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = session_with(&fixture, grid, |options| {
            options.stroke_dictionary = stroke_dictionary(fixture.path());
        });
        type_reading(&mut session, grid, "ni'hao");
        assert!(session.set_nine_key_filter(true, "").handled);
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'e'));
        let snapshot = session.snapshot();
        assert!(snapshot.nine_key_single_character, "{grid:?}");
        assert!(!snapshot.candidates.is_empty(), "{grid:?}");
        assert!(
            snapshot
                .candidates
                .iter()
                .all(|row| row.word.chars().count() == 1),
            "{grid:?}"
        );
        assert!(session.set_nine_key_filter(false, "h").handled);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.nine_key_strokes, "h", "{grid:?}");
        assert!(snapshot.grid_helpcode_display.is_empty(), "{grid:?}");
        apply(&mut session, Op::Begin);
        assert!(session.snapshot().nine_key_strokes.is_empty(), "{grid:?}");
    }
}

/// 筛过的列表里选词不调频、不记最近选择，也不能置顶、删除或固定位置。
#[test]
fn filtered_picks_leave_the_ordinary_order_alone() {
    let grid = KeyGrid::FourteenKey;
    let fixture = Fixture::new(SQL);
    let learning = |fixture: &Fixture| {
        let mut session = session_with(fixture, grid, |options| options.learning = true);
        type_reading(&mut session, grid, "ni'hao");
        session
    };
    let mut session = learning(&fixture);
    apply(&mut session, Op::Begin);
    apply(&mut session, Op::Exact(b'e'));
    assert_eq!(whole_words(&session), ["逆好", "你好", "拟好"]);
    for index in 0..3 {
        assert!(!session.pin(index).handled);
        assert!(!session.remove(index).handled);
        assert!(!session.fix_position(index, 1).handled);
    }
    assert_eq!(session.select(2).commit.as_deref(), Some("拟好"));
    assert_eq!(whole_words(&learning(&fixture))[0], "你好");
    // 对照：同一下标在普通列表里选中会记成最近选择，下次排到首位。
    let mut control = learning(&fixture);
    assert_eq!(whole_words(&control)[2], "逆好");
    assert_eq!(control.select(2).commit.as_deref(), Some("逆好"));
    assert_eq!(whole_words(&learning(&fixture))[0], "逆好");
}

#[test]
fn the_table_is_taken_when_the_separator_goes_down() {
    let grid = KeyGrid::EighteenKey;
    let fixture = Fixture::new(SQL);
    let mut session = typed(&fixture, grid, true);
    apply(&mut session, Op::Begin);
    session.set_helpcode_table(keymap(&[("你", "zz"), ("好", "zz")]));
    apply(&mut session, Op::Exact(b'a'));
    assert_eq!(whole_words(&session), ["你好"]);
    apply(&mut session, Op::Clear);
    apply(&mut session, Op::Begin);
    apply(&mut session, Op::Exact(b'a'));
    assert!(whole_words(&session).is_empty());
}

/// 已经查到、但排在普通候选池第 128 行之后的字，筛选后照样能出来。
#[test]
fn queried_rows_past_the_candidate_pool_still_match() {
    let rows: Vec<String> = (0..200u32)
        .map(|offset| {
            let word = char::from_u32(0x4e00 + offset).unwrap();
            format!("('ni','n','{word}',{})", 10_000 - offset)
        })
        .chain(std::iter::once("('ni','n','逆',1)".to_owned()))
        .collect();
    let sql = format!(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES{};",
        rows.join(",")
    );
    for grid in layouts() {
        let fixture = Fixture::new(&sql);
        let mut session = session_with(&fixture, grid, |options| {
            options.helpcode_table = Some(keymap(&[("逆", "qa")]));
            options.strict_quanpin_helpcode = true;
        });
        type_reading(&mut session, grid, "ni");
        assert!(!words(&session).iter().any(|word| word == "逆"), "{grid:?}");
        apply(&mut session, Op::Begin);
        apply(&mut session, Op::Exact(b'q'));
        assert_eq!(words(&session), ["逆"], "{grid:?}");
    }
}

/// 筛选后排到首屏的粗俗词照样挪出首屏（`offensive_placement`），往后翻仍在。
#[test]
fn vulgar_words_stay_off_the_first_page_after_filtering() {
    let sql = "CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_b VALUES('bie','b','别',1000);
CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_b VALUES('bie''ji','bj','别急',500);
CREATE TABLE tbl_3_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_3_b VALUES('bie''ji''ba','bjb','别急吧',3000),('bie''ji''ba','bjb','别鸡巴',100);";
    let grid = KeyGrid::FourteenKey;
    let fixture = Fixture::new(sql);
    let mut session = session_with(&fixture, grid, |options| {
        options.helpcode_table = Some(keymap(&[
            ("别", "bi"),
            ("急", "jj"),
            ("吧", "ba"),
            ("巴", "xa"),
        ]));
        options.first_page_rows = 3;
    });
    type_reading(&mut session, grid, "bie'ji'ba");
    apply(&mut session, Op::Begin);
    apply(&mut session, Op::Exact(b'x'));
    let listed = words(&session);
    assert!(
        !listed[..3].iter().any(|word| word == "别鸡巴"),
        "{listed:?}"
    );
    assert!(listed.iter().any(|word| word == "别鸡巴"), "{listed:?}");
}

#[test]
fn nine_key_and_switched_off_sessions_refuse_every_operation() {
    let fixture = Fixture::new(SQL);
    let mut nine_key = fixture.session_with(|options| {
        options.helpcode_table = Some(keymap(CODES));
        options.grid_helpcode = true;
    });
    nine_key.set_nine_key_enabled(true);
    type_text(&mut nine_key, "64426");
    let mut off = session_with(&fixture, KeyGrid::FourteenKey, |options| {
        options.grid_helpcode = false;
    });
    type_reading(&mut off, KeyGrid::FourteenKey, "ni'hao");
    let mut no_helpcode = session_with(&fixture, KeyGrid::EighteenKey, |options| {
        options.helpcode = false;
    });
    type_reading(&mut no_helpcode, KeyGrid::EighteenKey, "ni'hao");
    for session in [&mut nine_key, &mut off, &mut no_helpcode] {
        let before = session.snapshot();
        let revision = session.candidate_revision();
        assert!(!before.grid_helpcode_available);
        for op in [Op::Begin, Op::Exact(b'a'), Op::Clear] {
            assert!(!session.grid_helpcode(op).handled, "{op:?}");
        }
        assert_eq!(session.candidate_revision(), revision);
        assert_eq!(composition(&before), composition(&session.snapshot()));
    }
}
