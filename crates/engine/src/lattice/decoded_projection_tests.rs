use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::lattice::merge::block_buffer_tests::favouring_model;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[path = "decoded_projection_frozen.rs"]
mod frozen;

fn decoded(count: usize, typo: bool) -> DecodedSentences {
    let paths = (0..count)
        .map(|index| SentencePath {
            sentence: ["阿吧嚓", "啊吧嚓", "吧阿嚓", "嚓吧阿"][index % 4].into(),
            key: "a'ba'ca".into(),
            log_prob: -1.25 - index as f64,
            words: vec!["阿".into(), "吧嚓".into()],
            typo_edges: 0,
        })
        .collect();
    DecodedSentences {
        paths,
        typo: typo.then(|| TypoSentence {
            sentence: "啊吧嚓".into(),
            key: "a'ba'ca".into(),
            score: -1.5,
            words: vec!["啊".into(), "吧嚓".into()],
            edges: 1,
            literal_score: -2.5,
        }),
    }
}

fn existing(shape: usize) -> Vec<WordItem> {
    let words: &[&str] = match shape {
        0 => &[],
        1 => &["阿吧嚓"],
        _ => &["阿吧嚓", "啊吧嚓", "吧阿嚓", "嚓吧阿"],
    };
    let mut rows: Vec<_> = words
        .iter()
        .map(|word| WordItem::new("abaca", *word, 1, CandidateSource::Database, "a'ba'ca"))
        .collect();
    if shape == 3 {
        rows.extend((0..65).map(|index| {
            WordItem::new(
                "abaca",
                format!("合成占位{index}"),
                1,
                CandidateSource::CloudSuggestion,
                "",
            )
        }));
    }
    rows
}

fn models(mode: usize) -> Vec<NeuralReranker> {
    if mode == 0 {
        return vec![];
    }
    let model = Arc::new(favouring_model(&['阿', '啊', '吧', '嚓'], &['啊']));
    let sources: &[CandidateSource] = match mode {
        1 => &[CandidateSource::Generated],
        2 => &[CandidateSource::NeuralKeyboard],
        _ => &[
            CandidateSource::NeuralKeyboard,
            CandidateSource::Generated,
            CandidateSource::NeuralKeyboard,
        ],
    };
    sources
        .iter()
        .map(|source| NeuralReranker::new(*source, model.clone()))
        .collect()
}

fn pointers(decoded: &DecodedSentences) -> Vec<usize> {
    let mut values = vec![decoded.paths.as_ptr() as usize, decoded.paths.capacity()];
    for path in &decoded.paths {
        values.extend([
            path.sentence.as_ptr() as usize,
            path.sentence.capacity(),
            path.key.as_ptr() as usize,
            path.key.capacity(),
            path.words.as_ptr() as usize,
            path.words.capacity(),
            path.log_prob.to_bits() as usize,
        ]);
        for word in &path.words {
            values.extend([word.as_ptr() as usize, word.capacity()]);
        }
    }
    values
}

