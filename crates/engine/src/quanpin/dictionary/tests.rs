//! Dictionary-level ports of the reference tests: the autocorrect switch matrix and marking probes (`test_pinyin.cpp:910-993`, `:1169-1225`), the segmentation contract, longer phrases (`test_longer_phrase_candidates.cpp`), fuzzy rules and the protected alternative slot (`test_fuzzy_pinyin.cpp:97-130`), typo sentences (`test_typo_correction_input_session.cpp:260-330`), online rows, initial expansion and the word writers.

use rusqlite::params;
use std::borrow::Cow;
use std::collections::HashSet;

use super::*;
use crate::quanpin::fixture::Fixture;
use crate::types::fuzzy_rule;

const NONE: u32 = 0;
const TRANSPOSITION: u32 = autocorrect_type::TRANSPOSITION;
const NEIGHBOR: u32 = autocorrect_type::NEIGHBOR;
const BOTH: u32 = TRANSPOSITION | NEIGHBOR;
const WITH_DELETION: u32 = BOTH | autocorrect_type::DELETION;
const ALL_FOUR: u32 = WITH_DELETION | autocorrect_type::INSERTION;
const NO_FUZZY: FuzzyPinyinOptions = FuzzyPinyinOptions { rules: 0 };

fn words(items: &[WordItem]) -> Vec<&str> {
    items.iter().map(|item| item.word.as_str()).collect()
}

#[test]
fn segmentation_cache_lookup_borrows_query_key() {
    let mut cache = FifoCache::new(2);
    cache.insert("ni".to_owned(), vec!["ni".to_owned()]);
    assert_eq!(
        lookup_cached_segments(&cache, "ni"),
        Some(vec!["ni".to_owned()])
    );
}

#[test]
fn resolution_cache_key_hash_matches_borrowed_parts() {
    let key = "3\u{1f}nihao\u{1f}ni'hao";
    assert_eq!(
        resolution_cache_hash(3, "nihao", "ni'hao"),
        resolution_cache_hash(3, "nihao", "ni'hao")
    );
    assert!(resolution_cache_key_matches(key, 3, "nihao", "ni'hao"));
    assert!(!resolution_cache_key_matches(key, 3, "niha", "ni'hao"));
}

#[test]
fn primary_segmentation_is_checked_without_owning_a_key_copy() {
    let seen = HashSet::new();
    assert!(is_duplicate_segmentation("ni'hao", &seen, "ni'hao"));
    assert!(!is_duplicate_segmentation("ni'hao", &seen, "ni'he"));
}

#[test]
fn series_slot_key_encodes_switches_and_context() {
    let options = SentenceAssociationOptions {
        word_lattice: true,
        neural_keyboard: false,
        show_next_on_duplicate: true,
    };
    assert_eq!(
        series_slot_key("T0:A:ni", options, false, ""),
        "T0:A:ni\u{1f}S101"
    );
    assert_eq!(
        series_slot_key("T0:A:ni", options, true, "你好"),
        "T0:A:ni\u{1f}S101\u{1f}你好"
    );
}

#[test]
fn fuzzy_segmentation_borrows_explicit_input() {
    let borrowed = fuzzy_segmentation("ni'hao", &[]);
    assert!(matches!(borrowed, Cow::Borrowed("ni'hao")));

    let segments = vec!["ni".to_owned(), "hao".to_owned()];
    let owned = fuzzy_segmentation("", &segments);
    assert!(matches!(owned, Cow::Owned(_)));
    assert_eq!(owned, "ni'hao");
}

#[test]
fn path_cache_key_borrows_the_segmentation_when_present() {
    assert_eq!(path_cache_key("nihao", "ni'hao"), "ni'hao");
    assert_eq!(path_cache_key("nihao", ""), "nihao");
}

#[test]
fn truncating_a_joined_prefix_drops_only_the_last_segment() {
    let mut segmentation = "ni'hao'ma".to_owned();
    truncate_last_segment(&mut segmentation);
    assert_eq!(segmentation, "ni'hao");
    truncate_last_segment(&mut segmentation);
    assert_eq!(segmentation, "ni");
}

#[test]
fn fuzzy_cache_key_hash_checks_rules_and_segmentation() {
    let cached = CachedFuzzyCandidates {
        rules: 0x7ff,
        segmentation: "ni'hao".to_owned(),
        candidates: Vec::new(),
    };
    assert!(fuzzy_cache_key_matches(&cached, 0x7ff, "ni'hao"));
    assert!(!fuzzy_cache_key_matches(&cached, 0x3ff, "ni'hao"));
    assert!(!fuzzy_cache_key_matches(&cached, 0x7ff, "niha"));
}

fn contains(items: &[WordItem], word: &str) -> bool {
    items.iter().any(|item| item.word == word)
}

fn position(items: &[WordItem], word: &str) -> usize {
    items
        .iter()
        .position(|item| item.word == word)
        .unwrap_or_else(|| panic!("{word} missing from {:?}", words(items)))
}

