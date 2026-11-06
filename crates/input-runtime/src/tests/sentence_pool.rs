//! 整句读法池：挂着报了 `sentence_pool` 的逐键打分器时，引擎每条切分解满一次比较的条数，排在默认 6 条之后的读法也能被模型挑上首位；不报的打分器（随包模型）和不挂打分器时，候选与以前逐行相同。打分器给出整句打分器（`sentence_scorer`）时，引擎在束搜索里用它，被词频剪掉的读法也能进来。

use super::*;

/// ta 三个字、shuo 三个字，没有词组也没有二元表：九种读法按两字词频之积排，第 7 到第 9 条是 他朔 她朔 它朔。
const POOL_DICTIONARY: &str = "CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_t VALUES('ta','t','他',1000);
INSERT INTO tbl_1_t VALUES('ta','t','她',900);
INSERT INTO tbl_1_t VALUES('ta','t','它',800);
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_s VALUES('shuo','s','说',1000);
INSERT INTO tbl_1_s VALUES('shuo','s','硕',100);
INSERT INTO tbl_1_s VALUES('shuo','s','朔',50);";

/// 按词频排在第九的读法。
const NINTH: &str = "它朔";

/// 只给 `NINTH` 高分，其余 -5；`pool` 是它报的 `sentence_pool`。
struct Favouring {
    pool: usize,
}

impl CandidateScorer for Favouring {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        candidates
            .iter()
            .map(|text| if *text == NINTH { 0.0 } else { -5.0 })
            .collect()
    }

    fn sentence_pool(&self) -> usize {
        self.pool
    }
}

fn runtime(scorer: Option<Box<dyn CandidateScorer>>) -> (tempfile::TempDir, Runtime) {
    let directory = tempfile::tempdir().unwrap();
    let options = real_engine_options(directory.path());
    let database = std::path::Path::new(&options.dictionaries).join("msime-pinyin.db");
    rusqlite::Connection::open(database)
        .unwrap()
        .execute_batch(POOL_DICTIONARY)
        .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 9).unwrap();
    runtime.focus(true).unwrap();
    runtime.set_candidate_scorer(scorer);
    (directory, runtime)
}

fn sentences_after_typing(runtime: &mut Runtime) -> Vec<String> {
    for value in b"tashuo" {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    runtime
        .all_candidates()
        .candidates
        .iter()
        .filter(|row| row.text.chars().count() == 2 && !row.text.is_ascii())
        .map(|row| row.text.clone())
        .collect()
}

#[test]
fn without_a_pool_the_lattice_keeps_six_readings() {
    let (_directory, mut runtime) = runtime(None);
    let sentences = sentences_after_typing(&mut runtime);
    assert_eq!(sentences.len(), 6, "{sentences:?}");
    assert!(!sentences.iter().any(|text| text == NINTH));
}

#[test]
fn a_scorer_without_a_pool_sees_the_same_readings() {
    let (_directory, mut plain) = runtime(None);
    let (_directory, mut scored) = runtime(Some(Box::new(Favouring { pool: 0 })));
    assert_eq!(
        sentences_after_typing(&mut scored),
        sentences_after_typing(&mut plain)
    );
}

#[test]
fn a_pooled_scorer_can_promote_a_reading_past_the_sixth() {
    let (_directory, mut runtime) = runtime(Some(Box::new(Favouring {
        pool: msime_engine::ordering::SENTENCE_POOL,
    })));
    let sentences = sentences_after_typing(&mut runtime);
    assert_eq!(sentences.len(), 9, "{sentences:?}");
    assert_eq!(sentences[0], NINTH);
}

#[test]
fn removing_the_scorer_shrinks_the_pool_again() {
    let (_directory, mut runtime) = runtime(Some(Box::new(Favouring {
        pool: msime_engine::ordering::SENTENCE_POOL,
    })));
    runtime.set_candidate_scorer(None);
    let sentences = sentences_after_typing(&mut runtime);
    assert_eq!(sentences.len(), 6, "{sentences:?}");
}

/// 只给 `NINTH` 和它的前缀高分的整句打分器，交给引擎在束搜索里用。
struct FavouringPrefixes;

impl msime_engine::ordering::SentenceScorer for FavouringPrefixes {
    fn log_probabilities(&self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        candidates
            .iter()
            .map(|text| if NINTH.starts_with(text) { 0.0 } else { -5.0 })
            .collect()
    }
}

/// 不报整句池、但给出整句打分器的逐键打分器：引擎照默认只解六条，模型在束搜索里把第九条读法提进来。
struct LendsItsModel(std::sync::Arc<FavouringPrefixes>);

impl CandidateScorer for LendsItsModel {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        vec![-5.0; candidates.len()]
    }

    fn sentence_scorer(
        &self,
    ) -> Option<std::sync::Arc<dyn msime_engine::ordering::SentenceScorer>> {
        Some(self.0.clone())
    }
}

