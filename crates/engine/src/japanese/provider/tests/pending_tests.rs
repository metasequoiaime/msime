use super::*;

// 固定 f37203c77 的 provider 查询正文，矩阵调用保持当前实现以隔离 provider 拼接键。
fn reference_query_into(
    provider: &mut JapaneseProvider,
    request: &QueryRequest,
    destination: &mut Vec<WordItem>,
) {
    if !request.valid || request.scheme != SchemeType::JapaneseRomaji {
        destination.clear();
        return;
    }
    let mut rows = Rows {
        items: std::mem::take(destination),
        code: &request.raw_input_with_cases,
        used: 0,
    };
    if request.raw_input == "-" {
        rows.push("ー", KANA_WEIGHT, CandidateSource::Generated);
        rows.push("-", KATAKANA_WEIGHT, CandidateSource::Generated);
        rows.finish_into(destination);
        return;
    }
    convert_romaji_into(&request.raw_input, &mut provider.conversion);
    let dictionary = provider.dictionary();
    let conversion = &provider.conversion;
    hiragana_to_katakana_into(&conversion.hiragana, &mut provider.katakana);
    let kana_first = is_single_kana_conversion(conversion);
    if kana_first {
        rows.push_kana(&conversion.hiragana, &provider.katakana);
    }

    if let Some(dictionary) = dictionary {
        let mut predictions = Vec::new();
        if !conversion.hiragana.is_empty() && !conversion.pending.is_empty() {
            let pending_kana = kana_for_romaji_prefix_view(&conversion.pending);
            rows.reserve(
                pending_kana
                    .len()
                    .saturating_mul(PENDING_PREFIX_LEMMAS)
                    .saturating_add(SENTENCE_LIMIT + 1),
            );
            for kana in pending_kana {
                let prefix = join_reading(&conversion.hiragana, kana);
                for lemma in dictionary.prefix_lemma_views(&prefix, PENDING_PREFIX_LEMMAS) {
                    rows.push(
                        lemma.surface,
                        PREFIX_LEMMA_BASE - i64::from(lemma.word_cost),
                        CandidateSource::Database,
                    );
                }
            }
        } else if conversion.pending.is_empty()
            && conversion.hiragana.len() >= MIN_PREFIX_READING_BYTES
        {
            predictions =
                dictionary.prefix_lemma_views(&conversion.hiragana, READING_PREFIX_LEMMAS);
        }
        rows.reserve(predictions.len() + SENTENCE_LIMIT + 1);
        for sentence in search_converted(&dictionary, conversion, SENTENCE_LIMIT) {
            rows.push(
                &sentence.text,
                SENTENCE_BASE - sentence.cost,
                CandidateSource::Database,
            );
        }
        for lemma in predictions {
            rows.push(
                lemma.surface,
                PREFIX_LEMMA_BASE - i64::from(lemma.word_cost),
                CandidateSource::Database,
            );
        }
    }

    if !conversion.hiragana.is_empty() && !kana_first {
        rows.push_kana(&conversion.hiragana, &provider.katakana);
        if conversion.pending.is_empty() {
            rows.promote(&conversion.hiragana, KANA_SLOT);
        }
    }

    if let Some(dynamic) = provider.dynamic.get_ref(&request.raw_input) {
        let mut insertion = rows.used.min(if kana_first { 2 } else { 1 });
        for item in dynamic {
            if rows.contains_word(&item.word) {
                continue;
            }
            if let Some(target) = rows.items.get_mut(rows.used) {
                target.pinyin.clone_from(&item.pinyin);
                target.canonical_pinyin.clone_from(&item.canonical_pinyin);
                target.word.clone_from(&item.word);
                target.weight = item.weight;
                target.source = item.source;
                target.scheme = item.scheme;
                target.fixed_position = item.fixed_position;
                target.fuzzy = item.fuzzy;
                target.corrected_from.clone_from(&item.corrected_from);
                target.sentence_association = item.sentence_association;
                target.sentence_words.clone_from(&item.sentence_words);
            } else {
                rows.items.push(item.clone());
            }
            rows.used += 1;
            rows.items[insertion..rows.used].rotate_right(1);
            insertion += 1;
        }
    }
    rows.finish_into(destination);
}