fn count_marked(items: &[WordItem]) -> usize {
    items
        .iter()
        .filter(|item| !item.corrected_from.is_empty())
        .count()
}

fn query(
    dictionary: &mut QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    types: u32,
) -> Vec<WordItem> {
    dictionary.query(raw, segmentation, types, NO_FUZZY)
}

/// `create_autocorrect_probe_database`: 上 exists only under the corrected key `shang`, so a leading 上 proves the corrected path ran.
fn probe() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .insert("shang", "上", 100)
        .insert("shang'zhi", "上至", 100)
        .insert("jian'du", "监督", 100)
        .insert("sa'huang'na'ge", "撒谎那个", 1000);
    fixture
}

#[test]
fn autocorrect_switch_matrix() {
    let fixture = probe();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let is_shang = |item: &WordItem| item.word == "上";

    let corrected = query(&mut dictionary, "sahng", "sa'h'n'g", BOTH);
    assert_eq!(
        corrected[0].word, "上",
        "both switches on put the corrected row first"
    );
    assert_eq!(corrected[0].canonical_pinyin, "shang");

    let off = query(&mut dictionary, "sahng", "sa'h'n'g", NONE);
    assert!(
        !off.iter().any(is_shang),
        "both switches off keep the corrected row out"
    );
    assert!(
        contains(&off, "撒谎那个"),
        "the literal rows survive with correction off"
    );

    let transposed = query(&mut dictionary, "sahng", "sa'h'n'g", TRANSPOSITION);
    assert_eq!(transposed[0].word, "上");
    assert_eq!(transposed[0].corrected_from, "sahng");
    // The reference test asserted 上 absent here, but that group never ran and the reference returns it: any non-zero mask adds the correction-mode alias cuts (sahng -> shang, sang) as alternative readings, and quanpin.md §3.5 keeps that quirk for parity. The type gate is on the k-best search, so no row is marked as corrected from sahng and none leads.
    let neighbor_denied = query(&mut dictionary, "sahng", "sa'h'n'g", NEIGHBOR);
    assert_ne!(neighbor_denied[0].word, "上");
    assert!(neighbor_denied
        .iter()
        .all(|item| item.corrected_from.is_empty()));

    let neighbor = query(&mut dictionary, "shabg", "sha'b'g", NEIGHBOR);
    assert_eq!(neighbor[0].word, "上");
    // The prefix query can still reach shang through sha, so the gate is checked on the marks.
    let transposition_denied = query(&mut dictionary, "shabg", "sha'b'g", TRANSPOSITION);
    assert!(!transposition_denied
        .iter()
        .any(|item| item.corrected_from == "shabg"));

    let multi = query(&mut dictionary, "sahngzhi", "sa'h'n'g'zhi", BOTH);
    assert_eq!(
        multi[0].word, "上至",
        "a cross-syllable correction survives the mask wiring"
    );

    // A dropped initial cuts to nothing in correction mode, and must still reach the correction path.
    let dropped_initial = query(&mut dictionary, "iandu", "", WITH_DELETION);
    assert_eq!(dropped_initial[0].word, "监督");
    assert_eq!(dropped_initial[0].corrected_from, "iandu");
    let dropped_initial_off = query(&mut dictionary, "iandu", "", NONE);
    assert!(!contains(&dropped_initial_off, "监督"));

    // The jianpin guard fires before the search.
    query(&mut dictionary, "zheg", "", BOTH);
    assert_ne!(dictionary.pinyin_segmentation, "zu'ge");

    // A correctable head composes with a jianpin tail.
    query(&mut dictionary, "hauzh", "", BOTH);
    assert_eq!(dictionary.pinyin_segmentation, "hua'zh");
    query(&mut dictionary, "hauz", "", BOTH);
    assert_eq!(dictionary.pinyin_segmentation, "hua'z");
    query(&mut dictionary, "huazh", "", BOTH);
    assert_eq!(dictionary.pinyin_segmentation, "hua'zh");
    query(&mut dictionary, "hauzh", "", NONE);
    assert_ne!(dictionary.pinyin_segmentation, "hua'zh");
    // A neighbor head never composes with a tail.
    query(&mut dictionary, "shng", "sh'n'g", BOTH);
    assert_ne!(dictionary.pinyin_segmentation, "sun'g");
}