#[test]
fn a_scorer_that_lends_its_model_brings_a_pruned_reading_into_the_lattice() {
    let (_directory, mut runtime) = runtime(Some(Box::new(LendsItsModel(std::sync::Arc::new(
        FavouringPrefixes,
    )))));
    let sentences = sentences_after_typing(&mut runtime);
    assert_eq!(sentences.len(), 6, "{sentences:?}");
    assert_eq!(sentences[0], NINTH, "{sentences:?}");
}

#[test]
fn removing_the_scorer_takes_its_model_out_of_the_lattice() {
    let (_directory, mut runtime) = runtime(Some(Box::new(LendsItsModel(std::sync::Arc::new(
        FavouringPrefixes,
    )))));
    runtime.set_candidate_scorer(None);
    let sentences = sentences_after_typing(&mut runtime);
    assert!(!sentences.iter().any(|text| text == NINTH), "{sentences:?}");
}

/// wenjian 有两条切分：wen'jian 读出「文件」，wen'ji'an 只能拼出「问及按」「文集安」这类生造的三字组合。每个字都有几个同音字，两条切分各能解出一池读法。
const SEGMENTATION_DICTIONARY: &str =
    "CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_w VALUES('wen','w','问',900000);
INSERT INTO tbl_1_w VALUES('wen','w','文',800000);
INSERT INTO tbl_1_w VALUES('wen','w','闻',300000);
INSERT INTO tbl_1_w VALUES('wen','w','稳',200000);
INSERT INTO tbl_1_w VALUES('wu','w','无',900000);
INSERT INTO tbl_1_w VALUES('wu','w','物',500000);
INSERT INTO tbl_1_w VALUES('wu','w','屋',300000);
CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_j VALUES('jian','j','间',900000);
INSERT INTO tbl_1_j VALUES('jian','j','见',800000);
INSERT INTO tbl_1_j VALUES('jian','j','建',500000);
INSERT INTO tbl_1_j VALUES('jian','j','剑',200000);
INSERT INTO tbl_1_j VALUES('ji','j','及',900000);
INSERT INTO tbl_1_j VALUES('ji','j','即',800000);
INSERT INTO tbl_1_j VALUES('ji','j','集',700000);
INSERT INTO tbl_1_j VALUES('ji','j','机',600000);
INSERT INTO tbl_1_j VALUES('ji','j','急',500000);
CREATE TABLE tbl_1_a(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_a VALUES('an','a','按',900000);
INSERT INTO tbl_1_a VALUES('an','a','安',800000);
INSERT INTO tbl_1_a VALUES('an','a','案',700000);
INSERT INTO tbl_1_a VALUES('an','a','暗',600000);
CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_l VALUES('li','l','理',900000);
INSERT INTO tbl_1_l VALUES('li','l','力',800000);
INSERT INTO tbl_1_l VALUES('li','l','里',700000);
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_w VALUES('wen''jian','wj','文件',530000);
INSERT INTO tbl_2_w VALUES('wen''jian','wj','稳健',60000);
INSERT INTO tbl_2_w VALUES('wen''ji','wj','问及',3000);
INSERT INTO tbl_2_w VALUES('wen''ji','wj','文集',2000);
INSERT INTO tbl_2_w VALUES('wu''li','wl','物理',660000);
INSERT INTO tbl_2_w VALUES('wu''li','wl','无力',90000);
CREATE TABLE tbl_3_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_3_w VALUES('wu''li''ji','wlj','乌力吉',4471);";

/// 报整句池（`pool`）的逐键打分器，给 `favoured` 里的读法高分，其余一律低分。
struct FavouringReadings {
    favoured: &'static [&'static str],
    pool: usize,
}

impl CandidateScorer for FavouringReadings {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
        candidates
            .iter()
            .map(|text| {
                if self.favoured.contains(text) {
                    0.0
                } else {
                    -8.0
                }
            })
            .collect()
    }

    fn sentence_pool(&self) -> usize {
        self.pool
    }
}

/// 在 `SEGMENTATION_DICTIONARY` 上按 `scheme` 建 runtime（五笔时打开混输拼音、另放一张五笔码表），挂上 `scorer` 后打 `keys`，返回整张候选表（文字和来源）。
fn segmented_candidates(
    scheme: u8,
    scorer: Option<Box<dyn CandidateScorer>>,
    keys: &[u8],
) -> (tempfile::TempDir, Vec<(String, u8)>) {
    let directory = tempfile::tempdir().unwrap();
    let mut options = real_engine_options(directory.path());
    options.scheme = scheme;
    options.wubi_mixed_pinyin = true;
    options.mixed_english = false;
    let wubi = "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
INSERT INTO wubi86 VALUES('aaaa','工',100);";
    rusqlite::Connection::open(
        std::path::Path::new(&options.dictionaries).join(msime_engine::assets::MAIN_DICTIONARY),
    )
    .unwrap()
    .execute_batch(&format!("{SEGMENTATION_DICTIONARY}\n{wubi}"))
    .unwrap();
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 6).unwrap();
    runtime.focus(true).unwrap();
    runtime.set_candidate_scorer(scorer);
    for value in keys {
        runtime
            .dispatch(Action::Character {
                value: *value,
                shift: false,
            })
            .unwrap();
    }
    let rows = runtime
        .all_candidates()
        .candidates
        .into_iter()
        .map(|row| (row.text, row.source))
        .collect();
    (directory, rows)
}

