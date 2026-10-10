//! Engine-level tests over throwaway dictionaries: `test_shuangpin.cpp`, the shuangpin cases of `test_input_session.cpp` and `test_runtime_isolation.cpp` that exercise the engine itself, the double-helpcode cache overlay, online rows, initial expansion and the candidate lookup. A real-dictionary smoke test runs when `MSIME_EVAL_RESOURCES` names the dict-v2.0.1 resource directory.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::Connection;
use tempfile::TempDir;

use super::profile::profile;
use super::ShuangpinEngine;
use crate::assets;
use crate::helpcode::HelpcodeKeymap;
use crate::paths::RuntimePaths;
use crate::types::{
    fuzzy_rule, CandidateSource, FuzzyPinyinOptions, QueryRequest, SchemeType,
    SentenceAssociationOptions, ShuangpinProfileKind, WordItem,
};

struct Fixture {
    _root: TempDir,
    paths: RuntimePaths,
}

impl Fixture {
    /// One directory for all four roots, as the reference tests did.
    fn new(sql: &str) -> Self {
        let root = tempfile::tempdir().expect("fixture directory");
        let directory = root.path().to_path_buf();
        let paths = RuntimePaths {
            resources: directory.clone(),
            user_data: directory.clone(),
            cache: directory.clone(),
            dictionaries: directory,
        };
        Connection::open(paths.dictionary(assets::MAIN_DICTIONARY))
            .expect("fixture dictionary")
            .execute_batch(sql)
            .expect("fixture rows");
        Self { _root: root, paths }
    }

    fn engine(&self, kind: ShuangpinProfileKind) -> ShuangpinEngine {
        ShuangpinEngine::new(profile(kind).unwrap(), &self.paths)
    }

    fn weight(&self, table: &str, key: &str, value: &str) -> Option<i64> {
        Connection::open(self.paths.dictionary(assets::MAIN_DICTIONARY))
            .expect("fixture dictionary")
            .query_row(
                &format!("SELECT weight FROM \"{table}\" WHERE key = ?1 AND value = ?2"),
                [key, value],
                |row| row.get(0),
            )
            .ok()
    }
}

fn request(typed: &str, helpcode: bool) -> QueryRequest {
    QueryRequest {
        scheme: SchemeType::Shuangpin,
        raw_input: typed.to_ascii_lowercase(),
        raw_input_with_cases: typed.to_string(),
        enable_shuangpin_helpcode: helpcode,
        valid: true,
        ..QueryRequest::default()
    }
}

fn keymap(codes: &[(&str, &str)]) -> HelpcodeKeymap {
    HelpcodeKeymap::from_codes(
        codes
            .iter()
            .map(|(character, code)| (character.to_string(), code.to_string()))
            .collect::<HashMap<_, _>>(),
    )
}

fn words(candidates: &[WordItem]) -> Vec<&str> {
    candidates.iter().map(|item| item.word.as_str()).collect()
}

fn index_of(candidates: &[WordItem], word: &str) -> usize {
    candidates
        .iter()
        .position(|item| item.word == word)
        .unwrap_or_else(|| panic!("{word} missing from {:?}", words(candidates)))
}

/// A manual delimiter leaves empty pieces in the whole-input segmentation, so several of its shorter prefix groups are the same `ni` group; the reference appended each group whole and listed 你 and 拟 once per group. A word keeps its first, longest-prefix seat (decision 2026-09-30).
#[test]
fn prefix_groups_list_a_word_once() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_n VALUES('ni', 'n', '你', 10000),('ni', 'n', '拟', 9000);",
    );
    let microsoft = profile(ShuangpinProfileKind::Microsoft).unwrap();
    let mut dictionary = super::dictionary::ShuangpinDictionary::new(microsoft, &fixture.paths);
    let segmentation = super::utils::pinyin_segmentation("ni'nni", microsoft);
    assert_eq!(segmentation, "ni'''nn'i");
    let rows = dictionary.generate_series("ni'nni", &segmentation, "");
    assert_eq!(words(&rows), ["你", "拟"]);
}