#[test]
fn autocorrect_marking() {
    let fixture = Fixture::new();
    fixture
        .insert("shang", "上", 100)
        .insert("nv", "女", 100)
        .insert("shang'hao", "上好", 100)
        .insert("jian'du", "监督", 100)
        .insert("ke'neng", "可能", 100)
        .insert("sa'huang'na'ge", "撒谎那个", 1000);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);

    let corrected = query(&mut dictionary, "sahng", "sa'h'n'g", BOTH);
    assert_eq!(corrected[0].word, "上");
    assert_eq!(corrected[0].corrected_from, "sahng");
    assert!(corrected[position(&corrected, "撒谎那个")]
        .corrected_from
        .is_empty());
    assert_eq!(
        query(&mut dictionary, "sahng", "sa'h'n'g", BOTH),
        corrected,
        "a cached answer is marked as the first one was"
    );

    assert_eq!(
        count_marked(&query(&mut dictionary, "sahng", "sa'h'n'g", NONE)),
        0
    );

    // An alias-layer segmentation is marked whatever the switches say.
    let alias = query(&mut dictionary, "sahng", "shang", NONE);
    assert_eq!(alias[0].word, "上");
    assert_eq!(alias[0].corrected_from, "sahng");

    // Only the full-length corrected row is marked, never a prefix row.
    let full = query(&mut dictionary, "sahnghao", "sa'h'n'g'hao", BOTH);
    assert_eq!(full[0].word, "上好");
    assert_eq!(full[0].corrected_from, "sahnghao");
    assert!(full[position(&full, "上")].corrected_from.is_empty());
    assert_eq!(count_marked(&full), 1);

    assert_eq!(
        count_marked(&query(&mut dictionary, "keneng", "ke'neng", BOTH)),
        0
    );
    assert_eq!(count_marked(&query(&mut dictionary, "wj", "w'j", BOTH)), 0);
    let nv = query(&mut dictionary, "nv", "nv", BOTH);
    assert_eq!(nv[0].word, "女");
    assert!(
        nv[0].corrected_from.is_empty(),
        "the v spelling stays unmarked"
    );

    let deletion = query(&mut dictionary, "shng", "sh'n'g", WITH_DELETION);
    assert_eq!(deletion[0].word, "上");
    assert_eq!(deletion[0].corrected_from, "shng");
    assert_eq!(
        count_marked(&query(&mut dictionary, "shng", "sh'n'g", BOTH)),
        0
    );

    let insertion = query(&mut dictionary, "sshang", "", ALL_FOUR);
    assert_eq!(insertion[0].word, "上");
    assert_eq!(insertion[0].corrected_from, "sshang");
    assert_eq!(
        count_marked(&query(&mut dictionary, "sshang", "", WITH_DELETION)),
        0
    );
}

#[test]
fn umlaut_alias_is_queried_under_the_standard_key_and_marked() {
    let fixture = Fixture::new();
    fixture.insert("nve", "虐", 100).insert("lve", "略", 90);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let typed_nue = query(&mut dictionary, "nue", "nue", NONE);
    assert_eq!(typed_nue[0].word, "虐");
    assert_eq!(typed_nue[0].corrected_from, "nue");
    let typed_nve = query(&mut dictionary, "nve", "nve", NONE);
    assert_eq!(typed_nve[0].word, "虐");
    assert!(typed_nve[0].corrected_from.is_empty());
}

#[test]
fn short_inputs_compete_with_their_other_segmentations() {
    // test_segmentation_contract.cpp: four syllables re-segment, five and explicit apostrophes do not.
    let fixture = Fixture::new();
    fixture
        .insert("ao'shi'ke", "奥湿克", 1)
        .insert("xian", "__primary_xian_1__", 1000)
        .insert("xian", "__primary_xian_2__", 900)
        .insert("xian", "__primary_xian_3__", 800)
        .insert("xi'an", "__alternative_xi_an__", 1)
        .insert("xi'an'xian'xian", "__three_syllable_alternative__", 100)
        .insert("xi'an'xian'xian'xian", "__four_syllable_alternative__", 100)
        .insert(
            "xi'an'xian'xian'xian'xian",
            "__five_syllable_alternative__",
            100,
        );
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let four = query(
        &mut dictionary,
        "xianxianxianxian",
        "xian'xian'xian'xian",
        NONE,
    );
    let five = query(
        &mut dictionary,
        "xianxianxianxianxian",
        "xian'xian'xian'xian'xian",
        NONE,
    );
    let manual = query(
        &mut dictionary,
        "xian'xian'xian'xian",
        "xian'xian'xian'xian",
        NONE,
    );
    assert!(contains(&four, "__four_syllable_alternative__"));
    assert!(!contains(&five, "__five_syllable_alternative__"));
    assert!(!contains(&manual, "__four_syllable_alternative__"));
}

fn phrase_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .insert("ni", "你", 8000)
        .insert("ni", "泥", 7000)
        .insert("ni'hao", "你好", 10_000)
        .insert("ni'hao", "拟好", 30)
        .insert("ni'hao'ma", "你好吗", 900)
        .insert("an'quan'bao'wei", "安全保卫", 6000)
        .insert("ni'hao'a'ya", "你好啊呀", 500);
    fixture
}

