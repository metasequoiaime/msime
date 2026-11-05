//! `msime-cantonese.db` 的两项清洗，由 `cantonese::build` 调用：essay 补词要有佐证（`cap_uncorroborated`，维护者可以用 `curation/cantonese-essay-keep.txt` 豁免），以及维护者审定的否决表 `curation/cantonese-word-vetoes.txt`（`WordVetoes`）。
//!
//! essay 补词是词表（`jyut6ping3.words.dict.yaml`）没有收、essay 分数达到 `cantonese::ESSAY_WORD_MIN_COUNT` 的词。essay 是 Rime 按书面语料统计的词频，切分出的片段和小说人名也在其中（嘉琪 30967 分，压过同音的 假期 6318；歲的、中最），这些词按 essay 分数入库，在 `dict-v2.0.55` 的 945 个键上压过了词表收录的每一个词。所以 essay 补词要有佐证才保留 essay 分数：HKCanCor 词频表（`cantonese::CORPUS`）数到过它，或者逐字换成简体（OpenCC 的 `TSCharacters.txt`，每个字取第一个简体字形）后是拼音库的词（`PINYIN_TABLES`），或者是 `custom/cantonese.txt` 人工审校过的词，或者在豁免表里。没有佐证的词压到同键「有证据的」词表词里最轻的那个之下一位：有证据指 essay 或 HKCanCor 数到过这个词表词；只靠 `unlisted_word_weight` 兜底权重的词表词（壽飯 8、蚊飯 16、髀飯 13）本身没有证据，压到它们之下等于让没人用的词压过 essay 数过几千次的 收返、問返、畀返，所以不算。键上没有有证据的词表词时不动。拼音库只用来判断一个词在不在，库里不写入它的任何内容，所以粤拼库的 `LICENSE` 不变。
//!
//! 拼音库的佐证只取上游锁定的三张拼音表，不取 `custom/words.txt`：那是 msime-web 和 msime-cloud 直接写入的投稿表，拼音投稿不该悄悄改变粤拼库的排序。三张表只去掉 `curation/pinyin-vetoes.txt` 否决的读音，一个词至少有一个读音没被否决才算拼音库的词。`curation/pinyin-weight-overrides.txt` 不影响一个词在不在库：不带 `wubi` 的行是相对同一全拼键上竞争词定的降权（张相 让给 长相），带 `wubi` 的行是本地权重损坏恢复上游值这类频率修正，两种都只说明它在普通话同键上该排第几，不说明它不是词，张相、上都、劲舞 在粤语里照样是常用词；一个词有过拼音读音，就有过佐证。所以改拼音否决表会改变粤拼库，审否决表的 PR 时要连粤拼库的产物差异一起看，覆盖表不会。
//!
//! HKCanCor 把 咗、返、住、緊 这些体貌助词和 嗰、呢 这些指示词单独切成一个词，所以「动词加助词」「指示词加量词」（講咗、收返、開住、嗰種）永远得不到 HKCanCor 的佐证，拼音库也不会有。这类常用口语词由维护者审过 `cap_uncorroborated` 列出的压低清单后写进豁免表 `curation/cantonese-essay-keep.txt`，一行 `词<TAB>粤拼`（粤拼写库里的键：去掉声调的音节，一个空格分隔），保留 essay 分数。表里每一行都必须是这次构建会压低的 essay 补词，否则构建失败：输入变了、这个词已经有了佐证或者不再是 essay 补词时，表就过期了。同一个词既豁免又否决也让构建失败。
//!
//! 否决表从库里删掉条目，用来删 essay 里的错字写法这类压下去也不该出现的词：眼訓 是 眼瞓 的错写（essay 用「訓」代「瞓」），词表在同一读音上收了 眼瞓。一行 `词`（整词否决，至少两个字，这个词的所有读音都删）或 `词<TAB>粤拼`（只删这个读音），粤拼的写法同上。词写库里的字面，即换成香港字形之后的写法。表里每一条都必须删到至少一个条目，否则构建失败，免得输入变了、表却悄悄过期；否决 `custom/cantonese.txt` 收录的词也让构建失败，两张人工表不能互相打架。两张表的文件不在就是空表。

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{bail, Result};