#[test]
fn prefix_groups_reserve_their_candidate_rows() {
    let mut sql = String::from(
        "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);",
    );
    for index in 0..10 {
        sql.push_str(&format!(
            "INSERT INTO tbl_1_n VALUES('ni','n','单{index}',{});INSERT INTO tbl_2_n VALUES('ni''hao','nh','双{index}',{});",
            1000 - index,
            2000 - index
        ));
    }
    let fixture = Fixture::new(&sql);
    let mut dictionary = super::dictionary::ShuangpinDictionary::new(
        profile(ShuangpinProfileKind::Xiaohe).unwrap(),
        &fixture.paths,
    );

    let segmentation =
        super::query::segment_input("nihcma", profile(ShuangpinProfileKind::Xiaohe).unwrap());
    let rows = dictionary.generate_series("nihcma", &segmentation, "");

    assert_eq!(rows.len(), 20, "{rows:?}");
    assert_eq!(rows.capacity(), 20);
}

const TRAILING_HELPCODE: &str = "BEGIN;CREATE TABLE tbl_1_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_s VALUES('shi', 's', '使', 200);INSERT INTO tbl_1_s VALUES('shi', 's', '是', 100);COMMIT;";

/// Xiaohe reads `ui` as shi, so `uiu` is a complete syllable plus the single helpcode `u`, while `ui'u` is the same syllable followed by a user-delimited segment (test_shuangpin.cpp:129-158, golden sp_xiaohe_trailing_helpcode).
#[test]
fn manual_delimiter_disables_single_helpcode() {
    let fixture = Fixture::new(TRAILING_HELPCODE);
    let codes = keymap(&[("使", "ab"), ("是", "uc")]);
    // Each query on its own engine, so no cache of one raw input answers another.
    let query = |typed: &str, helpcode: bool| {
        fixture
            .engine(ShuangpinProfileKind::Xiaohe)
            .query(&request(typed, helpcode), Some(&codes))
    };

    let undelimited = query("uiu", true);
    assert!(index_of(&undelimited, "是") < index_of(&undelimited, "使"));
    // The reference listed the whole-input answer after the reordered rows again, [是, 使, 是, 使]; each word keeps its first seat (decision 2026-09-30).
    assert_eq!(words(&undelimited), ["是", "使"]);
    assert!(undelimited.iter().all(|item| item.pinyin == "ui"));

    let delimited = query("ui'u", true);
    assert!(index_of(&delimited, "使") < index_of(&delimited, "是"));
    assert_eq!(words(&delimited), ["使", "是"]);
    let without_helpcode = query("ui'u", false);
    assert_eq!(words(&delimited), words(&without_helpcode));
}

const HELPCODE_FILTER: &str = "CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 200);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '拟好', 100);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '𠀀方案𠮷', 90);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', 'C语言 2', 80);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', 'GitHub', 70);CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_h VALUES('hao','h','好',100);CREATE TABLE tbl_3_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);";

fn helpcode_filter_keymap() -> HelpcodeKeymap {
    keymap(&[("你", "ab"), ("拟", "cd"), ("好", "ef")])
}

/// Golden sp_helpcode_filter (test_input_session.cpp:1009-1067): matched rows, then the full input read as pinyin, then the rest, each word once (decision 2026-09-30; the reference listed a word again in every later list that answered it).
#[test]
fn single_and_double_helpcodes_filter_the_base() {
    let fixture = Fixture::new(HELPCODE_FILTER);
    let codes = helpcode_filter_keymap();
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);

    let plain = engine.query(&request("nihc", true), Some(&codes));
    assert_eq!(
        words(&plain),
        ["你好", "拟好", "𠀀方案𠮷", "C语言 2", "GitHub"]
    );
    assert!(plain
        .iter()
        .all(|item| item.pinyin == "nihc" && item.canonical_pinyin == "ni'hao"));

    let lowercase = engine.query(&request("nihcc", true), Some(&codes));
    assert_eq!(
        words(&lowercase),
        ["拟好", "你好", "𠀀方案𠮷", "C语言 2", "GitHub"]
    );

    let uppercase = engine.query(&request("nihcA", true), Some(&codes));
    assert_eq!(
        words(&uppercase),
        ["你好", "拟好", "𠀀方案𠮷", "C语言 2", "GitHub"]
    );

    assert!(engine
        .query(&request("nihcAB", true), Some(&codes))
        .is_empty());
    assert!(engine
        .query(&request("nihcAE", true), Some(&codes))
        .is_empty());
    // With the setting off the same letters are plain pinyin again.
    assert!(!engine
        .query(&request("nihcAE", false), Some(&codes))
        .is_empty());
}