#[test]
fn longer_phrases_join_the_full_key_group_by_weight() {
    let fixture = phrase_fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let listed = query(&mut dictionary, "nihao", "ni'hao", NONE);
    assert_eq!(listed[0].word, "你好");
    assert!(position(&listed, "你好吗") < position(&listed, "拟好"));
    assert!(position(&listed, "你好啊呀") < position(&listed, "拟好"));
    assert!(
        position(&listed, "拟好") < position(&listed, "泥"),
        "a first-syllable character outranked a full spelling"
    );
    let longer = &listed[position(&listed, "你好吗")];
    assert_eq!(
        longer.pinyin, "ni'hao",
        "composition advance consumes only what was typed"
    );
    assert_eq!(longer.canonical_pinyin, "ni'hao'ma");

    let exact = query(&mut dictionary, "anquanbaowei", "an'quan'bao'wei", NONE);
    assert_eq!(
        exact[0].word, "安全保卫",
        "a whole sentence displaced the exact entry"
    );
}

#[test]
fn sentence_rows_follow_the_exact_hits_and_precede_longer_phrases() {
    // quanpin.md §11.1: ping'guo lists 苹果, the sentence block, then 苹果电脑 苹果公司 评过 平果 平锅.
    let fixture = Fixture::new();
    fixture
        .insert("ping'guo", "苹果", 1_143_881)
        .insert("ping'guo", "评过", 1180)
        .insert("ping'guo", "平果", 169)
        .insert("ping'guo", "平锅", 1)
        .insert("ping'guo'dian'nao", "苹果电脑", 21_495)
        .insert("ping'guo'gong'si", "苹果公司", 19_725)
        .insert("ping", "平", 1000)
        .insert("guo", "国", 1000);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.set_sentence_alternatives(true);
    let listed = query(&mut dictionary, "pingguo", "ping'guo", NONE);
    assert_eq!(listed[0].word, "苹果");
    let longer = position(&listed, "苹果电脑");
    assert!(
        longer > 1,
        "the lattice block sits between the exact hit and the continuations"
    );
    assert!(listed[1..longer]
        .iter()
        .all(|item| item.source == CandidateSource::Generated && item.sentence_association));
    assert_eq!(
        words(&listed[longer..longer + 5]),
        ["苹果电脑", "苹果公司", "评过", "平果", "平锅"]
    );
}

#[test]
fn sparse_first_syllables_borrow_their_split_readings() {
    let fixture = Fixture::new();
    fixture
        .insert("dia", "嗲", 50)
        .insert("di'a", "低啊", 10)
        .insert("di", "地", 100);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let listed = query(&mut dictionary, "dia", "dia", NONE);
    assert_eq!(words(&listed), ["嗲", "低啊", "地"]);
}

#[test]
fn fuzzy_rules_expand_both_ways_without_polluting_the_exact_cache() {
    let pairs = [
        ("zan", "zhan"),
        ("can", "chan"),
        ("san", "shan"),
        ("na", "la"),
        ("fa", "ha"),
        ("ran", "lan"),
        ("ban", "bang"),
        ("ben", "beng"),
        ("bin", "bing"),
        ("lian", "liang"),
        ("guan", "guang"),
    ];
    let fixture = Fixture::new();
    for (i, (plain, partner)) in pairs.iter().enumerate() {
        fixture
            .insert(plain, &format!("原{i}"), 100)
            .insert(partner, &format!("糊{i}"), 100);
    }
    fixture
        .insert("xian", "先", 1_662_684)
        .insert("xi'an", "西安", 55_003)
        .insert("xie", "些", 3_752_167)
        .insert("xie", "写", 605_147)
        .insert("xi'e", "西鄂", 6)
        .insert("jiang", "将", 2_629_219)
        .insert("jiang", "僵", 94_955)
        .insert("ji'ang", "激昂", 23_740)
        .insert("you'dian", "邮电", 999)
        .insert("you'di'an", "尤迪安", 7);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    for (i, (plain, partner)) in pairs.iter().enumerate() {
        let rule = FuzzyPinyinOptions { rules: 1 << i };
        let forward = dictionary.query(plain, plain, NONE, rule);
        assert!(contains(&forward, &format!("糊{i}")), "forward rule {i}");
        let fuzzy_row = &forward[position(&forward, &format!("糊{i}"))];
        assert!(fuzzy_row.fuzzy);
        assert_eq!(fuzzy_row.canonical_pinyin, *partner);
        let reverse = dictionary.query(partner, partner, NONE, rule);
        assert!(contains(&reverse, &format!("原{i}")), "reverse rule {i}");
        let exact = query(&mut dictionary, plain, plain, NONE);
        assert!(
            !contains(&exact, &format!("糊{i}")),
            "fuzzy polluted exact cache {i}"
        );
        let other_rule = FuzzyPinyinOptions {
            rules: 1 << ((i + 1) % pairs.len()),
        };
        let other = dictionary.query(plain, plain, NONE, other_rule);
        assert!(
            !contains(&other, &format!("糊{i}")),
            "unselected rule expanded {i}"
        );
    }

    let fuzzy_on = FuzzyPinyinOptions {
        rules: fuzzy_rule::Z_ZH,
    };
    let xian = dictionary.query("xian", "xian", NONE, fuzzy_on);
    assert_eq!(
        words(&xian[..2]),
        ["先", "西安"],
        "real ambiguity lost its protected slot"
    );
    let xie = dictionary.query("xie", "xie", NONE, fuzzy_on);
    assert_eq!(words(&xie[..2]), ["些", "写"]);
    assert!(
        position(&xie, "西鄂") > 1,
        "a rare re-segmentation outranked the exact reading"
    );
    let jiang = dictionary.query("jiang", "jiang", NONE, fuzzy_on);
    assert_eq!(words(&jiang[..2]), ["将", "僵"]);
    assert!(position(&jiang, "激昂") > 1);
    let youdian = dictionary.query("youdian", "you'dian", NONE, fuzzy_on);
    assert_eq!(youdian[0].word, "邮电");
    assert!(
        position(&youdian, "尤迪安") > 0,
        "a longer alternative key outranked the exact reading"
    );
}