#[test]
fn provider_matches_old_pending_keys_across_edits_and_dynamic_rows() {
    let (_root, mut actual_provider) = provider_with(Some(test_model::bytes(
        &[
            ("か", "仮", 0, 0, -20),
            ("かか", "仮仮", 0, 0, -500),
            ("かきゃ", "仮甲", 0, 0, -500),
            ("かくぁ", "仮乙", 0, 0, 5),
            ("かし", "仮指", 0, 0, -10),
            ("かしゃ", "仮写", 0, 0, -10),
            ("かじゃ", "仮蛇", 0, 0, -30),
            ("かぢゃ", "仮地", 0, 0, -30),
            ("かっ", "仮促", 0, 0, 15),
            ("しし", "仮指", 0, 0, 20),
        ],
        1,
        &[0],
    )));
    let mut reference_provider = JapaneseProvider::new(&actual_provider.model);
    assert!(
        actual_provider.dictionary().is_some(),
        "合成词库必须成功加载"
    );
    assert!(
        reference_provider.dictionary().is_some(),
        "对照词库必须成功加载"
    );
    for provider in [&mut actual_provider, &mut reference_provider] {
        for input in ["kak", "kash", "kaxt", "kaq", "k", "sis"] {
            provider.cache_dynamic_candidate(input, "合成候補", CandidateSource::CloudSuggestion);
        }
    }
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    let long = format!("{}k", "ka".repeat(24));
    for input in [
        "kak", "KAK", "kash", "kaxt", "kaq", "k", "K", "ka", "-", "", "sis", "kaz", "kad", "kat",
        "kaあ", &long,
    ] {
        let request = request(input);
        actual_provider.query_into(&request, &mut actual);
        reference_query_into(&mut reference_provider, &request, &mut expected);
        assert_eq!(actual, expected, "合成输入长度 {}", input.len());
        actual_provider.query_into(&request, &mut actual);
        reference_query_into(&mut reference_provider, &request, &mut expected);
        let (_, old_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            reference_query_into(&mut reference_provider, &request, &mut expected)
        });
        let (_, new_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            actual_provider.query_into(&request, &mut actual)
        });
        assert_eq!(actual, expected);
        let conversion = super::super::super::romaji::convert_romaji(&request.raw_input);
        let saved = if conversion.hiragana.is_empty() || conversion.pending.is_empty() {
            0
        } else {
            kana_for_romaji_prefix_view(&conversion.pending)
                .len()
                .saturating_sub(1)
        };
        assert_eq!(
            new_allocations + saved,
            old_allocations,
            "provider 键分配差值"
        );
        for rows in [&mut actual, &mut expected] {
            for item in rows.iter_mut() {
                item.scheme = SchemeType::JapaneseRomaji;
                item.fixed_position = 7;
                item.fuzzy = true;
                item.corrected_from = "synthetic".to_owned();
                item.sentence_association = true;
                item.sentence_words = vec!["synthetic".to_owned()];
            }
        }
    }
}

#[test]
#[ignore = "本地 release 对照，只隔离 provider 拼接键；不设置 CI 时间阈值"]
fn benchmark_provider_pending_reading_keys() {
    use std::hint::black_box;
    use std::time::Instant;
    let (_root, mut actual_provider) = provider_with(Some(test_model::bytes(
        &[
            ("かか", "仮仮", 0, 0, 500),
            ("かき", "仮木", 0, 0, 600),
            ("かっ", "仮促", 0, 0, 700),
        ],
        1,
        &[0],
    )));
    let mut reference_provider = JapaneseProvider::new(&actual_provider.model);
    for input in ["kak", "kaxt", "kar", "kaq", "ka"] {
        let request = request(input);
        let mut actual = actual_provider.query(&request);
        let mut expected = Vec::new();
        reference_query_into(&mut reference_provider, &request, &mut expected);
        assert_eq!(actual, expected);
        let (_, old_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            reference_query_into(&mut reference_provider, &request, &mut expected)
        });
        let (_, new_allocations) = crate::ime::personal_rerank::allocations::count(|| {
            actual_provider.query_into(&request, &mut actual)
        });
        let mut old_batches = Vec::new();
        let mut new_batches = Vec::new();
        for batch in 0..10 {
            let mut old = || {
                let start = Instant::now();
                for _ in 0..1000 {
                    reference_query_into(
                        &mut reference_provider,
                        black_box(&request),
                        black_box(&mut expected),
                    );
                }
                old_batches.push(start.elapsed());
            };
            let mut new = || {
                let start = Instant::now();
                for _ in 0..1000 {
                    actual_provider.query_into(black_box(&request), black_box(&mut actual));
                }
                new_batches.push(start.elapsed());
            };
            if batch % 2 == 0 {
                old();
                new();
            } else {
                new();
                old();
            }
        }
        assert_eq!(actual, expected);
        old_batches.sort_unstable();
        new_batches.sort_unstable();
        eprintln!("provider {input}：分配 {old_allocations}→{new_allocations}；中位批次 {:?}→{:?}/1000次（矩阵两边均为新实现）", old_batches[5], new_batches[5]);
    }
}