#[test]
fn single_helpcode_capacity_includes_whole_and_unmatched_rows() {
    let fixture = Fixture::new(&format!(
        "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','前缀',50);{HELPCODE_FILTER}"
    ));
    let codes = helpcode_filter_keymap();
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);

    let candidates = engine.query(&request("nihcc", true), Some(&codes));
    assert!(candidates.capacity() >= 11);
}

#[test]
fn profiles_decode_their_own_codes() {
    let fixture = Fixture::new(
        "BEGIN;CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 100);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '拟好', 50);COMMIT;",
    );
    // Golden sp_ziranma_nihao: ziranma spells hao `hk`; `hc` is `h'c`.
    let mut ziranma = fixture.engine(ShuangpinProfileKind::Ziranma);
    let hk = ziranma.query(&request("nihk", false), None);
    assert_eq!(words(&hk), ["你好", "拟好"]);
    assert!(hk.iter().all(|item| item.pinyin == "nihk"));
    let hc = ziranma.query(&request("nihc", false), None);
    assert_eq!(words(&hc), ["你好", "拟好"]);
    assert!(hc.iter().all(|item| item.pinyin == "nih"));

    // Golden sp_microsoft_semicolon.
    let mut microsoft = fixture.engine(ShuangpinProfileKind::Microsoft);
    assert!(microsoft.query(&request("b;", false), None).is_empty());
    let semicolon = microsoft.query(&request("nihkb;", false), None);
    assert_eq!(words(&semicolon), ["你好", "拟好"]);
    assert!(semicolon.iter().all(|item| item.pinyin == "nihk"));
    let split = microsoft.query(&request("nihcb;", false), None);
    assert!(split.iter().all(|item| item.pinyin == "nih"));
}

/// 五笔 86 辅助码（`wubi86`）走的是同一套筛选：码表按方案名从资源目录载入，表头的 `#` 行被跳过；单字比整码，词比首字首码加末字首码。码表内容是合成的。
#[test]
fn wubi86_helpcodes_filter_by_the_first_two_letters() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_m VALUES('ma','m','吗',300),('ma','m','马',200),('ma','m','码',100);CREATE TABLE tbl_2_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_m VALUES('ma''ma','mm','妈妈',300),('ma''ma','mm','马码',200);CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);",
    );
    let helpcodes = fixture.paths.resources.join("helpcodes");
    std::fs::create_dir_all(&helpcodes).unwrap();
    std::fs::write(
        helpcodes.join("wubi86_helpcode.txt"),
        "# 合成表头\n吗=kc\n马=cn\n码=dc\n妈=vc\n",
    )
    .unwrap();
    let codes = crate::helpcode::load_helpcode_keymap(&fixture.paths.resources, "wubi86").unwrap();
    assert_eq!(codes.code("码"), Some("dc"));
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);

    // 第一个字母小写、第二个大写时两码按输入顺序读。
    assert_eq!(
        words(&engine.query(&request("madC", true), Some(&codes))),
        ["码"]
    );
    assert_eq!(
        words(&engine.query(&request("macN", true), Some(&codes))),
        ["马"]
    );
    // 单码把首码或末码相符的字提前。
    let single = engine.query(&request("maD", true), Some(&codes));
    assert_eq!(words(&single)[0], "码");
    // 马码：首字马的首码 c，末字码的首码 d；妈妈是 v、v。
    assert_eq!(
        words(&engine.query(&request("mamacD", true), Some(&codes))),
        ["马码"]
    );
    assert_eq!(
        words(&engine.query(&request("mamavV", true), Some(&codes))),
        ["妈妈"]
    );
}

