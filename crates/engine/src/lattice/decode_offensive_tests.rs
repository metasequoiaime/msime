use super::tests::{lookup, syllables, table, Table};
use super::*;

/// 九键 966369685878 的一条切分 wo'men'you'ju'ru，权重取自 dict-v2.0.39：巨乳 只有 3174，但多字词每个音节有 `PHRASE_LENGTH_BONUS`，它压过同码的 据、如 这些常用单字，整句成了「我们有巨乳」。
fn women_you_ju_ru() -> Table {
    table(&[
        ("wo", &[("我", 30_000_000)]),
        ("men", &[("们", 3_000_000)]),
        ("wo'men", &[("我们", 13_696_325)]),
        ("you", &[("有", 15_615_006), ("又", 2_794_198)]),
        ("wo'men'you", &[("我们有", 83_600)]),
        ("ju", &[("据", 323_831), ("具", 58_121)]),
        ("ru", &[("如", 2_514_076), ("乳", 26_706)]),
        ("ju'ru", &[("巨乳", 3_174)]),
        ("you'ju", &[("邮局", 22_016), ("有句", 15_824)]),
    ])
}

fn sentences(text: &str, rows: &Table) -> Vec<SentencePath> {
    let options = LatticeOptions {
        nbest: 6,
        ..LatticeOptions::default()
    };
    decode_sentences(&syllables(text), &mut lookup(rows), &options)
}

#[test]
fn an_offensive_word_is_not_assembled_into_a_sentence() {
    let rows = women_you_ju_ru();
    let paths = sentences("wo'men'you'ju'ru", &rows);
    assert!(!paths.is_empty());
    for path in &paths {
        assert!(
            !path.words.iter().any(|word| word == "巨乳"),
            "{}",
            path.sentence
        );
    }
    // 去掉粗俗词之后，原来排在它后面的整句照常解出来。
    assert_eq!(paths[0].sentence, "我们邮局如");
}

#[test]
fn an_offensive_word_spanning_the_whole_input_is_still_its_own_sentence() {
    let rows = women_you_ju_ru();
    let paths = sentences("ju'ru", &rows);
    assert_eq!(paths[0].sentence, "巨乳");
}

#[test]
fn an_offensive_word_does_not_take_a_span_limit_seat() {
    // 跨度只取一行时，被拦下的粗俗词不能占掉这个名额，让这一段没有多字词可用。
    let rows = table(&[
        ("wo", &[("我", 30_000_000)]),
        ("ju", &[("据", 323_831)]),
        ("ru", &[("如", 2_514_076)]),
        ("ju'ru", &[("巨乳", 3_174), ("句如", 100)]),
    ]);
    let options = LatticeOptions {
        span_limit: 1,
        ..LatticeOptions::default()
    };
    let paths = decode_sentences(&syllables("wo'ju'ru"), &mut lookup(&rows), &options);
    assert_eq!(paths[0].sentence, "我句如");
}

#[test]
fn a_typo_edge_never_substitutes_an_offensive_word() {
    let rows = table(&[
        ("wo", &[("我", 30_000_000)]),
        ("ju", &[("据", 323_831)]),
        ("lu", &[("路", 476_190)]),
    ]);
    let text = "wo'ju'lu";
    let options = LatticeOptions::default();
    let literal = decode_sentences(&syllables(text), &mut lookup(&rows), &options);
    let edge = |value: &str| TypoEdge {
        start: 1,
        end: 3,
        key: "ju'ru".into(),
        value: value.into(),
        weight: 3_174,
        penalty: 0.5,
    };
    assert!(decode_typo_sentence(
        &syllables(text),
        &mut lookup(&rows),
        &options,
        &literal[0],
        &[edge("巨乳")],
    )
    .is_none());
    // 同样的边换成普通词就会被采用，说明拦下的是词本身。
    let typo = decode_typo_sentence(
        &syllables(text),
        &mut lookup(&rows),
        &options,
        &literal[0],
        &[edge("句如")],
    )
    .expect("an ordinary typo word is used");
    assert_eq!(typo.sentence, "我句如");
}
