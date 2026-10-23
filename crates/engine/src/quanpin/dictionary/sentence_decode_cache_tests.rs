use super::*;
use crate::quanpin::fixture::Fixture;

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for (key, word, weight) in [
        ("fa", "发", 300),
        ("fan", "帆", 200),
        ("nou", "耨", 1),
        ("ou", "偶", 100),
        ("er", "尔", 100),
        ("ou'er", "偶尔", 1000),
    ] {
        fixture.insert(key, word, weight);
    }
    fixture
}

/// 解码缓存里各条目记下的个人模型版本；`FifoCache` 不公开长度，借 `retain_mut` 全部保留着读一遍。
fn decoded_versions(dictionary: &mut QuanpinDictionary) -> Vec<u64> {
    let mut versions = Vec::new();
    dictionary.sentence_decode_cache.retain_mut(|_, cached| {
        versions.push(cached.personal_version);
        true
    });
    versions
}

fn words(rows: &[WordItem]) -> Vec<(String, String, i64)> {
    rows.iter()
        .map(|row| (row.word.to_string(), row.pinyin.to_string(), row.weight))
        .collect()
}

#[test]
fn a_repeated_reading_reuses_the_decoded_sentences_with_the_same_rows() {
    let fixture = fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    for types in [0, TYPO_EDGE_TYPES] {
        dictionary.reset_cache();
        let first = dictionary.query("fanouer", "", types, FuzzyPinyinOptions::default());
        assert!(first.iter().any(|row| row.word == "帆偶尔"));
        let decoded = decoded_versions(&mut dictionary).len();
        assert!(decoded > 0, "整句应写入解码缓存");
        // 去掉整条答案的缓存，下一次只能从解码缓存重新合并。
        dictionary.series_cache.clear();
        let second = dictionary.query("fanouer", "", types, FuzzyPinyinOptions::default());
        assert_eq!(words(&first), words(&second));
        assert_eq!(decoded_versions(&mut dictionary).len(), decoded);
    }
}

#[test]
fn a_personal_model_change_decodes_again() {
    let fixture = fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.query("fanouer", "", 0, FuzzyPinyinOptions::default());
    let before = dictionary.personal.version();
    dictionary
        .record_personal_use(&[PersonalTransition {
            earlier: String::new(),
            previous: "帆".into(),
            word: "偶尔".into(),
            times: 1,
        }])
        .unwrap();
    let after = dictionary.personal.version();
    assert_ne!(before, after);
    dictionary.series_cache.clear();
    dictionary.query("fanouer", "", 0, FuzzyPinyinOptions::default());
    assert!(decoded_versions(&mut dictionary).contains(&after));
}

#[test]
fn switching_sentence_options_drops_the_decoded_sentences() {
    let fixture = fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.query("fanouer", "", 0, FuzzyPinyinOptions::default());
    assert!(!decoded_versions(&mut dictionary).is_empty());
    dictionary.set_sentence_alternatives(true);
    assert_eq!(decoded_versions(&mut dictionary).len(), 0);
    dictionary.query("fanouer", "", 0, FuzzyPinyinOptions::default());
    dictionary.set_sentence_association(SentenceAssociationOptions {
        word_lattice: false,
        ..SentenceAssociationOptions::default()
    });
    assert_eq!(decoded_versions(&mut dictionary).len(), 0);
}

#[test]
fn cached_duplicate_sentences_do_not_copy_discarded_paths() {
    let fixture = fixture();
    fixture.insert("fan", "翻", 190).insert("ou", "鸥", 90);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.set_sentence_alternatives(true);
    let segments = vec!["fan".to_owned(), "ou".to_owned(), "er".to_owned()];
    let mut candidates = Vec::new();
    dictionary.merge_sentences_with_typos(
        &mut candidates,
        "fanouer",
        "fan'ou'er",
        &segments,
        false,
    );
    assert!(candidates.len() > 1, "夹具必须覆盖多条缓存路径");
    let before = words(&candidates);
    let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        dictionary.merge_sentences_with_typos(
            &mut candidates,
            "fanouer",
            "fan'ou'er",
            &segments,
            false,
        );
    });
    assert_eq!(words(&candidates), before);
    assert_eq!(allocations, 2, "全部路径重复时只应分配解码缓存键");
}
