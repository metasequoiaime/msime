//! Engine-level tests over throwaway dictionaries: `test_shuangpin.cpp`, the shuangpin cases of `test_input_session.cpp` and `test_runtime_isolation.cpp` that exercise the engine itself, the double-helpcode cache overlay, online rows, initial expansion and the candidate lookup. A real-dictionary smoke test runs when `MSIME_EVAL_RESOURCES` names the dict-v2.0.1 resource directory.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::Connection;
use tempfile::TempDir;

use super::profile::profile;
use super::ShuangpinEngine;
use crate::assets;
use crate::helpcode::HelpcodeKeymap;
use crate::paths::RuntimePaths;
use crate::shuangpin::typo_edges::learned_discount;
use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
use crate::types::{
    fuzzy_rule, CandidateSource, FuzzyPinyinOptions, QueryRequest, SchemeType,
    SentenceAssociationOptions, ShuangpinProfileKind, WordItem,
};
use crate::user_dictionary::typo_profile::{PersonalTypoProfile, ShuangpinTypoOutcome};

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
    let mut request = QueryRequest {
        scheme: SchemeType::Shuangpin,
        raw_input: typed.to_ascii_lowercase(),
        raw_input_with_cases: typed.to_string(),
        enable_shuangpin_helpcode: helpcode,
        valid: true,
        ..QueryRequest::default()
    };
    // 本文件的辅助码用例都写于万象式直接恒开的版本：`direct_helpcode` 受控后，
    // 用 helpcode 的请求继续开着它才有原来的覆盖面。
    request.shuangpin_helpcode_modes.direct_helpcode = helpcode;
    request.shuangpin_helpcode_modes.direct_helpcode_uppercase = true;
    request
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

#[test]
fn builtin_profiles_find_den_in_a_synthetic_dictionary() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_d(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
         INSERT INTO tbl_1_d VALUES('den', 'd', '扽', 100), ('dia', 'd', '嗲', 95), ('deng', 'd', '灯', 90);",
    );
    for kind in [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
        ShuangpinProfileKind::SmartAbc,
    ] {
        let selected = profile(kind).unwrap();
        let code = if kind == ShuangpinProfileKind::Shoudao {
            assert_eq!(super::utils::cvt_single_sp_to_pinyin("dk", selected), "dia");
            "dk".to_string()
        } else {
            let code = super::utils::encode_pinyin("den", selected)
                .unwrap_or_else(|| panic!("{kind:?} cannot encode den"));
            assert_eq!(
                super::utils::cvt_single_sp_to_pinyin(&code, selected),
                "den"
            );
            code
        };
        let mut engine = fixture.engine(kind);
        let candidates = engine.query(&request(&code, false), None);
        assert!(
            words(&candidates).contains(&"扽"),
            "{kind:?}: {code}: {:?}",
            words(&candidates)
        );
        if kind == ShuangpinProfileKind::Shoudao {
            assert!(
                words(&candidates).contains(&"嗲"),
                "手心原有的 `dia` 候选应保留"
            );
        }
    }
}

#[test]
fn warm_queries_do_not_allocate_a_compact_input_only_for_the_empty_check() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER); INSERT INTO tbl_1_n VALUES('ni','n','你',100); CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER); INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200);",
    );
    let measured = [("ni", 8), ("ni'hc", 12), ("ni''hc'", 12)].map(|(raw, budget)| {
        let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
        let typed = request(raw, false);
        let expected = engine.query(&typed, None);
        assert!(!expected.is_empty(), "raw={raw:?}");
        engine.query(&typed, None);
        let (rows, allocations) =
            crate::ime::personal_rerank::allocations::count(|| engine.query(&typed, None));
        assert_eq!(rows, expected, "raw={raw:?}");
        (raw, allocations, budget)
    });
    assert!(
        measured
            .iter()
            .all(|(_, allocations, budget)| allocations <= budget),
        "{measured:?}"
    );
}

#[test]
fn delimiter_only_queries_return_without_allocating() {
    let fixture = Fixture::new("");
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for raw in ["", "'", "'''", "''''''''"] {
        let typed = request(raw, true);
        let (rows, allocations) =
            crate::ime::personal_rerank::allocations::count(|| engine.query(&typed, None));
        assert!(rows.is_empty());
        assert_eq!(allocations, 0, "raw={raw:?}");
    }
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
    // 真正命中的辅助码仍在补全句前；单字消耗基础编码，补全句消耗末尾声母。
    assert_eq!(words(&undelimited), ["是", "使使", "使"]);
    assert_eq!(undelimited[0].pinyin, "ui");
    assert_eq!(undelimited[1].pinyin, "uiu");
    assert_eq!(undelimited[2].pinyin, "ui");

    let delimited = query("ui'u", true);
    assert!(index_of(&delimited, "使") < index_of(&delimited, "是"));
    assert_eq!(words(&delimited), ["使使", "使", "是"]);
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
fn strict_single_helpcode_filters_direct_triggered_and_fuzzy_rows() {
    let fixture = Fixture::new(HELPCODE_FILTER);
    let codes = helpcode_filter_keymap();
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for (input, expected) in [
        ("nihcc", vec!["拟好"]),
        ("nihca", vec!["你好"]),
        ("nihcE", vec!["你好", "拟好"]),
        ("nihcA", vec!["你好"]),
    ] {
        let mut typed = request(input, true);
        let loose = engine.query(&typed, Some(&codes));
        assert!(words(&loose).contains(&"GitHub"));
        typed.strict_quanpin_helpcode = true;
        assert_eq!(words(&engine.query(&typed, Some(&codes))), expected);
        typed.strict_quanpin_helpcode = false;
        assert_eq!(words(&engine.query(&typed, Some(&codes))), words(&loose));
    }
    let mut triggered = request("nihc", true);
    triggered.end_helpcodes = "c".into();
    triggered.strict_quanpin_helpcode = true;
    assert_eq!(words(&engine.query(&triggered, Some(&codes))), ["拟好"]);
    triggered.shuangpin_helpcode_modes.mid_sentence = true;
    triggered.syllable_helpcodes = vec![crate::types::SyllableHelpcode {
        syllable: 0,
        first: b'a',
        second: 0,
    }];
    assert!(engine.query(&triggered, Some(&codes)).is_empty());

    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_n VALUES('nen','n','嫩',100);\
         CREATE TABLE tbl_1_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_r VALUES('ren','r','人',10000);",
    );
    let codes = keymap(&[("嫩", "ab"), ("人", "cd")]);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut fuzzy = request("lfa", true);
    fuzzy.fuzzy_pinyin.rules = fuzzy_rule::N_L | fuzzy_rule::R_L;
    fuzzy.strict_quanpin_helpcode = true;
    assert_eq!(words(&engine.query(&fuzzy, Some(&codes))), ["嫩"]);
}

