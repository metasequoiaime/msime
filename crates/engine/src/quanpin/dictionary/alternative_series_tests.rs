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

#[test]
fn literal_alternative_series_supplies_prefixes_and_sentences_on_cold_and_hot_queries() {
    let fixture = fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    for types in [0, TYPO_EDGE_TYPES] {
        for _ in 0..2 {
            let rows = dictionary.query("fanouer", "", types, FuzzyPinyinOptions::default());
            assert_eq!(dictionary.pinyin_segmentation, "fa'nou'er");
            let sentence = rows
                .iter()
                .find(|row| row.word == "帆偶尔")
                .expect("备选切分应组句");
            assert_eq!(sentence.pinyin, "fan'ou'er");
            assert_eq!(sentence.canonical_pinyin, "fan'ou'er");
            assert!(sentence.sentence_association);
            assert!(sentence.corrected_from.is_empty());
            let prefix = rows
                .iter()
                .find(|row| row.word == "帆")
                .expect("备选切分应查前缀");
            assert_eq!(prefix.pinyin, "fan");
            assert_eq!(prefix.canonical_pinyin, "fan");
            assert!(prefix.corrected_from.is_empty());
            assert!(
                rows.iter().position(|row| row.word == "帆偶尔").unwrap()
                    < rows.iter().position(|row| row.word == "帆").unwrap()
            );
            let mut words = HashSet::new();
            assert!(rows.iter().all(|row| words.insert(&row.word)));
        }
    }
}

#[test]
fn literal_alternative_series_respects_manual_boundaries_and_lattice_switch() {
    let fixture = fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let rows = dictionary.query_exact("fa'nouer", "", 0);
    assert!(!rows
        .iter()
        .any(|row| row.word == "帆" || row.word == "帆偶尔"));
    dictionary.set_sentence_association(SentenceAssociationOptions {
        word_lattice: false,
        ..SentenceAssociationOptions::default()
    });
    let rows = dictionary.query_exact("fanouer", "", 0);
    assert!(rows.iter().any(|row| row.word == "帆"));
    assert!(!rows.iter().any(|row| row.word == "帆偶尔"));
}

#[test]
fn literal_alternative_series_keeps_complete_dictionary_words_before_generated_sentences() {
    let fixture = fixture();
    fixture.insert("qia'nou'er", "恰耨尔", 3000);
    fixture.insert("qi'a'nou'er", "旗阿耨尔", 2000);
    fixture.insert("qian", "签", 200);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let rows = dictionary.query_exact("qianouer", "", 0);
    assert_eq!(rows[0].word, "恰耨尔");
    assert_eq!(rows[1].word, "旗阿耨尔");
    let sentence = rows
        .iter()
        .position(|row| row.word == "签偶尔")
        .expect("完整词存在时仍查备选整句");
    assert!(sentence > 1);
}

#[test]
fn literal_alternative_series_supports_different_syllable_counts_and_boundaries() {
    for (raw, canonical, expected, entries) in [
        (
            "qiefeihong",
            "qi'e'fei'hong",
            "旗鹅飞虹",
            [("qi'e", "旗鹅"), ("fei'hong", "飞虹")],
        ),
        (
            "shananfei",
            "shan'an'fei",
            "山安飞",
            [("shan", "山"), ("an'fei", "安飞")],
        ),
        (
            "quxianqie",
            "qu'xi'an'qie",
            "曲西岸切",
            [("qu", "曲"), ("xi'an'qie", "西岸切")],
        ),
    ] {
        let fixture = Fixture::new();
        for (key, word) in entries {
            fixture.insert(key, word, 100);
        }
        let mut dictionary = QuanpinDictionary::new(&fixture.paths);
        let rows = dictionary.query_exact(raw, "", 0);
        let sentence = rows
            .iter()
            .find(|row| row.word == expected)
            .expect("不同合法切分均可组句");
        assert_eq!(sentence.pinyin, canonical);
        assert_eq!(sentence.canonical_pinyin, canonical);
    }
}

#[test]
fn literal_alternative_series_registers_personal_scoring_for_single_syllable_primary() {
    let fixture = Fixture::new();
    fixture.insert("xi", "西", 100).insert("an", "岸", 100);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let rows = dictionary.query_exact("xian", "", 0);
    assert!(rows.iter().any(|row| row.word == "西岸"));
    let slot = dictionary.series_slot(&series_cache_key("xian", "xian", 0));
    assert!(dictionary.personal_scored_keys.contains(&slot));
    dictionary
        .personal
        .record(&[PersonalTransition {
            earlier: String::new(),
            previous: "西".into(),
            word: "岸".into(),
            times: 1,
        }])
        .unwrap();
    dictionary.drop_personal_scored_results();
    assert!(!dictionary.series_cache.contains(&slot));
    assert!(dictionary
        .query_exact("xian", "", 0)
        .iter()
        .any(|row| row.word == "西岸"));
}