fn typo_fixture(strong_literal_phrase: bool) -> Fixture {
    let fixture = Fixture::new();
    fixture
        .insert("mei", "没", 1000)
        .insert("mei", "美", 900)
        .insert("gan", "干", 1000)
        .insert("guan", "关", 500)
        .insert("xi", "系", 1000)
        .insert("guan'xi", "关系", 100_000)
        .insert("shang", "上", 1000)
        .insert("sang", "桑", 900)
        .insert("shang'zhi", "上知", 100)
        .insert("zhi", "知", 100);
    if strong_literal_phrase {
        fixture.insert("gan'xi", "干系", 200_000);
    }
    fixture.table("mei'guan'xi");
    fixture
}

fn typo_row(items: &[WordItem]) -> Option<usize> {
    items
        .iter()
        .position(|item| item.sentence_association && !item.corrected_from.is_empty())
}

#[test]
fn typo_sentence_follows_the_literal_sentence() {
    let fixture = typo_fixture(false);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.set_sentence_alternatives(true);

    let off = query(&mut dictionary, "meiganxi", "mei'gan'xi", NONE);
    assert_eq!(
        typo_row(&off),
        None,
        "a typo sentence appeared with correction off"
    );

    // Either legacy switch carries the missing-letter edges (`request_autocorrect_mask`).
    let legacy = crate::types::request_autocorrect_mask(true, true);
    let table_only = query(&mut dictionary, "meiganxi", "mei'gan'xi", legacy);
    let row = typo_row(&table_only).expect("the typo sentence appears");
    let item = &table_only[row];
    assert_eq!(item.word, "没关系");
    assert_eq!(item.corrected_from, "meiganxi");
    assert_eq!(item.canonical_pinyin, "mei'guan'xi");
    assert_eq!(item.source, CandidateSource::Generated);
    assert!(
        position(&table_only, "没干系") < row,
        "the literal sentence leads"
    );

    let correct = query(&mut dictionary, "meiguanxi", "mei'guan'xi", legacy);
    assert_eq!(typo_row(&correct), None);
    assert_eq!(
        count_marked(&correct),
        0,
        "a correctly typed input was corrected"
    );
    assert_eq!(correct[0].word, "没关系");
}

#[test]
fn typo_sentence_never_leads_a_strong_literal_phrase() {
    let fixture = typo_fixture(true);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.set_sentence_alternatives(true);
    let listed = query(&mut dictionary, "meiganxi", "mei'gan'xi", ALL_FOUR);
    assert_eq!(listed[0].word, "没干系");
    assert_ne!(typo_row(&listed), Some(0));
}

#[test]
fn online_rows_land_in_the_series_slot_the_query_used() {
    let fixture = phrase_fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let before = query(&mut dictionary, "nihao", "ni'hao", NONE);
    assert!(!contains(&before, "倪好"));

    let one = |word: &str| vec![word.to_string()];
    assert!(dictionary.insert_online_words(
        "nihao",
        "ni'hao",
        NONE,
        &one("倪好"),
        CandidateSource::CloudSuggestion
    ));
    let cloud = query(&mut dictionary, "nihao", "ni'hao", NONE);
    assert_eq!(cloud[1].word, "倪好");
    assert_eq!(cloud[1].source, CandidateSource::CloudSuggestion);
    assert_eq!(cloud[1].weight, 1);
    assert_eq!(cloud[1].pinyin, "nihao");

    let ai = vec![
        "智能甲".to_string(),
        "智能乙".to_string(),
        "智能甲".to_string(),
    ];
    assert!(dictionary.insert_online_words(
        "nihao",
        "ni'hao",
        NONE,
        &ai,
        CandidateSource::AiSuggestion
    ));
    // A second cloud answer replaces the first and keeps the AI block at index 2.
    assert!(dictionary.insert_online_words(
        "nihao",
        "ni'hao",
        NONE,
        &one("妮好"),
        CandidateSource::CloudSuggestion
    ));
    let listed = query(&mut dictionary, "nihao", "ni'hao", NONE);
    assert_eq!(words(&listed[..4]), ["你好", "妮好", "智能甲", "智能乙"]);
    assert!(!contains(&listed, "倪好"));

    let two = vec!["一".to_string(), "二".to_string()];
    assert!(!dictionary.insert_online_words(
        "nihao",
        "ni'hao",
        NONE,
        &two,
        CandidateSource::CloudSuggestion
    ));
    assert!(!dictionary.insert_online_words(
        "nihao",
        "ni'hao",
        NONE,
        &one("坏\u{7}"),
        CandidateSource::AiSuggestion
    ));
    assert!(!dictionary.insert_online_words(
        "",
        "",
        NONE,
        &one("空"),
        CandidateSource::AiSuggestion
    ));

    // Another mask is another slot.
    let masked = query(&mut dictionary, "nihao", "ni'hao", BOTH);
    assert!(!contains(&masked, "妮好"));
}