/// `shuangpin_helpcode_modes.direct` 关闭后，「完整基座 + 末尾裸字母」不再算辅助码：单码 `nihcc` 与双码 `nihcAB` 都按纯拼音读，与辅助码整体关闭一致；打开时维持现有的重排与筛选。
#[test]
fn direct_off_reads_trailing_letters_as_plain_pinyin() {
    let fixture = Fixture::new(HELPCODE_FILTER);
    let codes = helpcode_filter_keymap();
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);

    let off = |typed: &str| {
        let mut request = request(typed, true);
        request.shuangpin_helpcode_modes.direct = false;
        request
    };
    for typed in ["nihcc", "nihcA", "nihcAB"] {
        assert_eq!(
            words(&engine.query(&off(typed), Some(&codes))),
            words(&engine.query(&request(typed, false), Some(&codes))),
            "direct off must read {typed} as plain pinyin"
        );
    }
    // 开关确实生效：同一串开着时仍是单码重排 / 双码筛选。
    assert_eq!(
        words(&engine.query(&request("nihcc", true), Some(&codes))),
        ["拟好", "你好", "𠀀方案𠮷", "C语言 2", "GitHub"]
    );
    assert!(engine
        .query(&request("nihcAB", true), Some(&codes))
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
    // AI 候选按回答的顺序排在末尾（#6888）。
    assert_eq!(words(&listed)[..2], ["你好", "拟好"]);
    assert_eq!(words(&listed)[listed.len() - 2..], ["甲", "乙"]);
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
fn initial_letter_expansion_reuses_warm_candidate_buffer() {
    let mut sql =
        String::from("BEGIN;CREATE TABLE tbl_1_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);");
    for index in 0..30 {
        sql.push_str(&format!(
            "INSERT INTO tbl_1_s VALUES('shi', 's', '字{index:02}', {});",
            1000 - index
        ));
    }
    sql.push_str("COMMIT;");
    let fixture = Fixture::new(&sql);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let typed = request("u", false);
    let mut candidates = engine.query(&typed, None);
    candidates.reserve(256);

    assert!(engine.expand_initial_candidates(&typed, &mut candidates));
    assert_eq!(candidates.len(), 30);
    assert!(
        candidates.capacity() >= 256,
        "预留候选容量不应在首字母展开时被丢弃：{}",
        candidates.capacity()
    );
}

#[test]
fn invalid_flat_initial_pairs_still_accept_enabled_fuzzy_partners() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_s VALUES('shuang','s','双',1000);\
CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_s VALUES('shuang''ren','sr','双人',10000),('shuang''shuang','ss','双双',9000);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for input in ["slrf", "slsl", "sl'rf"] {
        let mut typed = request(input, false);
        let expected = if input == "slsl" { "双双" } else { "双人" };
        assert!(!words(&engine.query(&typed, None)).contains(&expected));
        typed.fuzzy_pinyin.rules = fuzzy_rule::S_SH;
        let rows = engine.query(&typed, None);
        let row = &rows[index_of(&rows, expected)];
        assert_eq!(row.pinyin.replace('\'', ""), input.replace('\'', ""));
        assert_eq!(
            row.canonical_pinyin,
            if input == "slsl" {
                "shuang'shuang"
            } else {
                "shuang'ren"
            }
        );
        assert!(row.fuzzy);
        typed.fuzzy_pinyin.rules = fuzzy_rule::Z_ZH;
        assert!(!words(&engine.query(&typed, None)).contains(&expected));
    }
    let mut separated = request("s'lrf", false);
    separated.fuzzy_pinyin.rules = fuzzy_rule::S_SH;
    assert!(!words(&engine.query(&separated, None)).contains(&"双人"));
}

#[test]
fn invalid_fuzzy_initials_use_each_profiles_keys_and_preserve_helpcodes() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_s VALUES('shuang''ren','sr','双人',10000),('shuang''ren','sr','双刃',9000);",
    );
    for kind in [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
        ShuangpinProfileKind::SmartAbc,
    ] {
        let mut input = super::utils::encode_pinyin("shuang'ren", profile(kind).unwrap()).unwrap();
        input.replace_range(..1, "s");
        let mut typed = request(&input, false);
        typed.fuzzy_pinyin.rules = fuzzy_rule::S_SH;
        let mut engine = fixture.engine(kind);
        let rows = engine.query(&typed, None);
        let row = &rows[index_of(&rows, "双人")];
        assert_eq!(row.pinyin.replace('\'', ""), input.replace('\'', ""));
        assert_eq!(row.canonical_pinyin, "shuang'ren");
        assert!(row.fuzzy);
    }

    let codes = keymap(&[("双", "ab"), ("人", "cd"), ("刃", "ef")]);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for (input, expected) in [("slrfc", "双人"), ("slrfe", "双刃")] {
        let mut typed = request(input, true);
        typed.fuzzy_pinyin.rules = fuzzy_rule::S_SH;
        let rows = engine.query(&typed, Some(&codes));
        let row = &rows[index_of(&rows, expected)];
        assert_eq!(row.pinyin.replace('\'', ""), input);
        assert_eq!(row.canonical_pinyin, "shuang'ren");
    }
}

#[test]
fn invalid_fuzzy_initial_paths_share_dictionary_weights_and_helpcode_priority() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('nen','n','嫩',100);\
CREATE TABLE tbl_1_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_r VALUES('ren','r','人',10000);",
    );
    let codes = keymap(&[("嫩", "ab"), ("人", "cd")]);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for (input, expected) in [
        ("lf", ["人", "嫩"]),
        ("lfa", ["嫩", "人"]),
        ("lfc", ["人", "嫩"]),
    ] {
        let mut typed = request(input, true);
        typed.fuzzy_pinyin.rules = fuzzy_rule::N_L | fuzzy_rule::R_L;
        for _ in 0..2 {
            let rows = engine.query(&typed, Some(&codes));
            assert_eq!(words(&rows), expected, "input={input}");
            assert!(rows.iter().all(|row| row.fuzzy));
            assert!(rows
                .iter()
                .all(|row| input.starts_with(&row.pinyin.replace('\'', ""))));
        }
    }

    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('nen''hao','nh','嫩好',100);\
CREATE TABLE tbl_2_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_r VALUES('ren''hao','rh','人好',10000);",
    );
    let codes = keymap(&[("嫩", "ab"), ("人", "cd"), ("好", "ae")]);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for (input, expected) in [("lfhc", ["人好", "嫩好"]), ("lfhca", ["嫩好", "人好"])] {
        let mut typed = request(input, true);
        typed.fuzzy_pinyin.rules = fuzzy_rule::N_L | fuzzy_rule::R_L;
        for _ in 0..2 {
            let rows = engine.query(&typed, Some(&codes));
            assert_eq!(words(&rows), expected, "input={input}");
            assert!(rows.iter().all(|row| row.pinyin.replace('\'', "") == input));
        }
    }
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
    assert_eq!(words(&fuzzy), ["之", "字"]);
    assert_eq!(fuzzy[0].pinyin, "zi");
    assert!(fuzzy[0].fuzzy);
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

