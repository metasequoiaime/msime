//! 模糊行按折后权重并进同一覆盖档的字面行（#11321），经 `QuanpinDictionary::query` 走完整的合并顺序。合并函数自己的边界情况在 `pinyin::fuzzy_rank` 的单测里。

use super::*;
use crate::pinyin::fuzzy_rank::FUZZY_WEIGHT_DIVISOR;
use crate::quanpin::fixture::Fixture;
use crate::types::fuzzy_rule;

const NONE: u32 = 0;
const NO_FUZZY: FuzzyPinyinOptions = FuzzyPinyinOptions { rules: 0 };
const Z_ZH: FuzzyPinyinOptions = FuzzyPinyinOptions {
    rules: fuzzy_rule::Z_ZH,
};

fn words(rows: &[WordItem]) -> Vec<&str> {
    rows.iter().map(|row| row.word.as_str()).collect()
}

/// 覆盖 `keys` 个键的词库行，按列表里的先后。
fn dictionary_words(rows: &[WordItem], keys: usize) -> Vec<&str> {
    rows.iter()
        .filter(|row| row.source.is_dictionary() && matched_letters(&row.pinyin) == keys)
        .map(|row| row.word.as_str())
        .collect()
}

/// #11321 的形状：`ziqian` 的字面读法只有几个冷僻词，开了 z=zh 的人要的是「之前」。
fn fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .insert("zi'qian", "自谦", 5_000)
        .insert("zi'qian", "自遣", 3_000)
        .insert("zi'qian", "自谴", 100)
        .insert("zi'qian'ming", "自签名", 90)
        .insert("zhi'qian", "之前", 4_000_000)
        // 折后正好等于「自遣」：权重相当时字面在前。
        .insert("zhi'qian", "值钱", 3_000 * FUZZY_WEIGHT_DIVISOR)
        // 原权重比「自谦」高，折后比它低。
        .insert("zhi'qian", "纸钱", 5_000 * FUZZY_WEIGHT_DIVISOR - 2)
        .insert("zhi'qian", "掷签", 60)
        .insert("zi", "自", 9_000)
        .insert("zi", "字", 8_000)
        .insert(
            "zhi",
            "只",
            9_000 * FUZZY_WEIGHT_DIVISOR + FUZZY_WEIGHT_DIVISOR,
        )
        .insert("zhi", "之", 8_000 * FUZZY_WEIGHT_DIVISOR)
        .insert("qian", "前", 7_000);
    fixture
}

#[test]
fn heavy_fuzzy_words_pass_light_exact_words_and_close_weights_keep_the_exact_word_first() {
    let fixture = fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let rows = dictionary.query("ziqian", "zi'qian", NONE, Z_ZH);
    assert_eq!(rows[0].word, "之前", "{:?}", words(&rows));
    assert!(rows[0].fuzzy);
    assert_eq!(rows[0].pinyin, "zi'qian");
    assert_eq!(rows[0].canonical_pinyin, "zhi'qian");
    assert_eq!(
        dictionary_words(&rows, 6),
        [
            "之前",
            "自谦",
            "纸钱",
            "自遣",
            "值钱",
            "自谴",
            "自签名",
            "掷签"
        ],
        "{:?}",
        words(&rows)
    );
    // 只覆盖第一个音节的那一档同样按折后权重并：「只」折后比「自」重一点，「之」折后正好等于「字」，字面在前。
    assert_eq!(
        dictionary_words(&rows, 2),
        ["只", "自", "字", "之"],
        "{:?}",
        words(&rows)
    );
}

#[test]
fn exact_rows_keep_their_order_and_the_plain_query_is_untouched() {
    let fixture = fixture();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let plain = dictionary.query("ziqian", "zi'qian", NONE, NO_FUZZY);
    assert!(plain.iter().all(|row| !row.fuzzy), "{:?}", words(&plain));
    let fuzzy = dictionary.query("ziqian", "zi'qian", NONE, Z_ZH);
    let kept: Vec<WordItem> = fuzzy.iter().filter(|row| !row.fuzzy).cloned().collect();
    assert_eq!(kept, plain, "模糊行以外的行换了先后");
    // 再查一次不开模糊音的，确认模糊合并没有写进普通缓存。
    assert_eq!(dictionary.query("ziqian", "zi'qian", NONE, NO_FUZZY), plain);
}

/// 原拼音没有整词时，模糊整词仍然整体排在替补整句前面（2026-10-08 的决定），不看它们和补全词谁重。
#[test]
fn fuzzy_whole_words_still_lead_a_stand_in_sentence() {
    let fixture = Fixture::new();
    fixture
        .insert("zong", "总", 3_000)
        .insert("guo", "国", 2_000)
        .insert("zhong'guo", "中国", 1_000_000)
        .insert("zhong'guo", "种过", 50)
        .insert("zong'guo'wu", "总国务", 40_000);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let rows = dictionary.query("zongguo", "zong'guo", NONE, Z_ZH);
    assert_eq!(words(&rows[..2]), ["中国", "种过"], "{:?}", words(&rows));
    let sentence = rows
        .iter()
        .position(|row| row.sentence_association)
        .expect("整句行");
    assert_eq!(sentence, 2, "{:?}", words(&rows));
    assert!(
        rows.iter().position(|row| row.word == "总国务") > Some(sentence),
        "{:?}",
        words(&rows)
    );
}
