//! `msime-cantonese.db` 的两项清洗，由 `cantonese::build` 调用：essay 补词要有佐证（`cap_uncorroborated`），以及维护者审定的否决表 `curation/cantonese-word-vetoes.txt`（`WordVetoes`）。
//!
//! essay 补词是词表（`jyut6ping3.words.dict.yaml`）没有收、essay 分数达到 `cantonese::ESSAY_WORD_MIN_COUNT` 的词。essay 是 Rime 按书面语料统计的词频，切分出的片段和小说人名也在其中（嘉琪 30967 分，压过同音的 假期 6318；仲好、講咗、再望），这些词按 essay 分数入库，在 `dict-v2.0.55` 的 945 个键上压过了词表收录的每一个词。所以 essay 补词要有佐证才保留 essay 分数：HKCanCor 词频表（`cantonese::CORPUS`）数到过它，或者逐字换成简体（OpenCC 的 `TSCharacters.txt`，每个字取第一个简体字形）后是拼音库的词（`PINYIN_TABLES`，去掉 `curation/pinyin-vetoes.txt` 整词否决的词），或者是 `custom/cantonese.txt` 人工审校过的词。没有佐证的词压到同键词表收录的最轻的词之下一位，键上没有词表的词时不动。拼音库只用来判断一个词在不在，库里不写入它的任何内容，所以粤拼库的 `LICENSE` 不变。
//!
//! 否决表从库里删掉条目，用来删 essay 里的错字写法这类压下去也不该出现的词：眼訓 是 眼瞓 的错写（essay 用「訓」代「瞓」），词表在同一读音上收了 眼瞓。一行 `词`（整词否决，至少两个字，这个词的所有读音都删）或 `词<TAB>粤拼`（只删这个读音），粤拼写库里的键：去掉声调的音节，一个空格分隔（`ngaan fan`）。词写库里的字面，即换成香港字形之后的写法。表里每一条都必须删到至少一个条目，否则构建失败，免得输入变了、表却悄悄过期；否决 `custom/cantonese.txt` 收录的词也让构建失败，两张人工表不能互相打架。文件不在就是空表，构建出的库与没有这套机制时相同。

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{bail, Result};

use crate::curation::PinyinVetoes;
use crate::text;

/// 维护者审定的粤语否决表，可以不存在。
pub const WORD_VETOES: &str = "curation/cantonese-word-vetoes.txt";
/// OpenCC 的繁转简字表，用来把粤语词换成简体去查拼音库。
pub const TRADITIONAL_TO_SIMPLIFIED: &str = "sources/opencc/TSCharacters.txt";
/// 拼音库的四份词表，与发布的全拼表同一组输入：rime-ice 词表、rime-ice 补充表、区划地名和人工词条。每行第一列是词。
pub const PINYIN_TABLES: [&str; 4] = [
    crate::wubi86_supplement::BASE,
    crate::wubi86_supplement::PINYIN_SUPPLEMENT,
    crate::wubi86_supplement::PLACES,
    crate::wubi86_supplement::CUSTOM,
];

/// `cantonese::build` 用的两项清洗输入；`Default` 是空的拼音库和空的否决表，测试和没有这些文件的 checkout 用它。
#[derive(Debug, Default)]
pub struct Curation {
    pub pinyin: PinyinWords,
    pub vetoes: WordVetoes,
}

/// 拼音库收的词（简体），以及查词前逐字换成简体用的对照。
#[derive(Debug, Default)]
pub struct PinyinWords {
    words: HashSet<String>,
    simplified: HashMap<char, char>,
}

impl PinyinWords {
    /// `tables` 是 `PINYIN_TABLES` 各文件的内容：`词<TAB>…` 的行，空行与 `#` 行跳过，只取第一列；`vetoes` 整词否决的词不算拼音库的词。`traditional_to_simplified` 是 `TSCharacters.txt` 的行，每个繁体字取第一个简体字形。
    pub fn new(
        tables: &[&str],
        traditional_to_simplified: &[(char, Vec<char>)],
        vetoes: &PinyinVetoes,
    ) -> Self {
        let mut words = HashSet::new();
        for table in tables {
            for line in text::without_bom(table).lines() {
                let line = text::strip(line);
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let word = line.split('\t').next().unwrap_or_default();
                if !word.is_empty() && !vetoes.vetoes_word(word) {
                    words.insert(word.to_owned());
                }
            }
        }
        let simplified = traditional_to_simplified
            .iter()
            .filter_map(|(traditional, simplified)| Some((*traditional, *simplified.first()?)))
            .collect();
        Self { words, simplified }
    }