/// 完整词条保持首位；显示整句备选时，其他词网格读法仍排在它后面。
#[test]
fn exact_entry_leads_the_lattice_sentence() {
    let fixture = Fixture::new(
        "BEGIN;CREATE TABLE tbl_4_a(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_4_a VALUES('an''quan''bao''wei', 'aqbw', '安全保卫', 6000);CREATE TABLE tbl_2_a(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_a VALUES('an''quan', 'aq', '安全', 9000);CREATE TABLE tbl_2_b(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_2_b VALUES('bao''wei', 'bw', '包围', 9000);COMMIT;",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("anqrbcww", false);
    let best_only = engine.query(&typed, None);
    assert_eq!(words(&best_only).first(), Some(&"安全保卫"));
    assert!(best_only
        .iter()
        .all(|item| item.source != CandidateSource::Generated));
    typed.sentence_alternatives = true;
    let candidates = engine.query(&typed, None);
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

#[test]
fn ziranma_shouyou_real_dictionary_keeps_complete_phrases() {
    let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
        eprintln!("跳过：未设置 MSIME_EVAL_RESOURCES，真实词库回归未执行");
        return;
    };
    let user = tempfile::tempdir().unwrap();
    let resources = PathBuf::from(resources);
    let paths = RuntimePaths {
        resources: resources.clone(),
        user_data: user.path().to_path_buf(),
        cache: user.path().to_path_buf(),
        dictionaries: resources,
    };
    let profile = profile(ShuangpinProfileKind::Ziranma).unwrap();
    let codes = HelpcodeKeymap::from_table_bytes(include_bytes!(
        "../../../../resources/helpcodes/shouyou2_0_helpcode.txt"
    ))
    .unwrap();
    let mut engine = ShuangpinEngine::new(profile, &paths);
    for _ in 0..2 {
        for (reading, expected) in [
            ("shu'ru'fa", "输入法"),
            ("kao'shi'fen'shu", "考试分数"),
            ("ni'xi'huan'wo'ma", "你喜欢我吗"),
        ] {
            let typed = super::utils::encode_pinyin(reading, profile)
                .unwrap()
                .replace('\'', "");
            let plain = engine.query(&request(&typed, false), None);
            let with_helpcode = engine.query(&request(&typed, true), Some(&codes));
            assert_eq!(words(&plain)[0], expected, "普通读法：{typed}");
            assert_eq!(words(&with_helpcode)[0], expected, "首右辅助码：{typed}");
        }
    }
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

/// 模糊音关着时 `append_fuzzy` 不排序，辅助码的重排就是最后一道排序。自然码 `mzgrci` 的字面整句是没关词，纠错整句是没关系：大写单辅助码 `X` 按末字取系，只有纠错行匹配，会被排到最前；双辅助码 `mX`（首字取没、末字取系）滤掉了别的行，只剩纠错行。前者纠错行让到第二位，后者没有可让的位置，不给纠错行。
#[test]
fn a_helpcode_never_moves_the_typo_sentence_to_the_first_seat() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let codes = keymap(&[("没", "mb"), ("美", "mc"), ("关", "gd"), ("系", "xa")]);
    let with_helpcode = |typed: &str| QueryRequest {
        enable_shuangpin_helpcode: true,
        ..autocorrect_request(typed, true)
    };
    let mut engine = fixture.engine(ShuangpinProfileKind::Ziranma);
    // 同一个引擎来回查：命中辅助码缓存的答案同样要守住。
    for _ in 0..2 {
        let single = engine.query(&with_helpcode("mzgrciX"), Some(&codes));
        assert_eq!(index_of(&single, "没关系"), 1, "{:?}", words(&single));
        assert!(single[0].corrected_from.is_empty(), "{:?}", words(&single));

        let double = engine.query(&with_helpcode("mzgrcimX"), Some(&codes));
        assert!(double.is_empty(), "{:?}", words(&double));
    }
}

// ---- 按落点计价的纠错整句（#10018） ----

/// QWERTY 的字母键，键宽 10、键高 14，每排比上一排右移半个键。
fn qwerty_geometry() -> Arc<KeyGeometry> {
    qwerty_geometry_with_semicolon(None)
}

fn qwerty_geometry_with_semicolon(semicolon: Option<[f32; 4]>) -> Arc<KeyGeometry> {
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
    Arc::new(KeyGeometry::new(11, &keys, semicolon, &[]).unwrap())
}

/// 测试里固定的计价参数，不随默认值的调参变化。
fn test_tuning() -> TouchTuning {
    TouchTuning {
        base: 12.0,
        scale: 8.0,
        sigma_x: 1.0,
        sigma_y: 1.0,
        bias_y: 0.0,
        min_penalty: 4.0,
        max_penalty: 28.0,
        cut: 16.0,
        show_margin: 0.0,
        show_penalty: f64::INFINITY,
        lead_margin: f64::INFINITY,
        lead_penalty: f64::INFINITY,
    }
}

/// `typed` 每个键都带触点：`leaning` 列出的位置按 (位置, dx, dy) 偏，其余按在键心。
fn touched_request(typed: &str, leaning: &[(usize, i8, i8)]) -> QueryRequest {
    QueryRequest {
        key_touches: (0..typed.len())
            .map(|index| {
                let (dx, dy) = leaning
                    .iter()
                    .find(|(position, _, _)| *position == index)
                    .map_or((0, 0), |&(_, dx, dy)| (dx, dy));
                Some(KeyTouch { dx, dy, copy: 0 })
            })
            .collect(),
        key_geometry: Some(qwerty_geometry()),
        touch_tuning: test_tuning(),
        ..autocorrect_request(typed, true)
    }
}

/// 不带纠错行的那些行：触点只给纠错整句计价，字面候选一行不变。
fn literal_words(candidates: &[WordItem]) -> Vec<&str> {
    candidates
        .iter()
        .filter(|item| item.corrected_from.is_empty())
        .map(|item| item.word.as_str())
        .collect()
}

/// 短词只有查表路径：字面词频刻意低于邻键目标，首选仍须保留。
const SHORT_TOUCH_FIXTURE: &str = "
CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('guan','g','关',2000000),('gei','g','给',1000000);
CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_m VALUES('mei','m','没',1000);
CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_m VALUES('mei''guan','mg','美观',1000000);";

fn short_touch_request(typed: &str, leaning: &[(usize, i8, i8)]) -> QueryRequest {
    QueryRequest {
        touch_tuning: TouchTuning::default(),
        ..touched_request(typed, leaning)
    }
}

#[test]
fn short_touch_words_correct_one_and_two_syllables_without_changing_the_leader() {
    let fixture = Fixture::new(SHORT_TOUCH_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for (typed, position, target, reading) in
        [("gf", 1, "关", "guan"), ("mwgf", 3, "美观", "mei'guan")]
    {
        let flat = engine.query(&autocorrect_request(typed, true), None);
        assert!(!flat.is_empty());
        let leaning = short_touch_request(typed, &[(position, -5, -10)]);
        for _ in 0..2 {
            let rows = engine.query(&leaning, None);
            assert_eq!(rows[0], flat[0]);
            let at = rows
                .iter()
                .position(|row| row.word == target && !row.corrected_from.is_empty())
                .expect("短词纠错");
            assert!(at > 0 && at < 6, "{:?}", words(&rows));
            assert_eq!(rows[at].pinyin, typed);
            assert_eq!(rows[at].canonical_pinyin, reading);
            assert!(rows[at].touch_supported);
            assert!(!rows[at].confident_correction);
            assert_eq!(engine.query(&autocorrect_request(typed, true), None), flat);
        }
        let centered = engine.query(&short_touch_request(typed, &[]), None);
        assert!(centered.iter().all(|row| row.corrected_from.is_empty()));
    }
}

/// 断言多个短词纠错候选按罚分后的词频排列：这个顺序来自降序排序加逐个按序插入，本用例守住这条不变量。
#[test]
fn short_touch_words_keep_multiple_corrections_in_rank_order() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_g VALUES('gen','g','跟',1000),('guan','g','关',2000000),('gai','g','改',2000000000);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let rows = engine.query(&short_touch_request("gf", &[(1, -5, -10)]), None);
    let corrected: Vec<&str> = rows
        .iter()
        .filter(|row| !row.corrected_from.is_empty())
        .map(|row| row.word.as_str())
        .collect();
    assert_eq!(corrected, ["改", "关"]);
}

#[test]
fn short_touch_words_require_the_switch_geometry_and_complete_touches() {
    let fixture = Fixture::new(SHORT_TOUCH_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let enabled = short_touch_request("gf", &[(1, -5, -10)]);
    let mut requests = Vec::new();
    let mut off = enabled.clone();
    off.enable_quanpin_autocorrect_neighbor = false;
    off.enable_quanpin_autocorrect_transposition = false;
    requests.push(off);
    let mut physical = enabled.clone();
    physical.key_touches.clear();
    requests.push(physical);
    let mut no_geometry = enabled.clone();
    no_geometry.key_geometry = None;
    requests.push(no_geometry);
    let mut partial = enabled.clone();
    partial.key_touches[0] = None;
    requests.push(partial);
    let mut helpcode = enabled.clone();
    helpcode.end_helpcodes = "a".into();
    requests.push(helpcode);
    let mut marked = enabled.clone();
    marked.raw_input_with_helpcode_marks = "g^f".into();
    requests.push(marked);
    for request in requests {
        let rows = engine.query(&request, None);
        assert!(
            rows.iter().all(|row| row.corrected_from.is_empty()),
            "{:?}",
            words(&rows)
        );
    }
    assert!(engine
        .query(&enabled, None)
        .iter()
        .any(|row| row.word == "关" && !row.corrected_from.is_empty()));
    let helpers_enabled = QueryRequest {
        enable_shuangpin_helpcode: true,
        ..enabled
    };
    assert!(
        engine
            .query(&helpers_enabled, None)
            .iter()
            .any(|row| row.word == "关" && !row.corrected_from.is_empty()),
        "启用辅助码本身不应关闭普通两键纠错"
    );
}

#[test]
fn invalid_short_touch_codes_can_replace_a_prefix_with_a_complete_correction() {
    let fixture = Fixture::new(SHORT_TOUCH_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    assert!(super::utils::cvt_single_sp_to_pinyin(
        "gq",
        profile(ShuangpinProfileKind::Xiaohe).unwrap()
    )
    .is_empty());
    let request = short_touch_request("gq", &[(1, 14, 0)]);
    let flat = engine.query(&autocorrect_request("gq", true), None);
    let rows = engine.query(&request, None);
    assert_eq!(rows[0], flat[0]);
    let correction = rows
        .iter()
        .find(|row| row.word == "给" && !row.corrected_from.is_empty())
        .expect("非法两键的邻键纠错");
    assert_eq!(correction.pinyin, "gq");
    assert_eq!(correction.canonical_pinyin, "gei");
    assert_eq!(rows.iter().filter(|row| row.word == "给").count(), 1);
    let pairs = super::typo_edges::touch_correction_pairs(
        profile(ShuangpinProfileKind::Xiaohe).unwrap(),
        "gq",
        &request.key_touches,
        request.key_geometry.as_deref().unwrap(),
        &request.touch_tuning,
        super::typo_edges::LearnedTypos::default(),
        "gei",
        crate::types::autocorrect_type::NEIGHBOR,
    )
    .unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].typed, *b"gq");
    assert_eq!(pairs[0].intended, *b"gw");
}

#[test]
fn short_touch_words_use_the_microsoft_semicolon_geometry() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_y VALUES('ya','y','丫',2000000),('ying','y','营',1000000);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Microsoft);
    let touched = QueryRequest {
        key_geometry: Some(qwerty_geometry_with_semicolon(Some([
            100.0, 21.0, 10.0, 14.0,
        ]))),
        ..short_touch_request("yl", &[(1, 14, 0)])
    };
    assert_eq!(
        super::utils::encode_pinyin("ying", profile(ShuangpinProfileKind::Microsoft).unwrap())
            .as_deref(),
        Some("y;")
    );
    let rows = engine.query(&touched, None);
    let row = rows
        .iter()
        .find(|row| row.word == "营" && !row.corrected_from.is_empty())
        .expect("分号邻键纠错");
    assert_eq!(row.pinyin, "yl");
    assert_eq!(row.canonical_pinyin, "ying");
    assert_eq!(rows[0].word, "丫");
}

