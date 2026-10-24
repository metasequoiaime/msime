use super::filtered_query_tests::assert_rows;
use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 冻结 e7fa91f121e0b071bddfaa96917f87869e37018e 的完整未命中函数，只改私有方法名。
impl QuanpinDictionary {
    fn before_inline_path_key(
        &mut self,
        cache_key: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        let result = self.query_database(segments, segmentation);
        self.cache.insert(cache_key.to_owned(), result.clone());
        result
    }
}

fn slot(index: usize) -> String {
    format!("合成路径槽{index:03}")
}

#[test]
fn short_path_misses_skip_only_the_owned_key_allocation() {
    let keys = [
        "".to_owned(),
        "n".to_owned(),
        "ni".to_owned(),
        "合成短键".to_owned(),
        "a".repeat(31),
        "a".repeat(32),
        "合成汉字".repeat(8),
    ];
    for n in [0, 1, 4, 128] {
        let fixture = Fixture::new();
        fixture.table("ni");
        for index in 0..n {
            fixture.insert("ni", &format!("合成路径{index:03}😀"), index as i64);
        }
        for full in [false, true] {
            for key in &keys {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                let segments = vec!["ni".to_owned()];
                let rows = before.query_database(&segments, "ni");
                assert_rows(&after.query_database(&segments, "ni"), &rows, false);
                for dictionary in [&mut before, &mut after] {
                    for index in 0..if full {
                        CACHE_CAPACITY
                    } else {
                        CACHE_CAPACITY - 1
                    } {
                        dictionary.cache.insert(slot(index), Vec::new());
                    }
                }
                let (expected, old_count) =
                    count(|| before.before_inline_path_key(key, "ni", &segments));
                let (actual, new_count) =
                    count(|| after.query_single_path_miss(key, "ni", &segments));
                assert_rows(&actual, &expected, false);
                assert_rows(
                    after.cache.get_ref_by(key).unwrap(),
                    before.cache.get_ref_by(key).unwrap(),
                    false,
                );
                let saved = usize::from(!key.is_empty() && key.len() <= 31);
                assert_eq!(
                    new_count + saved,
                    old_count,
                    "n={n} full={full} key_bytes={}",
                    key.len()
                );
                assert_eq!(after.cache.get_ref_by(&slot(0)).is_some(), !full);
                println!(
                    "inline-key n={n} full={full} key_bytes={} old={old_count} new={new_count}",
                    key.len()
                );
            }
        }
    }
}

#[test]
fn inline_path_keys_keep_fifo_reads_reinsertion_and_independent_results() {
    let fixture = Fixture::new();
    fixture.insert("ni", "合成原结果", 1);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let segments = vec!["ni".to_owned()];
    let original = dictionary.query_database(&segments, "ni");
    for index in 0..CACHE_CAPACITY {
        dictionary.cache.insert(slot(index), original.clone());
    }
    let oldest = slot(0);
    assert_eq!(
        dictionary.query_single_path(&oldest, "", &segments),
        original
    );
    dictionary.cache.insert(oldest.clone(), original.clone());
    let retained = dictionary.query_single_path("ni", "ni", &segments);
    assert!(!dictionary.cache.get_ref_by(&oldest).is_some());
    assert!(dictionary.cache.get_ref_by(&slot(1)).is_some());
    assert_eq!(retained, original);
    dictionary.cache.insert("合成后续路径".into(), Vec::new());
    assert!(!dictionary.cache.get_ref_by(&slot(1)).is_some());
    assert!(dictionary.cache.get_ref_by("ni").is_some());
    let mut owned = dictionary.query_single_path("ni", "ni", &segments);
    owned[0].word.push_str("合成独立修改");
    assert_eq!(
        dictionary.cache.get_ref_by("ni").unwrap(),
        &retained.clone()
    );
    dictionary.reset_cache();
    assert!(dictionary.cache.get_ref_by("ni").is_none());
    fixture.insert("ni", "合成更新结果", 2);
    let updated = dictionary.query("ni", "ni", 0, FuzzyPinyinOptions { rules: 0 });
    assert!(updated.iter().any(|row| row.word == "合成更新结果"));
    assert_eq!(retained, original);
}
