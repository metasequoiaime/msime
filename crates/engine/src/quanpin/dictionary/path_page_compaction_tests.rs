use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

impl QuanpinDictionary {
    // 固定 53df4a89c1 的完整冷路径；into 仅兼容缓存值的拥有型表示。
    fn legacy_page_miss(
        &mut self,
        cache_key: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        let result = self.query_database(segments, segmentation);
        self.cache
            .insert(cache_key.to_owned(), result.clone().into());
        result
    }
}

fn equal_rows(actual: &Vec<WordItem>, expected: &Vec<WordItem>) {
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
fn cold_dictionary_page_avoids_repeated_reading_copies() {
    for rows in [0usize, 1, 2, 3, 4, 32, 128] {
        let fixture = Fixture::new();
        for index in 0..rows {
            fixture.insert("ni", &format!("合成词{index}😀"), 1000 - index as i64);
        }
        for cache_key in ["ni", "typed-ni", "", "合成😀"] {
            let segments = vec!["ni".to_owned()];
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            // 数据库和原始行缓存的预热不进入本片复制计量。
            drop(before.query_database(&segments, "ni"));
            drop(after.query_database(&segments, "ni"));
            let (expected, old) = count(|| before.legacy_page_miss(cache_key, "ni", &segments));
            let (mut actual, new) =
                count(|| after.query_single_path_miss(cache_key, "ni", &segments));
            equal_rows(&actual, &expected);
            let overhead = 1 + usize::from(cache_key != "ni");
            let saved = rows.saturating_sub(overhead);
            assert_eq!(new + saved, old, "rows={rows} key={cache_key:?}");
            let expected_hot = before.query_single_path(cache_key, cache_key, &segments);
            let actual_hot = after.query_single_path(cache_key, cache_key, &segments);
            equal_rows(&actual_hot, &expected_hot);
            if let Some(first) = actual.first_mut() {
                first.word.push_str("修改副本");
                assert_ne!(actual, actual_hot);
            }
            after.reset_cache();
            drop(after);
            if rows != 0 {
                assert!(actual[0].word.ends_with("修改副本"));
            }
        }
    }
}

#[test]
fn expanded_initial_page_keeps_its_original_fifo_lifetime() {
    let fixture = Fixture::new();
    for index in 0..30 {
        fixture.insert("ni", &format!("合成首字母词{index}"), 1000 - index);
    }
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.cache = FifoCache::new(2);
    after.cache = FifoCache::new(2);
    let initial = vec!["n".to_owned()];
    let missing = vec!["missing".to_owned()];
    let mut old = before.query_single_path("n", "n", &initial);
    let mut new = after.query_single_path("n", "n", &initial);
    assert_eq!(old.len(), INITIAL_CANDIDATE_LIMIT);
    assert!(before.expand_initial_candidates("n", "n", 0, "n", &mut old));
    assert!(after.expand_initial_candidates("n", "n", 0, "n", &mut new));
    equal_rows(&new, &old);
    assert_eq!(new.len(), 30);
    for dictionary in [&mut before, &mut after] {
        dictionary.query_single_path("first", "first", &missing);
        assert_eq!(dictionary.query_single_path("n", "n", &initial).len(), 30);
        dictionary.query_single_path("second", "second", &missing);
        assert!(dictionary.cache.get_ref_by("n").is_none());
    }
    let old = before.query_single_path("n", "n", &initial);
    let new = after.query_single_path("n", "n", &initial);
    equal_rows(&new, &old);
    assert_eq!(new.len(), INITIAL_CANDIDATE_LIMIT);
}

#[test]
fn compact_page_preserves_metadata_and_both_output_paths() {
    use crate::types::SchemeType;
    for changed in 0..10 {
        let mut rows =
            vec![WordItem::new("ni", "合成😀", -3, CandidateSource::Database, "nve"); 4];
        match changed {
            0 => {}
            1 => rows[1].pinyin = "other".into(),
            2 => rows[1].source = CandidateSource::Generated,
            3 => rows[1].scheme = SchemeType::Wubi,
            4 => rows[1].fixed_position = 2,
            5 => rows[1].fuzzy = true,
            6 => rows[1].corrected_from = "synthetic".into(),
            7 => rows[1].sentence_association = true,
            8 => rows[1].sentence_words = vec!["合成".into(), "😀".into()],
            9 => rows.iter_mut().for_each(|row| row.pinyin.clear()),
            _ => unreachable!(),
        }
        let page = PathPage::from_dictionary("ni", &rows);
        assert_eq!(matches!(page, PathPage::Compact(_)), changed == 0);
        equal_rows(&page.copy_rows("ni"), &rows.clone());
        equal_rows(&page.cloned_rows("ni").collect(), &rows.clone());
        let mut expected = vec![WordItem::default()];
        let mut actual = expected.clone();
        expected.reserve(rows.len());
        expected.extend(rows.iter().cloned());
        page.append_to("ni", &mut actual);
        equal_rows(&actual, &expected);
    }
    let mut rows = Vec::with_capacity(12);
    rows.push(WordItem::new(
        "ni",
        "完整页",
        9,
        CandidateSource::Database,
        "ni",
    ));
    let capacity = rows.capacity();
    let PathPage::Full(rows) = PathPage::from(rows) else {
        panic!("完整页必须原样转移");
    };
    assert_eq!(rows.capacity(), capacity);
}