#[test]
fn short_touch_words_refresh_learned_prices_without_changing_physical_keys() {
    let fixture = Fixture::new(SHORT_TOUCH_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let touched = short_touch_request("gf", &[(1, -2, -4)]);
    let flat = autocorrect_request("gf", true);
    let before = engine.query(&touched, None);
    let physical = engine.query(&flat, None);
    assert!(before.iter().all(|row| row.corrected_from.is_empty()));
    let profile = PersonalTypoProfile::shared(&fixture.paths.user(assets::USER_JOURNAL));
    for _ in 0..3 {
        profile
            .record_shuangpin(
                ShuangpinProfileKind::Xiaohe,
                &[(*b"gf", *b"gr")],
                ShuangpinTypoOutcome::Accepted,
            )
            .unwrap();
    }
    let after = engine.query(&touched, None);
    assert!(after
        .iter()
        .any(|row| row.word == "关" && !row.corrected_from.is_empty()));
    assert_eq!(after[0], before[0]);
    assert_eq!(engine.query(&flat, None), physical);
}

#[test]
fn short_touch_transpositions_keep_the_existing_threshold_and_type_switch() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_f VALUES('feng','f','风',1000);
        CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_g VALUES('gen','g','跟',2000000);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut touched = short_touch_request("fg", &[]);
    touched.enable_quanpin_autocorrect_neighbor = false;
    let before = engine.query(&touched, None);
    // 未学习的对调罚分高于默认展示阈值，不能借短词路径放宽它。
    assert!(before.iter().all(|row| row.corrected_from.is_empty()));
    let profile = PersonalTypoProfile::shared(&fixture.paths.user(assets::USER_JOURNAL));
    for _ in 0..8 {
        profile
            .record_shuangpin(
                ShuangpinProfileKind::Xiaohe,
                &[(*b"fg", *b"gf")],
                ShuangpinTypoOutcome::Accepted,
            )
            .unwrap();
    }
    let learned = engine.query(&touched, None);
    assert_eq!(learned[0], before[0]);
    assert!(learned
        .iter()
        .any(|row| row.word == "跟" && row.corrected_from == "fg"));
    touched.enable_quanpin_autocorrect_transposition = false;
    touched.enable_quanpin_autocorrect_neighbor = true;
    assert!(engine
        .query(&touched, None)
        .iter()
        .all(|row| row.corrected_from.is_empty()));
}

/// 小鹤 `mwgfxi`：想打 gr（关）却按到了 f。f 的落点偏向 r 时这次改正最便宜，按在 f 键心时比平面表的 20 还贵。同一串字母带不同触点各有各的缓存答案，来回查不串。
#[test]
fn touches_price_the_correction_and_never_share_a_cached_answer() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let toward_r = touched_request("mwgfxi", &[(3, -5, -10)]);
    let centered = touched_request("mwgfxi", &[]);
    let flat = autocorrect_request("mwgfxi", true);
    let weight = |candidates: &[WordItem]| {
        assert_corrected(candidates, "mwgfxi");
        typo_row(candidates).unwrap().weight
    };
    let mut seen = None;
    for _ in 0..2 {
        let leaning = engine.query(&toward_r, None);
        let plain = engine.query(&flat, None);
        let center = engine.query(&centered, None);
        let weights = (weight(&leaning), weight(&plain), weight(&center));
        assert!(
            weights.0 > weights.1 && weights.1 > weights.2,
            "{weights:?}"
        );
        assert_eq!(literal_words(&leaning), literal_words(&plain));
        assert_eq!(literal_words(&center), literal_words(&plain));
        // 第二轮全部命中缓存，答案与第一轮相同。
        assert_eq!(*seen.get_or_insert(weights), weights);
    }
    // 换一份几何（代次不同）也不读旧答案：键距放大到 1.5 倍后同一个偏移离 r 更远，价更高。
    let mut keys = [[0.0f32; 4]; 26];
    for (letters, left, y) in [
        ("qwertyuiop", 7.5, 10.5),
        ("asdfghjkl", 15.0, 31.5),
        ("zxcvbnm", 30.0, 52.5),
    ] {
        for (column, letter) in letters.bytes().enumerate() {
            keys[usize::from(letter - b'a')] = [left + 15.0 * column as f32, y, 10.0, 14.0];
        }
    }
    let spread = QueryRequest {
        key_geometry: Some(Arc::new(KeyGeometry::new(12, &keys, None, &[]).unwrap())),
        ..toward_r.clone()
    };
    assert!(weight(&engine.query(&spread, None)) < seen.unwrap().0);
    assert_eq!(weight(&engine.query(&toward_r, None)), seen.unwrap().0);
}