#[test]
fn single_letters_are_capped_until_expanded() {
    let fixture = Fixture::new();
    for i in 0..30 {
        fixture.insert("ni", &format!("n{i:02}"), 1_000 - i);
    }
    fixture.insert("ma", "吗", 5);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let capped = query(&mut dictionary, "n", "n", NONE);
    assert_eq!(capped.len(), INITIAL_CANDIDATE_LIMIT);
    assert!(capped
        .iter()
        .all(|item| item.pinyin == "n" && item.canonical_pinyin == "ni"));

    let mut shown = capped.clone();
    assert!(!dictionary.expand_initial_candidates("n", "n", NONE, "ni", &mut shown));
    assert!(dictionary.expand_initial_candidates("n", "n", NONE, "n", &mut shown));
    assert_eq!(shown.len(), 30);
    assert_eq!(shown[29].word, "n29");
    assert_eq!(
        query(&mut dictionary, "n", "n", NONE),
        shown,
        "the series slot holds the expansion"
    );
    assert!(
        !dictionary.expand_initial_candidates("n", "n", NONE, "n", &mut shown),
        "an expanded list is not capped any more"
    );

    let mut short = query(&mut dictionary, "m", "m", NONE);
    assert_eq!(short.len(), 1);
    assert!(!dictionary.expand_initial_candidates("m", "m", NONE, "m", &mut short));
}

#[test]
fn external_writes_invalidate_cached_lists() {
    let fixture = phrase_fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    assert!(!contains(&query(&mut dictionary, "ni", "ni", NONE), "妮"));
    fixture
        .connection()
        .execute(
            "INSERT INTO tbl_1_n(key, jp, value, weight) VALUES ('ni', 'n', ?1, 9000)",
            params!["妮"],
        )
        .unwrap();
    let listed = query(&mut dictionary, "ni", "ni", NONE);
    assert_eq!(listed[0].word, "妮");
}

