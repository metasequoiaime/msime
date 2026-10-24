use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;
use crate::types::autocorrect_type;

const NONE: u32 = 0;
const BOTH: u32 = autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR;

// 固定 db43f05f0f1c067f65122376ebc0fc7dcf2376e9 的完整展开与旧首字母包装，只改方法名、转接旧入口并移除注释。
impl QuanpinDictionary {
    fn legacy_query_initial(&self, code: &str, limit: usize) -> Vec<WordItem> {
        if code.len() != 1 {
            return Vec::new();
        }
        initial_items(self.database.query_initial(code, limit))
    }

    fn legacy_expand_initial_candidates(
        &mut self,
        raw: &str,
        segmentation: &str,
        autocorrect_types: u32,
        code: &str,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        if code.len() != 1 {
            return false;
        }
        let is_limited_initial =
            |item: &WordItem| item.source == CandidateSource::Database && item.pinyin == code;
        let limited_count = candidates
            .iter()
            .filter(|item| is_limited_initial(item))
            .count();
        if limited_count != INITIAL_CANDIDATE_LIMIT {
            return false;
        }
        let mut expanded = self.legacy_query_initial(code, UNLIMITED_ROWS);
        if expanded.len() <= limited_count {
            return false;
        }
        for item in &mut expanded {
            item.canonical_pinyin = std::mem::replace(&mut item.pinyin, code.into());
        }

        let mut merged = Vec::with_capacity(candidates.len() - limited_count + expanded.len());
        let mut inserted = false;
        for item in candidates.drain(..) {
            if is_limited_initial(&item) {
                if !inserted {
                    merged.extend(expanded.iter().cloned());
                    inserted = true;
                }
                continue;
            }
            merged.push(item);
        }
        *candidates = merged;
        self.cache.insert(code.to_string(), expanded.into());
        let segments = self.resolve_segments(raw, segmentation);
        let resolution = self.resolution(raw, segmentation, &segments, autocorrect_types);
        let key = self.series_slot(&series_cache_key(
            raw,
            &resolution.segmentation,
            autocorrect_types,
        ));
        self.series_cache.insert(key, candidates.clone());
        true
    }
}

fn fixture(rows: usize) -> Fixture {
    let fixture = Fixture::new();
    for index in 0..rows {
        let key = ["ni", "na", "nu", "nai"][index % 4];
        let weight = match index % 4 {
            0 => -7,
            1 => i64::MAX,
            _ => 100,
        };
        fixture.insert(key, &format!("合成展开{index:04}😀"), weight);
    }
    fixture.insert("ma", "合成其它首字母", 5);
    fixture
}

fn query(dictionary: &mut QuanpinDictionary, segmentation: &str, types: u32) -> Vec<WordItem> {
    dictionary.query("n", segmentation, types, FuzzyPinyinOptions { rules: 0 })
}

#[test]
fn successful_expansion_saves_one_discarded_key_per_row() {
    for rows in [25, 32, 128] {
        let fixture = fixture(rows);
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let mut expected = query(&mut before, "n", NONE);
        let mut actual = query(&mut after, "n", NONE);
        drop(before.database.query_initial("n", UNLIMITED_ROWS));
        drop(after.database.query_initial("n", UNLIMITED_ROWS));
        let (old_result, old_count) =
            count(|| before.legacy_expand_initial_candidates("n", "n", NONE, "n", &mut expected));
        let (new_result, new_count) =
            count(|| after.expand_initial_candidates("n", "n", NONE, "n", &mut actual));
        assert_eq!(new_result, old_result);
        assert!(new_result);
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), expected.capacity());
        println!("首字母展开 {rows} 行：申请 {old_count}→{new_count}");
        // 行投影每行省一个完整键（字符串加 `SharedString` 的 `Arc`）；解析投影省两次复制，延迟音节另省列表与字符串两次申请。
        assert_eq!(
            new_count + 2 * rows + 4,
            old_count,
            "每行省一个完整键，解析投影与延迟音节再各省两次申请"
        );
    }
}

