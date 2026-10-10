use super::block_frozen::*;
use super::*;
use crate::ime::personal_rerank::allocations::measure;
use crate::lattice::decode::tests::{lookup, syllables, table};

fn path(sentence: &str, score: f64) -> SentencePath {
    SentencePath {
        sentence: sentence.into(),
        key: "a'ba".into(),
        log_prob: score,
        words: vec![sentence.into()],
        typo_edges: 0,
    }
}

fn existing(words: &[&str]) -> Vec<WordItem> {
    words
        .iter()
        .map(|word| WordItem::new("aba", *word, 123, CandidateSource::Database, "a'ba"))
        .collect()
}

#[test]
fn generated_empty_block_keeps_no_heap_storage() {
    let paths = [path("甲乙", -1.1), path("甲丙", -2.2)];
    let candidates = existing(&["甲乙", "甲丙"]);
    let (old, before) = measure(|| frozen_generated_block_linear(&candidates, &paths, "aba"));
    let (new, after) = measure(|| generated_block_linear(&candidates, &paths, "aba"));
    assert_eq!(new, old);
    assert!(new.is_empty());
    assert_eq!(before.allocations, 1);
    assert_eq!(
        before.remaining_bytes,
        (old.capacity() * size_of::<WordItem>()) as i128
    );
    assert_eq!(after.allocations, 0);
    assert_eq!(after.peak_bytes, 0);
    assert_eq!(after.remaining_bytes, 0);
    assert_eq!(new.capacity(), 0);
    eprintln!(
        "合成两路径空输出块 分配 {}→{} 返回存储 {}→{}",
        before.allocations, after.allocations, before.remaining_bytes, after.remaining_bytes
    );
}

#[test]
fn generated_nonempty_blocks_preserve_fields_capacity_and_allocations() {
    for count in [0, 1, 2, 5, 64, 65, 128] {
        let paths: Vec<_> = (0..count)
            .map(|index| path(&format!("合成{}", index % 5), -1.2345 - index as f64))
            .collect();
        for words in [
            &[][..],
            &["合成0"][..],
            &["合成0", "合成1", "合成2", "合成3", "合成4"][..],
        ] {
            let candidates = existing(words);
            let (old, before) =
                measure(|| frozen_generated_block_linear(&candidates, &paths, "aba"));
            let (new, after) = measure(|| generated_block_linear(&candidates, &paths, "aba"));
            assert_eq!(new, old);
            if new.is_empty() {
                assert_eq!(new.capacity(), 0);
                assert_eq!(after.allocations, 0);
            } else {
                assert_eq!(new.capacity(), old.capacity());
                assert_eq!(after.allocations, before.allocations);
                assert_eq!(after.remaining_bytes, before.remaining_bytes);
            }
        }
    }
}

#[test]
fn reranked_blocks_preserve_options_and_allocate_only_for_rows() {
    let paths = [
        path("甲乙", -1.2345),
        path("甲丙", -2.3456),
        path("甲丙", -9.0),
    ];
    let keyboard = [path("甲丙", -2.3456), path("甲乙", -1.2345)];
    for ranked in [&[][..], &paths[..]] {
        for pick in [None, Some(&[][..]), Some(&paths[..]), Some(&keyboard[..])] {
            for include_lattice_best in [false, true] {
                for show_next_on_duplicate in [false, true] {
                    let options = LatticeOptions {
                        include_lattice_best,
                        show_next_on_duplicate,
                        ..LatticeOptions::default()
                    };
                    for words in [&[][..], &["甲乙"][..], &["甲乙", "甲丙"][..]] {
                        let candidates = existing(words);
                        let (old, before) = measure(|| {
                            frozen_reranked_block_linear(&candidates, ranked, pick, &options, "aba")
                        });
                        let (new, after) = measure(|| {
                            reranked_block_linear(&candidates, ranked, pick, &options, "aba")
                        });
                        assert_block_equivalent(&old, &new, before, after);
                        let mut old_seen: HashSet<_> = words.iter().copied().collect();
                        let mut new_seen = old_seen.clone();
                        // 两个集合都在区间前创建；新增借用键不释放已有输入存储。
                        old_seen.reserve(2);
                        new_seen.reserve(2);
                        let (old, before) = measure(|| {
                            frozen_reranked_block(ranked, pick, &options, "aba", &mut old_seen)
                        });
                        let (new, after) = measure(|| {
                            reranked_block(ranked, pick, &options, "aba", &mut new_seen)
                        });
                        assert_eq!(old_seen, new_seen);
                        assert_block_equivalent(&old, &new, before, after);
                    }
                }
            }
        }
    }
}