/// overlays.md §5.1: a double-helpcode entry is keyed by its codes, and an online row lands only under the combination it was inserted for.
#[test]
fn online_rows_follow_the_query_cache() {
    let fixture = Fixture::new(HELPCODE_FILTER);
    let codes = helpcode_filter_keymap();
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let cloud = vec!["妮好".to_string()];

    let double = request("nihcAB", true);
    assert!(engine.query(&double, Some(&codes)).is_empty());
    assert!(engine.insert_online_words(&double, &cloud, CandidateSource::CloudSuggestion));
    assert_eq!(words(&engine.query(&double, Some(&codes))), ["妮好"]);
    // Another code pair has its own, never-queried entry.
    let other = request("nihcAE", true);
    assert!(!engine.insert_online_words(&other, &cloud, CandidateSource::CloudSuggestion));
    assert!(engine.query(&other, Some(&codes)).is_empty());

    let single = request("nihcc", true);
    engine.query(&single, Some(&codes));
    assert!(engine.insert_online_words(&single, &cloud, CandidateSource::CloudSuggestion));
    let listed = engine.query(&single, Some(&codes));
    assert_eq!(listed[1].word, "妮好");
    assert_eq!(listed[1].source, CandidateSource::CloudSuggestion);

    // The plain series entry is created when absent and read by the next query.
    let plain = request("ni'hc", true);
    assert!(engine.insert_online_words(&plain, &cloud, CandidateSource::CloudSuggestion));
    assert_eq!(words(&engine.query(&plain, Some(&codes))), ["妮好"]);
    let series = request("nihc", false);
    engine.query(&series, None);
    let ai: Vec<String> = ["甲", "乙", "甲"]
        .iter()
        .map(|word| word.to_string())
        .collect();
    assert!(engine.insert_online_words(&series, &ai, CandidateSource::AiSuggestion));
    let listed = engine.query(&series, None);
    assert_eq!(words(&listed)[..4], ["你好", "拟好", "甲", "乙"]);
    assert!(!engine.insert_online_words(&series, &["".to_string()], CandidateSource::AiSuggestion));
}

#[test]
fn initial_letter_expands_past_the_cap() {
    let mut sql =
        String::from("BEGIN;CREATE TABLE tbl_1_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);");
    for index in 0..30 {
        sql.push_str(&format!(
            "INSERT INTO tbl_1_s VALUES('shi', 's', '字{index:02}', {});",
            1000 - index
        ));
    }
    sql.push_str("INSERT INTO tbl_1_s VALUES('si', 's', '四', 1);COMMIT;");
    let fixture = Fixture::new(&sql);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let typed = request("u", false);
    let mut candidates = engine.query(&typed, None);
    assert_eq!(candidates.len(), 24);
    // Xiaohe `u` is the `sh` initial, so `si` stays out.
    assert!(candidates
        .iter()
        .all(|item| item.pinyin == "u" && item.canonical_pinyin == "shi"));
    assert!(engine.expand_initial_candidates(&typed, &mut candidates));
    assert_eq!(candidates.len(), 30);
    assert!(!engine.expand_initial_candidates(&typed, &mut candidates));
    assert!(!engine.expand_initial_candidates(&request("ui", false), &mut candidates));
}

