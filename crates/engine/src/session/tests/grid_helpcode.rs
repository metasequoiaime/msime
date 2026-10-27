//! 六种网格使用同一辅筛协议，夹具沿用普通行为基线。

use super::grid_baseline::{layouts, lock_ni, start, type_grid, SQL};
use super::*;
use crate::grid_helpcode::{GridHelpcodeAction as A, GridHelpcodeAvailability as Available};
use crate::types::KeyResult;

fn enable(session: &mut Session) {
    let map = crate::helpcode::HelpcodeKeymap::from_table_bytes("你=ab\n拟=ba\n好=bc\n".as_bytes())
        .unwrap();
    session.set_helpcode_table(Arc::new(map));
    session.set_grid_helpcode_enabled(true);
}

fn action(session: &mut Session, action: A) -> KeyResult {
    let generation = session.grid_helpcode_snapshot().generation;
    session.grid_helpcode_action(generation, action)
}

#[test]
fn grid_helpcode_six_layouts_ranking_case_reading_and_restore() {
    for (grid, first, tail) in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = start(&fixture, grid);
        enable(&mut session);
        type_grid(&mut session, grid, first);
        lock_ni(&mut session);
        type_grid(&mut session, grid, tail);
        let ordinary = session.snapshot();
        let revision = session.grid_helpcode_snapshot().candidate_generation;
        assert!(action(&mut session, A::Begin).handled);
        assert_eq!(
            session.grid_helpcode_snapshot().candidate_generation,
            revision
        );
        assert_eq!(session.snapshot(), ordinary, "空编辑不改普通快照 {grid:?}");
        action(&mut session, A::AppendExact(b'b'));
        let revision = session.grid_helpcode_snapshot().candidate_generation;
        // 全覆盖仍优先；首码 b 优先拟好，未匹配或次优仍保留。
        assert_eq!(words(&session), ["拟好", "你好", "拟", "你"], "{grid:?}");
        let raw = session.snapshot().editing_text;
        action(&mut session, A::End);
        assert_eq!(
            session.grid_helpcode_snapshot().candidate_generation,
            revision
        );
        assert_eq!(session.grid_helpcode_snapshot().display, "b");
        let reading = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|value| value == "hao")
            .unwrap();
        session.choose_nine_key_spelling(reading);
        assert_eq!(session.grid_helpcode_snapshot().display, "b");
        action(&mut session, A::Begin);
        session.command(Command::Backspace);
        action(&mut session, A::ToggleCase);
        action(&mut session, A::AppendExact(b'a'));
        assert_eq!(
            words(&session),
            ["你好", "拟好", "拟", "你"],
            "大写末向，覆盖优先 {grid:?}"
        );
        assert_eq!(session.grid_helpcode_snapshot().display, "A");
        assert_eq!(session.snapshot().editing_text, raw);
        assert!(!session.snapshot().preedit.contains('|'));
        action(&mut session, A::Clear);
        assert_eq!(words(&session), ["你好", "拟好", "你", "拟"]);
        assert!(!session.grid_helpcode_snapshot().editing);
    }
}

#[test]
fn grid_helpcode_strict_case_partial_commit_and_empty_results() {
    for (grid, first, tail) in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = start(&fixture, grid);
        enable(&mut session);
        type_grid(&mut session, grid, &format!("{first}{tail}"));
        action(&mut session, A::Begin);
        action(&mut session, A::AppendExact(b'b'));
        action(&mut session, A::ToggleCase);
        action(&mut session, A::AppendExact(b'a'));
        assert_eq!(session.grid_helpcode_snapshot().display, "bA");
        assert_eq!(words(&session), ["拟"], "第二码大小写不反转 {grid:?}");
        let result = session.select(0);
        assert_eq!(result.commit.as_deref(), Some("拟"));
        assert!(session.grid_helpcode_snapshot().slots.is_empty());
        assert!(!session.grid_helpcode_snapshot().editing);
        assert!(words(&session).contains(&"好".to_owned()));
        action(&mut session, A::Begin);
        action(&mut session, A::ToggleCase);
        action(&mut session, A::AppendExact(b'c'));
        action(&mut session, A::AppendExact(b'b'));
        assert_eq!(words(&session), ["好"]);
        session.command(Command::Backspace);
        action(&mut session, A::AppendExact(b'z'));
        assert!(words(&session).is_empty());
        let snapshot = session.grid_helpcode_snapshot();
        assert!(!session.select(0).handled);
        assert!(!session.command(Command::CommitCandidate).handled);
        assert!(!session.finish(0).handled);
        assert_eq!(session.grid_helpcode_snapshot(), snapshot);
        action(&mut session, A::Clear);
        assert_eq!(words(&session)[0], "好");
    }
}

