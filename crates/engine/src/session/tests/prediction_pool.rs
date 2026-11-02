//! 上屏后联想的候选池与模型排序：打开候选池时词库续写多取，按打分器的总分重排后仍只给 8 条，个人习惯行留在前面，选中的行就是上屏的行。

use super::*;
use crate::ordering::CandidateScorer;

/// 「吃」之后有 13 条词库续写，按词频是 饭 好 喝 药 苦 亏 惊 力 素 醋 东西 醋水 醋鱼；「吃醋」之后有 水 鱼 两条。
fn pool_fixture() -> Fixture {
    Fixture::new(
        "CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_c VALUES('chi','c','吃',1000);
CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_c VALUES('chi''fan','cf','吃饭',100);
INSERT INTO tbl_2_c VALUES('chi''hao','ch','吃好',95);
INSERT INTO tbl_2_c VALUES('chi''he','ch','吃喝',90);
INSERT INTO tbl_2_c VALUES('chi''yao','cy','吃药',85);
INSERT INTO tbl_2_c VALUES('chi''ku','ck','吃苦',80);
INSERT INTO tbl_2_c VALUES('chi''kui','ck','吃亏',75);
INSERT INTO tbl_2_c VALUES('chi''jing','cj','吃惊',70);
INSERT INTO tbl_2_c VALUES('chi''li','cl','吃力',65);
INSERT INTO tbl_2_c VALUES('chi''su','cs','吃素',60);
INSERT INTO tbl_2_c VALUES('chi''cu','cc','吃醋',55);
CREATE TABLE tbl_3_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_3_c VALUES('chi''dong''xi','cdx','吃东西',50);
INSERT INTO tbl_3_c VALUES('chi''cu''shui','ccs','吃醋水',40);
INSERT INTO tbl_3_c VALUES('chi''cu''yu','ccy','吃醋鱼',35);
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_w VALUES('wo','w','我',1000);
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_w VALUES('wo''men','wm','我们',100);",
    )
}

const BY_WEIGHT: [&str; 8] = ["饭", "好", "喝", "药", "苦", "亏", "惊", "力"];

/// 按表给逐字平均分，表里没有的候选 -5；记下每次打分的上文和候选。
struct Stub {
    means: Vec<(&'static str, f32)>,
    calls: Arc<Mutex<Vec<(String, Vec<String>)>>>,
}

impl Stub {
    fn new(means: &[(&'static str, f32)]) -> (Self, Arc<Mutex<Vec<(String, Vec<String>)>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                means: means.to_vec(),
                calls: calls.clone(),
            },
            calls,
        )
    }
}

impl CandidateScorer for Stub {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        self.calls.lock().unwrap().push((
            context.to_owned(),
            candidates.iter().map(|text| (*text).to_owned()).collect(),
        ));
        candidates
            .iter()
            .map(|text| {
                self.means
                    .iter()
                    .find(|(row, _)| row == text)
                    .map_or(-5.0, |(_, mean)| *mean)
            })
            .collect()
    }
}

/// 打错长度的打分器：排序不动。
struct Broken;

impl CandidateScorer for Broken {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _: &str, _: &[&str]) -> Vec<f32> {
        vec![0.0]
    }
}

fn commit_chi(session: &mut Session) {
    type_text(session, "chi");
    assert_eq!(select_word(session, "吃").commit.as_deref(), Some("吃"));
    assert!(session.snapshot().prediction);
}

fn session(fixture: &Fixture, pool: bool) -> Session {
    let mut session = fixture.session_with(|options| {
        options.next_word_prediction = true;
        options.learning = false;
    });
    session.set_prediction_pool(pool);
    session
}

#[test]
fn without_the_pool_prediction_is_the_first_eight_by_weight_and_is_never_scored() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, false);
    commit_chi(&mut session);
    assert_eq!(words(&session), BY_WEIGHT);
    let (mut stub, calls) = Stub::new(&[("东西", 0.0)]);
    assert!(!session.order_prediction(&mut stub, "我想"));
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(words(&session), BY_WEIGHT);
}

#[test]
fn the_pool_shows_the_same_eight_until_a_model_orders_it() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, true);
    commit_chi(&mut session);
    assert_eq!(words(&session), BY_WEIGHT);
}