#[test]
fn fuzzy_rows_take_the_typed_keys() {
    let fixture = Fixture::new(
        "BEGIN;CREATE TABLE tbl_1_z(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_z VALUES('zi', 'z', '字', 50);INSERT INTO tbl_1_z VALUES('zhi', 'z', '之', 100);COMMIT;",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("zi", false);
    assert_eq!(words(&engine.query(&typed, None)), ["字"]);
    typed.fuzzy_pinyin = FuzzyPinyinOptions {
        rules: fuzzy_rule::Z_ZH,
    };
    let fuzzy = engine.query(&typed, None);
    assert_eq!(words(&fuzzy), ["字", "之"]);
    assert_eq!(fuzzy[1].pinyin, "zi");
    assert!(fuzzy[1].fuzzy);
}

#[test]
fn fuzzy_rows_reserve_unique_results_before_appending() {
    let mut sql = String::from(
        "CREATE TABLE tbl_1_z(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_z VALUES('zi','z','精确',100);",
    );
    for index in 0..20 {
        sql.push_str(&format!(
            "INSERT INTO tbl_1_z VALUES('zhi','z','模糊{index:02}',{});",
            99 - index
        ));
    }
    let fixture = Fixture::new(&sql);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("zi", false);
    typed.fuzzy_pinyin = FuzzyPinyinOptions {
        rules: fuzzy_rule::Z_ZH,
    };

    let fuzzy = engine.query(&typed, None);

    assert_eq!(fuzzy.len(), 21);
    assert_eq!(fuzzy.capacity(), fuzzy.len());
}

#[test]
fn empty_fuzzy_rows_keep_exact_order() {
    let fixture = Fixture::new(
        "BEGIN;CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_n VALUES('ni', 'n', '你', 100);CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 200);COMMIT;",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("nihc", false);
    let exact = engine.query(&typed, None);
    typed.fuzzy_pinyin = FuzzyPinyinOptions {
        rules: fuzzy_rule::Z_ZH,
    };

    let with_empty_fuzzy = engine.query(&typed, None);

    assert_eq!(words(&exact), ["你好", "你"]);
    assert_eq!(words(&with_empty_fuzzy), words(&exact));
    assert!(with_empty_fuzzy.iter().all(|item| !item.fuzzy));
}

/// A fixed row missing from the list is looked up by its canonical quanpin key (user_dictionary positions).
#[test]
fn find_candidate_reads_the_canonical_key() {
    let fixture = Fixture::new(HELPCODE_FILTER);
    let engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let found = engine.find_candidate("ni'hao", "拟好").expect("row");
    assert_eq!(
        Some(found.weight),
        fixture.weight("tbl_2_n", "ni'hao", "拟好")
    );
    assert_eq!(found.pinyin, "ni'hao");
    assert_eq!(found.source, CandidateSource::Database);
    assert!(engine.find_candidate("ni'hao", "妮好").is_none());
}

#[test]
fn word_lattice_switch_controls_the_sentence_row() {
    let fixture = Fixture::new(
        "BEGIN;CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_n VALUES('ni', 'n', '你', 100);CREATE TABLE tbl_1_h(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_h VALUES('hao', 'h', '好', 100);COMMIT;",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("nihc", false);
    let with_lattice = engine.query(&typed, None);
    let sentence = with_lattice
        .iter()
        .find(|item| item.source == CandidateSource::Generated)
        .unwrap_or_else(|| panic!("no lattice row in {:?}", words(&with_lattice)));
    assert_eq!(sentence.word, "你好");
    assert!(sentence.sentence_association);
    assert_eq!(sentence.pinyin, "nihc");

    typed.sentence_association = SentenceAssociationOptions {
        word_lattice: false,
        ..SentenceAssociationOptions::default()
    };
    let without = engine.query(&typed, None);
    assert!(without
        .iter()
        .all(|item| item.source != CandidateSource::Generated));
    assert!(words(&without).contains(&"你"));
}

/// test_shuangpin.cpp:144-159: an exact dictionary entry for the whole code keeps row 0; the lattice sentence is inserted after it.
#[test]
fn exact_entry_leads_the_lattice_sentence() {
    let fixture = Fixture::new(
        "BEGIN;CREATE TABLE tbl_4_a(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_4_a VALUES('an''quan''bao''wei', 'aqbw', '安全保卫', 6000);CREATE TABLE tbl_2_a(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_a VALUES('an''quan', 'aq', '安全', 9000);CREATE TABLE tbl_2_b(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_b VALUES('bao''wei', 'bw', '包围', 9000);COMMIT;",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let candidates = engine.query(&request("anqrbcww", false), None);
    assert_eq!(
        words(&candidates).first(),
        Some(&"安全保卫"),
        "{:?}",
        words(&candidates)
    );
    let sentence = candidates
        .iter()
        .find(|item| item.source == CandidateSource::Generated)
        .unwrap_or_else(|| panic!("no lattice row in {:?}", words(&candidates)));
    assert_eq!(sentence.word, "安全包围");
}

#[test]
fn missing_dictionary_answers_empty() {
    let root = tempfile::tempdir().expect("fixture directory");
    let directory: PathBuf = root.path().to_path_buf();
    let paths = RuntimePaths {
        resources: directory.clone(),
        user_data: directory.clone(),
        cache: directory.clone(),
        dictionaries: directory,
    };
    let mut engine = ShuangpinEngine::new(profile(ShuangpinProfileKind::Xiaohe).unwrap(), &paths);
    assert!(engine.query(&request("nihc", true), None).is_empty());
    assert!(engine.query(&request("u", true), None).is_empty());
    assert!(!paths.dictionary(assets::MAIN_DICTIONARY).exists());
    let mut invalid = request("nihc", false);
    invalid.valid = false;
    assert!(engine.query(&invalid, None).is_empty());
    assert!(engine.query(&request("'", false), None).is_empty());
}

/// The shipped dictionary answers the common readings; skipped without the resource bundle.
#[test]
fn real_dictionary_answers_common_readings() {
    let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
        eprintln!("skipped: MSIME_EVAL_RESOURCES is not set to the dict-v2.0.1 resource directory");
        return;
    };
    let user = tempfile::tempdir().expect("user directory");
    let resources = PathBuf::from(resources);
    let paths = RuntimePaths {
        resources: resources.clone(),
        user_data: user.path().to_path_buf(),
        cache: user.path().to_path_buf(),
        dictionaries: resources,
    };
    let mut engine = ShuangpinEngine::new(profile(ShuangpinProfileKind::Xiaohe).unwrap(), &paths);
    let nihao = engine.query(&request("nihc", false), None);
    assert!(
        words(&nihao).iter().take(3).any(|word| *word == "你好"),
        "{:?}",
        words(&nihao)
    );
    let zhongguo = engine.query(&request("vsgo", false), None);
    assert!(
        words(&zhongguo).iter().take(3).any(|word| *word == "中国"),
        "{:?}",
        words(&zhongguo)
    );
}

/// Without a sentence model no answer reads the committed context, so a new context keeps every cached answer, the online rows included.
#[test]
fn context_changes_keep_the_caches_without_a_model() {
    let fixture = Fixture::new(HELPCODE_FILTER);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("nihc", false);
    typed.rescoring_context = "上文".into();
    engine.query(&typed, None);
    assert!(engine.insert_online_words(&typed, &["甲".to_string()], CandidateSource::AiSuggestion));
    typed.rescoring_context = "另一段上文".into();
    assert!(words(&engine.query(&typed, None)).contains(&"甲"));
}

/// With a sentence model loaded the trimmed context joins the series key (overlays.md §1.6.2): an answer cached under one context is not read under another, and it is read again when that context comes back. Needs the keyboard model in `MSIME_EVAL_RESOURCES`.
#[test]
fn a_loaded_model_keys_the_series_cache_by_context() {
    let model = match crate::lattice::neural::test_model_path(assets::NEURAL_MODEL_KEYBOARD) {
        Ok(model) => model,
        Err(reason) => {
            eprintln!("skipped: {reason}");
            return;
        }
    };
    let fixture = Fixture::new(HELPCODE_FILTER);
    std::fs::copy(
        &model,
        fixture.paths.resource(assets::NEURAL_MODEL_KEYBOARD),
    )
    .expect("model copy");
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("nihc", false);
    typed.sentence_association = SentenceAssociationOptions {
        neural_keyboard: true,
        ..SentenceAssociationOptions::default()
    };
    typed.rescoring_context = "上文".into();
    engine.query(&typed, None);
    assert!(engine.insert_online_words(&typed, &["甲".to_string()], CandidateSource::AiSuggestion));
    assert!(words(&engine.query(&typed, None)).contains(&"甲"));

    typed.rescoring_context = "另一段上文".into();
    assert!(!words(&engine.query(&typed, None)).contains(&"甲"));
    typed.rescoring_context = "上文".into();
    assert!(words(&engine.query(&typed, None)).contains(&"甲"));
}

/// SD:89-99: series answers scored with the personal model are dropped once the tables changed under it. The version is compared only after reading the model, which is what reloads it; a bare version read would keep the stale answer.
#[test]
fn a_changed_personal_model_drops_the_scored_series() {
    use crate::user_dictionary::ngram_store::PersonalNgramStore;
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','你',10000),('ni','n','拟',9000);CREATE TABLE tbl_1_h(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_h VALUES('hao','h','好',10000),('hao','h','号',9000);",
    );
    let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
    let mut dictionary = super::dictionary::ShuangpinDictionary::new(xiaohe, &fixture.paths);
    let segmentation = super::utils::pinyin_segmentation("nihc", xiaohe);
    let before = dictionary.generate_series("nihc", &segmentation, "");
    let store = PersonalNgramStore::for_journal(&fixture.paths.user(assets::USER_JOURNAL));
    let version = store.version();
    let journal = Connection::open(fixture.paths.user(assets::USER_JOURNAL)).unwrap();
    crate::user_dictionary::journal::ensure_schema(&journal).unwrap();
    journal
        .execute_batch("INSERT INTO personal_bigram VALUES(char(1),'拟',400),('拟','号',400);INSERT INTO personal_trigram VALUES(char(1),'拟','号',400);")
        .unwrap();
    store.invalidate_for_tests();
    let after = dictionary.generate_series("nihc", &segmentation, "");
    assert!(
        store.version() > version,
        "the query did not reload the model"
    );
    assert!(
        words(&after).contains(&"拟号") && !words(&before).contains(&"拟号"),
        "the answer scored with the old model was kept: {:?} then {:?}",
        words(&before),
        words(&after)
    );
}

/// neural-association.patch:3080-3095: an association change resets the caches, so a cached online row does not survive a lattice off/on round trip; quanpin behaves the same (`association_switch_resets_the_series_cache`).
#[test]
fn association_switch_resets_the_series_cache() {
    let fixture = Fixture::new(HELPCODE_FILTER);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("nihc", false);
    engine.query(&typed, None);
    assert!(engine.insert_online_words(
        &typed,
        &["甲".to_string()],
        CandidateSource::CloudSuggestion
    ));
    assert!(words(&engine.query(&typed, None)).contains(&"甲"));
    typed.sentence_association = SentenceAssociationOptions {
        word_lattice: false,
        ..SentenceAssociationOptions::default()
    };
    assert!(!words(&engine.query(&typed, None)).contains(&"甲"));
    typed.sentence_association = SentenceAssociationOptions::default();
    assert!(!words(&engine.query(&typed, None)).contains(&"甲"));
}

// ---- typo sentences (#6034) ----

/// 合成词库：没关系三个音节里只有关系是词，字面读法只能拼单字。小鹤 mei/guan/xi 是 mw/gr/xi，自然码是 mz/gr/xi；gen 和 ci 是按错一个邻键后得到的合法音节。
const TYPO_FIXTURE: &str = "BEGIN;CREATE TABLE tbl_1_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_m VALUES('mei','m','没',1000),('mei','m','美',900);CREATE TABLE tbl_1_g(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('guan','g','关',500);CREATE TABLE tbl_1_x(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_x VALUES('xi','x','系',1000);CREATE TABLE tbl_1_c(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_c VALUES('ci','c','词',1000);CREATE TABLE tbl_2_g(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_g VALUES('guan''xi','gx','关系',100000);CREATE TABLE tbl_2_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);CREATE TABLE tbl_3_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);COMMIT;";

fn autocorrect_request(typed: &str, enabled: bool) -> QueryRequest {
    QueryRequest {
        enable_quanpin_autocorrect_transposition: enabled,
        enable_quanpin_autocorrect_neighbor: enabled,
        ..request(typed, false)
    }
}

fn typo_row(candidates: &[WordItem]) -> Option<&WordItem> {
    candidates
        .iter()
        .find(|item| item.sentence_association && !item.corrected_from.is_empty())
}

/// 纠错整句排在字面整句之后，不抢首选，读音记成改正后的全拼。
fn assert_corrected(candidates: &[WordItem], typed: &str) {
    let row = typo_row(candidates)
        .unwrap_or_else(|| panic!("no typo sentence for {typed} in {:?}", words(candidates)));
    assert_eq!(row.word, "没关系");
    assert_eq!(row.corrected_from, typed);
    assert_eq!(row.pinyin, typed);
    assert_eq!(row.canonical_pinyin, "mei'guan'xi");
    assert_eq!(row.source, CandidateSource::Generated);
    let literal = candidates
        .iter()
        .position(|item| item.sentence_association && item.corrected_from.is_empty())
        .unwrap_or_else(|| panic!("no literal sentence in {:?}", words(candidates)));
    assert!(
        literal < index_of(candidates, "没关系"),
        "{:?}",
        words(candidates)
    );
    assert!(candidates[0].corrected_from.is_empty());
}

/// 小鹤 guan 是 gr，r 按成邻键 f 就成了 gen（gf）。
#[test]
fn xiaohe_neighbour_key_typo_is_corrected() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let corrected = engine.query(&autocorrect_request("mwgfxi", true), None);
    assert_corrected(&corrected, "mwgfxi");
    assert_eq!(corrected[0].word, "没跟系");
}

/// 自然码 xi 的声母按成邻键 c 就成了 ci。
#[test]
fn ziranma_neighbour_key_typo_is_corrected() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Ziranma);
    let corrected = engine.query(&autocorrect_request("mzgrci", true), None);
    assert_corrected(&corrected, "mzgrci");
}