    /// `text`（繁体）逐字换成简体之后是不是拼音库的词。
    pub fn contains(&self, text: &str) -> bool {
        let simplified: String = text
            .chars()
            .map(|character| {
                self.simplified
                    .get(&character)
                    .copied()
                    .unwrap_or(character)
            })
            .collect();
        self.words.contains(&simplified)
    }
}

/// 把没有佐证的 essay 补词压到同键词表收录的最轻的词之下一位（最低为 0），返回压低了的条目数。`essay_words` 是 essay 补词的 `(键, 字面)`，`listed` 是词表收录的 `(键, 字面)`，`corroborated` 判断一个字面有没有佐证。
pub fn cap_uncorroborated(
    entries: &mut BTreeMap<(String, String), i64>,
    essay_words: &[(String, String)],
    listed: &HashSet<(String, String)>,
    corroborated: impl Fn(&str) -> bool,
) -> usize {
    let mut lightest: HashMap<&str, i64> = HashMap::new();
    for entry in listed {
        if let Some(&weight) = entries.get(entry) {
            let slot = lightest.entry(entry.0.as_str()).or_insert(weight);
            *slot = (*slot).min(weight);
        }
    }
    let mut caps = Vec::new();
    for entry in essay_words {
        let (key, text) = entry;
        let (Some(&below), Some(&weight)) = (lightest.get(key.as_str()), entries.get(entry)) else {
            continue;
        };
        if weight >= below && !corroborated(text) {
            caps.push((entry.clone(), (below - 1).max(0)));
        }
    }
    let capped = caps.len();
    for (entry, weight) in caps {
        entries.insert(entry, weight);
    }
    capped
}

/// 否决表的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordVeto {
    pub word: String,
    /// `None` 是整词否决。
    pub key: Option<String>,
    /// 1 起的行号，报错时用。
    pub line: usize,
}

impl WordVeto {
    fn describe(&self) -> String {
        match &self.key {
            Some(key) => format!("{}:{} {} {key}", WORD_VETOES, self.line, self.word),
            None => format!("{}:{} {}", WORD_VETOES, self.line, self.word),
        }
    }

    fn matches(&self, key: &str, text: &str) -> bool {
        self.word == text && self.key.as_deref().is_none_or(|veto| veto == key)
    }
}

/// `curation/cantonese-word-vetoes.txt`，格式见模块文档。
#[derive(Debug, Default)]
pub struct WordVetoes {
    pub entries: Vec<WordVeto>,
}

/// 库里的键：小写字母组成的音节，一个空格分隔。
fn is_key(key: &str) -> bool {
    key.split(' ').all(|syllable| {
        !syllable.is_empty() && syllable.bytes().all(|byte| byte.is_ascii_lowercase())
    })
}

impl WordVetoes {
    /// 去掉首尾空白，跳过空行和 `#` 行。同一条写两遍、同一个词既整词否决又按读音否决、整词否决一个单字、键不是去声调的音节，都算错。
    pub fn parse(source: &str) -> Result<Self> {
        let mut vetoes = Self::default();
        let mut words = HashSet::new();
        let mut readings = HashSet::new();
        for (index, line) in text::without_bom(source).lines().enumerate() {
            let stripped = text::strip(line);
            if stripped.is_empty() || stripped.starts_with('#') {
                continue;
            }
            let number = index + 1;
            let fields: Vec<&str> = stripped.split('\t').map(text::strip).collect();
            let (word, key) = match fields[..] {
                [word] => (word, None),
                [word, key] => (word, Some(key)),
                _ => bail!("{WORD_VETOES}:{number}: expected a word, or a word and its toneless Jyutping: {stripped:?}"),
            };
            if word.is_empty() {
                bail!("{WORD_VETOES}:{number}: the word is empty");
            }
            match key {
                None => {
                    if word.chars().nth(1).is_none() {
                        bail!("{WORD_VETOES}:{number}: {word} is a single character; veto one of its readings instead of the character");
                    }
                    if !words.insert(word) {
                        bail!("{WORD_VETOES}:{number}: {word} is listed twice");
                    }
                }
                Some(key) => {
                    if !is_key(key) {
                        bail!("{WORD_VETOES}:{number}: {key:?} is not toneless Jyutping syllables separated by one space");
                    }
                    if !readings.insert((word, key)) {
                        bail!("{WORD_VETOES}:{number}: {word} {key} is listed twice");
                    }
                }
            }
            vetoes.entries.push(WordVeto {
                word: word.to_owned(),
                key: key.map(str::to_owned),
                line: number,
            });
        }
        if let Some(redundant) = vetoes
            .entries
            .iter()
            .find(|veto| veto.key.is_some() && words.contains(veto.word.as_str()))
        {
            bail!(
                "{}: the word is vetoed as a whole word, so its reading veto is redundant",
                redundant.describe()
            );
        }
        Ok(vetoes)
    }