#[test]
fn the_model_orders_the_whole_pool_by_total_log_probability_and_keeps_eight() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, true);
    commit_chi(&mut session);
    // 东西 排在词频第 11，逐字平均 -1 总分 -2；醋 平均 -1.5 总分 -1.5，按总分在 东西 前面，按平均分会在后面。
    let (mut stub, calls) = Stub::new(&[("东西", -1.0), ("醋", -1.5), ("饭", -3.0)]);
    let revision = session.candidate_revision();
    assert!(session.order_prediction(&mut stub, "今天我想"));
    assert!(session.candidate_revision() > revision);
    assert_eq!(
        words(&session),
        ["醋", "东西", "饭", "好", "喝", "药", "苦", "亏"]
    );
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let (context, candidates) = &calls[0];
    assert_eq!(context, "今天我想吃");
    // 13 条续写全部送去打分（吃醋水 给的是 醋水），不止显示的 8 条。
    assert_eq!(candidates.len(), 13);
    assert!(candidates.iter().any(|text| text == "醋水"));
}

#[test]
fn ordering_runs_once_per_commit() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, true);
    commit_chi(&mut session);
    let (mut stub, calls) = Stub::new(&[("东西", 0.0)]);
    assert!(session.order_prediction(&mut stub, ""));
    let revision = session.candidate_revision();
    assert!(!session.order_prediction(&mut stub, ""));
    assert_eq!(session.candidate_revision(), revision);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(words(&session)[0], "东西");
}

#[test]
fn a_scorer_that_cannot_score_leaves_the_weight_order() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, true);
    commit_chi(&mut session);
    let revision = session.candidate_revision();
    assert!(!session.order_prediction(&mut Broken, ""));
    assert_eq!(session.candidate_revision(), revision);
    assert_eq!(words(&session), BY_WEIGHT);
}

#[test]
fn selecting_a_reordered_row_commits_that_row_and_predicts_from_it() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, true);
    commit_chi(&mut session);
    let (mut stub, _) = Stub::new(&[("醋", 0.0), ("东西", -0.5)]);
    assert!(session.order_prediction(&mut stub, "今天"));
    assert_eq!(words(&session)[..2], ["醋", "东西"]);
    // 第 0 行按词频原本是第 10 条：选它上屏的就是它，下一轮联想按 吃醋 的种子查。
    assert_eq!(session.select(0).commit.as_deref(), Some("醋"));
    assert_eq!(words(&session), ["水", "鱼"]);
    // 这一轮上屏的只有 醋，宿主的上文已经记下了 今天吃，所以模型读到的是 今天吃醋。
    let (mut stub, calls) = Stub::new(&[("鱼", 0.0)]);
    assert!(session.order_prediction(&mut stub, "今天吃"));
    assert_eq!(words(&session), ["鱼", "水"]);
    assert_eq!(calls.lock().unwrap()[0].0, "今天吃醋");
    assert_eq!(session.select(0).commit.as_deref(), Some("鱼"));
}

#[test]
fn a_single_continuation_is_not_scored() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, true);
    type_text(&mut session, "wo");
    assert_eq!(
        select_word(&mut session, "我").commit.as_deref(),
        Some("我")
    );
    assert_eq!(words(&session), ["们"]);
    let (mut stub, calls) = Stub::new(&[]);
    assert!(!session.order_prediction(&mut stub, ""));
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn personal_rows_stay_in_front_in_their_own_order() {
    let fixture = pool_fixture();
    let store =
        crate::user_dictionary::ngram_store::PersonalNgramStore::for_journal(&fixture.journal());
    store
        .record(&[
            crate::lattice::personal::PersonalTransition {
                earlier: String::new(),
                previous: "吃".into(),
                word: "火锅".into(),
                times: 5,
            },
            crate::lattice::personal::PersonalTransition {
                earlier: String::new(),
                previous: "吃".into(),
                word: "饭".into(),
                times: 3,
            },
        ])
        .unwrap();
    let mut session = fixture.session_with(|options| {
        options.next_word_prediction = true;
        options.learning = true;
    });
    session.set_prediction_pool(true);
    commit_chi(&mut session);
    assert_eq!(words(&session)[..3], ["火锅", "饭", "好"]);
    // 模型最喜欢个人习惯行也不动它们：只排词库续写，饭 已经在个人习惯里，不再出现。
    let (mut stub, calls) = Stub::new(&[("火锅", 0.0), ("饭", 0.0), ("东西", -0.1)]);
    assert!(session.order_prediction(&mut stub, ""));
    assert_eq!(
        words(&session),
        ["火锅", "饭", "东西", "好", "喝", "药", "苦", "亏"]
    );
    let calls = calls.lock().unwrap();
    assert!(!calls[0].1.iter().any(|text| text == "火锅" || text == "饭"));
}

#[test]
fn turning_the_pool_off_drops_the_pending_order() {
    let fixture = pool_fixture();
    let mut session = session(&fixture, true);
    commit_chi(&mut session);
    session.set_prediction_pool(false);
    let (mut stub, calls) = Stub::new(&[("东西", 0.0)]);
    assert!(!session.order_prediction(&mut stub, ""));
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(words(&session), BY_WEIGHT);
}