/// 正确输入的首选和纠错开关无关，也不出纠错行；开关关着、或用户手打了 `'` 时不纠。
#[test]
fn correct_input_keeps_its_first_candidate() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    for (kind, correct, typo) in [
        (ShuangpinProfileKind::Xiaohe, "mwgrxi", "mwgfxi"),
        (ShuangpinProfileKind::Ziranma, "mzgrxi", "mzgrci"),
    ] {
        let mut engine = fixture.engine(kind);
        let on = engine.query(&autocorrect_request(correct, true), None);
        let mut plain = fixture.engine(kind);
        let off = plain.query(&autocorrect_request(correct, false), None);
        assert_eq!(on[0].word, "没关系", "{kind:?} {:?}", words(&on));
        assert_eq!(words(&on), words(&off), "{kind:?}");
        assert!(typo_row(&on).is_none(), "{kind:?} {:?}", words(&on));

        let switched_off = engine.query(&autocorrect_request(typo, false), None);
        assert!(typo_row(&switched_off).is_none(), "{kind:?}");
        assert!(!words(&switched_off).contains(&"没关系"), "{kind:?}");
        let delimited = format!("{}'{}", &typo[..2], &typo[2..]);
        let literal = engine.query(&autocorrect_request(&delimited, true), None);
        assert!(
            typo_row(&literal).is_none(),
            "{kind:?} {:?}",
            words(&literal)
        );
    }
}

