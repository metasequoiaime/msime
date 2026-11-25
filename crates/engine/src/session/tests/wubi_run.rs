//! 五笔连续片段累积窗（五笔按输入习惯自动造词）的会话级测试：段的拼接、关闭点、上限、学习开关与墓碑、计数行的备份可存性、对称删除后的候选刷新和置顶落库。

use super::*;

/// 六个单字加两个二字词的 86/98 双表。词组码按各表单字全码推导（86 甲 aaaa…己 ffff、98 甲 ffff…己 kkkk）：
/// 甲乙甲乙 → 86 `abab` / 98 `fgfg`，丙丁丙丁 → 86 `cdcd` / 98 `hihi`，甲乙丙丁 → 86 `abcd` / 98 `fghi`，
/// 12 字段 甲乙丙丁甲乙丙丁甲乙丙丁 → 86 `abcd` / 98 `fghi`（取前三字与末字的首码）。
const WUBI_RUN_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));\
INSERT INTO wubi86 VALUES('aaaa','甲',100),('bbbb','乙',90),('cccc','丙',80),('dddd','丁',70),('eeee','戊',60),('ffff','己',50),('abab','甲乙',95),('cdcd','丙丁',85);\
CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));\
INSERT INTO wubi98 VALUES('ffff','甲',100),('gggg','乙',90),('hhhh','丙',80),('iiii','丁',70),('jjjj','戊',60),('kkkk','己',50),('fgfg','甲乙',90),('hihi','丙丁',85);";

fn pick_wubi(session: &mut Session, code: &str, word: &str) {
    type_text(session, code);
    assert_eq!(select_word(session, word).commit.as_deref(), Some(word));
}

/// 一段 = 连续 picks（间隔 ≤10s）；推进 15s 后的 closer 上屏把这段关闭（按整段计数），closer 自己的
/// 片段开新段，再推进 15s 让下一轮的第一个上屏把只有一个片段的新段关掉（不计）。
fn close_wubi_run_round(
    session: &mut Session,
    clock: &TestClock,
    picks: &[(&str, &str)],
    closer: (&str, &str),
) {
    for (code, word) in picks {
        pick_wubi(session, code, word);
    }
    clock.advance(Duration::from_secs(15));
    pick_wubi(session, closer.0, closer.1);
    clock.advance(Duration::from_secs(15));
}

/// 五笔连续打出同一个二字词两次为一段，段关闭时按整段全文计数；凑满 3 次（默认 `thrice`）经
/// `wubi_learning::learn` 落库，86/98 各自按本表单字全码推导，日志行 `user_inserted=1`。
#[test]
fn wubi_run_learns_a_two_piece_phrase_after_three_closes() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    let clock = TestClock::install(&mut session);
    let round = |session: &mut Session, clock: &TestClock| {
        close_wubi_run_round(
            session,
            clock,
            &[("abab", "甲乙"), ("abab", "甲乙")],
            ("abab", "甲乙"),
        );
    };
    for _ in 0..2 {
        round(&mut session, &clock);
    }
    // 两段关闭后计数到 2，词还没落库。
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count FROM pick_transitions WHERE previous_key='wubi-run' AND previous_value='wubi-run' AND key='甲乙甲乙' AND value='甲乙甲乙'"
        ),
        2
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
        ),
        0
    );
    // 第三段关闭时凑满 3 次，落库。
    round(&mut session, &clock);
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi86 WHERE value='甲乙甲乙' AND key='abab'"
        ),
        9
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi98 WHERE value='甲乙甲乙' AND key='fgfg'"
        ),
        9
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi' AND value='甲乙甲乙' AND operation='upsert' AND user_inserted=1"
        ),
        1
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi98' AND value='甲乙甲乙' AND operation='upsert' AND user_inserted=1"
        ),
        1
    );
    // 学成即清计数（与拼音凑对同一时机）。
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run'"
        ),
        0
    );
    // 新会话能打出学到的词。
    let mut next = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    type_text(&mut next, "abab");
    assert!(words(&next).contains(&"甲乙甲乙".to_owned()));
}

