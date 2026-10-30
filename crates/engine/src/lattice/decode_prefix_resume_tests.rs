use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::tests::{lookup, row, Table};
use super::*;

/// 可复现的伪随机数（LCG），测试不依赖外部随机库。
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

const SYLLABLES: [&str; 4] = ["ba", "ma", "ni", "hao"];
const WORDS: [&str; 10] = [
    "甲", "乙", "丙", "丁", "吗", "啊啊", "甲乙", "乙丙", "测试", "好",
];

/// 四个音节的全部一到三音节跨度，各给几行；权重只取几个值，好让分数打平、考验平分时谁留下。
fn random_table(rng: &mut Lcg) -> Table {
    let mut table = Table::new();
    let mut spans = vec![Vec::new()];
    for _ in 0..3 {
        let mut longer = Vec::new();
        for span in &spans {
            for syllable in SYLLABLES {
                let mut next: Vec<&str> = span.clone();
                next.push(syllable);
                longer.push(next);
            }
        }
        for span in &longer {
            let key = span.join("'");
            let count = rng.below(4) as usize;
            let rows = (0..count)
                .map(|_| {
                    let word = WORDS[rng.below(WORDS.len() as u64) as usize];
                    let weight = [1, 100, 100, 5_000, 80_000][rng.below(5) as usize];
                    row(&key, word, weight)
                })
                .collect();
            table.insert(key, rows);
        }
        spans = longer;
    }
    table
}

/// 打字时一张接一张的图：多一个音节、换掉末音节、删掉末音节，偶尔整串换掉。
fn syllable_walk(rng: &mut Lcg, steps: usize) -> Vec<Vec<String>> {
    let mut current: Vec<String> = vec!["ni".into(), "hao".into()];
    let mut walk = Vec::with_capacity(steps);
    for _ in 0..steps {
        match rng.below(10) {
            0..=4 => current.push(SYLLABLES[rng.below(4) as usize].into()),
            5..=6 => {
                let last = current.len() - 1;
                current[last] = SYLLABLES[rng.below(4) as usize].into();
            }
            7..=8 if current.len() > 2 => {
                current.pop();
            }
            _ => {
                let length = 2 + rng.below(5) as usize;
                current = (0..length)
                    .map(|_| SYLLABLES[rng.below(4) as usize].to_owned())
                    .collect();
            }
        }
        if current.len() > 9 {
            current.drain(..current.len() - 9);
        }
        walk.push(current.clone());
    }
    walk
}

fn assert_same_paths(resumed: &[SentencePath], fresh: &[SentencePath], context: &str) {
    assert_eq!(resumed.len(), fresh.len(), "{context}");
    for (left, right) in resumed.iter().zip(fresh) {
        assert_eq!(left.sentence, right.sentence, "{context}");
        assert_eq!(left.key, right.key, "{context}");
        assert_eq!(left.words, right.words, "{context}");
        assert_eq!(left.typo_edges, right.typo_edges, "{context}");
        assert_eq!(
            left.log_prob.to_bits(),
            right.log_prob.to_bits(),
            "{context}: {} vs {}",
            left.log_prob,
            right.log_prob
        );
    }
}

/// 接着前缀解出的整句与从头解完全相同：句子、读音、词和分数逐位一致，覆盖束宽、末列宽度、二元表、备忘、三元表、个人模型和搭配打分的各种组合；同时确认确实接上过前缀。
#[test]
fn resumed_decodes_match_fresh_decodes_bit_for_bit() {
    let directory = tempfile::tempdir().unwrap();
    let bigram = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[
            (SENTENCE_START, "甲", 1.0),
            ("甲", "乙", 2.0),
            ("乙", "丙", 0.5),
            ("丙", "吗", 1.5),
            ("测试", "好", 0.75),
            ("甲乙", "丁", -1.0),
        ],
    ));
    let trigram = Arc::new(triple_table(
        directory.path(),
        "triples.bin",
        &[(SENTENCE_START, "甲", "乙", 0.75)],
    ));
    let mut personal = PersonalNgram::default();
    record_run(&mut personal, &["甲", "乙", "测试", "甲乙"], 7);
    let scorer = |tail: &str, word: &str, rear: bool| {
        (tail.chars().count() as f64) * 0.01
            + (word.len() as f64) * 0.02
            + f64::from(u8::from(rear))
    };
    let mut resumed_total = 0;
    for seed in 0..6u64 {
        let mut rng = Lcg(seed + 1);
        let table = random_table(&mut rng);
        let walk = syllable_walk(&mut rng, 60);
        for (beam, nbest) in [(1, 1), (4, 5), (8, 40), (32, 6)] {
            for mode in 0..32 {
                let memo = RefCell::new(BigramMemo::default());
                let fresh_memo = RefCell::new(BigramMemo::default());
                let options = LatticeOptions {
                    beam,
                    nbest,
                    bigram: (mode & 1 != 0).then(|| Arc::clone(&bigram)),
                    trigram: (mode & 2 != 0).then(|| Arc::clone(&trigram)),
                    personal: (mode & 4 != 0).then_some(&personal),
                    bigram_memo: (mode & 8 != 0).then_some(&memo),
                    collocation_scorer: (mode & 16 != 0)
                        .then_some(&scorer as &CollocationScorer<'_>),
                    collocation_weight: if mode & 16 != 0 { 0.5 } else { 0.0 },
                    ..LatticeOptions::default()
                };
                let fresh_options = LatticeOptions {
                    bigram_memo: (mode & 8 != 0).then_some(&fresh_memo),
                    ..options.clone()
                };
                let mut cache = BeamPrefixCache::default();
                for syllables in &walk {
                    let graph = build_graph(syllables, &mut lookup(&table), &options);
                    let fresh = decode_graph(&graph, &fresh_options, None);
                    let mut resume = BeamResume {
                        syllables,
                        cache: &mut cache,
                        personal_version: 1,
                    };
                    let resumed = decode_graph_resuming(&graph, &options, &mut resume);
                    assert_same_paths(
                        &resumed,
                        &fresh,
                        &format!("seed={seed} beam={beam} nbest={nbest} mode={mode} {syllables:?}"),
                    );
                }
                assert!(cache.len() <= BEAM_PREFIX_CAPACITY);
                resumed_total += cache.resumed;
            }
        }
    }
    assert!(resumed_total > 1000, "前缀几乎没有接上：{resumed_total}");
}