#[test]
fn grid_helpcode_whole_combination_stale_choice_limit_and_backspace() {
    let fixture = Fixture::new(SQL);
    let mut session = start(&fixture, KeyGrid::FourteenKey);
    enable(&mut session);
    type_grid(&mut session, KeyGrid::FourteenKey, "bu");
    action(&mut session, A::Begin);
    action(&mut session, A::AppendGroup(b'q'));
    assert_eq!(session.grid_helpcode_snapshot().choices, ["q", "w"]);
    let stale = session.grid_helpcode_snapshot().generation;
    action(&mut session, A::ToggleCase);
    action(&mut session, A::AppendGroup(b'e'));
    let before = session.grid_helpcode_snapshot();
    assert_eq!(before.display, "[qw][ER]");
    assert_eq!(before.choices, ["qE", "qR", "wE", "wR"]);
    assert!(
        !session
            .grid_helpcode_action(stale, A::Choose("q".into()))
            .handled
    );
    assert!(!action(&mut session, A::Choose("qe".into())).handled);
    assert_eq!(session.grid_helpcode_snapshot(), before);
    assert_eq!(
        action(&mut session, A::AppendExact(b'a'))
            .diagnostic
            .as_deref(),
        Some("辅助码最多输入两位")
    );
    assert_eq!(session.grid_helpcode_snapshot(), before);
    action(&mut session, A::Choose("qR".into()));
    assert_eq!(session.grid_helpcode_snapshot().display, "qR");
    assert!(session.grid_helpcode_snapshot().choices.is_empty());
    session.command(Command::Backspace);
    action(&mut session, A::AppendGroup(b'e'));
    assert_eq!(session.grid_helpcode_snapshot().choices, ["qE", "qR"]);
    session.command(Command::Backspace);
    session.command(Command::Backspace);
    assert!(session.grid_helpcode_snapshot().editing);
    let raw = session.snapshot().editing_text;
    session.command(Command::Backspace);
    assert!(!session.grid_helpcode_snapshot().editing);
    assert_eq!(session.snapshot().editing_text, raw);
    action(&mut session, A::Begin);
    assert!(!session.grid_helpcode_snapshot().uppercase);
}

#[test]
fn grid_helpcode_invalid_inputs_continuation_separator_and_disable() {
    for (grid, first, tail) in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = start(&fixture, grid);
        enable(&mut session);
        type_grid(&mut session, grid, first);
        action(&mut session, A::Begin);
        action(&mut session, A::AppendExact(b'b'));
        let before = session.grid_helpcode_snapshot();
        assert!(!session.character(b'|', false).handled);
        assert!(!action(&mut session, A::AppendExact(b'B')).handled);
        assert!(!action(&mut session, A::AppendGroup(b'!')).handled);
        assert!(!session.select(usize::MAX).handled);
        assert_eq!(session.grid_helpcode_snapshot(), before);
        session.command(Command::MoveHome);
        assert_eq!(session.grid_helpcode_snapshot(), before);
        session.command(Command::MoveEnd);
        type_grid(&mut session, grid, tail);
        assert!(session.grid_helpcode_snapshot().slots.is_empty());
        action(&mut session, A::Begin);
        action(&mut session, A::AppendExact(b'b'));
        session.character(b'\'', false);
        assert!(session.grid_helpcode_snapshot().slots.is_empty());
        action(&mut session, A::Begin);
        action(&mut session, A::AppendExact(b'b'));
        action(&mut session, A::End);
        session.command(Command::Backspace);
        assert!(session.grid_helpcode_snapshot().slots.is_empty());
        action(&mut session, A::Begin);
        action(&mut session, A::AppendExact(b'b'));
        session.set_grid_helpcode_enabled(false);
        assert_eq!(
            session.grid_helpcode_snapshot().availability,
            Available::Disabled
        );
        assert!(session.grid_helpcode_snapshot().slots.is_empty());
    }
}

#[test]
fn grid_helpcode_availability_table_change_and_layout_reset() {
    let fixture = Fixture::new(SQL);
    let mut session = start(&fixture, KeyGrid::NineKey);
    assert_eq!(
        session.grid_helpcode_snapshot().availability,
        Available::Disabled
    );
    enable(&mut session);
    assert_eq!(
        session.grid_helpcode_snapshot().availability,
        Available::NotComposing
    );
    assert!(!action(&mut session, A::Begin).handled);
    type_grid(&mut session, KeyGrid::NineKey, "64");
    action(&mut session, A::Begin);
    action(&mut session, A::AppendGroup(b'7'));
    action(&mut session, A::AppendGroup(b'9'));
    assert_eq!(session.grid_helpcode_snapshot().choices.len(), 16);
    let before = session.grid_helpcode_snapshot();
    assert!(!session.set_helpcode_schema("not-a-schema"));
    assert_eq!(session.grid_helpcode_snapshot(), before);
    session.set_helpcode_table(Arc::new(crate::helpcode::HelpcodeKeymap::default()));
    assert_eq!(
        session.grid_helpcode_snapshot().availability,
        Available::MissingTable
    );
    assert!(!action(&mut session, A::Begin).handled);
    assert_eq!(words(&session)[..2], ["你", "拟"]);
    enable(&mut session);
    action(&mut session, A::Begin);
    action(&mut session, A::AppendExact(b'b'));
    session.set_key_grid(Some(KeyGrid::FourteenKey));
    assert!(session.grid_helpcode_snapshot().slots.is_empty());
}

