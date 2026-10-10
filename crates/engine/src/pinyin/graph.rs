//! The syllable graph and its complete segmentations (quanpin.md §4.1), used for alternative segmentations of short inputs and by `normalize_full_pinyin`.

use super::syllables::{intact_piece, MAX_SYLLABLE_LENGTH};

pub const SYLLABLE_GRAPH_PATH_LIMIT: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyllableEdge {
    pub end: usize,
    pub syllable: &'static str,
}

/// `edges[start]` lists the intact syllables starting at `start`, longest first, pruned to those from which the end is reachable. Empty input or input containing `'` has no edges.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyllableGraph {
    pub input_length: usize,
    pub edges: Vec<Vec<SyllableEdge>>,
}

/// QU:245-285.
pub fn build_syllable_graph(pinyin: &str) -> SyllableGraph {
    let bytes = pinyin.as_bytes();
    let length = bytes.len();
    let mut graph = SyllableGraph {
        input_length: length,
        edges: vec![Vec::new(); length + 1],
    };
    if pinyin.is_empty() || pinyin.contains('\'') {
        return graph;
    }
    for start in 0..length {
        let bucket_capacity = MAX_SYLLABLE_LENGTH.min(length - start);
        for end in (start + 1..=length.min(start + MAX_SYLLABLE_LENGTH)).rev() {
            if let Some(syllable) = intact_piece(&bytes[start..end]) {
                let edges = &mut graph.edges[start];
                if edges.capacity() == 0 {
                    edges.reserve(bucket_capacity);
                }
                edges.push(SyllableEdge { end, syllable });
            }
        }
    }
    let mut reaches_end = vec![false; length + 1];
    reaches_end[length] = true;
    for start in (0..length).rev() {
        let edges = &mut graph.edges[start];
        edges.retain(|edge| reaches_end[edge.end]);
        reaches_end[start] = !edges.is_empty();
    }
    graph
}

/// Depth-first from 0 in edge order, stopping at `path_limit` paths (QU:288-325).
pub fn enumerate_complete_segmentations(
    graph: &SyllableGraph,
    path_limit: usize,
) -> Vec<Vec<String>> {
    fn visit(
        graph: &SyllableGraph,
        position: usize,
        path_limit: usize,
        current: &mut Vec<&'static str>,
        result: &mut Vec<Vec<String>>,
    ) {
        if result.len() >= path_limit {
            return;
        }
        if position == graph.input_length {
            if result.is_empty() {
                result.reserve_exact(path_limit);
            }
            result.push(
                current
                    .iter()
                    .map(|syllable| (*syllable).to_owned())
                    .collect(),
            );
            return;
        }
        for edge in &graph.edges[position] {
            if current.capacity() == 0 {
                current.reserve_exact(graph.input_length);
            }
            current.push(edge.syllable);
            visit(graph, edge.end, path_limit, current, result);
            current.pop();
            if result.len() >= path_limit {
                return;
            }
        }
    }

    let mut result = Vec::new();
    if path_limit == 0 || graph.input_length == 0 || graph.edges.len() != graph.input_length + 1 {
        return result;
    }
    visit(graph, 0, path_limit, &mut Vec::new(), &mut result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(pinyin: &str, limit: usize) -> Vec<String> {
        enumerate_complete_segmentations(&build_syllable_graph(pinyin), limit)
            .iter()
            .map(|path| path.join("'"))
            .collect()
    }

    #[test]
    fn complete_segmentations_come_longest_edge_first() {
        assert_eq!(paths("xian", SYLLABLE_GRAPH_PATH_LIMIT), ["xian", "xi'an"]);
        assert_eq!(
            paths("fangan", SYLLABLE_GRAPH_PATH_LIMIT),
            ["fang'an", "fan'gan"]
        );
        assert_eq!(
            paths("xianxian", 3),
            ["xian'xian", "xian'xi'an", "xi'an'xian"]
        );
        assert!(paths("nihz", SYLLABLE_GRAPH_PATH_LIMIT).is_empty());
        assert!(paths("xi'an", SYLLABLE_GRAPH_PATH_LIMIT).is_empty());
        assert!(paths("", SYLLABLE_GRAPH_PATH_LIMIT).is_empty());
        assert!(paths("xian", 0).is_empty());
    }

    #[test]
    fn complete_segmentations_reserve_after_the_first_path() {
        let graph = build_syllable_graph("xian");
        let paths = enumerate_complete_segmentations(&graph, 8);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths.capacity(), 8);
    }

    #[test]
    fn complete_segmentation_walk_borrows_temporary_syllables() {
        let graph = build_syllable_graph("xian");
        let (result, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            enumerate_complete_segmentations(&graph, SYLLABLE_GRAPH_PATH_LIMIT)
        });

        assert_eq!(result, [vec!["xian"], vec!["xi", "an"]]);
        assert_eq!(allocations, 7, "临时遍历路径仍复制音节: {allocations}");
    }

    #[test]
    fn dead_end_edges_are_pruned() {
        let graph = build_syllable_graph("nihz");
        assert_eq!(graph.edges.len(), 5);
        assert!(graph.edges.iter().all(Vec::is_empty));
        let graph = build_syllable_graph("zhonge");
        let from_start: Vec<_> = graph.edges[0].iter().map(|edge| edge.syllable).collect();
        // `zhong` is the only syllable at the start (`zhon`, `zho` and `zh` are prefixes), and it reaches `e`.
        assert_eq!(from_start, ["zhong"]);
    }

    #[test]
    fn edge_buckets_reserve_the_candidate_window() {
        let graph = build_syllable_graph("xianxian");

        for (start, edges) in graph.edges.iter().enumerate() {
            let expected = MAX_SYLLABLE_LENGTH.min(graph.input_length.saturating_sub(start));
            if !edges.is_empty() {
                assert!(
                    edges.capacity() >= expected,
                    "edge bucket at {start} has capacity {}, expected at least {expected}",
                    edges.capacity()
                );
            }
        }
    }

    #[test]
    fn unreadable_input_does_not_allocate_edge_buckets() {
        let graph = build_syllable_graph(&"z".repeat(256));

        assert!(graph.edges.iter().all(|edges| edges.capacity() == 0));
    }

    #[test]
    fn unreadable_graph_does_not_allocate_path_storage() {
        let graph = build_syllable_graph(&"z".repeat(256));
        let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            enumerate_complete_segmentations(&graph, SYLLABLE_GRAPH_PATH_LIMIT)
        });

        assert!(paths.is_empty());
        assert_eq!(
            allocations, 0,
            "无路径时不应申请遍历工作缓冲: {allocations}"
        );
    }

    #[test]
    fn the_empty_or_mismatched_graph_has_no_paths() {
        let graph = SyllableGraph {
            input_length: 2,
            edges: Vec::new(),
        };
        let paths = enumerate_complete_segmentations(&graph, 4);
        assert!(paths.is_empty());
        assert_eq!(paths.capacity(), 0);
    }
}
