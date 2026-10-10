use super::super::decoder::test_model;
use super::super::romaji::convert_romaji;
use super::inline_row_tests::vector_reference_search;
use super::*;

fn fixture(dense: bool) -> JapaneseDictionary {
    let surfaces: Vec<_> = (0..24).map(|index| format!("合成{index:02}")).collect();
    let mut entries = vec![
        ("か", "仮", 0, 1, -500),
        ("か", "甲", 1, 0, 0),
        ("か", "仮", 0, 0, 5),
        ("かか", "仮仮", 1, 0, -700),
        ("かき", "仮木", 1, 1, -300),
        ("かきゃ", "仮甲", 0, 0, 20),
        ("かっ", "仮促", 0, 0, 30),
        ("き", "木", 1, 0, -100),
        ("甲😀か", "合成長", 0, 1, -40),
    ];
    if dense {
        entries.extend(surfaces.iter().enumerate().map(|(index, surface)| {
            (
                "か",
                surface.as_str(),
                (index % 2) as u16,
                (index % 2) as u16,
                index as i32 % 7 - 12,
            )
        }));
    }
    entries.sort_by_key(|entry| entry.0);
    JapaneseDictionary::from_bytes(
        test_model::bytes(&entries, 2, &[0, -20, 80, 10]).into_boxed_slice(),
    )
    .expect("合成词库")
}

#[test]
fn exact_streamed_matrix_matches_fixed_vector_query_including_full_pages() {
    for dense in [false, true] {
        let dictionary = fixture(dense);
        for reading in [
            "",
            "か",
            "かき",
            "きき",
            "くく",
            "甲😀か",
            &"か".repeat(32),
            &"😀".repeat(64),
        ] {
            for pending in ["", "k", "K", "sh", "xt", "d", "z", "q", "あ", "toolong"] {
                let conversion = RomajiConversion {
                    hiragana: reading.to_owned(),
                    pending: pending.to_owned(),
                    complete: pending.is_empty(),
                };
                let _ = search_converted(&dictionary, &conversion, 16);
                for limit in [0, 1, 2, 8, 16, 32] {
                    let (expected, old_allocations) =
                        crate::ime::personal_rerank::allocations::count(|| {
                            vector_reference_search(&dictionary, &conversion, limit)
                        });
                    let (actual, new_allocations) =
                        crate::ime::personal_rerank::allocations::count(|| {
                            search_converted(&dictionary, &conversion, limit)
                        });
                    assert_eq!(
                        actual,
                        expected,
                        "密集={dense}，读音字符={}，待定={pending}，限额={limit}",
                        reading.chars().count()
                    );
                    assert!(new_allocations <= old_allocations);
                    if reading.is_empty() {
                        let saved = super::initial_prefix_tests::saved_initial_prefix_views(
                            &dictionary,
                            &conversion,
                            limit,
                        );
                        assert_eq!(new_allocations + saved, old_allocations);
                    } else if limit == 0 || reading == "くく" || reading.starts_with('😀') {
                        assert_eq!(
                            new_allocations
                                + super::rolling_row_tests::saved_row_buffers(&conversion, limit)
                                + super::boundary_tests::saved_boundary_buffers(&conversion, limit),
                            old_allocations
                        );
                    } else {
                        assert!(new_allocations < old_allocations);
                    }
                }
            }
        }
    }
}