#[test]
fn warm_expansion_reuses_the_candidate_outer_buffer() {
    let fixture = fixture(32);
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    let mut expected = query(&mut before, "n", NONE);
    let mut actual = query(&mut after, "n", NONE);
    expected.reserve(256);
    actual.reserve(256);
    drop(before.database.query_initial("n", UNLIMITED_ROWS));
    drop(after.database.query_initial("n", UNLIMITED_ROWS));

    let (old_result, old_count) =
        count(|| before.legacy_expand_initial_candidates("n", "n", NONE, "n", &mut expected));
    let (new_result, new_count) =
        count(|| after.expand_initial_candidates("n", "n", NONE, "n", &mut actual));

    assert_eq!(new_result, old_result);
    assert!(new_result);
    assert_eq!(actual, expected);
    assert!(actual.capacity() >= 256);
    assert!(
        new_count < old_count,
        "预留候选容量后，合并仍申请 {new_count} 次，旧实现为 {old_count} 次"
    );
}

#[test]
fn expansion_preserves_foreign_rows_slots_order_and_owned_fields() {
    let fixture = fixture(32);
    for segmentation in ["", "n"] {
        for types in [NONE, BOTH] {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            let mut expected = query(&mut before, segmentation, types);
            let mut marker =
                WordItem::new("n", "合成云候选", -13, CandidateSource::AiSuggestion, "nu");
            marker.fuzzy = true;
            marker.fixed_position = 3;
            marker.corrected_from = "合成来源".into();
            marker.sentence_words = vec!["合成片段".to_owned()];
            expected.insert(0, marker.clone());
            expected.insert(
                8,
                WordItem::new("ma", "合成其它行", 42, CandidateSource::Database, "ma"),
            );
            expected.push(marker);
            let mut actual = expected.clone();
            assert_eq!(
                before.legacy_expand_initial_candidates(
                    "n",
                    segmentation,
                    types,
                    "n",
                    &mut expected
                ),
                after.expand_initial_candidates("n", segmentation, types, "n", &mut actual)
            );
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            for (new, old) in actual.iter().zip(&expected) {
                assert_eq!(new.pinyin.capacity(), old.pinyin.capacity());
                assert_eq!(
                    new.canonical_pinyin.capacity(),
                    old.canonical_pinyin.capacity()
                );
                assert_eq!(new.word.capacity(), old.word.capacity());
            }
            assert_eq!(actual[0].word, "合成云候选");
            assert_eq!(actual[33].word, "合成其它行");
            assert_eq!(actual.last().unwrap().word, "合成云候选");
            assert_eq!(after.cache.get_ref_by("n"), before.cache.get_ref_by("n"));
            let slot = after.series_slot(&series_cache_key("n", "n", types));
            assert_eq!(after.series_cache.get_ref_by(&slot), Some(&actual));
            assert_eq!(before.series_cache.get_ref_by(&slot), Some(&expected));
            actual[1].word.push_str("修改副本");
            assert_ne!(
                after.cache.get_ref_by("n").unwrap().copy_rows("n")[0].word,
                actual[1].word
            );
            after.reset_cache();
            drop(after);
            assert!(actual[1].word.ends_with("修改副本"));
        }
    }
}

#[test]
fn unsuccessful_expansion_and_readonly_jianpin_query_keep_their_contract() {
    for rows in [0, 1, 24, 25] {
        let fixture = fixture(rows);
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        for code in ["", "ni", "x", "n"] {
            let mut expected = query(&mut before, "n", NONE);
            let mut actual = query(&mut after, "n", NONE);
            let original = actual.clone();
            let old = before.legacy_expand_initial_candidates("n", "n", NONE, code, &mut expected);
            let new = after.expand_initial_candidates("n", "n", NONE, code, &mut actual);
            assert_eq!(new, old);
            assert_eq!(actual, expected);
            if !new {
                assert_eq!(actual, original);
            }
        }
        let direct = after.query_jianpin_codes(&["n".to_owned()], UNLIMITED_ROWS);
        assert_eq!(direct.len(), rows);
        assert!(direct
            .iter()
            .all(|row| row.pinyin == row.canonical_pinyin && row.pinyin != "n"));
    }
}
