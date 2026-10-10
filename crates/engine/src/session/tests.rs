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
};

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

/// One directory standing in for all four runtime roots, as the reference session tests used.
struct Fixture {
    directory: tempfile::TempDir,
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
        .map(|item| item.word)
        .collect()
}

fn index_of(session: &Session, word: &str) -> usize {
    words(session)
        .iter()
        .position(|candidate| candidate == word)
        .unwrap_or_else(|| panic!("{word} is not a candidate of {:?}", words(session)))
}

fn select_word(session: &mut Session, word: &str) -> crate::types::KeyResult {
    let index = index_of(session, word);
    session.select(index)
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
            local: Box::new(|| LocalDateTime {
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
        .map(|item| item.word)
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
    assert_eq!(
        allocations, 353,
        "caret-prefix expansion allocations: {allocations}"
    );
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
        .map(|item| (item.word.clone(), item.scheme))
        .collect();
    assert_eq!(schemes.first(), Some(&("工".to_owned(), SchemeType::Wubi)));
    assert!(schemes.contains(&("哥哥".to_owned(), SchemeType::Quanpin)));
    assert!(schemes.contains(&("个".to_owned(), SchemeType::Quanpin)));
    assert!(!snapshot.answered_by_pinyin_fallback);
    // The quanpin rows beside the one wubi row keep the code open: the fourth key must not commit 工 over 哥哥 for someone typing pinyin.
    assert!(!snapshot.wubi_unique_four_code);
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
    assert_eq!(allocations, 32);
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
    assert_eq!(allocations, 132);
}

#[test]
fn backspacing_at_the_end_does_not_build_the_preedit_twice_for_caret_detection() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "ni");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.command(Command::Backspace));

    assert!(result.handled);
    assert_eq!(allocations, 23);
}

#[test]
fn url_entry_readiness_at_the_end_does_not_build_the_preedit_twice() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.character(b'.', false));

    assert!(result.handled);
    assert_eq!(allocations, 4);
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
    assert_eq!(allocations, 29);
}

#[test]
fn moving_the_caret_does_not_build_the_preedit_twice() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");

    let (result, allocations) =
        crate::ime::personal_rerank::allocations::count(|| session.command(Command::MoveLeft));

    assert!(result.handled);
    assert_eq!(
        allocations, 29,
        "caret movement should reuse the editing text length: {allocations} allocations"
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
    assert_eq!(
        allocations, 207,
        "caret insertion allocations: {allocations}"
    );
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
    assert_eq!(allocations, 17, "候选选择分配次数：{allocations}");
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
fn selecting_a_candidate_does_not_clone_unused_row_metadata() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.learning = false;
        options.personal_context = false;
    });
    type_text(&mut session, "nihao");
    let index = index_of(&session, "你好");
    session.input.mixed_candidates[index].corrected_from = "synthetic correction".to_owned();
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
    assert_eq!(allocations, 26, "双拼候选选择分配次数：{allocations}");
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
    type_text(&mut session, "nihao");
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
    type_text(&mut session, "nihao");
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
            .map(|item| item.word.clone())
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
    let sentence = snapshot.candidates[index].word.clone();
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
    let sentence = session.snapshot().candidates[index].word.clone();
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
        .map(|item| (item.word.clone(), item.scheme))
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
        .map(|item| (item.word, item.scheme))
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
    assert!(!disabled.character(b'H', false).handled);
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

/// test_online_input_session.cpp:149-176 through the singular entry every cloud response takes: cloud at the second seat, AI at the third, a new cloud answer replaces the old one, and a word the list already holds or a non-online source is refused without touching the list.
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
        (item.word, item.source)
    };

    assert!(session.apply_online_candidate(&query, "泥", CandidateSource::CloudSuggestion));
    assert_eq!(
        at(&session, 1),
        ("泥".to_owned(), CandidateSource::CloudSuggestion)
    );
    assert!(session.apply_online_candidate(&query, "逆", CandidateSource::AiSuggestion));
    assert_eq!(
        at(&session, 2),
        ("逆".to_owned(), CandidateSource::AiSuggestion)
    );
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
        Some("he")
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
        Some("k")
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
    assert_eq!(
        allocations, 54,
        "shuangpin snapshot allocations: {allocations}"
    );
}