/// 纠错整句只有在落点支持时才标上 `touch_supported`（runtime 据此不把它挪到表尾）：分数不比字面整句低出 `show_margin`，而且改动的那一键改过去的价不超过 `show_penalty`。没有触点时永远不标。
#[test]
fn the_correction_keeps_its_seat_only_when_the_touch_supports_it() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut supported = |request: &QueryRequest| {
        let candidates = engine.query(request, None);
        typo_row(&candidates)
            .unwrap_or_else(|| panic!("no typo sentence in {:?}", words(&candidates)))
            .touch_supported
    };
    let with = |leaning: &[(usize, i8, i8)], tuning: TouchTuning| QueryRequest {
        touch_tuning: tuning,
        ..touched_request("mwgfxi", leaning)
    };
    let strict = TouchTuning {
        show_penalty: 10.0,
        ..test_tuning()
    };
    // f 那一键落在 r 的键心上，改过去的价是下限 4；落在 f 和 r 正中，价是 B = 12。
    assert!(supported(&with(&[(3, -10, -20)], strict)));
    assert!(!supported(&with(&[(3, -5, -10)], strict)));
    assert!(supported(&with(&[(3, -5, -10)], test_tuning())));
    // 分数差的线压到字面整句之上很远时，同一个落点也不算支持。
    let demanding = TouchTuning {
        show_margin: -100.0,
        ..test_tuning()
    };
    assert!(!supported(&with(&[(3, -10, -20)], demanding)));
    assert!(!supported(&autocorrect_request("mwgfxi", true)));
}

/// 按错的那一键几乎落在想按的键心上时，这次改正的价降到下限，纠错整句仍然只排在字面整句之后，不抢首选（首选规则是后面一期的事）。
#[test]
fn a_touched_correction_never_takes_the_first_seat() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    for (kind, typed, position) in [
        (ShuangpinProfileKind::Xiaohe, "mwgfxi", 3),
        (ShuangpinProfileKind::Ziranma, "mzgrci", 4),
    ] {
        let mut engine = fixture.engine(kind);
        // 小鹤按到 f、想按左上方的 r；自然码按到 c、想按左边的 x。
        let mut request = touched_request(typed, &[]);
        let toward = match kind {
            ShuangpinProfileKind::Xiaohe => (-10, -20),
            _ => (-20, 0),
        };
        request.key_touches[position] = Some(KeyTouch {
            dx: toward.0,
            dy: toward.1,
            copy: 0,
        });
        let candidates = engine.query(&request, None);
        assert_corrected(&candidates, typed);
        assert!(candidates[0].corrected_from.is_empty(), "{kind:?}");
    }
}

/// 首选规则可用时的计价参数：分数比字面整句高就算明显更优（m_sp = 0），改动的那一键原价不超过 10。
fn lead_tuning() -> TouchTuning {
    TouchTuning {
        lead_margin: 0.0,
        lead_penalty: 10.0,
        ..test_tuning()
    }
}

/// 小鹤 `mwgfxi`，f 那一键按在 r 的键心（改过去的价是下限 4）；`tuning` 决定首选规则。
fn leaning_request(tuning: TouchTuning) -> QueryRequest {
    QueryRequest {
        touch_tuning: tuning,
        ..touched_request("mwgfxi", &[(3, -10, -20)])
    }
}

/// 纠错整句按首选规则坐上第一位：标上 `confident_correction`（同时是落点支持的那一行），字面整句紧跟在第二位。
fn assert_confident(candidates: &[WordItem]) {
    let first = &candidates[0];
    assert_eq!(first.word, "没关系", "{:?}", words(candidates));
    assert!(first.confident_correction && first.touch_supported);
    assert_eq!(first.corrected_from, "mwgfxi");
    assert_eq!(first.canonical_pinyin, "mei'guan'xi");
    assert_eq!(candidates[1].word, "没跟系");
    assert!(candidates[1].sentence_association && candidates[1].corrected_from.is_empty());
    assert!(candidates[1..].iter().all(|row| !row.confident_correction));
}

/// 纠错整句还在表里，但没坐首选，也没有一行带首选标记。
fn assert_not_confident(candidates: &[WordItem]) {
    assert_corrected(candidates, "mwgfxi");
    assert!(
        candidates[0].corrected_from.is_empty(),
        "{:?}",
        words(candidates)
    );
    assert!(candidates.iter().all(|row| !row.confident_correction));
}

/// 首选规则全部满足时纠错整句坐第一位，命中缓存时也一样；默认的无穷大 m_sp 下照旧不抢首选，同一串按键带同样的触点在两种参数下各有各的答案。
#[test]
fn a_clearly_better_supported_correction_takes_the_first_seat() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for _ in 0..2 {
        assert_confident(&engine.query(&leaning_request(lead_tuning()), None));
        assert_not_confident(&engine.query(&leaning_request(test_tuning()), None));
    }
    assert_eq!(TouchTuning::default().lead_margin, f64::INFINITY);
    // 自然码同理：按到 c、落点在 x 的键心。
    let mut ziranma = fixture.engine(ShuangpinProfileKind::Ziranma);
    let mut request = touched_request("mzgrci", &[(4, -20, 0)]);
    request.touch_tuning = lead_tuning();
    let candidates = ziranma.query(&request, None);
    assert_eq!(candidates[0].word, "没关系");
    assert!(candidates[0].confident_correction);
}

/// (c) 按在键心的字母永远不会被首选改掉：分数差的线放得再低，键心落点改到邻键的原价（B + s = 20）也过不了 `lead_penalty`；落在两键正中（价是 B = 12）时要 `lead_penalty` 放到 12 才行。
#[test]
fn a_key_centre_touch_never_puts_a_correction_first() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let eager = TouchTuning {
        lead_margin: -1000.0,
        lead_penalty: 19.0,
        ..test_tuning()
    };
    let centred = QueryRequest {
        touch_tuning: eager,
        ..touched_request("mwgfxi", &[])
    };
    assert_not_confident(&engine.query(&centred, None));
    let between = |lead_penalty| QueryRequest {
        touch_tuning: TouchTuning {
            lead_penalty,
            ..lead_tuning()
        },
        ..touched_request("mwgfxi", &[(3, -5, -10)])
    };
    assert_not_confident(&engine.query(&between(11.0), None));
    assert_confident(&engine.query(&between(12.0), None));
}