#[test]
fn grid_helpcode_filters_before_candidate_cap_and_restores_ordinary_pool() {
    let mut sql = String::from("CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);");
    for index in 0..150 {
        let word = char::from_u32(0x4e00 + index).unwrap();
        sql.push_str(&format!(
            "INSERT INTO tbl_1_n VALUES('ni','n','{word}',{});",
            1000 - index
        ));
    }
    let target = char::from_u32(0x4e00 + 149).unwrap().to_string();
    let fixture = Fixture::new(&sql);
    let mut session = start(&fixture, KeyGrid::NineKey);
    session.set_helpcode_table(Arc::new(
        crate::helpcode::HelpcodeKeymap::from_table_bytes(format!("{target}=ab").as_bytes())
            .unwrap(),
    ));
    session.set_grid_helpcode_enabled(true);
    type_grid(&mut session, KeyGrid::NineKey, "64");
    lock_ni(&mut session);
    let ordinary = session.snapshot();
    assert_eq!(ordinary.candidates.len(), 128);
    assert!(!words(&session).contains(&target));
    action(&mut session, A::Begin);
    action(&mut session, A::AppendExact(b'a'));
    assert_eq!(words(&session)[0], target);
    action(&mut session, A::AppendExact(b'b'));
    assert_eq!(words(&session), [target]);
    action(&mut session, A::Clear);
    assert_eq!(session.snapshot(), ordinary);
}

#[test]
fn grid_helpcode_strokes_are_mutually_exclusive_and_single_character_survives() {
    let fixture = Fixture::new(SQL);
    let stroke = stroke_dictionary(fixture.path());
    let mut session = fixture.session_with(|options| {
        options.learning = false;
        options.stroke_dictionary = stroke;
    });
    session.set_key_grid(Some(KeyGrid::NineKey));
    enable(&mut session);
    type_grid(&mut session, KeyGrid::NineKey, "64");
    assert!(session.set_nine_key_filter(true, "h").handled);
    assert_eq!(session.snapshot().nine_key_strokes, "h");
    action(&mut session, A::Begin);
    assert!(session.snapshot().nine_key_strokes.is_empty());
    assert!(session.snapshot().nine_key_single_character);
    assert!(!words(&session).is_empty());
    action(&mut session, A::AppendExact(b'b'));
    let before = session.grid_helpcode_snapshot();
    assert!(!session.set_nine_key_filter(false, "!").handled);
    assert_eq!(session.grid_helpcode_snapshot(), before);
    session.set_nine_key_filter(true, "h");
    assert!(session.grid_helpcode_snapshot().slots.is_empty());
    assert!(!session.grid_helpcode_snapshot().editing);
    action(&mut session, A::Begin);
    action(&mut session, A::End);
    assert!(session.snapshot().nine_key_strokes.is_empty());
}

#[test]
fn grid_helpcode_full_keyboard_and_strict_preference_are_independent() {
    let fixture = Fixture::new(SQL);
    let mut ordinary = fixture.session_with(|options| options.learning = false);
    let mut enabled = fixture.session_with(|options| options.learning = false);
    enable(&mut ordinary);
    ordinary.set_grid_helpcode_enabled(false);
    enable(&mut enabled);
    for letter in b"niA" {
        assert_eq!(
            ordinary.character(*letter, false),
            enabled.character(*letter, false)
        );
        assert_eq!(ordinary.snapshot(), enabled.snapshot());
        assert!(!action(&mut enabled, A::Begin).handled);
    }
    let mut grid = fixture.session_with(|options| {
        options.learning = false;
        options.strict_quanpin_helpcode = true;
    });
    grid.set_key_grid(Some(KeyGrid::NineKey));
    enable(&mut grid);
    type_grid(&mut grid, KeyGrid::NineKey, "64");
    action(&mut grid, A::Begin);
    action(&mut grid, A::AppendExact(b'z'));
    assert_eq!(words(&grid)[..2], ["你", "拟"], "网格一码保留不匹配候选");
    let before = grid.grid_helpcode_snapshot();
    grid.set_grid_helpcode_enabled(true);
    assert_eq!(
        grid.grid_helpcode_snapshot(),
        before,
        "重复应用开关不丢编辑状态"
    );
    grid.set_helpcode_enabled(false);
    assert!(grid.grid_helpcode_snapshot().slots.is_empty());
}