/// 单字片段连续拼出的 ≤4 字段按整段全文计数：凑满后整段成词，真前缀永不记成词。
#[test]
fn wubi_run_learns_char_by_char_run_without_prefixes() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    let clock = TestClock::install(&mut session);
    let singles = [
        ("aaaa", "甲"),
        ("bbbb", "乙"),
        ("cccc", "丙"),
        ("dddd", "丁"),
    ];
    for _ in 0..2 {
        close_wubi_run_round(&mut session, &clock, &singles, ("aaaa", "甲"));
    }
    // 两段关闭：计数列到 2，且只有整段全文一行，没有真前缀（「甲乙」「甲乙丙」从不出现）。
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙丙丁'"
        ),
        2
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run' AND (key<>'甲乙丙丁' OR value<>'甲乙丙丁')"
        ),
        0
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value IN ('甲乙丙', '甲乙丙丁')"
        ),
        0
    );
    close_wubi_run_round(&mut session, &clock, &singles, ("aaaa", "甲"));
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi86 WHERE value='甲乙丙丁' AND key='abcd'"
        ),
        9
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi98 WHERE value='甲乙丙丁' AND key='fghi'"
        ),
        9
    );
}

/// 整段 >4 字时任一片段是单字就不学：二字词加单字拼出的 5 字段、五个单字拼出的 5 字段都不计数。
#[test]
fn wubi_run_rejects_single_char_material_over_four_chars() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    // 段 = 甲乙 + 甲 + 乙 + 丙 → 甲乙甲乙丙（5 字，含单字片段）。
    {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Wubi;
        });
        let clock = TestClock::install(&mut session);
        close_wubi_run_round(
            &mut session,
            &clock,
            &[
                ("abab", "甲乙"),
                ("aaaa", "甲"),
                ("bbbb", "乙"),
                ("cccc", "丙"),
            ],
            ("dddd", "丁"),
        );
    }
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run'"
        ),
        0
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙丙'"
        ),
        0
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi98 WHERE value='甲乙甲乙丙'"
        ),
        0
    );
    // 段 = 五个单字 → 甲乙丙丁戊（5 字，全是单字片段）。
    {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Wubi;
        });
        let clock = TestClock::install(&mut session);
        close_wubi_run_round(
            &mut session,
            &clock,
            &[
                ("aaaa", "甲"),
                ("bbbb", "乙"),
                ("cccc", "丙"),
                ("dddd", "丁"),
                ("eeee", "戊"),
            ],
            ("ffff", "己"),
        );
    }
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run'"
        ),
        0
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙丙丁戊'"
        ),
        0
    );
}

/// 间隔 >10s 把段切开：两轮各自按 甲乙甲乙 计数，不合成 8 字的段；超过 12 字的段不计数。
#[test]
fn wubi_run_splits_on_gap_and_caps_length() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    let clock = TestClock::install(&mut session);
    for _ in 0..2 {
        close_wubi_run_round(
            &mut session,
            &clock,
            &[("abab", "甲乙"), ("abab", "甲乙")],
            ("abab", "甲乙"),
        );
    }
    // 两段各自计数（计数 2），没有 8 字的段。
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙甲乙'"
        ),
        2
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙甲乙甲乙甲乙'"
        ),
        0
    );
    // 第三段凑满落库，证明确实是「段」在计数。
    close_wubi_run_round(
        &mut session,
        &clock,
        &[("abab", "甲乙"), ("abab", "甲乙")],
        ("abab", "甲乙"),
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
        ),
        1
    );
    drop(session);

    // 12 字段（6 个二字词片段）×3 凑满落库；14 字段（7 个片段）超上限不计数。
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
    });
    let clock = TestClock::install(&mut session);
    let pairs = [
        ("abab", "甲乙"),
        ("cdcd", "丙丁"),
        ("abab", "甲乙"),
        ("cdcd", "丙丁"),
        ("abab", "甲乙"),
        ("cdcd", "丙丁"),
    ];
    for _ in 0..3 {
        close_wubi_run_round(&mut session, &clock, &pairs, ("abab", "甲乙"));
    }
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙丙丁甲乙丙丁甲乙丙丁' AND key='abcd'"
        ),
        1
    );
    drop(session);

    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
    });
    let clock = TestClock::install(&mut session);
    let seven = [
        ("abab", "甲乙"),
        ("cdcd", "丙丁"),
        ("abab", "甲乙"),
        ("cdcd", "丙丁"),
        ("abab", "甲乙"),
        ("cdcd", "丙丁"),
        ("abab", "甲乙"),
    ];
    close_wubi_run_round(&mut session, &clock, &seven, ("abab", "甲乙"));
    // 第 7 个片段超出 12 字上限，前 6 个片段（12 字）先关闭记一次计数；超长段不整段计数。固定三次阈值下这次计数不落库。
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙丙丁甲乙丙丁甲乙丙丁'"
        ),
        1
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙丙丁甲乙丙丁甲乙丙丁甲乙'"
        ),
        0
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙丙丁甲乙丙丁甲乙丙丁甲乙'"
        ),
        0
    );
}