/// (b) 分数要比字面整句高出每条纠错边 `lead_margin`：线抬到高过这次的分数差就不坐首选。
#[test]
fn a_correction_leads_only_by_the_margin() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let candidates = engine.query(&leaning_request(lead_tuning()), None);
    let literal = candidates[1].weight as f64 / 1000.0;
    let lead = candidates[0].weight as f64 / 1000.0 - literal;
    assert!(lead > 1.0, "{lead}");
    let with_margin = |lead_margin| TouchTuning {
        lead_margin,
        ..lead_tuning()
    };
    assert_confident(&engine.query(&leaning_request(with_margin(lead - 0.5)), None));
    assert_not_confident(&engine.query(&leaning_request(with_margin(lead + 0.5)), None));
}

/// (a) 每个键都要有触点：一键没有触点（宿主没送、或撤回选词重打的那一段）就不坐首选，纠错行照旧按落点计价排在后面。
#[test]
fn a_correction_leads_only_when_every_key_was_touched() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for missing in [0, 3, 5] {
        let mut request = leaning_request(lead_tuning());
        request.key_touches[missing] = None;
        if missing == 3 {
            // 按错的那一键没有触点时照平面表付 20，落点支持不了它。
            let candidates = engine.query(&request, None);
            assert!(candidates.iter().all(|row| !row.confident_correction));
            assert!(candidates[0].corrected_from.is_empty());
        } else {
            assert_not_confident(&engine.query(&request, None));
        }
    }
    assert_confident(&engine.query(&leaning_request(lead_tuning()), None));
}

/// (d) 字面有全码词条时不抢：词条排在第一位，纠错整句照旧坐在整句块之后。
#[test]
fn a_literal_dictionary_entry_keeps_the_first_seat() {
    let fixture = Fixture::new(&TYPO_FIXTURE.replace(
        "CREATE TABLE tbl_3_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);",
        "CREATE TABLE tbl_3_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_3_m VALUES('mei''gen''xi','mgx','没跟系',10);",
    ));
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let candidates = engine.query(&leaning_request(lead_tuning()), None);
    assert_eq!(candidates[0].word, "没跟系");
    assert!(candidates[0].source.is_dictionary());
    let typo = typo_row(&candidates).expect("the typo sentence is still offered");
    assert_eq!(typo.word, "没关系");
    assert!(index_of(&candidates, "没关系") > 0);
    assert!(candidates.iter().all(|row| !row.confident_correction));
}

/// (e) 这一对两键码被拒绝过（首选是纠错行、用户选了别的行）就一票否决首选，纠错行仍在后面；接受过的不否决。按旧档案算的缓存答案随档案代次作废。
#[test]
fn a_rejected_pair_vetoes_the_first_seat() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    assert_confident(&engine.query(&leaning_request(lead_tuning()), None));
    let profile = PersonalTypoProfile::shared(&fixture.paths.user(assets::USER_JOURNAL));
    profile
        .record_shuangpin(
            ShuangpinProfileKind::Xiaohe,
            &[(*b"gf", *b"gr")],
            ShuangpinTypoOutcome::Accepted,
        )
        .unwrap();
    assert_confident(&engine.query(&leaning_request(lead_tuning()), None));
    // 别的方案、别的码对的拒绝不算。
    profile
        .record_shuangpin(
            ShuangpinProfileKind::Ziranma,
            &[(*b"gf", *b"gr")],
            ShuangpinTypoOutcome::Rejected,
        )
        .unwrap();
    profile
        .record_shuangpin(
            ShuangpinProfileKind::Xiaohe,
            &[(*b"gf", *b"gd")],
            ShuangpinTypoOutcome::Rejected,
        )
        .unwrap();
    assert_confident(&engine.query(&leaning_request(lead_tuning()), None));
    profile
        .record_shuangpin(
            ShuangpinProfileKind::Xiaohe,
            &[(*b"gf", *b"gr")],
            ShuangpinTypoOutcome::Rejected,
        )
        .unwrap();
    assert_not_confident(&engine.query(&leaning_request(lead_tuning()), None));
    // 新开的引擎读到同一份日志，结果一样。
    assert_not_confident(
        &fixture
            .engine(ShuangpinProfileKind::Xiaohe)
            .query(&leaning_request(lead_tuning()), None),
    );
}

/// (f) 辅助码在起作用时不坐首选：末尾单辅助码读法下，同一串按键带同样的触点照旧把纠错行让到后面，而且不会读到允许首选时缓存下的整句答案；辅助码开着但这串输入没有辅助码时照常坐首选。
#[test]
fn an_active_helpcode_keeps_the_correction_off_the_first_seat() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let codes = keymap(&[("没", "ab"), ("关", "cd"), ("跟", "cd"), ("系", "mm")]);
    let with_helpcode = |typed: &str| {
        let mut request = touched_request(typed, &[(3, -10, -20)]);
        request.touch_tuning = lead_tuning();
        request.enable_shuangpin_helpcode = true;
        request
    };
    assert_confident(&engine.query(&with_helpcode("mwgfxi"), Some(&codes)));
    let helped = engine.query(&with_helpcode("mwgfxim"), Some(&codes));
    assert!(
        helped.iter().all(|row| !row.confident_correction),
        "{:?}",
        words(&helped)
    );
    assert!(helped
        .first()
        .is_none_or(|row| row.corrected_from.is_empty()));
    // 与没有首选规则（m_sp 无穷大）的新引擎逐行相同：没有读到上一键允许首选时缓存下的整句答案。
    let mut plain = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let unruled = QueryRequest {
        touch_tuning: test_tuning(),
        ..with_helpcode("mwgfxi")
    };
    plain.query(&unruled, Some(&codes));
    let expected = plain.query(
        &QueryRequest {
            touch_tuning: test_tuning(),
            ..with_helpcode("mwgfxim")
        },
        Some(&codes),
    );
    assert_eq!(words(&helped), words(&expected));
    assert_confident(&engine.query(&with_helpcode("mwgfxi"), Some(&codes)));
}

/// 模糊音打开、全码只有模糊词条时，`sort_by_coverage` 把模糊词条排到按首选规则坐上第一位的纠错整句前面：纠错行不再是首选，清掉标记、回到字面整句块之后，与没有首选规则（m_sp 无穷大）的新引擎逐行相同，不成为排在字面整句前面又没有标记的改正。命中缓存时也一样。
#[test]
fn a_fuzzy_word_ahead_sends_the_correction_back_behind_the_literal_sentence() {
    let fixture = Fixture::new(&TYPO_FIXTURE.replace(
        "COMMIT;",
        "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_n VALUES('nei','n','内',1000);CREATE TABLE tbl_1_l(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_1_l VALUES('lei','l','累',1000);CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);CREATE TABLE tbl_3_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);CREATE TABLE tbl_2_l(key TEXT, jp TEXT, value TEXT, weight INTEGER);CREATE TABLE tbl_3_l(key TEXT, jp TEXT, value TEXT, weight INTEGER);INSERT INTO tbl_3_l VALUES('lei''gen''xi','lgx','累跟系',10);COMMIT;",
    ));
    // 小鹤 `nwgfxi` 是 nei'gen'xi，f 那一键按在 r 的键心，纠错整句是内关系；n/l 模糊音下 `lwgfxi` 的全码词条是累跟系。
    let touched = |tuning: TouchTuning, fuzzy: bool| {
        let mut request = QueryRequest {
            touch_tuning: tuning,
            ..touched_request("nwgfxi", &[(3, -10, -20)])
        };
        if fuzzy {
            request.fuzzy_pinyin.rules = fuzzy_rule::N_L;
        }
        request
    };
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    // 前提：没有模糊音时这一行按首选规则坐第一位。
    let plain = engine.query(&touched(lead_tuning(), false), None);
    assert_eq!(plain[0].word, "内关系", "{:?}", words(&plain));
    assert!(plain[0].confident_correction);
    let expected = fixture
        .engine(ShuangpinProfileKind::Xiaohe)
        .query(&touched(test_tuning(), true), None);
    for _ in 0..2 {
        let candidates = engine.query(&touched(lead_tuning(), true), None);
        assert_eq!(candidates[0].word, "累跟系", "{:?}", words(&candidates));
        assert!(candidates[0].fuzzy && candidates[0].source.is_dictionary());
        assert!(candidates.iter().all(|row| !row.confident_correction));
        let literal = candidates
            .iter()
            .position(|row| row.sentence_association && row.corrected_from.is_empty())
            .unwrap_or_else(|| panic!("no literal sentence in {:?}", words(&candidates)));
        assert_eq!(candidates[literal].word, "内跟系");
        let typo = index_of(&candidates, "内关系");
        assert!(literal < typo, "{:?}", words(&candidates));
        assert_eq!(words(&candidates), words(&expected));
    }
}

