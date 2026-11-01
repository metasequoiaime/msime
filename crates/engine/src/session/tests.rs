//! Session-level tests: the caret-prefix rules of upstream bc46f27, wubi mixed routing (overlays.md §3.3, no C++ oracle), the personal-learning windows through `set_clock`, and ports of the `test_input_session.cpp`, `test_temporary_input_session.cpp` and `test_runtime_isolation.cpp` cases the goldens do not pin down. Every fixture is built inline, as the reference ctests did.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::Connection;

use super::editing::quanpin_raw_boundaries;
use super::{Clock, Session, SessionOptions};
use crate::assets;
use crate::local::date_time::LocalDateTime;
use crate::paths::RuntimePaths;
use crate::types::{
    CandidateEdge, CandidateSource, Command, FrequencyAdjustmentMode, FrequencyAdjustmentOptions,
    LocalInputMode, SchemeKey, SchemeSet, SchemeType, ShuangpinProfileKind,
    DEFAULT_FIRST_PAGE_ROWS,
};
use crate::user_dictionary::ngram_store::flush_all;
use crate::KeyGrid;

/// test_input_session.cpp:390-430 (fixture M), the rows the portable-selection, caret and edge cases read.
const QUANPIN_FIXTURE: &str =
    "CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 200);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '拟好', 100);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '𠀀方案𠮷', 90);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', 'C语言 2', 80);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', 'GitHub', 70);\
CREATE TABLE tbl_2_b(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_2_b VALUES('bu''hao', 'bh', '不好', 200);\
INSERT INTO tbl_2_b VALUES('bu''hao', 'bh', '补好', 100);\
CREATE TABLE tbl_1_x(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_x VALUES('xi', 'x', '西', 100);\
CREATE TABLE tbl_2_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_t VALUES('te''le','tl','特乐',100);\
CREATE TABLE tbl_3_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_t VALUES('te','t','特',100);\
CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_l VALUES('le','l','乐',100);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',100);\
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100);\
CREATE TABLE tbl_4_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);";

/// Upstream bc46f27 `run_caret_prefix_session_tests`.
const CARET_PREFIX_FIXTURE: &str =
    "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100);\
INSERT INTO tbl_1_n VALUES('ni','n','拟',90);\
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','拟好',100);\
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shi','sh','是',100);\
CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_j VALUES('jie','j','接',100);\
CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('aaaa','工',100);\
INSERT INTO wubi86 VALUES('aaaa','或',50);";

/// A wubi code that is also two quanpin syllables, so one mixed list holds both producers: 工 from the wubi table, 哥哥 and 个 from quanpin.
const WUBI_ROUTING_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('gege','工',100);\
CREATE TABLE tbl_2_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_g VALUES('ge''ge','gg','哥哥',1000);\
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_g VALUES('ge','g','个',500);\
INSERT INTO tbl_1_g VALUES('ge','g','各',400);";

const WUBI_BIANYI_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('bian','便',100);\
CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_b VALUES('bian','b','边',500);\
CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_b VALUES('bian''yi','by','编译',1000);";

const WUBI_XUQIU_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('xuq','绞',10);\
INSERT INTO wubi86 VALUES('xuqi','纯音乐',9);\
CREATE TABLE tbl_2_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_x VALUES('xu''qi','xq','续期',16735);\
INSERT INTO tbl_2_x VALUES('xu''qiu','xq','需求',519652);";

const WUBI_HANGUO_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('hang','虛',10);\
CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_h VALUES('han''guo','hg','韩国',500496);";

const WUBI_EDU_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('edui','膫',10);\
INSERT INTO wubi86 VALUES('eduq','须将',9);\
CREATE TABLE tbl_2_e(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_e VALUES('e''du','ed','额度',103485);";

/// test_personal_context_input_session.cpp:136-149 with test_pick_pair_input_session.cpp:73-94's shan/shui rows.
const CONTEXT_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','甲',100),('ni','n','乙',90),('ni','n','丙',80);\
CREATE TABLE tbl_1_h(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','子',100),('hao','h','丑',90),('hao','h','寅',80);\
CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);\
CREATE TABLE tbl_1_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shan','s','闪',100),('shan','s','山',90),('shui','s','睡',100),('shui','s','水',90);\
CREATE TABLE tbl_2_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);";

const ENGLISH_FIXTURE: &str = "INSERT INTO english_words VALUES('he','HE',110);\
INSERT INTO english_words VALUES('hello','Hello',100);\
INSERT INTO english_words VALUES('help','Help',90);";

#[test]
fn mixed_candidate_refresh_reuses_rows_for_engine_and_caret_prefix() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihc");
    let word_pointer = session.input.mixed_candidates[0].word.as_ptr();
    let before = session.snapshot();
    session.input.update_mixed_candidates();
    assert_eq!(
        session.input.mixed_candidates[0].word.as_ptr(),
        word_pointer
    );
    assert_eq!(session.snapshot(), before);

    session.set_caret(Some(2));
    assert!(session.input.prefix_active);
    let word_pointer = session.input.mixed_candidates[0].word.as_ptr();
    let before = session.snapshot();
    session.input.update_mixed_candidates();
    assert_eq!(
        session.input.mixed_candidates[0].word.as_ptr(),
        word_pointer
    );
    assert_eq!(session.snapshot(), before);
}

#[test]
fn journal_path_lookup_reuses_the_session_path() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let session = fixture.session();

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        assert_eq!(session.input.journal_path(), session.input.journal_path());
    });

    assert_eq!(
        allocations, 0,
        "journal path lookup allocated {allocations} buffers"
    );
}

#[test]
fn ordinary_candidate_positions_skip_missing_journal_work() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let mut candidates = session.input.engine.candidates().to_vec();

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.apply_candidate_positions(&mut candidates);
    });

    assert_eq!(
        allocations, 0,
        "ordinary position refresh allocated {allocations} buffers"
    );
}

#[test]
fn personal_context_keeps_only_the_matching_learned_leader() {
    // 日志必须准确对应当前首选；其它读音、来源、权重或删除记录不能锁住它。
    for (dictionary, key, value, operation, weight, enabled, expected) in [
        ("pinyin", "ni", "甲", "upsert", 100, true, "甲"),
        ("pinyin", "ni", "甲", "upsert", 100, false, "丙"),
        ("pinyin", "li", "甲", "upsert", 100, true, "丙"),
        ("pinyin", "ni", "乙", "upsert", 100, true, "丙"),
        ("pinyin", "ni", "甲", "upsert", 99, true, "丙"),
        ("pinyin", "ni", "甲", "delete", 100, true, "丙"),
        ("english", "ni", "甲", "upsert", 100, true, "丙"),
    ] {
        let fixture = Fixture::new(CONTEXT_FIXTURE);
        let journal = crate::user_dictionary::journal::open_journal(&fixture.journal()).unwrap();
        journal
            .execute(
                "INSERT INTO personal_bigram(previous,word,count) VALUES('子','丙',32)",
                [],
            )
            .unwrap();
        journal.execute(
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight) VALUES(?1,?2,?3,?4,?5)",
            rusqlite::params![dictionary, key, value, operation, weight],
        ).unwrap();
        drop(journal);
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Quanpin;
            if enabled {
                options.frequency.mode = FrequencyAdjustmentMode::Promote;
            }
        });
        type_text(&mut session, "hao");
        select_word(&mut session, "子");
        type_text(&mut session, "ni");
        assert_eq!(
            words(&session)[0],
            expected,
            "{dictionary}/{key}/{value}/{operation}/{weight}, enabled={enabled}"
        );
        if !enabled {
            let result = session.pin(index_of(&session, "甲"));
            assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
            assert_eq!(words(&session)[0], "甲", "关闭调频仍须尊重手动置顶");
        }
        if expected == "甲" {
            assert_eq!(
                words(&session),
                ["甲", "丙", "乙"],
                "保留首选后其它候选仍按上下文排序"
            );
            // 前缀管线也走同一重排入口；只有拼音字典行可以复用拼音权重日志。
            type_text(&mut session, "hao");
            session.set_caret(Some(2));
            assert!(session.input.prefix_active);
            assert_eq!(session.input.prefix_query_input, "ni");
            let original = session.input.prefix_candidates.clone();
            for (source, scheme, leader) in [
                (CandidateSource::UserDatabase, SchemeType::Quanpin, "甲"),
                (CandidateSource::Database, SchemeType::Wubi, "丙"),
                (CandidateSource::Generated, SchemeType::Quanpin, "甲"),
            ] {
                session.input.prefix_active = true;
                session.input.prefix_candidates = original.clone();
                session.input.prefix_candidates[0].source = source;
                session.input.prefix_candidates[0].scheme = scheme;
                session.input.update_mixed_candidates();
                assert!(session.input.prefix_active);
                let injected = session
                    .input
                    .mixed_candidates
                    .iter()
                    .find(|row| row.word == "甲")
                    .unwrap();
                assert_eq!(injected.source, source);
                assert_eq!(injected.scheme, scheme);
                assert_eq!(
                    session.input.mixed_candidates[0].word, leader,
                    "{source:?}/{scheme:?}"
                );
            }
        }
    }
}

#[test]
fn personal_rerank_refresh_reuses_shown_rows_and_preserves_learning_order() {
    let fixture = Fixture::new(CONTEXT_FIXTURE);
    let mut session = fixture.session();
    for _ in 0..4 {
        type_text(&mut session, "hao");
        select_word(&mut session, "子");
        type_text(&mut session, "ni");
        select_word(&mut session, "乙");
        session.punctuation(b',');
    }
    type_text(&mut session, "hao");
    select_word(&mut session, "子");
    type_text(&mut session, "ni");

    for prefix in [false, true] {
        if prefix {
            type_text(&mut session, "hao");
            session.set_caret(Some(2));
        }
        assert!(session.input.personal_reranked);
        assert_eq!(session.input.mixed_candidates[0].word, "乙");
        let before = session.snapshot();
        let ranking = session.input.ranking_list().to_vec();
        let mut string_pointers: Vec<_> = session
            .input
            .mixed_candidates
            .iter()
            .map(|row| row.word.as_ptr())
            .collect();
        string_pointers.sort_unstable();

        session.input.update_mixed_candidates();

        assert_eq!(session.snapshot(), before);
        assert_eq!(session.input.ranking_list(), ranking);
        let mut refreshed_pointers: Vec<_> = session
            .input
            .mixed_candidates
            .iter()
            .map(|row| row.word.as_ptr())
            .collect();
        refreshed_pointers.sort_unstable();
        assert_eq!(refreshed_pointers, string_pointers);
        let original_index = session.input.ranking_index(0).unwrap();
        assert_ne!(original_index, 0);
        assert_eq!(session.input.ranking_list()[original_index].word, "乙");
    }
}

#[test]
fn personal_context_keeps_an_exact_reading_ahead_of_a_shorter_reading() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_h VALUES('hao','h','子',100);
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_g VALUES('gei','g','甲',100);
INSERT INTO tbl_1_g VALUES('ge','g','乙',1000);",
    );
    let journal = crate::user_dictionary::journal::open_journal(&fixture.journal()).unwrap();
    journal
        .execute(
            "INSERT INTO personal_bigram(previous,word,count) VALUES('子','乙',32)",
            [],
        )
        .unwrap();
    drop(journal);
    let mut session = fixture.session_with(|options| {
        options.personal_context = true;
        options.helpcode = false;
    });
    type_text(&mut session, "gei");
    assert_eq!(words(&session)[0], "甲");
    assert!(words(&session).iter().any(|word| word == "乙"));
    session.command(Command::Cancel);
    type_text(&mut session, "hao");
    select_word(&mut session, "子");
    type_text(&mut session, "gei");
    assert_eq!(words(&session)[0], "甲");
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("甲")
    );
}

#[test]
fn personal_context_reranks_complete_alternative_segmentations_together() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_h VALUES('hen','h','很',100);
CREATE TABLE tbl_2_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_f VALUES('fang''an','fa','方案',1000);
INSERT INTO tbl_2_f VALUES('fan''gan','fg','反感',100);",
    );
    let journal = crate::user_dictionary::journal::open_journal(&fixture.journal()).unwrap();
    journal
        .execute(
            "INSERT INTO personal_bigram(previous,word,count) VALUES('很','反感',32)",
            [],
        )
        .unwrap();
    drop(journal);
    let mut session = fixture.session_with(|options| {
        options.personal_context = true;
        options.helpcode = false;
    });
    type_text(&mut session, "fangan");
    assert_eq!(words(&session)[0], "方案");
    assert!(words(&session).iter().any(|word| word == "反感"));
    session.command(Command::Cancel);
    type_text(&mut session, "hen");
    select_word(&mut session, "很");
    type_text(&mut session, "fangan");
    assert_eq!(words(&session)[0], "反感");
}

/// One directory standing in for all four runtime roots, as the reference session tests used.
struct Fixture {
    directory: tempfile::TempDir,
}

fn prediction_fixture() -> Fixture {
    Fixture::new(
        "CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_c VALUES('chi','c','吃',1000);
CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_c VALUES('chi''fan','cf','吃饭',100);
CREATE TABLE tbl_3_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_3_c VALUES('chi''dong''xi','cdx','吃东西',80);
INSERT INTO tbl_3_c VALUES('chi''fan''cai','cfc','吃饭菜',60);
CREATE TABLE tbl_1_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_f VALUES('fan','f','饭',1000);
CREATE TABLE tbl_2_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_f VALUES('fan''hou','fh','饭后',50);
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_n VALUES('ni','n','你',1000);",
    )
}

#[test]
fn next_word_prediction_is_opt_in_and_has_no_composing_text() {
    let fixture = prediction_fixture();
    let mut off = fixture.session_with(|options| options.learning = false);
    type_text(&mut off, "chi");
    assert_eq!(select_word(&mut off, "吃").commit.as_deref(), Some("吃"));
    assert!(off.snapshot().candidates.is_empty());
    let mut on = fixture.session_with(|options| {
        options.next_word_prediction = true;
        options.learning = false;
    });
    type_text(&mut on, "chi");
    assert_eq!(select_word(&mut on, "吃").commit.as_deref(), Some("吃"));
    let snapshot = on.snapshot();
    assert!(snapshot.prediction);
    assert!(snapshot.preedit.is_empty() && snapshot.editing_text.is_empty());
    assert!(snapshot.candidates.iter().any(|row| row.word == "饭"));
    assert!(snapshot.candidates.iter().any(|row| row.word == "东西"));
    assert!(snapshot.candidates.iter().all(|row| row.pinyin.is_empty()));
    assert_eq!(select_word(&mut on, "饭").commit.as_deref(), Some("饭"));
    assert!(on.snapshot().candidates.iter().any(|row| row.word == "菜"));
}

/// 联想按上屏的词查以它开头的长词，给的是后半截：上屏 巨 不联想出 巨乳 的 乳。
#[test]
fn next_word_prediction_skips_offensive_words() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_j VALUES('ju','j','巨',1000);
CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_j VALUES('ju''ru','jr','巨乳',300);
INSERT INTO tbl_2_j VALUES('ju''ren','jr','巨人',100);",
    );
    let mut session = fixture.session_with(|options| {
        options.next_word_prediction = true;
        options.learning = false;
    });
    type_text(&mut session, "ju");
    assert_eq!(
        select_word(&mut session, "巨").commit.as_deref(),
        Some("巨")
    );
    let snapshot = session.snapshot();
    assert!(snapshot.prediction);
    assert!(snapshot.candidates.iter().any(|row| row.word == "人"));
    assert!(snapshot.candidates.iter().all(|row| row.word != "乳"));
}

#[test]
fn next_word_prediction_never_commits_on_space_return_delete_or_finish() {
    let fixture = prediction_fixture();
    for command in [
        Command::CommitCandidate,
        Command::CommitRaw,
        Command::Backspace,
        Command::DeleteForward,
    ] {
        let mut session = fixture.session_with(|options| options.next_word_prediction = true);
        type_text(&mut session, "chi");
        select_word(&mut session, "吃");
        let result = session.command(command);
        assert!(!result.handled && result.commit.is_none(), "{command:?}");
        assert!(!session.snapshot().prediction);
    }
    let mut session = fixture.session_with(|options| options.next_word_prediction = true);
    type_text(&mut session, "chi");
    select_word(&mut session, "吃");
    assert!(session.finish(0).commit.is_none());
    assert!(session.snapshot().candidates.is_empty());
}

#[test]
fn next_word_prediction_clears_for_new_input_layout_mode_and_external_context() {
    let fixture = prediction_fixture();
    for action in 0..6 {
        let mut session = fixture.session_with(|options| options.next_word_prediction = true);
        type_text(&mut session, "chi");
        select_word(&mut session, "吃");
        match action {
            0 => {
                assert!(!session.character(b'1', false).handled);
            }
            1 => {
                session.character(b'n', false);
            }
            2 => session.set_key_grid(Some(KeyGrid::NineKey)),
            3 => session.set_dedicated_english(true),
            4 => session.reset_context(),
            _ => {
                session.punctuation(b',');
            }
        }
        assert!(!session.snapshot().prediction);
    }
}

#[test]
fn next_word_prediction_works_after_shuangpin_and_nine_key_selection() {
    let fixture = prediction_fixture();
    for grid in [false, true] {
        let mut session = fixture.session_with(|options| {
            options.next_word_prediction = true;
            if !grid {
                options.scheme = SchemeType::Shuangpin;
            }
        });
        if grid {
            session.set_nine_key_enabled(true);
        }
        type_text(&mut session, if grid { "244" } else { "ii" });
        select_word(&mut session, "吃");
        assert!(session.snapshot().prediction);
        assert!(session
            .snapshot()
            .candidates
            .iter()
            .any(|row| row.word == "饭"));
    }
}

#[test]
fn next_word_prediction_works_after_letter_grid_selection() {
    let fixture = prediction_fixture();
    for grid in [KeyGrid::FourteenKey, KeyGrid::EighteenKey] {
        let mut session = fixture.session_with(|options| options.next_word_prediction = true);
        session.set_key_grid(Some(grid));
        for value in b"chi" {
            assert!(session.grid_key(*value).handled);
        }
        assert_eq!(
            select_word(&mut session, "吃").commit.as_deref(),
            Some("吃")
        );
        assert!(session.snapshot().prediction);
        assert!(session.snapshot_for_host(false).prediction);
        assert_eq!(
            select_word(&mut session, "饭").commit.as_deref(),
            Some("饭")
        );
    }
}

#[test]
fn next_word_prediction_clears_on_direct_candidate_key_and_conversion_navigation() {
    let fixture = prediction_fixture();
    for conversion in [false, true] {
        let mut session = fixture.session_with(|options| options.next_word_prediction = true);
        type_text(&mut session, "chi");
        select_word(&mut session, "吃");
        let result = if conversion {
            session.conversion_left_from(0)
        } else {
            session.candidate_key(b'1')
        };
        assert!(!result.handled && result.commit.is_none());
        assert!(!session.snapshot().prediction);
    }
}

#[test]
fn next_word_prediction_uses_learned_transitions_only_when_learning_is_enabled() {
    let fixture = prediction_fixture();
    let store =
        crate::user_dictionary::ngram_store::PersonalNgramStore::for_journal(&fixture.journal());
    store
        .record(&[crate::lattice::personal::PersonalTransition {
            earlier: String::new(),
            previous: "吃".into(),
            word: "合成词".into(),
            times: 3,
        }])
        .unwrap();
    for learning in [false, true] {
        let mut session = fixture.session_with(|options| {
            options.next_word_prediction = true;
            options.learning = learning;
        });
        type_text(&mut session, "chi");
        select_word(&mut session, "吃");
        assert_eq!(
            session
                .snapshot()
                .candidates
                .iter()
                .any(|row| row.word == "合成词"),
            learning
        );
    }
}

#[test]
fn next_word_prediction_reset_does_not_learn_across_the_discarded_context() {
    let fixture = prediction_fixture();
    let mut session = fixture.session_with(|options| options.next_word_prediction = true);
    type_text(&mut session, "chi");
    select_word(&mut session, "吃");
    session.reset_context();
    type_text(&mut session, "fan");
    select_word(&mut session, "饭");
    let store =
        crate::user_dictionary::ngram_store::PersonalNgramStore::for_journal(&fixture.journal());
    assert!(store.next_words("吃", 8).is_empty());
}

impl Fixture {
    fn new(main_sql: &str) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        Connection::open(directory.path().join(assets::MAIN_DICTIONARY))
            .and_then(|connection| connection.execute_batch(main_sql))
            .expect("fixture msime-pinyin.db");
        let helpcodes = directory.path().join("helpcodes");
        std::fs::create_dir_all(&helpcodes).expect("helpcode directory");
        std::fs::write(helpcodes.join("helpcode.txt"), "你=ab\n拟=cd\n好=ef\n").expect("helpcodes");
        Self { directory }
    }

    fn with_english(self, sql: &str) -> Self {
        let path = self.path().join(assets::ENGLISH_DICTIONARY);
        crate::ensure_english_schema(&path).expect("english schema");
        Connection::open(&path)
            .and_then(|connection| connection.execute_batch(sql))
            .expect("fixture msime-english.db");
        self
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    fn paths(&self) -> RuntimePaths {
        let root = self.path().to_path_buf();
        RuntimePaths {
            resources: root.clone(),
            user_data: root.clone(),
            cache: root.clone(),
            dictionaries: root,
        }
    }

    fn options(&self) -> SessionOptions {
        let mut options = SessionOptions::new(self.paths());
        // 这些测试按参照实现的默认值写成：辅助码开、表名 `lantian`（引擎库默认关，见 `SessionOptions::new`）。
        options.helpcode = true;
        options.helpcode_schema = "lantian".to_owned();
        // Frequency learning off, as the reference session tests configured it, so only the behaviour under test moves the order.
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Disabled,
            trigger_count: 1,
            linear_step: 1,
        };
        options
    }

    fn session(&self) -> Session {
        Session::new(self.options()).expect("session")
    }

    fn session_with(&self, configure: impl FnOnce(&mut SessionOptions)) -> Session {
        let mut options = self.options();
        configure(&mut options);
        Session::new(options).expect("session")
    }

    fn main_db(&self) -> PathBuf {
        self.path().join(assets::MAIN_DICTIONARY)
    }

    fn journal(&self) -> PathBuf {
        self.path().join(assets::USER_JOURNAL)
    }
}

fn type_text(session: &mut Session, text: &str) {
    for byte in text.bytes() {
        assert!(
            session.character(byte, false).handled,
            "{:?} of {text:?} was not handled",
            byte as char
        );
    }
}

fn words(session: &Session) -> Vec<String> {
    session
        .snapshot()
        .candidates
        .into_iter()
        .map(|item| item.word.into_string())
        .collect()
}

fn index_of(session: &Session, word: &str) -> usize {
    words(session)
        .iter()
        .position(|candidate| candidate == word)
        .unwrap_or_else(|| panic!("{word} is not a candidate of {:?}", words(session)))
}

#[test]
fn quanpin_retroflex_selection_consumes_raw_letters_and_learns_canonical_reading() {
    for (input, word, rest, canonical, caret) in [
        ("shdx", "上海大学", "", "shang'hai'da'xue", None),
        ("zhg", "中国", "", "zhong'guo", None),
        ("shdxni", "上海大学", "ni", "shang'hai'da'xue", None),
        ("shdxni", "上海大学", "ni", "shang'hai'da'xue", Some(4)),
    ] {
        let fixture = Fixture::new(
            "CREATE TABLE tbl_4_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_4_s VALUES('shang''hai''da''xue','shdx','上海大雪',11000),('shang''hai''da''xue','shdx','上海大学',10000);\
CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_z VALUES('zhong''guo','zg','忠国',11000),('zhong''guo','zg','中国',9000);\
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',8000);",
        );
        let mut session = fixture.session_with(|options| {
            options.helpcode = false;
            options.frequency.mode = FrequencyAdjustmentMode::Promote;
        });
        type_text(&mut session, input);
        if let Some(caret) = caret {
            session.set_caret(Some(caret));
        }
        let index = index_of(&session, word);
        assert!(index > 0, "调频回归需要明确选择非首选词");
        let row = &session.snapshot().candidates[index];
        assert_eq!(row.canonical_pinyin, canonical);
        assert_eq!(row.pinyin.replace('\'', "").len(), input.len() - rest.len());
        assert_eq!(session.select(index).commit.as_deref(), Some(word));
        assert_eq!(session.snapshot().editing_text, rest);
        let journal = Connection::open(fixture.journal()).unwrap();
        let learned: i64 = journal.query_row(
            "SELECT count(*) FROM user_dictionary_operations WHERE key=?1 AND value=?2 AND operation<>'delete'",
            [canonical, word], |row| row.get(0),
        ).unwrap();
        assert!(learned > 0, "简拼选择未按标准读音学习");
    }
}

#[test]
fn invalid_shuangpin_fuzzy_initial_selection_keeps_raw_coverage_and_learning() {
    for (input, rest, caret) in [
        ("slrf", "", None),
        ("slrfni", "ni", None),
        ("slrfni", "ni", Some(4)),
    ] {
        let fixture = Fixture::new(
            "CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_s VALUES('shuang''ren','sr','双刃',11000),('shuang''ren','sr','双人',10000);\
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',8000);",
        );
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.helpcode = false;
            options.fuzzy_pinyin.rules = crate::types::fuzzy_rule::S_SH;
            options.frequency.mode = FrequencyAdjustmentMode::Promote;
        });
        type_text(&mut session, input);
        if let Some(caret) = caret {
            session.set_caret(Some(caret));
        }
        let index = index_of(&session, "双人");
        assert!(index > 0, "调频夹具明确选择非首选词");
        let row = &session.snapshot().candidates[index];
        assert_eq!(row.pinyin.replace('\'', ""), "slrf");
        assert_eq!(row.canonical_pinyin, "shuang'ren");
        assert!(row.fuzzy);
        assert_eq!(session.select(index).commit.as_deref(), Some("双人"));
        assert_eq!(session.snapshot().editing_text, rest);
        let journal = Connection::open(fixture.journal()).unwrap();
        let learned: i64 = journal.query_row(
            "SELECT count(*) FROM user_dictionary_operations WHERE key='shuang''ren' AND value='双人' AND operation<>'delete'",
            [], |row| row.get(0),
        ).unwrap();
        assert!(learned > 0, "模糊两键选择必须按标准读音学习");
    }
}

fn select_word(session: &mut Session, word: &str) -> crate::types::KeyResult {
    let index = index_of(session, word);
    session.select(index)
}

#[test]
fn xiaohe_initial_frequency_learning_survives_a_different_previous_word() {
    for extra_reading in ["", "INSERT INTO tbl_1_l VALUES('liao','l','乙',80);"] {
        for mode in [
            FrequencyAdjustmentMode::Disabled,
            FrequencyAdjustmentMode::Promote,
        ] {
            let fixture = Fixture::new(&format!(
                "CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_l VALUES('lai','l','甲',100),('le','l','乙',90);\
{extra_reading}\
CREATE TABLE tbl_2_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wan','w','丙',100);",
            ));
            let configure = |options: &mut SessionOptions| {
                options.scheme = SchemeType::Shuangpin;
                options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
                options.helpcode = false;
                options.frequency = FrequencyAdjustmentOptions {
                    mode,
                    trigger_count: 1,
                    linear_step: 1,
                };
            };
            let mut session = fixture.session_with(configure);
            type_text(&mut session, "l");
            assert_eq!(words(&session)[0], "甲");
            for _ in 0..5 {
                if session.snapshot().editing_text.is_empty() {
                    type_text(&mut session, "l");
                }
                let index = index_of(&session, "乙");
                assert_eq!(session.snapshot().candidates[index].canonical_pinyin, "le");
                let result = session.select(index);
                assert_eq!(result.commit.as_deref(), Some("乙"));
                assert_eq!(result.diagnostic, None);
            }
            type_text(&mut session, "l");
            assert_eq!(words(&session)[0], "乙", "连续选择时上下文也能提升候选");
            session.command(Command::Cancel);
            type_text(&mut session, "wj");
            assert_eq!(
                select_word(&mut session, "丙").commit.as_deref(),
                Some("丙")
            );
            let expected = if mode == FrequencyAdjustmentMode::Disabled {
                "甲"
            } else {
                "乙"
            };
            type_text(&mut session, "l");
            assert_eq!(
                words(&session)[0],
                expected,
                "改变上文后的排序，mode={mode:?}"
            );
            drop(session);
            let mut reopened = fixture.session_with(configure);
            type_text(&mut reopened, "l");
            assert_eq!(
                words(&reopened)[0],
                expected,
                "新会话的持久排序，mode={mode:?}"
            );
        }
    }
}

#[test]
fn xiaohe_initial_promote_reaches_first_from_beyond_the_first_five_rows() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_l VALUES('lai','l','甲',100),('lai','l','丁',95),\
('lai','l','戊',94),('lai','l','己',93),('lai','l','庚',92),\
('lai','l','辛',91),('le','l','乙',90);\
CREATE TABLE tbl_2_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wan','w','丙',100);",
    );
    let configure = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        options.helpcode = false;
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
    };
    let mut session = fixture.session_with(configure);
    type_text(&mut session, "l");
    assert_eq!(index_of(&session, "乙"), 6);
    for _ in 0..5 {
        if session.snapshot().editing_text.is_empty() {
            type_text(&mut session, "l");
        }
        let result = select_word(&mut session, "乙");
        assert_eq!(result.commit.as_deref(), Some("乙"));
        assert_eq!(result.diagnostic, None);
    }
    type_text(&mut session, "l");
    assert_eq!(words(&session)[0], "乙");
    session.command(Command::Cancel);
    type_text(&mut session, "wj");
    assert_eq!(
        select_word(&mut session, "丙").commit.as_deref(),
        Some("丙")
    );
    type_text(&mut session, "l");
    assert_eq!(words(&session)[0], "乙", "换上文后仍使用已调高的全局词频");
    drop(session);
    let mut reopened = fixture.session_with(configure);
    type_text(&mut reopened, "l");
    assert_eq!(words(&reopened)[0], "乙", "重建会话后保持已持久化的顺序");
}

#[test]
fn quanpin_alternative_series_selection_consumes_letters_and_learns_its_own_reading() {
    for (input, word, canonical, rest, caret) in [
        ("fanouer", "帆偶尔", "fan'ou'er", "", None),
        ("fanouer", "帆", "fan", "ouer", None),
        ("fanouerni", "帆偶尔", "fan'ou'er", "ni", Some(7)),
    ] {
        let fixture = Fixture::new(
            "CREATE TABLE tbl_1_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_f VALUES('fa','f','发',300),('fan','f','帆',200);\
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('nou','n','耨',1),('ni','n','尼',100);\
CREATE TABLE tbl_1_o(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_o VALUES('ou','o','偶',100);\
CREATE TABLE tbl_1_e(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_e VALUES('er','e','尔',100);\
CREATE TABLE tbl_2_o(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_o VALUES('ou''er','oe','偶尔',1000);\
CREATE TABLE tbl_3_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);",
        );
        if word == "帆" {
            // 调频需要同读音的另一条高权重词，不能依靠默认接受也写日志。
            Connection::open(fixture.main_db())
                .unwrap()
                .execute("INSERT INTO tbl_1_f VALUES('fan','f','凡',300)", [])
                .unwrap();
        }
        let mut session = fixture.session_with(|options| {
            options.helpcode = false;
            options.autocorrect_types = 0;
            options.frequency.mode = FrequencyAdjustmentMode::Promote;
        });
        type_text(&mut session, input);
        if let Some(caret) = caret {
            session.set_caret(Some(caret));
        }
        let index = index_of(&session, word);
        let row = &session.snapshot().candidates[index];
        assert_eq!(
            row.pinyin.replace('\'', "").len(),
            if word == "帆" { 3 } else { 7 }
        );
        assert_eq!(row.canonical_pinyin, canonical);
        assert!(row.corrected_from.is_empty());
        let selected = session.select(index);
        assert_eq!(selected.diagnostic, None);
        assert_eq!(selected.commit.as_deref(), Some(word));
        assert_eq!(session.snapshot().editing_text, rest);
        let journal = Connection::open(fixture.journal()).unwrap();
        let learned: i64 = journal.query_row(
            "SELECT count(*) FROM user_dictionary_operations WHERE key=?1 AND value=?2 AND operation<>'delete'",
            [canonical, word], |row| row.get(0),
        ).unwrap();
        assert!(
            learned > 0,
            "{input}/{word}/{canonical}：备选切分必须按候选自己的标准读音学习"
        );
        if word == "帆" {
            assert_eq!(
                select_word(&mut session, "偶尔").commit.as_deref(),
                Some("偶尔")
            );
            assert!(session.snapshot().editing_text.is_empty());
        }
    }
}

fn count(database: &Path, sql: &str) -> i64 {
    Connection::open(database)
        .and_then(|connection| connection.query_row(sql, [], |row| row.get(0)))
        .unwrap_or(0)
}

/// A steady clock tests move by hand, and a pinned wall clock.
#[derive(Clone)]
struct TestClock {
    now: Arc<Mutex<Instant>>,
}

impl TestClock {
    fn install(session: &mut Session) -> Self {
        let clock = Self {
            now: Arc::new(Mutex::new(Instant::now())),
        };
        let steady = clock.now.clone();
        session.set_clock(Clock {
            steady: Box::new(move || *steady.lock().expect("clock")),
            local: Arc::new(|| LocalDateTime {
                year: 2026,
                month: 8,
                day: 9,
                weekday: 0,
                hour: 14,
                minute: 30,
                second: 0,
            }),
        });
        clock
    }

    fn advance(&self, by: Duration) {
        *self.now.lock().expect("clock") += by;
    }
}

// ---- pure helpers ----

#[test]
fn quanpin_boundaries_follow_the_display_separators() {
    assert_eq!(
        quanpin_raw_boundaries("nihaoma", "ni'hao'ma"),
        vec![0, 2, 5, 7]
    );
    assert_eq!(quanpin_raw_boundaries("ni'hao", "ni'hao"), vec![0, 3, 6]);
    assert_eq!(quanpin_raw_boundaries("ni'", "ni'"), vec![0, 3]);
    assert_eq!(quanpin_raw_boundaries("sahng", "sahng"), vec![0, 5]);
    // A display that does not spell the raw letters maps nothing rather than an arbitrary span.
    assert!(quanpin_raw_boundaries("nihao", "ni'ha").is_empty());
    assert!(quanpin_raw_boundaries("'", "'").is_empty());
}

// ---- caret prefix (upstream bc46f27) ----

#[test]
fn caret_prefix_decodes_only_the_complete_units_before_the_caret() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let typed = |text: &str| {
        let mut other = fixture.session();
        type_text(&mut other, text);
        words(&other)
    };
    let mut session = fixture.session();
    let sentence = "ni'hao'shi'jie";
    type_text(&mut session, sentence);
    let full = words(&session);
    assert_eq!(session.prefix_end(), sentence.len());
    assert_eq!(session.pending_suffix(), "");

    for caret in [5, 6] {
        session.set_caret(Some(caret));
        assert_eq!(session.prefix_end(), 3, "caret {caret}");
        assert_eq!(session.pending_suffix(), "hao'shi'jie");
        assert!(words(&session).contains(&"你".to_owned()));
        assert_eq!(words(&session), typed("ni"));
    }

    session.set_caret(Some(7));
    assert_eq!(session.prefix_end(), 7);
    assert_eq!(session.pending_suffix(), "shi'jie");
    let at_hao = words(&session);
    assert_eq!(at_hao, typed("ni'hao"));
    session.set_caret(Some(9));
    assert_eq!(session.prefix_end(), 7);
    assert_eq!(words(&session), at_hao);

    // A caret inside the first unit has no whole unit before it, so the whole-input list stays (904bd0976) instead of an empty prefix query.
    for caret in [0, 1, 2] {
        session.set_caret(Some(caret));
        assert_eq!(words(&session), full, "caret {caret}");
        assert_eq!(session.prefix_end(), 0, "caret {caret}");
        assert_eq!(session.pending_suffix(), sentence, "caret {caret}");
    }
    session.set_caret(Some(0));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, sentence);
    assert_eq!(snapshot.caret_position, 0);
    assert_eq!(snapshot.preedit, sentence);

    session.set_caret(Some(sentence.len() + 10));
    assert_eq!(session.snapshot().caret_position, sentence.len());
    assert_eq!(session.prefix_end(), sentence.len());
    assert_eq!(words(&session), full);
}

#[test]
fn caret_prefix_leaves_schemes_without_pinyin_units_alone() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    type_text(&mut session, "aaaa");
    let native = words(&session);
    assert!(native.contains(&"工".to_owned()));
    session.set_caret(Some(0));
    assert_eq!(session.prefix_end(), 4);
    assert_eq!(session.pending_suffix(), "");
    assert_eq!(words(&session), native);
}

#[test]
fn caret_prefix_floors_to_a_shuangpin_unit() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let microsoft = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
    };
    let mut session = fixture.session_with(microsoft);
    type_text(&mut session, "nihkb;");
    assert_eq!(session.prefix_end(), 6);
    session.set_caret(Some(3));
    assert_eq!(session.prefix_end(), 2);
    assert_eq!(session.pending_suffix(), "hkb;");
    let mut prefix = fixture.session_with(microsoft);
    type_text(&mut prefix, "ni");
    assert_eq!(words(&session), words(&prefix));
}

// ---- 触屏触点（#10018） ----

/// a 到 z 排成一行，键宽 10、键高 10。
fn touch_geometry() -> Arc<crate::touch::KeyGeometry> {
    let mut keys = [[0.0f32; 4]; 26];
    for (index, key) in keys.iter_mut().enumerate() {
        *key = [index as f32 * 10.0, 0.0, 10.0, 10.0];
    }
    Arc::new(crate::touch::KeyGeometry::new(3, &keys, None, &[]).unwrap())
}

/// 当前请求带着的触点。
fn request_touches(session: &Session) -> Vec<Option<crate::touch::KeyTouch>> {
    session.input.engine.request().key_touches.clone()
}

/// 待用触点只交给紧接着的那一个 26 键双拼字母：命令、选词这些入口一开头就丢掉它，改道到别处的按键不会把它留给后面的字母；全拼和没有几何时不带触点。
#[test]
fn a_pending_touch_reaches_only_the_next_shuangpin_letter() {
    let touch = Some(crate::touch::KeyTouch {
        dx: 1,
        dy: 0,
        copy: 0,
    });
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Shuangpin);
    // 没有几何：触点进不了请求。
    session.set_key_touch(touch);
    session.character(b'n', false);
    assert!(request_touches(&session).is_empty());
    session.command(Command::Cancel);

    session.set_key_geometry(Some(touch_geometry()));
    session.set_key_touch(touch);
    session.character(b'n', false);
    session.character(b'i', false);
    assert_eq!(request_touches(&session), [touch, None]);
    // 退格删掉带触点的那个键之后，剩下的一个触点都没有。
    session.command(Command::Backspace);
    session.command(Command::Backspace);
    session.character(b'n', false);
    assert!(request_touches(&session).is_empty());
    session.command(Command::Cancel);

    // 设了触点、这一键却改道成命令：触点作废，下一个字母不带。
    session.set_key_touch(touch);
    session.command(Command::Cancel);
    session.character(b'n', false);
    assert!(request_touches(&session).is_empty());
    // 选词、标点同理。
    session.set_key_touch(touch);
    session.select(0);
    session.character(b'n', false);
    assert!(request_touches(&session).is_empty());
    session.set_key_touch(touch);
    session.punctuation(b',');
    session.character(b'n', false);
    assert!(request_touches(&session).is_empty());
    session.command(Command::Cancel);

    // 清掉几何之后不再带触点。
    session.set_key_geometry(None);
    session.set_key_touch(touch);
    session.character(b'n', false);
    assert!(request_touches(&session).is_empty());
    session.command(Command::Cancel);

    // 全拼不收触点。
    let mut quanpin = fixture.session_with(|_| {});
    quanpin.set_key_geometry(Some(touch_geometry()));
    quanpin.set_key_touch(touch);
    quanpin.character(b'n', false);
    assert!(quanpin.input.engine.request().key_touches.is_empty());
}

/// 以词定字、结束组字、移光标这些不派发字符的入口同样一开头丢掉待用触点：设了触点之后先调它们，下一个字母不带触点。
#[test]
fn non_character_entries_discard_the_pending_touch() {
    let touch = Some(crate::touch::KeyTouch {
        dx: 1,
        dy: 0,
        copy: 0,
    });
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Shuangpin);
    session.set_key_geometry(Some(touch_geometry()));
    type Entry = (&'static str, fn(&mut Session));
    let entries: [Entry; 17] = [
        ("select_edge", |session| {
            session.select_edge(0, CandidateEdge::FirstHan);
        }),
        ("finish", |session| {
            session.finish(0);
        }),
        ("set_caret", |session| session.set_caret(None)),
        ("candidate_key", |session| {
            session.candidate_key(b'1');
        }),
        ("conversion_left_from", |session| {
            session.conversion_left_from(0);
        }),
        ("pin", |session| {
            session.pin(0);
        }),
        ("remove", |session| {
            session.remove(5);
        }),
        ("fix_position", |session| {
            session.fix_position(0, 1);
        }),
        ("clear_position", |session| {
            session.clear_position(0);
        }),
        ("choose_nine_key_spelling", |session| {
            session.choose_nine_key_spelling(0);
        }),
        ("set_nine_key_filter", |session| {
            session.set_nine_key_filter(false, "");
        }),
        ("expand_initial_candidates", |session| {
            session.expand_initial_candidates();
        }),
        ("reset_context", Session::reset_context),
        ("set_key_grid", |session| session.set_key_grid(None)),
        ("set_dedicated_english", |session| {
            session.set_dedicated_english(false)
        }),
        ("switch_scheme", |session| {
            session.switch_scheme(SchemeType::Shuangpin).unwrap();
        }),
        ("set_key_geometry", |session| {
            session.set_key_geometry(Some(touch_geometry()))
        }),
    ];
    for (name, entry) in entries {
        session.command(Command::Cancel);
        // 先打一段不带触点的组字，选词、结束、移光标才有东西可动。
        session.character(b'n', false);
        session.character(b'i', false);
        session.set_key_touch(touch);
        entry(&mut session);
        session.character(b'h', false);
        assert!(
            request_touches(&session).iter().all(Option::is_none),
            "{name}: {:?}",
            request_touches(&session)
        );
    }
    // 对照：中间不调别的入口时，触点交给下一个字母。
    session.command(Command::Cancel);
    session.character(b'n', false);
    session.character(b'i', false);
    session.set_key_touch(touch);
    session.character(b'h', false);
    assert_eq!(request_touches(&session), [None, None, touch]);
}

/// 合成词库：小鹤 mw gr xi 是没关系，r 按成邻键 f 就成了 gen（跟）。
const TOUCH_TYPO_FIXTURE: &str = "CREATE TABLE tbl_1_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_m VALUES('mei','m','没',1000);CREATE TABLE tbl_1_g(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('guan','g','关',500);CREATE TABLE tbl_1_x(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_x VALUES('xi','x','系',1000);CREATE TABLE tbl_2_g(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_g VALUES('guan''xi','gx','关系',100000);";

/// QWERTY 字母键，键宽 10、键高 14，每排比上一排右移半个键。
fn qwerty_geometry() -> Arc<crate::touch::KeyGeometry> {
    qwerty_geometry_moving(&[])
}

/// 同一个代次（5）的 QWERTY 字母键，`moved` 里的键挪到给定的键心。
fn qwerty_geometry_moving(moved: &[(u8, f32, f32)]) -> Arc<crate::touch::KeyGeometry> {
    let mut keys = [[0.0f32; 4]; 26];
    for (letters, left, y) in [
        ("qwertyuiop", 5.0, 7.0),
        ("asdfghjkl", 10.0, 21.0),
        ("zxcvbnm", 20.0, 35.0),
    ] {
        for (column, letter) in letters.bytes().enumerate() {
            keys[usize::from(letter - b'a')] = [left + 10.0 * column as f32, y, 10.0, 14.0];
        }
    }
    for &(key, x, y) in moved {
        keys[usize::from(key - b'a')][..2].copy_from_slice(&[x, y]);
    }
    Arc::new(crate::touch::KeyGeometry::new(5, &keys, None, &[]).unwrap())
}

/// 落点支持的纠错整句在快照里标上 `touch_supported`，runtime 才不把这一行挪到表尾；同一串键不带触点（平面表）时一行都没有。
#[test]
fn a_supported_touch_correction_is_reported_in_the_snapshot() {
    let fixture = Fixture::new(TOUCH_TYPO_FIXTURE);
    let shuangpin = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Shuangpin;
        options.autocorrect_types = crate::types::autocorrect_type::TRANSPOSITION
            | crate::types::autocorrect_type::NEIGHBOR;
    };
    let typed = |session: &mut Session, touched: bool| {
        for (index, byte) in b"mwgfxi".iter().enumerate() {
            if touched {
                // f 那一键落在 f 和左上方的 r 之间。
                let (dx, dy) = if index == 3 { (-5, -10) } else { (0, 0) };
                session.set_key_touch(Some(crate::touch::KeyTouch { dx, dy, copy: 0 }));
            }
            session.character(*byte, false);
        }
    };
    let mut flat = fixture.session_with(shuangpin);
    typed(&mut flat, false);
    let snapshot = flat.snapshot();
    assert!(snapshot.candidates.iter().any(|item| item.word == "没关系"));
    assert!(snapshot.candidates.iter().all(|item| !item.touch_supported));

    let mut touched = fixture.session_with(shuangpin);
    touched.set_key_geometry(Some(qwerty_geometry()));
    typed(&mut touched, true);
    let snapshot = touched.snapshot();
    let row = snapshot
        .candidates
        .iter()
        .find(|item| item.word == "没关系")
        .unwrap();
    assert!(row.touch_supported);
    // 只有这一条纠错整句被标上，别的行（包括别的纠错行）不受影响。
    assert!(snapshot
        .candidates
        .iter()
        .filter(|item| item.word != "没关系")
        .all(|item| !item.touch_supported));
    assert!(snapshot.candidates[0].corrected_from.is_empty());
}

/// 宿主用同一个代次下发挪过键的几何：同样的量化触点在新布局下计价不同，词典缓存不能把旧布局算出的行交出来。r 挪到键盘最右边之后不再是 f 的邻键，f 改成 r 的那条纠错整句就不该再出现、更不该带着落点支持的标记。
#[test]
fn a_same_id_geometry_with_moved_keys_does_not_reuse_cached_touch_rows() {
    let fixture = Fixture::new(TOUCH_TYPO_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.autocorrect_types = crate::types::autocorrect_type::TRANSPOSITION
            | crate::types::autocorrect_type::NEIGHBOR;
    });
    let typed = |session: &mut Session| {
        for (index, byte) in b"mwgfxi".iter().enumerate() {
            // f 那一键落在 f 和左上方的 r 之间。
            let (dx, dy) = if index == 3 { (-5, -10) } else { (0, 0) };
            session.set_key_touch(Some(crate::touch::KeyTouch { dx, dy, copy: 0 }));
            session.character(*byte, false);
        }
    };
    let supported = |session: &Session| {
        session
            .snapshot()
            .candidates
            .iter()
            .any(|item| item.word == "没关系" && item.touch_supported)
    };
    session.set_key_geometry(Some(qwerty_geometry()));
    typed(&mut session);
    assert!(supported(&session));
    session.command(Command::Cancel);

    let moved = qwerty_geometry_moving(&[(b'r', 205.0, 7.0)]);
    assert_eq!(moved.id(), qwerty_geometry().id());
    assert_ne!(moved.fingerprint(), qwerty_geometry().fingerprint());
    session.set_key_geometry(Some(moved));
    typed(&mut session);
    assert!(!supported(&session));
    session.command(Command::Cancel);

    // 换回原来的布局，答案照旧。
    session.set_key_geometry(Some(qwerty_geometry()));
    typed(&mut session);
    assert!(supported(&session));
}

// ---- 双拼触点纠错的学习（#10018） ----

/// `TOUCH_TYPO_FIXTURE` 加上整句学习要写的三字表，选中纠错整句不带诊断。
fn touch_learning_fixture() -> Fixture {
    Fixture::new(&format!(
        "{TOUCH_TYPO_FIXTURE}CREATE TABLE tbl_3_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
    ))
}

fn touch_shuangpin(options: &mut SessionOptions) {
    options.scheme = SchemeType::Shuangpin;
    options.autocorrect_types =
        crate::types::autocorrect_type::TRANSPOSITION | crate::types::autocorrect_type::NEIGHBOR;
}

/// 带触点打小鹤 `mwgfxi`：`toward_r` 时 f 那一键落在 f 和左上方的 r 之间（落点支持改成 gr），否则每键都按在键心。
fn type_touched(session: &mut Session, toward_r: bool) {
    for (index, byte) in b"mwgfxi".iter().enumerate() {
        let (dx, dy) = if index == 3 && toward_r {
            (-5, -10)
        } else {
            (0, 0)
        };
        session.set_key_touch(Some(crate::touch::KeyTouch { dx, dy, copy: 0 }));
        session.character(*byte, false);
    }
}

fn shuangpin_typo_count(fixture: &Fixture, column: &str) -> Option<i64> {
    Connection::open(fixture.journal())
        .ok()?
        .query_row(
            &format!("SELECT {column} FROM shuangpin_typo_counts WHERE profile='xiaohe' AND typed='gf' AND intended='gr'"),
            [],
            |row| row.get(0),
        )
        .ok()
}

fn shuangpin_suppressions(fixture: &Fixture) -> i64 {
    Connection::open(fixture.journal())
        .and_then(|journal| {
            journal.query_row(
                "SELECT coalesce(sum(commits),0) FROM shuangpin_autocorrect_suppressions WHERE profile='xiaohe' AND input='mwgfxi'",
                [],
                |row| row.get(0),
            )
        })
        .unwrap_or(0)
}

/// 带触点选中纠错整句记一次接受：键码对是按到的 gf 和变体集里解到 guan 的 gr，记在小鹤名下。关着学习、不带触点（平面表、桌面）时选同一行什么也不记。
#[test]
fn selecting_a_touch_correction_records_its_two_key_pair() {
    let fixture = touch_learning_fixture();
    let mut quiet = fixture.session_with(|options| {
        touch_shuangpin(options);
        options.learning = false;
    });
    quiet.set_key_geometry(Some(qwerty_geometry()));
    type_touched(&mut quiet, true);
    assert_eq!(select_word(&mut quiet, "没关系").diagnostic, None);
    assert_eq!(shuangpin_typo_count(&fixture, "accepted"), None);

    let mut flat = fixture.session_with(touch_shuangpin);
    type_text(&mut flat, "mwgfxi");
    assert_eq!(select_word(&mut flat, "没关系").diagnostic, None);
    assert_eq!(shuangpin_typo_count(&fixture, "accepted"), None);

    let mut touched = fixture.session_with(touch_shuangpin);
    touched.set_key_geometry(Some(qwerty_geometry()));
    for round in 1..=2 {
        type_touched(&mut touched, true);
        assert_eq!(select_word(&mut touched, "没关系").diagnostic, None);
        assert_eq!(shuangpin_typo_count(&fixture, "accepted"), Some(round));
        assert_eq!(shuangpin_typo_count(&fixture, "rejected"), Some(0));
    }
    // 全拼的纠错表一行不多。
    assert_eq!(
        Connection::open(fixture.journal())
            .unwrap()
            .query_row("SELECT count(*) FROM pinyin_typo_counts", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

/// 短词纠错消费整串原码；非法两键替换过同文前缀后，也不能留下一个键。
#[test]
fn selecting_short_touch_words_consumes_the_raw_code_and_learns_the_pair() {
    for (typed, position, delta, target, intended) in [
        ("gf", 1, (-5, -10), "关", "gr"),
        ("gq", 1, (14, 0), "给", "gw"),
        ("mwgf", 3, (-5, -10), "美观", "gr"),
    ] {
        let fixture = Fixture::new(
            "CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
            INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('guan','g','关',2000000),('gei','g','给',1000000);
            CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
            INSERT INTO tbl_1_m VALUES('mei','m','没',1000);
            CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
            INSERT INTO tbl_2_m VALUES('mei''guan','mg','美观',1000000);",
        );
        let mut session = fixture.session_with(touch_shuangpin);
        session.set_key_geometry(Some(qwerty_geometry()));
        for (index, byte) in typed.bytes().enumerate() {
            let (dx, dy) = if index == position { delta } else { (0, 0) };
            session.set_key_touch(Some(crate::touch::KeyTouch { dx, dy, copy: 0 }));
            session.character(byte, false);
        }
        let snapshot = session.snapshot();
        let at = snapshot
            .candidates
            .iter()
            .position(|row| row.word == target && row.touch_supported)
            .expect("会话提供落点支持的短词纠错");
        assert!(at > 0);
        let result = session.select(at);
        assert_eq!(result.commit.as_deref(), Some(target));
        assert_eq!(result.diagnostic, None);
        assert!(session.snapshot().editing_text.is_empty());
        assert!(session.snapshot().preedit.is_empty());
        let pressed = &typed[typed.len() - 2..];
        let accepted: i64 = Connection::open(fixture.journal())
            .unwrap()
            .query_row(
                "SELECT accepted FROM shuangpin_typo_counts WHERE profile='xiaohe' AND typed=?1 AND intended=?2",
                [pressed, intended],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(accepted, 1);
    }
}

/// 个人上下文重排只排字面首组：上一个词之后常接的短词纠错再重，也不能凭原始词频挤掉触点罚分后仍领先的字面首选。
#[test]
fn personal_context_keeps_short_touch_words_behind_the_literal_leader() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('guan','g','关',2000000),('gei','g','给',1000000);
        CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_m VALUES('mei','m','没',1000);",
    );
    let journal = crate::user_dictionary::journal::open_journal(&fixture.journal()).unwrap();
    journal
        .execute(
            "INSERT INTO personal_bigram(previous,word,count) VALUES('没','关',32)",
            [],
        )
        .unwrap();
    drop(journal);
    let mut session = fixture.session_with(|options| {
        touch_shuangpin(options);
        options.personal_context = true;
    });
    session.set_key_geometry(Some(qwerty_geometry()));
    for byte in b"mw" {
        session.set_key_touch(Some(crate::touch::KeyTouch {
            dx: 0,
            dy: 0,
            copy: 0,
        }));
        session.character(*byte, false);
    }
    select_word(&mut session, "没");
    for (byte, (dx, dy)) in b"gf".iter().zip([(0, 0), (-5, -10)]) {
        session.set_key_touch(Some(crate::touch::KeyTouch { dx, dy, copy: 0 }));
        session.character(*byte, false);
    }
    assert_eq!(session.input.chain.previous.as_deref(), Some("没"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.candidates[0].word, "跟");
    assert!(snapshot.candidates[0].corrected_from.is_empty());
    assert!(snapshot
        .candidates
        .iter()
        .skip(1)
        .any(|row| row.word == "关" && row.touch_supported));
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("跟")
    );
}

/// 短词纠错行不进个人上下文首组，但也不截断首组：排在它后面的字面同音字照样按上一个词重排，纠错行留在引擎给它的位置。
#[test]
fn personal_context_reranks_literal_words_behind_a_short_touch_word() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('gen','g','根',10),('guan','g','关',2000000),('gei','g','给',1000000);
        CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_m VALUES('mei','m','没',1000);",
    );
    let journal = crate::user_dictionary::journal::open_journal(&fixture.journal()).unwrap();
    journal
        .execute(
            "INSERT INTO personal_bigram(previous,word,count) VALUES('没','根',32)",
            [],
        )
        .unwrap();
    drop(journal);
    let mut session = fixture.session_with(|options| {
        touch_shuangpin(options);
        options.personal_context = true;
    });
    session.set_key_geometry(Some(qwerty_geometry()));
    for byte in b"mw" {
        session.set_key_touch(Some(crate::touch::KeyTouch {
            dx: 0,
            dy: 0,
            copy: 0,
        }));
        session.character(*byte, false);
    }
    select_word(&mut session, "没");
    for (byte, (dx, dy)) in b"gf".iter().zip([(0, 0), (-5, -10)]) {
        session.set_key_touch(Some(crate::touch::KeyTouch { dx, dy, copy: 0 }));
        session.character(*byte, false);
    }
    assert_eq!(session.input.chain.previous.as_deref(), Some("没"));
    // 引擎把纠错行排在字面首选之后、「根」之前，这正是首组需要跳过它的情形。
    let ranking = session.input.ranking_list();
    let position = |list: &[crate::types::WordItem], word: &str| {
        list.iter()
            .position(|row| row.word == word && row.corrected_from.is_empty() == (word != "关"))
            .unwrap()
    };
    let corrected = position(ranking, "关");
    assert!(ranking[corrected].touch_supported);
    assert_eq!(ranking[0].word, "跟");
    assert!(corrected < position(ranking, "根"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.candidates[0].word, "根");
    assert!(snapshot.candidates[0].corrected_from.is_empty());
    assert_eq!(snapshot.candidates[corrected].word, "关");
    assert!(snapshot.candidates[corrected].touch_supported);
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("根")
    );
}

/// 带触点的输入有落点支持的纠错可选、用户却按 Enter 原样上屏：记一次抑制，之后同一串输入带触点时按字面回答，一行纠错都没有、请求也不再带触点。不带触点（平面表）的会话不受这条记录影响，照旧给纠错整句。落点不支持（沉到表尾）的纠错行不算可选，上屏不记。
#[test]
fn committing_raw_over_a_touch_correction_suppresses_it() {
    let fixture = touch_learning_fixture();
    let mut session = fixture.session_with(touch_shuangpin);
    session.set_key_geometry(Some(qwerty_geometry()));
    // 每键都按在键心：纠错整句还在，但落点不支持它。
    type_touched(&mut session, false);
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .all(|item| !item.touch_supported));
    let raw = session.command(Command::CommitRaw);
    assert_eq!(raw.commit.as_deref(), Some("mwgfxi"));
    assert_eq!(shuangpin_suppressions(&fixture), 0);

    type_touched(&mut session, true);
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .any(|item| item.touch_supported));
    let raw = session.command(Command::CommitRaw);
    assert_eq!(raw.commit.as_deref(), Some("mwgfxi"));
    assert_eq!(raw.diagnostic, None);
    assert_eq!(shuangpin_suppressions(&fixture), 1);

    type_touched(&mut session, true);
    assert!(
        session
            .snapshot()
            .candidates
            .iter()
            .all(|item| item.corrected_from.is_empty()),
        "{:?}",
        words(&session)
    );
    assert!(request_touches(&session).is_empty());
    // 已经按字面回答了，再按 Enter 也没有可拒绝的。
    session.command(Command::CommitRaw);
    assert_eq!(shuangpin_suppressions(&fixture), 1);
    // 改一个键就是另一串输入，不受抑制。
    type_touched(&mut session, true);
    session.command(Command::Backspace);
    session.set_key_touch(Some(crate::touch::KeyTouch {
        dx: 0,
        dy: 0,
        copy: 0,
    }));
    session.character(b'u', false);
    assert_eq!(request_touches(&session).len(), 6);
    session.command(Command::Cancel);

    let mut flat = fixture.session_with(touch_shuangpin);
    type_text(&mut flat, "mwgfxi");
    assert!(flat
        .snapshot()
        .candidates
        .iter()
        .any(|item| item.word == "没关系" && !item.corrected_from.is_empty()));
    flat.command(Command::CommitRaw);
    assert_eq!(shuangpin_suppressions(&fixture), 1);
}

/// 首选是纠错行、用户却选了别的行：选中的行覆盖到的、读音不是纠正后音节的位置记一次拒绝。只覆盖前一个音节的词没对改动表态，不记；选中纠错行本身是接受。这一期纠错行还坐不上首选（首选规则在后面一期），这里把它挪到第一行来测记账本身。
#[test]
fn picking_another_row_over_a_leading_correction_counts_a_rejection() {
    let fixture = touch_learning_fixture();
    let mut session = fixture.session_with(touch_shuangpin);
    session.set_key_geometry(Some(qwerty_geometry()));
    let lead_with_correction = |session: &mut Session| {
        type_touched(session, true);
        let rows = &mut session.input.mixed_candidates;
        let at = rows
            .iter()
            .position(|item| item.word == "没关系" && !item.corrected_from.is_empty())
            .unwrap();
        let row = rows.remove(at);
        rows.insert(0, row);
        session.input.ranking_candidates = None;
    };

    lead_with_correction(&mut session);
    assert_eq!(select_word(&mut session, "没").diagnostic, None);
    session.command(Command::Cancel);
    assert_eq!(shuangpin_typo_count(&fixture, "rejected"), None);

    lead_with_correction(&mut session);
    let literal = words(&session)
        .into_iter()
        .find(|word| word == "没跟系")
        .unwrap();
    assert_eq!(select_word(&mut session, &literal).diagnostic, None);
    assert_eq!(shuangpin_typo_count(&fixture, "rejected"), Some(1));
    assert_eq!(shuangpin_typo_count(&fixture, "accepted"), Some(0));

    lead_with_correction(&mut session);
    assert_eq!(select_word(&mut session, "没关系").diagnostic, None);
    assert_eq!(shuangpin_typo_count(&fixture, "rejected"), Some(1));
    assert_eq!(shuangpin_typo_count(&fixture, "accepted"), Some(1));
}

/// 首选规则可用时的计价参数（其余照默认值）：分数比字面整句高就算明显更优，改动的那一键原价不超过 10。f 那一键落在 f 和 r 正中时原价是 B = 8。
fn lead_tuning() -> crate::touch::TouchTuning {
    crate::touch::TouchTuning {
        lead_margin: 0.0,
        lead_penalty: 10.0,
        ..crate::touch::TouchTuning::default()
    }
}

/// 拒绝一票否决首选（#10018 PR3）：首选是按首选规则坐上去的纠错整句，用户选了字面整句，记一次拒绝；之后同一串输入带同样的触点，纠错整句不再坐首选，但仍在表里，选中它照样是接受。
#[test]
fn rejecting_a_confident_correction_vetoes_its_first_seat() {
    let fixture = touch_learning_fixture();
    let mut session = fixture.session_with(touch_shuangpin);
    session.set_key_geometry(Some(qwerty_geometry()));
    session.set_touch_tuning(lead_tuning());
    type_touched(&mut session, true);
    let snapshot = session.snapshot();
    assert_eq!(
        snapshot.candidates[0].word,
        "没关系",
        "{:?}",
        words(&session)
    );
    assert!(snapshot.candidates[0].confident_correction);
    assert!(snapshot.candidates[1..]
        .iter()
        .all(|row| !row.confident_correction));
    assert_eq!(select_word(&mut session, "没跟系").diagnostic, None);
    assert_eq!(shuangpin_typo_count(&fixture, "rejected"), Some(1));

    type_touched(&mut session, true);
    let snapshot = session.snapshot();
    assert_ne!(
        snapshot.candidates[0].word,
        "没关系",
        "{:?}",
        words(&session)
    );
    assert!(snapshot
        .candidates
        .iter()
        .all(|row| !row.confident_correction));
    assert!(snapshot
        .candidates
        .iter()
        .any(|row| row.word == "没关系" && !row.corrected_from.is_empty()));
    assert_eq!(select_word(&mut session, "没关系").diagnostic, None);
    assert_eq!(shuangpin_typo_count(&fixture, "accepted"), Some(1));
    assert_eq!(shuangpin_typo_count(&fixture, "rejected"), Some(1));
}

/// 首选规则 (a)、(f) 落在会话上：撤回选词后重打的那一段没有触点、按键全在键心、或者不带触点（平面表），纠错整句都不坐首选；被抑制的输入（用户按原样上屏过）不再带触点，也不坐首选。
#[test]
fn the_session_puts_a_correction_first_only_with_every_key_touched() {
    let fixture = touch_learning_fixture();
    let first_is_confident = |session: &Session| {
        let snapshot = session.snapshot();
        assert!(snapshot.candidates[1..]
            .iter()
            .all(|row| !row.confident_correction));
        snapshot.candidates[0].confident_correction
    };
    let mut session = fixture.session_with(touch_shuangpin);
    session.set_key_geometry(Some(qwerty_geometry()));
    session.set_touch_tuning(lead_tuning());
    type_touched(&mut session, false);
    assert!(!first_is_confident(&session));
    session.command(Command::Cancel);
    // 最后一键没有触点（宿主没送、无障碍点击）。
    for (index, byte) in b"mwgfxi".iter().enumerate() {
        let (dx, dy) = if index == 3 { (-5, -10) } else { (0, 0) };
        if index < 5 {
            session.set_key_touch(Some(crate::touch::KeyTouch { dx, dy, copy: 0 }));
        }
        session.character(*byte, false);
    }
    assert!(!first_is_confident(&session));
    session.command(Command::Cancel);
    type_touched(&mut session, true);
    assert!(first_is_confident(&session));
    // 用户按原样上屏：记抑制，之后同一串输入按字面回答、请求不带触点。
    session.command(Command::CommitRaw);
    type_touched(&mut session, true);
    assert!(request_touches(&session).is_empty());
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .all(|row| !row.confident_correction && row.corrected_from.is_empty()));
    session.command(Command::Cancel);

    // 本地模式（超级简拼）里同样的按键带同样的触点：触点不进方案，没有一行坐首选。
    let mut local = fixture.session_with(touch_shuangpin);
    local.set_key_geometry(Some(qwerty_geometry()));
    local.set_touch_tuning(lead_tuning());
    local.character(b'J', true);
    type_touched(&mut local, true);
    let snapshot = local.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::SuperJianpin);
    assert!(snapshot
        .candidates
        .iter()
        .all(|row| !row.confident_correction));

    let mut flat = fixture.session_with(touch_shuangpin);
    flat.set_touch_tuning(lead_tuning());
    type_text(&mut flat, "mwgfxi");
    assert!(flat
        .snapshot()
        .candidates
        .iter()
        .all(|row| !row.confident_correction));
}

/// 日志里有双拼的接受、拒绝和抑制记录，不带触点的会话（桌面、实体键盘）逐字节和没有这些记录时一样。
#[test]
fn shuangpin_typo_learning_never_reaches_a_session_without_touches() {
    let learned = touch_learning_fixture();
    let profile =
        crate::user_dictionary::typo_profile::PersonalTypoProfile::shared(&learned.journal());
    for outcome in [
        crate::user_dictionary::typo_profile::ShuangpinTypoOutcome::Accepted,
        crate::user_dictionary::typo_profile::ShuangpinTypoOutcome::Accepted,
        crate::user_dictionary::typo_profile::ShuangpinTypoOutcome::Accepted,
        crate::user_dictionary::typo_profile::ShuangpinTypoOutcome::Rejected,
    ] {
        profile
            .record_shuangpin(
                crate::types::ShuangpinProfileKind::Xiaohe,
                &[(*b"gf", *b"gr")],
                outcome,
            )
            .unwrap();
    }
    profile
        .record_shuangpin_suppression(crate::types::ShuangpinProfileKind::Xiaohe, "mwgfxi")
        .unwrap();
    let fresh = touch_learning_fixture();
    let typed = |fixture: &Fixture| {
        let mut session = fixture.session_with(touch_shuangpin);
        type_text(&mut session, "mwgfxi");
        session.snapshot()
    };
    let snapshot = typed(&learned);
    assert!(snapshot
        .candidates
        .iter()
        .any(|item| !item.corrected_from.is_empty()));
    assert_eq!(snapshot, typed(&fresh));
}

#[test]
fn caret_commands_redecode_and_selection_returns_to_the_end() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "i");
    session.command(Command::MoveHome);
    assert!(session.character(b'n', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "ni");
    assert_eq!(snapshot.caret_position, 1);
    // The caret sits inside the only unit, so there is no complete prefix to decode and the whole input answers.
    let whole: Vec<String> = snapshot
        .candidates
        .into_iter()
        .map(|item| item.word.into_string())
        .collect();
    assert_eq!(whole.first().map(String::as_str), Some("你"));
    session.command(Command::MoveEnd);
    assert_eq!(words(&session), whole);
    session.command(Command::Cancel);

    type_text(&mut session, "nihao");
    let full = words(&session);
    let prefix_index = index_of(&session, "你");
    session.command(Command::MoveHome);
    assert_eq!(words(&session), full);
    session.command(Command::MoveRight);
    assert_eq!(words(&session), full);
    session.command(Command::MoveEnd);
    let selected = session.select(prefix_index);
    assert_eq!(selected.commit.as_deref(), Some("你"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "hao");
    assert_eq!(snapshot.caret_position, 3);
}

#[test]
fn selecting_a_prefix_candidate_exits_prefix_mode() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihaoshijie");
    session.set_caret(Some(6));
    assert_eq!(session.prefix_end(), 5);
    let result = select_word(&mut session, "你好");
    assert_eq!(result.commit.as_deref(), Some("你好"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "shijie");
    assert_eq!(snapshot.caret_position, snapshot.editing_text.len());
    assert_eq!(session.prefix_end(), "shijie".len());
    assert!(words(&session).contains(&"是".to_owned()));
}

/// 904bd0976: the prefix list goes through the same personal context reorder as the whole input, and a pinned leader keeps its seat there too.
#[test]
fn caret_prefix_follows_the_personal_context_order_and_its_pins() {
    let fixture = Fixture::new(CONTEXT_FIXTURE);
    let mut session = fixture.session();
    // 子 is followed by 乙, never by 甲, however often.
    for _ in 0..4 {
        type_text(&mut session, "hao");
        select_word(&mut session, "子");
        type_text(&mut session, "ni");
        select_word(&mut session, "乙");
        session.punctuation(b',');
    }
    type_text(&mut session, "ni");
    assert_eq!(words(&session).first().map(String::as_str), Some("甲"));
    session.command(Command::Cancel);

    type_text(&mut session, "hao");
    select_word(&mut session, "子");
    type_text(&mut session, "nihao");
    session.set_caret(Some(2));
    assert_eq!(session.prefix_end(), 2);
    assert_eq!(
        words(&session).first().map(String::as_str),
        Some("乙"),
        "after 子 the prefix ni leads with 乙: {:?}",
        words(&session)
    );
    let result = session.pin(index_of(&session, "甲"));
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(session.prefix_end(), 2);
    assert_eq!(
        words(&session).first().map(String::as_str),
        Some("甲"),
        "the pinned leader was moved: {:?}",
        words(&session)
    );
}

/// Thirty `ni` characters, more than the 24 a lone initial lists before expansion, plus the `ni'hao` rows.
fn initial_fixture() -> Fixture {
    let rows: Vec<String> = (0..30)
        .map(|i| {
            format!(
                "('ni','n','{}',{})",
                char::from_u32(0x4e00 + i).unwrap(),
                1000 - i
            )
        })
        .collect();
    Fixture::new(&format!(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES{};\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',100);\
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',5000);",
        rows.join(",")
    ))
}

/// Paging past a lone initial's capped prefix list widens that list, the one on screen, not the hidden whole-input list.
#[test]
fn caret_prefix_expands_its_own_initial_list() {
    let fixture = initial_fixture();
    let mut session = fixture.session();
    type_text(&mut session, "nhao");
    let full = words(&session);
    session.set_caret(Some(1));
    assert_eq!(session.prefix_end(), 1);
    let capped = session.snapshot().candidates;
    assert_eq!(capped.len(), 24);
    assert!(capped.iter().all(|item| item.pinyin == "n"));

    let (grew, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.expand_initial_candidates());
    assert!(grew);
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(allocations, 207, "光标前缀展开申请次数：{allocations}");
    let widened = words(&session);
    assert_eq!(widened.len(), 30);
    assert!(widened.contains(&"丝".to_owned()), "{widened:?}");
    assert_eq!(session.prefix_end(), 1);
    assert!(
        !session.expand_initial_candidates(),
        "the widened list is not capped any more"
    );

    session.command(Command::MoveEnd);
    assert_eq!(words(&session), full);
    assert_eq!(full.first().map(String::as_str), Some("你好"));
}

/// Expanding the whole input's list writes the whole input's series slot, not the slot of the caret prefix decoded just before it on the same dictionary.
#[test]
fn a_whole_input_expansion_leaves_the_prefix_slot_alone() {
    let fixture = initial_fixture();
    let mut session = fixture.session();
    type_text(&mut session, "nhao");
    session.set_caret(Some(1));
    let prefix = words(&session);
    assert_eq!(prefix.len(), 24);
    session.command(Command::MoveEnd);
    assert!(session.expand_initial_candidates());
    assert!(words(&session).len() > prefix.len() + 1);

    session.set_caret(Some(1));
    assert_eq!(words(&session), prefix);
    session.command(Command::Cancel);
    type_text(&mut session, "n");
    assert_eq!(words(&session), prefix);
}

/// A removal or pin made while the prefix is decoded re-queries the prefix instead of showing the list from before the edit.
#[test]
fn caret_prefix_requeries_after_a_removal_a_pin_and_a_fixed_position() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100),('ni''hao','nh','拟好',90),('ni''hao','nh','泥好',80),('ni''hao','nh','妮好',70);\
CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_s VALUES('shi''jie','sj','世界',100);",
    );
    let mut session = fixture.session();
    type_text(&mut session, "nihaoshijie");
    session.set_caret(Some(5));
    assert_eq!(session.prefix_end(), 5);
    assert_eq!(words(&session)[..4], ["你好", "拟好", "泥好", "妮好"]);

    let result = session.remove(index_of(&session, "你好"));
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(session.prefix_end(), 5);
    assert!(
        !words(&session).contains(&"你好".to_owned()),
        "{:?}",
        words(&session)
    );

    // With no previous commit the personal context cannot reorder the list, so only a re-query shows the new weight.
    let result = session.pin(index_of(&session, "泥好"));
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(session.prefix_end(), 5);
    assert_eq!(words(&session).first().map(String::as_str), Some("泥好"));

    let result = session.fix_position(index_of(&session, "妮好"), 1);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(session.prefix_end(), 5);
    assert_eq!(words(&session).first().map(String::as_str), Some("妮好"));
}

/// Mixed English rows are looked up for the decoded prefix, not for the whole raw input.
#[test]
fn caret_prefix_mixes_english_for_the_prefix_only() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('nice','nice',100);");
    let mut session = fixture.session_with(|options| options.english.mixed_candidates = true);
    type_text(&mut session, "nihao");
    assert!(!words(&session).contains(&"nice".to_owned()));
    session.set_caret(Some(2));
    assert_eq!(session.prefix_end(), 2);
    assert!(
        words(&session).contains(&"nice".to_owned()),
        "{:?}",
        words(&session)
    );
    session.command(Command::MoveEnd);
    assert!(!words(&session).contains(&"nice".to_owned()));
}

/// 真实词库里 `hello` 拆不开（he'l'lo），全拼只能给出 和、何 这类前半截的字；英文库里正好有这个词。
const HELLO_FIXTURE: &str = "CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('he','h','和',200);\
INSERT INTO tbl_1_h VALUES('he','h','何',100);\
CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_l VALUES('lo','l','咯',100);";

fn candidate_sources(session: &Session) -> Vec<CandidateSource> {
    session
        .snapshot()
        .candidates
        .into_iter()
        .map(|item| item.source)
        .collect()
}

#[test]
fn short_exact_mixed_english_keeps_chinese_first_and_commits_without_a_mode_switch() {
    let fixture = Fixture::new(HELLO_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('he','HE',100),('hello','Hello',1000);");
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.english.mixed_candidates = true;
            options.english.minimum_prefix = 5;
            options.helpcode = false;
        });
        for _ in 0..2 {
            type_text(&mut session, "he");
            assert_eq!(&words(&session)[..2], ["和", "HE"]);
            assert!(!words(&session).contains(&"Hello".to_owned()));
            assert_eq!(session.select(1).commit.as_deref(), Some("HE"));
            assert!(session.snapshot().preedit.is_empty());
        }
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.english.mixed_candidates = false;
            options.english.minimum_prefix = 5;
            options.helpcode = false;
        });
        type_text(&mut session, "he");
        assert!(!words(&session).contains(&"HE".to_owned()));
    }
}

/// 拼音拆不开、英文完全匹配整段输入时英文占首位，空格上屏英文；结束组字的标点也整段上屏这个英文词。能切成完整音节的 `he` 照旧是中文首选。
#[test]
fn an_exact_english_word_leads_input_pinyin_cannot_spell() {
    let fixture = Fixture::new(HELLO_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('hello','hello',100);\
INSERT INTO english_words VALUES('he','he',500);",
    );
    let mut session = fixture.session_with(|options| options.english.mixed_candidates = true);
    type_text(&mut session, "hello");
    assert_eq!(words(&session)[0], "hello", "{:?}", words(&session));
    assert_eq!(
        candidate_sources(&session)[0],
        CandidateSource::EnglishDictionary
    );
    assert!(
        candidate_sources(&session)[1..]
            .iter()
            .any(|source| *source != CandidateSource::EnglishDictionary),
        "{:?}",
        words(&session)
    );
    // 空格选的是首行。
    let result = session.select(0);
    assert_eq!(result.commit.as_deref(), Some("hello"), "{result:?}");

    type_text(&mut session, "hello");
    assert_eq!(words(&session)[0], "hello", "{:?}", words(&session));
    let result = session.punctuation(b',');
    assert_eq!(result.commit.as_deref(), Some("hello，"), "{result:?}");

    type_text(&mut session, "he");
    assert_eq!(words(&session)[0], "和", "{:?}", words(&session));
    assert_eq!(words(&session)[1], "he", "{:?}", words(&session));
}

#[test]
fn capital_pinyin_spelling_keeps_the_typed_latin_word() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
        });
        let text = if scheme == SchemeType::Quanpin {
            "NIHAO"
        } else {
            "NIHC"
        };
        for byte in text.bytes() {
            assert!(session.character(byte, true).handled);
        }
        assert_eq!(words(&session).first().map(String::as_str), Some(text));
        assert!(!words(&session).iter().any(|word| word.contains("你好")));
        assert_eq!(session.select(0).commit.as_deref(), Some(text));
    }
}

#[test]
fn capital_literal_prefix_keeps_the_following_lowercase_chinese_conversion() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (scheme, tail) in [
        (SchemeType::Quanpin, "nihao"),
        (SchemeType::Shuangpin, "nihc"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
        });
        assert!(session.character(b'F', true).handled);
        type_text(&mut session, tail);
        assert!(
            words(&session).contains(&"F你好".to_owned()),
            "{:?}",
            words(&session)
        );
        assert!(!words(&session).contains(&"你好".to_owned()));
        let index = words(&session)
            .iter()
            .position(|word| word == "F你好")
            .unwrap();
        assert_eq!(session.select(index).commit.as_deref(), Some("F你好"));
    }
}

#[test]
fn capital_literal_partial_selection_consumes_only_its_raw_span_and_does_not_learn() {
    for (scheme, input, rest) in [
        (SchemeType::Quanpin, "Fnihao", "hao"),
        (SchemeType::Shuangpin, "Fnihc", "hc"),
    ] {
        let fixture = Fixture::new(QUANPIN_FIXTURE);
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
        });
        assert!(session.character(b'F', true).handled);
        type_text(&mut session, &input[1..]);
        let journal_before = journal_rows(&fixture.journal());
        let index = words(&session)
            .iter()
            .position(|word| word == "F你")
            .unwrap();
        assert!(!session.snapshot().candidate_answers_key[index]);
        assert!(!session.pin(index).handled);
        assert!(!session.remove(index).handled);
        assert_eq!(session.select(index).commit.as_deref(), Some("F你"));
        assert_eq!(session.snapshot().editing_text, rest);
        assert_eq!(journal_rows(&fixture.journal()), journal_before);
        assert_eq!(words(&session)[0], "好");
        assert_eq!(session.select(0).commit.as_deref(), Some("好"));
    }
}

#[test]
fn capital_literal_manual_delimiters_are_consumed_without_becoming_commit_text() {
    for (scheme, input, expected) in [
        (SchemeType::Quanpin, "F'ni'hao", "F你好"),
        (SchemeType::Shuangpin, "F'ni'hc", "F你好"),
        (SchemeType::Quanpin, "ni'hao'F", "你好F"),
        (SchemeType::Shuangpin, "ni'hc'F", "你好F"),
    ] {
        let fixture = Fixture::new(QUANPIN_FIXTURE);
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
        });
        assert!(session.character(input.as_bytes()[0], true).handled);
        type_text(&mut session, &input[1..]);
        assert_eq!(words(&session)[0], expected);
        assert_eq!(session.select(0).commit.as_deref(), Some(expected));
        assert!(session.snapshot().editing_text.is_empty());
    }
}

#[test]
fn capital_literal_segment_boundaries_keep_lowercase_syllables() {
    for (scheme, input, expected) in [
        (SchemeType::Quanpin, "FnihaoX", vec![0, 1, 3, 6, 7]),
        (SchemeType::Shuangpin, "FnihcX", vec![0, 1, 3, 5, 6]),
    ] {
        let fixture = Fixture::new(QUANPIN_FIXTURE);
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
        });
        assert!(session.character(b'F', true).handled);
        type_text(&mut session, &input[1..]);
        assert_eq!(session.segment_raw_boundaries(), expected);
    }
}

#[test]
fn capital_literal_caret_selection_preserves_the_unselected_suffix() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (scheme, input, caret, rest) in [
        (SchemeType::Quanpin, "FniXhao", 3, "Xhao"),
        (SchemeType::Shuangpin, "FniXhc", 3, "Xhc"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
        });
        assert!(session.character(b'F', true).handled);
        type_text(&mut session, &input[1..]);
        session.set_caret(Some(caret));
        assert_eq!(words(&session)[0], "F你");
        assert_eq!(session.select(0).commit.as_deref(), Some("F你"));
        assert_eq!(session.snapshot().editing_text, rest);
        assert_eq!(
            session.command(Command::CommitCandidate).commit.as_deref(),
            Some("X好")
        );
    }
}

#[test]
fn capital_literal_deletion_reenables_lowercase_conversion() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.helpcode = false);
    assert!(session.character(b'F', true).handled);
    type_text(&mut session, "nihao");
    session.set_caret(Some(1));
    assert!(session.command(Command::Backspace).handled);
    session.set_caret(None);
    assert_eq!(session.snapshot().editing_text, "nihao");
    assert_eq!(words(&session)[0], "你好");
    assert!(session.online_query().is_some());
}

#[test]
fn capital_literal_rejects_old_online_answers_and_finishes_without_rewriting_capitals() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (scheme, input) in [
        (SchemeType::Quanpin, "niXhao"),
        (SchemeType::Shuangpin, "niXhc"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
        });
        type_text(&mut session, "ni");
        let query = session.online_query().unwrap();
        assert!(session.character(b'X', true).handled);
        type_text(&mut session, &input[3..]);
        assert!(session.online_query().is_none());
        assert!(!session.apply_online_candidate(
            &query,
            "合成旧响应",
            CandidateSource::CloudSuggestion
        ));
        assert_eq!(session.finish(0).commit.as_deref(), Some("你X好"));
        assert!(session.snapshot().preedit.is_empty());
    }
}

#[test]
fn capital_literal_english_word_wins_over_chinese_and_keeps_completion() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('python','python',100),('pythonic','pythonic',90);",
    );
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
            options.english.mixed_candidates = true;
        });
        assert!(session.character(b'P', true).handled);
        type_text(&mut session, "ython");
        assert_eq!(words(&session)[0], "Python");
        assert_eq!(
            words(&session)
                .iter()
                .filter(|word| *word == "Python")
                .count(),
            1
        );
        assert!(words(&session).contains(&"Pythonic".to_owned()));
        assert_eq!(session.select(0).commit.as_deref(), Some("Python"));
    }
}

#[test]
fn mixed_master_switch_keeps_capital_literal_without_dictionary_completions() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('python','python',100),('pythonic','pythonic',90);",
    );
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
            options.mixed_candidates = false;
            options.english.mixed_candidates = true;
        });
        assert!(session.character(b'P', true).handled);
        type_text(&mut session, "ython");
        assert!(!session
            .snapshot()
            .candidates
            .iter()
            .any(|row| row.source == CandidateSource::EnglishDictionary));
        assert!(!words(&session).contains(&"Pythonic".to_owned()));
        let index = index_of(&session, "Python");
        assert_eq!(session.select(index).commit.as_deref(), Some("Python"));
    }
}

#[test]
fn capital_literal_english_case_does_not_mistake_the_word_for_helpcode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('hao','hao',100);");
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.english.mixed_candidates = true;
    });
    assert!(session.character(b'H', true).handled);
    type_text(&mut session, "AO");
    assert_eq!(words(&session)[0], "HAO");
    assert_eq!(session.select(0).commit.as_deref(), Some("HAO"));
}

#[test]
fn shifted_uppercase_starts_pinyin_and_mixes_the_cased_english_word() {
    let fixture = Fixture::new(HELLO_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('observable','observable',100);");
    let mut session = fixture.session_with(|options| {
        options.english.mixed_candidates = true;
        options.english.minimum_prefix = 1;
    });

    assert!(session.character(b'O', true).handled);
    assert_eq!(session.snapshot().preedit, "O");
    type_text(&mut session, "bservable");
    assert!(words(&session).contains(&"Observable".to_owned()));
    assert_eq!(session.snapshot().preedit, "Observable");
}

#[test]
fn mixed_english_completion_and_commit_keep_typed_case_without_changing_lookup_key() {
    let fixture = Fixture::new(HELLO_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('observable','observable',100),('openai','OpenAI',90);",
    );
    for (input, expected, key) in [
        ("O", "Observable", "observable"),
        ("Ob", "Observable", "observable"),
        ("OBS", "OBSERVABLE", "observable"),
        ("oBs", "oBservable", "observable"),
        ("open", "OpenAI", "openai"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.english.mixed_candidates = true;
            options.english.minimum_prefix = 1;
            options.helpcode = false;
        });
        assert!(session.character(input.as_bytes()[0], true).handled);
        type_text(&mut session, &input[1..]);
        let snapshot = session.snapshot();
        let index = snapshot
            .candidates
            .iter()
            .position(|item| {
                item.source == CandidateSource::EnglishDictionary && item.word == expected
            })
            .unwrap_or_else(|| panic!("英文补全未保留 {input:?} 的大小写：{:?}", words(&session)));
        assert_eq!(snapshot.candidates[index].pinyin, key);
        assert_eq!(snapshot.preedit, input);
        assert_eq!(session.select(index).commit.as_deref(), Some(expected));
    }
}

#[test]
fn mixed_english_case_preserves_dictionary_identity_for_frequency_learning() {
    let fixture = Fixture::new(HELLO_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('observer','observer',200),('observable','observable',100);",
    );
    let mut session = fixture.session_with(|options| {
        options.english.mixed_candidates = true;
        options.english.minimum_prefix = 1;
        options.helpcode = false;
        options.frequency.mode = FrequencyAdjustmentMode::Linear;
    });
    assert!(session.character(b'O', true).handled);
    type_text(&mut session, "b");
    let index = words(&session)
        .iter()
        .position(|word| word == "Observable")
        .unwrap();
    let promoted = session.pin(index);
    assert!(promoted.handled);
    assert_eq!(promoted.diagnostic, None);
    let index = words(&session)
        .iter()
        .position(|word| word == "Observable")
        .unwrap();
    let result = session.select(index);
    assert_eq!(result.commit.as_deref(), Some("Observable"));
    assert_eq!(result.diagnostic, None);
    let (display, weight): (String, i64) =
        Connection::open(fixture.path().join(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .query_row(
                "SELECT display, weight FROM english_words WHERE word='observable'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
    assert_eq!(display, "observable");
    assert!(weight > 100);
}

#[test]
fn disabled_shift_shortcuts_leave_every_capital_available_for_pinyin() {
    let fixture = Fixture::new(HELLO_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        for letter in b'A'..=b'Z' {
            let mut session = fixture.session_with(|options| {
                options.scheme = scheme;
                options.local_modes = crate::types::LocalModeOptions {
                    unicode: false,
                    date_time: false,
                    date_time_inline: false,
                    quick_phrase: false,
                    emoji: false,
                    kaomoji: false,
                    symbol: false,
                    super_jianpin: false,
                    temporary_english: false,
                    temporary_japanese: false,
                    ..Default::default()
                };
            });
            assert!(
                session.character(letter, true).handled,
                "{scheme:?}: {}",
                letter as char
            );
            let snapshot = session.snapshot();
            assert_eq!(snapshot.local_mode, LocalInputMode::None);
            assert_eq!(snapshot.preedit, (letter as char).to_string());
            assert_eq!(
                session.command(Command::CommitRaw).commit.as_deref(),
                Some((letter as char).to_string().as_str())
            );
        }
    }
}

#[test]
fn punctuation_after_unspellable_shifted_word_commits_typed_text() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.english.mixed_candidates = true;
        options.english.minimum_prefix = 1;
    });

    assert!(session.character(b'N', true).handled);
    type_text(&mut session, "ih");

    let result = session.punctuation(b'<');
    assert_eq!(result.commit.as_deref(), Some("Nih《"), "{result:?}");
}

#[test]
fn punctuation_after_capitalized_spelling_keeps_literal_capital() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();

    assert!(session.character(b'N', true).handled);
    type_text(&mut session, "i");

    let result = session.punctuation(b'<');
    assert_eq!(result.commit.as_deref(), Some("Ni《"), "{result:?}");
}

/// 辅助码激活时大写的辅助码被转成小写拼进查英文的输入（`niC` 查的是 `nic`），光标前缀解码时选英文会扔掉光标后的输入：两种情况英文都不领先。
#[test]
fn helpcode_and_caret_prefix_keep_an_exact_english_word_behind_the_first_seat() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('nic','nic',100);");
    let mut session = fixture.session_with(|options| options.english.mixed_candidates = true);
    // 对照：`nic` 拆不开，英文领先。
    type_text(&mut session, "nic");
    assert_eq!(words(&session)[0], "nic", "{:?}", words(&session));
    session.command(Command::Cancel);

    type_text(&mut session, "ni");
    assert!(session.character(b'C', false).handled);
    assert!(session.input.has_active_helpcode());
    assert_ne!(words(&session)[0], "nic", "{:?}", words(&session));
    assert!(
        words(&session).contains(&"nic".to_owned()),
        "{:?}",
        words(&session)
    );
    session.command(Command::Cancel);

    type_text(&mut session, "nicnihao");
    session.set_caret(Some(3));
    assert_eq!(session.prefix_end(), 3);
    assert!(session.input.prefix_active);
    assert_ne!(words(&session)[0], "nic", "{:?}", words(&session));
    assert!(
        words(&session).contains(&"nic".to_owned()),
        "{:?}",
        words(&session)
    );
}

/// 英文领先时紧跟着的第二行才是拼音的首选，选它仍是默认选择：不调词频，不写日志。
#[test]
fn picking_the_chinese_row_behind_a_leading_english_word_is_the_default_pick() {
    let fixture = Fixture::new(HELLO_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('hello','hello',100);");
    let mut session = fixture.session_with(|options| {
        options.english.mixed_candidates = true;
        options.personal_context = false;
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
        options.frequency.trigger_count = 1;
    });
    type_text(&mut session, "hello");
    assert_eq!(
        candidate_sources(&session)[0],
        CandidateSource::EnglishDictionary
    );
    let index = session.input.ranking_index(1).unwrap();
    assert_eq!(index, 1);
    assert!(session.input.ranking_list()[1].source.is_dictionary());

    let (learned, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.input.learn_candidate(index));

    assert_eq!(learned, None);
    assert_eq!(allocations, 0, "默认选择走了调词频：{allocations} 次分配");
    assert!(!fixture.journal().exists());
}

// ---- 日文混输（实验选项）----

/// 日文混输的拼音词库：`kaishi` 有整段的词 开始，`ohayou`、`sugoi`、`tabemasu` 只有单字。
const JAPANESE_MIXING_FIXTURE: &str =
    "CREATE TABLE tbl_2_k(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_k VALUES('kai''shi','ks','开始',300);\
CREATE TABLE tbl_1_k(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_k VALUES('kai','k','开',100);\
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shi','s','是',100),('su','s','苏',100);\
CREATE TABLE tbl_1_o(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_o VALUES('o','o','哦',100);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('ha','h','哈',100);\
CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_y VALUES('you','y','有',100),('yo','y','哟',90);\
CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_t VALUES('ta','t','他',100);\
CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('ma','m','吗',100);";

/// 日文模型：おはよう、かいし（開始）、すごい有完整词条，食べ和ます拼出 `tabemasu`；没有にはお之类的词条，`nihao` 只能靠未知假名拼出来。かい、しょう是两拍的词条；すこし（少し）的成本超过「认得」的上限（每假名 1800）。
fn japanese_mixing_model() -> Vec<u8> {
    crate::japanese::decoder::test_model::bytes(
        &[
            ("おはよう", "おはよう", 0, 0, 500),
            ("かい", "会", 0, 0, 400),
            ("かいし", "開始", 0, 0, 800),
            ("しょう", "章", 0, 0, 400),
            ("すこし", "少し", 0, 0, 6000),
            ("すごい", "すごい", 0, 0, 600),
            ("たべ", "食べ", 0, 0, 700),
            ("ます", "ます", 0, 0, 300),
        ],
        1,
        &[0],
    )
}

fn japanese_mixing_fixture() -> Fixture {
    let fixture = Fixture::new(JAPANESE_MIXING_FIXTURE);
    std::fs::write(
        fixture.path().join(assets::JAPANESE_MODEL),
        japanese_mixing_model(),
    )
    .unwrap();
    fixture
}

fn japanese_session(
    fixture: &Fixture,
    configure: impl FnOnce(&mut SessionOptions),
) -> (
    Session,
    Option<std::sync::Arc<crate::japanese::decoder::JapaneseDictionary>>,
) {
    // 行为断言持有已就绪的模型，避免并行用例的全局清缓存把它在首个查询前释放。
    let model = crate::japanese::decoder::JapaneseDictionary::shared(
        &fixture.path().join(assets::JAPANESE_MODEL),
    );
    let session = fixture.session_with(|options| {
        options.japanese_mixing = true;
        configure(options);
    });
    (session, model)
}

/// 列表里日文混输行的下标；没有时为 `None`，有两行就失败。
fn japanese_index(session: &Session) -> Option<usize> {
    let sources = candidate_sources(session);
    let mut rows = sources
        .iter()
        .enumerate()
        .filter(|(_, source)| **source == CandidateSource::MixedJapanese)
        .map(|(index, _)| index);
    let first = rows.next();
    assert_eq!(rows.next(), None, "{:?}", words(session));
    first
}

/// 拼音给不出整段的词（`ohayou` 只有单字凑出来的读法）时日文领先；`kaishi` 有整段的词 开始，日文进优先位置；拼音拆不开的 `sugoi` 领先。行的各字段按约定填写。
#[test]
fn a_japanese_conversion_leads_or_follows_the_whole_chinese_word() {
    let fixture = japanese_mixing_fixture();
    let (mut session, _japanese_model) = japanese_session(&fixture, |_| {});

    type_text(&mut session, "ohayou");
    assert_eq!(japanese_index(&session), Some(0), "{:?}", words(&session));
    let row = session.snapshot().candidates[0].clone();
    assert_eq!(row.word, "おはよう");
    assert_eq!(row.pinyin, "ohayou");
    assert_eq!(row.canonical_pinyin, "");
    assert_eq!(row.weight, 0);
    assert_eq!(row.scheme, SchemeType::JapaneseRomaji);
    assert!(session.snapshot().candidate_answers_key[0]);
    session.command(Command::Cancel);

    type_text(&mut session, "kaishi");
    assert_eq!(
        &words(&session)[..2],
        ["开始", "開始"],
        "{:?}",
        words(&session)
    );
    assert_eq!(japanese_index(&session), Some(1));
    session.command(Command::Cancel);

    type_text(&mut session, "sugoi");
    assert_eq!(words(&session)[0], "すごい", "{:?}", words(&session));
    session.command(Command::Cancel);

    // 没有精确词条：拼音拆不开且整句的每个假名都认得时才给，`tabemasu` 给出 食べます。
    type_text(&mut session, "tabemasu");
    assert_eq!(words(&session)[0], "食べます", "{:?}", words(&session));
    session.command(Command::Cancel);

    // `nihao` 拼得开，又没有精确词条；`hello` 拼不开，但整句要靠未知假名：都不给。
    for raw in ["nihao", "hello"] {
        type_text(&mut session, raw);
        assert_eq!(
            japanese_index(&session),
            None,
            "{raw}: {:?}",
            words(&session)
        );
        session.command(Command::Cancel);
    }
}

/// 两拍的精确词条不算精确：拼得开的 `kai`（かい）、`shou`（しょう，拗音的小写假名不单算拍）都不混日文。拼音拆不开时只有精确词条、整句不认得的 `sukoshi`（少し）给一行但不领先，进优先位置。
#[test]
fn short_exact_readings_stay_out_and_unknown_readings_do_not_lead() {
    let fixture = japanese_mixing_fixture();
    let (mut session, _japanese_model) = japanese_session(&fixture, |_| {});
    for raw in ["kai", "shou"] {
        type_text(&mut session, raw);
        assert_eq!(
            japanese_index(&session),
            None,
            "{raw}: {:?}",
            words(&session)
        );
        session.command(Command::Cancel);
    }

    type_text(&mut session, "sukoshi");
    assert_eq!(japanese_index(&session), Some(1), "{:?}", words(&session));
    assert_eq!(words(&session)[1], "少し");
}

/// 日文混输的门槛：选项关着、会话不带日文、模型缺失、带撇号、不到两个假名、末尾还有待定字母、辅助码激活、光标前缀解码、专用英文，都不混日文。
#[test]
fn japanese_mixing_stays_out_unless_every_gate_passes() {
    let fixture = japanese_mixing_fixture();
    let mut off = fixture.session();
    type_text(&mut off, "ohayou");
    assert_eq!(japanese_index(&off), None);

    let (mut quanpin_only, _japanese_model) = japanese_session(&fixture, |options| {
        options.enabled_schemes = SchemeSet::of(&[SchemeType::Quanpin]);
    });
    type_text(&mut quanpin_only, "ohayou");
    assert_eq!(japanese_index(&quanpin_only), None);

    let missing = Fixture::new(JAPANESE_MIXING_FIXTURE);
    let (mut missing, _japanese_model) = japanese_session(&missing, |_| {});
    type_text(&mut missing, "ohayou");
    assert_eq!(japanese_index(&missing), None);

    let (mut session, _japanese_model) = japanese_session(&fixture, |_| {});
    for raw in ["kai'shi", "su", "kaish"] {
        type_text(&mut session, raw);
        assert_eq!(
            japanese_index(&session),
            None,
            "{raw}: {:?}",
            words(&session)
        );
        session.command(Command::Cancel);
    }

    // `ohayo` 能切成 o'ha'yo，大写的 U 是辅助码，转成小写正好是 `ohayou`。
    type_text(&mut session, "ohayo");
    assert!(session.character(b'U', false).handled);
    assert!(session.input.has_active_helpcode());
    assert_eq!(japanese_index(&session), None, "{:?}", words(&session));
    session.command(Command::Cancel);

    type_text(&mut session, "kaishi");
    session.set_caret(Some(3));
    assert!(session.input.prefix_active);
    assert_eq!(japanese_index(&session), None, "{:?}", words(&session));
    session.command(Command::Cancel);

    session.set_dedicated_english(true);
    type_text(&mut session, "ohayou");
    assert_eq!(japanese_index(&session), None, "{:?}", words(&session));
}

/// 双拼也混：小鹤下 `sugoi` 是奇数长度，拆不开，日文领先；辅助码开着时第五键是辅助码，不混。
#[test]
fn shuangpin_mixes_a_japanese_conversion_too() {
    let fixture = japanese_mixing_fixture();
    let (mut session, _japanese_model) =
        japanese_session(&fixture, |options| options.scheme = SchemeType::Shuangpin);
    type_text(&mut session, "sugoi");
    // 双拼辅助码开着时奇数长度的第五键算辅助码，不混日文。
    assert!(session.input.has_active_helpcode());
    assert_eq!(japanese_index(&session), None, "{:?}", words(&session));

    let (mut session, _japanese_model) = japanese_session(&fixture, |options| {
        options.scheme = SchemeType::Shuangpin;
        options.helpcode = false;
    });
    type_text(&mut session, "sugoi");
    assert_eq!(japanese_index(&session), Some(0), "{:?}", words(&session));
    assert_eq!(words(&session)[0], "すごい");
}

/// 英文已经因为拼音拆不开、完全匹配占了首位时，领先的日文排在它后面；跟在这两行后面的中文才是默认选择。
#[test]
fn a_leading_japanese_conversion_follows_a_leading_english_word() {
    let fixture = japanese_mixing_fixture()
        .with_english("INSERT INTO english_words VALUES('tabemasu','tabemasu',100);");
    let (mut session, _japanese_model) = japanese_session(&fixture, |options| {
        options.english.mixed_candidates = true;
        options.personal_context = false;
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
    });
    type_text(&mut session, "tabemasu");
    assert_eq!(
        &words(&session)[..2],
        ["tabemasu", "食べます"],
        "{:?}",
        words(&session)
    );
    assert_eq!(
        candidate_sources(&session)[..2],
        [
            CandidateSource::EnglishDictionary,
            CandidateSource::MixedJapanese
        ]
    );
    let index = session.input.ranking_index(2).unwrap();
    assert!(session.input.ranking_list()[index].source.is_dictionary());
    assert_eq!(session.input.learn_candidate(index), None);
    assert!(!fixture.journal().exists());
}

/// 文字和已有候选重复时不加日文行；有云候选时不领先的日文排在云候选后面，领先的仍在首位。
#[test]
fn japanese_rows_skip_duplicates_and_respect_the_online_seats() {
    let fixture = japanese_mixing_fixture();
    Connection::open(fixture.main_db())
        .and_then(|connection| {
            connection.execute_batch("INSERT INTO tbl_2_k VALUES('kai''shi','ks','開始',50);")
        })
        .unwrap();
    let (mut session, _japanese_model) = japanese_session(&fixture, |_| {});
    type_text(&mut session, "kaishi");
    assert_eq!(japanese_index(&session), None, "{:?}", words(&session));
    assert_eq!(
        words(&session)
            .iter()
            .filter(|word| *word == "開始")
            .count(),
        1
    );
    session.command(Command::Cancel);

    let fixture = japanese_mixing_fixture();
    let (mut session, _japanese_model) = japanese_session(&fixture, |_| {});
    type_text(&mut session, "kaishi");
    let query = session.online_query().expect("全拼查询");
    assert!(session.apply_online_candidates(
        &query,
        &["云候选".to_owned()],
        CandidateSource::CloudSuggestion
    ));
    assert_eq!(
        &words(&session)[..3],
        ["开始", "云候选", "開始"],
        "{:?}",
        words(&session)
    );
    session.command(Command::Cancel);

    type_text(&mut session, "ohayou");
    let query = session.online_query().expect("全拼查询");
    assert!(session.apply_online_candidates(
        &query,
        &["云候选".to_owned()],
        CandidateSource::CloudSuggestion
    ));
    assert_eq!(words(&session)[0], "おはよう", "{:?}", words(&session));
}

/// 选中日文行整段上屏并结束组字：不推进组字、不学习（没有日志）、个人上下文链断开；它不能置顶、固定位置或删除。
#[test]
fn committing_a_japanese_row_ends_the_composition_and_learns_nothing() {
    let fixture = japanese_mixing_fixture();
    let (mut session, _japanese_model) = japanese_session(&fixture, |options| {
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
        options.frequency.trigger_count = 1;
    });
    // 先上屏一个中文词，个人上下文链有了上一个词。
    type_text(&mut session, "kaishi");
    assert_eq!(session.select(0).commit.as_deref(), Some("开始"));
    assert_eq!(session.input.chain.previous.as_deref(), Some("开始"));
    let journal_rows = |fixture: &Fixture| {
        if fixture.journal().exists() {
            count(&fixture.journal(), "SELECT COUNT(*) FROM sqlite_master")
        } else {
            0
        }
    };
    let before = journal_rows(&fixture);

    type_text(&mut session, "kaishi");
    let index = japanese_index(&session).unwrap();
    assert!(!session.pin(index).handled);
    assert!(!session.remove(index).handled);
    assert!(!session.fix_position(index, 1).handled);
    assert_eq!(japanese_index(&session), Some(index));
    assert!(!session
        .input
        .is_editable_source(&session.snapshot().candidates[index]));
    let ranking = session.input.ranking_index(index).unwrap();
    assert_eq!(session.input.learn_candidate(ranking), None);

    let result = session.select(index);
    assert_eq!(result.commit.as_deref(), Some("開始"));
    let snapshot = session.snapshot();
    assert!(snapshot.editing_text.is_empty() && snapshot.candidates.is_empty());
    assert_eq!(session.input.chain.previous, None);
    assert_eq!(journal_rows(&fixture), before);

    // 领先的日文行，空格（首行）和结束组字的标点都整段上屏。
    type_text(&mut session, "ohayou");
    assert_eq!(session.select(0).commit.as_deref(), Some("おはよう"));
    type_text(&mut session, "ohayou");
    assert_eq!(
        session.punctuation(b',').commit.as_deref(),
        Some("おはよう，")
    );
    assert!(session.snapshot().editing_text.is_empty());
}

/// 日文领先时紧跟着的中文是默认选择：不调词频，不写日志。
#[test]
fn picking_the_chinese_row_behind_a_leading_japanese_row_is_the_default_pick() {
    let fixture = japanese_mixing_fixture();
    let (mut session, _japanese_model) = japanese_session(&fixture, |options| {
        options.personal_context = false;
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
    });
    type_text(&mut session, "tabemasu");
    assert_eq!(japanese_index(&session), Some(0));
    let index = session.input.ranking_index(1).unwrap();
    assert!(session.input.ranking_list()[index].source.is_dictionary());
    assert_eq!(session.input.learn_candidate(index), None);
    assert!(!fixture.journal().exists());
}

/// 日文行不出辅助码：辅助码是给拼音筛汉字的，汉字写的日文词照样不标。
#[test]
fn japanese_rows_carry_no_helpcode() {
    let fixture = Fixture::new(JAPANESE_MIXING_FIXTURE);
    std::fs::write(
        fixture.path().join(assets::JAPANESE_MODEL),
        crate::japanese::decoder::test_model::bytes(&[("すごい", "好", 0, 0, 600)], 1, &[0]),
    )
    .unwrap();
    let (mut session, _japanese_model) = japanese_session(&fixture, |_| {});
    type_text(&mut session, "sugoi");
    let index = japanese_index(&session).unwrap();
    assert_eq!(words(&session)[index], "好");
    assert_eq!(session.snapshot().candidate_annotations[index], "");
}

/// 开着日文混输时建会话就在后台预热日文模型；关着时不碰它。
#[test]
fn japanese_mixing_warms_the_model_when_the_session_is_built() {
    use crate::japanese::decoder::{JapaneseDictionary, SHARED_CACHE_TEST_LOCK};
    let _serial = SHARED_CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = japanese_mixing_fixture();
    let model = fixture.path().join(assets::JAPANESE_MODEL);
    let _off = fixture.session();
    JapaneseDictionary::wait_until_warmed(&model);
    assert!(!JapaneseDictionary::is_shared(&model));
    // 预热专项直接建会话，不经行为夹具预先读取模型。
    let _on = fixture.session_with(|options| options.japanese_mixing = true);
    JapaneseDictionary::wait_until_warmed(&model);
    assert!(JapaneseDictionary::is_shared(&model));
}

/// 出货词库加真实日文模型时日文混输的实际效果（产品默认的混输英文：从 5 个字母起）。需要 `MSIME_EVAL_RESOURCES`（含 `msime-pinyin.db`、`msime-english.db` 的资源目录），日文模型取 `MSIME_JAPANESE_MODEL`，没有时取资源目录里的 `msime-japanese.dat`；缺一样就跳过。`--nocapture` 看每个输入的前几行。
#[test]
fn real_resources_mix_japanese_conversions() {
    let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
        eprintln!("skipped: MSIME_EVAL_RESOURCES is not set to a resource directory");
        return;
    };
    let model = std::env::var_os("MSIME_JAPANESE_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&resources).join(assets::JAPANESE_MODEL));
    if !model.is_file() {
        eprintln!("skipped: no Japanese model at {}", model.display());
        return;
    }
    let user = tempfile::tempdir().expect("user directory");
    let mut prepared = crate::host::prepare_options(
        resources.to_str().expect("UTF-8 resource path"),
        user.path().join("user").to_str().unwrap(),
        user.path().join("cache").to_str().unwrap(),
        "real-japanese-mixing",
    )
    .expect("prepared options");
    prepared.japanese_dictionary = model.to_string_lossy().into_owned();
    prepared.mixed_japanese = true;
    let options = crate::host::options::session_options(&prepared).expect("session options");
    let mut session = Session::new(options).expect("session");
    let mut shown = |raw: &str| {
        type_text(&mut session, raw);
        let snapshot = session.snapshot();
        let head: Vec<String> = snapshot
            .candidates
            .iter()
            .take(5)
            .map(|item| format!("{}({})", item.word, item.source as u8))
            .collect();
        eprintln!("{raw:12} {}", head.join(" "));
        let japanese = snapshot
            .candidates
            .iter()
            .position(|item| item.source == CandidateSource::MixedJapanese);
        let words: Vec<String> = snapshot
            .candidates
            .into_iter()
            .map(|item| item.word.into_string())
            .collect();
        session.command(Command::Cancel);
        (words, japanese)
    };
    for (raw, text) in [
        ("ohayou", "おはよう"),
        ("arigatou", "ありがとう"),
        ("konnnichiha", "こんにちは"),
        ("sugoi", "すごい"),
        ("watashi", "私"),
    ] {
        let (words, japanese) = shown(raw);
        assert_eq!(japanese, Some(0), "{raw}: {words:?}");
        assert_eq!(words[0], text, "{raw}");
    }
    let (words, japanese) = shown("kaishi");
    assert_eq!(&words[..2], ["开始", "開始"], "{words:?}");
    assert_eq!(japanese, Some(1));
    // 两拍的读音（すし、はお、かい、よう、しょう）有词条也不算精确；`wode`、`tamen` 只用来看效果。
    for raw in [
        "sushi",
        "konnichiha",
        "nihao",
        "women",
        "xiexie",
        "shenme",
        "zaijian",
        "hao",
        "kai",
        "you",
        "shou",
    ] {
        let (words, japanese) = shown(raw);
        assert_eq!(japanese, None, "{raw}: {words:?}");
    }
    for raw in ["wode", "tamen"] {
        shown(raw);
    }
}

/// The prefix request carries the session's fuzzy rules.
#[test]
fn caret_prefix_applies_fuzzy_pinyin() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_z VALUES('zhi','z','之',100);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',100);",
    );
    for (rules, expected) in [(crate::types::fuzzy_rule::Z_ZH, true), (0, false)] {
        let mut session = fixture.session_with(|options| options.fuzzy_pinyin.rules = rules);
        type_text(&mut session, "zihao");
        session.set_caret(Some(2));
        assert_eq!(session.prefix_end(), 2, "rules {rules}");
        assert_eq!(
            words(&session).contains(&"之".to_owned()),
            expected,
            "rules {rules}: {:?}",
            words(&session)
        );
    }
}

/// The prefix request carries the autocorrect switches, and the suppression a raw commit of the prefix recorded.
#[test]
fn caret_prefix_applies_autocorrect_and_its_suppression() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',100);",
    );
    let transposition = |options: &mut SessionOptions| {
        options.autocorrect_types = crate::types::autocorrect_type::TRANSPOSITION;
    };
    let mut session = fixture.session_with(transposition);
    type_text(&mut session, "hoani");
    session.set_caret(Some(3));
    assert_eq!(session.prefix_end(), 3);
    assert!(
        words(&session).contains(&"好".to_owned()),
        "{:?}",
        words(&session)
    );
    session.command(Command::Cancel);

    let mut plain = fixture.session();
    type_text(&mut plain, "hoani");
    plain.set_caret(Some(3));
    assert!(
        !words(&plain).contains(&"好".to_owned()),
        "{:?}",
        words(&plain)
    );

    type_text(&mut session, "hoa");
    assert!(words(&session).contains(&"好".to_owned()));
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("hoa")
    );
    type_text(&mut session, "hoani");
    session.set_caret(Some(3));
    assert_eq!(session.prefix_end(), 3);
    assert!(
        !words(&session).contains(&"好".to_owned()),
        "{:?}",
        words(&session)
    );
}

// ---- shuangpin candidate assembly ----

#[test]
fn shuangpin_trailing_initial_prediction_consumes_keys_and_learns_full_reading() {
    for kind in [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
    ] {
        let profile = crate::shuangpin::profile::profile(kind).unwrap();
        let mut encoded = crate::shuangpin::utils::encode_pinyin("ni'hao'te'le", profile).unwrap();
        encoded.pop();
        for manual in [false, true] {
            let fixture = Fixture::new(&format!(
                "{QUANPIN_FIXTURE} CREATE TABLE tbl_4_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
            ));
            let typed = if manual {
                encoded.clone()
            } else {
                encoded.replace('\'', "")
            };
            let mut session = fixture.session_with(|options| {
                options.scheme = SchemeType::Shuangpin;
                options.shuangpin_profile = kind;
                options.helpcode = false;
                options.autocorrect_types = 0;
            });
            type_text(&mut session, &typed);
            assert_eq!(words(&session)[0], "你好特乐", "{kind:?}, {manual}");
            let snapshot = session.snapshot();
            assert_eq!(snapshot.candidates[0].canonical_pinyin, "ni'hao'te'le");
            assert_eq!(snapshot.candidates[0].pinyin, typed.replace('\'', ""));
            let selected = session.select(0);
            assert_eq!(selected.diagnostic, None);
            assert_eq!(selected.commit.as_deref(), Some("你好特乐"));
            assert!(session.snapshot().editing_text.is_empty());
            let journal = Connection::open(fixture.journal()).unwrap();
            let learned: i64 = journal
                .query_row(
                    "SELECT count(*) FROM user_dictionary_operations \
                     WHERE key='ni''hao''te''le' AND value='你好特乐' AND operation<>'delete'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(learned > 0, "{kind:?}, {manual}");
        }
    }
}

#[test]
fn shuangpin_incomplete_tail_selection_preserves_the_unfinished_syllable() {
    for kind in [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
    ] {
        let profile = crate::shuangpin::profile::profile(kind).unwrap();
        let encoded = crate::shuangpin::utils::encode_pinyin("ni'hao'te'le", profile).unwrap();
        for manual in [false, true] {
            let fixture = Fixture::new(&format!(
                "{QUANPIN_FIXTURE} CREATE TABLE tbl_4_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
            ));
            let prefix = if manual {
                encoded.clone()
            } else {
                encoded.replace('\'', "")
            };
            let typed = format!("{prefix}{}q", if manual { "'" } else { "" });
            let mut session = fixture.session_with(|options| {
                options.scheme = SchemeType::Shuangpin;
                options.shuangpin_profile = kind;
                options.helpcode = false;
                options.autocorrect_types = 0;
            });
            type_text(&mut session, &typed);
            assert_eq!(words(&session)[0], "你好特乐", "{kind:?}, {manual}");
            assert_eq!(
                session.snapshot().candidates[0].source,
                CandidateSource::Generated
            );
            let selected = session.select(0);
            assert_eq!(selected.diagnostic, None);
            assert_eq!(selected.commit.as_deref(), Some("你好特乐"));
            assert_eq!(session.snapshot().editing_text, "q", "{kind:?}, {manual}");
        }
    }
}

#[test]
fn complete_shuangpin_and_ambiguous_direct_helpcodes_both_commit_the_whole_input() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_s VALUES('shu','s','书',100),('shu','s','叔',90),\
         ('shang','s','商',100),('su','s','苏',100);\
         CREATE TABLE tbl_3_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_s VALUES('shu''ru''he','srh','书如和',100);\
         CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_s VALUES('shu''shang','ss','叔商',10000),\
         ('su''shang','ss','苏商',10000);",
    );
    for (typed, fuzzy, direct) in [
        ("uuruhe", 0, "叔商"),
        ("suruhe", crate::types::fuzzy_rule::S_SH, "苏商"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
            options.fuzzy_pinyin.rules = fuzzy;
            options.shuangpin_helpcode_modes.direct_helpcode = true;
            options.helpcode_table = Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
                [("叔", "ra"), ("苏", "ra"), ("商", "ea")]
                    .map(|(word, code)| (word.to_owned(), code.to_owned()))
                    .into_iter()
                    .collect(),
            )));
        });
        for expected in ["书如和", direct, "书如和"] {
            type_text(&mut session, typed);
            assert!(index_of(&session, "书如和") < index_of(&session, direct));
            let selected = index_of(&session, expected);
            assert_eq!(session.select(selected).commit.as_deref(), Some(expected));
            assert!(session.snapshot().preedit.is_empty(), "{typed}: {expected}");
        }
    }
}

#[test]
fn ziranma_shouyou_complete_phrases_keep_the_plain_reading_and_commit_once() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_3_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_s VALUES('shu''ru''fa','srf','输入法',100);\
         CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_s VALUES('shu''shen','ss','舒谌',100000);\
         CREATE TABLE tbl_4_k(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_4_k VALUES('kao''shi''fen''shu','ksfs','考试分数',100);\
         CREATE TABLE tbl_3_k(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_k VALUES('kao''shi''fu','ksf','靠势浮',100000);\
         CREATE TABLE tbl_5_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_5_n VALUES('ni''xi''huan''wo''ma','nxhwm','你喜欢我吗',100);\
         CREATE TABLE tbl_4_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_4_n VALUES('ni''chang''wo''ma','ncwm','怩偿我吗',100000);",
    );
    let codes = Arc::new(
        crate::helpcode::HelpcodeKeymap::from_table_bytes(include_bytes!(
            "../../../../resources/helpcodes/shouyou2_0_helpcode.txt"
        ))
        .unwrap(),
    );
    // 使用随产品分发的真实码表；这两个首码正好让三音节输入与两音节直接辅助码重叠。
    assert!(codes.code("舒").unwrap().starts_with('r'));
    assert!(codes.code("谌").unwrap().starts_with('a'));
    let profile = crate::shuangpin::profile::profile(ShuangpinProfileKind::Ziranma).unwrap();
    for enabled in [true, false] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Ziranma;
            options.helpcode = enabled;
            options.helpcode_table = Some(codes.clone());
            options.shuangpin_helpcode_modes.direct_helpcode = true;
            options.fuzzy_pinyin.rules = 0;
        });
        for _ in 0..2 {
            for (reading, expected) in [
                ("shu'ru'fa", "输入法"),
                ("kao'shi'fen'shu", "考试分数"),
                ("ni'xi'huan'wo'ma", "你喜欢我吗"),
            ] {
                let typed = crate::shuangpin::utils::encode_pinyin(reading, profile)
                    .unwrap()
                    .replace('\'', "");
                for byte in typed.bytes() {
                    let result = session.character(byte, false);
                    assert!(
                        result.handled && result.commit.is_none(),
                        "{typed}: {result:?}"
                    );
                }
                assert_eq!(session.snapshot().editing_text, typed);
                assert_eq!(words(&session)[0], expected, "enabled={enabled}: {typed}");
                if enabled && expected == "输入法" {
                    assert!(index_of(&session, expected) < index_of(&session, "舒谌"));
                }
                assert_eq!(session.finish(0).commit.as_deref(), Some(expected));
                assert!(session.snapshot().editing_text.is_empty());
            }
        }
        if enabled {
            type_text(&mut session, "uurufa");
            let direct = index_of(&session, "舒谌");
            assert_eq!(session.select(direct).commit.as_deref(), Some("舒谌"));
            assert!(session.snapshot().editing_text.is_empty());
        }
    }
}

#[test]
fn direct_shuangpin_helpcodes_commit_without_leaving_auxiliary_letters() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_s VALUES('shi','s','实',100);\
         CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_s VALUES('shi''jian','sj','实践',100);\
         CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_j VALUES('jian','j','践',100);\
         CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_t VALUES('ta','t','他',100),('ta','t','她',90),('ta','t','它',80);\
         CREATE TABLE tbl_3_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_t VALUES('ta''ta''ta','ttt','他她它',100);",
    );
    for (typed, expected) in [
        ("uibjm", "实践"),
        ("uibTjm", "实践"),
        ("tartantab", "他她它"),
        ("tartantabA", "他她它"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Ziranma;
            options.shuangpin_helpcode_modes.direct_helpcode = true;
            options.shuangpin_helpcode_modes.direct_helpcode_uppercase = true;
            options.helpcode_table = Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
                [("实", "bt"), ("他", "ra"), ("她", "na"), ("它", "ba")]
                    .map(|(word, code)| (word.to_owned(), code.to_owned()))
                    .into_iter()
                    .collect(),
            )));
        });
        type_text(&mut session, typed);
        let index = index_of(&session, expected);
        assert_eq!(
            session.select(index).commit.as_deref(),
            Some(expected),
            "{typed}"
        );
        assert!(session.snapshot().preedit.is_empty(), "{typed}");
        if typed == "uibjm" {
            type_text(&mut session, typed);
            let prefix = index_of(&session, "实");
            assert_eq!(session.select(prefix).commit.as_deref(), Some("实"));
            assert_eq!(session.input.engine.request().raw_input, "jm");
            let tail = index_of(&session, "践");
            assert_eq!(session.select(tail).commit.as_deref(), Some("践"));
            assert!(session.snapshot().preedit.is_empty());
        }
    }
}

/// Microsoft `ni'nni` (golden ri_microsoft_semicolon_editing step 18): every shorter prefix group answers 你 and 拟, and the trailing `i` is also a single helpcode whose answer appends the whole-input series again, so the reference listed each word four times. A word keeps its first seat (decision 2026-09-30).
#[test]
fn shuangpin_lists_a_word_once() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','你',10000),('ni','n','拟',9000);",
    );
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
    });
    type_text(&mut session, "ni'nni");
    assert_eq!(words(&session), ["你", "拟"]);
}

fn end_trigger_modes(
    semicolon: bool,
    backtick: bool,
    uppercase: bool,
) -> crate::types::ShuangpinHelpcodeModes {
    crate::types::ShuangpinHelpcodeModes {
        direct: false,
        trigger_semicolon: semicolon,
        trigger_backtick: backtick,
        trigger_uppercase: uppercase,
        ..crate::types::ShuangpinHelpcodeModes::default()
    }
}

#[test]
fn shuangpin_manual_tab_filters_only_after_tab() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        options.shuangpin_helpcode_modes.manual_tab = true;
        options.helpcode_table = Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [
                ("你".to_owned(), "na".to_owned()),
                ("拟".to_owned(), "xq".to_owned()),
            ]
            .into_iter()
            .collect(),
        )));
    });
    assert!(!session.character(b'\t', false).handled);
    type_text(&mut session, "nihc");
    assert_eq!(words(&session)[0], "你好");
    assert!(session.character(b'\t', false).handled);
    assert_eq!(session.snapshot().editing_text, "nihc|");
    type_text(&mut session, "x");
    assert_eq!(words(&session)[0], "拟好");
    assert_eq!(session.command(Command::Backspace).commit, None);
    assert!(session.input.engine.request().end_helpcodes.is_empty());
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().editing_text, "nihc");
    type_text(&mut session, "x");
    assert!(session.input.engine.request().end_helpcodes.is_empty());
    assert!(!session.input.has_active_helpcode());
    session.command(Command::Cancel);
    session
        .input
        .engine
        .set_shuangpin_helpcode_modes(crate::types::ShuangpinHelpcodeModes::default());
    type_text(&mut session, "nihcx");
    assert!(session.input.has_active_helpcode());
}

#[test]
fn shuangpin_manual_tab_preserves_microsoft_pairs() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (typed, segmentation) in [
        ("veyige", "zhe'yi'ge"),
        ("uurufa", "shu'ru'fa"),
        ("vijxuuru", "zhi'jie'shu'ru"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
            options.shuangpin_helpcode_modes.manual_tab = true;
        });
        type_text(&mut session, typed);
        assert_eq!(
            session.snapshot().normalized_segmentation,
            segmentation,
            "{typed}"
        );
        assert!(!session.input.has_active_helpcode(), "{typed}");
        assert!(session.character(b'\t', false).handled, "{typed}");
    }
}

#[test]
fn shuangpin_manual_tab_owns_only_complete_normal_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (scheme, enabled, manual, typed, accepts) in [
        (SchemeType::Shuangpin, true, true, "", false),
        (SchemeType::Shuangpin, true, true, "n", false),
        (SchemeType::Shuangpin, true, true, "nihc", true),
        (SchemeType::Shuangpin, false, true, "nihc", false),
        (SchemeType::Shuangpin, true, false, "nihc", false),
        (SchemeType::Quanpin, true, true, "nihao", false),
    ] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = enabled;
            options.shuangpin_helpcode_modes.manual_tab = manual;
        });
        type_text(&mut session, typed);
        assert_eq!(session.snapshot().spelling_symbols.contains('\t'), accepts);
        assert_eq!(session.character(b'\t', false).handled, accepts);
    }
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.helpcode_trigger_key = Some(b'=');
        options.shuangpin_helpcode_modes.manual_tab = true;
    });
    type_text(&mut session, "nihc");
    assert!(!session.snapshot().spelling_symbols.contains('='));
    assert!(!session.character(b'=', false).handled);
    session.command(Command::MoveLeft);
    assert!(!session.snapshot().spelling_symbols.contains('\t'));
    session.command(Command::MoveEnd);
    assert!(session.character(b'\t', false).handled);
    assert!(!session.snapshot().spelling_symbols.contains('\t'));
    type_text(&mut session, "na");
    let result = session.finish(0);
    let commit = result.commit.unwrap();
    assert!(!commit.contains(['\t', '|']));
    session.set_dedicated_english(true);
    type_text(&mut session, "nihc");
    assert!(!session.snapshot().spelling_symbols.contains('\t'));
    session.character(b'\t', false);
    assert_eq!(session.snapshot().editing_text, "nihc");
}

#[test]
fn shuangpin_indirect_triggers_are_published_only_while_they_mark() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    // 末尾直接开着时 `;` 不开末尾间接段，不能发布，否则宿主会把它当输入而不是标点。
    for (direct, marks) in [(false, true), (true, false)] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.helpcode = true;
            options.shuangpin_helpcode_modes.direct = direct;
        });
        assert!(!session.snapshot().spelling_symbols.contains(';'));
        type_text(&mut session, "nihc");
        let symbols = session.snapshot().spelling_symbols;
        assert_eq!(symbols.contains(';'), marks, "direct={direct}");
        assert!(!symbols.contains(['`', '/']), "direct={direct}");
        assert_eq!(
            session.character(b';', false).handled,
            marks,
            "direct={direct}"
        );
        if marks {
            assert_eq!(session.snapshot().editing_text, "nihc|");
            // 一段只认一枚触发键。
            assert!(!session.snapshot().spelling_symbols.contains(';'));
        }
    }
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.helpcode = true;
        options.shuangpin_helpcode_modes.mid_sentence = true;
    });
    type_text(&mut session, "ni");
    assert!(session.snapshot().spelling_symbols.contains('`'));
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.helpcode = true;
        options.shuangpin_helpcode_modes.direct_helpcode = true;
    });
    type_text(&mut session, "nihc");
    assert!(session.snapshot().spelling_symbols.contains('/'));
    // 全拼没有这几种触发方式。
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Quanpin;
        options.helpcode = true;
        options.shuangpin_helpcode_modes.direct = false;
        options.shuangpin_helpcode_modes.mid_sentence = true;
    });
    type_text(&mut session, "nihao");
    assert!(!session
        .snapshot()
        .spelling_symbols
        .contains([';', '`', '/']));
}

#[test]
fn shuangpin_manual_tab_raw_commits_omit_internal_codes() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for finish in [false, true] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_helpcode_modes.manual_tab = true;
        });
        type_text(&mut session, "nihc");
        assert!(session.character(b'\t', false).handled);
        type_text(&mut session, "na");
        let result = if finish {
            session.finish(usize::MAX)
        } else {
            session.command(Command::CommitRaw)
        };
        assert_eq!(result.commit.as_deref(), Some("nihc"));
    }
}

/// 末尾间接触发（`trigger_semicolon`）：开关开着时 `;` 在输入串里记成末尾段 `|`，一码重排、
/// 两码筛选；关着时 `;` 走原路径（小鹤双拼不用这个键，直接还给宿主）。
#[test]
fn shuangpin_end_trigger_semicolon() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100),('ni''hao','nh','泥嚎',90),('ni''hao','nh','鸟蛤',80);",
    );
    let codes = Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
        [
            ("你", "na"),
            ("好", "ha"),
            ("泥", "xq"),
            ("嚎", "yg"),
            ("鸟", "na"),
            ("蛤", "ga"),
        ]
        .map(|(word, code)| (word.to_owned(), code.to_owned()))
        .into_iter()
        .collect(),
    ));
    let session_with_modes = |trigger| {
        let codes = codes.clone();
        fixture.session_with(move |options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
            options.helpcode_table = Some(codes);
            options.shuangpin_helpcode_modes = end_trigger_modes(trigger, false, false);
        })
    };
    // 关着：`;` 不是双拼的键，不进组字。
    let mut off = session_with_modes(false);
    type_text(&mut off, "nihc");
    assert!(!off.character(b';', false).handled);
    assert_eq!(off.snapshot().editing_text, "nihc");
    // 开着：`;` 记成末尾段。
    let mut on = session_with_modes(true);
    type_text(&mut on, "nihc");
    assert_eq!(words(&on)[0], "你好");
    assert!(on.character(b';', false).handled);
    assert_eq!(on.snapshot().editing_text, "nihc|");
    type_text(&mut on, "x");
    assert_eq!(on.snapshot().editing_text, "nihc|x");
    assert_eq!(on.input.engine.request().end_helpcodes, "x");
    // 一码只重排不筛：`泥` 的首码 x 提到最前，其它行还在。
    assert_eq!(words(&on)[0], "泥嚎");
    assert!(words(&on).contains(&"你好".to_owned()));
    type_text(&mut on, "y");
    // 两码筛选：只剩「泥」x +「嚎」y。
    assert_eq!(on.input.engine.request().end_helpcodes, "xy");
    assert_eq!(words(&on), ["泥嚎"]);
    // 段不再位于末尾时只当分隔符，不再筛选。
    type_text(&mut on, "'");
    assert!(on.input.engine.request().end_helpcodes.is_empty());
    assert!(words(&on).contains(&"你好".to_owned()));
}

/// 末尾间接触发的另外两枚触发键：反引号（`trigger_backtick`）记成 `|`；大写触发
/// （`trigger_uppercase`）时大写字母自己开一段，第二码大小写都算。
#[test]
fn shuangpin_end_trigger_backtick_and_uppercase() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100),('ni''hao','nh','泥嚎',90),('ni''hao','nh','鸟蛤',80);",
    );
    let codes = || {
        Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [
                ("你", "na"),
                ("好", "ha"),
                ("泥", "xq"),
                ("嚎", "yg"),
                ("鸟", "na"),
                ("蛤", "ga"),
            ]
            .map(|(word, code)| (word.to_owned(), code.to_owned()))
            .into_iter()
            .collect(),
        )))
    };
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        options.helpcode_table = codes();
        options.shuangpin_helpcode_modes = end_trigger_modes(false, true, true);
    });
    // 反引号记成末尾段 `|`。
    type_text(&mut session, "nihc");
    assert!(session.character(b'`', false).handled);
    assert_eq!(session.snapshot().editing_text, "nihc|");
    type_text(&mut session, "xy");
    assert_eq!(session.input.engine.request().end_helpcodes, "xy");
    assert_eq!(words(&session), ["泥嚎"]);
    assert_eq!(
        session.select(0).commit.as_deref(),
        Some("泥嚎"),
        "选中即上屏，末尾段随整个词一起消耗"
    );
    assert!(session.snapshot().preedit.is_empty());
    // 大写触发：大写字母开段，第二码小写也算。
    type_text(&mut session, "nihcXy");
    assert_eq!(session.snapshot().editing_text, "nihcXy");
    assert_eq!(session.input.engine.request().end_helpcodes, "xy");
    assert_eq!(words(&session), ["泥嚎"]);
    // 大写触发关着时，大写字母走原路径，不产生末尾段。
    let mut off = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        options.helpcode_table = codes();
        options.shuangpin_helpcode_modes = end_trigger_modes(false, false, false);
    });
    type_text(&mut off, "nihcXy");
    assert!(off.input.engine.request().end_helpcodes.is_empty());
}

/// 微软双拼的 `;` 是 `ing` 韵母：片段是奇数键时它仍是韵母，完整音节后才是触发键
/// （`accepts_semicolon_final_at`）。
#[test]
fn shuangpin_end_trigger_semicolon_stays_ing_final() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_b VALUES('bing''jing','bj','冰晶',100),('bing''jing','bj','病晶',90);",
    );
    let codes = Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
        [("冰", "bp"), ("晶", "jb"), ("病", "xp")]
            .map(|(word, code)| (word.to_owned(), code.to_owned()))
            .into_iter()
            .collect(),
    ));
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
        options.helpcode_table = Some(codes);
        options.shuangpin_helpcode_modes = end_trigger_modes(true, false, false);
    });
    type_text(&mut session, "b;j");
    // 片段 `b;j` 是三个键：`;` 补成 `jing`，仍是韵母不是触发键。
    assert!(session.character(b';', false).handled);
    assert_eq!(session.snapshot().editing_text, "b;j;");
    assert_eq!(words(&session)[0], "冰晶");
    // 完整音节后才是触发键。
    assert!(session.character(b';', false).handled);
    assert_eq!(session.snapshot().editing_text, "b;j;|");
    type_text(&mut session, "x");
    assert_eq!(session.input.engine.request().end_helpcodes, "x");
    assert_eq!(words(&session)[0], "病晶");
}

fn mid_modes(on: bool) -> crate::types::ShuangpinHelpcodeModes {
    crate::types::ShuangpinHelpcodeModes {
        mid_sentence: on,
        ..crate::types::ShuangpinHelpcodeModes::default()
    }
}

/// 句中间接（`mid_sentence` + 反引号键）：`` ` `` 把触发键记成 `` ` ``，段绑定它前面的音节，
/// 该位置的字按首码过滤，约束后面还能接着打。开关关着时 `` ` `` 不进组字。
#[test]
fn shuangpin_mid_sentence_backtick_constrains_its_syllable() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_3_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_l VALUES('lao''po''ma','lpm','老婆妈',100),('lao''po''ma','lpm','老破妈',90),('lao''po''ma','lpm','老排妈',80);\
         CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_l VALUES('lao','l','老',100);\
         CREATE TABLE tbl_2_p(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_p VALUES('po''ma','pm','破妈',100),('po''ma','pm','婆妈',90);",
    );
    let codes = || {
        Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [
                ("老", "la"),
                ("婆", "pa"),
                ("破", "xq"),
                ("排", "pa"),
                ("妈", "ma"),
            ]
            .map(|(word, code)| (word.to_owned(), code.to_owned()))
            .into_iter()
            .collect(),
        )))
    };
    // 关着：`` ` `` 不进组字。
    let mut off = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
        options.helpcode_table = codes();
        options.shuangpin_helpcode_modes = mid_modes(false);
    });
    type_text(&mut off, "lkpo");
    assert!(!off.character(b'`', false).handled);
    // 开着：`` ` `` 记段，`x` 约束 `po` 那个音节。
    let mut on = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
        options.helpcode_table = codes();
        options.shuangpin_helpcode_modes = mid_modes(true);
    });
    type_text(&mut on, "lkpo");
    assert!(on.character(b'`', false).handled);
    assert_eq!(on.snapshot().editing_text, "lkpo`");
    type_text(&mut on, "x");
    assert_eq!(
        on.input.engine.request().syllable_helpcodes,
        [crate::types::SyllableHelpcode {
            syllable: 1,
            first: b'x',
            second: 0,
        }]
    );
    type_text(&mut on, "ma");
    assert_eq!(on.snapshot().editing_text, "lkpo`xma");
    // `po` 位置的字按 x 过滤：`老破妈` 留下，`老婆妈`、`老排妈` 出列；
    // `老` 是单字前缀行，约束音节越出它的词长就不拦它。
    assert_eq!(words(&on), ["老破妈", "老"]);
    // 只选第一个音节，剩余的串连同段一起留下，约束重挂到新的第 0 音节上。
    let index = index_of(&on, "老");
    assert_eq!(on.select(index).commit.as_deref(), Some("老"));
    assert_eq!(on.snapshot().editing_text, "po`xma");
    assert_eq!(words(&on), ["破妈"]);
}

/// 句中间接的另外两枚触发键：`mid_sentence_semicolon` 把 `;` 记成 `` ` ``（仍然先让 ing），
/// `mid_sentence_uppercase` 让大写字母在完整音节后开段；第二码大写收窄到两码都要对。
#[test]
fn shuangpin_mid_sentence_semicolon_and_uppercase() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_b VALUES('bing''jing','bj','冰晶',100),('bing''jing','bj','病经',90),('bing''jing','bj','丙京',80);",
    );
    let codes = || {
        Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [
                ("冰", "bp"),
                ("晶", "jb"),
                ("经", "xj"),
                ("京", "xy"),
                ("病", "bp"),
                ("丙", "bp"),
            ]
            .map(|(word, code)| (word.to_owned(), code.to_owned()))
            .into_iter()
            .collect(),
        )))
    };
    let modes = |semicolon, uppercase| crate::types::ShuangpinHelpcodeModes {
        mid_sentence: true,
        mid_sentence_backtick: false,
        mid_sentence_semicolon: semicolon,
        mid_sentence_uppercase: uppercase,
        ..crate::types::ShuangpinHelpcodeModes::default()
    };
    // `;` 记段。
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
        options.helpcode_table = codes();
        options.shuangpin_helpcode_modes = modes(true, false);
    });
    type_text(&mut session, "b;j;");
    assert!(session.character(b';', false).handled);
    assert_eq!(session.snapshot().editing_text, "b;j;`");
    type_text(&mut session, "x");
    assert_eq!(words(&session), ["病经", "丙京"]);
    // 大写开段 + 第二码大写：`XY` 约束 `j;` 位置的字要 x、y 两码都对（经 xj 出列，京 xy 留下）。
    let mut upper = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
        options.helpcode_table = codes();
        options.shuangpin_helpcode_modes = modes(false, true);
    });
    type_text(&mut upper, "b;j;X");
    assert_eq!(upper.snapshot().editing_text, "b;j;X");
    assert_eq!(words(&upper), ["病经", "丙京"]);
    type_text(&mut upper, "Y");
    assert_eq!(words(&upper), ["丙京"]);
    // 退格删码删段，候选回到未筛。
    upper.command(Command::Backspace);
    upper.command(Command::Backspace);
    assert_eq!(upper.snapshot().editing_text, "b;j;");
    assert_eq!(words(&upper)[0], "冰晶");
}

#[test]
fn shuangpin_backtick_two_code_block_preserves_semicolon_final() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_b VALUES('bing''bing','bb','冰冰',100);",
    );
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
        options.helpcode_table = Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [("冰".to_owned(), "xx".to_owned())].into_iter().collect(),
        )));
        options.shuangpin_helpcode_modes = crate::types::ShuangpinHelpcodeModes {
            mid_sentence: true,
            mid_sentence_backtick: true,
            mid_sentence_uppercase: false,
            ..crate::types::ShuangpinHelpcodeModes::default()
        };
    });
    type_text(&mut session, "b;`xXb");
    assert!(session.character(b';', false).handled);
    assert_eq!(session.snapshot().editing_text, "b;`xXb;");
    assert_eq!(words(&session), ["冰冰"]);
}

#[test]
fn shuangpin_direct_mixed_endings_keep_chinese_candidates() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_s VALUES('shi''shi','ss','实实',100);",
    );
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Ziranma;
        options.helpcode_table = Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [("实".to_owned(), "bj".to_owned())].into_iter().collect(),
        )));
        options.shuangpin_helpcode_modes = direct_modes(true, true);
    });
    type_text(&mut session, "uibJuib/");
    assert_eq!(session.snapshot().editing_text, "uibJuib/");
    let index = index_of(&session, "实实");
    assert_eq!(session.select(index).commit.as_deref(), Some("实实"));
}

fn direct_modes(slash: bool, uppercase: bool) -> crate::types::ShuangpinHelpcodeModes {
    crate::types::ShuangpinHelpcodeModes {
        direct: false,
        direct_helpcode: true,
        direct_helpcode_slash: slash,
        direct_helpcode_uppercase: uppercase,
        ..crate::types::ShuangpinHelpcodeModes::default()
    }
}

/// 万象式直接的主开关（`direct_helpcode`）：出厂关。关着时 `uibj/` 的 `/` 与逐音节切分都
/// 不生效；开着 `/` 落进输入串结束一位或两位辅码，`uiaB` 要 `direct_helpcode_uppercase`。
#[test]
fn shuangpin_direct_helpcode_slash_and_uppercase_endings() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_s VALUES('shi','s','实',100),('shi','s','事',90),('shi','s','时',80);",
    );
    let codes = || {
        Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [("实", "bj"), ("事", "bp"), ("时", "sg")]
                .map(|(word, code)| (word.to_owned(), code.to_owned()))
                .into_iter()
                .collect(),
        )))
    };
    let session_with_modes = |modes| {
        let codes = codes();
        fixture.session_with(move |options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Ziranma;
            options.helpcode_table = codes.clone();
            options.shuangpin_helpcode_modes = modes;
        })
    };
    // slash 关着：`/` 还是标点键。
    let mut off = session_with_modes(direct_modes(false, true));
    type_text(&mut off, "uibj");
    assert!(!off.character(b'/', false).handled);
    // slash 开着：`ui` + `bj` 两码 + `/` 定字。
    let mut on = session_with_modes(direct_modes(true, false));
    type_text(&mut on, "uibj");
    assert!(on.character(b'/', false).handled);
    assert_eq!(on.snapshot().editing_text, "uibj/");
    assert_eq!(words(&on), ["实"]);
    // 一位辅码 + `/` 结束：留下首码 b 的全部 shi 字。
    let mut one = session_with_modes(direct_modes(true, false));
    type_text(&mut one, "uib/");
    assert_eq!(words(&one), ["实", "事"]);
    // `uiaB`：第二码大写结束，要 `direct_helpcode_uppercase` 开。
    let mut upper = session_with_modes(direct_modes(false, true));
    type_text(&mut upper, "uibJ");
    assert_eq!(words(&upper), ["实"]);
    let mut upper_off = session_with_modes(direct_modes(false, false));
    type_text(&mut upper_off, "uibJ");
    // `J` 不再当第二码：`uibJ` 落到字面输入，不会只剩「实」一个。
    assert_ne!(words(&upper_off), ["实"]);
}

/// `direct_helpcode` 出厂关：万象式逐音节切分不生效，`tartantab` 不再解出三个
/// 各带一码的音节；开着时才回到万象式行为。
#[test]
fn shuangpin_direct_helpcode_master_switch_gates_path_parsing() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_t VALUES('ta','t','他',100),('ta','t','她',90),('ta','t','它',80);\
         CREATE TABLE tbl_3_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_t VALUES('ta''ta''ta','ttt','他她它',100);",
    );
    let codes = || {
        Some(Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            [("他", "ra"), ("她", "na"), ("它", "ba")]
                .map(|(word, code)| (word.to_owned(), code.to_owned()))
                .into_iter()
                .collect(),
        )))
    };
    let session_with_modes = |modes| {
        let codes = codes();
        fixture.session_with(move |options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Ziranma;
            options.helpcode_table = codes.clone();
            options.shuangpin_helpcode_modes = modes;
        })
    };
    let mut off = session_with_modes(crate::types::ShuangpinHelpcodeModes {
        direct: false,
        ..crate::types::ShuangpinHelpcodeModes::default()
    });
    type_text(&mut off, "tartantab");
    assert!(!words(&off).iter().any(|word| word == "他她它"));
    let mut on = session_with_modes(direct_modes(false, false));
    type_text(&mut on, "tartantab");
    let index = index_of(&on, "他她它");
    assert_eq!(on.select(index).commit.as_deref(), Some("他她它"));
    assert!(on.snapshot().preedit.is_empty());
}

/// #6034：双拼会话读同一个「拼音纠错」开关。合成词库里只有关系是词，小鹤把 guan（gr）的 r 按成邻键 f 就是 gen；自然码把 xi 的 x 按成邻键 c 就是 ci。
const SHUANGPIN_TYPO_FIXTURE: &str =
    "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('mei','m','没',1000),('mei','m','美',900);\
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('guan','g','关',500);\
CREATE TABLE tbl_1_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_x VALUES('xi','x','系',1000);\
CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_c VALUES('ci','c','词',1000);\
CREATE TABLE tbl_2_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_g VALUES('guan''xi','gx','关系',100000);\
CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
CREATE TABLE tbl_3_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);";

/// 生产词库里字面整句可能比纠错词更高频；纠错候选仍应出现在候选列表中，而不是只在它胜过字面整句时出现。
const SHUANGPIN_NONLEADING_TYPO_FIXTURE: &str =
    "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('mei','m','没',1000000);\
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_g VALUES('gen','g','跟',1000000),('guan','g','关',1000);\
CREATE TABLE tbl_1_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_x VALUES('xi','x','系',1000000);\
CREATE TABLE tbl_2_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_g VALUES('guan''xi','gx','关系',1);\
CREATE TABLE tbl_3_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);";

const QUANPIN_EXTRA_LETTER_FIXTURE: &str =
    "CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shang','s','上',1000);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',1000);\
CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_s VALUES('shang''hao','sh','上好',10000);";

#[test]
fn quanpin_correction_consumes_extra_typed_letter() {
    let fixture = Fixture::new(QUANPIN_EXTRA_LETTER_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.autocorrect_types = crate::types::autocorrect_type::TRANSPOSITION
            | crate::types::autocorrect_type::NEIGHBOR;
    });
    type_text(&mut session, "shhanghao");
    let corrected = session
        .snapshot()
        .candidates
        .into_iter()
        .find(|item| item.word == "上好")
        .unwrap_or_else(|| panic!("{:?}", words(&session)));
    assert_eq!(corrected.corrected_from, "shhanghao");
    assert!(session.snapshot().candidate_answers_key[index_of(&session, "上好")]);

    assert_eq!(
        select_word(&mut session, "上好").commit.as_deref(),
        Some("上好")
    );
    assert_eq!(session.snapshot().preedit, "");
}

#[test]
fn converted_quanpin_correction_consumes_extra_typed_letter() {
    let fixture = Fixture::new(QUANPIN_EXTRA_LETTER_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.autocorrect_types = crate::types::autocorrect_type::TRANSPOSITION
            | crate::types::autocorrect_type::NEIGHBOR;
    });
    type_text(&mut session, "shhanghao");
    let corrected = index_of(&session, "上好");

    assert!(session
        .input
        .move_conversion_from(Command::ConversionLeft, corrected)
        .is_some());
    assert_eq!(session.snapshot().conversion, "上好");
    assert!(session.select(index_of(&session, "好")).handled);
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("上好")
    );
    assert_eq!(session.snapshot().preedit, "");
}

/// 纠错整句排在字面整句之后，选中它按全部按键上屏；开关关着时不出。
#[test]
fn shuangpin_offers_the_typo_sentence_under_the_autocorrect_switch() {
    let fixture = Fixture::new(SHUANGPIN_TYPO_FIXTURE);
    let shuangpin = |types: u32| {
        move |options: &mut SessionOptions| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
            options.autocorrect_types = types;
        }
    };
    let mut session = fixture.session_with(shuangpin(
        crate::types::autocorrect_type::TRANSPOSITION | crate::types::autocorrect_type::NEIGHBOR,
    ));
    type_text(&mut session, "mwgfxi");
    let snapshot = session.snapshot();
    let corrected = snapshot
        .candidates
        .iter()
        .position(|item| item.word == "没关系")
        .unwrap_or_else(|| panic!("{:?}", words(&session)));
    assert!(corrected > 0, "{:?}", words(&session));
    assert_eq!(snapshot.candidates[corrected].corrected_from, "mwgfxi");
    assert_eq!(
        select_word(&mut session, "没关系").commit.as_deref(),
        Some("没关系")
    );

    let mut plain = fixture.session_with(shuangpin(0));
    type_text(&mut plain, "mwgfxi");
    assert!(
        !words(&plain).contains(&"没关系".to_owned()),
        "{:?}",
        words(&plain)
    );
}

#[test]
fn shuangpin_offers_a_nonleading_typo_sentence_from_a_full_dictionary() {
    let fixture = Fixture::new(SHUANGPIN_NONLEADING_TYPO_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        options.autocorrect_types = crate::types::autocorrect_type::TRANSPOSITION
            | crate::types::autocorrect_type::NEIGHBOR;
    });
    type_text(&mut session, "mwgfxi");
    let snapshot = session.snapshot();
    let corrected = snapshot
        .candidates
        .iter()
        .position(|item| item.word == "没关系")
        .unwrap_or_else(|| panic!("{:?}", words(&session)));
    assert!(corrected > 0, "{:?}", words(&session));
    assert_eq!(snapshot.candidates[corrected].corrected_from, "mwgfxi");
}

/// 双拼方案在宿主重建会话时切换。重建后的会话按新方案的编码纠错：自然码下 `mzgrci` 纠回没关系，小鹤的误触串 `mwgfxi` 不再纠；切回小鹤后反过来。
#[test]
fn a_rebuilt_session_corrects_with_its_new_profile() {
    let fixture = Fixture::new(SHUANGPIN_TYPO_FIXTURE);
    let shuangpin = |kind: ShuangpinProfileKind| {
        move |options: &mut SessionOptions| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = kind;
            options.autocorrect_types = crate::types::autocorrect_type::TRANSPOSITION
                | crate::types::autocorrect_type::NEIGHBOR;
        }
    };
    for (kind, own, foreign) in [
        (ShuangpinProfileKind::Xiaohe, "mwgfxi", "mzgrci"),
        (ShuangpinProfileKind::Ziranma, "mzgrci", "mwgfxi"),
        (ShuangpinProfileKind::Xiaohe, "mwgfxi", "mzgrci"),
    ] {
        let mut session = fixture.session_with(shuangpin(kind));
        type_text(&mut session, own);
        let listed = words(&session);
        let corrected = listed.iter().position(|word| word == "没关系");
        assert!(
            corrected.is_some_and(|index| index > 0),
            "{kind:?} {own}: {listed:?}"
        );
        session.command(Command::Cancel);
        type_text(&mut session, foreign);
        assert!(
            !words(&session).contains(&"没关系".to_owned()),
            "{kind:?} {foreign}: {:?}",
            words(&session)
        );
    }
}

// ---- wubi mixed routing (overlays.md §3.3) ----

fn wubi_mixed(fixture: &Fixture) -> Session {
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "gege");
    session
}

#[test]
fn a_mixed_list_holds_both_producers_wubi_first() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let session = wubi_mixed(&fixture);
    let snapshot = session.snapshot();
    let schemes: Vec<(String, SchemeType)> = snapshot
        .candidates
        .iter()
        .map(|item| (item.word.to_string(), item.scheme))
        .collect();
    assert_eq!(schemes.first(), Some(&("工".to_owned(), SchemeType::Wubi)));
    assert!(schemes.contains(&("哥哥".to_owned(), SchemeType::Quanpin)));
    assert!(schemes.contains(&("个".to_owned(), SchemeType::Quanpin)));
    assert!(!snapshot.answered_by_pinyin_fallback);
    // The quanpin rows beside the one wubi row keep the code open: the fourth key must not commit 工 over 哥哥 for someone typing pinyin.
    assert!(!snapshot.wubi_unique_four_code);
}

#[test]
fn mixed_wubi_puts_exact_pinyin_before_unmatched_wubi_prefixes() {
    let fixture = Fixture::new(WUBI_EDU_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "edu");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.candidates[0].word, "额度");
    assert_eq!(snapshot.candidates[0].scheme, SchemeType::Quanpin);
    assert!(snapshot.candidates.iter().any(|item| item.word == "膫"));
    assert!(snapshot.candidates.iter().any(|item| item.word == "须将"));
}

#[test]
fn wubi_mixed_refresh_reuses_pinyin_request_buffer() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let mut session = wubi_mixed(&fixture);
    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.engine.handle_key(SchemeKey::Requery);
    });
    assert!(
        allocations <= 53,
        "mixed Wubi refresh should reuse the pinyin request buffer: {allocations} allocations"
    );
}

#[test]
fn ignored_scheme_key_does_not_clone_preedit_for_change_detection() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni'");

    let (result, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.handle_character(b'\'', false)
    });

    assert!(!result.handled);
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(allocations, 19);
}

#[test]
fn typing_at_the_end_does_not_build_the_preedit_twice_for_caret_detection() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");

    let (result, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.handle_character(b'h', false)
    });

    assert!(result.handled);
    let matched_rows = session
        .snapshot()
        .candidates
        .iter()
        .filter(|row| row.pinyin == "ni'h")
        .count();
    assert_eq!(matched_rows, 5);
    // SharedString 克隆只增加引用计数，候选行重建的分配预算低于旧实现。
    assert_eq!(allocations, 98);
}

#[test]
fn backspacing_at_the_end_does_not_build_the_preedit_twice_for_caret_detection() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.command(Command::Backspace));

    assert!(result.handled);
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(allocations, 11);
}

#[test]
fn url_entry_readiness_at_the_end_does_not_build_the_preedit_twice() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.character(b'.', false));

    assert!(result.handled);
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(allocations, 6);
}

#[test]
fn url_trigger_reversion_reuses_the_remaining_text_buffer() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    assert!(session.punctuation(b'.').handled);

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.command(Command::Backspace));

    assert!(result.handled);
    assert_eq!(session.snapshot().preedit, "www");
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert!(
        allocations <= 33,
        "URL trigger reversion allocations: {allocations}"
    );
}

#[test]
fn prefix_end_does_not_build_editing_text_to_clamp_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    session.set_caret(Some(2));

    let (prefix_end, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.prefix_end());

    assert_eq!(prefix_end, 2);
    assert_eq!(allocations, 1);
}

#[test]
fn setting_the_caret_does_not_build_editing_text_to_clamp_it() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.set_caret(Some(2));
    });

    assert_eq!(session.snapshot().caret_position, 2);
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(allocations, 18);
}

#[test]
fn moving_the_caret_does_not_build_the_preedit_twice() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.command(Command::MoveLeft));

    assert!(result.handled);
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(
        allocations, 18,
        "光标移动复用编辑文本长度后的分配次数：{allocations}"
    );
}

#[test]
fn typing_at_a_caret_reuses_the_editing_text_length() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    session.command(Command::MoveLeft);

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.character(b'x', false));

    assert!(result.handled);
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(allocations, 164, "光标处输入的分配次数：{allocations}");
}

#[test]
fn selecting_a_quanpin_candidate_clones_only_needed_request_fields() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.learning = false;
        options.personal_context = false;
    });
    type_text(&mut session, "nihao");
    let index = index_of(&session, "你好");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(index));

    assert_eq!(result.commit.as_deref(), Some("你好"));
    assert!(allocations <= 14, "候选选择分配次数：{allocations}");
}

#[test]
fn selecting_a_dictionary_candidate_does_not_build_context_row_vec() {
    let fixture = Fixture::new(CONTEXT_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.learning = true;
        options.personal_context = true;
    });
    type_text(&mut session, "ni");
    let index = index_of(&session, "甲");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(index));

    assert_eq!(result.commit.as_deref(), Some("甲"));
    assert!(
        allocations <= 71,
        "个人上下文记录的中间行分配了 {allocations} 次"
    );
}

#[test]
fn selecting_a_candidate_does_not_clone_unused_sentence_words() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.learning = false;
        options.personal_context = false;
    });
    type_text(&mut session, "nihao");
    let index = index_of(&session, "你好");
    session.input.mixed_candidates[index].sentence_words = vec!["你".to_owned(), "好".to_owned()];

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(index));

    assert_eq!(result.commit.as_deref(), Some("你好"));
    assert!(
        allocations <= 35,
        "选择候选不应复制未使用的行字段，却产生了 {allocations} 次分配"
    );
}

#[test]
fn selecting_a_shuangpin_candidate_does_not_clone_the_full_request() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.learning = false;
        options.personal_context = false;
    });
    type_text(&mut session, "ni'hc");
    let index = index_of(&session, "你好");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(index));

    assert_eq!(result.commit.as_deref(), Some("你好"));
    // 候选行字段是 `SharedString`：克隆行只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(allocations, 14, "双拼候选选择分配次数：{allocations}");
}

#[test]
fn selecting_an_unsupported_candidate_does_not_clone_its_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert!(session.character(b'Y', true).handled);
    let before = session.snapshot();
    assert_eq!(before.candidate_sources, [CandidateSource::Fallback]);

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(0));

    assert!(result.handled && result.diagnostic.is_none());
    assert_eq!(result.commit.as_deref(), Some("Y"));
    assert!(session.snapshot().preedit.is_empty());
    assert!(
        allocations <= 3,
        "选择不可编辑候选产生了 {allocations} 次分配"
    );
}

#[test]
fn selecting_the_top_candidate_does_not_clone_an_unused_learning_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.personal_context = false;
    });
    type_text(&mut session, "nihao");
    let index = index_of(&session, "你好");
    assert_eq!(index, 0);

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        assert_eq!(session.input.learn_candidate(index), None);
    });

    assert_eq!(
        allocations, 0,
        "top-candidate learning allocated {allocations} unused buffers"
    );
}

#[test]
fn learning_a_non_top_candidate_does_not_clone_the_whole_ranking_list() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.personal_context = false;
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
        options.frequency.trigger_count = 1;
    });
    // 固定音节边界，单独约束学习字段复制，不把备选切分冷查询计入该预算。
    type_text(&mut session, "ni'hao");
    let index = index_of(&session, "拟好");
    assert_eq!(index, 1);

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        assert_eq!(session.input.learn_candidate(index), None);
    });

    assert!(
        allocations <= 411,
        "non-top learning allocated {allocations} buffers"
    );
    assert!(
        count(
            &fixture.main_db(),
            "SELECT weight FROM tbl_2_n WHERE value='拟好'"
        ) > 200
    );
}

#[test]
fn frequency_learning_borrows_an_existing_entry_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.personal_context = false;
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
        options.frequency.trigger_count = 1;
    });
    // 固定音节边界，单独约束学习字段复制，不把备选切分冷查询计入该预算。
    type_text(&mut session, "ni'hao");
    let index = index_of(&session, "拟好");
    assert_eq!(index, 1);

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        assert_eq!(session.input.learn_candidate(index), None);
    });

    assert!(
        allocations <= 410,
        "entry-key learning allocated {allocations} buffers"
    );
}

/// The reported case: in mixed Wubi `jixu` is the wubi code of 曳光弹 and the pinyin of 继续. The fourth key must leave both on offer; without pinyin rows the same code still commits its one wubi row.
#[test]
fn a_four_letter_code_that_is_also_pinyin_stays_open_in_mixed_wubi() {
    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('jixu','曳光弹',100);\
CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_j VALUES('ji''xu','jx','继续',1000);",
    );
    let mut mixed = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut mixed, "jixu");
    assert!(words(&mixed).contains(&"曳光弹".to_owned()));
    assert!(words(&mixed).contains(&"继续".to_owned()));
    assert!(!mixed.snapshot().wubi_unique_four_code);

    let mut plain = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = false;
    });
    type_text(&mut plain, "jixu");
    assert_eq!(words(&plain), vec!["曳光弹".to_owned()]);
    assert!(plain.snapshot().wubi_unique_four_code);
}

#[test]
fn removing_a_quanpin_row_in_mixed_wubi_deletes_the_pinyin_row() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let mut session = wubi_mixed(&fixture);
    let index = index_of(&session, "哥哥");
    let result = session.remove(index);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    let main = fixture.main_db();
    assert_eq!(
        count(&main, "SELECT count(*) FROM tbl_2_g WHERE value='哥哥'"),
        0
    );
    assert_eq!(count(&main, "SELECT count(*) FROM wubi86"), 1);
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='pinyin' AND value='哥哥' AND operation='delete'"
        ),
        1
    );
    assert!(!words(&session).contains(&"哥哥".to_owned()));
}

#[test]
fn pinning_a_quanpin_row_in_mixed_wubi_writes_the_pinyin_table() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let mut session = wubi_mixed(&fixture);
    let before = count(
        &fixture.main_db(),
        "SELECT weight FROM tbl_1_g WHERE value='个'",
    );
    let index = index_of(&session, "个");
    let result = session.pin(index);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    let main = fixture.main_db();
    assert!(count(&main, "SELECT weight FROM tbl_1_g WHERE value='个'") > before);
    assert_eq!(
        count(&main, "SELECT weight FROM wubi86 WHERE value='工'"),
        100
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi'"
        ),
        0
    );
}

#[test]
fn removing_an_unsupported_candidate_does_not_clone_its_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert!(session.character(b'Y', true).handled);
    let before = session.snapshot();
    assert_eq!(before.candidate_sources, [CandidateSource::Fallback]);

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.remove(0));

    assert!(!result.handled && result.commit.is_none() && result.diagnostic.is_none());
    assert_eq!(session.snapshot(), before);
    assert_eq!(allocations, 0, "拒绝删除候选产生了 {allocations} 次分配");
}

#[test]
fn pinning_a_candidate_does_not_clone_the_full_learning_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.local_modes.temporary_english = true);
    assert!(session.character(b'Y', true).handled);
    assert_eq!(
        session.snapshot().candidates[0].source,
        CandidateSource::Fallback
    );

    let (result, allocations) = crate::ime::personal_rerank::allocations::count(|| session.pin(0));

    assert!(!result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(
        allocations, 0,
        "pinning an unsupported candidate allocated {allocations} buffers"
    );
}

#[test]
fn fixing_an_unsupported_candidate_does_not_clone_the_full_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.local_modes.temporary_english = true);
    assert!(session.character(b'Y', true).handled);
    assert_eq!(
        session.snapshot().candidates[0].source,
        CandidateSource::Fallback
    );

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.fix_position(0, 1));

    assert!(!result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(
        allocations, 0,
        "fixing an unsupported candidate allocated {allocations} buffers"
    );
}

#[test]
fn removing_an_unsupported_candidate_does_not_clone_the_full_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.local_modes.temporary_english = true);
    assert!(session.character(b'Y', true).handled);
    assert_eq!(
        session.snapshot().candidates[0].source,
        CandidateSource::Fallback
    );

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.remove(0));

    assert!(!result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(
        allocations, 0,
        "removing an unsupported candidate allocated {allocations} buffers"
    );
}

/// 904bd0976: learning ranks against the list before any personal reorder, filtered to the selected row's producer, so a wubi row heavier than every quanpin row does not take part in a quanpin row's rank.
#[test]
fn a_quanpin_row_in_mixed_wubi_ranks_among_the_quanpin_rows_only() {
    let fixture =
        Fixture::new(&WUBI_ROUTING_FIXTURE.replace("'gege','工',100", "'gege','工',5000"));
    let mut session = wubi_mixed(&fixture);
    assert_eq!(words(&session).first().map(String::as_str), Some("工"));
    let result = session.pin(index_of(&session, "个"));
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    let main = fixture.main_db();
    let pinned = count(&main, "SELECT weight FROM tbl_1_g WHERE value='个'");
    // Above 哥哥, the heaviest quanpin row, but ranked without 工: counting it would have lifted 个 past 5000.
    assert!(pinned > 1000 && pinned < 5000, "个 weighs {pinned}");
    assert_eq!(
        count(&main, "SELECT weight FROM wubi86 WHERE value='工'"),
        5000
    );
    assert_eq!(
        count(&main, "SELECT weight FROM tbl_2_g WHERE value='哥哥'"),
        1000
    );
}

#[test]
fn selecting_a_quanpin_row_in_mixed_wubi_advances_like_quanpin() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let mut session = wubi_mixed(&fixture);
    let result = select_word(&mut session, "个");
    assert_eq!(result.commit.as_deref(), Some("个"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "ge");
    assert_eq!(snapshot.scheme, SchemeType::Wubi);

    let mut session = wubi_mixed(&fixture);
    let answers: Vec<(String, bool)> = {
        let snapshot = session.snapshot();
        snapshot
            .candidates
            .iter()
            .map(|item| item.word.to_string())
            .zip(snapshot.candidate_answers_key)
            .collect()
    };
    assert!(answers.contains(&("工".to_owned(), true)));
    assert!(answers.contains(&("个".to_owned(), false)));
    let result = select_word(&mut session, "工");
    assert_eq!(result.commit.as_deref(), Some("工"));
    assert!(session.snapshot().editing_text.is_empty());
}

#[test]
fn fixed_positions_apply_within_each_producer_group() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let mut session = wubi_mixed(&fixture);
    let index = index_of(&session, "个");
    let result = session.fix_position(index, 1);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    let listed = words(&session);
    // Slot 1 of the pinyin group, which still follows the wubi group.
    assert_eq!(listed.first().map(String::as_str), Some("工"));
    assert_eq!(listed.get(1).map(String::as_str), Some("个"));
    let stored = count(
        &fixture.journal(),
        "SELECT count(*) FROM fixed_candidate_positions WHERE value='个' AND context_key='ge''ge'",
    );
    assert_eq!(stored, 1);
}

#[test]
fn position_context_borrows_normalized_pinyin() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "n");

    assert!(matches!(
        session.input.position_context(false, false),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn pinyin_segmentation_borrows_explicit_segmentation_when_normalized_input_is_empty() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "n'");

    assert!(matches!(
        session.input.pinyin_segmentation(),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn complete_pinyin_check_reuses_segmentation_storage() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");

    let (complete, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.is_all_complete_pure_pinyin()
    });
    assert!(complete);
    assert_eq!(allocations, 0, "完整拼音判定不应复制已有切分");
}

#[test]
fn pinyin_ranking_context_borrows_normalized_segmentation() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "n");

    assert!(matches!(
        session.input.pinyin_ranking_context(),
        std::borrow::Cow::Borrowed(_)
    ));
}

/// Phrase progress follows the selected row (`transition.wubi_native`), not the list: two quanpin picks out of a list that still holds 工 compose a storable pinyin phrase.
#[test]
fn quanpin_picks_beside_a_wubi_row_learn_a_pinyin_phrase() {
    let fixture = Fixture::new(
        &WUBI_ROUTING_FIXTURE.replace("INSERT INTO tbl_2_g VALUES('ge''ge','gg','哥哥',1000);", ""),
    );
    let mut session = wubi_mixed(&fixture);
    assert!(words(&session).contains(&"工".to_owned()));
    assert_eq!(
        select_word(&mut session, "个").commit.as_deref(),
        Some("个")
    );
    assert_eq!(
        select_word(&mut session, "各").commit.as_deref(),
        Some("各")
    );
    assert!(session.snapshot().preedit.is_empty());
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM tbl_2_g WHERE key='ge''ge' AND value='个各'"
        ),
        1
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='pinyin' AND operation<>'delete' AND value='个各'"
        ),
        1
    );
}

#[test]
fn composed_words_become_both_wubi_profiles_only_when_learning_is_enabled() {
    for learning in [false, true] {
        for scheme in [SchemeType::Quanpin, SchemeType::Wubi] {
            let fixture = Fixture::new(
                "CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                INSERT INTO tbl_1_j VALUES('jia','j','甲',100);
                CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                INSERT INTO tbl_1_y VALUES('yi','y','乙',100);
                CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                INSERT INTO wubi86 VALUES('abcd','甲',100),('efgh','乙',100);
                CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
                INSERT INTO wubi98 VALUES('ijkl','甲',100),('mnop','乙',100);",
            );
            let mut session = fixture.session_with(|options| {
                options.scheme = scheme;
                options.wubi.mixed_pinyin = true;
                options.learning = learning;
            });
            type_text(&mut session, "jiayi");
            assert_eq!(
                select_word(&mut session, "甲").commit.as_deref(),
                Some("甲")
            );
            assert_eq!(
                select_word(&mut session, "乙").commit.as_deref(),
                Some("乙")
            );
            drop(session);
            for (profile, code, table) in [
                (crate::WubiProfileKind::Wubi86, "abef", "wubi86"),
                (crate::WubiProfileKind::Wubi98, "ijmn", "wubi98"),
            ] {
                assert_eq!(
                    count(
                        &fixture.main_db(),
                        &format!("SELECT count(*) FROM {table} WHERE value='甲乙'")
                    ),
                    i64::from(learning)
                );
                if learning {
                    let mut next = fixture.session_with(|options| {
                        options.scheme = SchemeType::Wubi;
                        options.wubi.profile = profile;
                        options.learning = false;
                    });
                    type_text(&mut next, code);
                    assert_eq!(
                        select_word(&mut next, "甲乙").commit.as_deref(),
                        Some("甲乙")
                    );
                }
            }
        }
    }
}

/// A generated quanpin sentence selected beside a native wubi row is learned as a pinyin phrase (the per-row guards of `learn_sentence_candidate` and `follows_pinyin`); the wubi row itself writes nothing to the pinyin side.
#[test]
fn a_quanpin_sentence_beside_a_wubi_row_is_learned_and_the_wubi_row_is_not() {
    let fixture = Fixture::new(
        &WUBI_ROUTING_FIXTURE.replace("INSERT INTO tbl_2_g VALUES('ge''ge','gg','哥哥',1000);", ""),
    );
    let pinyin_rows = "SELECT count(*) FROM tbl_2_g";
    let pinyin_journal =
        "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='pinyin'";

    let mut session = wubi_mixed(&fixture);
    assert_eq!(
        select_word(&mut session, "工").commit.as_deref(),
        Some("工")
    );
    assert_eq!(count(&fixture.main_db(), pinyin_rows), 0);
    assert_eq!(count(&fixture.journal(), pinyin_journal), 0);

    let mut session = wubi_mixed(&fixture);
    let snapshot = session.snapshot();
    assert!(snapshot
        .candidates
        .iter()
        .any(|item| item.scheme == SchemeType::Wubi));
    let index = snapshot
        .candidates
        .iter()
        .position(|item| {
            item.scheme == SchemeType::Quanpin
                && item.source.is_generated_or_fallback()
                && item.word.chars().count() == 2
        })
        .unwrap_or_else(|| panic!("no generated sentence in {:?}", words(&session)));
    let sentence = snapshot.candidates[index].word.to_string();
    assert_eq!(
        session.select(index).commit.as_deref(),
        Some(sentence.as_str())
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            &format!("SELECT count(*) FROM tbl_2_g WHERE key='ge''ge' AND value='{sentence}'")
        ),
        1
    );
    assert!(count(&fixture.journal(), pinyin_journal) > 0);
}

#[test]
fn generated_sentence_learning_borrows_candidate_fields() {
    let fixture = Fixture::new(
        &WUBI_ROUTING_FIXTURE.replace("INSERT INTO tbl_2_g VALUES('ge''ge','gg','哥哥',1000);", ""),
    );
    let mut session = wubi_mixed(&fixture);
    let index = session
        .snapshot()
        .candidates
        .iter()
        .position(|item| {
            item.scheme == SchemeType::Quanpin
                && item.source.is_generated_or_fallback()
                && item.word.chars().count() == 2
        })
        .unwrap();
    let sentence = session.snapshot().candidates[index].word.to_string();
    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(index));
    assert_eq!(result.commit.as_deref(), Some(sentence.as_str()));
    assert!(result.diagnostic.is_none());
    assert!(session.snapshot().preedit.is_empty());
    assert_eq!(
        count(
            &fixture.main_db(),
            &format!("SELECT count(*) FROM tbl_2_g WHERE key='ge''ge' AND value='{sentence}'")
        ),
        1
    );
    assert!(
        allocations <= 196,
        "生成句子提交产生了 {allocations} 次分配"
    );
}

/// A native wubi row's fixed slot is stored under the wubi raw code and applied within the wubi group only.
#[test]
fn a_native_wubi_row_is_fixed_under_the_wubi_context_in_a_mixed_list() {
    let fixture = Fixture::new(&format!(
        "{WUBI_ROUTING_FIXTURE}INSERT INTO wubi86 VALUES('gege','或',50);"
    ));
    let mut session = wubi_mixed(&fixture);
    assert_eq!(words(&session)[..2], ["工", "或"]);
    let result = session.fix_position(index_of(&session, "或"), 1);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    let listed = words(&session);
    assert_eq!(listed[..2], ["或", "工"]);
    assert!(listed[2..].contains(&"哥哥".to_owned()));
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM fixed_candidate_positions WHERE value='或' AND context_key='gege'"
        ),
        1
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM fixed_candidate_positions WHERE value='或' AND context_key='ge''ge'"
        ),
        0
    );
    let result = session.clear_position(index_of(&session, "或"));
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(words(&session)[..2], ["工", "或"]);
}

/// On a one-letter code the fixed rows the capped list left out are put back through each producer's own lookup: a quanpin row fixed past the 24-row cap returns to its slot of the pinyin group, behind the wubi group.
#[test]
fn a_one_letter_mixed_code_reinserts_a_fixed_quanpin_row_through_the_pinyin_lookup() {
    let rows: Vec<String> = (0..30)
        .map(|i| {
            format!(
                "('ge','g','{}',{})",
                char::from_u32(0x4e00 + i).unwrap(),
                1000 - i
            )
        })
        .collect();
    let fixture = Fixture::new(&format!(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('g','王',100);\
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_g VALUES{};",
        rows.join(",")
    ));
    let late = char::from_u32(0x4e00 + 29).unwrap().to_string();

    let mut quanpin = fixture.session();
    type_text(&mut quanpin, "g");
    assert!(!words(&quanpin).contains(&late));
    assert!(quanpin.expand_initial_candidates());
    let result = quanpin.fix_position(index_of(&quanpin, &late), 2);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    drop(quanpin);

    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "g");
    let snapshot = session.snapshot();
    let listed: Vec<(String, SchemeType)> = snapshot
        .candidates
        .iter()
        .map(|item| (item.word.to_string(), item.scheme))
        .collect();
    assert_eq!(listed[0], ("王".to_owned(), SchemeType::Wubi));
    // Slot 2 of the pinyin group, which starts after the one wubi row.
    assert_eq!(listed[2], (late.clone(), SchemeType::Quanpin), "{listed:?}");
    assert!(snapshot.candidate_answers_key[0]);
}

/// test_runtime_isolation.cpp:500-507: a fixed-slot write the journal refuses is reported, and the list keeps the slots it had.
#[test]
fn a_rejected_fixed_slot_write_reports_and_keeps_the_snapshot() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let result = session.fix_position(index_of(&session, "拟好"), 1);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    assert_eq!(words(&session)[0], "拟好");
    Connection::open(fixture.journal())
        .and_then(|journal| journal.execute_batch("CREATE TRIGGER reject_fixed BEFORE INSERT ON fixed_candidate_positions BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;"))
        .expect("trigger");
    let result = session.fix_position(index_of(&session, "你好"), 2);
    assert!(result.handled);
    assert_eq!(result.commit, None);
    assert_eq!(
        result.diagnostic.as_deref(),
        Some(crate::diagnostics::POSITION_NOT_PERSISTED)
    );
    let snapshot = session.snapshot();
    assert_eq!(snapshot.candidates[0].word, "拟好");
    assert_eq!(snapshot.candidates[0].fixed_position, 1);
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM fixed_candidate_positions WHERE value='你好'"
        ),
        0
    );
}

/// test_candidate_removal.cpp:166-194: a removal the journal refuses is reported, commits nothing, and leaves the composition and the row where they were, for pinyin and English rows alike.
#[test]
fn a_removal_the_journal_refuses_reports_and_keeps_the_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let before = session.snapshot();
    crate::user_dictionary::journal::ensure_user_database(&fixture.journal()).expect("journal");
    Connection::open(fixture.journal())
        .and_then(|journal| journal.execute_batch("CREATE TRIGGER reject_removal BEFORE INSERT ON user_dictionary_operations BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;"))
        .expect("trigger");
    let result = session.remove(index_of(&session, "拟好"));
    assert!(result.handled);
    assert_eq!(result.commit, None);
    assert_eq!(
        result.diagnostic.as_deref(),
        Some(crate::diagnostics::REMOVAL_NOT_PERSISTED)
    );
    assert_eq!(session.snapshot(), before);
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM tbl_2_n WHERE value='拟好'"
        ),
        1
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations"
        ),
        0
    );
    session.command(Command::Cancel);

    session.set_dedicated_english(true);
    type_text(&mut session, "help");
    let before = session.snapshot();
    let result = session.remove(index_of(&session, "Help"));
    assert!(result.handled);
    assert_eq!(result.commit, None);
    assert_eq!(
        result.diagnostic.as_deref(),
        Some(crate::diagnostics::REMOVAL_NOT_PERSISTED)
    );
    assert_eq!(session.snapshot(), before);
    let english = fixture.path().join(assets::ENGLISH_DICTIONARY);
    assert_eq!(
        count(
            &english,
            "SELECT count(*) FROM english_words WHERE word='help'"
        ),
        1
    );
}

/// test_wubi_mixed_input_session.cpp's fixture plus `wo`, which both producers answer: 人 from the wubi table, 我 from quanpin.
const WUBI_TAIL_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',10000);\
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wo','w','我',10000);\
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',10000),('ni''hao','nh','拟好',9000);\
CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('wq','你好',10000),('wo','人',9000);";

/// Mixed wubi with 你好 picked out of `nihaowo`, leaving `wo` composing.
fn wubi_pinyin_tail(fixture: &Fixture) -> Session {
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "nihaowo");
    let result = session.select(index_of(&session, "你好"));
    assert_eq!(result.commit.as_deref(), Some("你好"));
    assert_eq!(session.snapshot().preedit, "wo");
    session
}

fn schemes(session: &Session) -> Vec<(String, SchemeType)> {
    session
        .snapshot()
        .candidates
        .into_iter()
        .map(|item| (item.word.into_string(), item.scheme))
        .collect()
}

/// Product decision 2026-09-30 (test_wubi_mixed_input_session.cpp:194-212): once a pinyin word is picked, the rest of the composition is the rest of a spelling and stays quanpin, even where the wubi table has a code for its letters.
#[test]
fn the_rest_after_a_pinyin_pick_stays_pinyin() {
    let fixture = Fixture::new(WUBI_TAIL_FIXTURE);
    let mut fresh = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut fresh, "wo");
    assert_eq!(
        schemes(&fresh).first(),
        Some(&("人".to_owned(), SchemeType::Wubi)),
        "outside a tail the table answers wo"
    );

    let mut session = wubi_pinyin_tail(&fixture);
    assert_eq!(schemes(&session), [("我".to_owned(), SchemeType::Quanpin)]);
    assert!(session.snapshot().answered_by_pinyin_fallback);
    assert!(!session.snapshot().wubi_unique_four_code);
    // Letters typed into the tail stay pinyin too.
    type_text(&mut session, "ni");
    assert_eq!(session.snapshot().preedit, "woni");
    assert!(schemes(&session)
        .iter()
        .all(|(_, scheme)| *scheme == SchemeType::Quanpin));
    // Committing the tail ends the composition, and the next code is the table's again.
    session.command(Command::Cancel);
    let mut session = wubi_pinyin_tail(&fixture);
    let result = session.select(index_of(&session, "我"));
    assert_eq!(result.commit.as_deref(), Some("我"));
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "wo");
    assert_eq!(
        schemes(&session).first(),
        Some(&("人".to_owned(), SchemeType::Wubi))
    );
}

#[test]
fn bianyi_continues_after_the_bian_wubi_code() {
    let fixture = Fixture::new(WUBI_BIANYI_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "bianyi");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "bianyi");
    assert!(
        snapshot.candidates.iter().any(|item| item.word == "编译"),
        "{snapshot:?}"
    );
}

const WUBI_WILDCARD_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('wqiy','你',100);\
INSERT INTO wubi86 VALUES('wqiy','伱',10);\
INSERT INTO wubi86 VALUES('abcd','合成',200);\
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100);";

/// 通配打开时的端到端行为：`z` 进码、查询走模式匹配、四码唯一与顶字都不会对带 `z` 的猜码动手。
#[test]
fn the_wubi_wildcard_matches_any_letter_and_keeps_guessing_codes_open() {
    let fixture = Fixture::new(WUBI_WILDCARD_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.z_wildcard = true;
    });
    type_text(&mut session, "wqzy");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "wqzy");
    assert_eq!(
        snapshot
            .candidates
            .iter()
            .map(|item| item.word.as_str())
            .collect::<Vec<_>>(),
        ["你", "伱"]
    );
    // 此快照只验证带 `z` 的猜码不算四码唯一；第五键的顶字边界由真实 runtime 用例验证。
    assert!(!snapshot.wubi_unique_four_code);
    assert!(!snapshot.answered_by_pinyin_fallback);
}

/// 带 `z` 的码即使是码表唯一命中也不算「四码唯一」：它是猜码，后台那个唯一候选与好几个码只差一个字母。
#[test]
fn a_wildcard_code_never_counts_as_a_unique_four_code() {
    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('wqzy','唯一',100);\
INSERT INTO wubi86 VALUES('abcd','合成',200);",
    );
    let mut wildcard = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.z_wildcard = true;
    });
    type_text(&mut wildcard, "wqzy");
    assert_eq!(words(&wildcard), ["唯一"]);
    assert!(!wildcard.snapshot().wubi_unique_four_code);

    // 同一个码表里不含 `z` 的四码仍然是四码唯一：守卫只针对 `z`。
    let mut plain = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
    });
    type_text(&mut plain, "abcd");
    assert_eq!(words(&plain), ["合成"]);
    assert!(plain.snapshot().wubi_unique_four_code);
}

/// 通配打开时 `z` 是未知码位，临时拼音反查让位：组字开头的 `z` 进码，反查不打开。
#[test]
fn the_wubi_wildcard_takes_z_from_the_pinyin_lookup() {
    let fixture = Fixture::new(WUBI_WILDCARD_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.z_wildcard = true;
    });
    type_text(&mut session, "zqiy");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "zqiy");
    // 反查不打开：候选来自五笔码表的通配匹配，不是全拼。
    assert!(!snapshot.answered_by_pinyin_fallback);
    assert_eq!(
        snapshot
            .candidates
            .iter()
            .map(|item| item.word.as_str())
            .collect::<Vec<_>>(),
        ["你", "伱"]
    );
}

/// 通配关闭时 `z` 照旧不进码，组字开头的 `z` 照旧打开临时拼音反查——升级用户零变化。
#[test]
fn turning_the_wubi_wildcard_off_keeps_z_out_of_the_code() {
    let fixture = Fixture::new(WUBI_WILDCARD_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.z_wildcard = false;
    });
    type_text(&mut session, "zni");
    let snapshot = session.snapshot();
    // 反查打开：组字是 `zni`，候选来自全拼。
    assert_eq!(snapshot.preedit, "zni");
    assert!(snapshot.answered_by_pinyin_fallback);
    assert!(snapshot.candidates.iter().any(|item| item.word == "你"));
}

#[test]
fn xuqiu_continues_after_a_four_letter_wubi_code() {
    let fixture = Fixture::new(WUBI_XUQIU_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "xuqi");
    assert!(session.input.engine.wubi_pinyin_continues(b'u'));
    type_text(&mut session, "u");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "xuqiu");
    assert!(
        snapshot.candidates.iter().any(|item| item.word == "需求"),
        "{snapshot:?}"
    );
}

#[test]
fn mixed_wubi_keeps_a_pinyin_prefix_when_the_fourth_code_is_a_complete_syllable() {
    let fixture = Fixture::new(WUBI_HANGUO_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "hang");
    assert!(session.input.engine.wubi_pinyin_continues(b'u'));
    type_text(&mut session, "uo");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "hanguo");
    assert!(
        snapshot.candidates.iter().any(|item| item.word == "韩国"),
        "{snapshot:?}"
    );
}

/// 五笔整码也可能正好是两个完整音节；混拼续输下一个音节时，拼音候选的规范读音带有 `'` 分隔符，仍应允许继续输入。
#[test]
fn mixed_wubi_starts_a_new_pinyin_syllable_after_a_four_letter_code() {
    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('qigu','乐平',100);\
CREATE TABLE tbl_2_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_q VALUES('qi''gu','qg','其故',100);\
INSERT INTO tbl_2_q VALUES('qi''guai','qg','奇怪',1000);",
    );
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "qiguai");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "qiguai");
    assert!(
        snapshot.candidates.iter().any(|item| item.word == "奇怪"),
        "{snapshot:?}"
    );
}

/// 四码时的续写判断只认真实读音前缀：前三码的简拼行让延长状态被继承，但 `wqiy` 之后的 `a` 续不上任何读音，不能挡住顶字。
#[test]
fn wubi_pinyin_continues_only_for_a_real_reading_after_a_four_code() {
    let mixed = |fixture: &Fixture| {
        fixture.session_with(|options| {
            options.scheme = SchemeType::Wubi;
            options.wubi.mixed_pinyin = true;
        })
    };
    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('wqiy','你',100);\
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_w VALUES('wan''qi','wq','晚期',1000);",
    );
    let mut session = mixed(&fixture);
    type_text(&mut session, "wqi");
    let rows = schemes(&session);
    assert!(
        rows.iter()
            .any(|(_, scheme)| *scheme == SchemeType::Quanpin),
        "{rows:?}"
    );
    type_text(&mut session, "y");
    assert!(session.input.engine.wubi_has_complete_code());
    assert!(!session.input.engine.wubi_pinyin_continues(b'a'));

    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('guiy','诡',100);\
CREATE TABLE tbl_2_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_g VALUES('gui''yi','gy','诡异',1000);",
    );
    let mut session = mixed(&fixture);
    type_text(&mut session, "guiy");
    assert!(session.input.engine.wubi_pinyin_continues(b'i'));

    let fixture = Fixture::new(WUBI_BIANYI_FIXTURE);
    let mut session = mixed(&fixture);
    type_text(&mut session, "bian");
    assert!(session.input.engine.wubi_pinyin_continues(b'y'));

    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('qigu','乐平',100);\
CREATE TABLE tbl_2_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_q VALUES('qi''gu','qg','其故',100);",
    );
    let mut session = mixed(&fixture);
    type_text(&mut session, "qigu");
    assert!(session.input.engine.wubi_pinyin_continues(b'a'));

    // 整段四码加下一字母仍在同一个音节里。
    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('chan','合成',100);",
    );
    let mut session = mixed(&fixture);
    type_text(&mut session, "chan");
    assert!(session.input.engine.wubi_pinyin_continues(b'g'));
}

/// Backspace never goes through a reset, so the emptied composition itself has to end the tail; Cancel ends it too.
#[test]
fn an_emptied_or_cancelled_tail_gives_codes_back_to_the_wubi_table() {
    let fixture = Fixture::new(WUBI_TAIL_FIXTURE);
    let mut session = wubi_pinyin_tail(&fixture);
    session.command(Command::Backspace);
    session.command(Command::Backspace);
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "wo");
    assert_eq!(
        schemes(&session).first(),
        Some(&("人".to_owned(), SchemeType::Wubi))
    );

    let mut session = wubi_pinyin_tail(&fixture);
    session.command(Command::Cancel);
    type_text(&mut session, "wo");
    assert_eq!(
        schemes(&session).first(),
        Some(&("人".to_owned(), SchemeType::Wubi))
    );
}

/// Turning mixed input off ends the tail with it: switched back on, the same letters are a wubi code again.
#[test]
fn switching_mixed_input_off_ends_the_tail() {
    let fixture = Fixture::new(WUBI_TAIL_FIXTURE);
    let mut session = wubi_pinyin_tail(&fixture);
    session.set_wubi_mixed_pinyin(false);
    session.set_wubi_mixed_pinyin(true);
    assert_eq!(
        schemes(&session).first(),
        Some(&("人".to_owned(), SchemeType::Wubi))
    );
}

// ---- personal learning windows (set_clock) ----

#[test]
fn a_short_pause_keeps_the_chain_and_a_long_one_breaks_it() {
    let fixture = Fixture::new(CONTEXT_FIXTURE);
    for (pause, keeps) in [(3, true), (30, false)] {
        let mut session = fixture.session();
        let clock = TestClock::install(&mut session);
        type_text(&mut session, "ni");
        assert_eq!(
            select_word(&mut session, "甲").commit.as_deref(),
            Some("甲")
        );
        assert_eq!(session.input.chain.previous.as_deref(), Some("甲"));
        clock.advance(Duration::from_secs(pause));
        type_text(&mut session, "hao");
        assert_eq!(
            session.input.chain.previous.is_some(),
            keeps,
            "pause of {pause} s"
        );
        assert_eq!(
            select_word(&mut session, "丑").commit.as_deref(),
            Some("丑")
        );
        assert_eq!(
            session.input.chain.earlier.as_deref(),
            keeps.then_some("甲"),
            "pause of {pause} s"
        );
    }
}

#[test]
fn the_pause_is_measured_from_the_last_commit() {
    let fixture = Fixture::new(CONTEXT_FIXTURE);
    let mut session = fixture.session();
    let clock = TestClock::install(&mut session);
    type_text(&mut session, "ni");
    select_word(&mut session, "甲");
    clock.advance(Duration::from_secs(8));
    type_text(&mut session, "hao");
    assert!(
        session.input.chain.previous.is_some(),
        "eight seconds is not yet a pause"
    );
    session.command(Command::Cancel);
    clock.advance(Duration::from_millis(1));
    type_text(&mut session, "hao");
    assert!(session.input.chain.previous.is_none());
}

#[test]
fn picks_within_ten_seconds_become_a_phrase() {
    let fixture = Fixture::new(CONTEXT_FIXTURE);
    // The gap runs from one pick to the next, so it can outgrow ten seconds while the second word is being composed, without the eight second pause before a new composition ever applying.
    let pair = |session: &mut Session, clock: &TestClock, compose_after: u64, pick_after: u64| {
        type_text(session, "shan");
        select_word(session, "山");
        clock.advance(Duration::from_secs(compose_after));
        type_text(session, "shui");
        clock.advance(Duration::from_secs(pick_after));
        select_word(session, "水");
        session.reset_context();
    };
    let phrase = "SELECT count(*) FROM tbl_2_s WHERE key='shan''shui' AND value='山水'";

    let mut session = fixture.session();
    let clock = TestClock::install(&mut session);
    for _ in 0..3 {
        pair(&mut session, &clock, 5, 6);
    }
    assert_eq!(count(&fixture.main_db(), phrase), 0, "picks 11 s apart");

    for _ in 0..3 {
        pair(&mut session, &clock, 5, 5);
    }
    assert_eq!(count(&fixture.main_db(), phrase), 1, "picks 10 s apart");
}

// ---- test_input_session.cpp ports ----

#[test]
fn nihao_selection_and_commit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.raw_segmentation, "ni'hao");
    assert_eq!(words(&session)[..2], ["你好".to_owned(), "拟好".to_owned()]);
    assert!(!session.select(99).handled);
    assert_eq!(session.select(1).commit.as_deref(), Some("拟好"));
    assert!(session.snapshot().preedit.is_empty());

    type_text(&mut session, "nihao");
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("你好")
    );
    type_text(&mut session, "nihao");
    assert_eq!(session.punctuation(b',').commit.as_deref(), Some("你好，"));
    type_text(&mut session, "nihao");
    assert_eq!(session.candidate_key(b'2').commit.as_deref(), Some("拟好"));
}

#[test]
fn portable_selection_keeps_the_rest_and_learns_the_phrase() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "xi'te'le");
    assert_eq!(
        select_word(&mut session, "西").commit.as_deref(),
        Some("西")
    );
    assert_eq!(session.snapshot().preedit, "te'le");
    let result = select_word(&mut session, "特乐");
    assert_eq!(result.commit.as_deref(), Some("特乐"));
    assert!(session.snapshot().preedit.is_empty());
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM tbl_3_x WHERE key='xi''te''le' AND value='西特乐'"
        ),
        1
    );

    type_text(&mut session, "xi'te'le");
    select_word(&mut session, "西");
    assert_eq!(session.punctuation(b',').commit.as_deref(), Some("特乐，"));
}

#[test]
fn backspacing_away_the_rest_abandons_the_phrase() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "xi'te'le");
    select_word(&mut session, "西");
    while !session.snapshot().preedit.is_empty() {
        assert!(session.command(Command::Backspace).handled);
    }
    type_text(&mut session, "nihao");
    assert_eq!(
        select_word(&mut session, "你好").commit.as_deref(),
        Some("你好")
    );
    assert_eq!(
        count(
            &fixture.main_db(),
            "SELECT count(*) FROM tbl_3_x WHERE value='西你好'"
        ),
        0
    );

    type_text(&mut session, "xi'te'le");
    select_word(&mut session, "西");
    assert!(session.command(Command::Cancel).handled);
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn selection_completion_prediction_matches_the_selection() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut probe = fixture.session();
    type_text(&mut probe, "xi'te'le");
    let count = probe.snapshot().candidates.len();
    for index in 0..count {
        let mut session = fixture.session();
        type_text(&mut session, "xi'te'le");
        let predicted = session.snapshot().candidate_answers_key[index];
        session.select(index);
        assert_eq!(
            session.snapshot().preedit.is_empty(),
            predicted,
            "candidate {index}"
        );
    }
}

#[test]
fn edge_selection_commits_one_han_character() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    for (word, first, last) in [
        ("拟好", "拟", "好"),
        ("𠀀方案𠮷", "𠀀", "𠮷"),
        ("C语言 2", "语", "言"),
    ] {
        type_text(&mut session, "nihao");
        let index = index_of(&session, word);
        let result = session.select_edge(index, CandidateEdge::FirstHan);
        assert_eq!(result.commit.as_deref(), Some(first));
        assert!(session.snapshot().preedit.is_empty());
        type_text(&mut session, "nihao");
        let result = session.select_edge(index, CandidateEdge::LastHan);
        assert_eq!(result.commit.as_deref(), Some(last));
    }
    type_text(&mut session, "nihao");
    let github = index_of(&session, "GitHub");
    assert!(!session.select_edge(github, CandidateEdge::FirstHan).handled);
    assert_eq!(session.snapshot().preedit, "nihao");
}

#[test]
fn backspace_commit_raw_cancel_and_scheme_switch() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().preedit, "niha");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("niha")
    );
    type_text(&mut session, "nihao");
    let cancelled = session.command(Command::Cancel);
    assert!(cancelled.handled && cancelled.commit.is_none());
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "nihao");
    session.switch_scheme(SchemeType::Shuangpin).unwrap();
    assert!(session.snapshot().preedit.is_empty());
    assert_eq!(session.snapshot().scheme, SchemeType::Shuangpin);
}

#[test]
fn idle_keys_pass_through() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert!(!session.character(b'1', false).handled);
    assert!(!session.command(Command::Backspace).handled);
    assert!(!session.command(Command::CommitRaw).handled);
    assert!(!session.select(0).handled);
    assert!(!session.character(b'\'', false).handled);
    assert!(!session.character(b'N', false).handled);
    assert!(!session.candidate_key(b'1').handled);
}

#[test]
fn shifted_letter_keeps_unfinished_quanpin_when_helpcode_is_off() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('ma','m','马',100);\
INSERT INTO tbl_1_m VALUES('mao','m','猫',100);",
    );
    let mut session = fixture.session_with(|options| options.helpcode = false);
    type_text(&mut session, "ma");
    assert!(words(&session).contains(&"马".to_owned()));

    let shifted = session.character(b'O', true);
    assert!(shifted.handled);
    assert!(shifted.commit.is_none());
    assert_eq!(session.snapshot().preedit, "maO");
    assert!(words(&session).contains(&"马O".to_owned()));
    assert!(!words(&session).contains(&"猫".to_owned()));
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("maO")
    );
}

#[test]
fn shifted_letter_inserts_at_caret_without_committing_quanpin() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.helpcode = false);
    type_text(&mut session, "nihao");
    session.set_caret(Some(2));

    let shifted = session.character(b'O', true);
    assert!(shifted.handled);
    assert!(shifted.commit.is_none());
    assert_eq!(session.snapshot().preedit, "niOhao");
}

#[test]
fn shifted_letter_keeps_unfinished_shuangpin_when_helpcode_is_off() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.helpcode = false;
    });
    type_text(&mut session, "ma");

    let shifted = session.character(b'O', true);
    assert!(shifted.handled);
    assert!(shifted.commit.is_none());
    assert_eq!(session.snapshot().preedit, "maO");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("maO")
    );
}

#[test]
fn shifted_literal_closes_the_shuangpin_online_query_without_helpcode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.helpcode = false;
    });
    type_text(&mut session, "ni'h");

    assert!(session.character(b'C', true).handled);
    assert!(session.online_query().is_none());
}

#[test]
fn uppercase_helpcode_and_duplicate_apostrophe_gates() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni'");
    assert!(!session.character(b'\'', false).handled);
    session.command(Command::Cancel);
    type_text(&mut session, "ni");
    assert!(session.character(b'H', false).handled);
    session.command(Command::Cancel);

    let mut disabled = fixture.session_with(|options| options.helpcode = false);
    type_text(&mut disabled, "ni");
    assert!(disabled.character(b'H', false).handled);
    assert_eq!(disabled.snapshot().preedit, "niH");
}

#[test]
fn segment_boundaries_per_scheme() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihaoma");
    assert_eq!(session.segment_raw_boundaries(), vec![0, 2, 5, 7]);
    session.command(Command::Cancel);
    type_text(&mut session, "ni'hao");
    assert_eq!(session.segment_raw_boundaries(), vec![0, 3, 6]);
    session.command(Command::Cancel);
    assert!(session.segment_raw_boundaries().is_empty());

    let mut shuangpin = fixture.session_with(|options| options.scheme = SchemeType::Shuangpin);
    type_text(&mut shuangpin, "nihaoma");
    assert_eq!(shuangpin.segment_raw_boundaries(), vec![0, 2, 4, 5, 7]);

    let mut wubi = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    type_text(&mut wubi, "aaaa");
    assert!(wubi.segment_raw_boundaries().is_empty());

    assert!(session.character(b'U', true).handled);
    assert!(session.segment_raw_boundaries().is_empty());
}

#[test]
fn caret_editing_keeps_the_local_marker() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert!(session.character(b'K', true).handled);
    type_text(&mut session, "ab");
    session.command(Command::MoveHome);
    assert_eq!(session.snapshot().caret_position, 1);
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().editing_text, "Kab");
    session.command(Command::MoveLeft);
    assert_eq!(session.snapshot().caret_position, 1);
    session.command(Command::Cancel);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    type_text(&mut session, "ni");
    session.command(Command::MoveHome);
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().editing_text, "ni");
    session.command(Command::MoveRight);
    session.character(b'\'', false);
    session.character(b'\'', false);
    assert_eq!(session.snapshot().editing_text, "n'i");
    session.command(Command::MoveEnd);
    assert!(session.command(Command::DeleteForward).handled);
    assert_eq!(session.snapshot().editing_text, "n'i");
}

#[test]
fn an_edit_rejects_the_answer_to_the_old_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    assert_eq!(query.query_text, "ni");
    assert_eq!(query.identity, "0:ni");
    assert_eq!(query.identity.capacity(), query.identity.len());
    assert!(query.cloud_eligible && query.ai_eligible);
    session.command(Command::MoveHome);
    session.command(Command::DeleteForward);
    session.character(b'n', false);
    session.command(Command::MoveEnd);
    assert_eq!(session.snapshot().editing_text, "ni");
    assert!(!session.apply_online_candidate(&query, "旧响应", CandidateSource::CloudSuggestion));
    let live = session.online_query().expect("a quanpin query");
    assert!(session.apply_online_candidate(&live, "妮", CandidateSource::CloudSuggestion));
    assert!(words(&session).contains(&"妮".to_owned()));
}

#[test]
fn accepting_an_online_answer_does_not_rebuild_the_live_query() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    let (accepted, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.apply_online_candidate(&query, "妮", CandidateSource::CloudSuggestion)
    });

    assert!(accepted);
    assert!(
        allocations <= 41,
        "在线回包校验不应重建完整查询，仍申请了 {allocations} 次分配"
    );
}

/// test_runtime_isolation.cpp:257-268: moving the caret changes no query field, so an answer to the unchanged composition is still accepted.
#[test]
fn caret_only_movement_keeps_the_online_generation() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    for command in [
        Command::MoveHome,
        Command::MoveRight,
        Command::MoveLeft,
        Command::MoveEnd,
    ] {
        session.command(command);
        assert_eq!(
            session.online_query().map(|live| live.generation),
            Some(query.generation),
            "{command:?}"
        );
    }
    session.command(Command::MoveHome);
    // Nothing precedes the caret, so Backspace edits nothing (C++ :265-266).
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().editing_text, "ni");
    assert_eq!(
        session.online_query().map(|live| live.generation),
        Some(query.generation)
    );
    session.command(Command::MoveEnd);
    assert!(session.apply_online_candidate(&query, "妮", CandidateSource::CloudSuggestion));
    assert!(words(&session).contains(&"妮".to_owned()));
}

/// While a caret prefix is decoded, an answer to the whole input goes into the whole input's list, which is not on screen: the call reports false, and the row shows once the caret returns to the end.
#[test]
fn caret_prefix_defers_a_whole_input_answer_until_move_end() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let query = session.online_query().expect("a quanpin query");
    session.set_caret(Some(2));
    assert_eq!(session.prefix_end(), 2);
    assert!(!session.apply_online_candidate(&query, "妮好", CandidateSource::CloudSuggestion));
    assert!(!words(&session).contains(&"妮好".to_owned()));
    session.command(Command::MoveEnd);
    let snapshot = session.snapshot();
    let row = snapshot
        .candidates
        .iter()
        .find(|item| item.word == "妮好")
        .expect("the answer shows at the end");
    assert_eq!(row.source, CandidateSource::CloudSuggestion);
}

/// test_online_input_session.cpp:149-176 through the singular entry every cloud response takes: cloud at the second seat, AI at the end (the reference seated it third; input-runtime now shows the first AI row in the last slot of page one, #6888), a new cloud answer replaces the old one, and a word the list already holds or a non-online source is refused without touching the list.
#[test]
fn a_single_online_answer_takes_its_slot_and_refuses_duplicates() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100),('ni','n','拟',90),('ni','n','妮',80),('ni','n','倪',70);",
    );
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    let at = |session: &Session, slot: usize| {
        let item = session.snapshot().candidates[slot].clone();
        (item.word.into_string(), item.source)
    };

    assert!(session.apply_online_candidate(&query, "泥", CandidateSource::CloudSuggestion));
    assert_eq!(
        at(&session, 1),
        ("泥".to_owned(), CandidateSource::CloudSuggestion)
    );
    assert!(session.apply_online_candidate(&query, "逆", CandidateSource::AiSuggestion));
    assert_eq!(
        at(&session, 5),
        ("逆".to_owned(), CandidateSource::AiSuggestion)
    );
    assert_eq!(words(&session), ["你", "泥", "拟", "妮", "倪", "逆"]);
    assert!(session.apply_online_candidate(&query, "呢", CandidateSource::CloudSuggestion));
    assert_eq!(
        at(&session, 1),
        ("呢".to_owned(), CandidateSource::CloudSuggestion)
    );
    assert!(!words(&session).contains(&"泥".to_owned()));

    let before = session.snapshot().candidates;
    assert!(!session.apply_online_candidate(&query, "你", CandidateSource::CloudSuggestion));
    assert_eq!(session.snapshot().candidates, before);
    assert!(!session.apply_online_candidate(&query, "腻", CandidateSource::Database));
    assert_eq!(session.snapshot().candidates, before);
}

#[test]
fn cloud_answer_replaces_its_fixed_pending_seat() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100),('ni','n','拟',90),('ni','n','妮',80);",
    );
    let mut session = fixture.session();
    session.set_cloud_candidates_enabled(true);
    type_text(&mut session, "ni");
    let before = session.snapshot();
    assert_eq!(before.candidates[1].word, "☁");
    assert_eq!(before.candidate_sources[1], CandidateSource::CloudPending);

    let query = session.online_query().expect("a cloud query");
    assert!(session.apply_online_candidate(&query, "泥", CandidateSource::CloudSuggestion));
    let after = session.snapshot();
    assert_eq!(after.candidates[1].word, "泥");
    assert_eq!(after.candidate_sources[1], CandidateSource::CloudSuggestion);
    assert_eq!(&after.candidates[2..], &before.candidates[2..]);
}

#[test]
fn clearing_one_online_source_removes_cached_rows_but_keeps_the_other_source() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100),('ni','n','拟',90);",
    );
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    assert!(session.apply_online_candidate(&query, "云候选", CandidateSource::CloudSuggestion));
    assert!(session.apply_online_candidate(&query, "AI候选", CandidateSource::AiSuggestion));
    assert!(words(&session).contains(&"云候选".to_owned()));
    assert!(words(&session).contains(&"AI候选".to_owned()));

    session.clear_online_candidates(CandidateSource::CloudSuggestion);
    assert!(!words(&session).contains(&"云候选".to_owned()));
    assert!(words(&session).contains(&"AI候选".to_owned()));

    session.command(Command::Cancel);
    type_text(&mut session, "ni");
    assert!(!words(&session).contains(&"云候选".to_owned()));
    assert!(words(&session).contains(&"AI候选".to_owned()));
}

#[test]
fn clearing_online_source_during_nine_key_mode_drops_cached_rows() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100),('ni','n','拟',90);",
    );
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    assert!(session.apply_online_candidate(&query, "云候选", CandidateSource::CloudSuggestion));
    session.command(Command::Cancel);

    session.set_nine_key_enabled(true);
    assert!(session.character(b'6', false).handled);
    session.clear_online_candidates(CandidateSource::CloudSuggestion);
    session.set_nine_key_enabled(false);
    type_text(&mut session, "ni");
    assert!(!words(&session).contains(&"云候选".to_owned()));
}

#[test]
fn repeated_dedicated_english_preserves_chinese_grid_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for grid in [KeyGrid::NineKey, KeyGrid::FourteenKey] {
        let mut session = fixture.session();
        session.set_key_grid(Some(grid));
        if grid == KeyGrid::FourteenKey {
            assert!(session.grid_key(b'n').handled);
            assert!(session.grid_key(b'i').handled);
        } else {
            type_text(&mut session, "64");
        }
        assert!(session.grid_composing());
        let before = session.snapshot();
        assert!(!before.candidates.is_empty());

        session.set_dedicated_english(false);

        assert_eq!(session.snapshot(), before, "{grid:?}");
        assert_eq!(
            session.select(0).commit,
            Some(before.candidates[0].word.to_string())
        );
    }
}

#[test]
fn repeated_dedicated_english_preserves_english_t9_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    session.set_nine_key_enabled(true);
    session.set_dedicated_english(true);
    type_text(&mut session, "435");
    let before = session.snapshot();
    assert!(session.grid_composing());
    assert!(!before.candidates.is_empty());

    session.set_dedicated_english(true);

    assert_eq!(session.snapshot(), before);
    assert_eq!(
        session.select(0).commit,
        Some(before.candidates[0].word.to_string())
    );

    type_text(&mut session, "435");
    session.set_dedicated_english(false);
    assert!(!session.grid_composing());
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "64");
    session.set_dedicated_english(true);
    assert!(!session.grid_composing());
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn fourteen_key_shuangpin_separates_touch_groups_from_hardware_letters() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
    });
    let table = crate::nine_key_shuangpin::resolve_fourteen_key_shuangpin(
        ShuangpinProfileKind::Xiaohe,
        None,
    )
    .unwrap();
    session.set_key_grid(Some(KeyGrid::FourteenKeyShuangpin(table)));
    type_text(&mut session, "nihc");
    assert!(!session.grid_composing());
    assert!(!session.grid_key(b'n').handled);
    assert_eq!(
        select_word(&mut session, "你好").commit.as_deref(),
        Some("你好")
    );
    for key in *b"nihc" {
        assert!(session.grid_key(key).handled);
    }
    assert!(session.grid_composing());
    assert_eq!(session.snapshot().editing_text, "bugc");
    assert_eq!(session.snapshot().scheme, SchemeType::Shuangpin);
    assert!(!session.character(b'q', false).handled);
    assert!(words(&session).contains(&"你好".to_owned()));
    assert_eq!(
        select_word(&mut session, "你好").commit.as_deref(),
        Some("你好")
    );
    session.set_dedicated_english(true);
    assert!(!session.grid_key(b'n').handled);
    session.set_dedicated_english(false);
    assert!(session.grid_key(b'n').handled);
    session.command(Command::Cancel);
    let mut quanpin = fixture.session();
    quanpin.set_key_grid(Some(KeyGrid::FourteenKeyShuangpin(table)));
    assert!(!quanpin.grid_key(b'n').handled);
}

/// 无候选时仍上屏小写字母，不能泄漏内部精确标记。
#[test]
fn eighteen_key_shuangpin_exact_markers_do_not_escape_raw_fallback() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for finish in [false, true] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
            options.english.mixed_candidates = false;
        });
        let table = crate::nine_key_shuangpin::resolve_eighteen_key_shuangpin(
            ShuangpinProfileKind::Xiaohe,
            None,
        )
        .unwrap();
        session.set_key_grid(Some(KeyGrid::EighteenKeyShuangpin(table)));
        assert!(session.grid_key(b'I').handled);
        assert!(session.snapshot().candidates.is_empty());
        let result = if finish {
            session.finish(0)
        } else {
            session.command(Command::CommitRaw)
        };
        assert_eq!(result.commit.as_deref(), Some("i"));
        assert!(session.snapshot().editing_text.is_empty());
    }
}

/// 逐段结束中文后，无候选的余码也不能泄漏内部精确标记。
#[test]
fn eighteen_key_shuangpin_exact_markers_do_not_escape_finish_tail() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        options.english.mixed_candidates = false;
    });
    let table = crate::nine_key_shuangpin::resolve_eighteen_key_shuangpin(
        ShuangpinProfileKind::Xiaohe,
        None,
    )
    .unwrap();
    session.set_key_grid(Some(KeyGrid::EighteenKeyShuangpin(table)));
    for key in b"NiI" {
        assert!(session.grid_key(*key).handled);
    }
    assert_eq!(session.snapshot().candidates[0].word, "你");
    assert_eq!(session.finish(0).commit.as_deref(), Some("你i"));
    assert!(session.snapshot().editing_text.is_empty());
}

/// 横滑的确定字母保留在同一组码串；点按仍可与它交替。
#[test]
fn eighteen_key_shuangpin_exact_letters_constrain_the_current_grid() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
    });
    let table = crate::nine_key_shuangpin::resolve_eighteen_key_shuangpin(
        ShuangpinProfileKind::Xiaohe,
        None,
    )
    .unwrap();
    session.set_key_grid(Some(KeyGrid::EighteenKeyShuangpin(table)));
    for key in b"bi" {
        assert!(session.grid_key(*key).handled);
    }
    let ambiguous = session.snapshot().nine_key_spellings;
    assert!(ambiguous.iter().any(|s| s == "bi") && ambiguous.iter().any(|s| s == "ni"));
    session.command(Command::Cancel);

    assert!(session.grid_key(b'N').handled, "十八键双拼应接收精确字母");
    assert!(session.grid_key(b'i').handled);
    let exact = session.snapshot();
    assert_eq!(exact.editing_text, "Ni");
    assert!(exact.nine_key_spellings.iter().any(|s| s == "ni"));
    assert!(!exact.nine_key_spellings.iter().any(|s| s == "bi"));
    assert_eq!(
        exact
            .nine_key_spellings
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        exact.nine_key_spellings.len()
    );
    for key in b"hC" {
        assert!(session.grid_key(*key).handled);
    }
    assert_eq!(
        select_word(&mut session, "你好").commit.as_deref(),
        Some("你好")
    );

    // 中间编辑仍在精确网格中；退格只删除当前位置之前的一位。
    for key in b"Ni" {
        assert!(session.grid_key(*key).handled);
    }
    session.command(Command::MoveLeft);
    assert!(session.grid_key(b'H').handled);
    assert_eq!(session.snapshot().editing_text, "NHi");
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().editing_text, "Ni");
    session.command(Command::DeleteForward);
    assert_eq!(session.snapshot().editing_text, "N");
    assert!(session.grid_key(b'I').handled);
    assert_eq!(session.snapshot().editing_text, "NI");
    let ni = session
        .snapshot()
        .nine_key_spellings
        .iter()
        .position(|s| s == "ni")
        .unwrap();
    assert!(session.choose_nine_key_spelling(ni).handled);
    assert_eq!(session.snapshot().nine_key_reading, "ni");
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().editing_text, "NI");
    assert!(!session
        .snapshot()
        .nine_key_spellings
        .iter()
        .any(|s| s == "bi"));
    session.command(Command::Cancel);
    assert!(session.character(b'n', false).handled);
    assert!(!session.grid_composing());
    assert!(!session.grid_key(b'I').handled);
    assert_eq!(session.snapshot().editing_text, "n");
    session.command(Command::Cancel);
    session.set_dedicated_english(true);
    assert!(!session.grid_key(b'N').handled);

    let mut quanpin = fixture.session();
    quanpin.set_key_grid(Some(KeyGrid::EighteenKey));
    assert!(!quanpin.grid_key(b'N').handled);
    assert!(quanpin.grid_key(b'n').handled);
    let fourteen = crate::nine_key_shuangpin::resolve_fourteen_key_shuangpin(
        ShuangpinProfileKind::Xiaohe,
        None,
    )
    .unwrap();
    session.set_dedicated_english(false);
    session.set_key_grid(Some(KeyGrid::FourteenKeyShuangpin(fourteen)));
    assert!(!session.grid_key(b'N').handled);
    assert!(session.grid_key(b'n').handled);
}

/// 14 键：组码只从 `grid_key` 进九宫格，组里任一字母都归成首字母；硬件键盘的字母照常走全拼，九宫格组字中不处理。
#[test]
fn fourteen_key_letters_enter_the_grid_only_through_grid_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    session.set_key_grid(Some(KeyGrid::FourteenKey));
    assert!(session.character(b'q', false).handled);
    assert!(!session.nine_key.active());
    assert_eq!(session.snapshot().editing_text, "q");
    // 26 键正在组字：`grid_key` 不处理，状态不变。
    assert!(!session.grid_key(b'n').handled);
    assert_eq!(session.snapshot().editing_text, "q");
    session.command(Command::Cancel);

    for letter in *b"nihao" {
        assert!(session.grid_key(letter).handled);
    }
    assert!(session.nine_key.active());
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "bugao");
    let candidates = words(&session);
    assert!(candidates.contains(&"你好".to_owned()) && candidates.contains(&"不好".to_owned()));
    // 九宫格组字中硬件字母和数字都不处理，`'` 切分。
    assert!(!session.character(b'h', false).handled);
    assert!(!session.character(b'6', false).handled);
    assert!(!session.grid_key(b'1').handled);
    assert!(!session.grid_key(b'A').handled);
    assert_eq!(session.snapshot().editing_text, "bugao");
    session.command(Command::Cancel);

    // 九键下数字进九宫格，`grid_key` 不处理。
    session.set_key_grid(Some(KeyGrid::NineKey));
    assert!(!session.grid_key(b'n').handled);
    assert!(session.character(b'6', false).handled);
    assert!(session.nine_key.active());
    session.command(Command::Cancel);

    // 专用英文画全键盘，`grid_key` 不处理；14 键不打开英文九键。
    session.set_key_grid(Some(KeyGrid::FourteenKey));
    session.set_dedicated_english(true);
    assert!(!session.grid_key(b'o').handled);
    assert!(!session.nine_key.active());
    session.set_dedicated_english(false);

    // 全拼以外的方案没有 14 键。
    session.switch_scheme(SchemeType::Shuangpin).unwrap();
    assert!(!session.grid_key(b'o').handled);
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    assert!(session.grid_key(b'o').handled);
    session.command(Command::Cancel);

    session.set_key_grid(None);
    assert!(!session.grid_key(b'o').handled);
    session.character(b'6', false);
    assert!(!session.nine_key.active());
}

/// Loading a helpcode table drops the cached pinyin answers, online rows included, as the reference's keymap setters did (quanpin/engine.h:37-41); the golden ri_session_a_resources records the same sequence.
#[test]
fn a_new_helpcode_table_drops_the_online_rows_of_an_earlier_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    assert!(session.apply_online_candidates(
        &query,
        &["本会话建议".to_owned()],
        CandidateSource::CloudSuggestion
    ));
    session.command(Command::Cancel);
    // Without a table change the series cache keeps the row for the same key.
    type_text(&mut session, "ni");
    assert!(words(&session).contains(&"本会话建议".to_owned()));
    session.command(Command::Cancel);
    assert!(session.set_helpcode_schema("lantian"));
    type_text(&mut session, "ni");
    let snapshot = session.snapshot();
    assert!(!snapshot.candidates.is_empty());
    assert!(
        snapshot
            .candidates
            .iter()
            .all(|item| !item.source.is_online()),
        "{:?}",
        words(&session)
    );
}

/// test_input_session.cpp (longer phrases): a row that continues past the typed syllables keeps the typed reading as its pinyin, and selecting it commits the whole phrase and ends the composition.
#[test]
fn selecting_a_longer_phrase_commits_it_whole() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',8000),('ni','n','泥',7000);\
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',10000),('ni''hao','nh','拟好',30);\
CREATE TABLE tbl_3_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_3_n VALUES('ni''hao''ma','nhm','你好吗',900);",
    );
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let index = index_of(&session, "你好吗");
    let row = &session.snapshot().candidates[index];
    assert_eq!(row.pinyin, "ni'hao");
    assert_eq!(row.canonical_pinyin, "ni'hao'ma");
    let result = session.select(index);
    assert!(result.handled);
    assert_eq!(result.commit.as_deref(), Some("你好吗"));
    assert!(session.snapshot().preedit.is_empty());
}

// ---- isolation between sessions (test_runtime_isolation.cpp:367-413, :447-464) ----

/// One root of test_runtime_isolation.cpp:39-85: its own `ni` rows, quick phrase, helpcode tables and Japanese model, each naming `own` so a leak from the other root shows.
fn isolation_root(own: &str, own_kanji: &str) -> Fixture {
    let fixture = Fixture::new(&format!(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','{own}',10000),('ni','n','拟',9000);\
CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('x','{own}短语',10);"
    ));
    let helpcodes = fixture.path().join("helpcodes");
    std::fs::write(helpcodes.join("helpcode.txt"), format!("{own}=aa\n拟=cc\n")).unwrap();
    std::fs::write(
        helpcodes.join("xiaohe_helpcode.txt"),
        format!("{own}=cc\n拟=aa\n"),
    )
    .unwrap();
    // The recorder's one-entry model (reading かな), with the entry's word swapped; both words are three UTF-8 bytes, so every offset holds.
    let model = format!(
        "MSJPDT1\u{0}\u{1}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}8\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}L\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}N\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\t\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{6}\u{0}\u{6}\u{0}\u{0}\u{0}\u{3}\u{0}\u{0}\u{0}\u{0}\u{0}\u{1}\u{0}\u{0}\u{0}\u{0}\u{0}かな{own_kanji}"
    );
    std::fs::write(fixture.path().join(assets::JAPANESE_MODEL), model).unwrap();
    fixture
}

/// Two sessions on different roots stay open together and never see each other's dictionary, online rows, helpcode table, quote pairing, quick phrases or Japanese model (test_runtime_isolation.cpp:367-413, :447-464).
#[test]
fn sessions_on_different_roots_stay_isolated() {
    let (root_a, root_b) = (isolation_root("你", "甲"), isolation_root("妮", "乙"));
    let mut a = root_a.session();
    let mut b = root_b.session();

    type_text(&mut a, "ni");
    type_text(&mut b, "ni");
    assert_eq!(words(&a)[0], "你");
    assert_eq!(words(&b)[0], "妮");
    let query = a.online_query().expect("a quanpin query");
    assert!(a.apply_online_candidates(
        &query,
        &["本会话建议".to_owned()],
        CandidateSource::CloudSuggestion
    ));
    assert!(!b.apply_online_candidates(
        &query,
        &["跨会话建议".to_owned()],
        CandidateSource::CloudSuggestion
    ));
    assert!(words(&a).contains(&"本会话建议".to_owned()));
    assert!(!words(&b).iter().any(|word| word.contains("会话建议")));
    a.command(Command::Cancel);
    b.command(Command::Cancel);

    assert!(a.set_helpcode_schema("lantian"));
    assert!(b.set_helpcode_schema("xiaohe"));
    type_text(&mut a, "niC");
    type_text(&mut b, "niC");
    assert_eq!(words(&a), ["拟", "你"]);
    assert_eq!(words(&b), ["妮", "拟"]);
    a.command(Command::Cancel);
    b.command(Command::Cancel);

    assert_eq!(a.punctuation(b'"').commit.as_deref(), Some("\u{201c}"));
    assert_eq!(b.punctuation(b'"').commit.as_deref(), Some("\u{201c}"));
    assert_eq!(a.punctuation(b'"').commit.as_deref(), Some("\u{201d}"));

    for (session, phrase) in [(&mut a, "你短语"), (&mut b, "妮短语")] {
        assert!(session.character(b'K', true).handled);
        assert!(session.character(b'x', false).handled);
        assert_eq!(words(session), [phrase]);
        session.command(Command::Cancel);
    }

    a.switch_scheme(SchemeType::JapaneseRomaji).unwrap();
    b.switch_scheme(SchemeType::JapaneseRomaji).unwrap();
    type_text(&mut a, "kana");
    type_text(&mut b, "kana");
    assert_eq!(words(&a)[0], "甲");
    assert_eq!(words(&b)[0], "乙");
}

/// 切到日文时模型在后台读进进程级缓存，下一个会话（Android 每个输入框一个）直接拿它；宿主清缓存（iOS 内存告警）把它放掉。
#[test]
fn japanese_model_outlives_sessions_until_the_host_resets_caches() {
    use crate::japanese::decoder::{JapaneseDictionary, SHARED_CACHE_TEST_LOCK};

    let _serial = SHARED_CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = isolation_root("你", "甲");
    let model = root.path().join(assets::JAPANESE_MODEL);
    let mut first = root.session();
    first.switch_scheme(SchemeType::JapaneseRomaji).unwrap();
    JapaneseDictionary::wait_until_warmed(&model);
    assert!(JapaneseDictionary::is_shared(&model));
    drop(first);
    assert!(JapaneseDictionary::is_shared(&model));

    let mut second = root.session();
    second.switch_scheme(SchemeType::JapaneseRomaji).unwrap();
    type_text(&mut second, "kana");
    assert_eq!(words(&second)[0], "甲");
    second.reset_cache();
    assert!(!JapaneseDictionary::is_shared(&model));
    second.command(Command::Cancel);
    type_text(&mut second, "kana");
    assert_eq!(words(&second)[0], "甲");

    // 清缓存时第二个会话还拿着模型：下一个会话拿回同一份，整个过程只读过一遍文件。
    let mut third = root.session();
    third.switch_scheme(SchemeType::JapaneseRomaji).unwrap();
    JapaneseDictionary::wait_until_warmed(&model);
    type_text(&mut third, "kana");
    assert_eq!(words(&third)[0], "甲");
    assert_eq!(JapaneseDictionary::load_count(&model), 1);
}

/// test_runtime_isolation.cpp:367-413: twenty threads, each with its own session on one of two roots, read only their own root's dictionary.
#[test]
fn concurrent_sessions_read_only_their_own_dictionary() {
    let roots = [isolation_root("你", "甲"), isolation_root("妮", "乙")];
    std::thread::scope(|scope| {
        for thread in 0..20 {
            let (root, own, other) = if thread % 2 == 0 {
                (&roots[0], "你", "妮")
            } else {
                (&roots[1], "妮", "你")
            };
            scope.spawn(move || {
                let mut session = root.session();
                type_text(&mut session, "ni");
                let listed = words(&session);
                assert_eq!(listed[0], own, "thread {thread}");
                assert!(
                    !listed.iter().any(|word| word == other),
                    "thread {thread}: {listed:?}"
                );
            });
        }
    });
}

// ---- local mode fallback rows and temporary modes (overlays.md §8.1, test_temporary_input_session.cpp) ----

#[test]
fn temporary_english_shows_its_prefix_until_a_word_matches() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    assert!(session.character(b'Y', true).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "Y");
    assert_eq!(snapshot.local_mode, LocalInputMode::TemporaryEnglish);
    assert_eq!(snapshot.candidates.len(), 1);
    assert_eq!(snapshot.candidates[0].word, "Y");
    assert_eq!(snapshot.candidates[0].source, CandidateSource::Fallback);
    assert_eq!(session.punctuation(b',').commit.as_deref(), Some("Y，"));

    session.character(b'Y', true);
    type_text(&mut session, "he");
    let snapshot = session.snapshot();
    assert!(snapshot
        .candidates
        .iter()
        .all(|item| item.source != CandidateSource::Fallback));
    assert_eq!(snapshot.candidates[0].source, CandidateSource::Generated);
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("Yhe")
    );

    session.character(b'Y', true);
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("Y")
    );
    session.character(b'Y', true);
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
}

#[test]
fn selecting_a_local_generated_candidate_does_not_clone_its_learning_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    assert!(session.character(b'Y', true).handled);
    type_text(&mut session, "he");
    assert_eq!(
        session.snapshot().candidates[0].source,
        CandidateSource::Generated
    );

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(0));

    assert_eq!(result.commit.as_deref(), Some("he"));
    assert!(result.diagnostic.is_none());
    assert!(session.snapshot().preedit.is_empty());
    assert!(
        allocations <= 3,
        "本地生成候选提交产生了 {allocations} 次分配"
    );
}

#[test]
fn selecting_a_temporary_english_candidate_does_not_clone_full_ranking_rows() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
    });
    assert!(session.character(b'Y', true).handled);
    type_text(&mut session, "he");
    assert_eq!(
        session.snapshot().candidates[0].source,
        CandidateSource::Generated
    );
    assert_eq!(
        session.snapshot().candidates[2].source,
        CandidateSource::EnglishDictionary
    );

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(2));

    assert_eq!(result.commit.as_deref(), Some("Help"));
    assert!(result.diagnostic.is_none(), "{result:?}");
    assert!(session.snapshot().preedit.is_empty());
    let english = fixture.path().join(assets::ENGLISH_DICTIONARY);
    assert!(
        count(
            &english,
            "SELECT weight FROM english_words WHERE word='help' AND display='Help'"
        ) > 90
    );
    assert!(
        allocations <= 67,
        "temporary English frequency selection allocations: {allocations}"
    );
}

#[test]
fn japanese_long_vowel_can_be_inserted_inside_the_romaji() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::JapaneseRomaji);
    type_text(&mut session, "kohi-");
    session.set_caret(Some(2));

    let result = session.character(b'-', false);

    assert!(result.handled);
    assert!(result.commit.is_none());
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "ko-hi-");
    assert_eq!(snapshot.caret_position, 3);
    assert!(words(&session).contains(&"こーひー".to_owned()));
    assert!(words(&session).contains(&"コーヒー".to_owned()));
}

#[test]
fn japanese_long_vowel_can_be_inserted_before_the_first_romaji() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::JapaneseRomaji);
    type_text(&mut session, "ka");
    session.command(Command::MoveHome);

    let result = session.character(b'-', false);

    assert!(result.handled);
    assert!(result.commit.is_none());
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "-ka");
    assert_eq!(snapshot.caret_position, 1);
    assert!(words(&session).contains(&"ーか".to_owned()));
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().editing_text, "ka");
    assert_eq!(session.snapshot().caret_position, 0);
}

#[test]
fn chinese_caret_editing_does_not_claim_the_japanese_long_vowel_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| options.scheme = scheme);
        type_text(&mut session, "ka");
        session.set_caret(Some(1));
        let before = session.snapshot();

        let result = session.character(b'-', false);

        assert!(!result.handled, "{scheme:?}");
        assert!(result.commit.is_none(), "{scheme:?}");
        assert_eq!(session.snapshot(), before, "{scheme:?}");
    }
}

#[test]
fn temporary_japanese_long_vowels_preserve_composition_and_commit_katakana() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        for punctuation_route in [false, true] {
            let mut session = fixture.session_with(|options| options.scheme = scheme);
            assert!(session.character(b'R', true).handled);
            for byte in b"ko-hi-" {
                let result = if punctuation_route && *byte == b'-' {
                    session.punctuation(*byte)
                } else {
                    session.character(*byte, false)
                };
                assert!(result.handled, "{scheme:?}, {punctuation_route}, {byte}");
                assert!(result.commit.is_none(), "{result:?}");
            }
            let snapshot = session.snapshot();
            assert_eq!(snapshot.editing_text, "Rko-hi-");
            assert_eq!(snapshot.scheme, scheme);
            assert_eq!(snapshot.local_mode, LocalInputMode::TemporaryJapanese);
            let index = snapshot
                .candidates
                .iter()
                .position(|row| row.word == "コーヒー")
                .unwrap();
            assert_eq!(session.select(index).commit.as_deref(), Some("コーヒー"));
            assert_eq!(session.snapshot().scheme, scheme);
            assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
            assert!(!session.snapshot().spelling_symbols.contains('-'));
        }
    }
}

#[test]
fn temporary_japanese_long_vowel_can_be_inserted_and_erased_inside_romaji() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| options.scheme = scheme);
        session.character(b'R', true);
        type_text(&mut session, "kohi");
        session.set_caret(Some(3));
        let result = session.character(b'-', false);
        assert!(result.handled);
        assert!(result.commit.is_none());
        assert_eq!(session.snapshot().editing_text, "Rko-hi");
        assert_eq!(session.snapshot().caret_position, 4);
        assert!(words(&session).contains(&"コーヒ".to_owned()));
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "Rkohi");
        assert_eq!(session.snapshot().caret_position, 3);
        assert_eq!(
            session.snapshot().local_mode,
            LocalInputMode::TemporaryJapanese
        );
        assert_eq!(session.snapshot().scheme, scheme);
    }
}

#[test]
fn temporary_japanese_returns_to_the_original_scheme() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_preedit_uses_raw = false;
    });
    assert!(session.character(b'R', true).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "R");
    assert_eq!(snapshot.scheme, SchemeType::Shuangpin);
    assert_eq!(snapshot.candidates[0].source, CandidateSource::Fallback);
    type_text(&mut session, "ka");
    assert!(words(&session).contains(&"か".to_owned()));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "Rka");
    assert_eq!(
        snapshot.editing_text.capacity(),
        snapshot.editing_text.len()
    );
    assert!(session.input.local_preedit.capacity() >= session.input.local_preedit.len());
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().preedit, "Rk");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("Rk")
    );
    assert_eq!(session.snapshot().scheme, SchemeType::Shuangpin);
    assert_eq!(
        session.input.engine.current_scheme_type(),
        SchemeType::Shuangpin
    );

    session.character(b'R', true);
    assert_eq!(session.finish(0).commit.as_deref(), Some("R"));
    assert_eq!(
        session.input.engine.current_scheme_type(),
        SchemeType::Shuangpin
    );
}

#[test]
fn shuangpin_snapshot_does_not_build_unused_raw_preedit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_preedit_uses_raw = false;
    });
    type_text(&mut session, "nihc");

    let (snapshot, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.snapshot());

    assert_eq!(snapshot.preedit, "ni'hao");
    // 邮箱入口 `@` 及追加的字面符号各占一次分配。快照克隆候选行时 `SharedString` 字段只加引用计数，新建行的每个非空字段多一个 `Arc`。
    assert_eq!(
        allocations, 32,
        "shuangpin snapshot allocations: {allocations}"
    );
}

#[test]
fn japanese_engine_refresh_reuses_candidate_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::JapaneseRomaji);
    type_text(&mut session, "ka");
    let before = session.input.engine.candidates().to_vec();
    session.input.engine.handle_key(SchemeKey::Requery);
    // 会话的混排候选经 `clone_candidate_rows` 与引擎候选共用 `SharedString` 存储。重查时日文 provider 对这些共享字段 `clear()` 只是放手，随后的 `push_str` 新建字符串和 `Arc`，原缓冲区不会原地复用，指针必然变化（混排那份也不再另行复制）；这里只能确认重查后内容不变。
    assert_eq!(session.input.engine.candidates(), before);
}

#[test]
fn japanese_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::JapaneseRomaji);
    type_text(&mut session, "ka");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn japanese_completeness_check_does_not_materialize_conversion_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (raw, expected) in [
        ("nihongo", true),
        ("nihong", false),
        ("k", false),
        ("", false),
    ] {
        let mut session =
            fixture.session_with(|options| options.scheme = SchemeType::JapaneseRomaji);
        type_text(&mut session, raw);
        assert_eq!(session.input.is_all_complete_pure_pinyin(), expected);
        let (complete, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            session.input.is_all_complete_pure_pinyin()
        });
        assert_eq!(complete, expected);
        eprintln!("日文完整性 {raw} 检查分配：{allocations}");
        assert_eq!(allocations, 0, "完整性检查只需扫描，不应物化转换结果");
    }
}

#[test]
fn temporary_japanese_refresh_reuses_candidate_buffer() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Shuangpin);
    assert!(session.character(b'R', true).handled);
    type_text(&mut session, "ka");

    let capacity = session.input.engine.candidates().len().saturating_add(1);
    session.input.local_candidates = Vec::with_capacity(capacity);
    let pointer = session.input.local_candidates.as_ptr();
    session.input.refresh_temporary_japanese();

    assert_eq!(session.input.local_candidates.as_ptr(), pointer);
    assert!(session.input.local_candidates.capacity() >= capacity);
}

#[test]
fn temporary_japanese_refresh_reuses_candidate_row_storage() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Shuangpin);
    assert!(session.character(b'R', true).handled);
    type_text(&mut session, "ka");
    session.input.refresh_temporary_japanese();

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.refresh_temporary_japanese();
    });

    assert_eq!(allocations, 0);
}

#[test]
fn an_unmatched_date_keyword_commits_as_typed() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    TestClock::install(&mut session);
    assert!(session.character(b'T', true).handled);
    type_text(&mut session, "xin");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "Txin");
    assert_eq!(snapshot.candidates.len(), 1);
    assert_eq!(snapshot.candidates[0].pinyin, "Txin");
    assert_eq!(snapshot.candidates[0].source, CandidateSource::Fallback);
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("Txin")
    );
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    session.character(b'T', true);
    type_text(&mut session, "rq");
    let dates = words(&session);
    assert_eq!(dates.first().map(String::as_str), Some("2026年8月9日"));
}

/// test_input_session.cpp:1388-1401: the whole date list on the pinned clock, lunar date last, and a selected row commits and leaves the mode.
#[test]
fn a_date_row_commits_and_leaves_date_time_mode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    TestClock::install(&mut session);
    assert!(session.character(b'T', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::DateTime);
    type_text(&mut session, "rq");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "Trq");
    assert_eq!(snapshot.candidates.len(), 20);
    assert_eq!(snapshot.candidates[0].word, "2026年8月9日");
    assert_eq!(snapshot.candidates[16].word, "丙午年六月二十七日");
    assert!(snapshot
        .candidates
        .iter()
        .all(|item| item.source == CandidateSource::Generated));
    let result = session.select(0);
    assert_eq!(result.commit.as_deref(), Some("2026年8月9日"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert!(snapshot.preedit.is_empty());

    // 农历关键词：首行是农历日期，后面是带星期的写法。
    session.character(b'T', true);
    type_text(&mut session, "nongli");
    assert_eq!(
        words(&session),
        [
            "丙午年六月二十七日",
            "丙午年六月二十七日 星期日",
            "丙午年六月二十七日 周日"
        ]
    );
    assert_eq!(
        session.select(1).commit.as_deref(),
        Some("丙午年六月二十七日 星期日")
    );
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // The pinned clock reads 14:30:00, as the reference fixture's did.
    session.character(b'T', true);
    type_text(&mut session, "sj");
    assert!(
        words(&session).contains(&"2026-08-09 14:30:00".to_owned()),
        "{:?}",
        words(&session)
    );
}

#[test]
fn date_time_mode_restores_keyword_letters_at_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (keyword, index) in [
        ("rq", 0),
        ("riqi", 1),
        ("date", 2),
        ("sj", 0),
        ("shijian", 3),
        ("time", 1),
        ("xq", 0),
        ("xingqi", 2),
        ("week", 1),
        ("nl", 0),
        ("nongli", 2),
        ("yinli", 3),
    ] {
        let mut session = fixture.session();
        TestClock::install(&mut session);
        assert!(session.character(b'T', true).handled);
        type_text(&mut session, keyword);
        let expected = words(&session);
        assert!(!expected.is_empty(), "{keyword}");
        session.command(Command::MoveHome);
        for _ in 0..index {
            session.command(Command::MoveRight);
        }
        assert_eq!(session.snapshot().caret_position, index + 1);
        assert!(session.command(Command::DeleteForward).handled);
        let mut missing = keyword.to_owned();
        let letter = missing.remove(index);
        assert_eq!(session.snapshot().preedit, format!("T{missing}"));

        let result = session.character(letter as u8, false);

        assert!(result.handled, "{keyword}: {result:?}");
        assert!(result.commit.is_none(), "{keyword}: {result:?}");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.local_mode, LocalInputMode::DateTime);
        assert_eq!(snapshot.preedit, format!("T{keyword}"));
        assert_eq!(snapshot.caret_position, index + 2);
        assert_eq!(words(&session), expected);
        assert!(snapshot
            .candidates
            .iter()
            .all(|item| item.source == CandidateSource::Generated));
        assert_eq!(
            session.select(0).commit.as_deref(),
            Some(expected[0].as_str())
        );
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
        assert!(session.snapshot().preedit.is_empty());
    }
}

#[test]
fn date_time_mode_still_rejects_non_lowercase_insertions() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for value in *b"A'1;" {
        let mut session = fixture.session();
        TestClock::install(&mut session);
        session.character(b'T', true);
        type_text(&mut session, "rq");
        session.command(Command::MoveLeft);
        let before = session.snapshot();

        let result = session.character(value, false);

        assert!(!result.handled, "{result:?}");
        assert_eq!(result.commit.as_deref(), Some(before.preedit.as_str()));
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
        assert!(session.snapshot().editing_text.is_empty());
    }
}

/// 组字里的日期时间行（#5952）用的词：每个关键词的锚定词，加上同一串字母、数字的其他读法。
const INLINE_DATE_TIME_FIXTURE: &str = "CREATE TABLE tbl_1_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_r VALUES('ri','r','日',100);\
CREATE TABLE tbl_2_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_r VALUES('ri''qi','rq','日期',500);\
CREATE TABLE tbl_2_p(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_p VALUES('pi''qi','pq','脾气',900);\
CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_s VALUES('shou''ji','sj','手机',900),('shi''jian','sj','时间',800),('shi''jie','sj','世界',700);\
CREATE TABLE tbl_2_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_x VALUES('xing''qi','xq','星期',500);\
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('nong''li','nl','农历',500);";

/// 26 键打 `riqi`、`ri'qi`、`sj`、`xingqi`、`nongli`：当前日期、时间、星期、农历接在对应的词后面，词前后的其他候选不动。
#[test]
fn date_time_keywords_add_rows_after_their_word() {
    let fixture = Fixture::new(INLINE_DATE_TIME_FIXTURE);
    let mut session = fixture.session();
    TestClock::install(&mut session);
    let date = [
        "日期",
        "2026年8月9日",
        "📅日期",
        "2026-08-09",
        "2026年8月9日 星期日",
    ];
    for keyword in ["riqi", "ri'qi"] {
        type_text(&mut session, keyword);
        assert_eq!(words(&session)[..5], date, "{keyword}");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.candidates[1].source, CandidateSource::Generated);
        assert_eq!(snapshot.candidates[1].pinyin, keyword);
        assert!(snapshot.candidate_answers_key[1]);
        session.command(Command::Cancel);
    }

    type_text(&mut session, "sj");
    let sj = words(&session);
    let at = sj.iter().position(|word| word == "时间").expect("时间");
    assert_eq!(sj[..at], ["手机"]);
    assert_eq!(
        sj[at + 1..at + 5],
        ["14:30", "🕒时间", "14:30:00", "下午2:30"]
    );
    session.command(Command::Cancel);

    type_text(&mut session, "xingqi");
    assert_eq!(words(&session)[..4], ["星期", "星期日", "📅星期", "周日"]);
    session.command(Command::Cancel);

    type_text(&mut session, "nongli");
    assert_eq!(
        words(&session)[..3],
        ["农历", "丙午年六月二十七日", "📅日期"]
    );
    session.command(Command::Cancel);

    // 不是关键词的组字、关键词的一部分都不加。
    for typed in ["ri", "riq", "shou"] {
        type_text(&mut session, typed);
        assert!(
            words(&session).iter().all(|word| !word.contains("2026")
                && !word.contains("14:30")
                && !word.starts_with("星期日")),
            "{typed}: {:?}",
            words(&session)
        );
        session.command(Command::Cancel);
    }
}

/// `utility.date_time_candidates`：行内日期时间混入与 T 模式是两个开关。混入关着时关键词只出它本来的词，T 模式仍照常；T 模式关着而混入开着时日期行照出。
#[test]
fn inline_date_time_is_a_separate_switch_from_the_mode() {
    let fixture = Fixture::new(INLINE_DATE_TIME_FIXTURE);
    let mut session = fixture.session_with(|options| options.local_modes.date_time_inline = false);
    TestClock::install(&mut session);
    type_text(&mut session, "riqi");
    assert!(
        !words(&session)
            .iter()
            .any(|word| word.contains("2026") || word.contains("14:30") || word.starts_with("📅")),
        "{:?}",
        words(&session)
    );
    session.command(Command::Cancel);

    let mut session = fixture.session_with(|options| {
        options.local_modes.date_time = false;
        options.local_modes.date_time_inline = true;
    });
    TestClock::install(&mut session);
    type_text(&mut session, "riqi");
    assert_eq!(
        words(&session)[..5],
        [
            "日期",
            "2026年8月9日",
            "📅日期",
            "2026-08-09",
            "2026年8月9日 星期日"
        ]
    );
    session.command(Command::Cancel);
}

/// 选中日期行上屏它的文字并结束组字；它不是拼音读出的词，打开学习时也不写进词库和日志。
#[test]
fn a_date_time_row_commits_without_learning() {
    let fixture = Fixture::new(INLINE_DATE_TIME_FIXTURE);
    let mut session = fixture.session_with(|options| options.learning = true);
    TestClock::install(&mut session);
    type_text(&mut session, "riqi");
    let result = select_word(&mut session, "2026-08-09");
    assert_eq!(result.commit.as_deref(), Some("2026-08-09"));
    assert!(session.snapshot().preedit.is_empty());
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations"
        ),
        0
    );
    assert_eq!(count(&fixture.main_db(), "SELECT count(*) FROM tbl_2_r"), 1);
}

/// `utility.date_time_menu`：「📅日期」入口跟在第一条行内日期行后面（锚定词在首位时是第三位），选中它不上屏、候选窗换成这一组的全部格式；再选一条格式上屏并结束组字；其他键先换回原来的候选再照常处理。
#[test]
fn date_time_menu_opens_the_full_format_list() {
    let fixture = Fixture::new(INLINE_DATE_TIME_FIXTURE);
    let mut session = fixture.session();
    TestClock::install(&mut session);
    type_text(&mut session, "riqi");
    let first = words(&session);
    assert_eq!(first[2], "📅日期", "{first:?}");
    let menu = &session.snapshot().candidates[2];
    assert_eq!(menu.source, CandidateSource::Generated);
    assert_eq!(menu.pinyin, "menu:riqi");

    // 选中入口：不上屏，候选窗换成 T 模式的整组格式，组字原样留着。
    let result = session.select(2);
    assert!(result.handled);
    assert!(result.commit.is_none());
    let page = words(&session);
    assert_eq!(page.len(), 20, "{page:?}");
    assert_eq!(page[0], "2026年8月9日");
    assert_eq!(page[16], "丙午年六月二十七日");
    assert!(!session.snapshot().preedit.is_empty());

    // 页里选一条格式：照常上屏并结束组字。
    assert_eq!(session.select(1).commit.as_deref(), Some("2026-08-09"));
    assert!(session.snapshot().preedit.is_empty());

    // 非选词键先换回原来的候选再照常处理：退格删掉最后一个字母，候选跟着变。
    type_text(&mut session, "riqi");
    session.select(2);
    assert_eq!(words(&session).len(), 20);
    assert!(session.command(Command::Backspace).handled);
    let after = words(&session);
    assert!(
        !after.contains(&"丙午年六月二十七日".to_owned()),
        "{after:?}"
    );
    assert!(!after.contains(&"📅日期".to_owned()), "{after:?}");
    session.command(Command::Cancel);

    // 星期、时间的入口文案与参考一致；锚定词不在首位时入口仍在第一条行内行后面。
    type_text(&mut session, "sj");
    let sj = words(&session);
    let at = sj.iter().position(|word| word == "时间").expect("时间");
    assert_eq!(sj[at + 2], "🕒时间", "{sj:?}");
    session.command(Command::Cancel);
    type_text(&mut session, "xingqi");
    assert_eq!(words(&session)[2], "📅星期");
    session.command(Command::Cancel);
    type_text(&mut session, "nongli");
    assert_eq!(words(&session)[2], "📅日期");
    session.command(Command::Cancel);

    // 关掉开关时只有行内日期行，没有入口。
    let mut session = fixture.session_with(|options| options.local_modes.date_time_menu = false);
    TestClock::install(&mut session);
    type_text(&mut session, "riqi");
    assert_eq!(
        words(&session)[..4],
        ["日期", "2026年8月9日", "2026-08-09", "2026年8月9日 星期日"]
    );
}

/// 词库里没有锚定词时什么都不加；关掉行内日期时间混入（`date_time_inline`）时 26 键、九宫格都不加。
#[test]
fn date_time_rows_need_their_word_and_the_date_time_switch() {
    let fixture = Fixture::new(&INLINE_DATE_TIME_FIXTURE.replace(
        "INSERT INTO tbl_2_x VALUES('xing''qi','xq','星期',500);",
        "",
    ));
    let mut session = fixture.session();
    TestClock::install(&mut session);
    type_text(&mut session, "xingqi");
    assert!(!words(&session).contains(&"星期日".to_owned()));
    session.command(Command::Cancel);

    let mut session = fixture.session_with(|options| options.local_modes.date_time_inline = false);
    TestClock::install(&mut session);
    type_text(&mut session, "riqi");
    assert_eq!(words(&session)[..1], ["日期"]);
    assert!(!words(&session).contains(&"2026年8月9日".to_owned()));
    session.command(Command::Cancel);
    session.set_nine_key_enabled(true);
    type_text(&mut session, "7474");
    assert!(words(&session).contains(&"日期".to_owned()));
    assert!(!words(&session).contains(&"2026年8月9日".to_owned()));
}

/// 九宫格打出关键词的数字（`7474` 是 riqi，`75` 是 sj 的简拼）时同样接在词后面，选中后吃掉全部数字。
#[test]
fn nine_key_digits_of_a_keyword_add_date_time_rows() {
    let fixture = Fixture::new(INLINE_DATE_TIME_FIXTURE);
    let mut session = fixture.session();
    TestClock::install(&mut session);
    session.set_nine_key_enabled(true);

    type_text(&mut session, "7474");
    let rows = words(&session);
    let at = rows.iter().position(|word| word == "日期").expect("日期");
    assert_eq!(
        rows[at + 1..at + 4],
        ["2026年8月9日", "2026-08-09", "2026年8月9日 星期日"]
    );
    assert!(rows.contains(&"脾气".to_owned()));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.candidates[at + 1].pinyin, "7474");
    assert_eq!(
        snapshot.candidates[at + 1].source,
        CandidateSource::Generated
    );
    let result = session.select(at + 1);
    assert_eq!(result.commit.as_deref(), Some("2026年8月9日"));
    assert!(session.snapshot().candidates.is_empty());

    type_text(&mut session, "75");
    let rows = words(&session);
    let at = rows.iter().position(|word| word == "时间").expect("时间");
    assert_eq!(rows[at + 1], "14:30");
    session.command(Command::Cancel);

    // `74` 是 ri 也是 pi，不是关键词。
    type_text(&mut session, "74");
    assert!(!words(&session).iter().any(|word| word.contains("2026")));
}

#[test]
fn super_jianpin_bare_entry_keeps_capital_words_literal() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        for text in ["JI", "JIT", "JNH", "JPEG", "JSON"] {
            let mut session = fixture.session_with(|options| options.scheme = scheme);
            for byte in text.bytes() {
                let result = session.character(byte, true);
                assert!(result.handled && result.commit.is_none());
            }
            let snapshot = session.snapshot();
            assert_eq!(
                snapshot.local_mode,
                LocalInputMode::None,
                "{scheme:?} {text}"
            );
            assert_eq!(snapshot.editing_text, text);
            assert_eq!(words(&session), [text]);
            assert_eq!(session.select(0).commit.as_deref(), Some(text));
        }
    }
}

#[test]
fn super_jianpin_lowercase_entry_and_unicode_hex_stay_available() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| options.scheme = scheme);
        session.character(b'J', true);
        type_text(&mut session, "nh");
        assert_eq!(session.snapshot().local_mode, LocalInputMode::SuperJianpin);
        assert!(words(&session).contains(&"你好".to_owned()));
        let at = words(&session)
            .iter()
            .position(|word| word == "你好")
            .unwrap();
        assert_eq!(session.select(at).commit.as_deref(), Some("你好"));

        session.character(b'U', true);
        session.character(b'F', true);
        assert_eq!(session.snapshot().local_mode, LocalInputMode::Unicode);
        type_text(&mut session, "F21");
        assert_eq!(session.snapshot().local_mode, LocalInputMode::Unicode);
        assert!(words(&session).contains(&"Ａ".to_owned()));
    }
}

#[test]
fn rejected_capital_after_bare_shortcut_keeps_both_letters_composing() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        for (entry, continuation) in [
            (b'U', b'V'),
            (b'U', b'I'),
            (b'T', b'A'),
            (b'K', b'A'),
            (b'V', b'Z'),
        ] {
            let mut session = fixture.session_with(|options| {
                options.scheme = scheme;
                options.local_modes.expression = true;
            });
            assert!(session.character(entry, true).handled);
            assert_ne!(session.snapshot().local_mode, LocalInputMode::None);
            let result = session.character(continuation, true);
            assert!(
                result.handled,
                "{scheme:?}: {}{}",
                entry as char, continuation as char
            );
            assert!(result.commit.is_none());
            let expected = String::from_utf8(vec![entry, continuation]).unwrap();
            let snapshot = session.snapshot();
            assert_eq!(snapshot.local_mode, LocalInputMode::None);
            assert_eq!(snapshot.preedit, expected);
            assert_eq!(
                session.command(Command::CommitRaw).commit.as_deref(),
                Some(expected.as_str())
            );
        }
    }
}

/// #10123：大写字母入口后只打了字母，模式不收的字母（小写或大写）把整串转回普通组字，驼峰英文不会在中途上屏。
#[test]
fn rejected_letter_after_letter_only_shortcut_keeps_the_whole_word_composing() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        for text in [
            "TextBlock",
            "TxB",
            "TbU",
            "UbU",
            "TuU",
            "Uu",
            "Ui",
            "Uq",
            "Uw",
            "Uo",
            "Ur",
            "Uy",
            "Ut",
            "Up",
            "KabA",
            "Trq",
        ] {
            let mut session = fixture.session_with(|options| options.scheme = scheme);
            for byte in text.bytes() {
                let result = session.character(byte, byte.is_ascii_uppercase());
                assert!(
                    result.handled && result.commit.is_none(),
                    "{scheme:?} {text}: {} gave {result:?}",
                    byte as char
                );
            }
            let snapshot = session.snapshot();
            // `Trq` 全是日期时间模式收的小写，留在模式里；其余都在第一个被拒字母处转回普通组字。
            let expected_mode = if text == "Trq" {
                LocalInputMode::DateTime
            } else {
                LocalInputMode::None
            };
            assert_eq!(snapshot.local_mode, expected_mode, "{scheme:?} {text}");
            assert_eq!(snapshot.editing_text, text, "{scheme:?} {text}");
            assert_eq!(
                session.command(Command::CommitRaw).commit.as_deref(),
                Some(text),
                "{scheme:?} {text}"
            );
        }

        // 转回后退格只删最后一个字母，仍是普通组字，不回到模式。
        let mut session = fixture.session_with(|options| options.scheme = scheme);
        assert!(session.character(b'U', true).handled);
        assert!(session.character(b'i', false).handled);
        assert!(session.command(Command::Backspace).handled);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.local_mode, LocalInputMode::None);
        assert_eq!(snapshot.editing_text, "U");

        // 十六进制字母仍是 Unicode 码位。
        let mut session = fixture.session_with(|options| options.scheme = scheme);
        assert!(session.character(b'U', true).handled);
        for byte in *b"ae" {
            assert!(session.character(byte, false).handled);
        }
        assert_eq!(session.snapshot().local_mode, LocalInputMode::Unicode);
        assert_eq!(session.snapshot().editing_text, "Uae");
    }
}

/// 五笔不走拼音的大写字面组字，入口后的拒绝字母沿用提交并透传。
#[test]
fn wubi_rejected_letter_after_shortcut_still_commits_and_returns_the_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Wubi);
    assert!(session.character(b'U', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Unicode);
    let result = session.character(b'i', false);
    assert!(!result.handled);
    assert_eq!(result.commit.as_deref(), Some("U"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert!(session.snapshot().editing_text.is_empty());
}

#[test]
fn rejected_local_mode_characters_commit_raw_and_return_the_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (entry, spelling, rejected) in [
        (b'U', "0041", b'I'),
        (b'U', "0041", b'g'),
        (b'T', "rq", b'1'),
        (b'K', "ab", b'1'),
        (b'E', "xiao", b'0'),
        (b'M', "xiao", b'0'),
        (b'J', "nh", b'0'),
        (b'C', "huo", b'1'),
        (b'V', "1", b'Z'),
        (b'/', "uuid", b'A'),
        (b'@', "user", b'A'),
    ] {
        let mut session = fixture.session_with(|options| {
            options.local_modes.chaizi = true;
            options.local_modes.expression = true;
            options.local_modes.command = true;
            options.local_modes.mention = true;
        });
        assert!(session.character(entry, entry.is_ascii_uppercase()).handled);
        type_text(&mut session, spelling);
        let raw = session.input.preedit();
        assert_eq!(raw, format!("{}{spelling}", entry as char));
        let result = session.character(rejected, rejected.is_ascii_uppercase());
        assert!(
            !result.handled,
            "{} rejected {}",
            entry as char, rejected as char
        );
        assert_eq!(result.commit.as_deref(), Some(raw.as_str()));
        let snapshot = session.snapshot();
        assert_eq!(snapshot.local_mode, LocalInputMode::None);
        assert!(snapshot.preedit.is_empty());
        assert!(snapshot.candidates.is_empty());
    }
}

#[test]
fn caret_rejected_local_characters_commit_raw_and_return_the_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (entry, spelling, rejected) in [
        (b'U', "0041", b'g'),
        (b'T', "rq", b'A'),
        (b'K', "ab", b'A'),
        (b'E', "xiao", b'1'),
        (b'M', "xiao", b'1'),
        (b'J', "nh", b'1'),
        (b'C', "huo", b'1'),
        (b'V', "1", b'Z'),
        (b'/', "uuid", b'A'),
        (b'@', "user", b'A'),
    ] {
        let mut session = fixture.session_with(|options| {
            options.local_modes.chaizi = true;
            options.local_modes.expression = true;
            options.local_modes.command = true;
            options.local_modes.mention = true;
        });
        assert!(session.character(entry, entry.is_ascii_uppercase()).handled);
        type_text(&mut session, spelling);
        session.command(Command::MoveHome);
        let raw = session.snapshot().editing_text;
        assert_eq!(raw, format!("{}{spelling}", entry as char));

        let result = session.character(rejected, rejected.is_ascii_uppercase());
        assert!(
            !result.handled,
            "{} rejected {}",
            entry as char, rejected as char
        );
        assert_eq!(result.commit.as_deref(), Some(raw.as_str()));
        let snapshot = session.snapshot();
        assert_eq!(snapshot.local_mode, LocalInputMode::None);
        assert!(snapshot.preedit.is_empty());
        assert!(snapshot.candidates.is_empty());
    }
}

#[test]
fn direct_local_caret_keeps_the_entry_marker_when_deleting() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (entry, spelling) in [
        (b'U', "0041"),
        (b'T', "rq"),
        (b'K', "ab"),
        (b'E', "xiao"),
        (b'M', "xiao"),
        (b'J', "nh"),
        (b'C', "huo"),
        (b'V', "12"),
        (b'/', "uuid"),
        (b'@', "user"),
        (b'Y', "ab"),
        (b'R', "ka"),
    ] {
        let mut session = fixture.session_with(|options| {
            options.local_modes.chaizi = true;
            options.local_modes.expression = true;
            options.local_modes.command = true;
            options.local_modes.mention = true;
        });
        assert!(session.character(entry, entry.is_ascii_uppercase()).handled);
        type_text(&mut session, spelling);
        let mode = session.snapshot().local_mode;
        assert_ne!(mode, LocalInputMode::None);
        session.set_caret(Some(0));

        let result = session.command(Command::DeleteForward);

        assert!(
            result.handled && result.commit.is_none(),
            "{entry}: {result:?}"
        );
        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.editing_text,
            format!("{}{}", entry as char, &spelling[1..])
        );
        assert_eq!(snapshot.local_mode, mode);
        assert_eq!(snapshot.caret_position, 1);
        let before = snapshot;
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot(), before);
    }
}

#[test]
fn direct_local_caret_inserts_after_the_entry_marker() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (entry, spelling, value) in [(b'U', "0041", b'+'), (b'Y', "ab", b'x'), (b'R', "ka", b'a')] {
        let mut session = fixture.session();
        assert!(session.character(entry, true).handled);
        type_text(&mut session, spelling);
        let mode = session.snapshot().local_mode;
        session.set_caret(Some(0));

        let result = session.character(value, false);

        assert!(
            result.handled && result.commit.is_none(),
            "{entry}: {result:?}"
        );
        let snapshot = session.snapshot();
        assert_eq!(
            snapshot.editing_text,
            format!("{}{}{spelling}", entry as char, value as char)
        );
        assert_eq!(snapshot.local_mode, mode);
        assert_eq!(snapshot.caret_position, 2);
    }
}

#[test]
fn direct_caret_keeps_zero_for_modes_without_an_entry_marker() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (typed, dedicated, expected) in [
        ("ni", false, "i"),
        ("ab", true, "b"),
        ("www.example", false, "ww.example"),
    ] {
        let mut session = fixture.session();
        session.set_dedicated_english(dedicated);
        type_text(&mut session, typed);
        let mode = session.snapshot().local_mode;
        session.set_caret(Some(0));
        assert_eq!(session.snapshot().caret_position, 0);

        let result = session.command(Command::DeleteForward);

        assert!(result.handled && result.commit.is_none());
        let snapshot = session.snapshot();
        assert_eq!(snapshot.editing_text, expected);
        assert_eq!(snapshot.caret_position, 0);
        assert_eq!(snapshot.local_mode, mode);
    }
}

#[test]
fn direct_local_caret_keeps_end_clamping_and_bare_marker_exit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    session.character(b'U', true);
    session.set_caret(Some(0));
    assert_eq!(session.snapshot().caret_position, 1);
    assert!(session.command(Command::DeleteForward).handled);
    assert_eq!(session.snapshot().editing_text, "U");
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert!(session.snapshot().editing_text.is_empty());

    session.character(b'U', true);
    type_text(&mut session, "0041");
    session.set_caret(Some(usize::MAX));
    assert_eq!(session.snapshot().caret_position, 5);
    session.set_caret(Some(0));
    session.set_caret(None);
    assert_eq!(session.snapshot().caret_position, 5);
    let result = session.character(b'2', false);
    assert!(result.handled && result.commit.is_none());
    assert_eq!(session.snapshot().editing_text, "U00412");
}

#[test]
fn accepted_local_mode_characters_keep_spelling_until_enter() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (entry, spelling) in [(b'U', "+4e2d"), (b'T', "rq"), (b'K', "ab")] {
        let mut session = fixture.session();
        assert!(session.character(entry, true).handled);
        for value in spelling.bytes() {
            let result = session.character(value, value.is_ascii_uppercase());
            assert!(result.handled);
            assert!(result.commit.is_none());
        }
        let raw = format!("{}{spelling}", entry as char);
        assert_eq!(session.snapshot().preedit, raw);
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some(raw.as_str())
        );
    }
}

#[test]
fn a_full_local_mode_commits_its_text_instead_of_swallowing_the_next_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.local_modes.chaizi = true);
    session.character(b'C', true);
    let spelling = "a".repeat(crate::local::GENERATED_MODE_INPUT_LIMIT - 1);
    type_text(&mut session, &spelling);
    let result = session.character(b'b', false);
    assert!(!result.handled);
    assert_eq!(result.commit, Some(format!("C{spelling}")));
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn temporary_local_modes_keep_their_composition_when_returning_a_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (entry, caret) in [(b'Y', None), (b'R', None), (b'Y', Some(1)), (b'R', Some(1))] {
        let mut session = fixture.session();
        assert!(session.character(entry, true).handled);
        type_text(&mut session, "ka");
        session.set_caret(caret);
        let before = session.snapshot();
        let result = session.character(b'0', false);
        assert!(!result.handled);
        assert!(result.commit.is_none());
        assert_eq!(session.input.preedit(), before.preedit);
        assert_eq!(session.snapshot().local_mode, before.local_mode);
        assert_eq!(session.snapshot().caret_position, before.caret_position);
    }
}

#[test]
fn disabled_local_modes_leave_the_key_to_pinyin() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.local_modes.temporary_english = false;
        options.local_modes.unicode = false;
    });
    for &letter in b"YU" {
        assert!(session.character(letter, true).handled);
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
        assert_eq!(session.snapshot().preedit, (letter as char).to_string());
        session.command(Command::Cancel);
    }
    assert!(session.character(b'K', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::QuickPhrase);
}

#[test]
fn shift_letter_entry_switch_off_types_every_capital() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.local_modes.shift_letter_entry = false;
        options.local_modes.expression = true;
        options.local_modes.command = true;
        options.local_modes.chaizi = true;
    });
    // 总开关关掉：每个入口字母都交还宿主打出大写，不进模式，也不留组字。
    for letter in *b"UTKEMJYRVC" {
        let result = session.character(letter, true);
        assert!(!result.handled, "{}", letter as char);
        assert_eq!(session.input.local_mode, LocalInputMode::None);
        assert!(!session.input.has_composition());
    }
    // 各模式自己的开关没被改动；`/` 不是 Shift+字母，照常打开指令模式。
    assert!(session.input.local_mode_options.date_time);
    assert!(session.character(b'/', false).handled);
    assert_eq!(session.input.local_mode, LocalInputMode::Command);

    // 对照：总开关开着时同一个键进入日期时间模式。
    let mut session = fixture.session();
    assert!(session.character(b'T', true).handled);
    assert_eq!(session.input.local_mode, LocalInputMode::DateTime);
}

/// #9696：五笔编码只用小写字母，没有组字时的 Shift+字母不会和编码冲突。输入不经过拼音拼写的 U、T、Y、S 和 K、C 一样在五笔下进入；E、M、J 按拼音查，V 的数字和运算符要宿主按方案路由，这几个仍只在全拼、双拼下进入，大写字母交还宿主。
#[test]
fn wubi_opens_the_shift_letter_modes_that_spell_no_pinyin() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.local_modes.expression = true;
    });
    TestClock::install(&mut session);
    for (letter, mode) in [
        (b'U', LocalInputMode::Unicode),
        (b'T', LocalInputMode::DateTime),
        (b'Y', LocalInputMode::TemporaryEnglish),
        (b'S', LocalInputMode::Symbol),
    ] {
        assert!(
            session.character(letter, true).handled,
            "{}",
            letter as char
        );
        assert_eq!(session.snapshot().local_mode, mode, "{}", letter as char);
        assert!(session.command(Command::Cancel).handled);
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    }

    // 报告里的两个例子：Shift+Y 后打 superuser 上屏原文，Shift+T 后打 date 上屏日期，都不进五笔组字。
    assert!(session.character(b'Y', true).handled);
    type_text(&mut session, "superuser");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.scheme, SchemeType::Wubi);
    assert_eq!(snapshot.preedit, "Ysuperuser");
    assert_eq!(session.select(0).commit.as_deref(), Some("superuser"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    assert!(session.character(b'T', true).handled);
    type_text(&mut session, "date");
    assert_eq!(session.snapshot().preedit, "Tdate");
    assert!(
        words(&session).contains(&"2026-08-09".to_owned()),
        "{:?}",
        words(&session)
    );
    assert_eq!(session.select(0).commit.as_deref(), Some("2026年8月9日"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    for letter in *b"EMJV" {
        assert!(
            !session.character(letter, true).handled,
            "{}",
            letter as char
        );
        assert_eq!(session.input.local_mode, LocalInputMode::None);
        assert!(!session.input.has_composition());
    }
}

fn chaizi_session(fixture: &Fixture, configure: impl FnOnce(&mut SessionOptions)) -> Session {
    fixture.session_with(|options| {
        options.local_modes.chaizi = true;
        configure(options);
    })
}

fn chaizi_rows(session: &Session) -> Vec<(String, CandidateSource)> {
    session
        .snapshot()
        .candidates
        .into_iter()
        .map(|item| (item.word.into_string(), item.source))
        .collect()
}

#[test]
fn chaizi_opens_on_shift_c_in_pinyin_and_wubi_only() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    // 拆字默认关：Shift+C 留在普通拼音组字中，不进入拆字模式。
    let mut session = fixture.session();
    assert!(session.character(b'C', true).handled);
    assert_eq!(session.snapshot().preedit, "C");
    assert_eq!(session.input.local_mode, LocalInputMode::None);
    assert!(!crate::types::LocalModeOptions::default().chaizi);

    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin, SchemeType::Wubi] {
        let mut session = chaizi_session(&fixture, |options| options.scheme = scheme);
        assert!(session.character(b'C', true).handled, "{scheme:?}");
        assert_eq!(
            session.input.local_mode,
            LocalInputMode::Chaizi,
            "{scheme:?}"
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.preedit, "C");
        // 只有原样兜底行。
        assert_eq!(
            chaizi_rows(&session),
            [("C".to_owned(), CandidateSource::Fallback)]
        );
        // 只剩 `C` 时退格退出模式。
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.input.local_mode, LocalInputMode::None);
    }
    for scheme in [SchemeType::JapaneseRomaji, SchemeType::Korean] {
        let mut session = chaizi_session(&fixture, |options| options.scheme = scheme);
        session.character(b'C', true);
        assert_ne!(
            session.input.local_mode,
            LocalInputMode::Chaizi,
            "{scheme:?}"
        );
    }
    // 没有 Shift 的 `C`（大写锁定、宿主当字符送来）不进模式。
    let mut session = chaizi_session(&fixture, |_| {});
    session.character(b'C', false);
    assert_ne!(session.input.local_mode, LocalInputMode::Chaizi);
}

#[test]
fn chaizi_finds_a_character_by_its_components_and_annotates_its_reading() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = chaizi_session(&fixture, |options| {
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
    });
    assert!(session.character(b'C', true).handled);
    type_text(&mut session, "huohuohuo");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "Chuohuohuo");
    assert_eq!(snapshot.candidates[0].word, "焱");
    assert_eq!(snapshot.candidates[0].source, CandidateSource::Generated);
    assert_eq!(snapshot.candidate_annotations[0], "yàn");
    // 选中后上屏这个字、退出模式，不写任何词库。
    let result = session.select(0);
    assert_eq!(result.commit.as_deref(), Some("焱"));
    assert_eq!(session.input.local_mode, LocalInputMode::None);
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT COUNT(*) FROM user_dictionary_operations"
        ),
        0
    );

    // 回车上屏原文，入口字母一起。
    session.character(b'C', true);
    type_text(&mut session, "nvzi");
    assert_eq!(words(&session)[0], "好");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("Cnvzi")
    );
}

#[test]
fn chaizi_takes_an_apostrophe_only_between_components() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = chaizi_session(&fixture, |_| {});
    session.character(b'C', true);
    // 紧跟 `C` 的 `'` 不收，也不当分隔符。
    assert!(!session.input.takes_local_separator());
    let rejected = session.character(b'\'', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("C"));
    assert!(session.snapshot().preedit.is_empty());
    session.character(b'C', true);
    // 宿主当字符送来。
    type_text(&mut session, "huo'");
    // 连续的 `'` 不收，提交现有原文并退出；重新进入后继续验证合法分隔符。
    let rejected = session.character(b'\'', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("Chuo'"));
    assert!(session.snapshot().preedit.is_empty());
    session.character(b'C', true);
    type_text(&mut session, "huo'huo");
    // 宿主当标点送来：先问 `takes_local_separator`，同样当分隔符。
    assert!(session.input.takes_local_separator());
    assert!(session.punctuation(b'\'').handled);
    type_text(&mut session, "huo");
    assert_eq!(session.snapshot().preedit, "Chuo'huo'huo");
    assert_eq!(words(&session)[0], "焱");
    // `'` 切在部件中间时没有命中，只剩原样行。
    session.command(Command::Cancel);
    session.character(b'C', true);
    type_text(&mut session, "hu'ohuohuo");
    assert_eq!(
        chaizi_rows(&session),
        [("Chu'ohuohuo".to_owned(), CandidateSource::Fallback)]
    );
}

#[test]
fn chaizi_inserts_letters_and_separators_at_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = chaizi_session(&fixture, |_| {});
    session.character(b'C', true);
    type_text(&mut session, "huohuo");
    session.command(Command::MoveHome);
    assert_eq!(session.snapshot().caret_position, 1);
    // 入口字母之后不能插 `'`。
    let rejected = session.character(b'\'', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("Chuohuo"));
    assert!(session.snapshot().editing_text.is_empty());
    session.character(b'C', true);
    type_text(&mut session, "huohuo");
    session.command(Command::MoveHome);
    session.command(Command::MoveRight);
    session.command(Command::MoveRight);
    session.command(Command::MoveRight);
    assert!(session.character(b'\'', false).handled);
    assert_eq!(session.snapshot().editing_text, "Chuo'huo");
    // 紧挨着已有的 `'` 再插一个被吞掉。
    session.character(b'\'', false);
    assert_eq!(session.snapshot().editing_text, "Chuo'huo");
    session.command(Command::MoveEnd);
    type_text(&mut session, "huo");
    assert_eq!(session.snapshot().editing_text, "Chuo'huohuo");
    assert_eq!(words(&session)[0], "焱");
    // 插在中间的字母同样收下。
    session.command(Command::MoveHome);
    assert!(session.character(b'x', false).handled);
    assert_eq!(session.snapshot().editing_text, "Cxhuo'huohuo");
}

#[test]
fn chaizi_leaves_english_words_and_unmatched_input_as_typed() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = chaizi_session(&fixture, |_| {});
    // `CPU`：有大写字母就不查表；`Chen`、`Cpan`：只打了一个部件，不补全。
    for (rest, typed) in [
        ("PU", "CPU"),
        ("hen", "Chen"),
        ("pan", "Cpan"),
        ("xyz", "Cxyz"),
    ] {
        session.character(b'C', true);
        type_text(&mut session, rest);
        assert_eq!(
            chaizi_rows(&session),
            [(typed.to_owned(), CandidateSource::Fallback)],
            "{typed}"
        );
        // 空格选中原样行，上屏的就是键入的原文。
        assert_eq!(session.select(0).commit.as_deref(), Some(typed));
        assert_eq!(session.input.local_mode, LocalInputMode::None);
    }
}

#[test]
fn chaizi_drops_rows_that_convert_to_the_same_traditional_form() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut plain = chaizi_session(&fixture, |_| {});
    plain.character(b'C', true);
    type_text(&mut plain, "yanyi");
    assert!(words(&plain).contains(&"議".to_owned()));
    let mut traditional =
        chaizi_session(&fixture, |options| options.chaizi_traditional_output = true);
    traditional.character(b'C', true);
    type_text(&mut traditional, "yanyi");
    let rows = words(&traditional);
    assert!(
        rows.contains(&"议".to_owned()) && !rows.contains(&"議".to_owned()),
        "{rows:?}"
    );
}

#[test]
fn chaizi_queries_the_quanpin_reading_of_shuangpin_input() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = chaizi_session(&fixture, |options| options.scheme = SchemeType::Shuangpin);
    session.character(b'C', true);
    // 小鹤双拼的 huo 是 `ho`。
    type_text(&mut session, "hohoho");
    assert_eq!(words(&session)[0], "焱");
}

#[test]
fn chaizi_takes_the_microsoft_shuangpin_ing_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let microsoft = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
    };
    let mut session = chaizi_session(&fixture, microsoft);
    session.character(b'C', true);
    type_text(&mut session, "sj");
    // 片段 `sj` 已是一个完整音节，`;` 补不了它：先提交原文，再交还分号。
    assert_eq!(session.snapshot().spelling_symbols, "'");
    let rejected = session.character(b';', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("Csj"));
    assert!(session.snapshot().editing_text.is_empty());
    session.character(b'C', true);
    type_text(&mut session, "sjq");
    // `q` 之后 `;` 是韵母 ing。组字原文 `Csjq` 是偶数长度，按原文数奇偶的宿主会把它当标点，所以要列进 `spelling_symbols`。
    assert_eq!(session.snapshot().spelling_symbols, "';");
    assert!(session.character(b';', false).handled);
    assert_eq!(session.snapshot().editing_text, "Csjq;");
    assert_eq!(words(&session)[0], "清");
    assert_eq!(session.snapshot().spelling_symbols, "'");
    // 宿主当标点送来也一样，不会先以首行结束组字。
    session.command(Command::Cancel);
    session.character(b'C', true);
    type_text(&mut session, "sjq");
    assert!(session.punctuation(b';').handled);
    assert_eq!(session.snapshot().editing_text, "Csjq;");
    assert_eq!(session.input.local_mode, LocalInputMode::Chaizi);
    assert_eq!(words(&session)[0], "清");
    // 分隔符之后重新数奇偶，光标处插入用同一条规则。
    session.command(Command::Cancel);
    session.character(b'C', true);
    type_text(&mut session, "sj'q");
    session.command(Command::MoveLeft);
    let rejected = session.character(b';', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("Csj'q"));
    assert!(session.snapshot().editing_text.is_empty());
    session.character(b'C', true);
    type_text(&mut session, "sj'q");
    session.command(Command::MoveEnd);
    assert!(session.character(b';', false).handled);
    assert_eq!(session.snapshot().editing_text, "Csj'q;");
    assert_eq!(words(&session)[0], "清");

    // 小鹤双拼和全拼用不到 `;`，拆字里它照旧是标点。
    for scheme in [SchemeType::Shuangpin, SchemeType::Quanpin] {
        let mut session = chaizi_session(&fixture, |options| options.scheme = scheme);
        session.character(b'C', true);
        type_text(&mut session, "sjq");
        assert!(!session.snapshot().spelling_symbols.contains(';'));
        assert!(!session.input.chaizi_takes_symbol(b';', 4));
    }
}

#[test]
fn chaizi_ignores_an_apostrophe_left_right_after_the_entry_letter() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = chaizi_session(&fixture, |_| {});
    session.character(b'C', true);
    type_text(&mut session, "huo'huohuo");
    // 删掉 `'` 之前的部件，`'` 就紧挨着入口字母。
    session.command(Command::MoveHome);
    for _ in 0..3 {
        session.command(Command::MoveRight);
    }
    for _ in 0..3 {
        session.command(Command::Backspace);
    }
    assert_eq!(session.snapshot().editing_text, "C'huohuo");
    assert!(
        words(&session).contains(&"炎".to_owned()),
        "{:?}",
        words(&session)
    );
}

#[test]
fn dedicated_english_offers_the_typed_word_when_unknown() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    session.set_dedicated_english(true);
    // Every non-letter is swallowed, even with nothing composed.
    assert!(session.character(b'1', false).handled);
    type_text(&mut session, "HE");
    assert_eq!(words(&session)[..2], ["HE".to_owned(), "Hello".to_owned()]);
    session.command(Command::Cancel);
    type_text(&mut session, "Codex");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.candidates.len(), 1);
    assert_eq!(snapshot.candidates[0].word, "Codex");
    assert_eq!(snapshot.candidates[0].source, CandidateSource::Generated);
    let result = session.command(Command::CommitRaw);
    assert_eq!(result.commit.as_deref(), Some("Codex"));
    assert!(result.diagnostic.is_none(), "{result:?}");
    assert!(session.snapshot().dedicated_english);
}

#[test]
fn english_online_requires_opt_in_and_answers_enter_the_visible_list() {
    for dedicated in [true, false] {
        let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
        let mut session = generated_modes_session(&fixture);
        if dedicated {
            session.set_dedicated_english(true);
        } else {
            session.character(b'Y', true);
        }
        type_text(&mut session, "accessability");
        assert!(session.online_query().is_none());
        let local = words(&session);
        session.set_english_online_candidates_enabled(true);
        let query = session.online_query().unwrap();
        assert_eq!(query.mode, crate::OnlineInputMode::English);
        assert_eq!(query.query_text, "accessability");
        assert_eq!(query.cache_key, "english:accessability");
        assert!(query.pinyin_segments.is_empty());
        assert!(query.ai_eligible && query.cloud_eligible);
        assert!(session.apply_online_candidates(
            &query,
            &[
                "accessibility".into(),
                "accessibility".into(),
                "accessible".into()
            ],
            CandidateSource::AiSuggestion,
        ));
        assert_eq!(&words(&session)[..local.len()], local);
        assert_eq!(
            &words(&session)[local.len()..],
            ["accessibility", "accessible"]
        );
        let chosen = session.select(local.len());
        assert_eq!(chosen.commit.as_deref(), Some("accessibility"));
        assert!(!session.apply_online_candidate(
            &query,
            "accessible",
            CandidateSource::AiSuggestion
        ));
        assert!(session.online_query().is_none());
    }
}

#[test]
fn english_online_edits_revocation_and_mode_changes_reject_old_answers() {
    for dedicated in [true, false] {
        let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
        let mut session = generated_modes_session(&fixture);
        session.set_english_online_candidates_enabled(true);
        if dedicated {
            session.set_dedicated_english(true);
        } else {
            session.character(b'Y', true);
        }
        type_text(&mut session, "accessability");
        let old = session.online_query().unwrap();
        assert!(session.apply_online_candidate(
            &old,
            "accessibility",
            CandidateSource::AiSuggestion
        ));
        session.character(b'x', false);
        assert!(!words(&session).contains(&"accessibility".to_owned()));
        session.command(Command::Backspace);
        assert!(!session.apply_online_candidate(
            &old,
            "accessibility",
            CandidateSource::AiSuggestion
        ));
        let current = session.online_query().unwrap();
        assert!(session.apply_online_candidate(
            &current,
            "accessibility",
            CandidateSource::AiSuggestion
        ));
        session.command(Command::MoveHome);
        session.command(Command::DeleteForward);
        assert!(!words(&session).contains(&"accessibility".to_owned()));
        assert!(!session.apply_online_candidate(
            &current,
            "accessibility",
            CandidateSource::AiSuggestion
        ));
        let current = session.online_query().unwrap();
        assert!(session.apply_online_candidate(
            &current,
            "accessible",
            CandidateSource::AiSuggestion
        ));
        session.set_english_online_candidates_enabled(false);
        assert!(session.online_query().is_none());
        assert!(!words(&session).contains(&"accessible".to_owned()));
        assert!(!session.apply_online_candidate(
            &current,
            "accessible",
            CandidateSource::AiSuggestion
        ));
        session.set_english_online_candidates_enabled(true);
        assert!(!session.apply_online_candidate(
            &current,
            "accessible",
            CandidateSource::AiSuggestion
        ));
        let current = session.online_query().unwrap();
        session.set_dedicated_english(!dedicated);
        assert!(!session.apply_online_candidate(
            &current,
            "accessible",
            CandidateSource::AiSuggestion
        ));
    }
}

#[test]
fn english_online_batches_replace_sources_and_validate_query_mode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    session.set_english_online_candidates_enabled(true);
    session.set_dedicated_english(true);
    type_text(&mut session, "HE");
    let query = session.online_query().unwrap();
    let mut wrong_mode = query.clone();
    wrong_mode.mode = crate::OnlineInputMode::Pinyin;
    assert!(!session.apply_online_candidate(&wrong_mode, "Helper", CandidateSource::AiSuggestion));
    assert!(session.apply_online_candidates(
        &query,
        &["Helper".into()],
        CandidateSource::AiSuggestion
    ));
    assert!(session.apply_online_candidate(&query, "Hero", CandidateSource::CloudSuggestion));
    assert!(!session.apply_online_candidates(&query, &[], CandidateSource::AiSuggestion));
    assert!(words(&session).contains(&"Helper".to_owned()));
    // 同源批次即使只重复本地词，也必须撤回旧在线行。
    assert!(session.apply_online_candidates(
        &query,
        &["Hello".into()],
        CandidateSource::AiSuggestion
    ));
    assert!(!words(&session).contains(&"Helper".to_owned()));
    assert_eq!(
        words(&session)
            .iter()
            .filter(|word| *word == "Hello")
            .count(),
        1
    );
    session.clear_online_candidates(CandidateSource::CloudSuggestion);
    assert!(!words(&session).contains(&"Hero".to_owned()));
    assert!(!session.apply_online_candidate(&query, "Hero", CandidateSource::CloudSuggestion));
}

#[test]
fn dedicated_english_refresh_reuses_candidate_row_storage() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut old_style = fixture.session();
    old_style.set_dedicated_english(true);
    type_text(&mut old_style, "HE");
    let (_, old_allocations) = crate::ime::personal_rerank::allocations::count(|| {
        let prefix =
            crate::ime::queries::lowercase_prefix(&old_style.input.dedicated_english_preedit);
        let mut candidates = old_style
            .input
            .queries
            .english_dictionary()
            .query_prefix(&prefix, crate::ime::queries::MODE_ENGLISH_LIMIT);
        old_style.input.apply_candidate_positions(&mut candidates);
        old_style.input.dedicated_english_candidates = candidates;
    });

    let mut session = fixture.session();
    session.set_dedicated_english(true);
    type_text(&mut session, "HE");
    assert!(session.input.dedicated_english_candidates.len() >= 2);
    let pointer = session.input.dedicated_english_candidates.as_ptr();
    let capacity = session.input.dedicated_english_candidates.capacity();
    let words = crate::ime::personal_rerank::allocations::unshared_rows(
        &session.input.dedicated_english_candidates,
    );

    let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.update_dedicated_english_candidates();
    });

    assert_eq!(session.input.dedicated_english_candidates, words);
    assert_eq!(session.input.dedicated_english_candidates.as_ptr(), pointer);
    assert_eq!(
        session.input.dedicated_english_candidates.capacity(),
        capacity
    );
    assert!(
        allocations < old_allocations,
        "专用英文热刷新分配 {allocations} 次，旧路径为 {old_allocations} 次"
    );
}

#[test]
fn dedicated_english_caret_rejects_nonletters_without_releasing_the_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    session.set_dedicated_english(true);
    type_text(&mut session, "HE");
    session.command(Command::MoveLeft);
    let before = session.snapshot();
    assert_eq!(before.caret_position, 1);

    for value in *b"0123456789 !\t" {
        let result = session.character(value, false);
        assert!(result.handled, "{value:?}: {result:?}");
        assert!(result.commit.is_none(), "{value:?}: {result:?}");
        assert_eq!(session.snapshot(), before, "{value:?}");
    }

    assert!(session.character(b'L', true).handled);
    assert_eq!(session.snapshot().preedit, "HLE");
    assert_eq!(session.snapshot().caret_position, 2);
}

#[test]
fn dedicated_english_writes_ascii_punctuation_unless_locked_to_chinese() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = fixture.session();
    session.set_dedicated_english(true);
    // 组字中：结束组字，接半角句号，不是「。」。
    type_text(&mut session, "HE");
    let period = session.punctuation(b'.');
    assert!(period.handled);
    let commit = period.commit.unwrap_or_default();
    assert!(commit.ends_with('.') && !commit.contains('。'), "{commit}");
    assert!(!session.input.has_composition());
    // 空闲：交还宿主，由宿主插入按键本身。
    let idle = session.punctuation(b'.');
    assert!(!idle.handled && idle.commit.is_none(), "{idle:?}");
    assert!(!session.punctuation(b',').handled);

    // 「始终使用中文标点」（lock 1）在英文模式下也给中文标点。
    session.set_punctuation_lock(1).unwrap();
    assert_eq!(session.punctuation(b'.').commit.as_deref(), Some("。"));
    // 「始终使用英文标点」（lock 2）同样是半角。
    session.set_punctuation_lock(2).unwrap();
    assert!(!session.punctuation(b'.').handled);

    // 中文模式不受影响：默认跟随中英文状态时仍是中文标点。
    session.set_punctuation_lock(0).unwrap();
    session.set_dedicated_english(false);
    assert_eq!(session.punctuation(b'.').commit.as_deref(), Some("。"));
}

// ---- construction ----

#[test]
fn invalid_options_are_refused() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut options = fixture.options();
    options.frequency.trigger_count = 0;
    assert_eq!(
        Session::new(options).err().map(|error| error.to_string()),
        Some(crate::diagnostics::INVALID_SESSION_OPTIONS.to_owned())
    );
    let mut options = fixture.options();
    options.english.minimum_prefix = 9;
    assert!(Session::new(options).is_err());
    // Both ends of each range (input_session.cpp:526-536).
    let refused = |configure: &dyn Fn(&mut SessionOptions)| {
        let mut options = fixture.options();
        configure(&mut options);
        Session::new(options).err().map(|error| error.to_string())
    };
    let invalid = Some(crate::diagnostics::INVALID_SESSION_OPTIONS.to_owned());
    assert_eq!(
        refused(&|options| options.frequency.linear_step = 0),
        invalid
    );
    assert_eq!(
        refused(&|options| options.frequency.linear_step = 11),
        invalid
    );
    assert_eq!(
        refused(&|options| options.frequency.trigger_count = 11),
        invalid
    );
    assert_eq!(
        refused(&|options| options.english.minimum_prefix = 0),
        invalid
    );
    assert_eq!(
        refused(&|options| {
            options.frequency.linear_step = 10;
            options.frequency.trigger_count = 10;
            options.english.minimum_prefix = 8;
        }),
        None
    );
    assert_eq!(refused(&|options| options.english.minimum_prefix = 1), None);
    let mut options = fixture.options();
    options.helpcode_schema = "nonsense".to_owned();
    assert!(Session::new(options).is_err());
    let mut options = fixture.options();
    options.helpcode_schema = "custom/missing".to_owned();
    assert_eq!(
        Session::new(options).err().map(|error| error.to_string()),
        Some(crate::diagnostics::UNKNOWN_HELPCODE_SCHEMA.to_owned())
    );
    let mut session = fixture.session();
    assert!(session.set_punctuation_lock(3).is_err());
    assert!(session.set_punctuation_lock(2).is_ok());
    assert!(!session.punctuation(b',').handled);
}

/// Types `keys` into a Korean session and returns what each key committed, asserting every letter is handled.
fn type_korean(session: &mut Session, keys: &str) -> Vec<Option<String>> {
    keys.bytes()
        .map(|byte| {
            let result = session.character(byte, byte.is_ascii_uppercase());
            assert!(
                result.handled,
                "{:?} of {keys:?} was not handled",
                byte as char
            );
            result.commit
        })
        .collect()
}

#[test]
fn korean_syllables_compose_in_the_preedit_and_commit_themselves() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Korean);
    let commits = type_korean(&mut session, "dkssud");
    assert_eq!(
        commits,
        [None, None, None, Some("안".to_owned()), None, None]
    );
    let snapshot = session.snapshot();
    assert_eq!(snapshot.scheme, SchemeType::Korean);
    assert_eq!(snapshot.preedit, "녕");
    assert_eq!(snapshot.normalized_segmentation, "녕");
    assert_eq!(snapshot.raw_segmentation, "sud");
    assert_eq!(snapshot.editing_text, "sud");
    assert_eq!(snapshot.caret_position, 3);
    assert!(snapshot.candidates.is_empty());
    assert!(snapshot.candidate_sources.is_empty());
    assert!(snapshot.candidate_answers_key.is_empty());
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert!(session.online_query().is_none());
    assert!(session.segment_raw_boundaries().is_empty());

    // 닭 + ㅏ splits the compound final: 달 is committed and 가 composes.
    let commits = type_korean(&mut session, "gkekfrk");
    assert_eq!(
        commits.into_iter().flatten().collect::<Vec<_>>().concat(),
        "녕하달"
    );
    assert_eq!(session.snapshot().preedit, "가");
}

#[test]
fn korean_shift_types_double_consonants_and_never_enters_a_local_mode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Korean);
    // Shift+R is the quanpin entry into temporary Japanese; in Korean it is ㄲ.
    assert!(session.character(b'R', true).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.preedit, "ㄲ");
    type_korean(&mut session, "kT");
    assert_eq!(session.snapshot().preedit, "깠");
    assert_eq!(
        type_korean(&mut session, "dO"),
        [Some("깠".to_owned()), None]
    );
    assert_eq!(session.snapshot().preedit, "얘");
}

#[test]
fn korean_backspace_removes_one_jamo_and_then_goes_to_the_host() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Korean);
    type_korean(&mut session, "rhkfr");
    let mut shown = vec![session.snapshot().preedit];
    while !session.snapshot().preedit.is_empty() {
        let result = session.command(Command::Backspace);
        assert!(result.handled);
        assert_eq!(result.commit, None);
        shown.push(session.snapshot().preedit);
    }
    assert_eq!(shown, ["괅", "괄", "과", "고", "ㄱ", ""]);
    // With nothing composed the host deletes the committed text itself.
    assert!(!session.command(Command::Backspace).handled);

    // A syllable that already left the composition is not reopened.
    assert_eq!(
        type_korean(&mut session, "rksk"),
        [None, None, None, Some("가".to_owned())]
    );
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().preedit, "ㄴ");
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().preedit, "");
}

#[test]
fn korean_space_enter_and_caret_keys_commit_the_open_syllable_and_pass_through() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Korean);
    for command in [
        Command::CommitCandidate,
        Command::CommitRaw,
        Command::CommitReading,
        Command::MoveLeft,
        Command::MoveRight,
        Command::MoveHome,
        Command::MoveEnd,
        Command::DeleteForward,
    ] {
        type_korean(&mut session, "gks");
        let result = session.command(command);
        assert!(!result.handled, "{command:?}");
        assert_eq!(result.commit.as_deref(), Some("한"), "{command:?}");
        assert_eq!(session.snapshot().preedit, "", "{command:?}");
        assert_eq!(session.snapshot().editing_text, "", "{command:?}");
        // Nothing is composing now, so the same key is the host's alone.
        let idle = session.command(command);
        assert!(!idle.handled && idle.commit.is_none(), "{command:?}");
    }

    // A standalone jamo is committed as it is shown.
    type_korean(&mut session, "r");
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("ㄱ")
    );

    // Finish is the host's explicit flush: handled, with the syllable.
    type_korean(&mut session, "rk");
    let finished = session.finish(0);
    assert!(finished.handled);
    assert_eq!(finished.commit.as_deref(), Some("가"));

    // Escape throws the open syllable away.
    type_korean(&mut session, "rk");
    let cancelled = session.command(Command::Cancel);
    assert!(cancelled.handled && cancelled.commit.is_none());
    assert_eq!(session.snapshot().preedit, "");

    // The Japanese-only variant key does nothing here.
    type_korean(&mut session, "rk");
    assert!(!session.command(Command::CycleKanaVariant).handled);
    assert_eq!(session.snapshot().preedit, "가");
}

#[test]
fn korean_digits_and_spaces_commit_the_open_syllable_before_the_host_inserts_them() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Korean);
    type_korean(&mut session, "rk");
    for key in *b"1 " {
        let result = session.character(key, false);
        assert!(!result.handled);
        assert_eq!(result.commit.as_deref(), Some("가"));
        assert_eq!(session.snapshot().preedit, "");
        let idle = session.character(key, false);
        assert!(!idle.handled && idle.commit.is_none());
        type_korean(&mut session, "rk");
    }
    // A candidate key has no candidate to pick.
    assert!(!session.candidate_key(b'1').handled);
    assert_eq!(session.snapshot().preedit, "가");
}

#[test]
fn korean_punctuation_is_half_width_and_follows_the_open_syllable() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    // Chinese punctuation is on by default; Korean ignores it.
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Korean);
    type_korean(&mut session, "dy");
    // As a character the mark is left for the punctuation route, and the syllable keeps composing.
    let typed = session.character(b'.', false);
    assert!(!typed.handled && typed.commit.is_none());
    assert_eq!(session.snapshot().preedit, "요");
    let result = session.punctuation(b'.');
    assert!(result.handled);
    assert_eq!(result.commit.as_deref(), Some("요."));
    assert_eq!(session.snapshot().preedit, "");
    // With nothing open the host inserts the ASCII key itself.
    for mark in *b",?!\"'" {
        let idle = session.punctuation(mark);
        assert!(!idle.handled && idle.commit.is_none());
    }
    type_korean(&mut session, "rk");
    assert_eq!(session.punctuation(b'?').commit.as_deref(), Some("가?"));
    type_korean(&mut session, "rk");
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("가\""));
}

#[test]
fn korean_ignores_the_caret_and_switching_to_it_starts_empty() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    session.switch_scheme(SchemeType::Korean).unwrap();
    assert_eq!(session.snapshot().preedit, "");
    type_korean(&mut session, "gks");
    session.set_caret(Some(1));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.caret_position, 3);
    assert_eq!(snapshot.preedit, "한");
    assert_eq!(session.prefix_end(), 3);
    assert_eq!(session.pending_suffix(), "");
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    assert_eq!(session.snapshot().preedit, "");
    type_text(&mut session, "ni");
    assert!(words(&session).contains(&"你".to_owned()));
}

// ---- Korean Hanja conversion ----

/// The leading Hanja of 한 in the embedded table, in its order.
const HAN_FIRST: [&str; 3] = ["韓", "漢", "寒"];

fn korean_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| options.scheme = SchemeType::Korean)
}

fn romaja_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| {
        options.scheme = SchemeType::Korean;
        options.korean_layout = crate::KoreanLayout::Romaja;
    })
}

#[test]
fn korean_romaja_retains_raw_keys_for_backspace_and_commits_stable_syllables() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (keys, expected, backspaced) in [("bang", "방", "반"), ("gwa", "과", "구")] {
        let mut session = romaja_session(&fixture);
        assert!(type_korean(&mut session, keys).iter().all(Option::is_none));
        assert_eq!(session.snapshot().preedit, expected);
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, backspaced);
    }
    let mut session = romaja_session(&fixture);
    assert!(type_korean(&mut session, "hang")
        .iter()
        .all(Option::is_none));
    assert_eq!(session.snapshot().preedit, "항");
    let commits = type_korean(&mut session, "ug");
    assert_eq!(commits.into_iter().flatten().collect::<String>(), "한");
    assert_eq!(session.snapshot().preedit, "국");
    assert_eq!(session.snapshot().editing_text, "gug");
    assert_eq!(
        session.command(Command::CommitReading).commit.as_deref(),
        Some("국")
    );
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    session.switch_scheme(SchemeType::Korean).unwrap();
    type_korean(&mut session, "xa");
    assert_eq!(session.snapshot().preedit, "아");
}

#[test]
fn korean_romaja_keeps_hanja_ascii_punctuation_and_unmapped_letters() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = romaja_session(&fixture);
    type_korean(&mut session, "han");
    assert!(session.command(Command::ConvertHanja).handled);
    assert_eq!(words(&session)[..3], HAN_FIRST);
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("韓")
    );
    for (keys, expected) in [
        ("Ga", "까"),
        ("Da", "따"),
        ("Ba", "빠"),
        ("Ja", "짜"),
        ("Sa", "싸"),
    ] {
        type_korean(&mut session, keys);
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
        assert_eq!(
            session.punctuation(b',').commit.as_deref(),
            Some(format!("{expected},").as_str())
        );
    }
    for letter in *b"qvzQVZ" {
        type_korean(&mut session, "han");
        let result = session.character(letter, letter.is_ascii_uppercase());
        assert!(result.handled);
        assert_eq!(result.commit, Some(format!("한{}", char::from(letter))));
        assert!(session.snapshot().preedit.is_empty());
        assert!(!session.command(Command::Backspace).handled);
    }
}

/// Rows in every table of the user journal, 0 when nothing ever opened it.
fn journal_rows(journal: &Path) -> i64 {
    let Ok(connection) = Connection::open(journal) else {
        return 0;
    };
    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()
        })
        .unwrap_or_default();
    tables
        .iter()
        .map(|table| count(journal, &format!("SELECT count(*) FROM \"{table}\"")))
        .sum()
}

/// Types `keys` and opens the Hanja list of the syllable they leave composing.
fn open_hanja(session: &mut Session, keys: &str) {
    type_korean(session, keys);
    let opened = session.command(Command::ConvertHanja);
    assert!(
        opened.handled && opened.commit.is_none(),
        "{keys}: {opened:?}"
    );
}

#[test]
fn korean_hanja_list_offers_the_composing_syllable_with_its_gloss() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    // 한국: 한 has left the composition, so the list is for 국.
    assert_eq!(
        type_korean(&mut session, "gksrnr"),
        [None, None, None, Some("한".to_owned()), None, None]
    );
    assert!(session.command(Command::ConvertHanja).handled);
    assert_eq!(words(&session)[0], "國");
    session.command(Command::Cancel);
    session.command(Command::Cancel);

    open_hanja(&mut session, "gks");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "한");
    assert_eq!(snapshot.editing_text, "gks");
    assert_eq!(words(&session)[..3], HAN_FIRST);
    assert_eq!(snapshot.candidate_annotations[0], "나라 이름 한, 한나라 한");
    assert_eq!(snapshot.candidate_annotations[1], "한수 한");
    // About three quarters of the source rows carry no 훈음, so some rows show none.
    assert!(snapshot.candidate_annotations.iter().any(String::is_empty));
    assert_eq!(
        snapshot.candidate_annotations.len(),
        snapshot.candidates.len()
    );
    assert!(snapshot
        .candidate_sources
        .iter()
        .all(|source| *source == CandidateSource::Database));
    assert!(snapshot
        .candidate_answers_key
        .iter()
        .all(|answers| *answers));
    assert!(snapshot
        .candidates
        .iter()
        .all(|item| item.scheme == SchemeType::Korean && item.pinyin == "gks"));
    assert!(session.online_query().is_none());
}

#[test]
fn korean_hanja_refresh_reuses_candidate_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    open_hanja(&mut session, "gks");
    let before = session.input.engine.candidates().to_vec();
    session.input.engine.handle_key(SchemeKey::Requery);
    // 会话的混排候选经 `clone_candidate_rows` 与引擎候选共用 `SharedString` 存储。重查时韩文汉字对这些共享字段 `clear()` 只是放手，随后的 `push_str` 新建字符串和 `Arc`，原缓冲区不会原地复用，指针必然变化（混排那份也不再另行复制）；这里只能确认重查后内容不变。
    assert_eq!(session.input.engine.candidates(), before);
}

#[test]
fn korean_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    type_korean(&mut session, "gks");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn choosing_a_hanja_commits_it_and_learns_nothing() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Korean;
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
    });
    TestClock::install(&mut session);
    for _ in 0..2 {
        open_hanja(&mut session, "gks");
        let chosen = session.select(1);
        assert!(chosen.handled);
        assert_eq!(chosen.commit.as_deref(), Some("漢"));
        assert_eq!(chosen.diagnostic, None);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.preedit, "");
        assert!(snapshot.candidates.is_empty());
    }
    // The order is the table's, not one the choices taught.
    open_hanja(&mut session, "gks");
    assert_eq!(words(&session)[..3], HAN_FIRST);
    // CommitCandidate takes the first Hanja while the list is open.
    let first = session.command(Command::CommitCandidate);
    assert!(first.handled);
    assert_eq!(first.commit.as_deref(), Some("韓"));
    // The engine's own candidate key picks from the list too.
    open_hanja(&mut session, "gks");
    assert_eq!(session.candidate_key(b'2').commit.as_deref(), Some("漢"));
    // Dropping the session writes whatever context learning it queued.
    drop(session);
    assert_eq!(journal_rows(&fixture.journal()), 0);
}

#[test]
fn korean_hanja_rows_cannot_be_pinned_removed_or_fixed() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    open_hanja(&mut session, "gks");
    assert!(!session.pin(1).handled);
    assert!(!session.remove(1).handled);
    assert!(!session.fix_position(1, 1).handled);
    assert!(!session.clear_position(1).handled);
    assert_eq!(words(&session)[..3], HAN_FIRST);
    assert_eq!(session.snapshot().preedit, "한");
    drop(session);
    assert_eq!(journal_rows(&fixture.journal()), 0);
}

#[test]
fn a_digit_in_the_open_hanja_list_is_left_to_page_selection() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    open_hanja(&mut session, "gks");
    // Unhandled with nothing committed: the runtime picks the row on the visible page, and the Hangul is still there if it does not.
    let digit = session.character(b'2', false);
    assert!(!digit.handled);
    assert_eq!(digit.commit, None);
    assert_eq!(session.snapshot().preedit, "한");
    assert_eq!(words(&session)[..3], HAN_FIRST);
    // 0 and Space are not selections: they commit the Hangul and go to the host, as with the list closed.
    for key in *b"0 " {
        let result = session.character(key, false);
        assert!(!result.handled);
        assert_eq!(result.commit.as_deref(), Some("한"));
        assert!(session.snapshot().candidates.is_empty());
        open_hanja(&mut session, "gks");
    }
}

#[test]
fn cancel_and_backspace_close_the_hanja_list_and_keep_the_syllable() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    for command in [Command::Cancel, Command::Backspace] {
        open_hanja(&mut session, "gks");
        let closed = session.command(command);
        assert!(closed.handled && closed.commit.is_none(), "{command:?}");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.preedit, "한", "{command:?}");
        assert!(snapshot.candidates.is_empty(), "{command:?}");
        assert!(snapshot.candidate_annotations.is_empty(), "{command:?}");
        // With the list closed the key does its usual work.
        session.command(command);
        let expected = if command == Command::Cancel {
            ""
        } else {
            "하"
        };
        assert_eq!(session.snapshot().preedit, expected, "{command:?}");
        session.command(Command::Cancel);
    }
    // The trigger closes the list it opened.
    open_hanja(&mut session, "gks");
    let toggled = session.command(Command::ConvertHanja);
    assert!(toggled.handled && toggled.commit.is_none());
    assert!(session.snapshot().candidates.is_empty());
    assert_eq!(session.snapshot().preedit, "한");
}

#[test]
fn a_letter_closes_the_hanja_list_and_keeps_composing() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    open_hanja(&mut session, "rk");
    let typed = session.character(b'r', false);
    assert!(typed.handled && typed.commit.is_none());
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "각");
    assert!(snapshot.candidates.is_empty());
    // A letter that starts the next syllable commits the Hangul, never a Hanja.
    open_hanja(&mut session, "");
    assert_eq!(words(&session)[0], "各");
    let next = session.character(b'k', false);
    assert_eq!(next.commit.as_deref(), Some("가"));
    assert_eq!(session.snapshot().preedit, "가");
    assert!(session.snapshot().candidates.is_empty());
}

#[test]
fn finishing_with_the_hanja_list_open_commits_the_hangul() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    // The index is the host's highlight; finishing closes the list first, so no highlight turns into a Hanja.
    for index in [0, 1, 99] {
        open_hanja(&mut session, "gks");
        let finished = session.finish(index);
        assert!(finished.handled);
        assert_eq!(finished.commit.as_deref(), Some("한"), "{index}");
        assert_eq!(session.snapshot().preedit, "");
        assert!(session.snapshot().candidates.is_empty());
    }
    open_hanja(&mut session, "gks");
    let punctuated = session.punctuation(b'.');
    assert!(punctuated.handled);
    assert_eq!(punctuated.commit.as_deref(), Some("한."));
    // The caret and commit commands end the syllable as Hangul too.
    open_hanja(&mut session, "gks");
    let raw = session.command(Command::CommitRaw);
    assert!(!raw.handled);
    assert_eq!(raw.commit.as_deref(), Some("한"));
}

#[test]
fn convert_hanja_is_unhandled_without_a_syllable_that_has_hanja() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = korean_session(&fixture);
    // Nothing composed.
    assert!(!session.command(Command::ConvertHanja).handled);
    // A lone jamo has no Hanja: the key goes back to the host and the jamo keeps composing.
    type_korean(&mut session, "r");
    let jamo = session.command(Command::ConvertHanja);
    assert!(!jamo.handled && jamo.commit.is_none());
    assert_eq!(session.snapshot().preedit, "ㄱ");
    assert!(session.snapshot().candidates.is_empty());
    // A letter after the refused trigger composes as usual.
    type_korean(&mut session, "k");
    assert_eq!(session.snapshot().preedit, "가");
    session.command(Command::Cancel);

    // Dedicated English keeps its own rules inside the Korean scheme.
    session.set_dedicated_english(true);
    type_text(&mut session, "gks");
    let english = session.command(Command::ConvertHanja);
    assert!(!english.handled && english.commit.is_none());
    assert_eq!(session.snapshot().preedit, "gks");
    session.set_dedicated_english(false);

    // Another scheme has no Hanja at all.
    let mut quanpin = fixture.session();
    type_text(&mut quanpin, "ni");
    let before = words(&quanpin);
    let result = quanpin.command(Command::ConvertHanja);
    assert!(!result.handled && result.commit.is_none());
    assert_eq!(words(&quanpin), before);
    assert_eq!(quanpin.snapshot().preedit, "ni");
}

#[test]
fn convert_hanja_is_named_for_the_golden_scenarios() {
    assert_eq!(Command::from_u8(11), Some(Command::ConvertHanja));
    assert_eq!(
        Command::from_name("ConvertHanja"),
        Some(Command::ConvertHanja)
    );
    assert_eq!(Command::ConvertHanja.name(), "ConvertHanja");
    assert_eq!(Command::from_u8(12), Some(Command::ConversionLeft));
    assert_eq!(
        Command::from_name("ConversionRight"),
        Some(Command::ConversionRight)
    );
    assert_eq!(
        Command::from_u8(14),
        Some(Command::CommitRawWithoutLearning)
    );
    assert_eq!(
        Command::from_name("CommitRawWithoutLearning"),
        Some(Command::CommitRawWithoutLearning)
    );
    assert_eq!(Command::from_u8(15), None);
}

// ---- expression, command and mention modes ----

fn generated_modes_session(fixture: &Fixture) -> Session {
    let mut session = fixture.session_with(|options| {
        options.local_modes.expression = true;
        options.local_modes.command = true;
        options.local_modes.mention = true;
        options.command_table = vec![crate::types::CommandTableEntry {
            trigger: "sig".to_owned(),
            title: "签名".to_owned(),
            template: "张三 {date}".to_owned(),
        }];
        options.mention_entries = vec![
            crate::types::MentionEntry {
                phone: String::new(),
                text: "张三".to_owned(),
                key: "zhang'san".to_owned(),
            },
            crate::types::MentionEntry {
                phone: String::new(),
                text: "深圳市".to_owned(),
                key: "shen'zhen'shi".to_owned(),
            },
        ];
    });
    TestClock::install(&mut session);
    session
}

#[test]
fn generated_modes_are_off_by_default() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert!(session.snapshot().spelling_symbols.is_empty());
    assert!(session.character(b'V', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert_eq!(session.snapshot().preedit, "V");
    session.command(Command::Cancel);
    assert!(!session.character(b'/', false).handled);
    assert!(!session.character(b'@', false).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    // `/` still has no Chinese mark and goes to the host as typed.
    assert!(!session.punctuation(b'/').handled);
}

#[test]
fn expression_mode_evaluates_what_follows_shift_v() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    assert!(session.character(b'V', true).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Expression);
    assert_eq!(snapshot.spelling_symbols, "0123456789+-*/.()%^#$&");
    assert_eq!(words(&session), ["V"]);
    // 数字之前的字母不能表示单位，入口和它一起转回普通组字（#10123），不提前上屏。
    let rejected = session.character(b'x', false);
    assert!(
        rejected.handled && rejected.commit.is_none(),
        "{rejected:?}"
    );
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.editing_text, "Vx");
    assert!(session.command(Command::Cancel).handled);

    session.character(b'V', true);
    assert!(session.character(b'1', false).handled);
    // An operator reported as punctuation extends the expression instead of finishing it.
    let plus = session.punctuation(b'+');
    assert!(plus.handled && plus.commit.is_none(), "{plus:?}");
    assert!(session.character(b'2', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "V1+2");
    assert_eq!(words(&session), ["3", "1+2=3", "叁元整"]);
    assert!(snapshot.candidate_annotations.iter().all(String::is_empty));
    assert!(session.online_query().is_none());

    let result = session.select(1);
    assert_eq!(result.commit.as_deref(), Some("1+2=3"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
}

#[test]
fn expression_mode_commits_finite_results_near_the_float_limit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    for (input, expected) in [
        ("(2^1023)*1.9999999999999998", "1.79769313486232e308"),
        ("(2^1023)*-1.9999999999999998", "-1.79769313486232e308"),
    ] {
        assert!(session.character(b'V', true).handled);
        type_text(&mut session, input);
        assert_eq!(
            words(&session),
            [expected.to_owned(), format!("{input}={expected}")]
        );
        assert_eq!(session.select(0).commit.as_deref(), Some(expected));
    }
}

#[test]
fn expression_mode_converts_units_after_a_number() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'V', true);
    // 单位字母之前的撇号不是分隔符，提交原文并退出。
    let rejected = session.character(b'\'', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("V"));
    session.character(b'V', true);
    type_text(&mut session, "3jin");
    // A host that reports the apostrophe as punctuation still gets the separator.
    let apostrophe = session.punctuation(b'\'');
    assert!(
        apostrophe.handled && apostrophe.commit.is_none(),
        "{apostrophe:?}"
    );
    // 第二个撇号同样交还宿主；重新进入后继续验证正常单位转换。
    let rejected = session.character(b'\'', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("V3jin'"));
    session.character(b'V', true);
    type_text(&mut session, "3jin'g");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "V3jin'g");
    assert_eq!(words(&session), ["1500克", "1500", "3斤=1500克"]);
    assert!(snapshot
        .candidates
        .iter()
        .all(|candidate| candidate.source == CandidateSource::Generated));
    assert!(session.online_query().is_none());
    assert_eq!(session.select(2).commit.as_deref(), Some("3斤=1500克"));

    // Input the unit table cannot read keeps the expression path and its raw fallback.
    session.character(b'V', true);
    type_text(&mut session, "3xyz");
    assert_eq!(words(&session), ["V3xyz"]);
}

#[test]
fn expression_mode_converts_signed_measurements() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    for (input, expected) in [
        ("-10cf", vec!["14℉", "14", "-10℃=14℉"]),
        ("+0.5kg'g", vec!["500克", "500", "+0.5千克=500克"]),
    ] {
        session.character(b'V', true);
        for byte in input.bytes() {
            let result = if matches!(byte, b'-' | b'+' | b'.' | b'\'') {
                session.punctuation(byte)
            } else {
                session.character(byte, false)
            };
            assert!(
                result.handled && result.commit.is_none(),
                "{input}: {result:?}"
            );
        }
        assert_eq!(session.snapshot().preedit, format!("V{input}"));
        assert_eq!(words(&session), expected);
        assert_eq!(
            session.select(2).commit.as_deref(),
            expected.get(2).copied()
        );
    }
}

#[test]
fn expression_mode_keeps_an_unfinished_input_as_raw_text_and_bounds_it() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'V', true);
    type_text(&mut session, "12*");
    assert_eq!(words(&session), ["V12*"]);
    assert_eq!(
        session.snapshot().candidates[0].source,
        CandidateSource::Fallback
    );
    // The caret inserts spelling symbols too.
    session.command(Command::Cancel);
    session.character(b'V', true);
    type_text(&mut session, "12");
    session.set_caret(Some(2));
    assert!(session.character(b'+', false).handled);
    assert_eq!(session.snapshot().preedit, "V1+2");
    assert_eq!(words(&session)[0], "3");

    session.command(Command::Cancel);
    session.character(b'V', true);
    for _ in 1..crate::local::GENERATED_MODE_INPUT_LIMIT {
        assert!(session.character(b'9', false).handled);
    }
    let raw = session.snapshot().preedit;
    assert_eq!(raw.len(), crate::local::GENERATED_MODE_INPUT_LIMIT);
    let rejected = session.character(b'9', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some(raw.as_str()));
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn expression_mode_inserts_unit_letters_at_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'V', true);
    type_text(&mut session, "3g");
    session.set_caret(Some(2));

    let result = session.character(b'k', false);

    assert!(result.handled, "{result:?}");
    assert!(result.commit.is_none(), "{result:?}");
    assert_eq!(session.snapshot().preedit, "V3kg");
    assert_eq!(session.snapshot().caret_position, 3);
    assert!(words(&session).contains(&"3000克".to_owned()));
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().preedit, "V3g");
    assert_eq!(session.snapshot().caret_position, 2);
}

#[test]
fn expression_mode_inserts_a_unit_separator_at_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (text, caret, raw, expected) in [
        ("3jinkg", 5, "V3jin'kg", ["1.5千克", "1.5", "3斤=1.5千克"]),
        ("3kgjin", 4, "V3kg'jin", ["6斤", "6", "3千克=6斤"]),
        ("3gkg", 3, "V3g'kg", ["0.003千克", "0.003", "3克=0.003千克"]),
    ] {
        for punctuation in [false, true] {
            let mut session = generated_modes_session(&fixture);
            session.character(b'V', true);
            type_text(&mut session, text);
            session.set_caret(Some(caret));
            assert!(session.takes_local_separator());

            let result = if punctuation {
                session.punctuation(b'\'')
            } else {
                session.character(b'\'', false)
            };

            assert!(result.handled, "{result:?}");
            assert!(result.commit.is_none(), "{result:?}");
            assert_eq!(session.snapshot().preedit, raw);
            assert_eq!(session.snapshot().caret_position, caret + 1);
            assert_eq!(words(&session), expected);
            assert!(!session.takes_local_separator());
            assert_eq!(session.select(2).commit.as_deref(), Some(expected[2]));

            session.character(b'V', true);
            type_text(&mut session, &raw[1..]);
            session.set_caret(Some(caret + 1));
            let rejected = session.character(b'\'', false);
            assert!(!rejected.handled);
            assert_eq!(rejected.commit.as_deref(), Some(raw));
            assert!(session.snapshot().editing_text.is_empty());
        }
    }
}

#[test]
fn expression_mode_does_not_accept_a_unit_letter_before_the_number_at_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    for (text, caret) in [("3g", 1), ("-3g", 2)] {
        session.character(b'V', true);
        type_text(&mut session, text);
        session.set_caret(Some(caret));
        let before = session.snapshot();

        let result = session.character(b'k', false);

        assert!(!result.handled, "{result:?}");
        assert_eq!(result.commit.as_deref(), Some(before.preedit.as_str()));
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
        assert!(session.snapshot().editing_text.is_empty());
        assert!(!session.takes_local_separator());
        session.command(Command::Cancel);
    }
}

#[test]
fn expression_mode_limits_unit_insertions_at_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for value in *b"k'" {
        let mut session = generated_modes_session(&fixture);
        session.character(b'V', true);
        type_text(&mut session, "3g");
        while session.snapshot().preedit.len() < crate::local::GENERATED_MODE_INPUT_LIMIT {
            assert!(session.character(b'g', false).handled);
        }
        session.set_caret(Some(3));
        let before = session.snapshot();
        let result = session.character(value, false);
        assert!(!result.handled);
        assert_eq!(result.commit.as_deref(), Some(before.preedit.as_str()));
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
        assert!(session.snapshot().editing_text.is_empty());
    }
}

#[test]
fn expression_mode_needs_a_pinyin_scheme_and_an_empty_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    type_text(&mut session, "ni");
    // With a composition an uppercase letter is a helpcode, not a mode entry.
    session.character(b'V', true);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    session.command(Command::Cancel);
    session.switch_scheme(SchemeType::Wubi).unwrap();
    assert!(!session.character(b'V', true).handled);
    assert!(session.character(b'/', false).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Command);
    session.command(Command::Cancel);
    assert!(session.character(b'@', false).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Mention);
    session.command(Command::Cancel);
    assert!(session.character(b'K', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::QuickPhrase);
}

#[test]
fn slash_opens_the_command_list_and_letters_filter_it() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    assert_eq!(session.snapshot().spelling_symbols, "/@");
    assert!(session.character(b'/', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Command);
    assert!(snapshot.spelling_symbols.is_empty());
    assert_eq!(
        words(&session),
        [
            "张三 2026-08-09",
            "2026年8月9日",
            "14:30",
            "星期日",
            "丙午年六月二十七日",
            "2026年第3季度",
            "已过220天/剩145天",
            "已过31周/剩22周",
            "马年"
        ]
    );
    assert_eq!(
        snapshot.candidate_annotations,
        ["签名", "日期", "时间", "星期", "农历", "季度", "天数", "周", "生肖"]
    );

    type_text(&mut session, "si");
    assert_eq!(words(&session), ["张三 2026-08-09"]);
    let result = session.select(0);
    assert_eq!(result.commit.as_deref(), Some("张三 2026-08-09"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // 内置日期触发词输入完整后，列出全部格式。
    session.character(b'/', false);
    type_text(&mut session, "rq");
    assert_eq!(words(&session).len(), 20);

    // Nothing matches: the literal text is the only row, and Enter commits it too.
    session.command(Command::Cancel);
    session.character(b'/', false);
    type_text(&mut session, "zz");
    assert_eq!(words(&session), ["/zz"]);
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("/zz")
    );
}

#[test]
fn slash_after_a_composition_is_still_the_typed_character() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    type_text(&mut session, "ni");
    assert!(!session.snapshot().spelling_symbols.contains('/'));
    assert!(!session.character(b'/', false).handled);
    // A runtime finishes the composition before it asks for the mark, so the session is empty by the time the mark is translated; it must not open the mode then.
    let finished = session.finish(0);
    assert_eq!(finished.commit.as_deref(), Some("你"));
    assert!(!session.punctuation(b'/').handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
}

#[test]
fn a_mark_on_a_bare_prefix_types_punctuation_instead_of_a_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'/', false);
    let result = session.punctuation(b'/');
    let commit = result.commit.expect("bare slash commits");
    assert_eq!(commit, "//");
    assert_eq!(commit.capacity(), commit.len());
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    session.character(b'/', false);
    assert_eq!(session.punctuation(b',').commit.as_deref(), Some("/，"));
    session.character(b'/', false);
    assert_eq!(session.punctuation(b'\\').commit.as_deref(), Some("/、"));
    session.character(b'@', false);
    assert_eq!(session.punctuation(b'@').commit.as_deref(), Some("@@"));
    // The character route reaches the same answer instead of swallowing the key.
    session.character(b'@', false);
    assert_eq!(
        session.character(b'.', false).commit.as_deref(),
        Some("@。")
    );
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    // 有关键词后不接受的标点先提交原文，再交还当前键。
    session.character(b'/', false);
    type_text(&mut session, "rq");
    let rejected = session.character(b',', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("/rq"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    // Space still picks the first row and Enter the literal prefix.
    session.command(Command::Cancel);
    session.character(b'/', false);
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("张三 2026-08-09")
    );
    session.character(b'/', false);
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("/")
    );
}

#[test]
fn symbol_entries_follow_the_punctuation_mode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.set_chinese_punctuation_enabled(false);
    assert!(session.snapshot().spelling_symbols.is_empty());
    assert!(!session.character(b'/', false).handled);
    session.set_chinese_punctuation_enabled(true);
    session.set_punctuation_lock(2).unwrap();
    assert!(!session.character(b'@', false).handled);
    session.set_punctuation_lock(0).unwrap();
    assert!(session.character(b'@', false).handled);
    // Shift+V opens the expression mode whatever the punctuation mode is.
    session.command(Command::Cancel);
    session.set_chinese_punctuation_enabled(false);
    assert!(session.character(b'V', true).handled);
}

#[test]
fn at_lists_the_mention_list_and_letters_filter_it() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    assert!(session.character(b'@', false).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Mention);
    assert_eq!(words(&session), ["张三", "深圳市"]);
    // Rows are names, not spellings: no helpcode beside them.
    assert!(session
        .snapshot()
        .candidate_annotations
        .iter()
        .all(String::is_empty));
    type_text(&mut session, "szs");
    assert_eq!(words(&session), ["深圳市"]);
    assert_eq!(session.select(0).commit.as_deref(), Some("深圳市"));

    // The list can be replaced while the mode is open.
    session.character(b'@', false);
    session.set_mention_entries(&[crate::types::MentionEntry {
        phone: String::new(),
        text: "李四".to_owned(),
        key: "li'si".to_owned(),
    }]);
    assert_eq!(words(&session), ["李四"]);
    session.command(Command::Cancel);
    session.character(b'/', false);
    session.set_command_table(&[]);
    assert_eq!(
        words(&session),
        [
            "2026年8月9日",
            "14:30",
            "星期日",
            "丙午年六月二十七日",
            "2026年第3季度",
            "已过220天/剩145天",
            "已过31周/剩22周",
            "马年"
        ]
    );
}

#[test]
fn mention_mode_offers_places_after_the_list_when_switched_on() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'@', false);
    type_text(&mut session, "chongqing");
    assert_eq!(words(&session), ["@chongqing"]);

    // Switching on while the mode is open refreshes the list.
    assert_eq!(session.set_mention_places(true), None);
    assert_eq!(words(&session), ["重庆市"]);
    assert_eq!(session.snapshot().candidate_annotations, [""]);
    session.command(Command::Cancel);

    session.character(b'@', false);
    type_text(&mut session, "sz");
    let snapshot = session.snapshot();
    let candidates: Vec<&str> = snapshot
        .candidates
        .iter()
        .map(|candidate| candidate.word.as_str())
        .collect();
    // The user's own 深圳市 leads and is not repeated by the place of the same name.
    assert_eq!(candidates[0], "深圳市");
    assert_eq!(
        candidates.iter().filter(|text| **text == "深圳市").count(),
        1
    );
    assert_eq!(snapshot.candidate_annotations[0], "");
    let suzhou = candidates
        .iter()
        .position(|text| *text == "苏州市")
        .unwrap();
    assert_eq!(snapshot.candidate_annotations[suzhou], "江苏省");
    assert_eq!(session.select(suzhou).commit.as_deref(), Some("苏州市"));

    session.set_mention_places(false);
    session.character(b'@', false);
    type_text(&mut session, "sz");
    assert_eq!(words(&session), ["深圳市"]);
}

#[test]
fn mention_place_annotation_uses_the_shown_row_when_a_list_name_does_not_match() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.set_mention_entries(&[crate::types::MentionEntry {
        phone: String::new(),
        text: "深圳市".to_owned(),
        key: "other".to_owned(),
    }]);
    session.set_mention_places(true);
    session.character(b'@', false);
    type_text(&mut session, "shenzhenshi");

    let snapshot = session.snapshot();
    let shenzhen = snapshot
        .candidates
        .iter()
        .position(|candidate| candidate.word == "深圳市")
        .expect("embedded place should be offered");
    assert_eq!(snapshot.candidates[shenzhen].pinyin, "shen'zhen'shi");
    assert_eq!(snapshot.candidate_annotations[shenzhen], "广东省");
}

#[test]
fn generated_mode_commits_are_never_learned() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.local_modes.expression = true;
        options.local_modes.command = true;
        options.local_modes.mention = true;
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
        options.mention_entries = vec![crate::types::MentionEntry {
            phone: String::new(),
            text: "张三".to_owned(),
            key: "zhang'san".to_owned(),
        }];
    });
    TestClock::install(&mut session);
    session.character(b'V', true);
    type_text(&mut session, "123");
    assert_eq!(session.select(2).commit.as_deref(), Some("壹佰贰拾叁元整"));
    session.character(b'/', false);
    assert!(session.select(1).commit.is_some());
    session.character(b'@', false);
    assert_eq!(session.select(0).commit.as_deref(), Some("张三"));
    session.character(b'V', true);
    type_text(&mut session, "1+");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("V1+")
    );
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT COUNT(*) FROM user_dictionary_operations"
        ),
        0
    );
}

/// 进入符号模式并打出 `typed`，返回候选。
fn symbol_mode_rows(fixture: &Fixture, typed: &str) -> Vec<String> {
    let mut session = fixture.session();
    assert!(session.character(b'S', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Symbol);
    type_text(&mut session, typed);
    session
        .snapshot()
        .candidates
        .iter()
        .map(|item| item.word.to_string())
        .collect()
}

/// 符号模式（Shift+S）的分组码 `xl` 出希腊字母。
#[test]
fn symbol_mode_group_xl_gives_greek_letters() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let rows = symbol_mode_rows(&fixture, "xl");
    assert_eq!(rows[..3], ["α", "β", "γ"], "{rows:?}");
}

/// 分组码 `sx` 出数学符号。
#[test]
fn symbol_mode_group_sx_gives_math_symbols() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let rows = symbol_mode_rows(&fixture, "sx");
    assert!(rows.iter().any(|text| text == "∑"), "{rows:?}");
    assert!(rows.iter().any(|text| text == "√"), "{rows:?}");
}

/// 分组码 `jt` 出箭头。
#[test]
fn symbol_mode_group_jt_gives_arrows() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let rows = symbol_mode_rows(&fixture, "jt");
    assert!(rows.iter().any(|text| text == "→"), "{rows:?}");
}

/// 符号名 `alpha` 出 α；符号名在分组码之后给，不重复。
#[test]
fn symbol_mode_name_alpha_gives_the_letter() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let rows = symbol_mode_rows(&fixture, "alpha");
    assert_eq!(rows[0], "α", "{rows:?}");
    assert_eq!(rows.iter().filter(|text| *text == "α").count(), 1);
}

/// 选中符号行整段上屏并结束组字；与 emoji 行一样不学习、不调频。
#[test]
fn symbol_mode_commits_the_row_and_ends_the_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert!(session.character(b'S', true).handled);
    type_text(&mut session, "xl");
    // 符号模式只收字母，数字交还宿主：选中高亮那一行走 `CommitCandidate`。
    let result = session.command(Command::CommitCandidate);
    assert_eq!(result.commit.as_deref(), Some("α"), "{result:?}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert!(snapshot.preedit.is_empty());
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT COUNT(*) FROM user_dictionary_operations"
        ),
        0
    );
}

/// 关掉偏好后 Shift+S 不再进入符号模式，照常打出一个大写 S（或开始拼音组字）。
#[test]
fn disabled_symbol_mode_leaves_shift_s_to_the_scheme() {
    let fixture = Fixture::new(HELLO_FIXTURE);
    // 同一份夹具下开着偏好时确实进模式，关掉后那条断言才不是空的。
    let mut on = fixture.session();
    assert!(on.character(b'S', true).handled);
    assert_eq!(on.snapshot().local_mode, LocalInputMode::Symbol);
    let mut session = fixture.session_with(|options| options.local_modes.symbol = false);
    assert!(session.character(b'S', true).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.preedit, "S");
    assert!(snapshot.candidates.iter().all(|item| item.word != "α"));
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("S")
    );
}

#[test]
fn local_modes_never_ask_for_online_candidates() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(ENGLISH_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    // The same session does ask while composing pinyin, so the checks below are not vacuous.
    type_text(&mut session, "ni");
    assert!(session.online_query().is_some());
    session.command(Command::Cancel);
    for (entry, shift, input) in [
        (b'U', true, "4e2d"),
        (b'T', true, "rq"),
        (b'K', true, "ab"),
        (b'E', true, "xl"),
        (b'M', true, "hx"),
        (b'J', true, "nh"),
        (b'Y', true, "hello"),
        (b'V', true, "1+2"),
        (b'V', true, "3jin'g"),
        (b'/', false, "si"),
        (b'@', false, "zs"),
        // The translate trigger means nothing outside `/`.
        (b'@', false, "fyhello"),
        (b'Y', true, "fyhello"),
        (b'E', true, "fyhello"),
        (b'/', false, "fyhello'world"),
    ] {
        assert!(session.character(entry, shift).handled, "{}", entry as char);
        let mode = session.snapshot().local_mode;
        assert_ne!(mode, LocalInputMode::None, "{}", entry as char);
        assert!(session.online_query().is_none(), "{mode:?} on entry");
        assert!(
            session.command_translation_query().is_none(),
            "{mode:?} on entry"
        );
        for byte in input.bytes() {
            session.character(byte, false);
            assert!(session.online_query().is_none(), "{mode:?} after {input:?}");
            // `/fy` is the one local input with a request of its own: a translation, never cloud or AI candidates.
            let translation = session.command_translation_query();
            let typed = session.snapshot().preedit;
            assert_eq!(
                translation.is_some(),
                mode == LocalInputMode::Command && typed.len() > "/fy".len(),
                "{mode:?} at {typed:?}"
            );
        }
        session.command(Command::Cancel);
    }
    // 网址模式从组字进入，不在上面的入口键里。
    type_text(&mut session, "www");
    assert!(session.punctuation(b'.').handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Url);
    assert!(session.online_query().is_none(), "Url on entry");
    type_text(&mut session, "github");
    assert!(session.online_query().is_none(), "Url after typing");
    assert!(session.command_translation_query().is_none());
}

#[test]
fn translate_command_inserts_word_separators_at_the_caret() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (text, caret, expected) in [
        ("fyhelloworld", 8, "hello world"),
        ("fanyihelloworld", 11, "hello world"),
        ("translatehelloworld", 15, "hello world"),
        ("fyab", 4, "a b"),
        ("fyhelloworld'", 8, "hello world"),
    ] {
        for punctuation in [false, true] {
            let mut session = generated_modes_session(&fixture);
            session.character(b'/', false);
            type_text(&mut session, text);
            session.set_caret(Some(caret));
            assert!(session.takes_local_separator(), "{text} at {caret}");

            let result = if punctuation {
                session.punctuation(b'\'')
            } else {
                session.character(b'\'', false)
            };

            assert!(result.handled, "{result:?}");
            assert!(result.commit.is_none(), "{result:?}");
            let mut raw = format!("/{text}");
            raw.insert(caret, '\'');
            assert_eq!(session.snapshot().preedit, raw);
            assert_eq!(session.snapshot().caret_position, caret + 1);
            assert_eq!(session.command_translation_query().unwrap().text, expected);
            assert_eq!(words(&session), [expected]);
            session.command(Command::Backspace);
            assert_eq!(session.snapshot().preedit, format!("/{text}"));
            assert_eq!(session.snapshot().caret_position, caret);
        }
    }
}

#[test]
fn translate_command_checks_the_caret_before_accepting_a_separator() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (text, caret) in [
        ("fyhello", 1),
        ("fyhello", 2),
        ("fyhello", 3),
        ("fanyihello", 6),
        ("translatehello", 10),
        ("fyhello'world", 9),
    ] {
        let mut session = generated_modes_session(&fixture);
        session.character(b'/', false);
        type_text(&mut session, text);
        session.set_caret(Some(caret));
        assert!(!session.takes_local_separator(), "{text} at {caret}");
        let before = session.snapshot();
        let rejected = session.character(b'\'', false);
        assert!(!rejected.handled);
        assert_eq!(rejected.commit.as_deref(), Some(before.preedit.as_str()));
        assert!(session.snapshot().editing_text.is_empty());
    }
}

#[test]
fn translate_command_preserves_separator_insertion_limits() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for punctuation in [false, true] {
        let mut session = generated_modes_session(&fixture);
        session.character(b'/', false);
        type_text(&mut session, "fyhello'world");
        session.set_caret(Some(8));
        let before = session.snapshot();
        let result = if punctuation {
            session.punctuation(b'\'')
        } else {
            session.character(b'\'', false)
        };
        assert!(result.handled && result.commit.is_none(), "{result:?}");
        assert_eq!(session.snapshot(), before);

        session.command(Command::Cancel);
        session.character(b'/', false);
        type_text(&mut session, "fyhello");
        while session.snapshot().preedit.len() < crate::local::GENERATED_MODE_INPUT_LIMIT {
            assert!(session.character(b'a', false).handled);
        }
        session.set_caret(Some(8));
        let before = session.snapshot();
        let rejected = session.character(b'\'', false);
        assert!(!rejected.handled);
        assert_eq!(rejected.commit.as_deref(), Some(before.preedit.as_str()));
        assert!(session.snapshot().editing_text.is_empty());
    }

    for (entry, text, caret) in [(b'@', "fyhello", 4), (b'/', "sighello", 5)] {
        let mut session = generated_modes_session(&fixture);
        session.character(entry, false);
        type_text(&mut session, text);
        session.set_caret(Some(caret));
        assert!(!session.takes_local_separator());
        let before = session.snapshot();
        let rejected = session.character(b'\'', false);
        assert!(!rejected.handled);
        assert_eq!(rejected.commit.as_deref(), Some(before.preedit.as_str()));
        assert!(session.snapshot().editing_text.is_empty());
    }
}

#[test]
fn the_translate_command_asks_for_its_english_and_commits_the_answer() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'/', false);
    type_text(&mut session, "fyhello");
    // 宿主当标点送来的撇号同样能分隔单词；连续撇号则提交原文并退出。
    let apostrophe = session.punctuation(b'\'');
    assert!(
        apostrophe.handled && apostrophe.commit.is_none(),
        "{apostrophe:?}"
    );
    let rejected = session.character(b'\'', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some("/fyhello'"));
    assert!(session.command_translation_query().is_none());
    session.character(b'/', false);
    type_text(&mut session, "fyhello'world");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "/fyhello'world");
    assert_eq!(words(&session), ["hello world"]);
    assert_eq!(snapshot.candidate_annotations, ["翻译"]);
    let query = session
        .command_translation_query()
        .expect("a translation request");
    assert_eq!(query.text, "hello world");

    // Answers that do not fit a row, or that belong to another session or another text, are refused.
    for translation in ["", "  ", "你好\n世界", &"字".repeat(200)] {
        assert!(
            !session.apply_command_translation(&query, translation),
            "{translation:?}"
        );
    }
    let mut other_session = query.clone();
    other_session.session_id += 1;
    assert!(!session.apply_command_translation(&other_session, "你好世界"));
    let mut other_text = query.clone();
    other_text.text = "hello".to_owned();
    assert!(!session.apply_command_translation(&other_text, "你好"));
    assert_eq!(words(&session), ["hello world"]);

    assert!(session.apply_command_translation(&query, "你好世界"));
    assert_eq!(words(&session), ["你好世界", "hello world"]);
    assert_eq!(session.snapshot().candidate_annotations, ["翻译", "翻译"]);
    // The same answer twice adds nothing.
    assert!(!session.apply_command_translation(&query, "你好世界"));
    assert_eq!(session.select(0).commit.as_deref(), Some("你好世界"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    // The mode is gone, and with it the request.
    assert!(!session.apply_command_translation(&query, "你好世界"));

    // A late answer for text since edited is refused.
    session.character(b'/', false);
    type_text(&mut session, "fyhello");
    let query = session
        .command_translation_query()
        .expect("a translation request");
    session.character(b'x', false);
    assert!(!session.apply_command_translation(&query, "你好"));
    assert_eq!(words(&session), ["hellox"]);
}

// ---- 脚本插件的 `/` 指令 ----

fn plugin_command(trigger: &str, title: &str) -> crate::types::PluginCommand {
    crate::types::PluginCommand {
        trigger: trigger.to_owned(),
        title: title.to_owned(),
        extended: false,
    }
}

fn plugin_commands() -> Vec<crate::types::PluginCommand> {
    vec![plugin_command("uuid", "UUID"), plugin_command("cz", "拆字")]
}

fn plugin_rows(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(text, note)| ((*text).to_owned(), (*note).to_owned()))
        .collect()
}

fn plugin_session(fixture: &Fixture, configure: impl FnOnce(&mut SessionOptions)) -> Session {
    let mut session = fixture.session_with(|options| {
        options.local_modes.command = true;
        options.plugin_commands = plugin_commands();
        configure(options);
    });
    TestClock::install(&mut session);
    session
}

/// 打 `/uuid`：引擎自己不列行，只有兜底行，并且要宿主去问插件。
fn open_uuid(session: &mut Session) -> crate::types::PluginCommandQuery {
    assert!(session.character(b'/', false).handled);
    type_text(session, "uuid");
    assert_eq!(words(session), ["/uuid"]);
    assert_eq!(candidate_sources(session), [CandidateSource::Fallback]);
    session
        .plugin_command_query()
        .expect("a plugin command query")
}

/// `arguments = "extended"` 的指令：完整打出触发词之后数字和运算符号是参数，引擎收下并列进 `spelling_symbols`（宿主照它把数字当输入、不当选词键）；触发词没打完、或者是只收字母的指令时，什么都和以前一样。
#[test]
fn extended_plugin_commands_take_digits_and_operators_after_the_trigger() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |options| {
        options.plugin_commands.push(crate::types::PluginCommand {
            extended: true,
            ..plugin_command("calc", "计算")
        });
    });
    assert!(session.character(b'/', false).handled);
    type_text(&mut session, "cal");
    assert_eq!(session.snapshot().spelling_symbols, "");
    type_text(&mut session, "c");
    let symbols = session.snapshot().spelling_symbols;
    assert_eq!(symbols, "0123456789.+-*/%()=,");
    for byte in b"12+3*(4)/5%=6,7.8-9" {
        assert!(
            session.character(*byte, false).handled,
            "{}",
            char::from(*byte)
        );
    }
    let query = session
        .plugin_command_query()
        .expect("a plugin command query");
    assert_eq!(query.preedit, "/calc12+3*(4)/5%=6,7.8-9");
    assert_eq!(
        (query.trigger.as_str(), query.argument.as_str()),
        ("calc", "12+3*(4)/5%=6,7.8-9")
    );
    // 光标挪回触发词里：数字不再列出、也不收。
    for _ in 0.."12+3*(4)/5%=6,7.8-9".len() + 1 {
        session.command(Command::MoveLeft);
    }
    assert_eq!(session.snapshot().spelling_symbols, "");
    // 光标在触发词里时数字不是参数：引擎上屏打出的原文，把数字交还宿主（和以前一样）。
    let typed = session.character(b'1', false);
    assert!(!typed.handled);
    assert_eq!(typed.commit.as_deref(), Some("/calc12+3*(4)/5%=6,7.8-9"));

    // 只收字母的指令：数字照旧交还宿主。
    session.command(Command::Cancel);
    session.character(b'/', false);
    type_text(&mut session, "uuid");
    assert_eq!(session.snapshot().spelling_symbols, "");
    let typed = session.character(b'4', false);
    assert!(!typed.handled);
    assert_eq!(typed.commit.as_deref(), Some("/uuid"));
}

#[test]
fn plugin_rows_replace_the_fallback_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |_| {});
    let query = open_uuid(&mut session);
    assert_eq!(query.preedit, "/uuid");
    assert_eq!(query.trigger, "uuid");
    assert_eq!(query.argument, "");
    // 用不了的行被丢掉：同文、空白、控制字符、超过 199 个码元。
    let long = "字".repeat(200);
    assert!(session.apply_plugin_command_rows(
        &query,
        &plugin_rows(&[
            ("a-b", "小写"),
            ("A-B", "大写"),
            ("a-b", "重复"),
            (" ", "空白"),
            ("x\ny", "换行"),
            (&long, "太长"),
        ]),
    ));
    assert_eq!(words(&session), ["a-b", "A-B"]);
    let snapshot = session.snapshot();
    assert!(snapshot
        .candidates
        .iter()
        .all(|item| item.source == CandidateSource::Plugin && item.pinyin == "uuid"));
    // 标题和插件注释由 runtime 画在插件注释格里，引擎注释格留空。
    assert_eq!(snapshot.candidate_annotations, ["", ""]);
    // 同一份预编辑串问过了，不再问；同一个查询再交一次被拒绝。
    assert!(session.plugin_command_query().is_none());
    assert!(!session.apply_plugin_command_rows(&query, &plugin_rows(&[("c", "")])));
    assert_eq!(words(&session), ["a-b", "A-B"]);

    // 参数跟在触发词后面。
    session.command(Command::Cancel);
    session.character(b'/', false);
    type_text(&mut session, "czhuohuo");
    let query = session
        .plugin_command_query()
        .expect("a plugin command query");
    assert_eq!(
        (query.trigger.as_str(), query.argument.as_str()),
        ("cz", "huohuo")
    );
}

#[test]
fn plugin_row_filter_does_not_allocate_a_second_text_index() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |_| {});
    let query = open_uuid(&mut session);
    let rows = plugin_rows(&[("甲", ""), ("乙", ""), ("甲", "重复")]);
    let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        assert!(session.apply_plugin_command_rows(&query, &rows));
    });
    assert_eq!(
        allocations, 12,
        "插件行过滤不应为临时文本索引多分配存储：{allocations}"
    );
    assert_eq!(words(&session), ["甲", "乙"]);
}

#[test]
fn cached_plugin_rows_do_not_allocate_a_temporary_row_vector() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for count in [1, 2, crate::local::command::RESULT_LIMIT] {
        let mut session = plugin_session(&fixture, |_| {});
        let query = open_uuid(&mut session);
        let rows: Vec<_> = (0..count)
            .map(|index| (format!("合成行{index}"), String::new()))
            .collect();
        assert!(session.apply_plugin_command_rows(&query, &rows));
        let cached = session.input.plugin_rows.as_ref().unwrap().1.clone();

        for _ in 0..2 {
            let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                assert!(session.input.update_local_candidates().is_none());
            });
            // 缓存行的 `SharedString` 字段克隆只加引用计数，只剩输出向量一次申请。
            assert_eq!(
                allocations, 1,
                "{count} 行缓存插件候选重建产生了 {allocations} 次分配"
            );
            assert_eq!(session.input.local_candidates, cached);
            assert_eq!(session.input.plugin_rows.as_ref().unwrap().1, cached);
            assert!(session.plugin_command_query().is_none());
        }

        session.input.local_candidates[0].word.push('改');
        assert_eq!(session.input.plugin_rows.as_ref().unwrap().1, cached);
        session.input.update_local_candidates();
        assert_eq!(session.input.local_candidates, cached);
    }
}

#[test]
fn zero_plugin_rows_keep_the_fallback_and_are_not_asked_again() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |_| {});
    let query = open_uuid(&mut session);
    assert!(session.apply_plugin_command_rows(&query, &[]));
    assert_eq!(words(&session), ["/uuid"]);
    assert!(session.plugin_command_query().is_none());
    // 只剩不能用的行，也按 0 行缓存。
    session.command(Command::Cancel);
    let query = open_uuid(&mut session);
    assert!(session.apply_plugin_command_rows(&query, &plugin_rows(&[("", "")])));
    assert_eq!(words(&session), ["/uuid"]);
    assert!(session.plugin_command_query().is_none());
}

/// 方向键和列表重建不丢行、不再问；退一个字母再打回来是新的一份预编辑串，重新问（`/uuid` 删掉重打换一个 UUID）。
#[test]
fn plugin_rows_survive_caret_moves_and_rebuilds_but_not_an_edit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |_| {});
    let query = open_uuid(&mut session);
    assert!(session.apply_plugin_command_rows(&query, &plugin_rows(&[("甲", ""), ("乙", "")])));
    session.command(Command::MoveLeft);
    session.command(Command::MoveRight);
    assert_eq!(words(&session), ["甲", "乙"]);
    assert!(session.input.update_local_candidates().is_none());
    assert_eq!(words(&session), ["甲", "乙"]);
    assert!(session.plugin_command_query().is_none());

    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().preedit, "/uui");
    assert!(session.plugin_command_query().is_none());
    assert_eq!(words(&session), ["/uui"]);
    type_text(&mut session, "d");
    assert_eq!(words(&session), ["/uuid"]);
    let again = session.plugin_command_query().expect("asked again");
    assert_eq!(again, query);
    assert!(session.apply_plugin_command_rows(&again, &plugin_rows(&[("丙", "")])));
    assert_eq!(words(&session), ["丙"]);
}

#[test]
fn replacing_the_plugin_commands_or_the_command_table_drops_the_cached_rows() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |_| {});
    let query = open_uuid(&mut session);
    assert!(session.apply_plugin_command_rows(&query, &plugin_rows(&[("甲", "")])));
    assert_eq!(session.set_plugin_commands(&plugin_commands()), None);
    assert_eq!(words(&session), ["/uuid"]);
    let query = session.plugin_command_query().expect("asked again");
    assert!(session.apply_plugin_command_rows(&query, &plugin_rows(&[("甲", "")])));
    assert_eq!(session.set_command_table(&[]), None);
    assert_eq!(words(&session), ["/uuid"]);
    assert!(session.plugin_command_query().is_some());

    // 用户的指令表有 `uu`：插件的 `uuid` 不再生效。
    assert_eq!(
        session.set_command_table(&[crate::types::CommandTableEntry {
            trigger: "uu".to_owned(),
            title: "我的".to_owned(),
            template: "我的".to_owned(),
        }]),
        None
    );
    assert!(session.plugin_command_query().is_none());
    assert_eq!(session.set_command_table(&[]), None);
    // 授权撤销：插件指令清空之后 `/uuid` 什么都不问。
    assert_eq!(session.set_plugin_commands(&[]), None);
    assert!(session.plugin_command_query().is_none());
    assert_eq!(words(&session), ["/uuid"]);
}

#[test]
fn stale_plugin_command_answers_are_refused() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |_| {});
    let query = open_uuid(&mut session);
    let mut other_session = query.clone();
    other_session.session_id += 1;
    assert!(!session.apply_plugin_command_rows(&other_session, &plugin_rows(&[("甲", "")])));
    // 预编辑串变了。
    type_text(&mut session, "x");
    assert!(!session.apply_plugin_command_rows(&query, &plugin_rows(&[("甲", "")])));
    assert_eq!(words(&session), ["/uuidx"]);
    // 模式已经关了。
    session.command(Command::Cancel);
    assert!(!session.apply_plugin_command_rows(&query, &plugin_rows(&[("甲", "")])));
    assert!(session.snapshot().candidates.is_empty());
}

/// 不属于插件指令的 `/` 输入照旧：内置和用户的指令照常列出、带标题，从不要宿主去问插件。插件指令也不在 `/u` 这样的前缀里列出。
#[test]
fn other_commands_are_untouched_by_plugin_commands() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |_| {});
    session.character(b'/', false);
    assert!(session.plugin_command_query().is_none());
    type_text(&mut session, "u");
    assert_eq!(words(&session), ["/u"]);
    assert!(session.plugin_command_query().is_none());
    session.command(Command::Cancel);
    session.character(b'/', false);
    type_text(&mut session, "rq");
    assert!(session.plugin_command_query().is_none());
    assert_eq!(session.snapshot().candidate_annotations[0], "日期");
    assert!(candidate_sources(&session)
        .iter()
        .all(|source| *source == CandidateSource::Generated));
}

/// 选中插件行只上屏这一行的文字，不学习、不能置顶固定删除；开着个人上下文和词频调整时，选中前后用户日志的行数和主词库的字节都不变。指令模式的上屏不计打字统计。
///
/// 能从外面看出来的是调词频这条路：插件行要是被当成词库行（`is_dictionary`），选中会去调词频，而 `/` 模式没有拼音可作键，调词频失败、带回诊断，所以这里要求没有诊断。整句学习那条路看不出来：`can_learn_sentence_candidate` 在本地模式里一律拒绝，来源 14 误进 `is_sentence_learning` 也不会写任何东西，那条由 `types.rs` 的 `plugin_rows_match_no_predicate` 守着。
#[test]
fn selecting_a_plugin_row_commits_its_text_and_learns_nothing() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = plugin_session(&fixture, |options| {
        options.personal_context = true;
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
        options.frequency.trigger_count = 1;
    });
    type_text(&mut session, "nihao");
    assert_eq!(session.select(0).commit.as_deref(), Some("你好"));
    let journal_before = journal_rows(&fixture.journal());
    let dictionary_before = std::fs::read(fixture.main_db()).unwrap();

    let query = open_uuid(&mut session);
    assert!(session.apply_plugin_command_rows(&query, &plugin_rows(&[("甲", ""), ("乙", "")])));
    assert!(!session.pin(1).handled);
    assert!(!session.remove(1).handled);
    assert!(!session.fix_position(1, 1).handled);
    assert_eq!(words(&session), ["甲", "乙"]);
    let item = session.snapshot().candidates[1].clone();
    assert!(!session.input.is_editable_source(&item));

    // 选第二行，不是首选：开着 Promote、触发次数 1，词库行在这里会调词频。
    let result = session.select(1);
    assert_eq!(result.commit.as_deref(), Some("乙"));
    assert_eq!(result.diagnostic, None);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert!(session.plugin_command_query().is_none());
    assert_eq!(journal_rows(&fixture.journal()), journal_before);
    assert_eq!(std::fs::read(fixture.main_db()).unwrap(), dictionary_before);
    assert!(!crate::host::local_mode_counts_as_typing(
        LocalInputMode::Command.name()
    ));

    // 再进 `/uuid` 是一次新的组字：重新问插件。
    let query = open_uuid(&mut session);
    assert!(session.apply_plugin_command_rows(&query, &plugin_rows(&[("丁", "")])));
    assert_eq!(session.select(0).commit.as_deref(), Some("丁"));
}

// ---- Vietnamese ----

#[test]
fn vietnamese_vni_digits_are_spelling_symbols_only_while_a_word_composes() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Vietnamese;
        options.vietnamese_input_method = crate::vietnamese::InputMethod::Vni;
    });
    assert!(session.snapshot().spelling_symbols.is_empty());
    type_text(&mut session, "a");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.spelling_symbols, "0123456789");
    assert!(!snapshot.candidate_list_open);
    assert!(snapshot.candidates.is_empty());
    // A blur or scheme switch finishes through `finish`, which commits the word as displayed.
    session.character(b'1', false);
    assert_eq!(session.finish(0).commit.as_deref(), Some("á"));
    assert!(session.snapshot().spelling_symbols.is_empty());
}

#[test]
fn vietnamese_dedicated_english_keeps_its_own_rules() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Vietnamese);
    session.set_dedicated_english(true);
    type_text(&mut session, "hoaf");
    assert_eq!(session.snapshot().preedit, "hoaf");
    // The English composition is discarded at once: no raw-restore step in between.
    assert!(session.command(Command::Cancel).handled);
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn vietnamese_option_change_keeps_the_raw_key_display() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Vietnamese);
    type_text(&mut session, "hoaf");
    assert_eq!(session.snapshot().preedit, "hoà");
    assert!(session.command(Command::Cancel).handled);
    assert_eq!(session.snapshot().preedit, "hoaf");
    session.input.engine.set_vietnamese_options(
        crate::vietnamese::InputMethod::Telex,
        crate::vietnamese::ToneStyle::Classic,
    );
    assert_eq!(session.snapshot().preedit, "hoaf");
    // The second Esc still clears rather than taking the raw-restore step again.
    assert!(session.command(Command::Cancel).handled);
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn vietnamese_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Vietnamese);
    type_text(&mut session, "Tieens");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

// ---- 藏文 ----

fn tibetan_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| options.scheme = SchemeType::Tibetan)
}

#[test]
fn tibetan_space_slash_and_enter_end_the_syllables() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    let mut text = String::new();
    type_text(&mut session, "bkra");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "བཀྲ");
    assert_eq!(snapshot.editing_text, "བཀྲ");
    assert!(snapshot.candidates.is_empty());
    let space = session.character(b' ', false);
    assert!(space.handled);
    text.push_str(space.commit.as_deref().unwrap());
    type_text(&mut session, "shis");
    let slash = session.character(b'/', false);
    assert!(slash.handled);
    text.push_str(slash.commit.as_deref().unwrap());
    assert_eq!(text, "བཀྲ་ཤིས།");

    // 空格走 CommitCandidate 时同样带音节点；回车只上屏藏文，并吞掉按键。
    type_text(&mut session, "sangs");
    let space = session.command(Command::CommitCandidate);
    assert!(space.handled);
    assert_eq!(space.commit.as_deref(), Some("སངས་"));
    type_text(&mut session, "rgyas");
    let enter = session.command(Command::CommitRaw);
    assert!(enter.handled);
    assert_eq!(enter.commit.as_deref(), Some("རྒྱས"));

    // 没有组字时 `/` 单独输出垂符，空格和数字交给宿主。
    let slash = session.character(b'/', false);
    assert!(slash.handled);
    assert_eq!(slash.commit.as_deref(), Some("།"));
    let space = session.character(b' ', false);
    assert!(!space.handled && space.commit.is_none());
    let digit = session.character(b'1', false);
    assert!(!digit.handled && digit.commit.is_none());
}

#[test]
fn tibetan_uppercase_letters_and_wylie_symbols_spell() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    assert_eq!(session.snapshot().spelling_symbols, "'/");
    assert!(session.character(b'T', true).handled);
    assert_eq!(session.snapshot().spelling_symbols, "'+-./");
    type_text(&mut session, "a");
    assert_eq!(session.snapshot().preedit, "ཊ");
    session.command(Command::Backspace);
    session.command(Command::Backspace);
    assert!(session.snapshot().preedit.is_empty());

    // 以 achung 起头的音节：空闲时 `'` 也是拼写。
    type_text(&mut session, "'od");
    assert_eq!(session.snapshot().preedit, "འོད");
    session.command(Command::Cancel);
    session.command(Command::Cancel);

    // 宿主把拼写符号当标点送来时仍然拼写。
    type_text(&mut session, "pad");
    assert!(session.punctuation(b'+').handled);
    type_text(&mut session, "ma");
    assert_eq!(session.snapshot().preedit, "པདྨ");
    type_text(&mut session, "sh");
    session.command(Command::Backspace);
    session.command(Command::Backspace);
    assert_eq!(session.snapshot().preedit, "པདྨ");
    assert_eq!(session.punctuation(b'/').commit.as_deref(), Some("པདྨ།"));
    assert_eq!(session.punctuation(b'/').commit.as_deref(), Some("།"));
}

#[test]
fn tibetan_other_keys_commit_without_a_tsheg() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    type_text(&mut session, "ka");
    // 其他标点由标点路由在藏文后面跟上半角标点。
    assert!(!session.character(b',', false).handled);
    let comma = session.punctuation(b',');
    assert!(comma.handled);
    assert_eq!(comma.commit.as_deref(), Some("ཀ,"));
    // 数字和光标键上屏藏文，按键交回宿主。
    type_text(&mut session, "kha");
    let digit = session.character(b'2', false);
    assert!(!digit.handled);
    assert_eq!(digit.commit.as_deref(), Some("ཁ"));
    type_text(&mut session, "ga");
    let left = session.command(Command::MoveLeft);
    assert!(!left.handled);
    assert_eq!(left.commit.as_deref(), Some("ག"));
    // 失焦或切换方案经 `finish` 按显示上屏。
    type_text(&mut session, "nga");
    assert_eq!(session.finish(0).commit.as_deref(), Some("ང"));
}

#[test]
fn tibetan_shad_after_nga_keeps_the_tsheg() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    type_text(&mut session, "dang");
    assert_eq!(
        session.character(b'/', false).commit.as_deref(),
        Some("དང་།")
    );
    type_text(&mut session, "nga");
    assert_eq!(session.punctuation(b'/').commit.as_deref(), Some("ང་།"));
    // 不以 ང 结尾的音节直接接垂符。
    type_text(&mut session, "ngo");
    assert_eq!(session.character(b'/', false).commit.as_deref(), Some("ངོ།"));
}

#[test]
fn tibetan_letters_wylie_cannot_spell_are_written_as_typed() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    // 大写锁定下的 `BOD`：`B` `O` 原样上屏、不进原文，`D` 是威利的反写辅音，藏文上屏里没有拉丁字母。
    let b = session.character(b'B', true);
    assert!(b.handled);
    assert_eq!(b.commit.as_deref(), Some("B"));
    assert_eq!(session.character(b'O', true).commit.as_deref(), Some("O"));
    assert!(session.snapshot().preedit.is_empty());
    assert!(session.character(b'D', true).handled);
    assert_eq!(session.character(b' ', false).commit.as_deref(), Some("ཌ་"));
    // 有组字时，威利读不了的字母先上屏藏文，再原样跟在后面。
    type_text(&mut session, "ka");
    let q = session.character(b'q', false);
    assert!(q.handled);
    assert_eq!(q.commit.as_deref(), Some("ཀq"));
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn tibetan_first_esc_shows_the_wylie_and_the_second_cancels() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    type_text(&mut session, "bod");
    assert!(session.command(Command::Cancel).handled);
    assert_eq!(session.snapshot().preedit, "bod");
    // 锁定原文后组字只是拉丁字母：空格只上屏原文，把空格交回宿主。
    let space = session.character(b' ', false);
    assert!(!space.handled);
    assert_eq!(space.commit.as_deref(), Some("bod"));
    type_text(&mut session, "bod");
    session.command(Command::Cancel);
    assert_eq!(
        session.character(b'/', false).commit.as_deref(),
        Some("bod/")
    );
    type_text(&mut session, "bod");
    session.command(Command::Cancel);
    let cancelled = session.command(Command::Cancel);
    assert!(cancelled.handled && cancelled.commit.is_none());
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn tibetan_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    type_text(&mut session, "kSha");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn tibetan_dedicated_english_keeps_its_own_rules() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = tibetan_session(&fixture);
    session.set_dedicated_english(true);
    type_text(&mut session, "bod");
    assert_eq!(session.snapshot().preedit, "bod");
    assert!(session.snapshot().spelling_symbols.is_empty());
    assert!(session.command(Command::Cancel).handled);
    assert!(session.snapshot().preedit.is_empty());
}

/// A `msime-cantonese.db` with the rows the Cantonese session tests read, written with the shipped schema.
fn cantonese_dictionary(directory: &Path) -> PathBuf {
    use crate::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let path = directory.join("msime-cantonese.db");
    let connection = Connection::open(&path).expect("msime-cantonese.db");
    connection.execute_batch(SCHEMA).expect("cantonese schema");
    connection
        .execute(
            "INSERT INTO metadata VALUES (?1, ?2)",
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
        )
        .expect("cantonese metadata");
    connection
        .execute_batch(
            "INSERT INTO syllables VALUES ('nei'),('hou'),('ngo'),('ngoi'),('oi'),('i'),('ming');\
             INSERT INTO entries VALUES ('nei hou','你好',900),('nei hou','妳好',40),('nei','你',5000),('nei','妳',300),('hou','好',4000),('hou','號',500),('ngo','我',6000),('oi','愛',2500),('i','衣',1000),('ngoi','外',1000),('ming','名',1000);",
        )
        .expect("cantonese rows");
    path
}

/// Every table of a database with its row count, to show that nothing was written.
fn table_counts(path: &Path) -> Vec<(String, i64)> {
    if !path.exists() {
        return Vec::new();
    }
    let connection = Connection::open(path).expect("database");
    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get(0))
                .and_then(|rows| rows.collect())
        })
        .expect("tables");
    tables
        .into_iter()
        .map(|table| {
            let count = connection
                .query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |row| {
                    row.get(0)
                })
                .expect("count");
            (table, count)
        })
        .collect()
}

#[test]
fn cantonese_without_its_dictionary_is_unavailable() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let error = Session::new({
        let mut options = fixture.options();
        options.scheme = SchemeType::Cantonese;
        options
    })
    .err()
    .expect("no msime-cantonese.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );

    // Switching to it fails and leaves the scheme and the composition as they were.
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let error = session
        .switch_scheme(SchemeType::Cantonese)
        .expect_err("no msime-cantonese.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    assert_eq!(session.snapshot().scheme, SchemeType::Quanpin);
    assert_eq!(session.snapshot().preedit, "nihao");
    assert_eq!(words(&session)[0], "你好");

    // With the file in place the same switch goes through.
    let mut session = fixture.session_with(|options| {
        options.cantonese_dictionary = cantonese_dictionary(fixture.path());
    });
    type_text(&mut session, "nihao");
    session.switch_scheme(SchemeType::Cantonese).unwrap();
    assert_eq!(session.snapshot().scheme, SchemeType::Cantonese);
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn cantonese_double_pinyin_uses_the_existing_dictionary_and_spelling_symbols() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese.clone();
    });
    type_text(&mut session, "nzhb");
    assert_eq!(session.snapshot().editing_text, "nz hb");
    assert!(session.snapshot().spelling_symbols.contains(';'));
    assert_eq!(words(&session)[0], "你好");
    assert!(session.select(0).commit.is_some());
}

/// #10557：粤语双拼的整句候选一次上屏；关掉「双拼显示原始按键」时宿主画展开后的粤拼，编辑文字和原始快照仍是按键。
#[test]
fn cantonese_shuangpin_sentence_commits_whole_and_preedit_follows_the_raw_key_preference() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    for raw in [true, false] {
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Cantonese;
            options.cantonese_dictionary = cantonese.clone();
            options.force_cantonese_shuangpin = true;
            options.shuangpin_preedit_uses_raw = raw;
        });
        type_text(&mut session, "vonzhb");
        assert_eq!(words(&session)[0], "我你好");
        let display = session.display_snapshot();
        assert_eq!(
            display.preedit,
            if raw { "vonzhb" } else { "ngo nei hou" },
            "raw={raw}"
        );
        assert_eq!(display.editing_text, "vo nz hb");
        assert_eq!(session.snapshot().preedit, "vonzhb");
        // 末尾单独的声母键原样显示。
        type_text(&mut session, "n");
        assert_eq!(
            session.display_snapshot().preedit,
            if raw { "vonzhbn" } else { "ngo nei hou n" },
            "raw={raw}"
        );
        session.command(Command::Backspace);
        let result = session.select(0);
        assert_eq!(result.commit.as_deref(), Some("我你好"));
        assert!(session.snapshot().preedit.is_empty());
    }
}

#[test]
fn selected_cantonese_shuangpin_survives_scheme_switches() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    let mut plain = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese.clone();
    });
    type_text(&mut plain, "oi");
    assert_eq!(words(&plain)[0], "愛");

    let mut selected = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese;
        options.force_cantonese_shuangpin = true;
    });
    type_text(&mut selected, "oi");
    assert_eq!(words(&selected)[0], "衣");
    selected.switch_scheme(SchemeType::Quanpin).unwrap();
    selected.switch_scheme(SchemeType::Cantonese).unwrap();
    type_text(&mut selected, "oi");
    assert_eq!(words(&selected)[0], "衣");
}

#[test]
fn cantonese_double_pinyin_routes_semicolon_as_a_final_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese;
    });
    type_text(&mut session, "m;");
    assert_eq!(session.snapshot().editing_text, "m;");
    assert_eq!(words(&session)[0], "名");
}

#[test]
fn cantonese_rows_are_never_learned_or_edited() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let cantonese = cantonese_dictionary(fixture.path());
    let configure = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese.clone();
        options.learning = true;
        options.personal_context = true;
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
    };
    let mut session = fixture.session_with(configure);
    let journal = table_counts(&fixture.journal());
    let main = table_counts(&fixture.main_db());

    type_text(&mut session, "neihou");
    assert_eq!(session.snapshot().editing_text, "nei hou");
    assert_eq!(words(&session), ["你好", "妳好", "你", "妳"]);
    // Pins, removals and fixed slots would key these rows by Jyutping in the pinyin stores.
    assert!(!session.pin(1).handled);
    assert!(!session.remove(0).handled);
    assert!(!session.fix_position(1, 1).handled);
    assert_eq!(words(&session), ["你好", "妳好", "你", "妳"]);

    // A row covering the first syllable commits at once and the rest keeps composing.
    let result = session.select(2);
    assert_eq!(result.commit.as_deref(), Some("你"));
    assert_eq!(session.snapshot().editing_text, "hou");
    assert_eq!(words(&session), ["好", "號"]);
    let result = session.select(1);
    assert_eq!(result.commit.as_deref(), Some("號"));
    assert!(session.snapshot().preedit.is_empty());

    // Typing the same reading again finds the order unchanged.
    type_text(&mut session, "neihou");
    assert_eq!(words(&session), ["你好", "妳好", "你", "妳"]);
    assert_eq!(session.select(1).commit.as_deref(), Some("妳好"));
    type_text(&mut session, "nei'h");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("nei'h")
    );
    drop(session);
    crate::flush_personal_learning();

    assert_eq!(table_counts(&fixture.journal()), journal);
    assert_eq!(table_counts(&fixture.main_db()), main);
    let mut session = fixture.session_with(configure);
    type_text(&mut session, "neihou");
    assert_eq!(words(&session), ["你好", "妳好", "你", "妳"]);
}

#[test]
fn selecting_a_cantonese_candidate_does_not_clone_the_full_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese_dictionary(fixture.path());
    });
    type_text(&mut session, "neihou");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(0));

    assert_eq!(result.commit.as_deref(), Some("你好"));
    assert!(session.snapshot().preedit.is_empty());
    assert!(
        allocations <= 1,
        "Cantonese candidate selection allocations: {allocations}"
    );
}

#[test]
fn cantonese_caret_edits_keep_the_shown_syllables() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese_dictionary(fixture.path());
    });
    type_text(&mut session, "ngo'oi");
    assert_eq!(session.snapshot().editing_text, "ngo oi");
    assert_eq!(words(&session), ["我愛", "我"]);
    assert!(session.command(Command::MoveLeft).handled);
    assert!(session.command(Command::Backspace).handled);
    // `ngo i`, not the single syllable `ngoi` (外).
    assert_eq!(session.snapshot().editing_text, "ngo i");
    assert_eq!(words(&session), ["我衣", "我"]);
}

#[test]
fn cantonese_candidate_refresh_reuses_row_storage() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese_dictionary(fixture.path());
    });
    type_text(&mut session, "neihou");
    let word_pointer = session.input.engine.candidates()[0].word.as_ptr();

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.engine.handle_key(SchemeKey::Requery);
    });

    assert_eq!(allocations, 0);
    assert_eq!(
        session.input.engine.candidates()[0].word.as_ptr(),
        word_pointer
    );
}

#[test]
fn cantonese_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese_dictionary(fixture.path());
    });
    type_text(&mut session, "neihou");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    // 大写字母被粤拼忽略，但仍会走一次查询刷新。
    session.input.engine.handle_key(SchemeKey::Letter(b'A'));
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn cantonese_key_refresh_reuses_query_rows() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese_dictionary(fixture.path());
    });
    type_text(&mut session, "neihou");
    session.input.engine.handle_key(SchemeKey::Backspace);
    let word_pointer = session.input.engine.candidates()[0].word.as_ptr();

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.engine.handle_key(SchemeKey::Letter(b'u'));
    });

    assert!(allocations <= 35, "逐键查询不应重建候选行：{allocations}");
    assert_eq!(words(&session), ["你好", "妳好", "你", "妳"]);
    assert_eq!(
        session.input.engine.candidates()[0].word.as_ptr(),
        word_pointer
    );
}

#[test]
fn pinyin_candidate_refresh_reuses_preedit_storage() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let preedit_pointer = session.input.engine.preedit().as_ptr();

    session.input.engine.handle_key(SchemeKey::Requery);

    assert_eq!(session.input.engine.preedit().as_ptr(), preedit_pointer);
}

#[test]
fn pinyin_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn shuangpin_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::Shuangpin);
    type_text(&mut session, "nihc");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn shuangpin_double_helpcode_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.helpcode = true;
    });
    type_text(&mut session, "nihcAB");
    let request = session.input.engine.request();
    assert_eq!(request.raw_segmentation, "ni'hc'AB");
    assert_eq!(request.normalized_segmentation, "ni'hao'AB");
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];

    session.input.engine.handle_key(SchemeKey::Requery);

    let request = session.input.engine.request();
    assert_eq!(request.raw_segmentation, "ni'hc'AB");
    assert_eq!(request.normalized_segmentation, "ni'hao'AB");
    assert_eq!(request.segmentation, "ni'hao'AB");
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

/// 合成的 `msime-stroke.db`（`stroke::fixture`），用共享的 schema 写成。
fn stroke_dictionary(directory: &Path) -> PathBuf {
    let path = directory.join("msime-stroke.db");
    crate::stroke::fixture::build(&path);
    path
}

fn stroke_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| {
        options.scheme = SchemeType::Stroke;
        options.stroke_dictionary = stroke_dictionary(fixture.path());
    })
}

#[test]
fn stroke_without_its_dictionary_is_unavailable() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let error = Session::new({
        let mut options = fixture.options();
        options.scheme = SchemeType::Stroke;
        options
    })
    .err()
    .expect("no msime-stroke.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );

    // 切换失败时方案和组合保持原样。
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let error = session
        .switch_scheme(SchemeType::Stroke)
        .expect_err("no msime-stroke.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );
    assert_eq!(session.snapshot().scheme, SchemeType::Quanpin);
    assert_eq!(session.snapshot().preedit, "nihao");

    // 文件在位时切换成功。
    let mut session = fixture.session_with(|options| {
        options.stroke_dictionary = stroke_dictionary(fixture.path());
    });
    type_text(&mut session, "nihao");
    session.switch_scheme(SchemeType::Stroke).unwrap();
    assert_eq!(session.snapshot().scheme, SchemeType::Stroke);
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn stroke_keeps_its_dictionary_open_across_scheme_switches() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = stroke_session(&fixture);
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    // 文件没了，但激活时打开的连接仍归这个会话。
    std::fs::remove_file(fixture.path().join("msime-stroke.db")).unwrap();
    session.switch_scheme(SchemeType::Stroke).unwrap();
    type_text(&mut session, "pn");
    assert_eq!(words(&session), ["人"]);
}

#[test]
fn stroke_composes_glyphs_from_its_keys() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = stroke_session(&fixture);
    // 空组合时通配符、其他字母和大写字母都交还宿主。
    for key in *b"xaH1 " {
        assert!(!session.character(key, false).handled, "{}", key as char);
        assert!(session.snapshot().preedit.is_empty());
    }
    assert_eq!(session.snapshot().spelling_symbols, "");

    type_text(&mut session, "hs");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "一丨");
    assert_eq!(snapshot.normalized_segmentation, "一丨");
    assert_eq!(snapshot.editing_text, "hs");
    assert_eq!(snapshot.caret_position, 2);
    assert_eq!(snapshot.spelling_symbols, "");
    assert_eq!(words(&session), ["十", "土"]);
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .all(|item| item.scheme == SchemeType::Stroke && item.pinyin == "hs"));

    // 组合中其他字母被吞掉，组合不变；通配符追加一笔。
    for key in *b"aqH" {
        assert!(session.character(key, false).handled, "{}", key as char);
        assert_eq!(session.snapshot().editing_text, "hs");
    }
    assert!(session.character(b'x', false).handled);
    assert_eq!(session.snapshot().preedit, "一丨＊");
    assert_eq!(words(&session), ["土"]);

    // Backspace 删最后一笔，Esc 清空组合。
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().preedit, "一丨");
    let result = session.command(Command::Cancel);
    assert!(result.handled);
    assert!(result.commit.is_none());
    assert!(session.snapshot().preedit.is_empty());
    assert!(session.snapshot().candidates.is_empty());
}

#[test]
fn stroke_candidate_refresh_reuses_row_storage() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = stroke_session(&fixture);
    type_text(&mut session, "hs");
    let before = session.input.engine.candidates().to_vec();
    session.input.engine.handle_key(SchemeKey::Requery);
    // 会话的混排候选经 `clone_candidate_rows` 与引擎候选共用 `SharedString` 存储。重查时笔画对 `pinyin` 的 `clear()` 只是放手、随后 `push_str` 新建存储，对 `canonical_pinyin`、`word` 的 `clone_from` 经 `DerefMut` 先复制共享的那份，原缓冲区都不会原地复用，指针必然变化（混排那份也不再另行复制）；这里只能确认重查后内容不变。
    assert_eq!(session.input.engine.candidates(), before);
}

#[test]
fn stroke_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = stroke_session(&fixture);
    type_text(&mut session, "hs");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn stroke_rows_are_never_learned_or_edited() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let stroke = stroke_dictionary(fixture.path());
    let configure = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Stroke;
        options.stroke_dictionary = stroke.clone();
        options.learning = true;
        options.personal_context = true;
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
    };
    let mut session = fixture.session_with(configure);
    let journal = table_counts(&fixture.journal());
    let main = table_counts(&fixture.main_db());
    let order = ["一", "大", "二", "十", "三", "王", "土", "干"];

    type_text(&mut session, "h");
    assert_eq!(words(&session), order);
    // 置顶、删除、固定位置都会把这些行按笔画字母写进拼音用户词典。
    assert!(!session.pin(1).handled);
    assert!(!session.remove(0).handled);
    assert!(!session.fix_position(1, 1).handled);
    assert_eq!(words(&session), order);

    // 选中任一字都结束整个组合。
    let result = session.select(1);
    assert_eq!(result.commit.as_deref(), Some("大"));
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "h");
    assert_eq!(words(&session), order);
    // Space 选高亮候选。
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("一")
    );
    // Enter 上屏键入的字母串，不是字形。
    type_text(&mut session, "hsx");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("hsx")
    );
    assert!(session.snapshot().preedit.is_empty());
    // 没有候选时 Space 也上屏字母串。
    type_text(&mut session, "zzzz");
    assert!(words(&session).is_empty());
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("zzzz")
    );
    // 标点先上屏首选再跟标点。
    type_text(&mut session, "pn");
    assert_eq!(session.punctuation(b',').commit.as_deref(), Some("人，"));
    // 数字键选词。
    type_text(&mut session, "hs");
    assert_eq!(session.candidate_key(b'2').commit.as_deref(), Some("土"));
    drop(session);
    crate::flush_personal_learning();

    assert_eq!(table_counts(&fixture.journal()), journal);
    assert_eq!(table_counts(&fixture.main_db()), main);
    let mut session = fixture.session_with(configure);
    type_text(&mut session, "h");
    assert_eq!(words(&session), order);
}

#[test]
fn selecting_a_stroke_candidate_does_not_clone_the_full_row() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = stroke_session(&fixture);
    type_text(&mut session, "h");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.select(1));

    assert_eq!(result.commit.as_deref(), Some("大"));
    assert!(session.snapshot().preedit.is_empty());
    assert!(
        allocations <= 1,
        "stroke candidate selection allocations: {allocations}"
    );
}

#[test]
fn stroke_caret_edits_take_only_strokes() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = stroke_session(&fixture);
    type_text(&mut session, "hh");
    assert!(session.segment_raw_boundaries().is_empty());
    assert!(session.command(Command::MoveLeft).handled);
    assert_eq!(session.snapshot().caret_position, 1);
    // 光标处插入一笔。
    assert!(session.character(b's', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "hsh");
    assert_eq!(snapshot.preedit, "一丨一");
    assert_eq!(snapshot.caret_position, 2);
    // 其他字母被吞掉，组合不变。
    assert!(session.character(b'a', false).handled);
    assert_eq!(session.snapshot().editing_text, "hsh");
    // 通配符不能插在最前面，插在中间可以。
    assert!(session.command(Command::MoveHome).handled);
    assert!(session.character(b'x', false).handled);
    assert_eq!(session.snapshot().editing_text, "hsh");
    assert!(session.command(Command::MoveRight).handled);
    assert!(session.character(b'x', false).handled);
    assert_eq!(session.snapshot().editing_text, "hxsh");
    assert_eq!(session.snapshot().preedit, "一＊丨一");
    // Backspace 删光标前的一笔。
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().editing_text, "hsh");
    assert_eq!(words(&session), ["土"]);
}

#[test]
fn stroke_caret_insert_stops_at_the_stroke_limit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = stroke_session(&fixture);
    let full = format!("{}z", "h".repeat(crate::stroke::MAX_STROKES - 1));
    type_text(&mut session, &full);
    assert_eq!(session.snapshot().editing_text, full);
    assert!(session.command(Command::MoveHome).handled);
    // 已满时在中间插入一笔被吞掉，不能挤掉末尾的「折」。
    assert!(session.character(b's', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, full);
    assert_eq!(snapshot.caret_position, 0);
    // 删掉一笔后又能插入。
    assert!(session.command(Command::DeleteForward).handled);
    assert!(session.character(b's', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text.len(), crate::stroke::MAX_STROKES);
    assert!(snapshot.editing_text.starts_with('s') && snapshot.editing_text.ends_with('z'));
}

/// A `msime-zhuyin.db` with the rows the Zhuyin session tests read, written with the shipped schema.
fn zhuyin_dictionary(directory: &Path) -> PathBuf {
    use crate::language_dictionary::{FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA};
    let path = directory.join("msime-zhuyin.db");
    let connection = Connection::open(&path).expect("msime-zhuyin.db");
    connection.execute_batch(SCHEMA).expect("zhuyin schema");
    connection
        .execute(
            "INSERT INTO metadata VALUES (?1, ?2)",
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
        )
        .expect("zhuyin metadata");
    connection
        .execute_batch(
            "INSERT INTO syllables VALUES ('ㄋㄧˇ'),('ㄌㄧˇ'),('ㄏㄠˇ'),('ㄊㄞˊ'),('ㄨㄢ');\
             INSERT INTO entries VALUES ('ㄋㄧˇ','你',1000),('ㄋㄧˇ','妳',300),('ㄌㄧˇ','李',1200),('ㄏㄠˇ','好',2000),('ㄏㄠˇ','郝',10),('ㄋㄧˇ ㄏㄠˇ','你好',500),('ㄊㄞˊ','台',900),('ㄊㄞˊ','臺',400),('ㄨㄢ','彎',500),('ㄨㄢ','灣',300),('ㄊㄞˊ ㄨㄢ','臺灣',800),('ㄊㄞˊ ㄨㄢ','台灣',600);",
        )
        .expect("zhuyin rows");
    path
}

fn zhuyin_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| {
        options.scheme = SchemeType::Zhuyin;
        options.zhuyin_dictionary = zhuyin_dictionary(fixture.path());
    })
}

// 注音九键：数字进注音编辑器而不是拼音九宫格，候选读音经 `nine_key_spellings` 给宿主，`choose_nine_key_spelling` 钉读音。
#[test]
fn zhuyin_nine_key_digits_go_to_the_bopomofo_editor() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = zhuyin_session(&fixture);
    session.set_nine_key_enabled(true);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.spelling_symbols, "1234567890");
    assert!(!session.character(b' ', false).handled);

    assert!(session.character(b'2', false).handled);
    assert!(!session.nine_key.active());
    let snapshot = session.snapshot();
    assert_eq!(snapshot.scheme, SchemeType::Zhuyin);
    assert_eq!(snapshot.preedit, "2");
    assert_eq!(snapshot.spelling_symbols, "1234567890 ");
    // 候选键路线上的数字同样是拼写。
    assert!(session.candidate_key(b'8').handled);
    type_text(&mut session, "c39c");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "你好");
    assert_eq!(snapshot.editing_text, "28c39c");
    assert_eq!(snapshot.nine_key_spellings, ["ㄋㄧˇ", "ㄌㄧˇ"]);
    assert!(snapshot.candidates.is_empty());

    assert!(!session.choose_nine_key_spelling(2).handled);
    let chosen = session.choose_nine_key_spelling(1);
    assert!(chosen.handled && chosen.commit.is_none());
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "李好");
    assert!(snapshot.nine_key_spellings.is_empty());

    // 列表打开时没有候选读音。
    assert!(session.command(Command::ConvertHanja).handled);
    let snapshot = session.snapshot();
    assert!(snapshot.candidate_list_open);
    assert_eq!(snapshot.spelling_symbols, "1234567890");
    assert!(snapshot.nine_key_spellings.is_empty());
    session.command(Command::ConvertHanja);

    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("李好")
    );
    // ， 和 。 不是九键的拼写符号：走普通标点路线，先提交转换文字再上屏标点；? 由编辑器的 Shift 标点认领。
    type_text(&mut session, "28c");
    let comma = session.punctuation(b',');
    assert!(comma.handled);
    assert_eq!(comma.commit.as_deref(), Some("李，"));
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "28c");
    let question = session.punctuation(b'?');
    assert!(question.handled);
    assert_eq!(question.commit.as_deref(), Some("李？"));
    let period = session.punctuation(b'.');
    assert!(period.handled);
    assert_eq!(period.commit.as_deref(), Some("。"));
    // 关掉九键后数字又是大千键：2 是 ㄉ。
    session.set_nine_key_enabled(false);
    assert!(session.character(b'2', false).handled);
    assert_eq!(session.snapshot().preedit, "ㄉ");
    assert!(session.snapshot().nine_key_spellings.is_empty());
}

#[test]
fn zhuyin_nine_key_mode_follows_scheme_switches() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.zhuyin_dictionary = zhuyin_dictionary(fixture.path());
    });
    session.set_nine_key_enabled(true);
    // 全拼九键不受影响：数字仍进拼音九宫格。
    assert!(session.character(b'6', false).handled);
    assert!(session.nine_key.active());
    assert_eq!(session.snapshot().scheme, SchemeType::Quanpin);
    session.command(Command::Cancel);
    assert!(!session.nine_key.active());
    // 切到注音时九键模式带过去，切回全拼再切回来也一样。
    session.switch_scheme(SchemeType::Zhuyin).unwrap();
    type_text(&mut session, "28");
    assert!(!session.nine_key.active());
    assert_eq!(session.snapshot().preedit, "28");
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    session.switch_scheme(SchemeType::Zhuyin).unwrap();
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "28c");
    assert_eq!(session.snapshot().preedit, "李");
    assert_eq!(session.snapshot().nine_key_spellings, ["ㄌㄧˇ", "ㄋㄧˇ"]);
    // 正在组字时关掉九键：注音组字被丢掉，什么都不提交。
    session.set_nine_key_enabled(false);
    let snapshot = session.snapshot();
    assert!(snapshot.preedit.is_empty());
    assert!(snapshot.nine_key_spellings.is_empty());
    // 九键关着时 choose_nine_key_spelling 在注音里什么都不做。
    type_text(&mut session, "su3");
    assert!(!session.choose_nine_key_spelling(0).handled);
    assert_eq!(session.snapshot().preedit, "你");
}

#[test]
fn zhuyin_keeps_its_dictionary_open_across_scheme_switches() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = zhuyin_session(&fixture);
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    // The file is gone, but the connection opened at activation is still the session's.
    std::fs::remove_file(fixture.path().join("msime-zhuyin.db")).unwrap();
    session.switch_scheme(SchemeType::Zhuyin).unwrap();
    session.switch_scheme(SchemeType::Zhuyin).unwrap();
    type_text(&mut session, "su3");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("你")
    );
}

#[test]
fn zhuyin_without_its_dictionary_is_unavailable() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let error = Session::new({
        let mut options = fixture.options();
        options.scheme = SchemeType::Zhuyin;
        options
    })
    .err()
    .expect("no msime-zhuyin.db");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
    );

    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    assert!(session.switch_scheme(SchemeType::Zhuyin).is_err());
    assert_eq!(session.snapshot().scheme, SchemeType::Quanpin);
    assert_eq!(session.snapshot().preedit, "nihao");

    // With the file in place the switch goes through, and switching away and back opens it again.
    let mut session = fixture.session_with(|options| {
        options.zhuyin_dictionary = zhuyin_dictionary(fixture.path());
    });
    session.switch_scheme(SchemeType::Zhuyin).unwrap();
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    session.switch_scheme(SchemeType::Zhuyin).unwrap();
    type_text(&mut session, "su3cl3");
    assert_eq!(session.snapshot().preedit, "你好");
}

#[test]
fn zhuyin_composes_and_reports_its_spelling_symbols() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = zhuyin_session(&fixture);
    // Idle, the tone digits and Space type themselves.
    assert_eq!(session.snapshot().spelling_symbols, "125890,./;-");
    assert!(!session.character(b'3', false).handled);
    assert!(!session.character(b' ', false).handled);

    type_text(&mut session, "su3cl");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.preedit, "你ㄏㄠ");
    assert_eq!(snapshot.editing_text, "su3cl");
    assert_eq!(snapshot.caret_position, 5);
    assert_eq!(snapshot.spelling_symbols, "1234567890,./;- ");
    // Nothing is listed until the user opens the list.
    assert!(snapshot.candidates.is_empty());
    assert!(!snapshot.candidate_list_open);
    // A tone digit on the candidate-key route spells.
    assert!(session.candidate_key(b'3').handled);
    assert_eq!(session.snapshot().preedit, "你好");
    // The caret stays at the end.
    assert!(session.command(Command::MoveLeft).commit.is_some());
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn zhuyin_candidate_refresh_reuses_row_storage() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = zhuyin_session(&fixture);
    type_text(&mut session, "su3cl3");
    assert!(session.command(Command::ConvertHanja).handled);

    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        session.input.engine.handle_key(SchemeKey::Requery);
    });

    assert!(
        allocations <= 6,
        "requery should not recreate Zhuyin candidate rows: {allocations} allocations"
    );
}

#[test]
fn zhuyin_refresh_reuses_request_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = zhuyin_session(&fixture);
    type_text(&mut session, "su3cl3");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

#[test]
fn zhuyin_list_selection_pins_without_committing_or_learning() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Zhuyin;
        options.zhuyin_dictionary = zhuyin_dictionary(fixture.path());
        options.learning = true;
        options.personal_context = true;
    });
    let journal = table_counts(&fixture.journal());
    let main = table_counts(&fixture.main_db());

    type_text(&mut session, "su3cl3");
    assert!(session.command(Command::ConvertHanja).handled);
    let snapshot = session.snapshot();
    assert!(snapshot.candidate_list_open);
    assert_eq!(snapshot.spelling_symbols, "0,./;-");
    assert_eq!(words(&session), ["你好", "好", "郝"]);
    // Rows come from the read-only msime-zhuyin.db: none can be pinned, removed or fixed.
    assert!(!session.pin(1).handled);
    assert!(!session.remove(0).handled);
    assert!(!session.fix_position(1, 1).handled);

    // A selection digit with the list open is left for the runtime to select with.
    let digit = session.character(b'3', false);
    assert!(!digit.handled && digit.commit.is_none());
    let space = session.character(b' ', false);
    assert!(!space.handled && space.commit.is_none());

    let selected = session.select(2);
    assert!(selected.handled);
    assert_eq!(selected.commit, None);
    let snapshot = session.snapshot();
    assert!(!snapshot.candidate_list_open);
    assert!(snapshot.candidates.is_empty());
    assert_eq!(snapshot.preedit, "你郝");

    // The candidate-key route selects too, and the edge route chooses rather than commits.
    session.command(Command::ConvertHanja);
    assert!(session.candidate_key(b'2').handled);
    assert_eq!(session.snapshot().preedit, "你好");
    session.command(Command::ConvertHanja);
    let edge = session.select_edge(2, CandidateEdge::FirstHan);
    assert!(edge.handled && edge.commit.is_none());
    assert_eq!(session.snapshot().preedit, "你郝");
    // With the list open CommitCandidate chooses the first row.
    session.command(Command::ConvertHanja);
    let first = session.command(Command::CommitCandidate);
    assert!(first.handled && first.commit.is_none());
    assert_eq!(session.snapshot().preedit, "你好");

    // With the list closed it commits the converted text.
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("你好")
    );
    assert!(session.snapshot().preedit.is_empty());
    drop(session);
    crate::flush_personal_learning();
    assert_eq!(table_counts(&fixture.journal()), journal);
    assert_eq!(table_counts(&fixture.main_db()), main);
}

#[test]
fn zhuyin_cancel_and_backspace_close_the_list_first() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = zhuyin_session(&fixture);
    type_text(&mut session, "w96j0 ");
    assert_eq!(session.snapshot().preedit, "臺灣");
    session.command(Command::ConvertHanja);
    assert_eq!(words(&session), ["臺灣", "台灣", "彎", "灣"]);
    assert!(session.command(Command::Cancel).handled);
    assert!(!session.snapshot().candidate_list_open);
    assert_eq!(session.snapshot().preedit, "臺灣");
    session.command(Command::ConvertHanja);
    assert!(session.command(Command::Backspace).handled);
    assert!(!session.snapshot().candidate_list_open);
    assert_eq!(session.snapshot().preedit, "臺灣");
    // A phonetic key closes the list and keeps composing.
    session.command(Command::ConvertHanja);
    assert!(session.character(b's', false).handled);
    assert!(!session.snapshot().candidate_list_open);
    assert_eq!(session.snapshot().preedit, "臺灣ㄋ");
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().preedit, "臺灣");
    assert!(session.command(Command::Cancel).handled);
    assert!(session.snapshot().preedit.is_empty());
    assert!(!session.command(Command::Cancel).handled);
}

#[test]
fn zhuyin_enter_shift_punctuation_and_other_keys_commit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = zhuyin_session(&fixture);
    // Enter commits the conversion and drops the pending syllable.
    type_text(&mut session, "su3c");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("你")
    );
    assert!(session.snapshot().preedit.is_empty());

    // A Shift punctuation key commits the text with its full-width mark on either route, even idle.
    type_text(&mut session, "su3");
    let marked = session.punctuation(b'<');
    assert!(marked.handled);
    assert_eq!(marked.commit.as_deref(), Some("你，"));
    let idle = session.character(b'?', false);
    assert!(idle.handled);
    assert_eq!(idle.commit.as_deref(), Some("？"));

    // A phonetic punctuation key spells on the punctuation route.
    assert!(session.punctuation(b',').handled);
    assert_eq!(session.snapshot().preedit, "ㄝ");
    session.command(Command::Cancel);

    // Other ASCII punctuation finishes the composition ahead of its mark.
    type_text(&mut session, "su3");
    let finished = session.punctuation(b'!');
    assert!(finished.handled);
    assert_eq!(finished.commit.as_deref(), Some("你！"));
    assert!(session.snapshot().preedit.is_empty());

    // With Chinese punctuation off the Shift overlay still writes its full-width mark, while other ASCII punctuation is left to the host.
    session.set_chinese_punctuation_enabled(false);
    let overlay = session.punctuation(b'<');
    assert!(overlay.handled);
    assert_eq!(overlay.commit.as_deref(), Some("，"));
    let plain = session.punctuation(b'!');
    assert!(!plain.handled);
    assert_eq!(plain.commit, None);
    session.set_chinese_punctuation_enabled(true);

    // A key the editor does not claim commits the text and goes to the host.
    type_text(&mut session, "su3");
    let capital = session.character(b'A', true);
    assert!(!capital.handled);
    assert_eq!(capital.commit.as_deref(), Some("你"));
    assert!(session.snapshot().preedit.is_empty());
}

/// K 模式先列数据库里的短语，再接上宿主短语表里编码匹配的行；文本重复的不再列出。短语表可以在 K 模式打开时实时替换。
#[test]
fn quick_phrase_mode_appends_the_host_table_after_the_database_rows() {
    let fixture = Fixture::new(
        "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('dh','电话',10);",
    );
    let phrase = |key: &str, text: &str| crate::types::QuickPhraseEntry {
        key: key.into(),
        text: text.into(),
    };
    let mut session = fixture.session_with(|options| {
        options.quick_phrase_table = vec![phrase("dh", "电话"), phrase("dhhm", "电话号码")];
    });
    assert!(session.character(b'K', true).handled);
    type_text(&mut session, "dh");
    assert_eq!(words(&session), ["电话", "电话号码"]);
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .all(|row| row.source == CandidateSource::QuickPhrase));

    assert_eq!(
        session.set_quick_phrase_table(&[phrase("dhh", "大户号")]),
        None
    );
    assert_eq!(words(&session), ["电话", "大户号"]);
    assert_eq!(session.set_quick_phrase_table(&[]), None);
    assert_eq!(words(&session), ["电话"]);
}

/// macOS 的普通组字把编码匹配的快捷短语追加到原有候选之后，选中后直接上屏。
#[test]
fn macos_quick_phrase_candidates_join_regular_composition() {
    let fixture = Fixture::new(
        "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('dh','电话',20);",
    );
    let mut session = fixture.session_with(|options| {
        options.quick_phrase_candidates = true;
    });
    type_text(&mut session, "dh");
    assert!(words(&session).contains(&"电话".to_owned()));
    let phrase = session
        .snapshot()
        .candidates
        .iter()
        .position(|item| item.word == "电话")
        .expect("quick phrase candidate");
    assert_eq!(session.select(phrase).commit.as_deref(), Some("电话"));
}

/// 普通组字只认整个编码：K 模式按前缀列表，普通候选若也按前缀，单个字母就会带出以它开头的所有短语。数据库行和宿主短语表都一样。
#[test]
fn regular_composition_matches_quick_phrase_keys_exactly() {
    let fixture = Fixture::new(
        "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('dh','电话',20);\
INSERT INTO quick_parases VALUES('dhx','电话线',30);",
    );
    let phrase = |key: &str, text: &str| crate::types::QuickPhraseEntry {
        key: key.into(),
        text: text.into(),
    };
    let mut session = fixture.session_with(|options| {
        options.quick_phrase_candidates = true;
        options.quick_phrase_table = vec![phrase("d", "地址"), phrase("dhh", "电话号码")];
    });
    type_text(&mut session, "dh");
    let regular = words(&session);
    assert!(regular.contains(&"电话".to_owned()), "{regular:?}");
    for prefixed in ["电话线", "电话号码", "地址"] {
        assert!(!regular.contains(&prefixed.to_owned()), "{regular:?}");
    }
    session.command(Command::Cancel);

    type_text(&mut session, "d");
    let single = words(&session);
    assert!(single.contains(&"地址".to_owned()), "{single:?}");
    assert!(!single.contains(&"电话".to_owned()), "{single:?}");
}

/// 设置页的「快捷短语」开关同时管两种入口：关掉后普通候选里也不出现短语。
#[test]
fn the_quick_phrase_switch_also_keeps_phrases_out_of_regular_candidates() {
    let fixture = Fixture::new(
        "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('dh','电话',20);",
    );
    let mut session = fixture.session_with(|options| {
        options.quick_phrase_candidates = true;
        options.local_modes.quick_phrase = false;
    });
    type_text(&mut session, "dh");
    assert!(!words(&session).contains(&"电话".to_owned()));
}

/// `quick_phrase_frequency` 开着（`mixed_input.quick_phrase_frequency`，Windows `utility.quick_phrase_frequency`）：选中组内快捷短语让整组按调频规则前移，越过它选普通候选让整组后退一格。
#[test]
fn quick_phrase_frequency_moves_the_group_on_selection() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 200);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '拟好', 100);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '逆好', 90);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '泥号', 80);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '溺号', 70);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '腻号', 60);\
CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('nihao','号码一',10);\
INSERT INTO quick_parases VALUES('nihao','号码二',20);",
    );
    let mut session = fixture.session_with(|options| {
        options.quick_phrase_candidates = true;
        options.quick_phrase_frequency = true;
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
        options.first_page_rows = 10;
        options.personal_context = false;
    });
    let group_at = |session: &Session| {
        session
            .snapshot()
            .candidates
            .iter()
            .position(|item| item.word == "号码一")
            .expect("quick phrase group")
    };
    type_text(&mut session, "nihao");
    let before = group_at(&session);
    assert_eq!(
        before, 7,
        "普通候选不满一页，整组接在它们之后（组内按权重排序）"
    );

    // 选中组内一条：整组按调频规则前移（Promote 把组的位置拉到第四格），选中的这条升到同码最高权重、排到组首。
    assert_eq!(session.select(before).commit.as_deref(), Some("号码一"));
    type_text(&mut session, "nihao");
    let promoted = group_at(&session);
    assert_eq!(promoted, 4, "整组前移后落在第五格");
    assert_eq!(
        words(&session)[promoted..promoted + 2],
        ["号码一", "号码二"],
        "整组一起挪"
    );

    // 越过整组选排在后面的普通候选：整组后退一格。
    session.select(7);
    type_text(&mut session, "nihao");
    assert_eq!(group_at(&session), 5, "越过组选普通候选后整组后退一格");

    // 再越过一次：组退到末尾，再往后没有普通候选可越。
    session.select(7);
    type_text(&mut session, "nihao");
    assert_eq!(group_at(&session), 6, "整组退到末尾");
    assert_eq!(words(&session)[6..], ["号码一", "号码二"]);
}

/// `quick_phrase_frequency` 关着时，整组固定在普通候选末尾：选中短语、越过它选普通候选，位置都不变。
#[test]
fn the_group_stays_at_the_end_when_quick_phrase_frequency_is_off() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 200);\
INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '拟好', 100);\
CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('nihao','号码一',10);",
    );
    let mut session = fixture.session_with(|options| {
        options.quick_phrase_candidates = true;
        options.quick_phrase_frequency = false;
        options.frequency = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
        options.personal_context = false;
    });
    let group_at = |session: &Session| {
        session
            .snapshot()
            .candidates
            .iter()
            .position(|item| item.word == "号码一")
            .expect("quick phrase group")
    };
    type_text(&mut session, "nihao");
    let position = group_at(&session);
    assert_eq!(session.select(position).commit.as_deref(), Some("号码一"));
    type_text(&mut session, "nihao");
    assert_eq!(group_at(&session), position, "关掉调频后选中短语不挪组");
    let last = session.snapshot().candidates.len() - 1;
    session.select(last);
    type_text(&mut session, "nihao");
    assert_eq!(group_at(&session), position, "关掉调频后越过组不挪组");
}

/// 小鹤双拼 `zj`（zan）的合成词库：`ordinary` 个单字读 zan，权重递减，第一个是 U+4E00；再加 `phrases` 里编码为 `zj` 的快捷短语（权重递减，组内按这个顺序排）。#9702 实测这个码有 83 个普通候选。
fn zan_fixture(ordinary: usize, phrases: &[&str]) -> Fixture {
    let mut sql = String::from("CREATE TABLE tbl_1_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);");
    for index in 0..ordinary {
        let word = char::from_u32(0x4E00 + index as u32).unwrap();
        sql.push_str(&format!(
            "INSERT INTO tbl_1_z VALUES('zan','z','{word}',{});",
            10_000 - index
        ));
    }
    sql.push_str("CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);");
    for (index, phrase) in phrases.iter().enumerate() {
        sql.push_str(&format!(
            "INSERT INTO quick_parases VALUES('zj','{phrase}',{});",
            100 - index
        ));
    }
    Fixture::new(&sql)
}

fn zan_session(fixture: &Fixture, page: usize, frequency: FrequencyAdjustmentMode) -> Session {
    fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
        options.helpcode = false;
        options.quick_phrase_candidates = true;
        options.quick_phrase_frequency = true;
        options.frequency = FrequencyAdjustmentOptions {
            mode: frequency,
            trigger_count: 1,
            linear_step: 1,
        };
        options.first_page_rows = page;
        options.personal_context = false;
    })
}

/// 快捷短语组在列表里的起点。
fn quick_phrase_group_at(session: &Session) -> usize {
    session
        .snapshot()
        .candidates
        .iter()
        .position(|item| item.source == CandidateSource::QuickPhrase)
        .expect("quick phrase group")
}

/// #9702：普通候选有一页以上时，没有学习记录的快捷短语落在首页最后一格，而不是排到 83 个普通候选之后（每页 6 个时在第 14 页）。首选仍是普通候选，每页 5、6、9 个都一样；调频关着时也是这个默认位置。
#[test]
fn an_unlearned_quick_phrase_takes_the_last_slot_of_the_first_page() {
    let fixture = zan_fixture(83, &["「」"]);
    for frequency in [
        FrequencyAdjustmentMode::Promote,
        FrequencyAdjustmentMode::Disabled,
    ] {
        for page in [5, 6, 9] {
            let mut session = zan_session(&fixture, page, frequency);
            type_text(&mut session, "zj");
            let words = words(&session);
            assert_eq!(words.len(), 84, "{frequency:?} {page}: {words:?}");
            assert_eq!(
                quick_phrase_group_at(&session),
                page - 1,
                "{frequency:?} 每页 {page} 个"
            );
            assert_eq!(words[page - 1], "「」");
            assert_eq!(words[0], "一", "首选仍是普通候选");
            // 被挤下去的普通候选接在短语后面，相对顺序不变。
            let ordinary: Vec<&String> = words.iter().filter(|word| *word != "「」").collect();
            assert_eq!(ordinary.len(), 83);
            assert!(ordinary.iter().enumerate().all(|(index, word)| **word
                == char::from_u32(0x4E00 + index as u32).unwrap().to_string()));
        }
    }
}

/// 默认落位之后，学习规则照旧：选中短语整组前移（Promote 从首页末格拉到第 4 个普通候选之后，再选一次到第 3 个之后）；越过整组选后面的普通候选，整组最多退回首页末位，不会再退到首页之外。
#[test]
fn learning_still_moves_the_quick_phrase_group_from_its_default_slot() {
    let fixture = zan_fixture(83, &["「」"]);
    let mut session = zan_session(&fixture, 6, FrequencyAdjustmentMode::Promote);
    type_text(&mut session, "zj");
    assert_eq!(quick_phrase_group_at(&session), 5);

    assert_eq!(session.select(5).commit.as_deref(), Some("「」"));
    type_text(&mut session, "zj");
    assert_eq!(quick_phrase_group_at(&session), 4, "选中一次前移一格");
    assert_eq!(session.select(4).commit.as_deref(), Some("「」"));
    type_text(&mut session, "zj");
    assert_eq!(quick_phrase_group_at(&session), 3, "再选一次继续前移");

    // 越过整组选后面的普通候选：在首页末位时不再后退；前移过一格的组退回首页末位就停。
    let fixture = zan_fixture(83, &["「」"]);
    let mut session = zan_session(&fixture, 6, FrequencyAdjustmentMode::Promote);
    type_text(&mut session, "zj");
    session.select(20);
    type_text(&mut session, "zj");
    assert_eq!(quick_phrase_group_at(&session), 5, "默认位置已是首页末位");
    assert_eq!(session.select(5).commit.as_deref(), Some("「」"));
    type_text(&mut session, "zj");
    assert_eq!(quick_phrase_group_at(&session), 4);
    for _ in 0..2 {
        session.select(20);
        type_text(&mut session, "zj");
        assert_eq!(quick_phrase_group_at(&session), 5, "最多退回首页末位");
    }
}

/// 同码有多条短语：组内顺序不变、整组连在一起，首页最多让出一半（向下取整）的格子给它们，普通候选的前一半（向上取整）留在首页；放不下的接在下一页开头。每页只有 1 个时首页不让格子，整组紧跟首选。
#[test]
fn several_quick_phrases_share_the_tail_of_the_first_page() {
    let phrases = ["甲短语", "乙短语", "丙短语", "丁短语", "戊短语"];
    // (每页候选数, 短语条数, 组的起点)
    for (page, count, start) in [
        (6, 3, 3),
        (5, 3, 3),
        (9, 3, 6),
        (9, 5, 5),
        (6, 2, 4),
        (2, 3, 1),
        (1, 1, 1),
    ] {
        let fixture = zan_fixture(83, &phrases[..count]);
        let mut session = zan_session(&fixture, page, FrequencyAdjustmentMode::Disabled);
        type_text(&mut session, "zj");
        let words = words(&session);
        assert_eq!(
            words[start..start + count],
            phrases[..count],
            "每页 {page} 个、{count} 条短语: {words:?}"
        );
        assert_eq!(words[0], "一");
    }
}

/// 普通候选不满一页时短语照旧接在普通候选之后，不把任何普通候选挤到下一页。
#[test]
fn quick_phrases_follow_fewer_than_a_page_of_candidates() {
    let phrases = ["甲短语", "乙短语", "丙短语"];
    for (ordinary, count) in [(3, 1), (3, 3), (5, 3), (0, 1)] {
        let fixture = zan_fixture(ordinary, &phrases[..count]);
        let mut session = zan_session(&fixture, 6, FrequencyAdjustmentMode::Promote);
        type_text(&mut session, "zj");
        let words = words(&session);
        let start = words.len() - count;
        assert_eq!(
            words[start..],
            phrases[..count],
            "{ordinary} 个普通候选: {words:?}"
        );
        assert_eq!(quick_phrase_group_at(&session), start);
    }
}

/// K 模式只列快捷短语，不经过混输落位，每页候选数也不改变它的顺序。
#[test]
fn quick_phrase_mode_is_unaffected_by_the_first_page_default() {
    let fixture = zan_fixture(83, &["甲短语", "乙短语", "丙短语"]);
    for page in [1, 5, 6, 9] {
        let mut session = zan_session(&fixture, page, FrequencyAdjustmentMode::Promote);
        assert!(session.character(b'K', true).handled);
        type_text(&mut session, "zj");
        assert_eq!(
            words(&session),
            ["甲短语", "乙短语", "丙短语"],
            "每页 {page} 个"
        );
    }
}

/// 五笔四码唯一时，同编码的快捷短语追加在后面也不妨碍自动上屏，提交的仍是五笔那一行。
#[test]
fn a_quick_phrase_with_the_same_key_keeps_the_wubi_unique_four_code() {
    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('addr','莽原',100);\
CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('addr','示例路1号',10);",
    );
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = false;
        options.quick_phrase_candidates = true;
    });
    type_text(&mut session, "addr");
    assert_eq!(
        words(&session),
        vec!["莽原".to_owned(), "示例路1号".to_owned()]
    );
    assert!(session.snapshot().wubi_unique_four_code);
}

/// 宿主给的辅助码表替换 schema 对应的表，也可以在会话中途换掉。
#[test]
fn a_host_helpcode_table_replaces_the_schema_table() {
    let fixture = isolation_root("你", "甲");
    let table = |pairs: &[(&str, &str)]| {
        std::sync::Arc::new(crate::helpcode::HelpcodeKeymap::from_codes(
            pairs
                .iter()
                .map(|(character, code)| ((*character).to_owned(), (*code).to_owned()))
                .collect(),
        ))
    };
    // schema 的表里 你=aa、拟=cc，所以 niC 把「拟」排到前面。
    let mut plain = fixture.session();
    type_text(&mut plain, "niC");
    assert_eq!(words(&plain), ["拟", "你"]);

    let mut session = fixture.session_with(|options| {
        options.helpcode_table = Some(table(&[("你", "cc"), ("拟", "aa")]));
    });
    type_text(&mut session, "niC");
    assert_eq!(words(&session), ["你", "拟"]);
    session.command(Command::Cancel);

    session.set_helpcode_table(table(&[("你", "aa"), ("拟", "cc")]));
    type_text(&mut session, "niC");
    assert_eq!(words(&session), ["拟", "你"]);
}

/// 只允许五笔的会话（五笔版）：混拼照样从全拼 provider 拿到拼音行，和全部方案都允许时的列表一模一样。
#[test]
fn a_wubi_only_session_still_mixes_quanpin_rows() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let wubi_only = |options: &mut SessionOptions| {
        options.scheme = SchemeType::Wubi;
        options.enabled_schemes = SchemeSet::of(&[SchemeType::Wubi]);
        options.wubi.mixed_pinyin = true;
    };
    let mut narrowed = fixture.session_with(wubi_only);
    let mut full = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut narrowed, "gege");
    type_text(&mut full, "gege");
    let schemes = |session: &Session| -> Vec<(String, SchemeType)> {
        session
            .snapshot()
            .candidates
            .into_iter()
            .map(|item| (item.word.into_string(), item.scheme))
            .collect()
    };
    let narrowed_rows = schemes(&narrowed);
    assert_eq!(
        narrowed_rows.first(),
        Some(&("工".to_owned(), SchemeType::Wubi))
    );
    assert!(narrowed_rows.contains(&("哥哥".to_owned(), SchemeType::Quanpin)));
    assert_eq!(narrowed_rows, schemes(&full));

    // 只有拼音行的编码走拼音回退。
    narrowed.command(Command::Cancel);
    type_text(&mut narrowed, "gg");
    let snapshot = narrowed.snapshot();
    assert!(snapshot.answered_by_pinyin_fallback, "{snapshot:?}");
    assert_eq!(snapshot.candidates[0].word, "哥哥");
}

/// 集合外的方案切不过去，会话留在原方案，组合原样保留；也不能以集合外的方案建会话。
#[test]
fn a_wubi_only_session_refuses_schemes_outside_its_set() {
    let fixture = Fixture::new(WUBI_ROUTING_FIXTURE);
    let enabled = SchemeSet::of(&[SchemeType::Wubi]);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.enabled_schemes = enabled;
    });
    type_text(&mut session, "ge");
    let before = session.snapshot();
    for scheme in [
        SchemeType::Quanpin,
        SchemeType::Shuangpin,
        SchemeType::JapaneseRomaji,
        SchemeType::Korean,
        SchemeType::Cantonese,
        SchemeType::Zhuyin,
        SchemeType::Vietnamese,
    ] {
        let error = session.switch_scheme(scheme).unwrap_err();
        assert_eq!(
            error.to_string(),
            crate::diagnostics::INPUT_SCHEME_NOT_ENABLED,
            "{scheme:?}"
        );
        assert_eq!(session.snapshot(), before, "{scheme:?}");
    }
    session.switch_scheme(SchemeType::Wubi).unwrap();
    assert_eq!(session.snapshot().scheme, SchemeType::Wubi);

    let mut options = fixture.options();
    options.scheme = SchemeType::Quanpin;
    options.enabled_schemes = enabled;
    let error = Session::new(options).err().expect("quanpin is not enabled");
    assert_eq!(
        error.to_string(),
        crate::diagnostics::INPUT_SCHEME_NOT_ENABLED
    );
}

/// 临时日文要切到日文方案：集合里没有日文时 `R` 进不了这个模式，有日文（拼音版）时照常进入并回到原方案。
#[test]
fn temporary_japanese_needs_japanese_in_the_enabled_set() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut wubi = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.enabled_schemes = SchemeSet::of(&[SchemeType::Wubi]);
        options.local_modes.temporary_japanese = true;
    });
    wubi.character(b'R', true);
    let snapshot = wubi.snapshot();
    assert_ne!(snapshot.local_mode, LocalInputMode::TemporaryJapanese);
    assert_eq!(snapshot.scheme, SchemeType::Wubi);
    assert_eq!(wubi.input.engine.current_scheme_type(), SchemeType::Wubi);

    let mut pinyin = fixture.session_with(|options| {
        options.scheme = SchemeType::Quanpin;
        options.enabled_schemes = SchemeSet::of(&[
            SchemeType::Quanpin,
            SchemeType::Shuangpin,
            SchemeType::JapaneseRomaji,
        ]);
        options.local_modes.temporary_japanese = true;
    });
    assert!(pinyin.character(b'R', true).handled);
    assert_eq!(
        pinyin.snapshot().local_mode,
        LocalInputMode::TemporaryJapanese
    );
    type_text(&mut pinyin, "ka");
    assert!(words(&pinyin).contains(&"か".to_owned()));
    pinyin.command(Command::Cancel);
    assert_eq!(
        pinyin.input.engine.current_scheme_type(),
        SchemeType::Quanpin
    );
}

// ---- 网址模式 ----

/// 五笔码表里 `www` 是“众”的简码，`http` 没有词。
const URL_WUBI_FIXTURE: &str = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('www','众',100);";

fn wubi_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| options.scheme = SchemeType::Wubi)
}

#[test]
fn wubi_refresh_reuses_request_strings() {
    let fixture = Fixture::new(CARET_PREFIX_FIXTURE);
    let mut session = wubi_session(&fixture);
    type_text(&mut session, "aaaa");
    let request = session.input.engine.request();
    let pointers = [
        request.raw_input.as_ptr(),
        request.raw_input_with_cases.as_ptr(),
        request.normalized_input.as_ptr(),
        request.raw_segmentation.as_ptr(),
        request.normalized_segmentation.as_ptr(),
        request.segmentation.as_ptr(),
    ];
    session.input.engine.handle_key(SchemeKey::Requery);
    let request = session.input.engine.request();
    assert_eq!(
        [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ],
        pointers
    );
}

/// 依次送入网址的各个字符：字母和数字走 `character`，符号走 `punctuation`，与宿主把符号报成标点时一样。
fn type_url(session: &mut Session, text: &str) {
    for byte in text.bytes() {
        let result = if byte.is_ascii_alphanumeric() {
            session.character(byte, byte.is_ascii_uppercase())
        } else {
            session.punctuation(byte)
        };
        assert!(
            result.handled && result.commit.is_none(),
            "{:?} of {text:?}: {result:?}",
            byte as char
        );
    }
}

#[test]
fn url_www_dot_opens_url_mode_in_wubi() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = wubi_session(&fixture);
    type_text(&mut session, "www");
    assert_eq!(words(&session), ["众", "www."]);
    assert_eq!(session.snapshot().spelling_symbols, ".@");

    let dot = session.punctuation(b'.');
    assert!(dot.handled && dot.commit.is_none(), "{dot:?}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "www.");
    assert_eq!(snapshot.editing_text, "www.");
    assert_eq!(
        snapshot.spelling_symbols,
        crate::local::url::SPELLING_SYMBOLS
    );
    assert_eq!(snapshot.candidate_sources, [CandidateSource::Fallback]);

    type_url(&mut session, "google.com");
    assert_eq!(words(&session), ["www.google.com"]);
    let committed = session.select(0);
    assert_eq!(committed.commit.as_deref(), Some("www.google.com"));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert!(snapshot.preedit.is_empty());
}

#[test]
fn url_scheme_colon_in_quanpin_and_shuangpin() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| options.scheme = scheme);
        type_text(&mut session, "https");
        assert_eq!(session.snapshot().spelling_symbols, ":@#$%&", "{scheme:?}");
        type_url(&mut session, "://github.com/a?b=1");
        assert_eq!(session.snapshot().local_mode, LocalInputMode::Url);
        let committed = session.command(Command::CommitRaw);
        assert_eq!(
            committed.commit.as_deref(),
            Some("https://github.com/a?b=1"),
            "{scheme:?}"
        );
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

        // `ftp` 用 `.` 和 `:` 都能进入。
        for key in *b".:" {
            type_text(&mut session, "ftp");
            assert!(session.punctuation(key).handled);
            assert_eq!(session.snapshot().preedit, format!("ftp{}", key as char));
            session.command(Command::Cancel);
        }
    }
}

#[test]
fn url_wubi_https_opens_on_the_fifth_letter() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = wubi_session(&fixture);
    type_text(&mut session, "http");
    assert_eq!(session.snapshot().spelling_symbols, ":@");
    // 五笔码长上限是 4，别的第 5 个字母照旧被拒。
    assert!(!session.character(b't', false).handled);
    assert_eq!(session.snapshot().preedit, "http");

    let s = session.character(b's', false);
    assert!(s.handled && s.commit.is_none(), "{s:?}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "https");
    type_url(&mut session, "://x.com");
    assert_eq!(session.snapshot().preedit, "https://x.com");
}

#[test]
fn url_mark_outside_the_url_ends_it() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    type_url(&mut session, ".a.com");
    // 网址不收的键作为字符时交还宿主，组字不变；runtime 接着走标点路径。
    let typed = session.character(b'"', false);
    assert!(!typed.handled && typed.commit.is_none(), "{typed:?}");
    assert_eq!(session.snapshot().preedit, "www.a.com");
    let quote = session.punctuation(b'"');
    assert!(quote.handled);
    assert_eq!(quote.commit.as_deref(), Some("www.a.com\u{201c}"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // 空格也不是网址的一部分。
    type_text(&mut session, "www");
    type_url(&mut session, ".a");
    assert!(!session.character(b' ', false).handled);
    assert_eq!(session.snapshot().preedit, "www.a");
}

#[test]
fn url_backspace_past_the_trigger_restores_the_code() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = wubi_session(&fixture);
    type_text(&mut session, "www");
    type_url(&mut session, ".a");
    // 删掉字母时仍在网址模式。
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "www.");
    assert_eq!(words(&session), ["www."]);

    // 删掉触发键后退回组字，误触发时还能选回“众”。
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.preedit, "www");
    assert_eq!(snapshot.editing_text, "www");
    assert_eq!(words(&session), ["众", "www."]);
    assert_eq!(session.select(0).commit.as_deref(), Some("众"));
}

#[test]
fn url_wubi_backspace_never_clips_the_letters_and_undoes_the_s() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = wubi_session(&fixture);
    type_text(&mut session, "http");
    assert!(session.character(b's', false).handled);
    type_url(&mut session, ":");
    // 删掉 `:` 剩 5 个字母，五笔码长装不下，留在网址模式，`s` 不能丢。
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "https");
    assert_eq!(words(&session), ["https"]);
    type_url(&mut session, ":");
    assert_eq!(session.snapshot().preedit, "https:");
    assert!(session.command(Command::Backspace).handled);

    // 删掉进入网址模式的那个 `s`，退回五笔组字 `http`。
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.preedit, "http");
    assert_eq!(snapshot.spelling_symbols, ":@");
}

#[test]
fn url_caret_delete_of_the_trigger_restores_the_code() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = wubi_session(&fixture);
    type_text(&mut session, "www");
    type_url(&mut session, ".");
    assert!(session.command(Command::MoveLeft).handled);
    assert_eq!(session.snapshot().caret_position, 3);
    // 光标处删掉触发键与行末退格同一条规则：退回组字，还能选回“众”。
    assert!(session.command(Command::DeleteForward).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.preedit, "www");
    assert_eq!(snapshot.spelling_symbols, ".@");
    assert_eq!(words(&session), ["众", "www."]);
}

/// 五笔开混拼时码长不再限 4，`http` 加 `s` 仍直接进入网址模式；`https:` 删掉 `:` 能退回组字 `https`。
#[test]
fn url_wubi_mixed_pinyin_reverts_https_colon_to_the_composition() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = true;
    });
    type_text(&mut session, "http");
    assert!(session.character(b's', false).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "https");
    type_url(&mut session, ":");
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.preedit, "https");
    assert_eq!(snapshot.spelling_symbols, ":@");
}

/// 退回组字只是进入的逆操作：删掉中间的 `.` 后剩下的 `wwwexample` 不是触发词，留在网址模式。
#[test]
fn url_deleting_a_middle_dot_keeps_the_mode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    type_url(&mut session, ".example");
    for _ in 0.."example".len() {
        assert!(session.command(Command::MoveLeft).handled);
    }
    assert_eq!(session.snapshot().caret_position, 4);
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "wwwexample");
}

/// 光标在中间且网址已到长度上限时，网址收的键被吞掉而不是交还宿主；网址不收的键照旧交还。
#[test]
fn url_caret_insert_at_the_limit_is_swallowed() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    assert!(session.punctuation(b'.').handled);
    while session.snapshot().preedit.len() < crate::local::url::INPUT_LIMIT {
        assert!(session.character(b'a', false).handled);
    }
    assert!(session.command(Command::MoveLeft).handled);
    let before = session.snapshot().preedit;
    let digit = session.character(b'1', false);
    assert!(digit.handled && digit.commit.is_none(), "{digit:?}");
    assert!(session.punctuation(b'/').handled);
    assert_eq!(session.snapshot().preedit, before);
    assert!(!session.character(b'|', false).handled);
}

#[test]
fn url_backspace_keeps_the_mode_when_the_rest_is_not_lowercase_letters() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    type_url(&mut session, ".A.");
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "www.A");
}

#[test]
fn url_digits_and_uppercase_are_kept() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    assert!(session.punctuation(b'.').handled);
    assert!(session.character(b'1', false).handled);
    assert!(session.character(b'G', true).handled);
    assert_eq!(session.snapshot().preedit, "www.1G");
    // Shift+数字行的符号全是输入，不选候选。
    type_url(&mut session, "!@#$%^&*()");
    assert_eq!(session.snapshot().preedit, "www.1G!@#$%^&*()");
    // 数字作为选候选键时同样是输入。
    assert!(session.character(b'2', false).handled);
    assert_eq!(session.snapshot().preedit, "www.1G!@#$%^&*()2");
}

#[test]
fn url_input_commits_at_the_limit_and_returns_the_overflow_key() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    assert!(session.punctuation(b'.').handled);
    for _ in 4..crate::local::url::INPUT_LIMIT {
        assert!(session.character(b'a', false).handled);
    }
    let raw = session.snapshot().preedit;
    assert_eq!(raw.len(), crate::local::url::INPUT_LIMIT);
    let rejected = session.character(b'a', false);
    assert!(!rejected.handled);
    assert_eq!(rejected.commit.as_deref(), Some(raw.as_str()));
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn url_spelling_symbols_follow_the_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    for (raw, keys) in [
        ("www", ".@#$%&"),
        ("ftp", ".:@#$%&"),
        ("http", ":@#$%&"),
        ("https", ":@#$%&"),
        // 不是触发词的字母和数字仍能做邮箱的本地部分。
        ("ni", "@#$%&"),
        ("wwww", "@#$%&"),
        // 大写的辅助码不算触发词。
        ("wwW", "@#$%&"),
        ("xi'an", "#$%&"),
    ] {
        type_text(&mut session, raw);
        assert_eq!(session.snapshot().spelling_symbols, keys, "{raw}");
        session.command(Command::Cancel);
    }
    // 空闲时不变：生成类模式默认关闭，没有任何符号。
    assert!(session.snapshot().spelling_symbols.is_empty());

    // 光标不在末尾时只发布字面符号，按下的 `.` 不进入网址模式。
    type_text(&mut session, "www");
    session.command(Command::MoveLeft);
    assert_eq!(session.snapshot().spelling_symbols, "#$%&");
    assert!(!session.character(b'.', false).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    session.command(Command::Cancel);

    // 专用英文不识别网址。
    session.set_dedicated_english(true);
    type_text(&mut session, "www");
    assert!(session.snapshot().spelling_symbols.is_empty());
    session.set_dedicated_english(false);

    // 不识别网址的方案不发布。
    let mut korean = fixture.session_with(|options| options.scheme = SchemeType::Korean);
    korean.character(b'd', false);
    assert!(!korean.snapshot().spelling_symbols.contains('.'));
}

/// 网址模式里补全了邮箱后缀或顶级域名的行：整段网址那一行之后的全部行。
fn url_completions(session: &Session) -> Vec<String> {
    words(session).split_off(1)
}

#[test]
fn url_at_after_a_local_part_offers_email_suffixes() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "zhangsan");
    assert_eq!(session.snapshot().spelling_symbols, "@#$%&");
    let at = session.punctuation(b'@');
    assert!(at.handled && at.commit.is_none(), "{at:?}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "zhangsan@");
    // 打的网址留在第一行，空格和结束网址的标点上屏的仍是它；后缀按表的顺序接在后面。
    let expected: Vec<String> = crate::local::url::EMAIL_SUFFIXES
        .iter()
        .map(|suffix| format!("zhangsan{suffix}"))
        .collect();
    assert_eq!(words(&session)[0], "zhangsan@");
    assert_eq!(url_completions(&session), expected);
    assert!(snapshot
        .candidate_sources
        .iter()
        .all(|source| *source == CandidateSource::Fallback));
    assert!(snapshot.candidate_answers_key[0]);
    assert!(!snapshot.candidate_answers_key[1..].contains(&true));

    // 后缀显示着时数字照样是网址的一部分，不选候选，并按已打的域名缩小范围。
    let digit = session.character(b'1', false);
    assert!(digit.handled && digit.commit.is_none(), "{digit:?}");
    assert_eq!(
        url_completions(&session),
        [
            "zhangsan@163.com",
            "zhangsan@126.com",
            "zhangsan@139.com",
            "zhangsan@189.cn"
        ]
    );
    // 已经打成一个完整后缀时不再补全。
    type_url(&mut session, "63.com");
    assert_eq!(words(&session), ["zhangsan@163.com"]);

    // 选中后缀上屏整个邮箱并结束网址模式。
    for _ in 0..".com".len() {
        assert!(session.command(Command::Backspace).handled);
    }
    assert_eq!(words(&session), ["zhangsan@163", "zhangsan@163.com"]);
    let picked = session.select(1);
    assert_eq!(picked.commit.as_deref(), Some("zhangsan@163.com"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert!(session.snapshot().preedit.is_empty());

    // 空格不是网址的一部分：交还 runtime，由它上屏高亮的第一行，也就是打的网址。
    type_text(&mut session, "abc");
    assert!(session.character(b'@', true).handled);
    assert!(!session.character(b' ', false).handled);
    assert_eq!(session.select(0).commit.as_deref(), Some("abc@"));
}

#[test]
fn url_email_backspace_over_the_at_returns_to_the_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    assert!(session.punctuation(b'@').handled);
    assert_eq!(session.snapshot().preedit, "nihao@");
    // 删掉 `@` 退回组字，误触发后还能选回“你好”。
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.editing_text, "nihao");
    assert_eq!(words(&session)[0], "你好");
    assert_eq!(session.select(0).commit.as_deref(), Some("你好"));

    // 只有删掉末尾的 `@` 才是进入的逆操作：`abc@def` 删掉中间的 `@` 留在网址模式。
    type_text(&mut session, "abc");
    assert!(session.punctuation(b'@').handled);
    type_url(&mut session, "def");
    for _ in 0.."def".len() {
        assert!(session.command(Command::MoveLeft).handled);
    }
    assert!(session.command(Command::Backspace).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "abcdef");
}

#[test]
fn url_at_needs_a_plain_local_part() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    // 带撇号的组字不是邮箱：`@` 照旧结束组字。
    type_text(&mut session, "ni'hao");
    assert!(!session.snapshot().spelling_symbols.contains('@'));
    assert!(!session.character(b'@', true).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert_eq!(snapshot.editing_text, "ni'hao");
}

#[test]
fn url_scheme_completions_enter_url_mode_when_picked() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "htt");
    // 第一行是词典里的词（`h` 的“好”）时补全排在它后面。
    assert_eq!(words(&session)[..3], ["好", "https://", "http://"]);
    let snapshot = session.snapshot();
    assert!(!snapshot.candidate_answers_key[1] && !snapshot.candidate_answers_key[2]);

    // 选中补全不上屏，以它为预编辑进入网址模式，接着打网址。
    let picked = session.select(1);
    assert!(picked.handled && picked.commit.is_none(), "{picked:?}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "https://");
    assert_eq!(snapshot.caret_position, "https://".len());
    assert_eq!(words(&session), ["https://"]);
    type_url(&mut session, "github");
    assert_eq!(
        url_completions(&session),
        [
            "https://github.com",
            "https://github.cn",
            "https://github.net",
            "https://github.org",
            "https://github.com.cn"
        ]
    );
    // 选中顶级域名同样不上屏，还可以接着打路径。
    let tld = session.select(1);
    assert!(tld.handled && tld.commit.is_none(), "{tld:?}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::Url);
    assert_eq!(snapshot.preedit, "https://github.com");
    assert_eq!(words(&session), ["https://github.com"]);
    type_url(&mut session, "/a");
    let committed = session.select(0);
    assert_eq!(committed.commit.as_deref(), Some("https://github.com/a"));

    for (raw, completions) in [
        ("ht", &["https://", "http://"][..]),
        ("http", &["https://", "http://"]),
        ("https", &["https://"]),
        ("ww", &["www."]),
        ("www", &["www."]),
        ("ft", &["ftp://"]),
    ] {
        type_text(&mut session, raw);
        let shown = words(&session);
        assert!(
            shown
                .windows(completions.len())
                .any(|window| window == completions),
            "{raw}: {shown:?}"
        );
        session.command(Command::Cancel);
    }
    // 一个字母、别的拼音都不给。
    for raw in ["h", "w", "nihao"] {
        type_text(&mut session, raw);
        assert!(
            !words(&session)
                .iter()
                .any(|word| word.contains("://") || word == "www."),
            "{raw}"
        );
        session.command(Command::Cancel);
    }
}

#[test]
fn url_scheme_completion_is_not_taken_by_finishing() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = wubi_session(&fixture);
    // 码表里没有 `ft`：补全是唯一的行，排在最前面。
    type_text(&mut session, "ft");
    assert_eq!(words(&session), ["ftp://"]);
    // 结束组字（标点、失焦）时高亮停在补全上不算选中：没有别的行，上屏原文。
    let finished = session.finish(0);
    assert_eq!(finished.commit.as_deref(), Some("ft"));
    assert!(session.snapshot().preedit.is_empty());
    type_text(&mut session, "ft");
    let comma = session.punctuation(b',');
    assert_eq!(comma.commit.as_deref(), Some("ft，"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // 后面有组字自己的行时上屏那一行。
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "htt");
    let finished = session.finish(1);
    assert!(finished.commit.is_some_and(|text| text.starts_with('好')));
}

#[test]
fn url_wubi_keeps_its_word_ahead_of_the_scheme_completion() {
    let fixture = Fixture::new(URL_WUBI_FIXTURE);
    let mut session = wubi_session(&fixture);
    type_text(&mut session, "www");
    assert_eq!(words(&session), ["众", "www."]);
    assert_eq!(session.select(0).commit.as_deref(), Some("众"));

    type_text(&mut session, "www");
    let picked = session.select(1);
    assert!(picked.handled && picked.commit.is_none(), "{picked:?}");
    assert_eq!(session.snapshot().preedit, "www.");
    type_url(&mut session, "baidu");
    assert_eq!(url_completions(&session)[0], "www.baidu.com");
    session.command(Command::Cancel);

    // 五笔四码 `http` 不给补全：runtime 的顶字会在下一个字母上选第一行。
    type_text(&mut session, "http");
    assert!(words(&session).is_empty());
}

#[test]
fn plain_pinyin_period_is_still_chinese() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    assert_eq!(session.punctuation(b'.').commit.as_deref(), Some("你好。"));
    // 触发词后面的其他标点照旧结束组字。
    type_text(&mut session, "www");
    let comma = session.punctuation(b',');
    assert!(comma.handled);
    assert!(comma
        .commit
        .as_deref()
        .is_some_and(|text| text.ends_with('，')));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
}

#[test]
fn url_caret_editing_reaches_the_first_character_and_an_empty_url_leaves_the_mode() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    // 删掉 `.` 时剩下的不是触发词，留在网址模式，才能删到空。
    type_url(&mut session, ".A");
    session.command(Command::MoveHome);
    assert_eq!(session.snapshot().caret_position, 0);
    // 光标在中间时插入同样按网址规则：数字、符号接受，网址不收的键交还。
    assert!(session.character(b'1', false).handled);
    assert!(session.character(b'\'', false).handled);
    assert!(session.character(b'\'', false).handled);
    assert!(!session.character(b'|', false).handled);
    assert_eq!(session.snapshot().preedit, "1''www.A");
    session.command(Command::MoveHome);
    for _ in 0..7 {
        assert!(session.command(Command::DeleteForward).handled);
    }
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Url);
    assert_eq!(session.snapshot().preedit, "A");
    assert!(session.command(Command::DeleteForward).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.local_mode, LocalInputMode::None);
    assert!(snapshot.preedit.is_empty());
    assert!(snapshot.spelling_symbols.is_empty());
    // 退出后数字不再被吞掉。
    assert!(!session.character(b'1', false).handled);
}

/// 中 outweighs 总 the way it does in the shipped dictionary, and 西安 is a word while 先 is a character.
const GLIDE_FIXTURE: &str = "CREATE TABLE tbl_1_z(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_z VALUES('zhong','z','中',7680869),('zong','z','总',1273655);\
CREATE TABLE tbl_1_x(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_x VALUES('xi','x','西',1000000),('xian','x','先',2000000);\
CREATE TABLE tbl_1_a(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_1_a VALUES('an','a','安',900000);\
CREATE TABLE tbl_2_x(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
INSERT INTO tbl_2_x VALUES('xi''an','xa','西安',300000);";

fn glide(session: &mut Session, word: &str) -> crate::types::KeyResult {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    session.glide(&keyboard(), &stroke(word, 0.1, &[]))
}

#[test]
fn a_glide_types_the_letters_the_dictionary_prefers() {
    let fixture = Fixture::new(GLIDE_FIXTURE);
    let mut session = fixture.session();
    // `h` 在 `z` 到 `o` 的连线上，单看笔画，`zong` 和 `zhong` 一样像。
    assert!(glide(&mut session, "zhong").handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "zhong");
    assert_eq!(words(&session).first().map(String::as_str), Some("中"));
}

#[test]
fn a_glide_into_a_composition_starts_a_new_syllable() {
    let fixture = Fixture::new(GLIDE_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "xi");
    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| glide(&mut session, "an"));
    assert!(result.handled);
    assert!(
        allocations <= 7160,
        "glide insertion allocations: {allocations}"
    );
    assert_eq!(session.snapshot().editing_text, "xi'an");
    assert_eq!(words(&session).first().map(String::as_str), Some("西安"));
}

#[test]
fn a_glide_is_left_to_the_host_outside_quanpin_composition() {
    let fixture = Fixture::new(GLIDE_FIXTURE);
    let mut session = fixture.session();
    session.set_dedicated_english(true);
    assert!(!glide(&mut session, "zhong").handled);
    assert!(session.snapshot().editing_text.is_empty());

    let mut session = fixture.session();
    let mut broken = crate::pinyin::glide::tests::keyboard();
    broken.key_width = 0.0;
    let points = crate::pinyin::glide::tests::stroke("zhong", 0.0, &[]);
    assert!(!session.glide(&broken, &points).handled);
    assert!(session.snapshot().editing_text.is_empty());
}

/// 只出单字：全拼列表里只剩单字（不含汉字的 `GitHub` 照旧），选一个字上屏一个字，剩下的拼写接着出单字；九宫格从同一个选项拿到开关。
#[test]
fn single_character_only_offers_one_character_at_a_time() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    assert!(words(&session).contains(&"你好".to_owned()));
    session.command(Command::Cancel);

    let mut session = fixture.session_with(|options| options.single_character_only = true);
    type_text(&mut session, "nihao");
    assert_eq!(words(&session), ["GitHub", "你"]);
    assert_eq!(
        select_word(&mut session, "你").commit.as_deref(),
        Some("你")
    );
    assert_eq!(session.snapshot().editing_text, "hao");
    assert_eq!(words(&session), ["好"]);
    assert_eq!(
        select_word(&mut session, "好").commit.as_deref(),
        Some("好")
    );
    assert!(session.snapshot().editing_text.is_empty());

    let mut session = fixture.session_with(|options| options.single_character_only = true);
    session.set_nine_key_enabled(true);
    type_text(&mut session, "64426");
    assert_eq!(words(&session), ["GitHub", "你"]);
}

/// #6059：`SessionOptions` 的句子联想设置同样交给九宫格，关掉词网格时 26 键和九宫格都不出整句行，打开整句备选时两边都交回全部读法。
#[test]
fn nine_key_follows_the_sentence_options_like_the_full_keyboard() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('mi','m','米',100);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',100),('hao','h','号',50);",
    );
    let sentences = |session: &Session| -> Vec<String> {
        session
            .snapshot()
            .candidates
            .into_iter()
            .filter(|item| item.source == CandidateSource::Generated && item.sentence_association)
            .map(|item| item.word.into_string())
            .collect()
    };
    let cases = |session: &mut Session, expected: &[&str]| {
        for nine_key in [false, true] {
            session.set_nine_key_enabled(nine_key);
            type_text(session, if nine_key { "64426" } else { "mihao" });
            assert_eq!(sentences(session), expected, "nine key: {nine_key}");
            session.command(Command::Cancel);
        }
    };
    cases(&mut fixture.session(), &["米好"]);
    cases(
        &mut fixture.session_with(|options| options.sentence_association.word_lattice = false),
        &[],
    );
    cases(
        &mut fixture.session_with(|options| options.sentence_alternatives = true),
        &["米好", "米号"],
    );
}

/// 九宫格选中整句时存词的音节上限与全拼键盘相同（#5640）。
#[test]
fn nine_key_sentence_learning_shares_the_syllable_cap() {
    assert_eq!(
        crate::nine_key::MAX_LEARNED_SENTENCE_SYLLABLES,
        super::learning::MAX_LEARNED_SENTENCE_SYLLABLES
    );
}

/// #6185：在 26 键上打过的词，九宫格按首字母分词输入时也排到前面；关掉个人上下文的会话不读模型里早先记下的使用，也就不挪。
#[test]
fn words_typed_on_the_full_keyboard_lead_the_nine_key_initials() {
    let mut main = String::from(
        "CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_y VALUES('yin','y','因',100);\
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_s VALUES('si','s','四',100);\
CREATE TABLE tbl_2_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_y VALUES('yin''si','ys','隐私',10);",
    );
    for index in 0..100 {
        main.push_str(&format!(
            "INSERT INTO tbl_2_y VALUES('ya''qi','yq','压{index}',{});",
            100_000 - index
        ));
    }
    let fixture = Fixture::new(&main);
    let promote = |options: &mut SessionOptions| {
        options.frequency = crate::types::FrequencyAdjustmentOptions {
            mode: crate::types::FrequencyAdjustmentMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        };
    };
    let position_after_typing = |session: &mut Session| {
        type_text(session, "yinsi");
        assert_eq!(select_word(session, "隐私").commit.as_deref(), Some("隐私"));
        session.set_nine_key_enabled(true);
        for key in *b"9'7" {
            session.character(key, false);
        }
        let position = words(session).iter().position(|word| word == "隐私");
        session.command(Command::Cancel);
        session.set_nine_key_enabled(false);
        position
    };
    let mut session = fixture.session_with(promote);
    assert_eq!(position_after_typing(&mut session), Some(4));
    let mut quiet = fixture.session_with(|options| {
        promote(options);
        options.personal_context = false;
    });
    assert_eq!(position_after_typing(&mut quiet), None);
}

/// 九宫格选一个词记进个人上下文模型的次数与全拼键盘显式选词相同（#6185）。
#[test]
fn nine_key_personal_picks_count_like_the_full_keyboard() {
    assert_eq!(
        crate::nine_key::PERSONAL_PICK_TIMES,
        super::learning::DICTIONARY_PICK_TIMES
    );
}

/// #5848、#5667、#5907：`SessionOptions::expressive` 同样交给九宫格；26 键和九宫格的 emoji、颜文字都紧跟它描绘的那个词，接不上任何词的排在末尾。
#[test]
fn nine_key_mixes_emoji_and_kaomoji_like_the_full_keyboard() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('mei','m','美',300);\
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_g VALUES('guo','g','国',300),('gao','g','高',200);\
CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_m VALUES('mei''guo','mg','美国',1000);\
CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_j VALUES('ji','j','鸡',300),('ji','j','几',200),('jing','j','警',100);\
CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_j VALUES('jing''gao','jg','警告',900);\
CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_l VALUES('li','l','里',500),('li','l','梨',100);",
    );
    Connection::open(fixture.path().join(assets::OTHER_DICTIONARY))
        .and_then(|connection| {
            connection.execute_batch(
                "CREATE TABLE emoji_pinyin(key TEXT,emoji TEXT,sort_order INTEGER);\
INSERT INTO emoji_pinyin VALUES('meiguo','🇺🇸',1893),('meiguobentuwaixiaodaoyu','🇺🇲',1891),('jinggao','⚠️',1433),('ji','🐔',626),('jiqiren','🤖',100),('li','🍐',730),('liwu','🎁',50);\
CREATE TABLE emoji(emoji TEXT PRIMARY KEY,keywords TEXT);\
INSERT INTO emoji VALUES('🇺🇸','美国 美利坚 星条旗'),('🇺🇲','美国本土外小岛屿 flag: u.s. outlying islands'),('⚠️','警告 注意 危险'),('🐔','鸡 鸡头 chicken'),('🤖','机器人 robot'),('🍐','梨 梨子 pear'),('🎁','礼物 gift');\
CREATE TABLE kaomoji(pinyin TEXT,jianpin TEXT,kaomoji TEXT,sort_order INTEGER);\
INSERT INTO kaomoji VALUES('meiguo','mg','(•̀ᴗ•́)و',10),('jinggao','jg','(ﾟДﾟ≡ﾟдﾟ)!?',20);\
CREATE TABLE kaomoji_catalog(kaomoji TEXT PRIMARY KEY,keywords TEXT);\
INSERT INTO kaomoji_catalog VALUES('(•̀ᴗ•́)و','mei guo'),('(ﾟДﾟ≡ﾟдﾟ)!?','jing gao 警告');",
            )
        })
        .expect("fixture msime-others.db");
    let position = |session: &Session, word: &str| {
        words(session)
            .iter()
            .position(|candidate| candidate == word)
            .unwrap_or_else(|| panic!("missing {word} in {:?}", words(session)))
    };
    // 每一项：26 键的输入、九宫格的数字、词、紧跟它的 emoji。
    let cases = [
        ("meiguo", "634486", "美国", "🇺🇸"),
        ("jinggao", "5464426", "警告", "⚠️"),
        ("ji", "54", "鸡", "🐔"),
        ("li", "54", "梨", "🍐"),
    ];
    let mut session = fixture.session_with(|options| {
        options.expressive.emoji_candidates = true;
        options.expressive.kaomoji_candidates = true;
    });
    for nine_key in [false, true] {
        session.set_nine_key_enabled(nine_key);
        for (letters, digits, word, emoji) in cases {
            type_text(&mut session, if nine_key { digits } else { letters });
            let at = position(&session, word);
            assert!(at > 0 || word == words(&session)[0]);
            assert_eq!(position(&session, emoji), at + 1, "{nine_key} {word}");
            assert_eq!(
                session.snapshot().candidates[at + 1].source,
                CandidateSource::Emoji
            );
            session.command(Command::Cancel);
        }
        // 🇺🇲 画的不是 美国，排在末尾，后面只有接不上的颜文字；警告 的颜文字接在 ⚠️ 后面。
        type_text(&mut session, if nine_key { "634486" } else { "meiguo" });
        let listed = words(&session);
        assert_eq!(listed[listed.len() - 2..], ["🇺🇲", "(•̀ᴗ•́)و"], "{nine_key}");
        session.command(Command::Cancel);
        type_text(&mut session, if nine_key { "5464426" } else { "jinggao" });
        assert_eq!(
            words(&session)[..3],
            ["警告", "⚠️", "(ﾟДﾟ≡ﾟдﾟ)!?"],
            "{nine_key}"
        );
        session.command(Command::Cancel);
    }

    // 默认两个开关都关：九宫格不混入。
    let mut off = fixture.session();
    off.set_nine_key_enabled(true);
    type_text(&mut off, "634486");
    assert!(!off.snapshot().candidates.iter().any(|item| matches!(
        item.source,
        CandidateSource::Emoji | CandidateSource::Kaomoji
    )));
}

// ---- 打拼音出符号（local/symbol_candidates.rs） ----

/// 正负（zheng'fu）排在 政府 后面；大于 供部分选择后再选符号；派（pai）可以由 pa 加辅助码 I 拼出。
const SYMBOL_FIXTURE: &str = "CREATE TABLE tbl_1_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_z VALUES('zheng','z','正',300),('zheng','z','政',200);\
CREATE TABLE tbl_1_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_f VALUES('fu','f','负',300),('fu','f','府',200);\
CREATE TABLE tbl_1_d(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_d VALUES('da','d','大',300);\
CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_y VALUES('yu','y','于',300);\
CREATE TABLE tbl_1_p(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_p VALUES('pai','p','派',300),('pa','p','怕',300);\
INSERT INTO tbl_1_p VALUES('ping','p','平',300);\
CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('mi','m','米',300);\
CREATE TABLE tbl_2_p(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_p VALUES('ping''mi','pm','平米',500);\
CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_z VALUES('zheng''fu','zf','政府',1000),('zheng''fu','zf','正负',500);\
CREATE TABLE tbl_2_d(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_d VALUES('da''yu','dy','大于',900);\
CREATE TABLE tbl_4_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);";

fn symbol_session(fixture: &Fixture, configure: impl FnOnce(&mut SessionOptions)) -> Session {
    fixture.session_with(|options| {
        options.expressive.symbol_candidates = true;
        configure(options);
    })
}

/// 列表里文字为 `symbol` 的符号行（来源 Emoji）的位置。
fn symbol_index(session: &Session, symbol: &str) -> Option<usize> {
    session
        .snapshot()
        .candidates
        .iter()
        .position(|item| item.source == CandidateSource::Emoji && item.word == symbol)
}

#[test]
fn a_whole_symbol_name_offers_its_symbol_after_the_name() {
    let fixture = Fixture::new(SYMBOL_FIXTURE);
    let mut session = symbol_session(&fixture, |_| {});
    type_text(&mut session, "zhengfu");
    assert_eq!(words(&session)[..3], ["政府", "正负", "±"]);
    assert_eq!(symbol_index(&session, "±"), Some(2));
    session.command(Command::Cancel);
    // 没打完整的名称不出。
    type_text(&mut session, "zhengf");
    assert_eq!(symbol_index(&session, "±"), None, "{:?}", words(&session));
    session.command(Command::Cancel);

    // 双拼小鹤 vgfu 归一成 zhengfu。
    let mut shuangpin = symbol_session(&fixture, |options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
    });
    type_text(&mut shuangpin, "vgfu");
    assert!(
        symbol_index(&shuangpin, "±").is_some(),
        "{:?}",
        words(&shuangpin)
    );

    // 开关关着时没有符号行。
    let mut off = fixture.session();
    type_text(&mut off, "zhengfu");
    assert_eq!(symbol_index(&off, "±"), None);
}

#[test]
fn microsoft_semicolon_spelling_offers_the_symbol_after_its_name() {
    let fixture = Fixture::new(SYMBOL_FIXTURE);
    let mut session = symbol_session(&fixture, |options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
    });
    type_text(&mut session, "p;mi");
    let name = words(&session)
        .iter()
        .position(|word| *word == "平米")
        .unwrap();
    assert_eq!(symbol_index(&session, "㎡"), Some(name + 1));
}

#[test]
fn committing_a_symbol_ends_the_composition_and_learns_nothing() {
    let fixture = Fixture::new(SYMBOL_FIXTURE);
    let mut session = symbol_session(&fixture, |options| {
        options.frequency.mode = FrequencyAdjustmentMode::Promote;
        options.frequency.trigger_count = 1;
    });
    // 先上屏一个中文词，个人上下文链有了上一个词。
    type_text(&mut session, "zhengfu");
    assert_eq!(session.select(0).commit.as_deref(), Some("政府"));
    assert_eq!(session.input.chain.previous.as_deref(), Some("政府"));
    // 个人上下文由后台线程延迟写入，并行的测试随时可能 `flush_all` 把它提前写下；比较前先写完，前后差的才只是选符号这一步。
    flush_all();
    let before = journal_rows(&fixture.journal());

    type_text(&mut session, "zhengfu");
    let index = symbol_index(&session, "±").unwrap();
    assert!(!session.pin(index).handled);
    assert!(!session.remove(index).handled);
    assert!(!session.fix_position(index, 1).handled);
    assert_eq!(symbol_index(&session, "±"), Some(index));
    assert!(!session
        .input
        .is_editable_source(&session.snapshot().candidates[index]));
    let ranking = session.input.ranking_index(index).unwrap();
    assert_eq!(session.input.learn_candidate(ranking), None);

    let result = session.select(index);
    assert_eq!(result.commit.as_deref(), Some("±"));
    let snapshot = session.snapshot();
    assert!(snapshot.editing_text.is_empty() && snapshot.candidates.is_empty());
    assert_eq!(session.input.chain.previous, None);
    flush_all();
    assert_eq!(journal_rows(&fixture.journal()), before);
}

#[test]
fn a_symbol_after_a_partial_pick_makes_no_word() {
    let fixture = Fixture::new(SYMBOL_FIXTURE);
    let mut session = symbol_session(&fixture, |_| {});
    type_text(&mut session, "zhengfudayu");
    select_word(&mut session, "正负");
    assert!(!session.snapshot().editing_text.is_empty());
    flush_all();
    let before = journal_rows(&fixture.journal());
    let index = symbol_index(&session, "＞").unwrap_or_else(|| panic!("{:?}", words(&session)));
    let result = session.select(index);
    assert!(
        result
            .commit
            .as_deref()
            .is_some_and(|text| text.ends_with('＞')),
        "{result:?}"
    );
    assert!(session.snapshot().editing_text.is_empty());
    flush_all();
    assert_eq!(journal_rows(&fixture.journal()), before);
    type_text(&mut session, "zhengfudayu");
    assert!(!words(&session).iter().any(|word| word.contains('＞')));
}

#[test]
fn symbols_stay_out_of_helpcode_caret_prefix_and_dedicated_english() {
    let fixture = Fixture::new(SYMBOL_FIXTURE);
    let mut session = symbol_session(&fixture, |_| {});
    // 对照：pai 出 π。
    type_text(&mut session, "pai");
    assert!(
        symbol_index(&session, "π").is_some(),
        "{:?}",
        words(&session)
    );
    session.command(Command::Cancel);
    // 大写的 I 是辅助码，转成小写正好是 pai，但输入里混着辅助码字母。
    type_text(&mut session, "pa");
    assert!(session.character(b'I', false).handled);
    assert!(session.input.has_active_helpcode());
    assert_eq!(symbol_index(&session, "π"), None, "{:?}", words(&session));
    session.command(Command::Cancel);

    type_text(&mut session, "zhengfudayu");
    session.set_caret(Some(7));
    assert!(session.input.prefix_active);
    assert_eq!(symbol_index(&session, "±"), None, "{:?}", words(&session));
    session.command(Command::Cancel);

    session.set_dedicated_english(true);
    type_text(&mut session, "zhengfu");
    assert_eq!(symbol_index(&session, "±"), None, "{:?}", words(&session));
}

/// 符号跟在名称词后面，名称词被挪走时符号不能顶到首位：只出单字筛掉了多字的名称词，或者用户把名称词固定到了后面的位置。两种情况下空格都不能直接上屏符号。
#[test]
fn a_symbol_never_leads_once_its_name_moves_away() {
    let fixture = Fixture::new(SYMBOL_FIXTURE);
    let mut single = symbol_session(&fixture, |options| options.single_character_only = true);
    type_text(&mut single, "zhengfu");
    assert_eq!(words(&single)[0], "正");
    assert!(
        symbol_index(&single, "±").is_some_and(|index| index > 0),
        "{:?}",
        words(&single)
    );
    single.command(Command::Cancel);
    type_text(&mut single, "dayu");
    assert_eq!(words(&single)[0], "大");
    single.command(Command::Cancel);

    let mut session = symbol_session(&fixture, |_| {});
    type_text(&mut session, "dayu");
    assert_eq!(words(&session)[..3], ["大于", ">", "＞"]);
    let result = session.fix_position(0, 3);
    assert!(result.handled && result.diagnostic.is_none(), "{result:?}");
    // 名称词留在固定的第 3 位，首位换成后面第一个没固定的词，符号依次补进空位。
    let listed = words(&session);
    assert_eq!(listed[..4], ["大", ">", "大于", "＞"], "{listed:?}");
    assert_eq!(
        session.snapshot().candidates[0].source,
        CandidateSource::Database
    );
}

/// 五笔不混排，符号开关开着时四码唯一候选照样自动上屏。
#[test]
fn the_wubi_unique_four_code_ignores_the_symbol_switch() {
    let fixture = Fixture::new(
        "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('dayu','大鱼',100);",
    );
    let mut session = symbol_session(&fixture, |options| {
        options.scheme = SchemeType::Wubi;
        options.wubi.mixed_pinyin = false;
    });
    type_text(&mut session, "dayu");
    assert_eq!(words(&session), vec!["大鱼".to_owned()]);
    assert!(session.snapshot().wubi_unique_four_code);
}

// ---- 整句改字（session/conversion.rs） ----

/// 「woqubeijing」的首选整句是「我去背景」，想要的「我去北京」不在候选里。
const CONVERSION_FIXTURE: &str =
    "CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wo','w','我',9000);\
CREATE TABLE tbl_1_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_q VALUES('qu','q','去',9000);\
CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_b VALUES('bei','b','被',9000);\
INSERT INTO tbl_1_b VALUES('bei','b','北',5000);\
INSERT INTO tbl_1_b VALUES('bei','b','背',4000);\
CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_j VALUES('jing','j','经',9000);\
INSERT INTO tbl_1_j VALUES('jing','j','京',3000);\
INSERT INTO tbl_1_j VALUES('jing','j','景',2000);\
CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_b VALUES('bei''jing','bj','背景',30000);\
INSERT INTO tbl_2_b VALUES('bei''jing','bj','北京',20000);\
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
CREATE TABLE tbl_4_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);";

fn conversion_session(fixture: &Fixture) -> Session {
    let mut session = fixture.session_with(|options| options.personal_context = false);
    type_text(&mut session, "woqubeijing");
    assert_eq!(words(&session)[0], "我去背景");
    session
}

#[test]
fn the_left_key_enters_the_sentence_and_moves_by_character() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = conversion_session(&fixture);
    assert!(session.command(Command::ConversionLeft).handled);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.conversion, "我去背景");
    // 光标在最后一个字「景」前，候选替换的是从它到词尾这一段，先列当前的字。
    assert_eq!(snapshot.conversion_focus, (3, 4));
    assert_eq!(words(&session), ["景", "经", "京"]);
    // 组字原文和字母光标不动。
    assert_eq!(snapshot.editing_text, "woqubeijing");
    assert_eq!(snapshot.caret_position, 11);

    session.command(Command::ConversionLeft);
    assert_eq!(session.snapshot().conversion_focus, (2, 4));
    // 光标在词首：先是整个词，再是更短的单字。
    assert_eq!(words(&session), ["背景", "北京", "被", "北", "背"]);

    for _ in 0..5 {
        session.command(Command::ConversionLeft);
    }
    assert_eq!(session.snapshot().conversion_focus, (0, 1));
    for _ in 0..6 {
        session.command(Command::ConversionRight);
    }
    // 句末没有候选，空格（选择或 CommitCandidate）上屏整句。
    assert_eq!(session.snapshot().conversion_focus, (4, 4));
    assert!(words(&session).is_empty());
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("我去背景")
    );
    assert!(session.snapshot().conversion.is_empty());
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn choosing_a_word_pins_it_and_commits_the_sentence() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = conversion_session(&fixture);
    session.command(Command::ConversionLeft);
    session.command(Command::ConversionLeft);
    let picked = session.select(index_of(&session, "北京"));
    assert!(picked.handled && picked.commit.is_none(), "{picked:?}");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.conversion, "我去北京");
    // 选完光标移到这段后面，正好是句末。
    assert_eq!(snapshot.conversion_focus, (4, 4));

    // 回车上屏改好的汉字，不是拼音。
    let committed = session.command(Command::CommitRaw);
    assert_eq!(committed.commit.as_deref(), Some("我去北京"));
    assert_eq!(committed.diagnostic, None);
    assert!(session.snapshot().preedit.is_empty());
    // 改好的整句与选中整句候选一样存成个人词条。
    assert_eq!(
        count(
            &fixture.journal(),
            "SELECT count(*) FROM user_dictionary_operations WHERE key='wo''qu''bei''jing' AND value='我去北京' AND user_inserted=1"
        ),
        1
    );
}

#[test]
fn a_pinned_character_inside_a_word_reconverts_its_neighbours() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = conversion_session(&fixture);
    session.command(Command::ConversionLeft);
    assert!(session.select(index_of(&session, "京")).handled);
    // 「背景」被拆开，「bei」单独重新转换；光标到了句末。
    assert_eq!(session.snapshot().conversion, "我去被京");
    assert_eq!(session.snapshot().conversion_focus, (4, 4));
    session.command(Command::ConversionLeft);
    session.command(Command::ConversionLeft);
    assert_eq!(session.snapshot().conversion_focus, (2, 3));
    assert!(session.select(index_of(&session, "北")).handled);
    assert_eq!(session.snapshot().conversion, "我去北京");
    assert_eq!(session.snapshot().conversion_focus, (3, 4));
    // 标点和失去焦点一样，上屏的是改好的整句。
    let finished = session.punctuation(b',');
    assert_eq!(finished.commit.as_deref(), Some("我去北京，"));
}

#[test]
fn typing_or_escaping_leaves_the_sentence_and_keeps_the_letters() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = conversion_session(&fixture);
    session.command(Command::ConversionLeft);
    session.command(Command::ConversionLeft);
    session.select(index_of(&session, "北京"));

    // Esc 和退格回到拼音，组字和原来的候选都在，钉住的段丢掉。
    assert!(session.command(Command::Cancel).handled);
    let snapshot = session.snapshot();
    assert!(snapshot.conversion.is_empty());
    assert_eq!(snapshot.conversion_focus, (0, 0));
    assert_eq!(snapshot.editing_text, "woqubeijing");
    assert_eq!(words(&session)[0], "我去背景");
    session.command(Command::ConversionLeft);
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().editing_text, "woqubeijing");
    assert!(session.snapshot().conversion.is_empty());

    // 数字和空格不是组字的键：交回去由选择处理，不退出改字。
    session.command(Command::ConversionLeft);
    assert!(!session.character(b'1', false).handled);
    assert!(!session.character(b' ', false).handled);
    assert_eq!(session.snapshot().conversion, "我去背景");
    // 字母退出改字，接在拼音末尾。
    assert!(session.character(b'a', false).handled);
    assert!(session.snapshot().conversion.is_empty());
    assert_eq!(session.snapshot().editing_text, "woqubeijinga");
    session.command(Command::Backspace);

    // 字母光标命令（Ctrl+左右）退出改字后照常移字母光标。
    session.command(Command::ConversionLeft);
    assert!(session.command(Command::MoveLeft).handled);
    assert!(session.snapshot().conversion.is_empty());
    assert_eq!(session.snapshot().caret_position, 10);
}

#[test]
fn without_a_sentence_the_conversion_keys_move_the_letter_caret() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = fixture.session();
    // 读音不完整，首选不是一个字对一个完整音节的整句。
    type_text(&mut session, "woqub");
    assert!(session.command(Command::ConversionLeft).handled);
    let snapshot = session.snapshot();
    assert!(snapshot.conversion.is_empty());
    assert_eq!(snapshot.caret_position, 4);
    assert!(session.command(Command::ConversionRight).handled);
    assert_eq!(session.snapshot().caret_position, 5);
    session.command(Command::Cancel);

    // 右移不进入改字；单字也不进入。
    type_text(&mut session, "woqubeijing");
    session.command(Command::ConversionRight);
    assert!(session.snapshot().conversion.is_empty());
    session.command(Command::Cancel);
    type_text(&mut session, "wo");
    session.command(Command::ConversionLeft);
    assert!(session.snapshot().conversion.is_empty());
    assert_eq!(session.snapshot().caret_position, 1);
    session.command(Command::Cancel);

    // 光标在中间时首选是光标前那段的候选，不覆盖整个组字，也不进入改字。
    type_text(&mut session, "woqubeijing");
    session.command(Command::MoveLeft);
    session.command(Command::MoveLeft);
    session.command(Command::MoveLeft);
    session.command(Command::MoveLeft);
    session.command(Command::ConversionLeft);
    assert!(session.snapshot().conversion.is_empty());
}

#[test]
fn the_conversion_candidates_cannot_be_pinned_or_removed() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = conversion_session(&fixture);
    session.command(Command::ConversionLeft);
    assert!(!session.pin(1).handled);
    assert!(!session.remove(1).handled);
    assert!(!session.fix_position(1, 1).handled);
    assert!(!session.select_edge(1, CandidateEdge::FirstHan).handled);
    assert!(!session.expand_initial_candidates());
    assert_eq!(words(&session), ["景", "经", "京"]);
}

#[test]
fn shuangpin_enters_the_same_sentence() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.personal_context = false;
    });
    // 小鹤双拼：wo qu bw(bei) jk(jing)
    type_text(&mut session, "woqubwjk");
    assert_eq!(words(&session)[0], "我去背景");
    session.command(Command::ConversionLeft);
    session.command(Command::ConversionLeft);
    session.select(index_of(&session, "北京"));
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("我去北京")
    );
}

#[test]
fn the_conversion_starts_from_the_highlighted_row() {
    let fixture = Fixture::new(CONVERSION_FIXTURE);
    let mut session = conversion_session(&fixture);
    // 另一条整句排在第二位（宿主高亮着它，比如 runtime 把它重排到了前面）。
    let mut second = session.input.mixed_candidates[0].clone();
    second.word = "我去北京".into();
    second.sentence_words = vec!["我".into(), "去".into(), "北京".into()];
    session.input.mixed_candidates.insert(1, second);
    assert!(session.conversion_left_from(1).handled);
    assert_eq!(session.snapshot().conversion, "我去北京");
    assert_eq!(session.snapshot().conversion_focus, (3, 4));
    // 已在改字里时与普通的左移相同，不再换行。
    session.conversion_left_from(0);
    assert_eq!(session.snapshot().conversion, "我去北京");
    assert_eq!(session.snapshot().conversion_focus, (2, 4));
    // 什么都没改就上屏时，上屏的是进入改字时的那一行。
    session.command(Command::Cancel);
    session.conversion_left_from(1);
    for _ in 0..4 {
        session.command(Command::ConversionRight);
    }
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("我去北京")
    );

    // 高亮的不是覆盖整个组字的句子时进不了改字，按字母光标左移。
    type_text(&mut session, "woqubeijing");
    let prefix = index_of(&session, "我");
    assert!(session.conversion_left_from(prefix).handled);
    assert!(session.snapshot().conversion.is_empty());
    assert_eq!(session.snapshot().caret_position, 10);
}

/// #6888 的 AI 回答：读音 ni 有四个本地候选。
const AI_ANSWER_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',100),('ni','n','拟',90),('ni','n','妮',80),('ni','n','倪',70);";

fn sources(session: &Session) -> Vec<(String, CandidateSource)> {
    session
        .snapshot()
        .candidates
        .into_iter()
        .map(|item| (item.word.into_string(), item.source))
        .collect()
}

/// 不再有 AI 占位行：等回答时列表里只有本地候选，第 2 位就是第二个本地候选。
#[test]
fn no_ai_placeholder_is_ever_inserted() {
    let fixture = Fixture::new(AI_ANSWER_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "n");
    session.character(b'i', false);
    assert!(session
        .online_query()
        .is_some_and(|query| query.ai_eligible));
    assert_eq!(
        sources(&session),
        [
            ("你".to_owned(), CandidateSource::Database),
            ("拟".to_owned(), CandidateSource::Database),
            ("妮".to_owned(), CandidateSource::Database),
            ("倪".to_owned(), CandidateSource::Database),
        ]
    );
}

/// AI 回答到达时整批接在本地候选之后，本地候选一个都不挪；第一条显示在第一页最后一格是 input-runtime 的事（#6888）。
#[test]
fn an_ai_answer_goes_after_the_local_rows_without_moving_them() {
    let fixture = Fixture::new(AI_ANSWER_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    let batch = ["智一", "智二", "智三"].map(str::to_owned);
    assert!(session.apply_online_candidates(&query, &batch, CandidateSource::AiSuggestion));
    assert_eq!(
        sources(&session),
        [
            ("你".to_owned(), CandidateSource::Database),
            ("拟".to_owned(), CandidateSource::Database),
            ("妮".to_owned(), CandidateSource::Database),
            ("倪".to_owned(), CandidateSource::Database),
            ("智一".to_owned(), CandidateSource::AiSuggestion),
            ("智二".to_owned(), CandidateSource::AiSuggestion),
            ("智三".to_owned(), CandidateSource::AiSuggestion),
        ]
    );
}

#[test]
fn an_answer_of_listed_words_changes_nothing() {
    let fixture = Fixture::new(AI_ANSWER_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    assert!(session.apply_online_candidates(
        &query,
        &["妮".to_owned()],
        CandidateSource::AiSuggestion
    ));
    assert_eq!(words(&session), ["你", "拟", "妮", "倪"]);
}

#[test]
fn a_cloud_answer_takes_slot_one_and_ai_rows_stay_last() {
    let fixture = Fixture::new(AI_ANSWER_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");
    let query = session.online_query().expect("a quanpin query");
    assert!(session.apply_online_candidates(
        &query,
        &["智一".to_owned(), "智二".to_owned()],
        CandidateSource::AiSuggestion
    ));
    assert!(session.apply_online_candidate(&query, "云", CandidateSource::CloudSuggestion));
    assert_eq!(
        words(&session),
        ["你", "云", "拟", "妮", "倪", "智一", "智二"]
    );
}

#[test]
fn first_page_rows_reaches_the_dictionary_through_the_session_options() {
    // 首页行数从会话选项一路走到词库：默认 6 下 6 条精确同音词把补全词挤到首页之后，设成 8 后补全词按权重并进来。
    // 词库级测试直接调 `set_first_page_rows`，这条钉住的是 `SessionOptions` → 请求 → 引擎这段接线。
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_q(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
         INSERT INTO tbl_2_q VALUES('qi''shi','qs','其实',500);\
         INSERT INTO tbl_2_q VALUES('qi''shi','qs','歧视',400);\
         INSERT INTO tbl_2_q VALUES('qi''shi','qs','岂是',300);\
         INSERT INTO tbl_2_q VALUES('qi''shi','qs','气势',200);\
         INSERT INTO tbl_2_q VALUES('qi''shi','qs','启示',100);\
         INSERT INTO tbl_2_q VALUES('qi''shi','qs','骑士',90);\
         CREATE TABLE tbl_5_q(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
         INSERT INTO tbl_5_q VALUES('qi''shi''ye''dan''wei','qsydw','企事业单位',600);",
    );
    let mut narrow = fixture.session();
    type_text(&mut narrow, "qishi");
    let first_page = &words(&narrow)[..DEFAULT_FIRST_PAGE_ROWS];
    assert_eq!(
        first_page,
        ["其实", "歧视", "岂是", "气势", "启示", "骑士"],
        "每页 6 行时补全词占进了首页: {:?}",
        words(&narrow)
    );

    let mut wide = fixture.session_with(|options| options.first_page_rows = 8);
    type_text(&mut wide, "qishi");
    assert_eq!(
        index_of(&wide, "企事业单位"),
        1,
        "每页 8 行时补全词应按权重并入: {:?}",
        words(&wide)
    );
}

#[test]
fn a_zero_first_page_rows_reads_as_one_row() {
    // `QueryRequest` 派生 `Default`，裸 `usize` 会默认成 0，而 0 会让 `exact_rows < 首页行数` 永远不成立，
    // 与全键盘「0 当 1」的做法相反。归一只在 `FirstPageRows::new` 做一次，两个键盘都从这里过。
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_q(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
         INSERT INTO tbl_2_q VALUES('qi''shi''yuan','qsy','歧视源',100);\
         CREATE TABLE tbl_5_q(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
         INSERT INTO tbl_5_q VALUES('qi''shi''ye''dan''wei','qsydw','企事业单位',600);",
    );
    let mut zero = fixture.session_with(|options| options.first_page_rows = 0);
    type_text(&mut zero, "qishi");
    let mut one = fixture.session_with(|options| options.first_page_rows = 1);
    type_text(&mut one, "qishi");
    assert_eq!(words(&zero), words(&one), "首页行数 0 没有按 1 处理");
    assert_eq!(
        index_of(&one, "企事业单位"),
        0,
        "精确同音词一行都没有时补全词应按权重排在最前: {:?}",
        words(&one)
    );
}

// ---- 引号与方头括号样式 ----

const CORNER_HOLLOW: crate::punctuation::PunctuationMarks = crate::punctuation::PunctuationMarks {
    quotes: crate::punctuation::QuoteMarks::Corner,
    brackets: crate::punctuation::BracketMarks::Hollow,
    enumeration_comma_backslash: true,
    enumeration_comma_slash: false,
};

#[test]
fn punctuation_marks_reach_the_bare_prefix_route() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.punctuation_marks = CORNER_HOLLOW;
        options.local_modes.command = true;
    });
    session.character(b'/', false);
    assert_eq!(session.punctuation(b'\'').commit.as_deref(), Some("/「"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert_eq!(session.punctuation(b'\'').commit.as_deref(), Some("」"));
    assert_eq!(session.punctuation(b'[').commit.as_deref(), Some("〖"));
    // 组字后带标点也按样式出字。
    type_text(&mut session, "nihao");
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("你好『"));
}

#[test]
fn punctuation_marks_follow_every_scheme_with_chinese_punctuation() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut japanese = fixture.session_with(|options| options.punctuation_marks = CORNER_HOLLOW);
    japanese.switch_scheme(SchemeType::JapaneseRomaji).unwrap();
    assert_eq!(japanese.punctuation(b'\'').commit.as_deref(), Some("「"));
    assert_eq!(japanese.punctuation(b']').commit.as_deref(), Some("〗"));

    let mut cantonese = fixture.session_with(|options| {
        options.punctuation_marks = CORNER_HOLLOW;
        options.scheme = SchemeType::Cantonese;
        options.cantonese_dictionary = cantonese_dictionary(fixture.path());
    });
    // 粤拼空闲时 `'` 是标点，不是音节分隔。
    assert_eq!(cantonese.punctuation(b'\'').commit.as_deref(), Some("「"));
    assert_eq!(cantonese.punctuation(b'"').commit.as_deref(), Some("『"));

    // 韩文写半角 ASCII，样式不起作用。
    let mut korean = fixture.session_with(|options| {
        options.punctuation_marks = CORNER_HOLLOW;
        options.scheme = SchemeType::Korean;
    });
    for mark in *b"'\"[]" {
        let idle = korean.punctuation(mark);
        assert!(!idle.handled && idle.commit.is_none());
    }
}

#[test]
fn zhuyin_square_keys_keep_their_own_corner_quotes() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Zhuyin;
        options.zhuyin_dictionary = zhuyin_dictionary(fixture.path());
        options.punctuation_marks = CORNER_HOLLOW;
    });
    // 大千键位自己占着 `[` `]`，出「」，不经过标点样式。
    assert_eq!(session.character(b'[', false).commit.as_deref(), Some("「"));
    assert_eq!(session.character(b']', false).commit.as_deref(), Some("」"));
    // `'` 不是注音键，照样按样式出字。
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("『"));
}

#[test]
fn set_punctuation_marks_applies_live_without_resetting_alternation() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("“"));
    session.set_punctuation_marks(CORNER_HOLLOW);
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("』"));
    assert_eq!(session.punctuation(b'[').commit.as_deref(), Some("〖"));
    session.set_punctuation_marks(crate::punctuation::PunctuationMarks::default());
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("“"));
    assert_eq!(session.punctuation(b'[').commit.as_deref(), Some("【"));
}

#[test]
fn idle_backspace_undoes_a_deleted_quote_but_not_an_older_one() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("“"));
    assert!(!session.command(Command::Backspace).handled);
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("“"));

    assert_eq!(session.punctuation(b':').commit.as_deref(), Some("："));
    assert!(!session.command(Command::Backspace).handled);
    assert_eq!(session.punctuation(b'"').commit.as_deref(), Some("”"));
}

/// #7616：已经透传给宿主的大写串不会中途进入快捷模式；未启用的 Shift 入口则在同一拼音组字里继续。
#[test]
fn only_the_first_letter_of_an_uppercase_run_opens_a_shortcut() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.enabled_schemes = SchemeSet::of(&[
            SchemeType::Quanpin,
            SchemeType::Shuangpin,
            SchemeType::JapaneseRomaji,
        ]);
        options.local_modes.temporary_japanese = true;
        // 关闭颜文字后，Shift+M 应进入普通拼音组字。
        options.local_modes.kaomoji = false;
    });

    assert!(session.character(b'M', true).handled);
    assert!(session.character(b'R', true).handled);
    assert!(session.character(b'E', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("MRE")
    );

    // 非 Shift 大写先透传，后续入口键继续交给宿主。
    assert!(!session.character(b'M', false).handled);
    // 临时日文的 `R` 和表情的 `E` 都是入口键，但都在 `M` 之后，所以照常当大写字母交给宿主。
    assert!(!session.character(b'R', true).handled);
    assert!(!session.character(b'E', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // 换应用、失焦、上屏这类边界之后这一串结束，入口键照旧进模式。
    session.reset_context();
    assert!(session.character(b'R', true).handled);
    assert_eq!(
        session.snapshot().local_mode,
        LocalInputMode::TemporaryJapanese
    );
    assert!(session.command(Command::Cancel).handled);

    // 命令同样结束这一串：回车之后的第一枚字母重新算第一枚。
    assert!(!session.character(b'M', false).handled);
    assert!(!session.command(Command::CommitRaw).handled);
    assert!(session.character(b'E', true).handled);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Emoji);
}

/// #7616：本地模式里回车上屏的是屏幕上的预编辑原文，入口字母算在内；只有入口字母时就是它自己。以前两个临时模式回车会把入口字母当标记丢掉，空输入时什么都不上屏。
#[test]
fn enter_commits_the_local_preedit_with_its_entry_letter() {
    let enters = |letter: u8, typed: &str| {
        let fixture = Fixture::new(QUANPIN_FIXTURE);
        let mut session = fixture.session_with(|options| {
            options.enabled_schemes = SchemeSet::of(&[
                SchemeType::Quanpin,
                SchemeType::Shuangpin,
                SchemeType::JapaneseRomaji,
            ]);
            options.local_modes.chaizi = true;
        });
        assert!(session.character(letter, true).handled);
        type_text(&mut session, typed);
        session.command(Command::CommitRaw).commit
    };

    assert_eq!(enters(b'R', "").as_deref(), Some("R"));
    assert_eq!(enters(b'R', "ka").as_deref(), Some("Rka"));
    assert_eq!(enters(b'Y', "").as_deref(), Some("Y"));
    assert_eq!(enters(b'Y', "he").as_deref(), Some("Yhe"));
    // 其它本地模式本来就是原样上屏含入口字母，这条规则没有改变它们。
    assert_eq!(enters(b'T', "rq").as_deref(), Some("Trq"));
    assert_eq!(enters(b'C', "ab").as_deref(), Some("Cab"));
}

/// #7616：宿主结束组字那条路（`Action::Finish`、失焦结束、macOS 的 `Cmd+A` 走 `MSIME_FINISH_COMPOSITION`）也要结束大写串。它调用 `InputSession::finish`，不经过 `handle_command`；runtime 的字面标点路由（`Action::PunctuationAscii`，runtime.rs:2226 与 2416）同样调 `Session::finish`，所以这里直接覆盖这个入口。
#[test]
fn a_host_finish_ends_the_uppercase_run() {
    let setup = || {
        let fixture = Fixture::new(QUANPIN_FIXTURE);
        fixture.session_with(|options| {
            options.enabled_schemes = SchemeSet::of(&[
                SchemeType::Quanpin,
                SchemeType::Shuangpin,
                SchemeType::JapaneseRomaji,
            ]);
            // 非 Shift 大写交还宿主，用它建立一串已透传的大写。
            options.local_modes.kaomoji = false;
            options.local_modes.temporary_english = true;
        })
    };

    let mut session = setup();
    assert!(!session.character(b'M', false).handled);
    session.finish(0);
    assert!(session.character(b'Y', true).handled);
    assert_eq!(
        session.snapshot().local_mode,
        LocalInputMode::TemporaryEnglish
    );

    // 引擎侧的标点路由本来就结束这一串，一并钉住。
    let mut session = setup();
    assert!(!session.character(b'M', false).handled);
    session.punctuation(b'.');
    assert!(session.character(b'Y', true).handled);
    assert_eq!(
        session.snapshot().local_mode,
        LocalInputMode::TemporaryEnglish
    );
}

#[test]
fn contacts_lead_with_name_and_combination_without_learning() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (scheme, input) in [
        (SchemeType::Quanpin, "nihao"),
        (SchemeType::Quanpin, "ni'hao"),
        (SchemeType::Quanpin, "nh"),
        (SchemeType::Shuangpin, "nihc"),
    ] {
        for slot in 0..2 {
            let mut session = fixture.session_with(|options| {
                options.scheme = scheme;
                options.frequency.mode = FrequencyAdjustmentMode::Promote;
                options.mention_entries = vec![crate::types::MentionEntry {
                    text: "拟好".into(),
                    key: "ni'hao".into(),
                    phone: "202-555-0101".into(),
                }];
            });
            session.set_cloud_candidates_enabled(true);
            type_text(&mut session, input);
            let expected = ["拟好", "拟好 202-555-0101"];
            let view = session.snapshot();
            assert_eq!(&words(&session)[..2], expected, "{scheme:?}: {input}");
            assert!(view.candidate_sources[..2]
                .iter()
                .all(|source| *source == CandidateSource::Contact));
            assert!(view.candidate_answers_key[..2].iter().all(|answer| !answer));
            assert_eq!(
                words(&session)
                    .iter()
                    .filter(|word| *word == "拟好")
                    .count(),
                1
            );
            if let Some(query) = session.online_query() {
                session.apply_online_candidate(
                    &query,
                    "合成云词",
                    CandidateSource::CloudSuggestion,
                );
                assert_eq!(&words(&session)[..2], expected);
            }
            assert_eq!(session.select(slot).commit.as_deref(), Some(expected[slot]));
            assert!(session.snapshot().editing_text.is_empty());
            assert!(!fixture.journal().exists(), "联系人选择不得写入个人词库");
        }
    }
}

#[test]
fn contacts_refresh_live_and_do_not_replace_caret_prefixes() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    session.set_mention_entries(&[crate::types::MentionEntry {
        text: "拟好".into(),
        key: "ni'hao".into(),
        phone: "202-555-0102".into(),
    }]);
    assert_eq!(&words(&session)[..2], ["拟好", "拟好 202-555-0102"]);
    session.set_caret(Some(2));
    assert!(!session
        .snapshot()
        .candidate_sources
        .contains(&CandidateSource::Contact));
    session.set_caret(None);
    assert_eq!(words(&session)[0], "拟好");
    session.set_mention_entries(&[]);
    assert!(!session
        .snapshot()
        .candidate_sources
        .contains(&CandidateSource::Contact));
    assert_eq!(words(&session)[0], "你好");
}

#[test]
fn literal_symbol_keeps_pinyin_uncommitted_with_one_raw_candidate() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        for prefix in ["C", "Something", "nihao"] {
            for symbol in *b"#$%&" {
                for punctuation_route in [false, true] {
                    let mut session = fixture.session_with(|options| {
                        options.scheme = scheme;
                        // `S` 是已启用的符号入口；本用例验证普通英文字母串，模式自身内容另测。
                        options.local_modes.symbol = false;
                    });
                    for value in prefix.bytes() {
                        assert!(session.character(value, value.is_ascii_uppercase()).handled);
                    }
                    let result = if punctuation_route {
                        session.punctuation(symbol)
                    } else {
                        session.character(symbol, false)
                    };
                    assert!(
                        result.handled && result.commit.is_none(),
                        "{prefix} {symbol} {result:?}"
                    );
                    let expected = format!("{prefix}{}", char::from(symbol));
                    assert_eq!(session.snapshot().editing_text, expected);
                    assert_eq!(words(&session), vec![expected.clone()]);
                    assert_eq!(session.select(0).commit.as_deref(), Some(expected.as_str()));
                    assert!(session.snapshot().editing_text.is_empty());
                }
            }
        }
    }
}

#[test]
fn literal_symbol_deletion_and_caret_editing_restore_pinyin() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for (scheme, raw) in [
        (SchemeType::Quanpin, "nihao"),
        (SchemeType::Shuangpin, "nihc"),
    ] {
        let mut session = fixture.session_with(|options| options.scheme = scheme);
        type_text(&mut session, raw);
        assert!(session.character(b'#', false).handled);
        assert_eq!(words(&session), vec![format!("{raw}#")]);
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, raw);
        assert_eq!(words(&session)[0], "你好");
        session.command(Command::MoveHome);
        assert!(session.punctuation(b'#').commit.is_none());
        assert_eq!(session.snapshot().editing_text, format!("#{raw}"));
        session.command(Command::MoveHome);
        session.command(Command::DeleteForward);
        session.command(Command::MoveEnd);
        assert_eq!(words(&session)[0], "你好");
    }
}

#[test]
fn literal_symbol_stops_chinese_queries_and_does_not_learn() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let query = session.online_query().unwrap();
    assert!(session.character(b'#', false).handled);
    type_text(&mut session, "nihao");
    assert_eq!(words(&session), vec!["nihao#nihao"]);
    assert!(session.online_query().is_none());
    assert!(!session.apply_online_candidate(
        &query,
        "合成旧响应",
        CandidateSource::CloudSuggestion
    ));
    let journal_before = journal_rows(&fixture.journal());
    assert!(session.snapshot().candidate_answers_key[0]);
    assert!(!session.pin(0).handled);
    assert!(!session.remove(0).handled);
    assert_eq!(session.finish(0).commit.as_deref(), Some("nihao#nihao"));
    assert_eq!(journal_rows(&fixture.journal()), journal_before);
}

#[test]
fn literal_symbol_keeps_mode_content_and_sentence_punctuation_boundaries() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    for initial in *b"UCS" {
        let mut session = fixture.session_with(|options| options.local_modes.chaizi = true);
        assert!(session.character(initial, true).handled);
        assert_ne!(session.snapshot().local_mode, LocalInputMode::None);
        let symbol = session.character(b'#', false);
        assert!(symbol.handled && symbol.commit.is_none());
        assert_eq!(session.snapshot().local_mode, LocalInputMode::None);
        assert_eq!(words(&session), vec![format!("{}#", char::from(initial))]);
    }
    let mut session = fixture.session();
    assert!(!session.character(b'#', false).handled);
    session.character(b'C', true);
    session.character(b'#', false);
    type_text(&mut session, "nihao");
    assert_eq!(
        session.punctuation(b',').commit.as_deref(),
        Some("C#nihao，")
    );
    session.character(b'U', true);
    type_text(&mut session, "4e2d");
    assert!(session.character(b'#', false).commit.is_some());
    session.command(Command::Cancel);
    type_text(&mut session, "nihao");
    assert_eq!(session.punctuation(b'.').commit.as_deref(), Some("你好。"));
}

#[test]
fn custom_helpcode_trigger_filters_both_pinyin_schemes_like_shift() {
    let fixture = isolation_root("你", "甲");
    std::fs::write(
        fixture.path().join("helpcodes/helpcode.txt"),
        "你=rx\n拟=fr\n",
    )
    .unwrap();
    for (scheme, profile) in [
        (SchemeType::Quanpin, ShuangpinProfileKind::Xiaohe),
        (SchemeType::Shuangpin, ShuangpinProfileKind::Xiaohe),
        (SchemeType::Shuangpin, ShuangpinProfileKind::Ziranma),
        (SchemeType::Shuangpin, ShuangpinProfileKind::Shoudao),
        (SchemeType::Shuangpin, ShuangpinProfileKind::Microsoft),
        (SchemeType::Shuangpin, ShuangpinProfileKind::SmartAbc),
    ] {
        for trigger in b";/\\`=" {
            let mut custom = fixture.session_with(|options| {
                options.scheme = scheme;
                options.shuangpin_profile = profile;
                options.helpcode_trigger_key = Some(*trigger);
            });
            let mut shifted = fixture.session_with(|options| {
                options.scheme = scheme;
                options.shuangpin_profile = profile;
            });
            type_text(&mut custom, "ni");
            type_text(&mut shifted, "ni");
            assert!(custom
                .snapshot()
                .spelling_symbols
                .contains(*trigger as char));
            assert!(custom.character(*trigger, false).handled);
            assert_eq!(custom.snapshot().editing_text, "ni");
            for (lower, upper) in [(b'f', b'F'), (b'r', b'R')] {
                assert!(custom.character(lower, false).handled);
                if scheme == SchemeType::Shuangpin && lower == b'r' {
                    // 双拼的顺序双码是小写首码加大写次码。
                    shifted.command(Command::Backspace);
                    shifted.character(b'f', false);
                }
                assert!(shifted.character(upper, true).handled);
                assert_eq!(
                    custom.snapshot().editing_text,
                    shifted.snapshot().editing_text
                );
                assert_eq!(words(&custom), words(&shifted));
                assert_eq!(words(&custom)[0], "拟");
            }
            assert!(!words(&custom).contains(&"你".to_owned()));
            assert_eq!(custom.select(0).commit.as_deref(), Some("拟"));
            type_text(&mut custom, "ni");
            assert_eq!(custom.snapshot().editing_text, "ni");
            // 尚未输入码时选词也必须结束前导状态。
            assert!(custom.character(*trigger, false).handled);
            assert_eq!(
                custom.select(index_of(&custom, "你")).commit.as_deref(),
                Some("你")
            );
        }
    }
}

#[test]
fn custom_helpcode_trigger_backspace_and_exit_boundaries() {
    let fixture = isolation_root("你", "甲");
    std::fs::write(
        fixture.path().join("helpcodes/helpcode.txt"),
        "你=rx\n拟=fr\n",
    )
    .unwrap();
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode_trigger_key = Some(b';');
        });
        type_text(&mut session, "ni;");
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "ni");
        assert!(session.input.helpcode_entry.is_none());
        type_text(&mut session, ";fr");
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "niF");
        assert_eq!(words(&session)[0], "拟");
        session.character(b'r', false);
        assert_eq!(words(&session)[0], "拟");
        assert_eq!(
            session.snapshot().editing_text,
            if scheme == SchemeType::Shuangpin {
                "nifR"
            } else {
                "niFR"
            }
        );
        session.command(Command::Backspace);
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "ni");
        assert!(session.input.helpcode_entry.is_some());
        session.command(Command::Backspace);
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "n");
        session.command(Command::Cancel);
        for command in [
            Command::MoveLeft,
            Command::MoveHome,
            Command::CommitRaw,
            Command::Cancel,
        ] {
            type_text(&mut session, "ni;");
            session.command(command);
            assert!(session.input.helpcode_entry.is_none(), "{command:?}");
            session.command(Command::Cancel);
        }
        type_text(&mut session, "ni;");
        session.set_caret(None);
        session.character(b'c', false);
        assert_eq!(session.snapshot().editing_text, "nic");
        session.command(Command::Cancel);
        type_text(&mut session, "ni;");
        session.reset_context();
        session.character(b'c', false);
        assert_eq!(session.snapshot().editing_text, "nic");
        session.command(Command::Cancel);
        type_text(&mut session, "ni;");
        session.set_helpcode_enabled(false);
        assert!(session.input.helpcode_entry.is_none());
        assert!(!session.snapshot().spelling_symbols.contains(';'));
        session.character(b'c', false);
        assert_eq!(session.snapshot().editing_text, "nic");
    }
}

#[test]
fn custom_helpcode_trigger_preserves_microsoft_final_and_other_input_modes() {
    let fixture = isolation_root("你", "甲");
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Shuangpin;
        options.shuangpin_profile = ShuangpinProfileKind::Microsoft;
        options.local_modes.command = true;
        options.helpcode_trigger_key = Some(b';');
    });
    type_text(&mut session, "n;");
    assert_eq!(session.snapshot().editing_text, "n;");
    assert!(session.input.helpcode_entry.is_none());
    session.character(b';', false);
    assert!(session.input.helpcode_entry.is_some());
    session.command(Command::Cancel);
    type_text(&mut session, "ni'n;");
    assert!(session.input.helpcode_entry.is_none());
    session.command(Command::Cancel);
    session.switch_scheme(SchemeType::Quanpin).unwrap();
    assert!(!session.character(b';', false).handled);
    type_text(&mut session, "ni");
    assert!(session.snapshot().spelling_symbols.contains('@'));
    session.command(Command::MoveLeft);
    assert!(!session.snapshot().spelling_symbols.contains(';'));
    session.command(Command::Cancel);
    type_text(&mut session, "ni;");
    session.switch_scheme(SchemeType::Shuangpin).unwrap();
    type_text(&mut session, "ni");
    assert_eq!(session.snapshot().editing_text, "ni");
    session.command(Command::Cancel);
    session.set_dedicated_english(true);
    type_text(&mut session, "ni;");
    assert_eq!(session.snapshot().editing_text, "ni");
    assert!(session.input.helpcode_entry.is_none());
    session.set_dedicated_english(false);
    type_text(&mut session, "/");
    session.character(b';', false);
    assert!(session.input.helpcode_entry.is_none());
}

// 顿号开关不能抢占词首的命令模式；组字和裸前缀提交仍按相同的标点表出字。
#[test]
fn enumeration_comma_respects_command_prefix_and_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.local_modes.command = true;
        options.punctuation_marks.enumeration_comma_slash = true;
    });
    session.character(b'/', false);
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Command);
    assert_eq!(session.punctuation(b',').commit.as_deref(), Some("、，"));
    type_text(&mut session, "nihao");
    assert_eq!(session.punctuation(b'/').commit.as_deref(), Some("你好、"));
    let mut disabled = fixture.session_with(|options| {
        options.local_modes.command = false;
        options.punctuation_marks.enumeration_comma_slash = true;
    });
    assert_eq!(disabled.punctuation(b'/').commit.as_deref(), Some("、"));
}

/// 相对日期复用普通候选与九键出口，整段提交不留下原始拼写，也不学习生成文字。
#[test]
fn relative_date_keywords_reach_quanpin_and_nine_key_and_commit() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
        INSERT INTO tbl_2_j VALUES('jin''tian','jt','今天',500),('jin''nian','jn','今年',500);\
        CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
        INSERT INTO tbl_2_z VALUES('zuo''tian','zt','昨天',500);\
        CREATE TABLE tbl_2_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
        INSERT INTO tbl_2_q VALUES('qian''tian','qt','前天',500),('qu''nian','qn','去年',500);\
        CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
        INSERT INTO tbl_2_m VALUES('ming''tian','mt','明天',500),('ming''nian','mn','明年',500);\
        CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
        INSERT INTO tbl_2_h VALUES('hou''tian','ht','后天',500),('hou''nian','hn','后年',500);\
        CREATE TABLE tbl_2_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
        INSERT INTO tbl_2_l VALUES('lai''nian','ln','来年',500);\
        CREATE TABLE tbl_2_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
        INSERT INTO tbl_2_r VALUES('ri''qi','rq','日期',500);",
    );
    let cases = [
        ("riqi", "日期", "今天(8月9日 星期日)"),
        ("jintian", "今天", "今天(8月9日 星期日)"),
        ("zuotian", "昨天", "昨天(8月8日 星期六)"),
        ("qiantian", "前天", "前天(8月7日 星期五)"),
        ("mingtian", "明天", "明天(8月10日 星期一)"),
        ("houtian", "后天", "后天(8月11日 星期二)"),
        ("mingnian", "明年", "明年(2027年)"),
        ("qunian", "去年", "去年(2025年)"),
        ("jinnian", "今年", "今年(2026年)"),
        ("lainian", "来年", "来年(2027年)"),
        ("hounian", "后年", "后年(2028年)"),
    ];
    for nine_key in [false, true] {
        let mut session = fixture.session_with(|options| options.learning = true);
        TestClock::install(&mut session);
        session.set_nine_key_enabled(nine_key);
        for (keyword, anchor, expected) in cases {
            let typed: String = if nine_key {
                keyword
                    .bytes()
                    .map(|letter| match letter {
                        b'a'..=b'c' => '2',
                        b'd'..=b'f' => '3',
                        b'g'..=b'i' => '4',
                        b'j'..=b'l' => '5',
                        b'm'..=b'o' => '6',
                        b'p'..=b's' => '7',
                        b't'..=b'v' => '8',
                        _ => '9',
                    })
                    .collect()
            } else {
                keyword.to_owned()
            };
            type_text(&mut session, &typed);
            let rows = words(&session);
            assert!(
                rows.contains(&anchor.to_owned()),
                "{nine_key} {keyword}: {rows:?}"
            );
            assert!(
                rows.contains(&expected.to_owned()),
                "{nine_key} {keyword}: {rows:?}"
            );
            if !nine_key && keyword == "jintian" {
                assert!(select_word(&mut session, "📅日期").commit.is_none());
                assert!(words(&session).contains(&expected.to_owned()));
                assert_eq!(words(&session).len(), 20);
            }
            assert_eq!(
                select_word(&mut session, expected).commit.as_deref(),
                Some(expected)
            );
            assert!(session.snapshot().editing_text.is_empty());
            assert!(session.snapshot().candidates.is_empty());
        }
        assert_eq!(
            count(
                &fixture.journal(),
                "SELECT count(*) FROM user_dictionary_operations"
            ),
            0
        );
    }
    let mut disabled = fixture.session_with(|options| options.local_modes.date_time_inline = false);
    TestClock::install(&mut disabled);
    type_text(&mut disabled, "jintian");
    assert!(!words(&disabled)
        .iter()
        .any(|word| word.contains("2026") || word.contains('(')));
}

/// 十八键的两个字母都参与全拼解码；实体字母仍走确定拼写，组码本身不作为文字上屏。
#[test]
fn eighteen_key_uses_ambiguous_groups_and_preserves_exact_hardware_input() {
    let sql = format!(
        "{QUANPIN_FIXTURE}
        CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_b VALUES('bo','b','波',150);
        INSERT INTO tbl_2_b VALUES('bo''hao','bh','波好',150);"
    );
    let fixture = Fixture::new(&sql);
    let mut session = fixture.session();
    session.set_key_grid(Some(KeyGrid::EighteenKey));
    type_text(&mut session, "nihao");
    assert_eq!(session.snapshot().editing_text, "nihao");
    assert!(!words(&session).contains(&"波好".into()));
    assert!(!session.grid_key(b'n').handled);
    session.command(Command::Cancel);

    for letter in b"nihao" {
        assert!(session.grid_key(*letter).handled);
    }
    assert_eq!(session.snapshot().editing_text, "bihai");
    let ambiguous = words(&session);
    assert!(ambiguous.contains(&"你好".into()), "{ambiguous:?}");
    assert!(ambiguous.contains(&"波好".into()), "{ambiguous:?}");
    assert!(!session.grid_key(b'1').handled);
    assert!(!session.grid_key(b'A').handled);
    assert_eq!(
        select_word(&mut session, "波好").commit.as_deref(),
        Some("波好")
    );

    for letter in b"ni" {
        assert!(session.grid_key(*letter).handled);
    }
    let spelling = session
        .snapshot()
        .nine_key_spellings
        .iter()
        .position(|item| item == "ni")
        .unwrap();
    assert!(session.choose_nine_key_spelling(spelling).handled);
    for letter in b"hao" {
        assert!(session.grid_key(*letter).handled);
    }
    assert!(!words(&session).contains(&"波好".into()));
    assert!(session.command(Command::Backspace).handled);
    assert_eq!(session.snapshot().editing_text, "biha");
    assert!(session.grid_key(b'o').handled);
    let expected = session.snapshot().candidates[0].word.clone();
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some(expected.as_str())
    );
    assert!(session.snapshot().editing_text.is_empty());
    session.set_dedicated_english(true);
    assert!(!session.grid_key(b'n').handled);
}

/// 五笔 14 键：拼音主词库给词频，码表与它同在一个工作副本里。
const WUBI_GRID_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',3000000);\
CREATE TABLE tbl_1_d(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_d VALUES('duo','d','多',1500000);\
CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_c VALUES('cong','c','从',1400000);\
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wo','w','我',9000000);\
CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO wubi86 VALUES('q','我',20),('w','人',20),('qq','多',20),('qw','欠',10),('wq','你',10),('ww','从',10),\
('wqiy','你',20),('qqiy','铄',20),('x','经',20),('xxxx','纟',10);";

fn wubi_grid_session(fixture: &Fixture, configure: impl FnOnce(&mut SessionOptions)) -> Session {
    let mut session = fixture.session_with(|options| {
        options.scheme = SchemeType::Wubi;
        configure(options);
    });
    session.set_key_grid(Some(KeyGrid::FourteenKey));
    session
}

fn grid_type(session: &mut Session, letters: &[u8]) {
    for &letter in letters {
        assert!(
            session.grid_key(letter).handled,
            "{:?} was not handled",
            char::from(letter)
        );
    }
}

#[test]
fn wubi_fourteen_key_expands_each_key_into_its_letters() {
    let fixture = Fixture::new(WUBI_GRID_FIXTURE);
    let mut session = wubi_grid_session(&fixture, |_| {});
    // 宿主送这一组的首字母；W 也归 QW 组。
    grid_type(&mut session, b"qw");
    assert!(session.grid_composing());
    let snapshot = session.snapshot();
    assert_eq!(
        words(&session)[..4],
        [
            "你".to_owned(),
            "多".to_owned(),
            "从".to_owned(),
            "欠".to_owned()
        ]
    );
    // 组码是每组的代表字母；预编辑和读音行显示首选的编码。
    assert_eq!(snapshot.editing_text, "qq");
    assert_eq!(snapshot.preedit, "wq");
    assert_eq!(snapshot.nine_key_reading, "wq");
    assert!(snapshot.nine_key_spellings.is_empty());
    assert_eq!(snapshot.scheme, SchemeType::Wubi);
    // 选词照常是五笔的选词。
    let result = session.select(0);
    assert_eq!(result.commit.as_deref(), Some("你"));
    assert!(!session.grid_composing());
    assert!(session.snapshot().editing_text.is_empty());
}

#[test]
fn wubi_fourteen_key_keeps_hardware_letters_apart() {
    let fixture = Fixture::new(WUBI_GRID_FIXTURE);
    let mut session = wubi_grid_session(&fixture, |_| {});
    grid_type(&mut session, b"q");
    // 14 键组字中硬件键盘的字母不处理，组字不变。
    assert!(!session.character(b'w', false).handled);
    assert_eq!(session.snapshot().editing_text, "q");
    // 退格到空，硬件字母照常打 26 键的码，这时 14 键的键不处理。
    assert!(session.command(Command::Backspace).handled);
    assert!(!session.grid_composing());
    type_text(&mut session, "w");
    assert_eq!(words(&session).first().map(String::as_str), Some("人"));
    assert!(!session.grid_composing());
    assert!(!session.grid_key(b'q').handled);
    assert_eq!(session.snapshot().editing_text, "w");
    // 14 键组码没有可落脚的光标。
    session.command(Command::Cancel);
    grid_type(&mut session, b"qq");
    session.set_caret(Some(0));
    assert_eq!(session.snapshot().caret_position, 2);
}

#[test]
fn wubi_fourteen_key_stops_at_four_codes_and_ignores_mixed_pinyin() {
    let fixture = Fixture::new(WUBI_GRID_FIXTURE);
    let mut session = wubi_grid_session(&fixture, |options| {
        options.wubi.mixed_pinyin = true;
        options.wubi.z_wildcard = true;
    });
    // W Q I Y：QW QW UI TY，四码只有「你」「铄」两个字。
    grid_type(&mut session, b"wqiy");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "qqut");
    assert!(snapshot
        .candidates
        .iter()
        .all(|item| item.scheme == SchemeType::Wubi));
    assert!(!snapshot.answered_by_pinyin_fallback);
    assert!(!snapshot.wubi_unique_four_code);
    // 第五键由 runtime 顶字；引擎自己不收。
    assert!(!session.grid_key(b'q').handled);
    assert_eq!(session.snapshot().editing_text, "qqut");

    // ZX 键代表 x，四码唯一时报告给 runtime 自动上屏。
    session.command(Command::Cancel);
    grid_type(&mut session, b"zxzx");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.editing_text, "xxxx");
    assert_eq!(words(&session), ["纟"]);
    assert!(snapshot.wubi_unique_four_code);
}

#[test]
fn wubi_fourteen_key_needs_the_wubi_scheme_and_plain_input() {
    let fixture = Fixture::new(WUBI_GRID_FIXTURE);
    let mut session = wubi_grid_session(&fixture, |_| {});
    session.set_dedicated_english(true);
    assert!(!session.grid_key(b'q').handled);
    session.set_dedicated_english(false);
    // 没有 14 键网格时不处理。
    session.set_key_grid(None);
    assert!(!session.grid_key(b'q').handled);
    // 全拼 14 键照旧走九宫格会话。
    let mut quanpin = fixture.session();
    quanpin.set_key_grid(Some(KeyGrid::FourteenKey));
    assert!(quanpin.grid_key(b'n').handled);
    assert!(!quanpin.input.wubi_grid_composing());
}

/// 快捷短语按用户打出的字母对码；五笔 14 键的组码（`qq`）不是这样的字母，不拿它去对。26 键照旧。
#[test]
fn wubi_fourteen_key_codes_do_not_match_quick_phrases() {
    let fixture = Fixture::new(&format!(
        "{WUBI_GRID_FIXTURE}CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);\
INSERT INTO quick_parases VALUES('qq','腾讯',20);"
    ));
    let mut session = wubi_grid_session(&fixture, |options| {
        options.quick_phrase_candidates = true;
    });
    grid_type(&mut session, b"qq");
    assert!(!words(&session).contains(&"腾讯".to_owned()));
    session.command(Command::Cancel);
    type_text(&mut session, "qq");
    assert!(words(&session).contains(&"腾讯".to_owned()));
}

mod caret_suffix;
mod lifecycle;
mod raw_commit;
