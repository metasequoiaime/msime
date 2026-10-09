use super::super::decoder::test_model;
use super::inline_row_tests::vector_reference_search;
use super::*;

// 按旧空读音分支核算实际执行的非空查询；满页后不计后续假名前缀。
pub(super) fn saved_initial_prefix_views(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
) -> usize {
    if !conversion.hiragana.is_empty() || limit == 0 {
        return 0;
    }
    let mut words = Vec::new();
    let mut hits = 0;
    for kana in kana_for_romaji_prefix_view(&conversion.pending) {
        let views = dictionary.prefix_lemma_views(kana, 24);
        hits += usize::from(!views.is_empty());
        for lemma in views {
            if !lemma.surface.is_empty() && !words.contains(&lemma.surface) {
                words.push(lemma.surface);
            }
            if words.len() >= limit {
                return hits;
            }
        }
    }
    hits
}

fn fixture(dense: bool) -> JapaneseDictionary {
    let surfaces: Vec<_> = (0..70).map(|index| format!("合成語{index:02}😀")).collect();
    let mut entries = vec![
        ("か", "共通😀", 0, 0, -500),
        ("か", "仮", 0, 0, -500),
        ("かき", "仮木", 0, 0, 100),
        ("き", "共通😀", 0, 0, -900),
        ("き", "木", 0, 0, -700),
        ("し", "詩", 0, 0, -20),
        ("しゃ", "共通😀", 0, 0, -40),
        ("しゃ", "合成社", 0, 0, -40),
        ("しゅ", "合成朱", 0, 0, 0),
        ("っ", "仮促", 0, 0, 10),
    ];
    if dense {
        entries.extend(
            surfaces
                .iter()
                .enumerate()
                .map(|(index, surface)| ("か", surface.as_str(), 0, 0, index as i32 % 7 - 1000)),
        );
    }
    entries.sort_by_key(|entry| entry.0);
    JapaneseDictionary::from_bytes(test_model::bytes(&entries, 1, &[0]).into_boxed_slice())
        .expect("合成词库")
}

#[test]
fn initial_prefix_matrix_matches_fixed_query_and_saves_each_hit_vector() {
    for dense in [false, true] {
        let dictionary = fixture(dense);
        for pending in [
            "", "k", "K", "sh", "sy", "xt", "d", "q", "z", "あ", "toolong",
        ] {
            let conversion = RomajiConversion {
                hiragana: String::new(),
                pending: pending.to_owned(),
                complete: pending.is_empty(),
            };
            for limit in [0, 1, 2, 12, 24, 25, 48, 64, 100] {
                let saved = saved_initial_prefix_views(&dictionary, &conversion, limit);
                let (expected, old_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        vector_reference_search(&dictionary, &conversion, limit)
                    });
                let (actual, new_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        search_converted(&dictionary, &conversion, limit)
                    });
                assert_eq!(
                    actual, expected,
                    "密集={dense}，待定={pending}，限额={limit}"
                );
                assert_eq!(
                    new_allocations + saved,
                    old_allocations,
                    "只省已执行的视图容器：密集={dense}，待定={pending}，限额={limit}"
                );
                let mut destination = Vec::with_capacity(limit);
                let pointer = destination.as_ptr();
                let ((), reused_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        search_converted_into(&dictionary, &conversion, limit, &mut destination);
                    });
                assert_eq!(destination, expected);
                assert_eq!(destination.as_ptr(), pointer);
                assert_eq!(reused_allocations + usize::from(limit > 0), new_allocations);
            }
        }
    }
}

#[test]
fn initial_prefix_provider_reuses_full_rows_without_view_vectors() {
    use super::super::provider::JapaneseProvider;
    use crate::types::{CandidateSource, QueryRequest, SchemeType, WordItem};
    let root = tempfile::tempdir().expect("合成目录");
    let model = root.path().join("synthetic-japanese.dat");
    std::fs::write(
        &model,
        test_model::bytes(
            &[
                ("か", "蚊", 0, 0, 500),
                ("かき", "柿", 0, 0, 300),
                ("き", "木", 0, 0, 100),
            ],
            1,
            &[0],
        ),
    )
    .expect("合成词库");
    let mut provider = JapaneseProvider::new(&model);
    assert!(provider.cache_dynamic_candidate("k", "合成雲", CandidateSource::CloudSuggestion));
    let mut destination = Vec::new();
    for code in ["k", "K", "k"] {
        let request = QueryRequest {
            scheme: SchemeType::JapaneseRomaji,
            raw_input: "k".to_owned(),
            raw_input_with_cases: code.to_owned(),
            valid: true,
            ..QueryRequest::default()
        };
        let expected = vec![
            WordItem::new(code, "柿", 899_700, CandidateSource::Database, code),
            WordItem::new("k", "合成雲", 1, CandidateSource::CloudSuggestion, "k"),
            WordItem::new(code, "蚊", 899_500, CandidateSource::Database, code),
            WordItem::new(code, "木", 899_900, CandidateSource::Database, code),
        ];
        // 动态行会旋转槽位，预热所有槽位到最长词面的容量。
        for _ in 0..4 {
            provider.query_into(&request, &mut destination);
        }
        assert_eq!(destination, expected);
        let pointer = destination.as_ptr();
        for row in &mut destination {
            row.source = CandidateSource::AiSuggestion;
            row.fixed_position = 7;
            row.fuzzy = true;
            row.corrected_from.push_str("synthetic");
            row.sentence_association = true;
            row.sentence_words.push("synthetic".to_owned());
        }
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            provider.query_into(&request, &mut destination);
        });
        assert_eq!(destination, expected);
        assert_eq!(destination.as_ptr(), pointer);
        eprintln!("日文仅待定字母 provider 热查询分配：{allocations}");
        assert_eq!(allocations, 3, "只保留三个句子结果文本，不重建前缀视图向量");
    }
}