/// 学习关着时不记片段也不落库；86 里删过的词不因同样的习惯复活，98 照常学。
#[test]
fn wubi_run_respects_learning_disabled_and_tombstones() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Wubi;
            options.learning = false;
        });
        let clock = TestClock::install(&mut session);
        for _ in 0..3 {
            close_wubi_run_round(
                &mut session,
                &clock,
                &[("abab", "甲乙"), ("abab", "甲乙")],
                ("abab", "甲乙"),
            );
        }
    }
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run'"
        ),
        0
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
        ),
        0
    );

    // 86 里删过 甲乙甲乙：再凑满也不复活；98 没删过，照常落库。
    crate::user_dictionary::journal::open_journal(&fixture.journal())
        .unwrap()
        .execute(
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES('wubi','abab','甲乙甲乙','delete',0,'',1)",
            [],
        )
        .unwrap();
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
    });
    let clock = TestClock::install(&mut session);
    for _ in 0..3 {
        close_wubi_run_round(
            &mut session,
            &clock,
            &[("abab", "甲乙"), ("abab", "甲乙")],
            ("abab", "甲乙"),
        );
    }
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
        ),
        0
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM wubi98 WHERE value='甲乙甲乙'"
        ),
        1
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi' AND value='甲乙甲乙' AND operation='delete'"
        ),
        1
    );
}

