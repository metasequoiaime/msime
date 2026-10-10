//! 固定基线 f58d02672 的拼接查询对照，保留后缀遍历与排名逻辑。

use super::*;

#[test]
fn continuing_views_stream_in_order_without_the_result_vector() {
    let surfaces: Vec<_> = (0..70)
        .map(|index| format!("合成語{}", index % 7))
        .collect();
    let mut entries = vec![
        ("か", "仮", 0, 1, -100),
        ("かない", "仮内", 1, 0, -20),
        ("かに", "仮荷", 0, 0, 30),
        ("かん", "仮漢", 1, 1, 10),
        ("甲😀か", "仮長", 0, 1, -40),
        ("甲😀か😀", "仮長続", 1, 0, -40),
    ];
    entries.extend(
        surfaces
            .iter()
            .enumerate()
            .map(|(index, surface)| ("かな", surface.as_str(), 0, 1, index as i32 % 9 - 8)),
    );
    let dictionary = dictionary(&entries, 2);
    for prefix in ["", "か", "かな", "甲😀", "未知"] {
        for next in [
            &[][..],
            &[""][..],
            &["な", "ない", "ん"][..],
            &["な", "な"][..],
            &["に", "な"][..],
            &["な", "", "ない"][..],
            &["😀", "か"][..],
            &["未"][..],
        ] {
            for limit in [0, 1, 2, 24, 48, 64, 65, 70, 100] {
                let (expected, view_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        dictionary.continuing_lemma_views(prefix, next, limit)
                    });
                let mut actual = Vec::with_capacity(expected.len());
                let ((), streamed_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        dictionary.for_each_continuing_lemma_view(prefix, next, limit, |view| {
                            actual.push(view);
                        });
                    });
                assert_eq!(actual, expected, "prefix={prefix}, limit={limit}");
                assert_eq!(
                    streamed_allocations + usize::from(!expected.is_empty()),
                    view_allocations,
                    "只减少命中时的视图结果向量：prefix={prefix}, limit={limit}"
                );
            }
        }
    }
    let saved = {
        let prefix = String::from("か");
        let next = String::from("な");
        let mut saved = None;
        dictionary.for_each_continuing_lemma_view(&prefix, &[next.as_str()], 1, |view| {
            saved = Some(view);
        });
        saved.expect("合成查询必须命中")
    };
    // 临时前缀与后缀已释放，词条视图仍只借用词库。
    assert_eq!(
        saved,
        dictionary.continuing_lemma_views("か", &["な"], 1)[0]
    );
}

fn dictionary(entries: &[test_model::Entry<'_>], size: u32) -> JapaneseDictionary {
    let mut entries = entries.to_vec();
    entries.sort_by_key(|entry| entry.0);
    JapaneseDictionary::from_bytes(
        test_model::bytes(&entries, size, &vec![0; (size * size) as usize]).into_boxed_slice(),
    )
    .expect("synthetic model")
}

fn reference_lower_bound(dictionary: &JapaneseDictionary, reading: &str) -> usize {
    let (mut first, mut last) = (0, dictionary.token_count);
    while first < last {
        let middle = first + (last - first) / 2;
        if dictionary.reading(&dictionary.token_at(middle)) < reading {
            first = middle + 1;
        } else {
            last = middle;
        }
    }
    first
}

fn reference_query<'a>(
    dictionary: &'a JapaneseDictionary,
    prefix: &str,
    next_kana: &[&str],
    limit: usize,
) -> Vec<JapaneseLemmaRef<'a>> {
    if prefix.is_empty() || next_kana.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut best: BinaryHeap<((i32, u32), u32)> = BinaryHeap::new();
    let mut consider = |id: u32| {
        let key = (dictionary.cost_of(id), id);
        if best.len() < limit {
            if best.is_empty() {
                best.reserve_exact(limit);
            }
            best.push((key, id));
        } else if key < best.peek().expect("non-empty bounded heap").0 {
            best.pop();
            best.push((key, id));
        }
    };
    for kana in next_kana {
        if kana.is_empty() {
            for index in reference_lower_bound(dictionary, prefix)..dictionary.token_count {
                let reading = dictionary.reading(&dictionary.token_at(index));
                let Some(remaining) = reading.strip_prefix(prefix) else {
                    break;
                };
                if !remaining.is_empty() {
                    consider(index as u32);
                }
            }
            continue;
        }
        let mut query = String::with_capacity(prefix.len() + kana.len());
        query.push_str(prefix);
        query.push_str(kana);
        let start = reference_lower_bound(dictionary, &query);
        for index in start..dictionary.token_count {
            if !dictionary
                .reading(&dictionary.token_at(index))
                .starts_with(&query)
            {
                break;
            }
            consider(index as u32);
        }
    }
    let mut ids: Vec<u32> = best.into_iter().map(|(_, id)| id).collect();
    ids.sort_unstable_by_key(|id| (dictionary.cost_of(*id), *id));
    ids.into_iter().map(|id| dictionary.lemma_ref(id)).collect()
}