/// 打分参数或个人模型版本不同的存档不接：换了二元权重、换了版本号，都从头解。
#[test]
fn a_saved_beam_is_reused_only_under_the_same_scoring() {
    let table = super::tests::table(&[
        ("ni", &[("你", 100), ("泥", 90)]),
        ("hao", &[("好", 100)]),
        ("ma", &[("吗", 80), ("妈", 70)]),
        ("ni'hao", &[("你好", 900)]),
    ]);
    let first: Vec<String> = vec!["ni".into(), "hao".into(), "ni".into()];
    let second: Vec<String> = vec!["ni".into(), "hao".into(), "ma".into()];
    let options = LatticeOptions::default();
    let mut cache = BeamPrefixCache::default();
    let graph = build_graph(&first, &mut lookup(&table), &options);
    decode_graph_resuming(
        &graph,
        &options,
        &mut BeamResume {
            syllables: &first,
            cache: &mut cache,
            personal_version: 1,
        },
    );
    assert_eq!(cache.resumed, 0);

    let reweighted = LatticeOptions {
        bigram_weight: 3.0,
        ..LatticeOptions::default()
    };
    let graph = build_graph(&second, &mut lookup(&table), &reweighted);
    let resumed = decode_graph_resuming(
        &graph,
        &reweighted,
        &mut BeamResume {
            syllables: &second,
            cache: &mut cache,
            personal_version: 1,
        },
    );
    assert_eq!(cache.resumed, 0, "二元权重不同也接上了");
    assert_same_paths(
        &resumed,
        &decode_graph(&graph, &reweighted, None),
        "reweighted",
    );

    let graph = build_graph(&second, &mut lookup(&table), &options);
    decode_graph_resuming(
        &graph,
        &options,
        &mut BeamResume {
            syllables: &second,
            cache: &mut cache,
            personal_version: 2,
        },
    );
    assert_eq!(cache.resumed, 0, "个人模型版本不同也接上了");

    let third: Vec<String> = vec!["ni".into(), "hao".into(), "ma".into(), "ni".into()];
    let graph = build_graph(&third, &mut lookup(&table), &options);
    let resumed = decode_graph_resuming(
        &graph,
        &options,
        &mut BeamResume {
            syllables: &third,
            cache: &mut cache,
            personal_version: 2,
        },
    );
    assert_eq!(cache.resumed, 1, "同样打分、共用前缀的图应当接上");
    assert_same_paths(&resumed, &decode_graph(&graph, &options, None), "third");
}

/// 存档里的边下标在新图里越界或终点对不上时不接，照样从头解。正常情况下调用方在词库行变化时清空缓存，这道核对只是兜底。
#[test]
fn a_saved_beam_whose_edges_do_not_fit_is_not_reused() {
    let before = super::tests::table(&[
        ("ni", &[("你", 100), ("泥", 90), ("尼", 80)]),
        ("hao", &[("好", 100)]),
        ("ma", &[("吗", 80)]),
    ]);
    let after = super::tests::table(&[
        ("ni", &[("你", 100)]),
        ("hao", &[("好", 100)]),
        ("ma", &[("吗", 80)]),
    ]);
    let first: Vec<String> = vec!["ni".into(), "hao".into(), "ma".into()];
    let second: Vec<String> = vec!["ni".into(), "hao".into(), "hao".into()];
    let options = LatticeOptions::default();
    let mut cache = BeamPrefixCache::default();
    let graph = build_graph(&first, &mut lookup(&before), &options);
    decode_graph_resuming(
        &graph,
        &options,
        &mut BeamResume {
            syllables: &first,
            cache: &mut cache,
            personal_version: 0,
        },
    );
    let graph = build_graph(&second, &mut lookup(&after), &options);
    let resumed = decode_graph_resuming(
        &graph,
        &options,
        &mut BeamResume {
            syllables: &second,
            cache: &mut cache,
            personal_version: 0,
        },
    );
    assert_eq!(cache.resumed, 0, "下标越界的存档也接上了");
    assert_same_paths(
        &resumed,
        &decode_graph(&graph, &options, None),
        "unfit edges",
    );
}
