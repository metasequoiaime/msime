use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

fn old_filtered(
    dictionary: &mut QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    fuzzy: FuzzyPinyinOptions,
    mut keep: impl FnMut(&str, &str, &str, bool) -> bool,
) -> Vec<WordItem> {
    let mut rows = dictionary.query(raw, segmentation, 0, fuzzy);
    rows.retain(|row| keep(&row.word, &row.pinyin, &row.canonical_pinyin, row.fuzzy));
    rows
}

fn assert_rows(actual: &Vec<WordItem>, expected: &Vec<WordItem>) {
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
    for (actual, expected) in actual.iter().zip(expected) {
        for (actual, expected) in [
            (&actual.pinyin, &expected.pinyin),
            (&actual.canonical_pinyin, &expected.canonical_pinyin),
            (&actual.word, &expected.word),
            (&actual.corrected_from, &expected.corrected_from),
        ] {
            assert_eq!(actual.capacity(), expected.capacity());
        }
        assert_eq!(
            actual.sentence_words.capacity(),
            expected.sentence_words.capacity()
        );
        for (actual, expected) in actual.sentence_words.iter().zip(&expected.sentence_words) {
            assert_eq!(actual.capacity(), expected.capacity());
        }
    }
}

#[test]
fn filtered_hot_answers_do_not_clone_rejected_row_fields() {
    for n in [1, 4, 32, 128] {
        let fixture = Fixture::new();
        for index in 0..n {
            fixture.insert("ni", &format!("合成{index}😀"), index as i64);
        }
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let fuzzy = FuzzyPinyinOptions { rules: 0 };
        let rows = before.query("ni", "ni", 0, fuzzy);
        assert_eq!(rows.len(), n);
        drop(after.query("ni", "ni", 0, fuzzy));
        for keep_count in [0, 1, n / 2, n] {
            let mut old_index = 0;
            let mut new_index = 0;
            let (expected, old_count) = count(|| {
                old_filtered(&mut before, "ni", "ni", fuzzy, |_, _, _, _| {
                    old_index += 1;
                    old_index <= keep_count
                })
            });
            let (actual, new_count) = count(|| {
                after.query_filtered("ni", "ni", fuzzy, |_, _, _, _| {
                    new_index += 1;
                    new_index <= keep_count
                })
            });
            assert_rows(&actual, &expected);
            assert_eq!((old_index, new_index), (n, n));
            let (_, rejected_copies) = count(|| rows[keep_count..].iter().cloned().for_each(drop));
            assert_eq!(
                new_count + rejected_copies,
                old_count,
                "行数={n}/保留={keep_count}"
            );
        }
    }
}

#[test]
fn filtered_queries_preserve_cold_fuzzy_marking_and_cache_state() {
    let fixture = Fixture::new();
    for (key, word, weight) in [
        ("ni", "合成甲", 500),
        ("ni", "合成乙", -2),
        ("ni'hao", "合成补全", 100),
        ("xi", "合成西", 90),
        ("xi'an", "合成分段", 80),
        ("xian", "合成单段", 70),
        ("nve", "合成别名", 60),
        ("zi", "合成平舌", 50),
        ("zhi", "合成翘舌", 40),
        ("shang", "合成换位", 30),
    ] {
        fixture.insert(key, word, weight);
    }
    for (raw, segmentation) in [
        ("ni", "ni"),
        ("nihao", "ni'hao"),
        ("nihao", ""),
        ("xian", ""),
        ("nue", ""),
        ("zi", "zi"),
        ("sahng", ""),
        ("zzz", ""),
        ("!", ""),
        ("", ""),
    ] {
        for rules in [0, crate::types::fuzzy_rule::Z_ZH] {
            for predicate in 0..4 {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                let fuzzy = FuzzyPinyinOptions { rules };
                for phase in 0..4 {
                    if phase == 2 {
                        before.reset_cache();
                        after.reset_cache();
                    }
                    let keep =
                        |word: &str, reading: &str, canonical: &str, fuzzy: bool| match predicate {
                            0 => true,
                            1 => false,
                            2 => word.ends_with('甲') || canonical.contains('\''),
                            _ => !fuzzy && reading.len() <= 3,
                        };
                    let (expected, old_count) =
                        count(|| old_filtered(&mut before, raw, segmentation, fuzzy, keep));
                    let (mut actual, new_count) =
                        count(|| after.query_filtered(raw, segmentation, fuzzy, keep));
                    assert_rows(&actual, &expected);
                    assert!(
                        new_count <= old_count,
                        "{raw}/{segmentation}/{rules}/{predicate}/{phase}: {new_count}>{old_count}"
                    );
                    assert_eq!(after.pinyin_segmentation, before.pinyin_segmentation);
                    assert_eq!(
                        after.alternative_segmentations,
                        before.alternative_segmentations
                    );
                    assert_eq!(after.personal_scored_keys, before.personal_scored_keys);
                    assert_eq!(after.personal_version, before.personal_version);
                    assert_eq!(after.typo_generation, before.typo_generation);
                    assert_eq!(
                        after.current_autocorrect_types,
                        before.current_autocorrect_types
                    );
                    // 修改本次拥有结果不能污染后续热槽。
                    for row in &mut actual {
                        row.word.clear();
                        row.pinyin.clear();
                        row.canonical_pinyin.clear();
                        row.sentence_words.clear();
                    }
                }
            }
        }
    }
}