#[test]
fn continuing_queries_match_concatenated_reference() {
    let readings = [
        "a",
        "aa",
        "ab",
        "é",
        "éb",
        "あ",
        "か",
        "かな",
        "かなか",
        "かに",
        "か😀",
        "か😀あ",
        "漢",
        "漢字",
        "😀",
        "😀か",
        "🀄",
    ];
    let suffix_sets: &[&[&str]] = &[
        &[],
        &[""],
        &["な"],
        &["ん", "に", "な"],
        &["な", "なか", "な"],
        &["", "に", ""],
        &["😀", "あ", "字", "b"],
        &["ん", "q", "🀄"],
    ];
    for seed in 0..24_u32 {
        let words: Vec<_> = (0..80)
            .map(|index| format!("合成{seed:02}-{index:02}"))
            .collect();
        let entries: Vec<_> = words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                let value = (index as u32).wrapping_mul(37).wrapping_add(seed * 13);
                (
                    readings[value as usize % readings.len()],
                    word.as_str(),
                    (index % 3) as u16,
                    ((index + 1) % 3) as u16,
                    (value % 11) as i32 - 7,
                )
            })
            .collect();
        let dictionary = dictionary(&entries, 3);
        for prefix in [
            "", "a", "ab", "é", "あ", "か", "かな", "か😀", "漢", "😀", "z",
        ] {
            for suffixes in suffix_sets {
                for limit in [0, 1, 2, 8, 24, 64, 80, 200] {
                    assert_eq!(
                        dictionary.continuing_lemma_views(prefix, suffixes, limit),
                        reference_query(&dictionary, prefix, suffixes, limit),
                        "seed={seed}, prefix={prefix:?}, suffixes={suffixes:?}, limit={limit}"
                    );
                }
            }
        }
    }
}

#[test]
fn multiple_missing_suffixes_share_one_query_key() {
    let dictionary = dictionary(&[("かな", "仮名", 0, 0, 500)], 1);
    for suffixes in [&["ん", "q", "😀"][..], &["", "ん", "q"][..]] {
        let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            dictionary.continuing_lemma_views("かな", suffixes, 24)
        });
        assert!(actual.is_empty());
        eprintln!("日文多后缀未命中查询键分配：{allocations}");
        assert_eq!(allocations, 1, "多个非空后缀应只分配一份拼接键");
    }
}

#[test]
fn concatenated_reference_matches_original_lower_bound() {
    let dictionary = dictionary(
        &[
            ("a", "甲", 0, 0, 0),
            ("é", "乙", 0, 0, 0),
            ("あ", "丙", 0, 0, 0),
            ("か😀", "丁", 0, 0, 0),
            ("😀", "戊", 0, 0, 0),
        ],
        1,
    );
    for query in [
        "", "a", "aa", "é", "あ", "か", "か😀", "漢", "😀", "😀あ", "🀄",
    ] {
        assert_eq!(
            dictionary.lower_bound(query),
            reference_lower_bound(&dictionary, query)
        );
    }
}

#[test]
#[ignore = "本地比较逐后缀拼接与查询内单缓冲的耗时"]
fn continuing_query_timing_against_concatenated_reference() {
    use std::hint::black_box;
    use std::time::Instant;
    for count in [16, 1024] {
        let readings: Vec<_> = (0..count)
            .map(|index| format!("かな{:04}", index % 64))
            .collect();
        let words: Vec<_> = (0..count).map(|index| format!("合成{index:04}")).collect();
        let entries: Vec<_> = readings
            .iter()
            .zip(&words)
            .enumerate()
            .map(|(index, (reading, word))| {
                (reading.as_str(), word.as_str(), 0, 0, index as i32 % 17 - 8)
            })
            .collect();
        let dictionary = dictionary(&entries, 1);
        for (label, suffixes) in [
            ("命中", &["ん", "な0", "な00", "な01"][..]),
            ("未命中", &["ん", "😀", "q", "漢"][..]),
            ("单后缀命中", &["な00"][..]),
            ("单后缀未命中", &["ん"][..]),
        ] {
            assert_eq!(
                dictionary.continuing_lemma_views("か", suffixes, 24),
                reference_query(&dictionary, "か", suffixes, 24)
            );
            let mut original = Vec::new();
            let mut borrowed = Vec::new();
            for round in 0..10 {
                let mut measure = |reference: bool| {
                    let start = Instant::now();
                    for _ in 0..1000 {
                        if reference {
                            black_box(reference_query(
                                black_box(&dictionary),
                                black_box("か"),
                                black_box(suffixes),
                                black_box(24),
                            ));
                        } else {
                            black_box(dictionary.continuing_lemma_views(
                                black_box("か"),
                                black_box(suffixes),
                                black_box(24),
                            ));
                        }
                    }
                    if reference {
                        original.push(start.elapsed());
                    } else {
                        borrowed.push(start.elapsed());
                    }
                };
                measure(round % 2 == 0);
                measure(round % 2 != 0);
            }
            original.sort();
            borrowed.sort();
            eprintln!("日文 continuing {count} 条合成词库{label}：原拼接 {:?}，单缓冲 {:?}（每批1000次，中位批次）", original[5], borrowed[5]);
        }
    }
}

#[test]
fn query_key_capacity_covers_all_suffix_orders_and_long_utf8_prefixes() {
    let dictionary = dictionary(&[("かな", "仮名", 0, 0, 500)], 1);
    let long_prefix = "漢😀".repeat(512);
    let long_suffix = "😀".repeat(512);
    for prefix in ["かに", "é", long_prefix.as_str()] {
        for suffixes in [
            vec!["q", long_suffix.as_str(), "", "😀"],
            vec![long_suffix.as_str(), "q", "😀", ""],
            vec!["", "", ""],
        ] {
            for limit in [0, 1, 24] {
                let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    dictionary.continuing_lemma_views(prefix, &suffixes, limit)
                });
                assert!(actual.is_empty());
                assert_eq!(
                    actual,
                    reference_query(&dictionary, prefix, &suffixes, limit)
                );
                assert_eq!(
                    allocations,
                    usize::from(limit != 0 && suffixes.iter().any(|suffix| !suffix.is_empty()))
                );
            }
        }
    }
}
