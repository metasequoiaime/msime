//! 网格辅筛实验前的普通输入基线：只调用现有会话入口，不引入辅码状态。

use super::*;

const SQL: &str = "
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_n VALUES('ni','n','你',9000),('ni','n','拟',8000);
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_h VALUES('hao','h','好',7000);
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200),('ni''hao','nh','拟好',100);
";

// 输入串独立固定，不使用生产编码函数生成预期，避免编码变化与断言同时漂移。
fn layouts() -> Vec<(KeyGrid, &'static str, &'static str)> {
    use crate::nine_key_shuangpin::{
        resolve, resolve_eighteen_key_shuangpin, resolve_fourteen_key_shuangpin,
    };
    vec![
        (KeyGrid::NineKey, "64", "426"),
        (KeyGrid::FourteenKey, "bu", "gao"),
        (KeyGrid::EighteenKey, "bi", "hai"),
        (
            KeyGrid::NineKeyShuangpin(
                resolve(crate::types::NineKeyShuangpinKind::Migala3, None).unwrap(),
            ),
            "64",
            "47",
        ),
        (
            KeyGrid::FourteenKeyShuangpin(
                resolve_fourteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None).unwrap(),
            ),
            "bu",
            "gc",
        ),
        (
            KeyGrid::EighteenKeyShuangpin(
                resolve_eighteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None).unwrap(),
            ),
            "bi",
            "hx",
        ),
    ]
}

fn start(fixture: &Fixture, grid: KeyGrid) -> Session {
    let mut session = fixture.session_with(|options| {
        options.learning = false;
        options.english.mixed_candidates = false;
        if matches!(
            grid,
            KeyGrid::FourteenKeyShuangpin(_) | KeyGrid::EighteenKeyShuangpin(_)
        ) {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        }
    });
    session.set_key_grid(Some(grid));
    session
}

fn type_grid(session: &mut Session, grid: KeyGrid, keys: &str) {
    for key in keys.bytes() {
        let result = if grid.uses_digits() {
            session.character(key, false)
        } else {
            session.grid_key(key)
        };
        assert!(
            result.handled && result.commit.is_none(),
            "{grid:?}: {keys}"
        );
    }
}

fn lock_ni(session: &mut Session) {
    let index = session
        .snapshot()
        .nine_key_spellings
        .iter()
        .position(|s| s == "ni")
        .unwrap();
    assert!(session.choose_nine_key_spelling(index).handled);
}

#[test]
fn grid_baseline_coverage_order_lock_undo_and_cancel() {
    for (grid, first, tail) in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = start(&fixture, grid);
        type_grid(&mut session, grid, first);
        let before_lock = session.snapshot();
        // 各布局短码补全范围不同，这里固定共同的前两项；完整输入另断言全列表。
        assert_eq!(&words(&session)[..2], ["你", "拟"], "{grid:?}");
        assert_eq!(before_lock.nine_key_reading, "ni");
        lock_ni(&mut session);
        assert_eq!(session.snapshot().preedit, "ni");
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(
            session.snapshot(),
            before_lock,
            "撤销锁定恢复原快照：{grid:?}"
        );
        lock_ni(&mut session);
        type_grid(&mut session, grid, tail);
        let full = session.snapshot();
        assert_eq!(full.preedit, format!("ni'{tail}"));
        assert_eq!(full.nine_key_reading, "ni'hao");
        // 单字频率远高于词语，仍然先排列覆盖全部输入的两个词。
        assert_eq!(words(&session), ["你好", "拟好", "你", "拟"], "{grid:?}");
        assert!(session.command(Command::Cancel).handled);
        assert!(session.snapshot().editing_text.is_empty());
        type_grid(&mut session, grid, &format!("{first}{tail}"));
        assert_eq!(words(&session), ["你好", "拟好", "你", "拟"], "{grid:?}");
        assert_eq!(
            select_word(&mut session, "你好").commit.as_deref(),
            Some("你好")
        );
        assert!(session.snapshot().editing_text.is_empty());
    }
}

#[test]
fn grid_baseline_filter_restore_and_partial_commit() {
    for (grid, first, tail) in layouts() {
        let fixture = Fixture::new(SQL);
        let strokes = fixture.path().join("baseline-stroke.db");
        crate::stroke::fixture::build(&strokes);
        Connection::open(&strokes)
            .unwrap()
            .execute_batch("INSERT INTO entries VALUES('pspzspn','你',100),('hsh','拟',90);")
            .unwrap();
        let mut session = start(&fixture, grid);
        session.nine_key.set_stroke_dictionary(strokes);
        assert!(!session.set_nine_key_filter(true, "").handled);
        type_grid(&mut session, grid, &format!("{first}{tail}"));
        let original = session.snapshot();
        assert!(session.set_nine_key_filter(false, "ps").handled);
        assert_eq!(words(&session), ["你好", "你"], "{grid:?}");
        let filtered = session.snapshot();
        assert!(!session.set_nine_key_filter(false, "q").handled);
        assert_eq!(session.snapshot(), filtered);
        assert!(session.set_nine_key_filter(false, "").handled);
        assert_eq!(
            session.snapshot(),
            original,
            "清笔画恢复完整候选和读音：{grid:?}"
        );
        assert!(session.set_nine_key_filter(true, "").handled);
        assert_eq!(words(&session), ["你", "拟"]);
        assert_eq!(
            select_word(&mut session, "你").commit.as_deref(),
            Some("你")
        );
        let remaining = session.snapshot();
        assert_eq!(remaining.editing_text, tail, "{grid:?}");
        assert_eq!(remaining.nine_key_reading, "hao");
        assert!(remaining.nine_key_single_character);
        assert_eq!(words(&session), ["好"]);
        assert_eq!(
            select_word(&mut session, "好").commit.as_deref(),
            Some("好")
        );
        assert!(!session.snapshot().nine_key_single_character);
        type_grid(&mut session, grid, &format!("{first}{tail}"));
        assert_eq!(session.snapshot(), original, "下一段不继承筛选：{grid:?}");
    }
}

#[test]
fn grid_baseline_separator_backspace_and_invalid_selection() {
    for (grid, first, tail) in layouts() {
        let fixture = Fixture::new(SQL);
        let mut session = start(&fixture, grid);
        type_grid(&mut session, grid, first);
        let before = session.snapshot();
        assert!(!session.choose_nine_key_spelling(usize::MAX).handled);
        assert!(!session.character(b'|', false).handled);
        assert_eq!(session.snapshot(), before);
        assert!(session.character(b'\'', false).handled);
        let split = session.snapshot();
        assert_eq!(split.preedit, format!("{first}'"));
        assert!(session.character(b'\'', false).handled);
        assert_eq!(session.snapshot(), split);
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot(), before);
        assert!(session.character(b'\'', false).handled);
        type_grid(&mut session, grid, tail);
        assert_eq!(session.snapshot().preedit, format!("{first}'{tail}"));
        assert_eq!(session.snapshot().nine_key_reading, "ni'hao");
        assert_eq!(
            select_word(&mut session, "你").commit.as_deref(),
            Some("你")
        );
        assert_eq!(session.snapshot().editing_text, tail);
        assert_eq!(words(&session), ["好"]);
    }
}