fn journal_row(fixture: &Fixture, key: &str, value: &str) -> Option<(String, i64, i64)> {
    let connection = rusqlite::Connection::open(fixture.journal()).ok()?;
    connection
        .query_row(
            "SELECT operation, weight, user_inserted FROM user_dictionary_operations WHERE dictionary = 'pinyin' AND key = ?1 AND value = ?2",
            params![key, value],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()
}

#[test]
fn inserted_phrase_is_journaled_and_listed() {
    let fixture = phrase_fixture();
    fixture.table("ce'shi");
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    assert!(!contains(
        &query(&mut dictionary, "ceshi", "ce'shi", NONE),
        "测试"
    ));

    dictionary
        .create_word_from_canonical_pinyin("ce'shi", "测试")
        .unwrap();
    assert_eq!(fixture.weight("ce'shi", "测试"), Some(INSERTED_WEIGHT));
    assert_eq!(
        journal_row(&fixture, "ce'shi", "测试"),
        Some(("upsert".to_string(), INSERTED_WEIGHT, 1))
    );
    assert!(
        contains(&query(&mut dictionary, "ceshi", "ce'shi", NONE), "测试"),
        "the insert resets the cached lists"
    );
    assert!(
        dictionary
            .create_word_from_canonical_pinyin("ce'shi", "测")
            .is_err(),
        "one syllable per character"
    );
    assert!(dictionary
        .create_word_from_canonical_pinyin("", "测试")
        .is_err());
    let found = dictionary.find_candidate("ce'shi", "测试").unwrap();
    assert_eq!(found.weight, INSERTED_WEIGHT);
    assert_eq!(found.canonical_pinyin, "ce'shi");
    assert!(dictionary.find_candidate("ce'shi", "侧室").is_none());
}

#[test]
fn canonical_writer_keeps_the_given_boundaries() {
    let fixture = Fixture::new();
    fixture.table("qi'e'huan");
    fixture.table("lve'duo");
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    // A re-cut would read qie'huan and reject the three-character phrase.
    dictionary
        .create_word_from_canonical_pinyin("qi'e'huan", "企鹅换")
        .unwrap();
    assert_eq!(fixture.weight("qi'e'huan", "企鹅换"), Some(INSERTED_WEIGHT));
    // Online rows arrive with the typed spelling; they are stored under the standard key.
    dictionary
        .create_word_from_canonical_pinyin("lue'duo", "掠夺")
        .unwrap();
    assert_eq!(fixture.weight("lve'duo", "掠夺"), Some(INSERTED_WEIGHT));
    assert!(dictionary
        .create_word_from_canonical_pinyin("qie'huan", "企鹅换")
        .is_err());
    assert!(dictionary
        .create_word_from_canonical_pinyin("qi'x'huan", "企鹅换")
        .is_err());
    // An existing row is success without a journal write.
    fixture.insert("guan'xi", "关系", 5);
    dictionary
        .create_word_from_canonical_pinyin("guan'xi", "关系")
        .unwrap();
    assert_eq!(fixture.weight("guan'xi", "关系"), Some(5));
    assert_eq!(journal_row(&fixture, "guan'xi", "关系"), None);
}

#[test]
fn missing_dictionary_answers_nothing() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.database()).unwrap();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    assert!(query(&mut dictionary, "nihao", "ni'hao", BOTH).is_empty());
    assert!(dictionary
        .fuzzy_candidates("ni'hao", FuzzyPinyinOptions { rules: 0x7ff })
        .is_empty());
    assert!(!dictionary.knows_han_char("你"));
    assert!(dictionary
        .create_word_from_canonical_pinyin("ni'hao", "你好")
        .is_err());
    assert!(
        !fixture.database().exists(),
        "a missing dictionary stays missing"
    );
}

#[test]
fn sentence_alternatives_toggle_rebuilds_the_lists() {
    let fixture = Fixture::new();
    fixture
        .insert("ping", "平", 1000)
        .insert("ping", "瓶", 900)
        .insert("guo", "国", 1000)
        .insert("guo", "果", 900);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let generated = |items: &[WordItem]| {
        items
            .iter()
            .filter(|item| item.source == CandidateSource::Generated)
            .count()
    };
    let one = generated(&query(&mut dictionary, "pingguo", "ping'guo", NONE));
    dictionary.set_sentence_alternatives(true);
    let all = generated(&query(&mut dictionary, "pingguo", "ping'guo", NONE));
    assert_eq!(
        one, 1,
        "without alternatives only the best reading is emitted"
    );
    assert!(all > one);
}

/// neural-association.patch:2667-2682: an association change resets the caches, so an online row cached before a lattice off/on round trip is gone after it, as in shuangpin.
#[test]
fn association_switch_resets_the_series_cache() {
    let fixture = Fixture::new();
    fixture
        .insert("ping", "平", 1000)
        .insert("ping", "瓶", 900)
        .insert("guo", "国", 1000)
        .insert("guo", "果", 900);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let lattice_on = SentenceAssociationOptions::default();
    let lattice_off = SentenceAssociationOptions {
        word_lattice: false,
        ..lattice_on
    };
    let has_sentence = |items: &[WordItem]| items.iter().any(|item| item.sentence_association);
    let cloud = |dictionary: &mut QuanpinDictionary| {
        assert!(dictionary.insert_online_words(
            "pingguo",
            "ping'guo",
            NONE,
            &["苹果".to_string()],
            CandidateSource::CloudSuggestion
        ));
    };

    assert!(has_sentence(&query(
        &mut dictionary,
        "pingguo",
        "ping'guo",
        NONE
    )));
    cloud(&mut dictionary);
    assert!(contains(
        &query(&mut dictionary, "pingguo", "ping'guo", NONE),
        "苹果"
    ));

    dictionary.set_sentence_association(lattice_off);
    let off = query(&mut dictionary, "pingguo", "ping'guo", NONE);
    assert!(
        !has_sentence(&off),
        "the lattice switch reaches a fresh list"
    );
    assert!(!contains(&off, "苹果"));

    dictionary.set_sentence_association(lattice_on);
    let back = query(&mut dictionary, "pingguo", "ping'guo", NONE);
    assert!(has_sentence(&back));
    assert!(
        !contains(&back, "苹果"),
        "the round trip dropped the cached online row"
    );

    // Without a model the context is not part of the slot, so a commit keeps every list.
    cloud(&mut dictionary);
    dictionary.set_rescoring_context("我想吃");
    assert!(contains(
        &query(&mut dictionary, "pingguo", "ping'guo", NONE),
        "苹果"
    ));
}

