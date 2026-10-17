use super::*;

#[test]
fn constrained_filter_reuses_endpoint_counts() {
    use crate::ime::personal_rerank::allocations::measure;

    let syllables: Vec<String> = (0..16).map(|index| format!("s{index}")).collect();
    let options = LatticeOptions {
        span_limit: 8,
        ..LatticeOptions::default()
    };
    let mut lookup = |_span: &[String]| Vec::new();
    let (paths, stats) =
        measure(|| decode_constrained_sentences(&syllables, &mut lookup, &options, |_, _| true));
    assert!(paths.is_empty());
    eprintln!("constrained stats: {stats:?}");
    assert!(
        stats.allocations <= 6,
        "constrained filter allocations: {stats:?}"
    );
}

#[test]
fn constrained_filter_empty_input_stays_allocation_free() {
    use crate::ime::personal_rerank::allocations::measure;

    let options = LatticeOptions::default();
    let mut lookup = |_span: &[String]| -> Vec<LatticeLexeme> {
        panic!("empty constrained input must not query the dictionary");
    };
    let (paths, stats) =
        measure(|| decode_constrained_sentences(&[], &mut lookup, &options, |_, _| true));
    assert!(paths.is_empty());
    assert_eq!(stats.allocations, 0, "empty constrained input: {stats:?}");
}

#[test]
fn constrained_filter_preserves_rows_and_span_limit() {
    use super::tests::{lookup, syllables, table};

    let rows = table(&[
        ("a", &[("甲", 100), ("阿", 200)]),
        ("b", &[("乙", 100)]),
        ("a'b", &[("甲乙", 300)]),
    ]);
    let options = LatticeOptions {
        span_limit: 1,
        ..LatticeOptions::default()
    };
    let paths = decode_constrained_sentences(
        &syllables("a'b"),
        &mut lookup(&rows),
        &options,
        |_, word| word != "阿",
    );
    assert_eq!(
        paths.first().map(|path| path.sentence.as_str()),
        Some("甲乙")
    );
}
