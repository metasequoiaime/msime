//! 释义打分的回归测试：用 ECDICT 原样的行（`ecdict-regression.csv`，只清空了与释义无关的长列）和拼音表里这些中文词的真实权重，钉住出过错的那几个词：影响 曾经译成 win; reach，一点 译成 ghost，is 译成「信息系统」，page 译成「事件」，post 译成「柱；杆」，淡灰色、公羊、签证、格言 借了 `english_words` 的品牌写法（PuTTY、RAM、Visa）或 ECDICT 词头的大写（Maxim）。

use std::collections::HashSet;

use rusqlite::Connection;

use super::*;

const ECDICT: &str = include_str!("ecdict-regression.csv");

/// `msime-pinyin.db` 里这些词的最大权重：原有的取自 dict-v2.0.55，2026-11-04 补的（自然、天然、post、school、putty、ram、visa、Maxim 这几行的义项）取自 dict-v2.0.57 试跑。
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
    ("自然", 1_712_884),
    ("天然", 250_544),
    ("淡灰色", 552),
    ("油灰", 1_224),
    ("氧化锡", 4_342),
    ("公羊", 5_511),
    ("撞锤", 125),
    ("猛击", 4_131),
    ("撞", 23_164),
    ("填塞", 819),
    ("签证", 84_612),
    ("签发", 13_782),
    ("格言", 7_746),
    ("座右铭", 6_670),
    ("准则", 65_660),
    ("原则", 562_681),
    ("柱", 8_431),
    ("杆", 19_405),
    ("准星", 13_377),
    ("邮件", 294_805),
    ("邮政", 18_290),
    ("职位", 400_881),
    ("岗位", 1_077_118),
    ("哨所", 2_456),
    ("兵营", 12_043),
    ("邮递", 2_622),
    ("公布", 227_663),
    ("布置", 109_710),
    ("学校", 2_348_714),
    ("鱼群", 7_599),
    ("门派", 45_062),
    ("学派", 69_681),
    ("教育", 903_707),
    ("训练", 1_303_198),
    ("培养", 620_540),
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
    ("putty", "PuTTY"),
    ("ram", "RAM"),
    ("visa", "Visa"),
    ("maxim", "maxim"),
    ("post", "post"),
    ("school", "school"),
];

/// SCOWL 拼写词典里这些词的写法（只列测试用到的）：maxim 只有小写，所以 ECDICT 的 Maxim 词头显示成小写；China 和 china 都有。
const ATTESTED: &[&str] = &[
    "America", "China", "States", "china", "maxim", "putty", "ram", "states", "visa",
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
        &ATTESTED.iter().map(|word| (*word).to_owned()).collect(),
        &GlossVetoes::parse(vetoes).unwrap(),
    )
}

fn get<'a>(map: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    map.get(key).map(String::as_str)
}

/// 英译中按义项位置：page 是「页」不是更常用的「事件」；is 只有屈折说明和 `[计]` 行，用原形 be 的义项；give 的名词行排在前面，但首项远不如动词行的首项常用，授予 不到 给 的 1/50，第二项是 产生。冷僻义项排到后面：post 的 柱、杆、准星 不到 岗位 的 1/50，school 的 鱼群、门派 也是。
#[test]
fn english_to_chinese_follows_sense_position() {
    let glosses = derive("").unwrap();
    assert_eq!(get(&glosses.en_zh, "page"), Some("页；记录"));
    assert_eq!(get(&glosses.en_zh, "is"), Some("是；表示"));
    assert_eq!(get(&glosses.en_zh, "give"), Some("给；产生"));
    assert_eq!(get(&glosses.en_zh, "win"), Some("赢得；成功"));
    assert_eq!(get(&glosses.en_zh, "china"), Some("中国；瓷器"));
    assert_eq!(get(&glosses.en_zh, "post"), Some("邮件；职位"));
    assert_eq!(get(&glosses.en_zh, "school"), Some("学校；学派"));
}

/// 中译英按义项在英文词里的位置打分：影响 是 affect 的首项、effect 的第二项，不再是 win、reach 靠后的义项；is 的 `[计]` 义项不记到 be 名下。一点 是 damn 名词行的首项（not a damn），只能靠成对否决去掉，见下一个测试。
///
/// 大小写只从 ECDICT 词头和 SCOWL 来：专名保留大写，china 的「瓷器」用小写；putty、ram、visa 的词头是小写，`english_words` 里只有 PuTTY、RAM、Visa 也显示小写；Maxim 的词头是大写，但 SCOWL 只收 maxim。
#[test]
fn chinese_to_english_follows_sense_position() {
    let glosses = derive("").unwrap();
    assert_eq!(get(&glosses.zh_en, "影响"), Some("affect; effect"));
    assert_eq!(get(&glosses.zh_en, "信息系统"), None);
    assert_eq!(get(&glosses.zh_en, "国际标准"), None);
    assert_eq!(get(&glosses.zh_en, "页面"), Some("page"));
    assert_eq!(get(&glosses.zh_en, "中国"), Some("China"));
    assert_eq!(get(&glosses.zh_en, "中国的"), Some("China"));
    assert_eq!(get(&glosses.zh_en, "瓷器"), Some("china"));
    let mut america: Vec<&str> = get(&glosses.zh_en, "美国").unwrap().split("; ").collect();
    america.sort_unstable();
    assert_eq!(america, ["America", "States"]);
    assert_eq!(get(&glosses.zh_en, "淡灰色"), Some("putty"));
    assert_eq!(get(&glosses.zh_en, "公羊"), Some("ram"));
    assert_eq!(get(&glosses.zh_en, "签证"), Some("visa"));
    assert_eq!(get(&glosses.zh_en, "格言"), Some("maxim"));
}

/// 成对否决两个方向都管：一点 不再译成 damn，damn 的英译中也不选「一点」；natural 的「白痴」两个方向都不用；匹配不到 ECDICT 义项的一对让构建失败。
#[test]
fn gloss_vetoes_remove_a_sense_in_both_directions() {
    let glosses = derive("# 冒犯\n一点\tdamn\n白痴\tnatural\n").unwrap();
    // 否决 damn 之后只剩 ghost 第五项的「一点」，位置太靠后，不给译文。
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