/// 自定义方案不学，拒绝没有记录可查，所以不坐首选；纠错行照旧在后面。
#[test]
fn a_custom_profile_never_puts_a_correction_first() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
    let owned = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(unit, key)| ((*unit).to_owned(), (*key).to_owned()))
            .collect()
    };
    // 小鹤的韵母，zh、ch、sh 换了键：`mwgfxi` 仍是 mei'gen'xi。
    let custom = crate::shuangpin::custom::custom_profile(&crate::types::ShuangpinCustomTable {
        initials: owned(&[("zh", "a"), ("ch", "i"), ("sh", "u")]),
        finals: owned(xiaohe.finals),
        zero_initials: owned(&[
            ("a", "oa"),
            ("ai", "od"),
            ("an", "oj"),
            ("ang", "oh"),
            ("ao", "oc"),
            ("e", "oe"),
            ("ei", "ow"),
            ("en", "of"),
            ("eng", "og"),
            ("er", "or"),
            ("o", "oo"),
            ("ou", "oz"),
        ]),
    })
    .unwrap();
    assert_eq!(custom.kind, ShuangpinProfileKind::Custom);
    let mut engine = ShuangpinEngine::new(custom, &fixture.paths);
    let flat = engine.query(&leaning_request(test_tuning()), None);
    assert_corrected(&flat, "mwgfxi");
    let candidates = engine.query(&leaning_request(lead_tuning()), None);
    assert!(candidates.iter().all(|row| !row.confident_correction));
    assert!(
        candidates[0].corrected_from.is_empty(),
        "{:?}",
        words(&candidates)
    );
}

/// 用户接受过 3 次 gf → gr（小鹤：想打 guan 却按到了 gen）之后，同一个落点改过去的价少了 `learned_discount(3)`：纠错整句的分升高，原来过不了 `show_penalty` 的这次改正留在了座位上。按旧学习结果计价的缓存答案随档案代次作废。没有触点的同一串输入（平面表）不论新旧引擎都一行不变；别的方案没有这条记录，不打折。
#[test]
fn accepted_shuangpin_typos_discount_only_the_touch_path() {
    let fixture = Fixture::new(TYPO_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let strict = TouchTuning {
        show_penalty: 10.0,
        ..test_tuning()
    };
    // f 那一键落在 f 和 r 正中：改过去的价是 B = 12，超过 10。
    let touched = QueryRequest {
        touch_tuning: strict,
        ..touched_request("mwgfxi", &[(3, -5, -10)])
    };
    let flat = autocorrect_request("mwgfxi", true);
    let before = engine.query(&touched, None);
    let flat_before = engine.query(&flat, None);
    let row = typo_row(&before).unwrap().clone();
    assert!(!row.touch_supported);

    let profile = PersonalTypoProfile::shared(&fixture.paths.user(assets::USER_JOURNAL));
    for _ in 0..3 {
        profile
            .record_shuangpin(
                ShuangpinProfileKind::Xiaohe,
                &[(*b"gf", *b"gr")],
                ShuangpinTypoOutcome::Accepted,
            )
            .unwrap();
    }
    let after = engine.query(&touched, None);
    let learned = typo_row(&after).unwrap();
    assert_eq!(learned.word, row.word);
    assert!(learned.touch_supported);
    let gained = (learned.weight - row.weight) as f64 / 1000.0;
    assert!((gained - learned_discount(3)).abs() < 0.01, "{gained}");
    assert_eq!(literal_words(&after), literal_words(&before));

    assert_eq!(engine.query(&flat, None), flat_before);
    assert_eq!(
        fixture
            .engine(ShuangpinProfileKind::Xiaohe)
            .query(&flat, None),
        flat_before
    );
    // 微软双拼没有这条记录：同一串按键在它那里照旧计价（这里只比较带不带折扣，不要求有纠错行）。
    let mut microsoft = fixture.engine(ShuangpinProfileKind::Microsoft);
    let fresh = Fixture::new(TYPO_FIXTURE);
    let mut unlearned = fresh.engine(ShuangpinProfileKind::Microsoft);
    assert_eq!(
        microsoft.query(&touched, None),
        unlearned.query(&touched, None)
    );
}

#[test]
fn complete_shuangpin_keeps_its_reading_when_direct_helpcodes_also_match() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_s VALUES('shu','s','书',100),('shu','s','叔',90),\
         ('shang','s','商',100),('su','s','苏',100),('sheng','s','声',100);\
         CREATE TABLE tbl_1_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_r VALUES('ru','r','如',100);\
         CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_h VALUES('he','h','和',100);\
         CREATE TABLE tbl_3_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_s VALUES('shu''ru''he','srh','书如和',100);\
         CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_s VALUES('shu''shang','ss','叔商',10000),\
         ('su''shang','ss','苏商',10000),('shu''sheng','ss','叔声',10000);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let codes = keymap(&[("叔", "ra"), ("苏", "ra"), ("商", "ea"), ("声", "ea")]);
    let fuzzy = FuzzyPinyinOptions {
        rules: fuzzy_rule::S_SH,
    };

    // 合成词条同时命中三音节双拼和两音节直接辅助码；重复查询覆盖两条缓存。
    for _ in 0..2 {
        for (typed, fuzzy_enabled, direct) in [("uuruhe", false, "叔商"), ("suruhe", true, "苏商")]
        {
            let mut query = request(typed, true);
            if fuzzy_enabled {
                query.fuzzy_pinyin = fuzzy;
            }
            let candidates = engine.query(&query, Some(&codes));
            let ordinary = index_of(&candidates, "书如和");
            assert!(
                ordinary < index_of(&candidates, direct),
                "{typed}: {:?}",
                words(&candidates)
            );
            let candidate = &candidates[ordinary];
            assert_eq!(candidate.canonical_pinyin, "shu'ru'he");
            assert_eq!(candidate.pinyin.replace('\'', ""), typed);
        }
    }

    // 大写明确指定音节内双辅助码时，仍然按约束选词。
    let explicit = engine.query(&request("uurAuh", true), Some(&codes));
    assert_eq!(words(&explicit)[0], "叔商");
    assert_eq!(explicit[0].pinyin, "uurauh");

    // 普通双拼无完整候选时保留直接辅助码优先级。
    let fallback = engine.query(&request("uuruge", true), Some(&codes));
    assert_eq!(words(&fallback)[0], "叔声");

    // 没有整词也不能抢占普通逐音节组句。
    Connection::open(fixture.paths.dictionary(assets::MAIN_DICTIONARY))
        .unwrap()
        .execute("DELETE FROM tbl_3_s", [])
        .unwrap();
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let generated = engine.query(&request("uuruhe", true), Some(&codes));
    assert!(index_of(&generated, "书如和") < index_of(&generated, "叔商"));
    assert!(generated[index_of(&generated, "书如和")].sentence_association);
}