/// 凑三次模式：flush 关闭段时照常记一次计数、不满阈值不落库；三次分段（每次会话一段、各自
/// flush 收尾）攒满即落库。
#[test]
fn wubi_run_thrice_mode_counts_partial_runs_closed_by_flush() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    for round in 1..=3 {
        let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
        pick_wubi(&mut session, "abab", "甲乙");
        pick_wubi(&mut session, "abab", "甲乙");
        session.finish(0);
        // 前两轮只记数不落库；第三轮学成即清计数。
        let expected = if round == 3 { 0 } else { round };
        assert_eq!(
            count(
                &fixture.journal(),
                "SELECT count FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
        let expected = if round == 3 { 1 } else { 0 };
        assert_eq!(
            count(
                &fixture.main_db(),
                "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
    }
}

/// 凑三次模式：回车原文上屏（`CommitRaw`）关闭打开的累积段记一次计数，三次攒满即落库，不是静默丢弃。
#[test]
fn wubi_run_thrice_mode_counts_partial_runs_closed_by_commit_raw() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    for round in 1..=3 {
        let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
        pick_wubi(&mut session, "abab", "甲乙");
        pick_wubi(&mut session, "abab", "甲乙");
        session.command(Command::CommitRaw);
        let expected = if round == 3 { 0 } else { round };
        assert_eq!(
            count(
                &fixture.journal(),
                "SELECT count FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
        let expected = if round == 3 { 1 } else { 0 };
        assert_eq!(
            count(
                &fixture.main_db(),
                "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
    }
}

/// 凑三次模式：会话丢弃（换窗口、设置变更重建、宿主停用）时关闭打开的累积段记一次计数，
/// 三次攒满即落库，段不跟着会话一起无声消失。
#[test]
fn wubi_run_thrice_mode_counts_partial_runs_closed_by_drop() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    for round in 1..=3 {
        {
            let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
            pick_wubi(&mut session, "abab", "甲乙");
            pick_wubi(&mut session, "abab", "甲乙");
        }
        let expected = if round == 3 { 0 } else { round };
        assert_eq!(
            count(
                &fixture.journal(),
                "SELECT count FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
        let expected = if round == 3 { 1 } else { 0 };
        assert_eq!(
            count(
                &fixture.main_db(),
                "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
    }
}

/// 打开的累积段后面接一次不构成片段的五笔上屏（这里是非汉字的五笔行）：段照样收尾计数，三次攒满即落库，不随这次上屏无声丢掉。
#[test]
fn wubi_run_counts_runs_closed_by_a_non_material_commit() {
    let fixture = Fixture::new(&format!(
        "{WUBI_RUN_FIXTURE}INSERT INTO wubi86 VALUES('xyxy','OK',100);"
    ));
    for round in 1..=3 {
        let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
        pick_wubi(&mut session, "abab", "甲乙");
        pick_wubi(&mut session, "abab", "甲乙");
        pick_wubi(&mut session, "xyxy", "OK");
        let expected = if round == 3 { 0 } else { round };
        assert_eq!(
            count(
                &fixture.journal(),
                "SELECT coalesce(max(count),0) FROM pick_transitions WHERE previous_key='wubi-run' AND key='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
        let expected = if round == 3 { 1 } else { 0 };
        assert_eq!(
            count(
                &fixture.main_db(),
                "SELECT count(*) FROM wubi86 WHERE value='甲乙甲乙'"
            ),
            expected,
            "round {round}"
        );
    }
}

/// 组字为空时按退格或向后删除，是在删刚上屏的字：打开的累积段直接丢弃，不计数。
#[test]
fn wubi_run_is_discarded_by_deleting_the_committed_text() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    for command in [Command::Backspace, Command::DeleteForward] {
        let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
        pick_wubi(&mut session, "abab", "甲乙");
        pick_wubi(&mut session, "abab", "甲乙");
        session.command(command);
        drop(session);
        assert_eq!(
            count(
                &fixture.journal(),
                "SELECT count(*) FROM pick_transitions WHERE previous_key='wubi-run'"
            ),
            0,
            "{command:?}"
        );
    }
}

/// 累积段的计数行要能进本地备份：导出端用 `habit_is_storable` 跳过存不下的行，`previous_value` 留空就会被整行跳过。
#[test]
fn wubi_run_counts_are_storable_learning_habits() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    pick_wubi(&mut session, "abab", "甲乙");
    pick_wubi(&mut session, "abab", "甲乙");
    session.finish(0);
    let mut runs = 0;
    crate::user_dictionary::habits::stream_learning_habits(&fixture.paths(), &mut |habit| {
        if let crate::user_dictionary::habits::LearningHabit::PickTransition { key, .. } = habit {
            if key == "甲乙甲乙" {
                assert!(crate::user_dictionary::habits::habit_is_storable(habit));
                runs += 1;
            }
        }
        true
    })
    .unwrap();
    assert_eq!(runs, 1);
}

/// 对称删除另两版失败时，当前版已经删掉并提交：报 `REMOVAL_NOT_PERSISTED`，但候选要照样刷新，删掉的词不能还留在列表里。
#[test]
fn wubi_removal_refreshes_candidates_when_other_versions_fail() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    let clock = TestClock::install(&mut session);
    for _ in 0..3 {
        close_wubi_run_round(
            &mut session,
            &clock,
            &[("abab", "甲乙"), ("abab", "甲乙")],
            ("abab", "甲乙"),
        );
    }
    session.command(Command::Cancel);
    Connection::open(fixture.journal()).unwrap().execute_batch("CREATE TRIGGER reject_other_version BEFORE INSERT ON user_dictionary_operations WHEN NEW.dictionary='wubi98' AND NEW.operation='delete' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    type_text(&mut session, "abab");
    let index = index_of(&session, "甲乙甲乙");
    let result = session.remove(index);
    assert_eq!(
        result.diagnostic.as_deref(),
        Some(crate::diagnostics::REMOVAL_NOT_PERSISTED)
    );
    assert!(!words(&session).contains(&"甲乙甲乙".to_owned()));
}

/// 置顶模式：学入词在各表的权重是同码已有行最大权重 + 1（86/98 各表独立算）；沉底默认仍是 ≤9 的低权重。
#[test]
fn wubi_run_top_placement_outranks_same_code_rows() {
    let fixture = Fixture::new(WUBI_RUN_FIXTURE);
    // 置顶：甲乙甲乙 的码 86 `abab`（同码已有 甲乙 95）→ 96；98 `fgfg`（同码已有 甲乙 90）→ 91。
    {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Wubi;
            options.wubi.wubi_phrase_placement = crate::types::WubiPhrasePlacement::Top;
        });
        let clock = TestClock::install(&mut session);
        for _ in 0..3 {
            close_wubi_run_round(
                &mut session,
                &clock,
                &[("abab", "甲乙"), ("abab", "甲乙")],
                ("abab", "甲乙"),
            );
        }
    }
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi86 WHERE value='甲乙甲乙' AND key='abab'"
        ),
        96
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi98 WHERE value='甲乙甲乙' AND key='fgfg'"
        ),
        91
    );
    // 沉底（默认时机下同段计数 1 次不够，用一次模式直接落库）：丙丁丙丁 的码 86 `cdcd`（同码已有 丙丁 85）→ 84 收到 9；98 `hihi`（同码已有 丙丁 85）→ 9。
    {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Wubi;
        });
        let clock = TestClock::install(&mut session);
        for _ in 0..3 {
            close_wubi_run_round(
                &mut session,
                &clock,
                &[("cdcd", "丙丁"), ("cdcd", "丙丁")],
                ("cdcd", "丙丁"),
            );
        }
    }
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi86 WHERE value='丙丁丙丁' AND key='cdcd'"
        ),
        9
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM wubi98 WHERE value='丙丁丙丁' AND key='hihi'"
        ),
        9
    );
}
