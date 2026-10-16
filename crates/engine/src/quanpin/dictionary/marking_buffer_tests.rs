use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

fn dictionary(fixture: &Fixture) -> QuanpinDictionary {
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.sentence_association.word_lattice = false;
    dictionary
}

#[test]
fn repeated_queries_reuse_the_primary_marking_string() {
    let fixture = Fixture::new();
    fixture.insert("ni", "合成单字", 100);
    fixture.insert("ni'hao", "合成双字", 100);
    fixture.insert("nve", "合成别名", 100);
    fixture.insert("gua", "合成纠错", 100);
    let mut before = dictionary(&fixture);
    let mut after = dictionary(&fixture);
    // 先用长读法准备容量，随后短读法交替不能丢掉这份存储。
    let longest = "he'cheng'ce'shi'du'yin";
    before.query_exact(longest, longest, 0);
    after.query_exact(longest, longest, 0);
    let capacity = after.pinyin_segmentation.capacity();
    for mode in 0..3 {
        for (raw, segmentation, types) in [
            ("ni", "ni", 0),
            ("nihao", "ni'hao", 0),
            ("nue", "nue", 0),
            (
                "gau",
                "",
                autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
            ),
            ("uanli", "", autocorrect_type::DELETION),
            ("!", "", 0),
        ] {
            before.query_exact(raw, segmentation, types);
            after.query_exact(raw, segmentation, types);
            if mode > 0 {
                before.series_cache.clear();
                after.series_cache.clear();
            }
            if mode == 2 {
                before.resolution_cache.clear();
                after.resolution_cache.clear();
            }
            // 对照只丢弃主标记存储，模拟每次重新复制；不改缓存或候选状态。
            before.pinyin_segmentation = String::new();
            let (expected, old_count) = count(|| before.query_exact(raw, segmentation, types));
            let (actual, new_count) = count(|| after.query_exact(raw, segmentation, types));
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert_eq!(after.pinyin_segmentation, before.pinyin_segmentation);
            assert_eq!(
                after.alternative_segmentations,
                before.alternative_segmentations
            );
            assert_eq!(after.personal_scored_keys, before.personal_scored_keys);
            eprintln!("mode={mode} raw={raw}: {old_count}→{new_count}");
            assert_eq!(new_count + 1, old_count, "已有标记容量应少申请一次");
            assert_eq!(after.pinyin_segmentation.capacity(), capacity);
        }
    }
}

#[test]
fn growing_and_empty_queries_keep_marking_and_owned_results() {
    let fixture = Fixture::new();
    fixture.insert("ni'hao'ma", "合成候选", 100);
    let mut dictionary = dictionary(&fixture);
    dictionary.query_exact("ni", "ni", 0);
    let short_capacity = dictionary.pinyin_segmentation.capacity();
    assert!(short_capacity < "ni'hao'ma".len());
    let mut rows = dictionary.query_exact("nihaoma", "ni'hao'ma", 0);
    assert_eq!(dictionary.pinyin_segmentation, "ni'hao'ma");
    let capacity = dictionary.pinyin_segmentation.capacity();
    let (empty, count) = count(|| dictionary.query_exact("", "", 0));
    assert!(empty.is_empty());
    assert_eq!(count, 0);
    assert_eq!(dictionary.pinyin_segmentation, "ni'hao'ma");
    assert_eq!(dictionary.pinyin_segmentation.capacity(), capacity);
    rows[0].word.push_str("独立副本");
    assert_ne!(rows, dictionary.query_exact("nihaoma", "ni'hao'ma", 0));
    dictionary.reset_cache();
    drop(dictionary);
    assert!(rows[0].word.ends_with("独立副本"));
}