#[test]
fn japanese_engine_refresh_reuses_candidate_strings() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| options.scheme = SchemeType::JapaneseRomaji);
    type_text(&mut session, "ka");
    let before = session.input.engine.candidates().to_vec();
    let pointers = session
        .input
        .engine
        .candidates()
        .iter()
        .map(|row| {
            (
                row.word.as_ptr(),
                row.pinyin.as_ptr(),
                row.canonical_pinyin.as_ptr(),
            )
        })
        .collect::<Vec<_>>();
    session.input.engine.handle_key(SchemeKey::Requery);
    assert_eq!(session.input.engine.candidates(), before);
    assert_eq!(
        session
            .input
            .engine
            .candidates()
            .iter()
            .map(|row| (
                row.word.as_ptr(),
                row.pinyin.as_ptr(),
                row.canonical_pinyin.as_ptr()
            ))
            .collect::<Vec<_>>(),
        pointers
    );
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
    assert_eq!(snapshot.candidates.len(), 17);
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
fn disabled_local_modes_leave_the_key_to_the_host() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session_with(|options| {
        options.local_modes.temporary_english = false;
        options.local_modes.unicode = false;
    });
    assert!(!session.character(b'Y', true).handled);
    assert!(!session.character(b'U', true).handled);
    assert!(session.character(b'K', true).handled);
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
    let pointers = session
        .input
        .engine
        .candidates()
        .iter()
        .map(|row| (row.word.as_ptr(), row.pinyin.as_ptr()))
        .collect::<Vec<_>>();
    session.input.engine.handle_key(SchemeKey::Requery);
    assert_eq!(
        session
            .input
            .engine
            .candidates()
            .iter()
            .map(|row| (row.word.as_ptr(), row.pinyin.as_ptr()))
            .collect::<Vec<_>>(),
        pointers
    );
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
    assert_eq!(Command::from_u8(12), None);
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
                text: "张三".to_owned(),
                key: "zhang'san".to_owned(),
            },
            crate::types::MentionEntry {
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
    assert!(!session.character(b'V', true).handled);
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
    assert_eq!(snapshot.spelling_symbols, "0123456789+-*/.()%^");
    assert_eq!(words(&session), ["V"]);
    // Letters are not part of the spelling and, before any number, name no unit; the key is swallowed like in the other local modes.
    assert!(session.character(b'x', false).handled);
    assert_eq!(session.snapshot().preedit, "V");

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
fn expression_mode_converts_units_after_a_number() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'V', true);
    // An apostrophe before any unit letter is not a separator.
    assert!(session.character(b'\'', false).handled);
    type_text(&mut session, "3jin");
    // A host that reports the apostrophe as punctuation still gets the separator.
    let apostrophe = session.punctuation(b'\'');
    assert!(
        apostrophe.handled && apostrophe.commit.is_none(),
        "{apostrophe:?}"
    );
    // A second apostrophe is swallowed.
    assert!(session.character(b'\'', false).handled);
    session.character(b'g', false);
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
    for _ in 0..100 {
        session.character(b'9', false);
    }
    assert_eq!(
        session.snapshot().preedit.len(),
        crate::local::GENERATED_MODE_INPUT_LIMIT
    );
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
        ["张三 2026-08-09", "2026年8月9日", "14:30", "星期日"]
    );
    assert_eq!(
        snapshot.candidate_annotations,
        ["签名", "日期", "时间", "星期"]
    );

    type_text(&mut session, "si");
    assert_eq!(words(&session), ["张三 2026-08-09"]);
    let result = session.select(0);
    assert_eq!(result.commit.as_deref(), Some("张三 2026-08-09"));
    assert_eq!(session.snapshot().local_mode, LocalInputMode::None);

    // A complete built-in trigger lists every reading of the date/time mode.
    session.character(b'/', false);
    type_text(&mut session, "rq");
    assert_eq!(words(&session).len(), 17);

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
    assert!(session.snapshot().spelling_symbols.is_empty());
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
    // Once a letter follows, a mark is not input and the mode keeps it out.
    session.character(b'/', false);
    type_text(&mut session, "rq");
    assert!(session.character(b',', false).commit.is_none());
    assert_eq!(session.snapshot().local_mode, LocalInputMode::Command);
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
        text: "李四".to_owned(),
        key: "li'si".to_owned(),
    }]);
    assert_eq!(words(&session), ["李四"]);
    session.command(Command::Cancel);
    session.character(b'/', false);
    session.set_command_table(&[]);
    assert_eq!(words(&session), ["2026年8月9日", "14:30", "星期日"]);
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
fn the_translate_command_asks_for_its_english_and_commits_the_answer() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = generated_modes_session(&fixture);
    session.character(b'/', false);
    type_text(&mut session, "fyhello");
    // A host reporting the apostrophe as punctuation still separates the words; a second one in a row is swallowed.
    let apostrophe = session.punctuation(b'\'');
    assert!(
        apostrophe.handled && apostrophe.commit.is_none(),
        "{apostrophe:?}"
    );
    assert!(session.character(b'\'', false).handled);
    type_text(&mut session, "world");
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
            "INSERT INTO syllables VALUES ('nei'),('hou'),('ngo'),('ngoi'),('oi'),('i');\
             INSERT INTO entries VALUES ('nei hou','你好',900),('nei hou','妳好',40),('nei','你',5000),('nei','妳',300),('hou','好',4000),('hou','號',500),('ngo','我',6000),('oi','愛',2500),('ngoi','外',1000);",
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
    assert_eq!(words(&session), ["我"]);
    assert!(session.command(Command::MoveLeft).handled);
    assert!(session.command(Command::Backspace).handled);
    // `ngo i`, not the single syllable `ngoi` (外).
    assert_eq!(session.snapshot().editing_text, "ngo i");
    assert_eq!(words(&session), ["我"]);
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
    let pointers = session
        .input
        .engine
        .candidates()
        .iter()
        .map(|row| {
            (
                row.word.as_ptr(),
                row.pinyin.as_ptr(),
                row.canonical_pinyin.as_ptr(),
            )
        })
        .collect::<Vec<_>>();
    session.input.engine.handle_key(SchemeKey::Requery);
    assert_eq!(
        session
            .input
            .engine
            .candidates()
            .iter()
            .map(|row| (
                row.word.as_ptr(),
                row.pinyin.as_ptr(),
                row.canonical_pinyin.as_ptr()
            ))
            .collect::<Vec<_>>(),
        pointers
    );
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
            .map(|item| (item.word, item.scheme))
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
    assert_eq!(words(&session), ["众"]);
    assert_eq!(session.snapshot().spelling_symbols, ".");

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
        assert_eq!(session.snapshot().spelling_symbols, ":", "{scheme:?}");
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
    assert_eq!(session.snapshot().spelling_symbols, ":");
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
    assert_eq!(words(&session), ["众"]);
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
    assert_eq!(snapshot.spelling_symbols, ":");
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
    assert_eq!(snapshot.spelling_symbols, ".");
    assert_eq!(words(&session), ["众"]);
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
    assert_eq!(snapshot.spelling_symbols, ":");
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
fn url_input_stops_at_the_limit() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "www");
    assert!(session.punctuation(b'.').handled);
    for _ in 0..600 {
        assert!(session.character(b'a', false).handled);
    }
    assert_eq!(
        session.snapshot().preedit.len(),
        crate::local::url::INPUT_LIMIT
    );
}

#[test]
fn url_spelling_symbols_follow_the_composition() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    for (raw, keys) in [
        ("www", "."),
        ("ftp", ".:"),
        ("http", ":"),
        ("https", ":"),
        ("ni", ""),
        ("wwww", ""),
        // 大写的辅助码不算触发词。
        ("wwW", ""),
    ] {
        type_text(&mut session, raw);
        assert_eq!(session.snapshot().spelling_symbols, keys, "{raw}");
        session.command(Command::Cancel);
    }
    // 空闲时不变：生成类模式默认关闭，没有任何符号。
    assert!(session.snapshot().spelling_symbols.is_empty());

    // 光标不在末尾时不发布，按下的 `.` 也不进入网址模式（列出的键必须正是接受的键）。
    type_text(&mut session, "www");
    session.command(Command::MoveLeft);
    assert!(session.snapshot().spelling_symbols.is_empty());
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

/// 九宫格选中整句时存词的音节上限与全拼键盘相同（#5640）。
#[test]
fn nine_key_sentence_learning_shares_the_syllable_cap() {
    assert_eq!(
        crate::nine_key::MAX_LEARNED_SENTENCE_SYLLABLES,
        super::learning::MAX_LEARNED_SENTENCE_SYLLABLES
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
