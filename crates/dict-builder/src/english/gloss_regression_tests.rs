//! 释义打分的回归测试：用 ECDICT 原样的行（`ecdict-regression.csv`，只清空了与释义无关的长列）和 2026-11-04 dict-v2.0.55 拼音表里这些中文词的真实权重，钉住常用度压过义项位置时出错的那几个词：一点 曾经译成 ghost; damn，影响 译成 win; reach，is 译成「信息系统」，page 译成「事件」。

use std::collections::HashSet;

use rusqlite::Connection;

use super::*;

const ECDICT: &str = include_str!("ecdict-regression.csv");

/// dict-v2.0.55 的 `msime-pinyin.db` 里这些词的最大权重。
const WEIGHTS: &[(&str, i64)] = &[
    ("一丝", 77_758),
    ("一点", 2_761_840),
    ("专栏", 242_457),
    ("丝毫", 51_649),
    ("中国", 4_037_650),
    ("中国的", 22_770),
    ("事件", 939_522),
    ("产生", 1_885_779),
    ("信息系统", 19_614),
    ("冲击", 287_114),
    ("冲突", 312_395),
    ("后端", 87_759),
    ("呼叫", 33_249),
    ("国际标准", 10_865),
    ("天然的", 14_039),
    ("延伸", 163_585),
    ("弹性", 99_029),
    ("影响", 2_881_293),
    ("影响力", 363_501),
    ("感动", 183_006),
    ("成功", 1_368_831),
    ("授予", 58_061),
    ("效果", 2_369_828),
    ("是", 53_237_347),
    ("权力", 62_450),
    ("灵魂", 386_897),
    ("瓷器", 25_364),
    ("白痴", 34_141),
    ("结果", 3_591_735),
    ("给", 5_566_758),
    ("美国", 2_996_606),
    ("美洲", 53_986),
    ("自然的", 28_446),
    ("表示", 1_306_317),
    ("记录", 899_079),
    ("诅咒", 91_172),
    ("赢得", 106_361),
    ("达到", 1_340_891),
    ("页", 144_625),
    ("页面", 442_189),
    ("鬼", 28_109),
];

/// ECDICT 行对应的英文候选词和它们在 `english_words` 里的写法（china 有大小写两种）。
const DISPLAYS: &[(&str, &str)] = &[
    ("affect", "affect"),
    ("america", "America"),
    ("be", "be"),
    ("china", "China"),
    ("china", "china"),
    ("damn", "damn"),
    ("effect", "effect"),
    ("ghost", "ghost"),
    ("give", "give"),
    ("impact", "impact"),
    ("influence", "influence"),
    ("is", "is"),
    ("natural", "natural"),
    ("page", "page"),
    ("reach", "reach"),
    ("states", "states"),
    ("win", "win"),
];

fn derive(vetoes: &str) -> Result<Glosses> {
    let english = Connection::open_in_memory().unwrap();
    english
        .execute_batch("CREATE TABLE english_words(word, display, weight);")
        .unwrap();
    for (word, display) in DISPLAYS {
        english
            .execute(
                "INSERT INTO english_words VALUES (?1, ?2, 0)",
                params![word, display],
            )
            .unwrap();
    }
    let msime = Connection::open_in_memory().unwrap();
    msime
        .execute_batch("CREATE TABLE tbl_2_y(key, jp, value, weight);")
        .unwrap();
    for (value, weight) in WEIGHTS {
        msime
            .execute(
                "INSERT INTO tbl_2_y VALUES ('', '', ?1, ?2)",
                params![value, weight],
            )
            .unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("ecdict.csv");
    std::fs::write(&csv, ECDICT).unwrap();
    derive_glosses(
        &csv,
        &english,
        &msime,
        &HashSet::new(),
        &GlossVetoes::parse(vetoes).unwrap(),
    )
}

fn get<'a>(map: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    map.get(key).map(String::as_str)
}

/// 英译中按义项位置：page 是「页」不是更常用的「事件」；is 只有屈折说明和 `[计]` 行，用原形 be 的义项；give、natural 的名词行排在前面，但首项远不如动词、形容词行的首项常用。
#[test]
fn english_to_chinese_follows_sense_position() {
    let glosses = derive("").unwrap();
    assert_eq!(get(&glosses.en_zh, "page"), Some("页；记录"));
    assert_eq!(get(&glosses.en_zh, "is"), Some("是；表示"));
    assert_eq!(get(&glosses.en_zh, "give"), Some("给；授予"));
    assert_eq!(get(&glosses.en_zh, "ghost"), Some("鬼；灵魂"));
    assert_eq!(get(&glosses.en_zh, "win"), Some("赢得；成功"));
    assert_eq!(get(&glosses.en_zh, "china"), Some("中国；瓷器"));
}

/// 中译英按义项在英文词里的位置打分：影响 是 affect 的首项、effect 的第二项，不再是 win、reach 靠后的义项；一点 在 ghost 那里排第五，位置太靠后不用；is 的 `[计]` 义项不记到 be 名下；专名保留词头的大写，china 的「瓷器」用小写。
#[test]
fn chinese_to_english_follows_sense_position() {
    let glosses = derive("").unwrap();
    assert_eq!(get(&glosses.zh_en, "影响"), Some("affect; effect"));
    assert_eq!(get(&glosses.zh_en, "一点"), Some("damn"));
    assert_eq!(get(&glosses.zh_en, "信息系统"), None);
    assert_eq!(get(&glosses.zh_en, "国际标准"), None);
    assert_eq!(get(&glosses.zh_en, "中国"), Some("China"));
    assert_eq!(get(&glosses.zh_en, "中国的"), Some("China"));
    assert_eq!(get(&glosses.zh_en, "瓷器"), Some("china"));
    assert_eq!(get(&glosses.zh_en, "美国"), Some("States; America"));
    assert_eq!(get(&glosses.zh_en, "页面"), Some("page"));
}

/// 成对否决两个方向都管：一点 不再译成 damn，damn 的英译中也不选「一点」；natural 的「白痴」两个方向都不用；匹配不到 ECDICT 义项的一对让构建失败。
#[test]
fn gloss_vetoes_remove_a_sense_in_both_directions() {
    let glosses = derive("# 冒犯\n一点\tdamn\n白痴\tnatural\n").unwrap();
    assert_eq!(get(&glosses.zh_en, "一点"), None);
    assert_eq!(get(&glosses.en_zh, "damn"), Some("诅咒；丝毫"));
    assert_eq!(get(&glosses.en_zh, "natural"), Some("自然的；天然的"));
    assert_eq!(get(&glosses.zh_en, "白痴"), None);
    assert_eq!(get(&glosses.zh_en, "丝毫"), Some("damn"));

    let error = derive("一点\tpage\n").unwrap_err().to_string();
    assert!(error.contains("一点\tpage"), "{error}");
    assert!(GlossVetoes::parse("一点\tDamn\n").is_err());
    assert!(GlossVetoes::parse("一点\n").is_err());
    assert!(GlossVetoes::parse("一点\tdamn\n一点\tdamn\n").is_err());
}