#[test]
fn direct_helpcode_accepts_mobile_sentence_examples() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_s VALUES('shi','s','实',100),('shi','s','使',90);\
         CREATE TABLE tbl_2_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_s VALUES('shi''jian','sj','实践',100);\
         CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_t VALUES('ta','t','他',100),('ta','t','她',90),('ta','t','它',80);\
         CREATE TABLE tbl_3_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_t VALUES('ta''ta''ta','ttt','他她它',100);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Ziranma);
    let codes = keymap(&[
        ("实", "bt"),
        ("使", "zz"),
        ("他", "ra"),
        ("她", "na"),
        ("它", "ba"),
    ]);

    // 重复查询同时覆盖普通候选缓存命中后的辅助码过滤和输入范围。
    for _ in 0..2 {
        for (typed, expected) in [
            ("uib", "实"),
            ("uibT", "实"),
            ("uibjm", "实践"),
            ("uibTjm", "实践"),
            ("Uibjm", "实践"),
            ("tartantab", "他她它"),
            ("tartantabA", "他她它"),
        ] {
            let candidates = engine.query(&request(typed, true), Some(&codes));
            assert_eq!(words(&candidates)[0], expected, "{typed}");
            if typed.len() >= 4 {
                assert_eq!(candidates[0].pinyin, typed.to_ascii_lowercase(), "{typed}");
            }
        }
    }
    let disabled = engine.query(&request("uibjm", false), Some(&codes));
    assert!(!words(&disabled).contains(&"实践"));
    let no_keymap = engine.query(&request("uibjm", true), None);
    assert!(!words(&no_keymap).contains(&"实践"));
}

#[test]
fn direct_helpcode_builds_an_unstored_sentence_before_pruning_homophones() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_t VALUES('ta','t','他',100),('ta','t','她',90),('ta','t','它',1);\
         WITH RECURSIVE filler(n) AS (SELECT 0 UNION ALL SELECT n+1 FROM filler WHERE n<39)\
         INSERT INTO tbl_1_t SELECT 'ta','t',char(19968+n),1000 FROM filler;",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Ziranma);
    let codes = keymap(&[("他", "ra"), ("她", "na"), ("它", "ba")]);
    let candidates = engine.query(&request("tartantab", true), Some(&codes));
    assert_eq!(words(&candidates)[0], "他她它");
    assert_eq!(candidates[0].canonical_pinyin, "ta'ta'ta");
    assert_eq!(candidates[0].pinyin, "tartantab");
}

#[test]
fn unmatched_trailing_helpcode_keeps_longer_prefix_first() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_f VALUES('feng''kuang','fk','疯狂',1000);\
         CREATE TABLE tbl_3_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_f VALUES('feng''kuang''shui','fks','疯狂水',900);\
         CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_s VALUES('shui','s','水',800);\
         CREATE TABLE tbl_4_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let candidates = engine.query(&request("fgkluvq", true), Some(&keymap(&[("水", "ln")])));
    assert_eq!(words(&candidates)[0], "疯狂水");
}

#[test]
fn trailing_initial_completes_a_generated_sentence_with_canonical_reading() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);\
         CREATE TABLE tbl_2_k(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_k VALUES('ke''ji','kj','科技',900);\
         CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_g VALUES('gong','g','工',800);\
         CREATE TABLE tbl_2_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_g VALUES('gong''ju','gj','工具',700),\
         ('gong''ji','gj','公鸡',100),('gong''zuo','gz','工作',9000);",
    );
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for _ in 0..2 {
        for typed in ["nihckejigsj", "ni'hc'ke'ji'gs'j"] {
            let rows = engine.query(&request(typed, false), None);
            assert_eq!(words(&rows)[0], "你好科技工具", "{typed}");
            assert_eq!(rows[0].canonical_pinyin, "ni'hao'ke'ji'gong'ju");
            assert_eq!(rows[0].pinyin, "nihckejigsj");
            assert!(rows[0].corrected_from.is_empty());
            assert_eq!(rows[0].source, CandidateSource::Generated);
            assert!(words(&rows).contains(&"你好科技工"));
            assert_eq!(
                rows.iter().filter(|row| row.word == "你好科技工具").count(),
                1
            );
        }
    }
}

#[test]
fn incomplete_tail_keeps_generated_prefix_and_its_input_span() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);\
         CREATE TABLE tbl_2_k(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_2_k VALUES('ke''ji','kj','科技',900);\
         CREATE TABLE tbl_3_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_3_n VALUES('ni''hao''ke','nhk','你好可',800);\
         CREATE TABLE tbl_1_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_q VALUES('qing','q','清',700);",
    );
    let codes = keymap(&[
        ("你", "ab"),
        ("好", "ab"),
        ("科", "ab"),
        ("技", "ab"),
        ("清", "ab"),
    ]);
    for helpcode in [false, true] {
        let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
        for _ in 0..2 {
            for (typed, expected, consumed) in [
                ("nihckeji", "你好科技", "nihckeji"),
                ("nihckejix", "你好科技", "nihckeji"),
                ("nihckejiqkl", "你好科技清", "nihckejiqk"),
                ("ni'hc'ke'ji'x", "你好科技", "nihckeji"),
                ("ni'hc'ke'ji'qk'l", "你好科技清", "nihckejiqk"),
            ] {
                let mut typed_request = request(typed, helpcode);
                typed_request.shuangpin_helpcode_modes.direct_helpcode = false;
                let rows = engine.query(&typed_request, Some(&codes));
                assert_eq!(words(&rows)[0], expected, "{typed}, helpcode={helpcode}");
                assert_eq!(rows[0].source, CandidateSource::Generated);
                assert_eq!(rows[0].pinyin, consumed, "{typed}, helpcode={helpcode}");
                assert_eq!(rows.iter().filter(|row| row.word == expected).count(), 1);
            }
        }
    }
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut disabled = request("nihckejiq", false);
    disabled.sentence_association.word_lattice = false;
    let rows = engine.query(&disabled, None);
    assert_eq!(words(&rows)[0], "你好可");
    assert!(!rows
        .iter()
        .any(|row| row.source == CandidateSource::Generated));

    let rows = engine.query(&request("n'hc'ke'ji'q", false), None);
    assert!(!rows
        .iter()
        .any(|row| row.source == CandidateSource::Generated));

    Connection::open(fixture.paths.dictionary(assets::MAIN_DICTIONARY))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_5_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
             INSERT INTO tbl_5_n VALUES('ni''hao''ke''ji''qing','nhkjq','你好科技清',600);",
        )
        .unwrap();
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let rows = engine.query(&request("nihckejiq", false), None);
    assert_eq!(words(&rows)[0], "你好科技清");
    assert_eq!(rows[0].source, CandidateSource::Database);
    assert_eq!(words(&rows)[1], "你好科技");
}