fn assert_block_equivalent(
    old: &Vec<WordItem>,
    new: &Vec<WordItem>,
    before: crate::ime::personal_rerank::allocations::Measurement,
    after: crate::ime::personal_rerank::allocations::Measurement,
) {
    assert_eq!(new, old);
    if new.is_empty() {
        assert_eq!(new.capacity(), 0);
        assert_eq!(before.allocations, 1);
        assert_eq!(after.allocations, 0);
        assert_eq!(after.peak_bytes, 0);
        assert_eq!(after.remaining_bytes, 0);
    } else {
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(after.allocations, before.allocations);
        assert_eq!(after.remaining_bytes, before.remaining_bytes);
    }
}

#[test]
fn real_merge_all_duplicates_saves_one_allocation_across_dedup_boundary() {
    let rows = table(&[("a", &[("阿", 1000), ("啊", 500)]), ("ba", &[("吧", 1000)])]);
    let keys = syllables("a'ba");
    for filler_count in [0, 61, 62, 63, 64, 65] {
        for emit in [0, 1] {
            let options = LatticeOptions {
                emit,
                ..LatticeOptions::default()
            };
            let mut input = existing(&["阿吧", "啊吧"]);
            input.extend((0..filler_count).map(|index| {
                WordItem::new(
                    "aba",
                    format!("占位{index}"),
                    1,
                    CandidateSource::CloudSuggestion,
                    "",
                )
            }));
            let run = |frozen: bool| {
                let mut candidates = input.clone();
                let mut dictionary = lookup(&rows);
                let merge = if frozen {
                    frozen_merge_lattice_candidates
                } else {
                    merge_lattice_candidates
                };
                let typo = merge(
                    &mut candidates,
                    &keys,
                    &mut dictionary,
                    "aba",
                    &options,
                    None,
                    &mut [],
                    "",
                );
                (candidates, typo)
            };
            // 完整拼音检测会初始化全局表，先热身两边，区间内只比较同一合并操作。
            assert_eq!(run(true), run(false));
            let (old, before) = measure(|| run(true));
            let (new, after) = measure(|| run(false));
            assert_eq!(new, old);
            assert_eq!(new.0.capacity(), old.0.capacity());
            assert_eq!(new.0, input);
            assert!(new.1.is_none());
            assert_eq!(
                after.allocations + 1,
                before.allocations,
                "filler={filler_count}, emit={emit}"
            );
            assert_eq!(after.remaining_bytes, before.remaining_bytes);
            assert_eq!(after.minimum_bytes, 0);
            eprintln!(
                "合成全重复合并 filler={filler_count} emit={emit} 分配 {}→{} 请求峰值 {}→{}",
                before.allocations, after.allocations, before.peak_bytes, after.peak_bytes
            );
        }
    }
}