use crate::curation::PinyinVetoes;
use crate::places_supplement::weighted_rows;
use crate::text;

/// 维护者审定的粤语否决表，可以不存在。
pub const WORD_VETOES: &str = "curation/cantonese-word-vetoes.txt";
/// 维护者审定的 essay 补词豁免表，可以不存在。
pub const ESSAY_KEEPS: &str = "curation/cantonese-essay-keep.txt";
/// OpenCC 的繁转简字表，用来把粤语词换成简体去查拼音库。
pub const TRADITIONAL_TO_SIMPLIFIED: &str = "sources/opencc/TSCharacters.txt";
/// 拼音库里上游锁定的三张词表：rime-ice 词表、rime-ice 补充表和区划地名，都是 `词<TAB>全拼<TAB>权重` 的行。不取 `custom/words.txt`，见模块文档。
pub const PINYIN_TABLES: [&str; 3] = [
    crate::wubi86_supplement::BASE,
    crate::wubi86_supplement::PINYIN_SUPPLEMENT,
    crate::wubi86_supplement::PLACES,
];

/// `cantonese::build` 用的清洗输入；`Default` 是空的拼音库、空的豁免表和空的否决表，测试和没有这些文件的 checkout 用它。
#[derive(Debug, Default)]
pub struct Curation {
    pub pinyin: PinyinWords,
    pub keeps: EssayKeeps,
    pub vetoes: WordVetoes,
}

impl Curation {
    /// 豁免表和否决表写到同一个 `(词, 键)` 时报错：一个说留、一个说删，只能留一条。
    pub fn check(&self) -> Result<()> {
        if let Some((keep, veto)) = self.keeps.entries.iter().find_map(|keep| {
            self.vetoes
                .entries
                .iter()
                .find(|veto| veto.matches(&keep.key, &keep.word))
                .map(|veto| (keep, veto))
        }) {
            bail!(
                "{ESSAY_KEEPS}:{} keeps {} {}, which {} vetoes; keep one of the two",
                keep.line,
                keep.word,
                keep.key,
                veto.describe()
            );
        }
        Ok(())
    }
}

/// 拼音库收的词（简体），以及查词前逐字换成简体用的对照。
#[derive(Debug, Default)]
pub struct PinyinWords {
    words: HashSet<String>,
    simplified: HashMap<char, char>,
}

