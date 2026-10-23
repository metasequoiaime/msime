//! octagram `.gram` 语法模型的逐边打分：有模型时词格解码被搭配强度纠正，
//! 模型缺失/损坏/标定系数为零时解码输出与现状逐字节一致。
use std::path::Path;
use std::sync::Arc;

use super::tests::{lookup, syllables, table, Table};
use super::*;
use crate::lattice::octagram::{encode, GramConfig, GramDb};

/// 「一名+医」强搭配的模型：词格按词频偏好「一生」，搭配分把它拉回「医生」。
fn fixture_db(dir: &Path) -> Arc<GramDb> {
    let path = crate::lattice::octagram::fixture::gram_file(
        dir,
        "collocation.gram",
        &[([encode("一名"), encode("医")].concat(), 200_000)],
    );
    GramDb::shared(&path).expect("fixture .gram loads")
}

fn rows() -> Table {
    table(&[
        ("yi'ming", &[("一名", 10_000)]),
        ("yi'sheng", &[("医生", 1_000), ("一生", 2_000)]),
    ])
}

fn decode_with(
    rows: &Table,
    scorer: Option<&CollocationScorer<'_>>,
    weight: f64,
) -> Vec<SentencePath> {
    let options = LatticeOptions {
        collocation_scorer: scorer,
        collocation_weight: weight,
        ..LatticeOptions::default()
    };
    decode_sentences(&syllables("yi'ming'yi'sheng"), &mut lookup(rows), &options)
}

#[test]
fn a_gram_model_corrects_the_frequency_winner() {
    let dir = tempfile::tempdir().unwrap();
    let rows = rows();
    let baseline = decode_with(&rows, None, 0.0);
    assert_eq!(
        baseline[0].sentence, "一名一生",
        "没有语法模型时词频首选是串错句"
    );

    let db = fixture_db(dir.path());
    let config = GramConfig::default();
    let scorer = |tail: &str, word: &str, is_rear: bool| {
        db.query(tail, word, is_rear, &config) * std::f64::consts::LOG10_E
    };
    let scored = decode_with(&rows, Some(&scorer), 1.0);
    assert_eq!(
        scored[0].sentence, "一名医生",
        "「一名+医」的搭配项应把医生顶回首位"
    );
}

#[test]
fn no_gram_model_decodes_byte_identical_to_baseline() {
    let rows = rows();
    let baseline = decode_with(&rows, None, 0.0);
    // 标定系数为 0：scorer 即便在场也被短路，分位完全一致。
    let scorer = |_: &str, _: &str, _: bool| 9.9;
    let weightless = decode_with(&rows, Some(&scorer), 0.0);
    assert_eq!(
        baseline, weightless,
        "scorer 在场但 weight=0 时输出须逐字节一致"
    );
}

#[test]
fn missing_or_corrupt_gram_falls_back_to_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let rows = rows();
    let baseline = decode_with(&rows, None, 0.0);

    // 缺失路径：`GramDb::load` 回 `None`，装配方拿不到 scorer，解码退回基线。
    let missing = dir.path().join("absent.gram");
    assert!(GramDb::load(&missing).is_none());

    // 损坏的镜像同样回 `None`，不 panic。
    let corrupt = dir.path().join("corrupt.gram");
    std::fs::write(&corrupt, b"Rime::Grammar/\xFF\xFF\xFF").unwrap();
    assert!(GramDb::load(&corrupt).is_none());

    // 装配方视角：拿到的 `Option<GramDb>` 为 `None` 时 scorer 就是 `None`。
    let db = GramDb::shared(&missing);
    let config = GramConfig::default();
    let scorer = db.map(|db| {
        move |tail: &str, word: &str, is_rear: bool| {
            db.query(tail, word, is_rear, &config) * std::f64::consts::LOG10_E
        }
    });
    let scorer = scorer.as_ref().map(|f| f as &CollocationScorer<'_>);
    let fallen = decode_with(&rows, scorer, 1.0);
    assert_eq!(baseline, fallen, "模型缺失时解码输出与基线逐字节一致");
    GramDb::evict_shared(&missing);
}