#[test]
fn provider_exact_streaming_reuses_full_candidate_fields_and_saves_two_vectors() {
    use super::super::provider::JapaneseProvider;
    use crate::types::{CandidateSource, QueryRequest, SchemeType, WordItem};
    let root = tempfile::tempdir().expect("合成目录");
    let model = root.path().join("synthetic-japanese.dat");
    std::fs::write(
        &model,
        test_model::bytes(&[("か", "仮", 0, 0, 500)], 1, &[0]),
    )
    .expect("合成词库");
    let mut provider = JapaneseProvider::new(&model);
    assert!(provider.cache_dynamic_candidate("ka", "合成雲", CandidateSource::CloudSuggestion));
    let request = QueryRequest {
        scheme: SchemeType::JapaneseRomaji,
        raw_input: "ka".to_owned(),
        raw_input_with_cases: "Ka".to_owned(),
        valid: true,
        ..QueryRequest::default()
    };
    let mut destination = provider.query(&request);
    provider.query_into(&request, &mut destination);
    let expected = vec![
        WordItem::new("Ka", "か", 1_000_000, CandidateSource::Generated, "Ka"),
        WordItem::new("Ka", "カ", 999_999, CandidateSource::Generated, "Ka"),
        WordItem::new("ka", "合成雲", 1, CandidateSource::CloudSuggestion, "ka"),
        WordItem::new("Ka", "仮", 899_500, CandidateSource::Database, "Ka"),
    ];
    assert_eq!(destination, expected);
    let pointer = destination.as_ptr();
    for item in &mut destination {
        item.source = CandidateSource::AiSuggestion;
        item.fixed_position = 7;
        item.fuzzy = true;
        item.corrected_from.push_str("synthetic");
        item.sentence_association = true;
        item.sentence_words.push("synthetic".to_owned());
    }
    let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
        provider.query_into(&request, &mut destination)
    });
    assert_eq!(destination, expected);
    assert_eq!(destination.as_ptr(), pointer);
    assert_eq!(allocations, 7);
}

#[test]
#[ignore = "本地 release 与固定 Vec 查询对照；不设置 CI 时间阈值"]
fn benchmark_exact_lemma_streaming() {
    use std::hint::black_box;
    use std::time::Instant;
    for (dense, input, iterations) in [
        (false, "ka".to_owned(), 20_000),
        (false, "kiki".to_owned(), 20_000),
        (true, "ka".to_owned(), 10_000),
        (true, "kaka".to_owned(), 10_000),
        (false, "ka".repeat(32), 1000),
        (true, "ka".repeat(32), 1000),
        (false, "😀".repeat(128), 1000),
        (false, "kak".to_owned(), 10_000),
        (false, "k".to_owned(), 20_000),
        (false, "".to_owned(), 20_000),
    ] {
        let dictionary = fixture(dense);
        let conversion = if input.starts_with('😀') {
            RomajiConversion {
                hiragana: input.clone(),
                pending: String::new(),
                complete: true,
            }
        } else {
            convert_romaji(&input)
        };
        for limit in [0, 1, 16] {
            assert_eq!(
                search_converted(&dictionary, &conversion, limit),
                vector_reference_search(&dictionary, &conversion, limit)
            );
            let (_, old_allocations) = crate::ime::personal_rerank::allocations::count(|| {
                vector_reference_search(&dictionary, &conversion, limit)
            });
            let (_, new_allocations) = crate::ime::personal_rerank::allocations::count(|| {
                search_converted(&dictionary, &conversion, limit)
            });
            let mut timings = [Vec::new(), Vec::new()];
            for batch in 0..10 {
                for index in if batch % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let start = Instant::now();
                    for _ in 0..iterations {
                        black_box(if index == 0 {
                            vector_reference_search(
                                black_box(&dictionary),
                                black_box(&conversion),
                                black_box(limit),
                            )
                        } else {
                            search_converted(
                                black_box(&dictionary),
                                black_box(&conversion),
                                black_box(limit),
                            )
                        });
                    }
                    timings[index].push(start.elapsed());
                }
            }
            for samples in &mut timings {
                samples.sort_unstable();
            }
            eprintln!("exact流式：密集={dense}，读音字符={}，待定={}，限额={limit}，分配={old_allocations}→{new_allocations}，中位批次={:?}→{:?}/{iterations}次", conversion.hiragana.chars().count(), conversion.pending.len(), timings[0][5], timings[1][5]);
        }
    }
}
