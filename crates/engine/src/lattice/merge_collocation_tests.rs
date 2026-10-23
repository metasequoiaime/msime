//! `include_collocation_best`：语法模型激活时解码首选以 `Collocation` 行展示，
//! 其余 n-best 行仍是 `Generated`；两开关都关时没有词格行。
use super::*;
use crate::lattice::decode::tests::{lookup, syllables, table};

fn merge(
    rows: &crate::lattice::decode::tests::Table,
    options: &LatticeOptions<'_>,
) -> Vec<crate::types::WordItem> {
    let mut candidates = Vec::new();
    let mut rerankers = Vec::new();
    merge_lattice_candidates(
        &mut candidates,
        &syllables("yi'ming'yi'sheng"),
        &mut lookup(rows),
        "yi'ming'yi'sheng",
        options,
        None,
        &mut rerankers,
        "",
    );
    candidates
}

fn rows() -> crate::lattice::decode::tests::Table {
    table(&[
        ("yi'ming", &[("一名", 10_000)]),
        ("yi'sheng", &[("医生", 1_000), ("一生", 2_000)]),
    ])
}

#[test]
fn collocation_flag_marks_the_best_path_collocation() {
    let rows = rows();
    let options = LatticeOptions {
        emit: 2,
        include_collocation_best: true,
        ..LatticeOptions::default()
    };
    let out = merge(&rows, &options);
    assert_eq!(out[0].source, CandidateSource::Collocation);
    assert_eq!(out[0].word, "一名一生");
    assert!(
        out.iter()
            .skip(1)
            .all(|row| row.source == CandidateSource::Generated),
        "其余 n-best 行仍是 Generated"
    );
}

#[test]
fn collocation_only_keeps_a_single_collocation_row() {
    let rows = rows();
    let options = LatticeOptions {
        emit: 2,
        include_lattice_best: false,
        include_collocation_best: true,
        ..LatticeOptions::default()
    };
    let out = merge(&rows, &options);
    assert_eq!(out.len(), 1, "词格自己的行不出，只留搭配行");
    assert_eq!(out[0].source, CandidateSource::Collocation);
}

#[test]
fn flags_off_keeps_the_no_reranker_baseline_rows() {
    // 基线语义：无重排通道从不看 include_lattice_best，两个开关都关时
    // 词格行照常以 Generated 出——门控在上游 merge_sentences。
    let rows = rows();
    let options = LatticeOptions {
        include_lattice_best: false,
        include_collocation_best: false,
        ..LatticeOptions::default()
    };
    let out = merge(&rows, &options);
    assert!(!out.is_empty(), "基线行为照常出词格行");
    assert!(out
        .iter()
        .all(|row| row.source == CandidateSource::Generated));
}