/// 读满整串的词库词（文件、稳健）都排在 wen'ji'an 的三字读法前面；单字之前至多留一条三字读法（另一种切分里排在最前的那条，没有整句池时引擎也把它放在这一带）。
fn assert_generated_readings_follow_the_words(rows: &[(String, u8)]) {
    let generated = |(text, source): &(String, u8)| {
        *source == msime_engine::ordering::LATTICE_SOURCE && text.chars().count() == 3
    };
    let first_generated = rows.iter().position(generated).expect("wen'ji'an 的读法");
    for word in ["文件", "稳健"] {
        let seat = rows.iter().position(|(text, _)| text == word).expect(word);
        assert!(seat < first_generated, "{word} 排在生造组合后面：{rows:?}");
    }
    let first_single = rows
        .iter()
        .position(|(text, _)| text.chars().count() == 1)
        .expect("单字");
    let ahead = rows[..first_single]
        .iter()
        .filter(|row| generated(row))
        .count();
    assert!(ahead <= 1, "单字前面还有 {ahead} 条生造组合：{rows:?}");
}

/// 全拼 wenjian，挂着报整句池的逐键打分器（macOS「增强」在快机器上报 32 条）：「文件」排第一，「稳健」排在 wen'ji'an 的三字生造组合前面（#10998）。以前引擎把它们放进主切分整句的座位供模型比较，按字数认读法的降位又认不出它们，首页第 3 到第 6 位全是「问及按」「问及安」，「稳健」「闻见」这些词库词被挤到后面。
#[test]
fn readings_of_another_segmentation_stay_behind_dictionary_words() {
    for pool in [msime_engine::ordering::SENTENCE_POOL, 32] {
        let (_directory, rows) = segmented_candidates(
            0,
            Some(Box::new(FavouringReadings {
                favoured: &["问及按"],
                pool,
            })),
            b"wenjian",
        );
        assert_eq!(rows[0].0, "文件", "pool {pool}: {rows:?}");
        assert!(
            rows.iter().any(|(text, _)| text.chars().count() == 3),
            "pool {pool}: wen'ji'an 没有解出读法，用例没有测到要测的东西：{rows:?}"
        );
        assert_generated_readings_follow_the_words(&rows);
    }
}

/// 没有整句池（不挂打分器）时引擎不挪座位，另一种切分的读法照旧排在主切分的整串词之后、单字之前：chulia 要的「处理啊」就是这样留在第一页附近的，降位不动它们。
#[test]
fn without_a_pool_readings_of_another_segmentation_keep_their_seats() {
    let (_directory, rows) = segmented_candidates(0, None, b"wenjian");
    assert_eq!(rows[0].0, "文件", "{rows:?}");
    let first_reading = rows
        .iter()
        .position(|(text, source)| {
            *source == msime_engine::ordering::LATTICE_SOURCE && text.chars().count() == 3
        })
        .expect("wen'ji'an 的读法");
    let first_single = rows
        .iter()
        .position(|(text, _)| text.chars().count() == 1)
        .expect("单字");
    assert!(first_reading < first_single, "{rows:?}");
}

/// 五笔混输拼音打 wenjian 走同一条拼音路径，同样不让生造组合排到词库词前面。
#[test]
fn wubi_mixed_pinyin_keeps_generated_readings_behind_words() {
    let (_directory, rows) = segmented_candidates(
        msime_engine::SchemeType::Wubi as u8,
        Some(Box::new(FavouringReadings {
            favoured: &["问及按"],
            pool: 32,
        })),
        b"wenjian",
    );
    assert_eq!(rows[0].0, "文件", "{rows:?}");
    assert_generated_readings_follow_the_words(&rows);
}

/// 全拼 wuliji：整串的词库词「乌力吉」是可信的首选，打分器再偏爱「物理及」也不越过它；它后面只留同一条切分的三条读法，「物理」「无力」这些词库词紧跟其后。
#[test]
fn a_whole_key_dictionary_word_keeps_the_lead_over_pooled_readings() {
    let (_directory, rows) = segmented_candidates(
        0,
        Some(Box::new(FavouringReadings {
            favoured: &["物理及", "物理急"],
            pool: 32,
        })),
        b"wuliji",
    );
    assert_eq!(rows[0].0, "乌力吉", "{rows:?}");
    let first_partial_word = rows
        .iter()
        .position(|(text, _)| text == "物理")
        .expect("物理");
    let generated_before = rows[..first_partial_word]
        .iter()
        .filter(|(_, source)| *source == msime_engine::ordering::LATTICE_SOURCE)
        .count();
    assert!(generated_before <= 3, "{rows:?}");
}