#[test]
fn literal_alternative_series_deduplicates_by_coverage_without_comparing_sentence_scores() {
    let row = |pinyin: &str, word: &str, weight: i64, source| {
        WordItem::new(pinyin, word, weight, source, pinyin)
    };
    let mut rows = vec![
        row("fan'ou'er", "合成整词", 1, CandidateSource::Database),
        row("fan", "合成同词", 10000, CandidateSource::Database),
    ];
    merge_series_by_coverage(
        &mut rows,
        vec![
            row("fan'ou'er", "合成同词", -100, CandidateSource::Generated),
            row("fan'ou'er", "合成整句", 9000, CandidateSource::Generated),
        ],
    );
    assert_eq!(
        rows.iter().map(|row| row.word.as_str()).collect::<Vec<_>>(),
        ["合成整词", "合成同词", "合成整句"]
    );
    assert_eq!(rows[1].pinyin, "fan'ou'er");
    assert_eq!(rows[1].canonical_pinyin, "fan'ou'er");
}

#[test]
fn literal_alternative_series_reuses_prefix_rows_across_dense_paths() {
    let fixture = Fixture::new();
    fixture
        .insert("xian", "先", 100)
        .insert("xi'an", "西岸", 90)
        .insert("xi", "西", 80)
        .insert("an", "岸", 80);
    let raw = "xianxianxianxian";
    let primary = "xian'xian'xian'xian";
    let segments = vec!["xian".to_owned(); 4];
    let resolution = resolve_series_query(raw, &segments, 0);
    let alternatives = alternative_segmentations(raw, &segments, &resolution, 0);
    assert!(alternatives.len() >= 8);
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    // 对照保留逐路径查询；预热共享表和行缓存，计数只比较重复前缀的字段复制。
    let unshared = |dictionary: &mut QuanpinDictionary, primary_rows| {
        let mut rows =
            dictionary.merge_alternative_full(raw, primary, &segments, &alternatives, primary_rows);
        let mut supplemental = Vec::new();
        for path in &alternatives {
            supplemental.extend(dictionary.query_series_with_typos(
                raw,
                &join_segments(path),
                path,
                false,
                None,
            ));
        }
        merge_series_by_coverage(&mut rows, supplemental);
        rows
    };
    let primary_rows = before.query_series(raw, primary, &segments);
    drop(unshared(&mut before, primary_rows));
    let primary_rows = after.query_series(raw, primary, &segments);
    drop(after.merge_alternatives(raw, primary, &segments, &alternatives, primary_rows));
    let old_primary = before.query_series(raw, primary, &segments);
    let new_primary = after.query_series(raw, primary, &segments);
    let (expected, old) =
        crate::ime::personal_rerank::allocations::count(|| unshared(&mut before, old_primary));
    let (actual, new) = crate::ime::personal_rerank::allocations::count(|| {
        after.merge_alternatives(raw, primary, &segments, &alternatives, new_primary)
    });
    assert_eq!(actual, expected);
    // 行字段改成 `SharedString` 后，逐路径查询的对照克隆重复前缀行只加引用计数，共享前缀不再省下申请；剩下的差额是前缀表 `queried_prefixes` 自身的哈希表和键。
    assert_eq!(new, old + 19, "共享前缀的额外申请变化：{old}→{new}");
}

#[test]
fn literal_alternative_series_shared_prefixes_keep_the_original_sparse_threshold() {
    let fixture = Fixture::new();
    for word in "甲乙丙丁戊己庚辛".chars() {
        fixture.insert("dia", &word.to_string(), 100);
    }
    fixture.insert("di", "低", 100);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.set_sentence_association(SentenceAssociationOptions {
        word_lattice: false,
        ..SentenceAssociationOptions::default()
    });
    let segments = ["dia", "xi", "an"].map(str::to_owned);
    let control =
        dictionary.query_series_with_typos("diaxian", "dia'xi'an", &segments, false, None);
    assert_eq!(control.len(), 8);
    let mut queried = HashMap::from([("dia".to_owned(), control.len())]);
    let shared = dictionary.query_series_with_typos(
        "diaxian",
        "dia'xi'an",
        &segments,
        false,
        Some(&mut queried),
    );
    assert!(
        shared.is_empty(),
        "已在主路径显示的八行仍满足稀疏门槛，不能额外回退到 di"
    );
}
