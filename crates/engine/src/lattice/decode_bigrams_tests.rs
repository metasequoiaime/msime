use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::*;

// 小图穷举每条完整路径，逐边直接查表，独立于按列展开和二元分行缓存。
fn exhaustive_paths(
    graph: &Graph,
    extra: Option<&Graph>,
    options: &LatticeOptions<'_>,
) -> Vec<SentencePath> {
    fn visit(
        graph: &Graph,
        extra: Option<&Graph>,
        options: &LatticeOptions<'_>,
        position: usize,
        path: SentencePath,
        paths: &mut Vec<SentencePath>,
    ) {
        if position == graph.len() {
            if !path.words.is_empty() {
                paths.push(path);
            }
            return;
        }
        let previous = path.words.last().map_or(SENTENCE_START, String::as_str);
        for edge in graph[position]
            .iter()
            .chain(extra.into_iter().flat_map(|extra| &extra[position]))
        {
            let mut next = path.clone();
            next.log_prob += edge.log_prob;
            let bonus = options
                .bigram
                .as_ref()
                .map_or(0.0, |table| f64::from(table.bigram(previous, &edge.word)));
            if options.bigram.is_some() {
                next.log_prob += options.bigram_weight * bonus.max(0.0);
            }
            if let Some(personal) = active_personal(options) {
                let earlier = path
                    .words
                    .len()
                    .checked_sub(2)
                    .map(|index| path.words[index].as_str());
                let context = personal.context(earlier, Some(previous));
                if context.confidence > 0.0 {
                    next.log_prob +=
                        personal_transition(personal, &context, &edge.word, bonus, options);
                }
            }
            next.sentence.push_str(&edge.word);
            if !next.key.is_empty() {
                next.key.push('\'');
            }
            next.key.push_str(&edge.key);
            next.words.push(edge.word.clone());
            next.typo_edges += usize::from(edge.typo);
            visit(graph, extra, options, edge.end, next, paths);
        }
    }

    let mut paths = Vec::new();
    visit(
        graph,
        extra,
        options,
        0,
        SentencePath {
            sentence: String::new(),
            key: String::new(),
            log_prob: 0.0,
            words: Vec::new(),
            typo_edges: 0,
        },
        &mut paths,
    );
    paths.sort_by(|a, b| b.log_prob.total_cmp(&a.log_prob));
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(path.sentence.clone()));
    paths.truncate(options.nbest);
    rescore_with_trigram(&mut paths, options);
    paths
}

#[test]
fn varying_bigram_rows_preserve_paths_and_score_bits() {
    let edge = |end, word: &str, score, typo| Edge {
        end,
        word: word.to_owned(),
        key: format!("key{end}"),
        log_prob: score,
        typo,
    };
    // 前词数和出边宽度都先增后减；空列有的可达，有的仅由附加图接通。
    let graph = vec![
        vec![
            edge(1, "甲", -0.1, false),
            edge(1, "乙", -0.7, false),
            edge(2, "丙", -1.3, false),
        ],
        vec![edge(3, "甲", -0.3, false)],
        vec![],
        vec![
            edge(4, "乙", -0.17, false),
            edge(4, "丙", -1.03, false),
            edge(5, "丁", -1.37, false),
            edge(4, "甲", -2.07, false),
        ],
        vec![edge(6, "甲", -0.21, false)],
        vec![],
    ];
    let extra = vec![
        vec![],
        vec![],
        vec![edge(3, "丁", -0.9, true), edge(4, "乙", -2.2, true)],
        vec![edge(4, "丁", -1.99, true)],
        vec![],
        vec![edge(6, "甲", -0.51, true)],
    ];
    let directory = tempfile::tempdir().unwrap();
    let bigram = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[
            (SENTENCE_START, "甲", 0.75),
            ("甲", "甲", -0.5),
            ("甲", "乙", 1.25),
            ("甲", "丁", 0.375),
            ("乙", "甲", 0.625),
            ("丁", "乙", 0.875),
            ("丙", "丁", 1.5),
        ],
    ));
    let trigram = Arc::new(triple_table(
        directory.path(),
        "triples.bin",
        &[("甲", "乙", "甲", 0.75), ("丁", "乙", "甲", -0.5)],
    ));
    let mut personal = PersonalNgram::default();
    record_run(&mut personal, &["甲", "甲", "丁", "甲"], 7);
    for mode in 0..32 {
        let memo = RefCell::new(BigramMemo::default());
        let options = LatticeOptions {
            beam: 128,
            nbest: 128,
            bigram: (mode & 1 != 0).then(|| Arc::clone(&bigram)),
            trigram: (mode & 2 != 0).then(|| Arc::clone(&trigram)),
            personal: (mode & 4 != 0).then_some(&personal),
            bigram_memo: (mode & 8 != 0).then_some(&memo),
            ..LatticeOptions::default()
        };
        let extra = (mode & 16 != 0).then_some(&extra);
        let expected = exhaustive_paths(&graph, extra, &options);
        assert!(!expected.is_empty());
        for _ in 0..2 {
            let actual = decode_graph(&graph, &options, extra);
            assert_eq!(actual, expected, "选项组合 {mode}");
            assert_eq!(
                actual
                    .iter()
                    .map(|path| path.log_prob.to_bits())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|path| path.log_prob.to_bits())
                    .collect::<Vec<_>>(),
                "选项组合 {mode} 的评分位模式"
            );
        }
    }
}