impl PinyinWords {
    /// `tables` 是 `PINYIN_TABLES` 各文件的内容，按 `weighted_rows` 读 `词<TAB>全拼<TAB>权重` 的行；`vetoes`（`curation/pinyin-vetoes.txt`）否决的读音不算，一个词有一个读音留下就是拼音库的词。权重覆盖表不在这里，见模块文档。`traditional_to_simplified` 是 `TSCharacters.txt` 的行，每个繁体字取第一个简体字形。
    pub fn new(
        tables: &[&str],
        traditional_to_simplified: &[(char, Vec<char>)],
        vetoes: &PinyinVetoes,
    ) -> Self {
        let mut words = HashSet::new();
        for table in tables {
            for (word, key, _) in weighted_rows(text::without_bom(table)) {
                if !word.is_empty() && !vetoes.vetoes(word, key) {
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

/// `cap_uncorroborated` 压低的一个条目，给构建摘要列出来供维护者审。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capped {
    pub key: String,
    pub text: String,
    pub from: i64,
    pub to: i64,
}

/// 把没有佐证的 essay 补词压到同键有证据的词表词里最轻的那个之下一位（最低为 0），返回压低了的条目和豁免表留下的条目数。`essay_words` 是 essay 补词的 `(键, 字面)`，`evidenced` 是 essay 或 HKCanCor 数到过的词表词的 `(键, 字面)`，`corroborated` 判断一个字面有没有佐证，`keeps` 是豁免表：其中每一行都必须是一个本来要压低的 essay 补词，否则报错。
pub fn cap_uncorroborated(
    entries: &mut BTreeMap<(String, String), i64>,
    essay_words: &[(String, String)],
    evidenced: &HashSet<(String, String)>,
    corroborated: impl Fn(&str) -> bool,
    keeps: &EssayKeeps,
) -> Result<(Vec<Capped>, usize)> {
    let mut lightest: HashMap<&str, i64> = HashMap::new();
    for entry in evidenced {
        if let Some(&weight) = entries.get(entry) {
            let slot = lightest.entry(entry.0.as_str()).or_insert(weight);
            *slot = (*slot).min(weight);
        }
    }
    let mut caps = Vec::new();
    let mut kept = HashSet::new();
    for entry in essay_words {
        let (key, text) = entry;
        let (Some(&below), Some(&weight)) = (lightest.get(key.as_str()), entries.get(entry)) else {
            continue;
        };
        if weight < below || corroborated(text) {
            continue;
        }
        if keeps.contains(key, text) {
            kept.insert((key.as_str(), text.as_str()));
            continue;
        }
        caps.push(Capped {
            key: key.clone(),
            text: text.clone(),
            from: weight,
            to: (below - 1).max(0),
        });
    }
    if let Some(stale) = keeps
        .entries
        .iter()
        .find(|keep| !kept.contains(&(keep.key.as_str(), keep.word.as_str())))
    {
        bail!(
            "{ESSAY_KEEPS}:{} {} {} is not an essay word this build would hold down: it is not in the essay, the words file lists it, it is corroborated, or no evidenced words-file word on its key outweighs it; remove the line",
            stale.line,
            stale.word,
            stale.key
        );
    }
    for capped in &caps {
        entries.insert((capped.key.clone(), capped.text.clone()), capped.to);
    }
    Ok((caps, kept.len()))
}

/// 豁免表的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EssayKeep {
    pub word: String,
    pub key: String,
    /// 1 起的行号，报错时用。
    pub line: usize,
}

/// `curation/cantonese-essay-keep.txt`，格式见模块文档。
#[derive(Debug, Default)]
pub struct EssayKeeps {
    pub entries: Vec<EssayKeep>,
    readings: HashSet<(String, String)>,
}

impl EssayKeeps {
    /// 去掉首尾空白，跳过空行和 `#` 行。每行必须是 `词<TAB>粤拼`，词至少两个字；同一条写两遍、键不是去声调的音节都算错。
    pub fn parse(source: &str) -> Result<Self> {
        let mut keeps = Self::default();
        for (index, line) in text::without_bom(source).lines().enumerate() {
            let stripped = text::strip(line);
            if stripped.is_empty() || stripped.starts_with('#') {
                continue;
            }
            let number = index + 1;
            let fields: Vec<&str> = stripped.split('\t').map(text::strip).collect();
            let [word, key] = fields[..] else {
                bail!("{ESSAY_KEEPS}:{number}: expected a word and its toneless Jyutping: {stripped:?}");
            };
            if word.chars().nth(1).is_none() {
                bail!("{ESSAY_KEEPS}:{number}: {word:?} is not a word of two or more characters");
            }
            if !is_key(key) {
                bail!("{ESSAY_KEEPS}:{number}: {key:?} is not toneless Jyutping syllables separated by one space");
            }
            if !keeps.readings.insert((key.to_owned(), word.to_owned())) {
                bail!("{ESSAY_KEEPS}:{number}: {word} {key} is listed twice");
            }
            keeps.entries.push(EssayKeep {
                word: word.to_owned(),
                key: key.to_owned(),
                line: number,
            });
        }
        Ok(keeps)
    }

    fn contains(&self, key: &str, text: &str) -> bool {
        self.readings.contains(&(key.to_owned(), text.to_owned()))
    }
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
    /// 后续批量补词仍遵守维护者否决，不把已删除的词重新加回。
    pub fn rejects(&self, key: &str, text: &str) -> bool {
        self.entries.iter().any(|entry| entry.matches(key, text))
    }

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
    fn pinyin_words_are_looked_up_in_simplified_form_without_vetoed_readings() {
        let vetoes = PinyinVetoes::parse("个鸡巴\n我训\two'xun\n").unwrap();
        let traditional = crate::cantonese::parse_variants(
            TRADITIONAL_TO_SIMPLIFIED,
            "假\t假\n個\t个\n濕\t湿\n紙\t纸\n雞\t鸡\n乾\t干 乾\n號\t号\n",
        )
        .unwrap();
        let words = PinyinWords::new(
            &[
                "# 注释\r\n假期\tjia'qi\t6318\r\n湿纸巾\tshi'zhi'jin\t100\r\n个鸡巴\tge'ji'ba\t100\r\n我训\two'xun\t175\n中最\tzhong'zui\t500\n加号\tjia'hao\t100\n",
                "\u{feff}干杯\tgan'bei\t100\n没有权重\tmei'you\n",
            ],
            &traditional,
            &vetoes,
        );
        assert!(words.contains("假期"));
        // 濕紙巾 逐字换成 湿纸巾 之后在拼音库里；乾 取第一个简体字形 干；对照里没有的字原样查，所以 濕紙巾 写成 濕纸巾 也查得到。
        assert!(words.contains("濕紙巾"));
        assert!(words.contains("濕纸巾"));
        assert!(words.contains("乾杯"));
        // 整词否决、按读音否决的词不算拼音库的词。
        assert!(!words.contains("個雞巴"));
        assert!(!words.contains("我訓"));
        // 权重覆盖表不决定一个词在不在库，所以这里不读它：中最、加号 不管在覆盖表里被压低还是抬高，都照算拼音库的词。
        assert!(words.contains("中最"));
        assert!(words.contains("加號"));
        assert!(!words.contains("嘉琪"));
        // 没有权重列的行不是拼音表的行。
        assert!(!words.contains("没有权重"));
    }

    #[test]
    fn uncorroborated_essay_words_fall_below_the_lightest_evidenced_listed_word() {
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
            ("sau faan", "收返", 2847),
            ("sau faan", "壽飯", 8),
            ("gong zo", "講咗", 8809),
            ("gong zo", "講座", 177),
        ]);
        let essay_words = pairs(&[
            ("gaa kei", "嘉琪"),
            ("ngaan fan", "眼訓"),
            ("nei hou", "你好"),
            ("zung hou", "仲好"),
            ("zung hou", "種好"),
            ("sau faan", "收返"),
            ("gong zo", "講咗"),
        ]);
        // 家企、眼睏、壽飯 只有兜底权重，没有证据，不在 `evidenced` 里。
        let evidenced: HashSet<(String, String)> = pairs(&[
            ("gaa kei", "假期"),
            ("ngaan fan", "眼瞓"),
            ("zung hou", "中號"),
            ("gong zo", "講座"),
        ])
        .into_iter()
        .collect();
        let keeps = EssayKeeps::parse("講咗\tgong zo\n").unwrap();
        // 仲好 有佐证（比如 HKCanCor 数到过）；種好 本来就比 中號 轻、不用压。
        let (capped, kept) = cap_uncorroborated(
            &mut dictionary,
            &essay_words,
            &evidenced,
            |text| text == "仲好",
            &keeps,
        )
        .unwrap();
        let weight = |key: &str, text: &str| dictionary[&(key.to_owned(), text.to_owned())];
        assert_eq!(
            capped,
            [
                Capped {
                    key: "gaa kei".to_owned(),
                    text: "嘉琪".to_owned(),
                    from: 30967,
                    to: 6317
                },
                Capped {
                    key: "ngaan fan".to_owned(),
                    text: "眼訓".to_owned(),
                    from: 11924,
                    to: 450
                }
            ]
        );
        assert_eq!(kept, 1);
        assert_eq!(weight("gaa kei", "嘉琪"), 6317);
        assert_eq!(weight("ngaan fan", "眼訓"), 450);
        // 你好 的键上没有词表的词，收返 的键上只有没有证据的 壽飯，都不动；講咗 在豁免表里。
        assert_eq!(weight("nei hou", "你好"), 21493);
        assert_eq!(weight("sau faan", "收返"), 2847);
        assert_eq!(weight("gong zo", "講咗"), 8809);
        assert_eq!(weight("zung hou", "仲好"), 11301);
        assert_eq!(weight("zung hou", "種好"), 700);
        // 有证据的词表词只有 0 分时压到 0，与它并列。
        let mut dictionary = entries(&[("a b", "甲乙", 0), ("a b", "丙丁", 5)]);
        let evidenced: HashSet<(String, String)> = pairs(&[("a b", "甲乙")]).into_iter().collect();
        cap_uncorroborated(
            &mut dictionary,
            &pairs(&[("a b", "丙丁")]),
            &evidenced,
            |_| false,
            &EssayKeeps::default(),
        )
        .unwrap();
        assert_eq!(dictionary[&("a b".to_owned(), "丙丁".to_owned())], 0);
    }

    #[test]
    fn a_keep_line_that_holds_nothing_down_fails_the_build() {
        let mut dictionary = entries(&[
            ("gong zo", "講咗", 8809),
            ("gong zo", "講座", 177),
            ("sau faan", "收返", 2847),
            ("sau faan", "壽飯", 8),
        ]);
        let essay_words = pairs(&[("gong zo", "講咗"), ("sau faan", "收返")]);
        let evidenced: HashSet<(String, String)> =
            pairs(&[("gong zo", "講座")]).into_iter().collect();
        // 收返 的键上没有有证据的词表词，本来就不压，豁免它的行过期了；講咗 有佐证时也一样。
        for (keeps, corroborated, line) in [
            ("講咗\tgong zo\n收返\tsau faan\n", "", ":2 收返 sau faan"),
            ("講咗\tgong zo\n", "講咗", ":1 講咗 gong zo"),
            ("講咗\tgong zo\n講咗\tgong zoo\n", "", ":2 講咗 gong zoo"),
        ] {
            let error = cap_uncorroborated(
                &mut dictionary.clone(),
                &essay_words,
                &evidenced,
                |text| text == corroborated,
                &EssayKeeps::parse(keeps).unwrap(),
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(&format!("curation/cantonese-essay-keep.txt{line} is not an essay word this build would hold down")),
                "{error}"
            );
        }
        // 出错时不改任何条目。
        let before = dictionary.clone();
        let _ = cap_uncorroborated(
            &mut dictionary,
            &essay_words,
            &evidenced,
            |_| false,
            &EssayKeeps::parse("收返\tsau faan\n").unwrap(),
        );
        assert_eq!(dictionary, before);
    }

    #[test]
    fn malformed_keep_lines_and_keep_veto_conflicts_name_their_line() {
        for (source, expected) in [
            (
                "講\tgong\n",
                ":1: \"講\" is not a word of two or more characters",
            ),
            ("講咗\n", ":1: expected a word and its toneless Jyutping"),
            (
                "講咗\tgong2 zo2\n",
                "\"gong2 zo2\" is not toneless Jyutping",
            ),
            (
                "講咗\tgong zo\n講咗\tgong zo\n",
                ":2: 講咗 gong zo is listed twice",
            ),
        ] {
            let error = EssayKeeps::parse(source).unwrap_err().to_string();
            assert!(error.contains(expected), "{source:?}: {error}");
        }
        assert!(EssayKeeps::parse("\u{feff}# 注释\n\n")
            .unwrap()
            .entries
            .is_empty());
        let curation = Curation {
            keeps: EssayKeeps::parse("眼訓\tngaan fan\n").unwrap(),
            vetoes: WordVetoes::parse("眼訓\n").unwrap(),
            ..Curation::default()
        };
        let error = curation.check().unwrap_err().to_string();
        assert!(
            error.contains("curation/cantonese-essay-keep.txt:1 keeps 眼訓 ngaan fan, which curation/cantonese-word-vetoes.txt:1 眼訓 vetoes"),
            "{error}"
        );
        let curation = Curation {
            keeps: EssayKeeps::parse("眼訓\tngaan fan\n").unwrap(),
            vetoes: WordVetoes::parse("眼訓\tngaan fat\n").unwrap(),
            ..Curation::default()
        };
        assert!(curation.check().is_ok());
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