/// 开关来回切换时，缓存里按另一个设置组出的答案不能被读到。
#[test]
fn the_switch_is_read_on_every_query() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    assert!(typo_row(&engine.query(&autocorrect_request("mwgfxi", true), None)).is_some());
    assert!(typo_row(&engine.query(&autocorrect_request("mwgfxi", false), None)).is_none());
    assert!(typo_row(&engine.query(&autocorrect_request("mwgfxi", true), None)).is_some());
    // 只开其中一项也算开：会话只有一个「拼音纠错」开关。
    let neighbor_only = QueryRequest {
        enable_quanpin_autocorrect_transposition: false,
        ..autocorrect_request("mwgfxi", true)
    };
    assert!(typo_row(&engine.query(&neighbor_only, None)).is_some());
}

/// 变体按各自方案的键位生成：同一串按键在小鹤和自然码里是不同的音节，交替使用两个方案时各自只按自己的编码纠错。
#[test]
fn each_profile_corrects_its_own_codes() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut xiaohe = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut ziranma = fixture.engine(ShuangpinProfileKind::Ziranma);
    for _ in 0..2 {
        assert_corrected(
            &xiaohe.query(&autocorrect_request("mwgfxi", true), None),
            "mwgfxi",
        );
        assert_corrected(
            &ziranma.query(&autocorrect_request("mzgrci", true), None),
            "mzgrci",
        );
        // 小鹤的 mz 是 mou，自然码的 mw 不是合法编码：换了方案，这两串都不是没关系的误触。
        let foreign = xiaohe.query(&autocorrect_request("mzgrci", true), None);
        assert!(
            !words(&foreign).contains(&"没关系"),
            "{:?}",
            words(&foreign)
        );
        let foreign = ziranma.query(&autocorrect_request("mwgfxi", true), None);
        assert!(
            !words(&foreign).contains(&"没关系"),
            "{:?}",
            words(&foreign)
        );
    }
}

/// 手打的 `'` 在辅助码路径上同样关掉纠错：辅助码的基础部分按带 `'` 的原始输入准入和缓存，和不带 `'` 的同一串按键各有各的答案。
#[test]
fn a_manual_delimiter_disables_correction_with_a_helpcode() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let codes = keymap(&[("没", "mb"), ("美", "mc"), ("系", "xa")]);
    let with_helpcode = |typed: &str| QueryRequest {
        enable_shuangpin_helpcode: true,
        ..autocorrect_request(typed, true)
    };
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    // 同一个引擎来回查，任何一种答案都不能从缓存里顶替另一种。
    for _ in 0..2 {
        let undelimited = engine.query(&with_helpcode("mwgfxim"), Some(&codes));
        assert!(
            typo_row(&undelimited).is_some(),
            "{:?}",
            words(&undelimited)
        );
        let delimited = engine.query(&with_helpcode("mw'gfxim"), Some(&codes));
        assert!(typo_row(&delimited).is_none(), "{:?}", words(&delimited));
        assert!(typo_row(&engine.query(&autocorrect_request("mwgfxi", true), None)).is_some());
        assert!(typo_row(&engine.query(&autocorrect_request("mw'gfxi", true), None)).is_none());
    }
}