#[test]
fn cached_and_owned_projection_match_the_frozen_merge() {
    let syllables: Vec<String> = ["a", "ba", "ca"].map(str::to_owned).into();
    for paths in [0, 1, 2, 6, 12, 65] {
        for shape in 0..4 {
            for emit in [0, 1, 2] {
                for flags in 0..8 {
                    for mode in 0..4 {
                        for typo in [false, true] {
                            let cached = decoded(paths, typo);
                            let before = cached.clone();
                            let addresses = pointers(&cached);
                            let input = existing(shape);
                            let options = LatticeOptions {
                                emit,
                                include_lattice_best: flags & 1 != 0,
                                include_collocation_best: flags & 2 != 0,
                                show_next_on_duplicate: flags & 4 != 0,
                                ..LatticeOptions::default()
                            };
                            for borrowed in [false, true] {
                                let mut expected = input.clone();
                                let mut actual = input.clone();
                                let mut old_models = models(mode);
                                let mut new_models = models(mode);
                                let original = cached.clone();
                                let owned = cached.clone();
                                let (expected_typo, old_count) = count(|| {
                                    frozen::merge_decoded(
                                        &mut expected,
                                        &syllables,
                                        "abaca",
                                        &options,
                                        &mut old_models,
                                        "合成上下文",
                                        if borrowed { cached.clone() } else { original },
                                    )
                                });
                                let (actual_typo, new_count) = count(|| {
                                    if borrowed {
                                        merge_decoded_borrowed(
                                            &mut actual,
                                            &syllables,
                                            "abaca",
                                            &options,
                                            &mut new_models,
                                            "合成上下文",
                                            &cached,
                                        )
                                    } else {
                                        merge_decoded(
                                            &mut actual,
                                            &syllables,
                                            "abaca",
                                            &options,
                                            &mut new_models,
                                            "合成上下文",
                                            owned,
                                        )
                                    }
                                });
                                assert_eq!(
                                    actual, expected,
                                    "{paths}/{shape}/{emit}/{flags}/{mode}/{typo}/{borrowed}"
                                );
                                assert_eq!(actual.capacity(), expected.capacity());
                                for (new, old) in actual.iter().zip(&expected) {
                                    assert_eq!(new.word.capacity(), old.word.capacity());
                                    assert_eq!(new.pinyin.capacity(), old.pinyin.capacity());
                                    assert_eq!(
                                        new.canonical_pinyin.capacity(),
                                        old.canonical_pinyin.capacity()
                                    );
                                    assert_eq!(
                                        new.sentence_words.capacity(),
                                        old.sentence_words.capacity()
                                    );
                                    for (new, old) in
                                        new.sentence_words.iter().zip(&old.sentence_words)
                                    {
                                        assert_eq!(new.capacity(), old.capacity());
                                    }
                                }
                                assert_eq!(actual_typo, expected_typo);
                                assert!(new_count <= old_count, "{paths}/{shape}/{emit}/{flags}/{mode}/{typo}/{borrowed}: {new_count}>{old_count}");
                                if !borrowed {
                                    assert_eq!(new_count, old_count, "拥有路径不增加申请");
                                }
                                if let Some(row) = actual.first_mut() {
                                    row.word.push_str("独立输出");
                                    row.pinyin.clear();
                                    row.canonical_pinyin.clear();
                                    row.sentence_words.clear();
                                }
                                assert_eq!(cached, before);
                                assert_eq!(pointers(&cached), addresses);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn timed(
    repetitions: usize,
    frozen: bool,
    borrowed: bool,
    cached: &DecodedSentences,
    input: &[WordItem],
    options: &LatticeOptions<'_>,
    models: &mut [NeuralReranker],
) -> Duration {
    let syllables: Vec<String> = ["a", "ba", "ca"].map(str::to_owned).into();
    let start = Instant::now();
    for _ in 0..repetitions {
        let mut candidates = black_box(input).to_vec();
        let typo = if frozen {
            frozen::merge_decoded(
                &mut candidates,
                &syllables,
                "abaca",
                black_box(options),
                models,
                "合成上下文",
                black_box(cached).clone(),
            )
        } else if borrowed {
            merge_decoded_borrowed(
                &mut candidates,
                &syllables,
                "abaca",
                black_box(options),
                models,
                "合成上下文",
                black_box(cached),
            )
        } else {
            merge_decoded(
                &mut candidates,
                &syllables,
                "abaca",
                black_box(options),
                models,
                "合成上下文",
                black_box(cached).clone(),
            )
        };
        black_box((candidates, typo));
    }
    start.elapsed()
}

#[test]
#[ignore = "普通系统分配器下手动运行的交替配对计时"]
fn cached_projection_paired_latency() {
    let mut failures = Vec::new();
    println!("pair,paths,shape,emit,mode,borrowed,repetitions,old_ns,new_ns,ratio");
    for paths in [0, 1, 6, 12] {
        for shape in [0, 2, 3] {
            for emit in [0, 1] {
                for mode in [0, 2] {
                    for borrowed in [false, true] {
                        let cached = decoded(paths, false);
                        let input = existing(shape);
                        let options = LatticeOptions {
                            emit,
                            ..LatticeOptions::default()
                        };
                        let mut old_models = models(mode);
                        let mut new_models = models(mode);
                        let mut repetitions = 1;
                        loop {
                            let old = timed(
                                repetitions,
                                true,
                                borrowed,
                                &cached,
                                &input,
                                &options,
                                &mut old_models,
                            );
                            let new = timed(
                                repetitions,
                                false,
                                borrowed,
                                &cached,
                                &input,
                                &options,
                                &mut new_models,
                            );
                            if old.min(new) >= Duration::from_millis(5) {
                                break;
                            }
                            repetitions *= 2;
                        }
                        let mut ratios = Vec::new();
                        for pair in 0..21 {
                            let (old, new) = if pair % 2 == 0 {
                                let old = timed(
                                    repetitions,
                                    true,
                                    borrowed,
                                    &cached,
                                    &input,
                                    &options,
                                    &mut old_models,
                                );
                                let new = timed(
                                    repetitions,
                                    false,
                                    borrowed,
                                    &cached,
                                    &input,
                                    &options,
                                    &mut new_models,
                                );
                                (old, new)
                            } else {
                                let new = timed(
                                    repetitions,
                                    false,
                                    borrowed,
                                    &cached,
                                    &input,
                                    &options,
                                    &mut new_models,
                                );
                                let old = timed(
                                    repetitions,
                                    true,
                                    borrowed,
                                    &cached,
                                    &input,
                                    &options,
                                    &mut old_models,
                                );
                                (old, new)
                            };
                            let ratio = new.as_secs_f64() / old.as_secs_f64();
                            ratios.push(ratio);
                            println!(
                            "{pair},{paths},{shape},{emit},{mode},{borrowed},{repetitions},{},{},{ratio:.6}",
                            old.as_nanos(),
                            new.as_nanos()
                        );
                        }
                        ratios.sort_by(f64::total_cmp);
                        let target =
                            borrowed && paths > 1 && (shape > 0 || (mode == 0 && emit == 1));
                        let limit = if target { 0.98 } else { 1.05 };
                        let median = ratios[ratios.len() / 2];
                        if median >= limit {
                            failures.push((paths, shape, emit, mode, borrowed, median, limit));
                        }
                        eprintln!("summary {paths}/{shape}/{emit}/{mode}/{borrowed}: {median:.6}, limit={limit}");
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "配对门禁失败：{failures:?}");
}