    /// 删掉 `entries` 里被否决的条目，返回删掉的条目数。`custom` 是 `custom/cantonese.txt` 收录的 `(键, 字面)`：否决到其中一个，或者有一条一个条目都没删到，都报错。
    pub fn apply(
        &self,
        entries: &mut BTreeMap<(String, String), i64>,
        custom: &[(String, String)],
    ) -> Result<usize> {
        if let Some((veto, (key, text))) = self.entries.iter().find_map(|veto| {
            custom
                .iter()
                .find(|(key, text)| veto.matches(key, text))
                .map(|word| (veto, word))
        }) {
            bail!(
                "{} vetoes {text} {key}, which {} adds; keep one of the two",
                veto.describe(),
                crate::cantonese::CUSTOM
            );
        }
        let mut removed = 0;
        for veto in &self.entries {
            let matched: Vec<(String, String)> = entries
                .keys()
                .filter(|(key, text)| veto.matches(key, text))
                .cloned()
                .collect();
            if matched.is_empty() {
                bail!(
                    "{} matches no entry of {}",
                    veto.describe(),
                    crate::cantonese::DATABASE
                );
            }
            for entry in matched {
                entries.remove(&entry);
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(rows: &[(&str, &str, i64)]) -> BTreeMap<(String, String), i64> {
        rows.iter()
            .map(|(key, text, weight)| (((*key).to_owned(), (*text).to_owned()), *weight))
            .collect()
    }

    fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
        rows.iter()
            .map(|(key, text)| ((*key).to_owned(), (*text).to_owned()))
            .collect()
    }

    #[test]
    fn pinyin_words_are_looked_up_in_simplified_form_without_vetoed_words() {
        let vetoes = PinyinVetoes::parse("个鸡巴\n").unwrap();
        let traditional = crate::cantonese::parse_variants(
            TRADITIONAL_TO_SIMPLIFIED,
            "假\t假\n個\t个\n濕\t湿\n紙\t纸\n雞\t鸡\n乾\t干 乾\n",
        )
        .unwrap();
        let words = PinyinWords::new(
            &[
                "# 注释\r\n假期\tjia'qi\t6318\r\n湿纸巾\tshi'zhi'jin\t100\r\n个鸡巴\tge'ji'ba\t100\r\n",
                "\u{feff}干杯\tgan'bei\t100\n",
            ],
            &traditional,
            &vetoes,
        );
        assert!(words.contains("假期"));
        // 濕紙巾 逐字换成 湿纸巾 之后在拼音库里；乾 取第一个简体字形 干；对照里没有的字原样查，所以 濕紙巾 写成 濕纸巾 也查得到。
        assert!(words.contains("濕紙巾"));
        assert!(words.contains("濕纸巾"));
        assert!(words.contains("乾杯"));
        // 整词否决的词不算拼音库的词。
        assert!(!words.contains("個雞巴"));
        assert!(!words.contains("嘉琪"));
    }

    #[test]
    fn uncorroborated_essay_words_fall_below_the_lightest_listed_word() {
        let mut dictionary = entries(&[
            ("gaa kei", "假期", 6318),
            ("gaa kei", "嘉琪", 30967),
            ("gaa kei", "家企", 12),
            ("ngaan fan", "眼瞓", 451),
            ("ngaan fan", "眼睏", 8),
            ("ngaan fan", "眼訓", 11924),
            ("nei hou", "你好", 21493),
            ("zung hou", "仲好", 11301),
            ("zung hou", "中號", 736),
            ("zung hou", "種好", 700),
        ]);
        let essay_words = pairs(&[
            ("gaa kei", "嘉琪"),
            ("ngaan fan", "眼訓"),
            ("nei hou", "你好"),
            ("zung hou", "仲好"),
            ("zung hou", "種好"),
        ]);
        let listed: HashSet<(String, String)> = pairs(&[
            ("gaa kei", "假期"),
            ("gaa kei", "家企"),
            ("ngaan fan", "眼瞓"),
            ("ngaan fan", "眼睏"),
            ("zung hou", "中號"),
        ])
        .into_iter()
        .collect();
        // 仲好 有佐证（比如 HKCanCor 数到过）；種好 本来就比 中號 轻、不用压。
        let capped = cap_uncorroborated(&mut dictionary, &essay_words, &listed, |text| {
            text == "仲好"
        });
        let weight = |key: &str, text: &str| dictionary[&(key.to_owned(), text.to_owned())];
        assert_eq!(capped, 2);
        assert_eq!(weight("gaa kei", "嘉琪"), 11);
        assert_eq!(weight("ngaan fan", "眼訓"), 7);
        // 你好 的键上没有词表的词，不动。
        assert_eq!(weight("nei hou", "你好"), 21493);
        assert_eq!(weight("zung hou", "仲好"), 11301);
        assert_eq!(weight("zung hou", "種好"), 700);
        // 词表最轻的词只有 0 分时压到 0，与它并列。
        let mut dictionary = entries(&[("a b", "甲乙", 0), ("a b", "丙丁", 5)]);
        let listed: HashSet<(String, String)> = pairs(&[("a b", "甲乙")]).into_iter().collect();
        cap_uncorroborated(
            &mut dictionary,
            &pairs(&[("a b", "丙丁")]),
            &listed,
            |_| false,
        );
        assert_eq!(dictionary[&("a b".to_owned(), "丙丁".to_owned())], 0);
    }

    #[test]
    fn vetoes_delete_whole_words_or_one_reading() {
        let vetoes = WordVetoes::parse("\u{feff}# 维护者审定\n\n眼訓\n訓得\tfan dak\n").unwrap();
        assert_eq!(vetoes.entries.len(), 2);
        let mut dictionary = entries(&[
            ("ngaan fan", "眼訓", 7),
            ("ngaan fan", "眼瞓", 451),
            ("fan dak", "訓得", 4271),
            ("fan dak", "瞓得", 19),
            ("fan dak gwo", "訓得過", 3),
        ]);
        assert_eq!(vetoes.apply(&mut dictionary, &[]).unwrap(), 2);
        let left: Vec<&str> = dictionary.keys().map(|(_, text)| text.as_str()).collect();
        assert_eq!(left, ["瞓得", "訓得過", "眼瞓"]);
        assert!(WordVetoes::parse("").unwrap().entries.is_empty());
    }

    #[test]
    fn a_stale_or_conflicting_veto_fails_the_build() {
        let vetoes = WordVetoes::parse("眼訓\n訓得\tfan dak\n").unwrap();
        let mut dictionary = entries(&[("ngaan fan", "眼訓", 7)]);
        let error = vetoes
            .apply(&mut dictionary.clone(), &[])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("curation/cantonese-word-vetoes.txt:2 訓得 fan dak matches no entry of msime-cantonese.db"),
            "{error}"
        );
        let error = vetoes
            .apply(&mut dictionary, &pairs(&[("ngaan fan", "眼訓")]))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("vetoes 眼訓 ngaan fan, which custom/cantonese.txt adds"),
            "{error}"
        );
    }

    #[test]
    fn malformed_veto_lines_name_their_line() {
        for (source, expected) in [
            ("眼\n", ":1: 眼 is a single character"),
            ("眼訓\n眼訓\n", ":2: 眼訓 is listed twice"),
            (
                "眼訓\tngaan fan\n眼訓\tngaan fan\n",
                ":2: 眼訓 ngaan fan is listed twice",
            ),
            (
                "眼訓\tngaan5 fan3\n",
                "\"ngaan5 fan3\" is not toneless Jyutping",
            ),
            ("眼訓\tngaan  fan\n", "is not toneless Jyutping"),
            ("眼訓\tngaan fan\textra\n", ":1: expected a word"),
            (
                "眼訓\n眼訓\tngaan fan\n",
                ":2 眼訓 ngaan fan: the word is vetoed as a whole word",
            ),
        ] {
            let error = WordVetoes::parse(source).unwrap_err().to_string();
            assert!(error.contains(expected), "{source:?}: {error}");
        }
        // 单字可以按读音否决。
        assert!(WordVetoes::parse("眼\tngaan\n").is_ok());
    }
}