#[test]
fn real_merge_preserves_nonempty_and_neural_rows_with_typo_return() {
    use crate::lattice::decode::TypoEdge;
    use std::sync::Arc;

    let rows = table(&[
        ("a", &[("阿", 1000), ("啊", 500)]),
        ("ba", &[("吧", 1000)]),
        ("ca", &[("嚓", 1000)]),
    ]);
    let keys = syllables("a'ba'ca");
    let model = Arc::new(favouring_model(&['阿', '啊', '吧', '嚓'], &['啊']));
    // 先独立证实模型确实重排，而不是仅靠非空 rerankers 进入分支。
    let mut probe = NeuralReranker::new(CandidateSource::NeuralKeyboard, model.clone());
    let mut probe_paths = [path("阿吧嚓", -1.0), path("啊吧嚓", -2.0)];
    assert!(probe.rerank(&mut probe_paths, ""));
    assert_eq!(probe_paths[0].sentence, "啊吧嚓");

    for filler_count in [0, 61, 62, 63, 65] {
        for words in [&[][..], &["阿吧嚓"][..], &["阿吧嚓", "啊吧嚓"][..]] {
            for neural in [false, true] {
                for include_lattice_best in [false, true] {
                    for show_next_on_duplicate in [false, true] {
                        let options = LatticeOptions {
                            include_lattice_best,
                            show_next_on_duplicate,
                            ..LatticeOptions::default()
                        };
                        let mut input = existing(words);
                        input.extend((0..filler_count).map(|index| {
                            WordItem::new(
                                "abaca",
                                format!("占位{index}"),
                                1,
                                CandidateSource::CloudSuggestion,
                                "",
                            )
                        }));
                        let run = |frozen: bool| {
                            let mut candidates = input.clone();
                            let mut dictionary = lookup(&rows);
                            let mut rerankers = if neural {
                                vec![NeuralReranker::new(
                                    CandidateSource::NeuralKeyboard,
                                    model.clone(),
                                )]
                            } else {
                                Vec::new()
                            };
                            let mut source = |_: &SentencePath| {
                                vec![TypoEdge {
                                    start: 1,
                                    end: 3,
                                    key: "ba'ca".into(),
                                    value: "叭嚓".into(),
                                    weight: 900_000,
                                    penalty: 0.5,
                                }]
                            };
                            let merge = if frozen {
                                frozen_merge_lattice_candidates
                            } else {
                                merge_lattice_candidates
                            };
                            let typo = merge(
                                &mut candidates,
                                &keys,
                                &mut dictionary,
                                "abaca",
                                &options,
                                Some(&mut source),
                                &mut rerankers,
                                "",
                            );
                            (candidates, typo)
                        };
                        assert_eq!(run(true), run(false));
                        let (old, before) = measure(|| run(true));
                        let (new, after) = measure(|| run(false));
                        assert_eq!(new, old);
                        assert_eq!(new.0.capacity(), old.0.capacity());
                        let typo = new.1.as_ref().expect("合成纠错边确实参与解码");
                        assert_eq!(typo.edges, 1);
                        assert!(typo.sentence.ends_with("叭嚓"));
                        let empty = new.0 == input;
                        assert_eq!(after.allocations + usize::from(empty), before.allocations);
                        assert_eq!(after.remaining_bytes, before.remaining_bytes);
                        assert_eq!(after.minimum_bytes, 0);
                        if neural
                            && words.len() == 2
                            && include_lattice_best
                            && !show_next_on_duplicate
                        {
                            eprintln!("合成神经全重复合并 filler={filler_count} 分配 {}→{} 请求峰值 {}→{}", before.allocations, after.allocations, before.peak_bytes, after.peak_bytes);
                        }
                    }
                }
            }
        }
    }
}

// 沿用 input-runtime 的合成零层模型，固定字符偏好，不依赖真实模型资源。
fn favouring_model(characters: &[char], favoured: &[char]) -> chinese_ime_lm::SentenceModel {
    const CONTEXT: usize = 16;
    let vocabulary: Vec<String> = ["<pad>", "<unk>", "<bos>"]
        .into_iter()
        .map(str::to_owned)
        .chain(characters.iter().map(char::to_string))
        .collect();
    let logits: Vec<f32> = [0.0, -8.0, 0.0]
        .into_iter()
        .chain(
            characters
                .iter()
                .map(|c| if favoured.contains(c) { 8.0 } else { -8.0 }),
        )
        .collect();
    let config = format!(
        r#"{{"vocab":{},"n_layer":0,"n_head":1,"n_embd":1,"context":{CONTEXT},"dropout":0.0}}"#,
        vocabulary.len()
    );
    let tensors: [(&str, usize, Vec<f32>); 4] = [
        ("tok.weight", vocabulary.len(), logits),
        ("pos.weight", CONTEXT, vec![0.0; CONTEXT]),
        ("ln_f.weight", 1, vec![0.0]),
        ("ln_f.bias", 1, vec![1.0]),
    ];
    let mut header = serde_json::Map::new();
    header.insert(
        "__metadata__".into(),
        serde_json::json!({
            "format": "chinese-ime-lm",
            "version": "1",
            "precision": "f32",
            "config": config,
            "vocab": serde_json::to_string(&vocabulary).unwrap(),
        }),
    );
    let mut data = Vec::new();
    for (name, rows, values) in tensors {
        let start = data.len();
        data.extend(values.iter().flat_map(|value| value.to_le_bytes()));
        header.insert(
            name.into(),
            serde_json::json!({"dtype": "F32", "shape": [rows, 1], "data_offsets": [start, data.len()]}),
        );
    }
    // 一维张量保留真实形状。
    header["ln_f.weight"]["shape"] = serde_json::json!([1]);
    header["ln_f.bias"]["shape"] = serde_json::json!([1]);
    let header = serde_json::to_vec(&serde_json::Value::Object(header)).unwrap();
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(&data);
    chinese_ime_lm::SentenceModel::load(&bytes).unwrap()
}
