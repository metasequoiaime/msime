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
fn a_blocked_word_is_filtered_before_the_span_limit_is_taken() {
    // 过滤在 `take(span_limit)` 之前：这里的 lookup 不限行数，跨度只取一行时拦下的 巨乳 之后的 句如 补上来。产品路径的词库查询在 SQL 里已经按 `span_limit` 限了行数，那时拦下一个词只是让这一段少一行。
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

fn assert_clean(paths: &[SentencePath]) {
    assert!(!paths.is_empty());
    for path in paths {
        assert!(
            !path.words.iter().any(|word| offensive::is_offensive(word)),
            "{}",
            path.sentence
        );
    }
}

/// 含粗俗词的长词条同样不拼进整句：小狗吃食了 的 chi'shi'le 上词库有 吃屎了（837），三个音节的奖励让它压过 吃食|了。权重取自 dict-v2.0.39。
#[test]
fn a_longer_word_containing_an_offensive_word_is_not_assembled() {
    let rows = table(&[
        ("xiao", &[("小", 2_876_209)]),
        ("gou", &[("狗", 28_740)]),
        ("xiao'gou", &[("小狗", 39_026)]),
        ("chi", &[("吃", 1_402_977)]),
        ("shi", &[("食", 10_192), ("屎", 5_709)]),
        ("le", &[("了", 52_009_647)]),
        ("gou'chi", &[("狗吃", 1_067)]),
        ("chi'shi", &[("吃屎", 18_952), ("吃食", 9_706)]),
        ("chi'shi'le", &[("吃屎了", 837)]),
    ]);
    let paths = sentences("xiao'gou'chi'shi'le", &rows);
    assert_clean(&paths);
    assert_eq!(paths[0].sentence, "小狗吃食了");
}

/// 约炮（17,710）、炮友（7,075）比 巨乳 常见：我们约跑步吧 曾解成「我们约炮不把」，我的跑友很多 曾解成「我的炮友很多」（词库没有 跑友）。权重取自 dict-v2.0.39。
#[test]
fn sexual_slang_is_not_assembled_into_everyday_sentences() {
    let rows = table(&[
        ("wo", &[("我", 26_297_004)]),
        ("men", &[("们", 544_382)]),
        ("wo'men", &[("我们", 13_696_325)]),
        ("de", &[("的", 454_876_841)]),
        ("wo'de", &[("我的", 4_559_189)]),
        ("yue", &[("约", 444_511)]),
        ("pao", &[("跑", 660_436), ("炮", 25_754)]),
        ("bu", &[("不", 4_807_412), ("步", 57_481)]),
        ("ba", &[("吧", 8_469_475), ("把", 8_805_251)]),
        ("you", &[("友", 19_462)]),
        ("hen", &[("很", 2_544_248)]),
        ("duo", &[("多", 4_347_697)]),
        ("yue'pao", &[("约炮", 17_710)]),
        ("pao'bu", &[("跑步", 335_220)]),
        ("bu'ba", &[("不把", 61_125)]),
        ("pao'you", &[("炮友", 7_075)]),
        ("hen'duo", &[("很多", 8_495_676)]),
    ]);
    let paths = sentences("wo'men'yue'pao'bu'ba", &rows);
    assert_clean(&paths);
    assert_eq!(paths[0].sentence, "我们约跑步吧");
    let paths = sentences("wo'de'pao'you'hen'duo", &rows);
    assert_clean(&paths);
    assert_eq!(paths[0].sentence, "我的跑友很多");
}

/// 豁免写法盖住的词照常拼句：水乳交融 含 乳交，但它是成语。
#[test]
fn an_exempt_word_is_still_assembled() {
    let rows = table(&[
        ("ta", &[("他", 13_506_151)]),
        ("men", &[("们", 544_382)]),
        ("ta'men", &[("他们", 8_697_542)]),
        ("shui", &[("水", 125_437)]),
        ("ru", &[("乳", 26_706)]),
        ("jiao", &[("交", 392_654)]),
        ("rong", &[("融", 24_888)]),
        ("ru'jiao", &[("乳交", 1_196)]),
        ("jiao'rong", &[("交融", 18_980)]),
        ("shui'ru'jiao'rong", &[("水乳交融", 3_008)]),
    ]);
    let paths = sentences("ta'men'shui'ru'jiao'rong", &rows);
    assert_clean(&paths);
    assert_eq!(paths[0].sentence, "他们水乳交融");
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

/// 纠错边盖住整段输入时也不换成含粗俗词的词：整段例外只给词库行（整句就是用户打的这串读音对应的词），纠错边的词来自另一串读音，不是用户打的。全拼三个音节的输入就能有 `0..3` 的纠错边（跨度 2 到 3 个音节）。
#[test]
fn a_typo_edge_spanning_the_whole_input_never_substitutes_an_offensive_word() {
    let rows = table(&[
        ("chi", &[("吃", 1_402_977)]),
        ("si", &[("四", 1_011_573)]),
        ("le", &[("了", 52_009_647)]),
    ]);
    let text = "chi'si'le";
    let options = LatticeOptions::default();
    let literal = decode_sentences(&syllables(text), &mut lookup(&rows), &options);
    let edge = |value: &str| TypoEdge {
        start: 0,
        end: 3,
        key: "chi'shi'le".into(),
        value: value.into(),
        weight: 837,
        penalty: 0.5,
    };
    assert!(decode_typo_sentence(
        &syllables(text),
        &mut lookup(&rows),
        &options,
        &literal[0],
        &[edge("吃屎了")],
    )
    .is_none());
    let typo = decode_typo_sentence(
        &syllables(text),
        &mut lookup(&rows),
        &options,
        &literal[0],
        &[edge("吃食了")],
    )
    .expect("an ordinary typo word is used");
    assert_eq!(typo.sentence, "吃食了");
}