/// The engine emits the keyboard model's row only. A desktop host installs the desktop model in the resource bundle's `settled-model` sibling and the input runtime runs it as its settled reranker; even a copy inside the bundle itself never reaches a query here, since no engine switch names it.
#[test]
fn only_the_keyboard_model_answers_a_query() {
    const DESKTOP_MODEL: &str = "sentence-model-desktop.safetensors";
    let keyboard = match crate::lattice::neural::test_model_path(NEURAL_MODEL_KEYBOARD) {
        Ok(path) => path,
        Err(reason) => {
            eprintln!("skipping only_the_keyboard_model_answers_a_query: {reason}");
            return;
        }
    };
    let fixture = Fixture::new();
    fixture
        .insert("shu'ru", "输入", 20000)
        .insert("fa", "法", 800000)
        .insert("fa", "发", 900000)
        .insert("fa", "罚", 100000);
    std::fs::copy(&keyboard, fixture.paths.resource(NEURAL_MODEL_KEYBOARD)).unwrap();
    // Any readable model file stands in for the desktop one: nothing may load it.
    std::fs::copy(&keyboard, fixture.paths.resource(DESKTOP_MODEL)).unwrap();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.set_sentence_association(SentenceAssociationOptions {
        word_lattice: true,
        neural_keyboard: true,
        show_next_on_duplicate: true,
    });
    dictionary.set_rescoring_context("我在用一个新的");
    assert_eq!(dictionary.rerankers.len(), 1);
    let rows = query(&mut dictionary, "shurufa", "", NONE);
    let count = |source| rows.iter().filter(|item| item.source == source).count();
    assert_eq!(
        count(CandidateSource::NeuralKeyboard),
        1,
        "{:?}",
        words(&rows)
    );
}

fn fuzzy_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .insert("zhong'guo", "中国", 1_000_000)
        .insert("zong'guo", "宗国", 10);
    fixture
}

/// A warm fuzzy query reads its cached hash slot instead of expanding the fuzzy paths again: a marker row planted there comes back.
#[test]
fn warm_fuzzy_queries_reuse_the_fuzzy_slot() {
    let fixture = fuzzy_fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let all = FuzzyPinyinOptions {
        rules: fuzzy_rule::ALL,
    };
    let cold = dictionary.query("zongguo", "zong'guo", NONE, all);
    assert!(contains(&cold, "中国"), "{:?}", words(&cold));
    let slot = fuzzy_cache_hash(fuzzy_rule::ALL, "zong'guo");
    let mut cached = dictionary.fuzzy_cache.get(&slot).expect("fuzzy slot");
    let mut marker = cached.candidates[0].clone();
    marker.word = "哨兵".to_string();
    cached.candidates.push(marker);
    dictionary.fuzzy_cache.insert(slot, cached);
    let warm = dictionary.query("zongguo", "zong'guo", NONE, all);
    assert!(contains(&warm, "哨兵"), "{:?}", words(&warm));
}

/// test_fuzzy_pinyin.cpp:265-270: two hundred warm fuzzy queries stay well inside the reference's five-second budget.
#[test]
fn two_hundred_warm_fuzzy_queries_stay_under_budget() {
    let fixture = fuzzy_fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let all = FuzzyPinyinOptions {
        rules: fuzzy_rule::ALL,
    };
    assert!(contains(
        &dictionary.query("zongguo", "zong'guo", NONE, all),
        "中国"
    ));
    let started = std::time::Instant::now();
    for _ in 0..200 {
        assert!(contains(
            &dictionary.query("zongguo", "zong'guo", NONE, all),
            "中国"
        ));
    }
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// QD:89-99, the quanpin side of `a_changed_personal_model_drops_the_scored_series`: once the tables change under the personal model, reading it reloads, the version moves, and the series scored with the old model is dropped.
#[test]
fn a_changed_personal_model_drops_the_scored_series() {
    use crate::user_dictionary::ngram_store::PersonalNgramStore;
    let fixture = Fixture::new();
    fixture
        .insert("ni", "你", 10000)
        .insert("ni", "拟", 9000)
        .insert("hao", "好", 10000)
        .insert("hao", "号", 9000);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let before = query(&mut dictionary, "nihao", "", NONE);
    let store = PersonalNgramStore::for_journal(&fixture.journal());
    let version = store.version();
    let journal = rusqlite::Connection::open(fixture.journal()).unwrap();
    crate::user_dictionary::journal::ensure_schema(&journal).unwrap();
    journal
        .execute_batch("INSERT INTO personal_bigram VALUES(char(1),'拟',400),('拟','号',400);INSERT INTO personal_trigram VALUES(char(1),'拟','号',400);")
        .unwrap();
    store.invalidate_for_tests();
    let after = query(&mut dictionary, "nihao", "", NONE);
    assert!(
        store.version() > version,
        "the query did not reload the model"
    );
    assert!(
        contains(&after, "拟号") && !contains(&before, "拟号"),
        "the answer scored with the old model was kept: {:?} then {:?}",
        words(&before),
        words(&after)
    );
}