/// 真实模型包的读取验证：只在本机有模型文件时运行，路径经 `MSIME_REAL_GRAM` 注入。
///
/// 两个模型的覆盖不同（八股文·词级紧凑认得「北京+大学」，白霜/墨奇不认），所以这里只断言
/// 「能加载、能对已知搭配给出非默认分」；端到端纠正另有一条用八股文固定验证。
#[test]
fn a_real_downloaded_gram_model_loads_and_scores() {
    let Some(path) = option_env!("MSIME_REAL_GRAM") else {
        eprintln!("skipping: set MSIME_REAL_GRAM=/path/to/model.gram to run this");
        return;
    };
    let db = GramDb::shared(Path::new(path)).expect("real .gram loads");
    let config = GramConfig::default();
    // 模型之间的搭配覆盖差别很大：八股文·词级紧凑对常见二元组给 −0.4 到 −4，白霜/墨奇（7 MB）
    // 很稀疏、多数组合落在未命中惩罚 −6.0 上。所以这里只断言读取端成立——给出有限分，且至少
    // 一个组合高于未命中惩罚；端到端纠正另有一条用八股文固定验证。
    let pairs = [
        ("我们", "一起"),
        ("已经", "完成"),
        ("非常", "奇怪"),
        ("这个", "年纪"),
    ];
    let scores: Vec<f64> = pairs
        .iter()
        .map(|(a, b)| db.query(a, b, false, &config))
        .collect();
    assert!(
        scores.iter().all(|s| s.is_finite()),
        "real model must give finite scores: {scores:?}"
    );
    assert!(
        scores.iter().any(|s| *s > config.non_collocation_penalty),
        "at least one known bigram should beat the miss penalty {}. scores={scores:?}",
        config.non_collocation_penalty
    );
}

/// 端到端：八股文·词级紧凑版里「北京+大学」约 −0.39、「北京+大雪」约 −2.68，差 2.3 个自然
/// 对数量级，足以把词频偏向的串错句拉回来。2026-10-21 用下载的真实模型实测通过。
#[test]
fn a_real_baguwen_model_corrects_the_frequency_winner() {
    let Some(path) = option_env!("MSIME_REAL_GRAM") else {
        eprintln!("skipping: set MSIME_REAL_GRAM=/path/to/zh-hans-t-essay-bgw-compact.gram");
        return;
    };
    // 只有八股文系的表收「北京+大学」这对；白霜/墨奇（7 MB）覆盖稀疏，跑这条没有意义。
    if !path.contains("bgw") {
        eprintln!("skipping: {path} has no 北京+大学 entry; run this with a baguwen model");
        return;
    }
    let db = GramDb::shared(Path::new(path)).expect("real .gram loads");
    let config = GramConfig::default();
    let scorer = |tail: &str, word: &str, is_rear: bool| {
        db.query(tail, word, is_rear, &config) * std::f64::consts::LOG10_E
    };
    let rows = table(&[
        ("bei'jing", &[("北京", 10_000)]),
        ("da'xue", &[("大学", 1_000), ("大雪", 2_000)]),
    ]);
    let text = syllables("bei'jing'da'xue");
    let plain = LatticeOptions::default();
    let baseline = decode_sentences(&text, &mut lookup(&rows), &plain);
    assert_eq!(
        baseline[0].sentence, "北京大雪",
        "没有语法模型时词频首选是串错句"
    );
    let scored_options = LatticeOptions {
        collocation_scorer: Some(&scorer),
        collocation_weight: 1.0,
        ..LatticeOptions::default()
    };
    let scored = decode_sentences(&text, &mut lookup(&rows), &scored_options);
    assert_eq!(
        scored[0].sentence, "北京大学",
        "真实模型要把「北京+大学」拉回首位"
    );
}

/// 调参真的进打分：同一 fixture 上把命中惩罚 -14 收紧到 -30，原本被搭配分顶回的
/// 「医生」让位给词频首选（命中项 20-30=-10 落到未命中常数 -6 之下）。偏好键
/// `sentence_collocation_penalty` 经 host-api 写进 `collocation_grammar` 后走的就是这条路径。
#[test]
fn a_stiffer_collocation_penalty_surrenders_to_the_frequency_winner() {
    let dir = tempfile::tempdir().unwrap();
    let rows = rows();
    let db = fixture_db(dir.path());
    let config = GramConfig {
        collocation_penalty: -30.0,
        ..GramConfig::default()
    };
    let scorer = |tail: &str, word: &str, is_rear: bool| {
        db.query(tail, word, is_rear, &config) * std::f64::consts::LOG10_E
    };
    let scored = decode_with(&rows, Some(&scorer), 1.0);
    assert_eq!(
        scored[0].sentence, "一名一生",
        "命中惩罚收紧后词频首选应回到首位"
    );
}
