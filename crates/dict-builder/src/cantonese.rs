//! `msime-cantonese.db`：粤语方案的粤拼词库，按 `msime_engine::language_dictionary` 定义的结构写出。数据是 rime-cantonese（CC BY 4.0，见 `resources/licenses/rime-cantonese-CC-BY-4.0.txt`）在 `resources/dictionary-sources.lock.json` 的 `rime-cantonese` 引用所记提交的文件，由 msime-dictionary 原样收在 `sources/cantonese/` 下；构建从 `--dictionary` checkout 读取，并按它的 `upstream.lock.json` 校验。
//!
//! 读取的上游文件：`jyut6ping3.chars.dict.yaml`（每行一个字、它的带声调读音，以及可选的 `N%`，即这个读音占该字用量的比例）、`jyut6ping3.words.dict.yaml`（每行一个词和它的带声调读音）、`essay-cantonese.txt`（Rime 的词频表，每行 `text<TAB>count`）、`hkcancor-word-counts.txt`（香港粤语语料库 HKCanCor 的词频，CC BY 4.0，由 `hkcancor.rs` 生成到 msime-dictionary，只列两个及以上汉字的词），以及 OpenCC 的 `TWVariants.txt`（`VARIANTS`，只用来列出候选的异体对，见下文）；另外读本项目自己的 `custom/cantonese.txt`（`CUSTOM`，见下文）。`jyut6ping3.maps.dict.yaml` 是 ODbL，`jyut6ping3.phrase.dict.yaml` 没有读音、来源也不清楚，两者都不固定、不读取；`jyut6ping3.lettered.dict.yaml` 也不读。
//!
//! 方案输入不带声调的粤拼，所以每个读音都去掉声调数字，条目的键是去声调的音节以一个空格连接（`nei hou`）。只差声调的读音合成一个条目，取最大的权重。词的权重是它的 essay 分数。essay 以书面语为主，词表里约五分之二的词它没有收，最常用的口语词也在其中（我哋、佢哋、嗰啲、食咗）。这类词若语料有计数，权重是语料次数乘以 `corpus_scale`，即两者都收的词上 essay 分数与语料次数之比的中位数，把语料次数换算到 essay 的尺度（我哋在语料里出现 446 次，落在點解附近）。语料次数不到 `CORPUS_MIN_COUNT` 的词压到同键 essay 所列最轻的词之下一位，免得只出现过一次的异写或切分偶然（物証之于物證）排到 essay 收录的词前面。其余没收的词按 `unlisted_word_weight` 加权：1 加上它最罕用的字的 essay 分数以 2 为底的对数，这样同音词按用字的常见程度排序，又远低于 essay 给收录词的分数（大多至少 100）。字的权重是它的 essay 分数，行里给了比例时按比例折算，最低为 1。
//!
//! 字表、词表和 essay 用 OpenCC 标准字形（爲、衆），HKCanCor 用香港实际书写的字形（為、眾）。构建时把 OpenCC 字形换成香港字形，换哪些字由数据决定：异体对取自 OpenCC 的 `TWVariants.txt`（msime-dictionary 的 `sources/opencc/` 原样收录），一对里只有 HKCanCor 词频表用另一形、完全不用 OpenCC 形时才换（`hong_kong_forms`）；两形语料都没有的对、两形都用的对（着/著）、香港仍用 OpenCC 形的对（裏/裡）都不动。词频表不收单字词，判定只看两个及以上汉字的词：语料里单独成词的字不算，固定提交的 HKCanCor 里「羣」单独出现过 1 次，按词频表仍换成「群」。这张表不收 說/説、溫/温、戶/户、衛/衞 这类《常用字字形表》的异体，它们本来就不在考虑之内。替换只改写入的字面：字的权重仍按原字面查 essay，词、essay 补词和人工词表按替换后的字面查 essay 与语料（`essay_by_hong_kong_form` 把各写法的 essay 分数合到香港字形上，HKCanCor 本来就是香港字形），合并后的两行由 `Dictionary::insert` 取较大权重，所以「為」继承「爲」的 184291，「因爲」写成「因為」。单字另外保留 OpenCC 字形：「峯」「羣」这类字本身也常用（人名里尤其多），粤语方案又没有学习数据，换掉就打不出来，所以 OpenCC 字形的单字行照样写入，权重取它自己的分数与香港字形权重减一中较小的那个（至少为 1），「爲」得 184290，紧排在「為」下面；词只写香港字形。
//!
//! The words file lacks some of the commonest words because Rime composes them from characters at typing time (你好 is one), and the engine does not compose sentences. So an essay word the words file does not list becomes an entry too when the essay counts it at least `ESSAY_WORD_MIN_COUNT` times and each of its characters has exactly one toneless reading in the characters file, which makes its key certain.
//!
//! 上面两条规则都不会收进 essay 分数不到门槛、或含多音字的词（周杰倫只有 711 分）。这类词由 msime-dictionary 的 `custom/cantonese.txt` 逐词收录：每行 `词<TAB>带声调粤拼`，一个汉字一个音节，逐行人工审校。构建要求每个字在这一行里的读音（去声调后）是字表给这个字的读音之一，否则整个构建失败：这拦得住笔误和字根本没有的读音（把「倫」写成 `lun4`），拦不住在多音字的几个读音里选错（「銀行」写成 `ngan4 haang4` 能通过，因为「行」确有 `haang4`），后者只能靠人工审校。收录的词按 essay 分数加权，essay 没有列它时取 `ESSAY_WORD_MIN_COUNT`；已有条目只升不降，不删除任何条目。checkout 里没有这个文件（加入它之前的 msime-dictionary 提交）时按空表构建。
//!
//! The syllable inventory is every toneless syllable some key uses, so every entry stays typable; the words file uses a few colloquial syllables (`fi`, `za`, `gwek`) that no character row has.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};

pub const CHARACTERS: &str = "sources/cantonese/jyut6ping3.chars.dict.yaml";
pub const WORDS: &str = "sources/cantonese/jyut6ping3.words.dict.yaml";
pub const ESSAY: &str = "sources/cantonese/essay-cantonese.txt";
pub const CORPUS: &str = crate::hkcancor::OUTPUT;
/// 人工审校的粤语词表，本项目自己的数据（CC BY 4.0，与 msime-cantonese.db 的其余来源同一许可），不是上游数据，不在 `upstream.lock.json` 里，内容由 checkout 的提交固定。
pub const CUSTOM: &str = "custom/cantonese.txt";
/// OpenCC 的臺灣异体字表（Apache-2.0），只用来列出候选的异体对，哪一对替换由 HKCanCor 决定；表里的字不写进词库，词库的许可不变。
pub const VARIANTS: &str = "sources/opencc/TWVariants.txt";
/// The sources lock reference whose commit is recorded as the database's `source_commit`.
pub const REFERENCE: &str = "rime-cantonese";
/// The SPDX identifier recorded as the database's `license`.
pub const LICENSE: &str = "CC-BY-4.0";
pub const DATABASE: &str = "msime-cantonese.db";
/// The licence text in `resources/licenses/` and the name it ships under beside the database.
pub const LICENSE_SOURCE: &str = "rime-cantonese-CC-BY-4.0.txt";
pub const LICENSE_NAME: &str = "msime-rime_cantonese_LICENSE.txt";

/// The essay count from which a word missing from the words file is added. The essay's counts are scaled scores rather than raw counts (the median word of the words file scores 900), and 1000 keeps about five thousand frequent words such as 你好 while leaving out the long tail of novel names and segmentation fragments the essay also lists.
pub const ESSAY_WORD_MIN_COUNT: i64 = 1000;

/// The corpus count from which a word may rank ahead of a word the essay lists on the same key. The corpus has about 160 thousand tokens, so a word seen once or twice there is as likely a variant spelling (物証, 遊盪) or a segmentation accident (唔知 is split while 唔止 is kept whole) as a common word; below this count its corpus weight is held one below the lightest essay-listed word of its key.
pub const CORPUS_MIN_COUNT: i64 = 3;

/// The floors `verify` enforces on a release build.
pub const FLOORS: Floors = Floors {
    syllables: 600,
    characters: 25_000,
    words: 90_000,
};

/// Keys whose top three entries must include the text, as a check against gross ranking errors.
pub const EXPECTED: [(&str, &str); 4] = [
    ("nei hou", "你好"),
    ("gwong dung waa", "廣東話"),
    ("wai", "為"),
    ("jan wai", "因為"),
];

#[derive(Debug, Clone, Copy)]
pub struct Floors {
    pub syllables: usize,
    pub characters: usize,
    pub words: usize,
}

/// One row of the characters or words file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<'a> {
    pub text: &'a str,
    /// Toneless syllables.
    pub syllables: Vec<&'a str>,
    /// The row's share of the character's use in percent, when the row gives one.
    pub percent: Option<i64>,
}

#[derive(Debug, Default)]
pub struct Dictionary {
    pub syllables: BTreeSet<String>,
    /// `(key, text)` to weight.
    pub entries: BTreeMap<(String, String), i64>,
    /// Entries that came from the characters file, the words file and the essay.
    pub characters: usize,
    pub words: usize,
    pub essay_words: usize,
    /// Words-file entries the essay does not list that took their weight from the corpus, and the factor that put the corpus counts on the essay's scale.
    pub corpus_words: usize,
    pub corpus_scale: i64,
    /// Corpus-weighted words below `CORPUS_MIN_COUNT` whose weight was held below an essay-listed word of their key.
    pub corpus_capped: usize,
    /// `custom/cantonese.txt` 里新增了条目或抬高了已有条目权重的行数。
    pub custom_words: usize,
}

/// `custom/cantonese.txt` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomWord<'a> {
    pub text: &'a str,
    /// 行里写的带声调粤拼，原样保留，供 `check-words` 的报告显示。
    pub reading: &'a str,
    /// 去掉声调的音节，一个汉字一个。
    pub syllables: Vec<&'a str>,
}

impl CustomWord<'_> {
    /// 词库里的键：去声调音节以空格连接。
    pub fn key(&self) -> String {
        self.syllables.join(" ")
    }
}

/// 字表给每个字的去声调读音（两个音节的单位字读音里带空格）。
pub type Readings<'a> = HashMap<&'a str, BTreeSet<String>>;

/// 一个换成香港字形的 OpenCC 字形：香港字形，以及它在 HKCanCor 词表里出现的次数（含它的词的计数之和；OpenCC 字形按规则为 0）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HongKongForm {
    pub hong_kong: char,
    pub corpus_count: i64,
}

/// OpenCC 字形到香港字形，按 OpenCC 字形排序。
pub type HongKongForms = BTreeMap<char, HongKongForm>;

/// `TWVariants.txt`：`字<TAB>异体[ 异体…]`，`#` 注释行与空行跳过。键和每个值都必须是一个字。
pub fn parse_variants(source: &str) -> Result<Vec<(char, Vec<char>)>> {
    let one = |text: &str| {
        let mut characters = text.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) => Some(character),
            _ => None,
        }
    };
    let mut pairs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parsed = line.split_once('\t').and_then(|(key, values)| {
            let values: Option<Vec<char>> = values.split(' ').map(one).collect();
            Some((one(key)?, values?))
        });
        let Some(pair) = parsed else {
            bail!(
                "{VARIANTS}: line {} is not character<TAB>character[ character...]",
                index + 1
            );
        };
        pairs.push(pair);
    }
    Ok(pairs)
}

/// 要换成香港字形的 OpenCC 字形。一个字在 HKCanCor 里的次数是含它的词的计数之和（这份词表只列两字及以上的词）。一对异体里 OpenCC 字形（键）在语料里出现过就不换；没出现过时，换成语料里出现过的值中次数最多的那个（并列取表里先列的），一个值都没出现过也不换。
pub fn hong_kong_forms(
    variants: &[(char, Vec<char>)],
    corpus: &HashMap<&str, i64>,
) -> HongKongForms {
    let mut counts: HashMap<char, i64> = HashMap::new();
    for (word, count) in corpus {
        for character in word.chars() {
            *counts.entry(character).or_insert(0) += count;
        }
    }
    let count = |character: &char| counts.get(character).copied().unwrap_or(0);
    let mut forms = HongKongForms::new();
    for (opencc, values) in variants {
        if count(opencc) > 0 {
            continue;
        }
        let mut best: Option<HongKongForm> = None;
        for value in values.iter().filter(|value| *value != opencc) {
            let corpus_count = count(value);
            if corpus_count > best.map_or(0, |form| form.corpus_count) {
                best = Some(HongKongForm {
                    hong_kong: *value,
                    corpus_count,
                });
            }
        }
        if let Some(form) = best {
            forms.insert(*opencc, form);
        }
    }
    forms
}

/// `text` 的香港字形：逐字替换 `forms` 里的 OpenCC 字形，没有要换的字时原样借用。
pub fn hong_kong_text<'a>(text: &'a str, forms: &HongKongForms) -> Cow<'a, str> {
    if !text.chars().any(|character| forms.contains_key(&character)) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .map(|character| {
                forms
                    .get(&character)
                    .map_or(character, |form| form.hong_kong)
            })
            .collect(),
    )
}

/// essay 分数按香港字形合并：几种写法换成同一个香港字形时取最大的分数。
fn essay_by_hong_kong_form<'a>(
    essay: &HashMap<&'a str, i64>,
    forms: &HongKongForms,
) -> HashMap<Cow<'a, str>, i64> {
    let mut merged: HashMap<Cow<'a, str>, i64> = HashMap::with_capacity(essay.len());
    for (&text, &count) in essay {
        let slot = merged.entry(hong_kong_text(text, forms)).or_insert(count);
        *slot = (*slot).max(count);
    }
    merged
}

/// The rows after the YAML header, which ends at the `...` line, with their 1-based line numbers. Blank lines and `#` comments are skipped.
fn body<'a>(name: &str, source: &'a str) -> Result<impl Iterator<Item = (usize, &'a str)>> {
    let mut lines = source.lines().enumerate();
    if !lines.by_ref().any(|(_, line)| line == "...") {
        bail!("{name}: no `...` line ends the YAML header");
    }
    Ok(lines
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(index, line)| (index + 1, line)))
}

/// 一个带声调粤拼音节（`nei5`）去掉声调后的字母；不是小写字母加 1 到 6 的声调时为 `None`。
fn strip_tone(syllable: &str) -> Option<&str> {
    syllable
        .strip_suffix(|c: char| ('1'..='6').contains(&c))
        .filter(|letters| !letters.is_empty() && letters.bytes().all(|b| b.is_ascii_lowercase()))
}

/// The toneless syllables of a space-separated toned reading such as `nei5 hou2`.
fn toneless<'a>(name: &str, line: usize, reading: &'a str) -> Result<Vec<&'a str>> {
    reading
        .split(' ')
        .map(|syllable| {
            strip_tone(syllable).with_context(|| {
                format!("{name}: line {line}: {syllable:?} is not a toned Jyutping syllable")
            })
        })
        .collect()
}

/// `jyut6ping3.chars.dict.yaml`: `character<TAB>reading[<TAB>N%]`.
pub fn parse_characters(source: &str) -> Result<Vec<Row<'_>>> {
    let mut rows = Vec::new();
    for (line, row) in body(CHARACTERS, source)? {
        let fields: Vec<&str> = row.split('\t').collect();
        let (text, reading, percent) = match fields[..] {
            [text, reading] => (text, reading, None),
            [text, reading, percent] => (text, reading, Some(percent)),
            _ => bail!("{CHARACTERS}: line {line} is not character<TAB>reading[<TAB>N%]"),
        };
        if text.chars().count() != 1 {
            bail!("{CHARACTERS}: line {line}: {text:?} is not one character");
        }
        // A few unit characters read as two syllables (兡 baak3 hak1, a hectogram).
        let syllables = toneless(CHARACTERS, line, reading)?;
        let percent = percent
            .map(|percent| {
                percent
                    .strip_suffix('%')
                    .and_then(|number| number.parse::<i64>().ok())
                    .filter(|number| (0..=100).contains(number))
                    .with_context(|| {
                        format!("{CHARACTERS}: line {line}: {percent:?} is not a percentage")
                    })
            })
            .transpose()?;
        rows.push(Row {
            text,
            syllables,
            percent,
        });
    }
    Ok(rows)
}

/// `jyut6ping3.words.dict.yaml`: `word<TAB>reading`.
pub fn parse_words(source: &str) -> Result<Vec<Row<'_>>> {
    let mut rows = Vec::new();
    for (line, row) in body(WORDS, source)? {
        let fields: Vec<&str> = row.split('\t').collect();
        let [text, reading] = fields[..] else {
            bail!("{WORDS}: line {line} is not word<TAB>reading");
        };
        if text.is_empty() {
            bail!("{WORDS}: line {line} has no word");
        }
        rows.push(Row {
            text,
            syllables: toneless(WORDS, line, reading)?,
            percent: None,
        });
    }
    Ok(rows)
}

/// `essay-cantonese.txt`: `text<TAB>count`. A text listed twice keeps its larger count.
pub fn parse_essay(source: &str) -> Result<HashMap<&str, i64>> {
    let mut counts = HashMap::new();
    for (index, row) in source.lines().enumerate() {
        if row.is_empty() {
            continue;
        }
        let parsed = row.split_once('\t').and_then(|(text, count)| {
            Some((text, count.parse::<i64>().ok().filter(|count| *count >= 0)?))
        });
        let Some((text, count)) = parsed.filter(|(text, _)| !text.is_empty()) else {
            bail!("{ESSAY}: line {} is not text<TAB>count", index + 1);
        };
        let kept = counts.entry(text).or_insert(count);
        *kept = (*kept).max(count);
    }
    Ok(counts)
}

/// `custom/cantonese.txt` 在 `--dictionary` checkout 里的路径。checkout 里还没有这个文件（加入它之前的 msime-dictionary 提交）时为 `None`，构建按空表进行并在摘要里写明；文件存在时与其他 checkout 文件一样经 `Sources::pinned` 检查路径。
pub fn custom_source(sources: &crate::sources::Sources) -> Result<Option<std::path::PathBuf>> {
    let Some(path) = sources.checkout_file(CUSTOM) else {
        return Ok(None);
    };
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        _ => sources.pinned(CUSTOM).map(Some),
    }
}

/// 每个字在字表里的去声调读音。
pub fn character_readings<'a>(characters: &[Row<'a>]) -> Readings<'a> {
    let mut readings: Readings = HashMap::new();
    for row in characters {
        readings
            .entry(row.text)
            .or_default()
            .insert(row.syllables.join(" "));
    }
    readings
}

/// `custom/cantonese.txt` 的一行：空行和 `#` 注释行为 `None`，格式不对时返回不带位置的错误。要求两个及以上汉字、每字一个带声调音节；读音是否与字表一致由 `check_custom_reading` 另查。`check-words` 用同一个函数检查投稿的行。
pub fn parse_custom_line(line: &str) -> Result<Option<CustomWord<'_>>> {
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let fields: Vec<&str> = line.split('\t').collect();
    let [text, reading] = fields[..] else {
        bail!("expected word<TAB>toned Jyutping, got {line:?}");
    };
    if text.is_empty() {
        bail!("the word is empty");
    }
    let syllables = reading
        .split(' ')
        .map(|syllable| {
            strip_tone(syllable)
                .with_context(|| format!("{syllable:?} is not a toned Jyutping syllable"))
        })
        .collect::<Result<Vec<_>>>()?;
    let characters = text.chars().count();
    if characters < 2 {
        bail!("{text:?} is a single character; character readings come from the characters file");
    }
    if syllables.len() != characters {
        bail!(
            "{text:?} has {characters} characters but {} syllables",
            syllables.len()
        );
    }
    Ok(Some(CustomWord {
        text,
        reading,
        syllables,
    }))
}

/// 每个字在这一行里的读音（去声调后）必须是字表给它的读音之一；字表没有这个字也算错。
pub fn check_custom_reading(word: &CustomWord, readings: &Readings) -> Result<()> {
    let mut buffer = [0; 4];
    for (character, syllable) in word.text.chars().zip(&word.syllables) {
        let character = &*character.encode_utf8(&mut buffer);
        let Some(known) = readings.get(character) else {
            bail!("{character} is not in {CHARACTERS}");
        };
        if !known.contains(*syllable) {
            let known: Vec<&str> = known.iter().map(String::as_str).collect();
            bail!(
                "{character} does not read {syllable} in {CHARACTERS}, which gives {}",
                known.join(", ")
            );
        }
    }
    Ok(())
}

/// `custom/cantonese.txt`：逐行解析并对照字表核对读音，同一个词同一个键写了两次也算错。任何一行不对都让构建失败，错误写明行号。
pub fn parse_custom<'a>(source: &'a str, readings: &Readings) -> Result<Vec<CustomWord<'a>>> {
    let mut words = Vec::new();
    let mut seen: HashMap<(String, &str), usize> = HashMap::new();
    for (index, line) in crate::text::without_bom(source).lines().enumerate() {
        let number = index + 1;
        let located = |error: anyhow::Error| anyhow::anyhow!("{CUSTOM}: line {number}: {error}");
        let Some(word) = parse_custom_line(line).map_err(located)? else {
            continue;
        };
        check_custom_reading(&word, readings).map_err(located)?;
        if let Some(previous) = seen.insert((word.key(), word.text), number) {
            bail!("{CUSTOM}: line {number} repeats line {previous}");
        }
        words.push(word);
    }
    Ok(words)
}

impl Dictionary {
    fn insert(&mut self, key: String, text: &str, weight: i64) {
        let slot = self.entries.entry((key, text.to_owned())).or_insert(weight);
        *slot = (*slot).max(weight);
    }
}

/// The weight of a word the essay does not list: 1 plus the base-2 logarithm of the essay count of its rarest character, or 1 when one of its characters is not counted at all. Real counts top out near 2^24, so this stays below 64 and under almost every count the essay gives a listed word.
fn unlisted_word_weight(text: &str, essay: &HashMap<&str, i64>) -> i64 {
    let mut buffer = [0; 4];
    let rarest = text
        .chars()
        .map(|character| {
            essay
                .get(&*character.encode_utf8(&mut buffer))
                .copied()
                .unwrap_or(0)
        })
        .min()
        .unwrap_or(0);
    1 + rarest.checked_ilog2().map_or(0, i64::from)
}

/// essay 与语料都列出的词上 `essay 分数 / 语料次数`（整除）的下中位数，没有共同的词时为 1。essay 按香港字形合并后再对照，语料里的「因為」能对上 essay 的「因爲」。
fn corpus_scale(essay: &HashMap<Cow<str>, i64>, corpus: &HashMap<&str, i64>) -> i64 {
    let mut ratios: Vec<i64> = corpus
        .iter()
        .filter_map(|(&word, count)| Some(essay.get(word)? / count))
        .collect();
    ratios.sort_unstable();
    ratios
        .get(ratios.len().saturating_sub(1) / 2)
        .copied()
        .unwrap_or(1)
        .max(1)
}

/// 四份输入与 `custom/cantonese.txt` 的人工审校词（读音已由 `parse_custom` 核对）构成的词库，写入的字面按 `forms` 换成香港字形。
pub fn build(
    characters: &[Row],
    words: &[Row],
    essay: &HashMap<&str, i64>,
    corpus: &HashMap<&str, i64>,
    custom: &[CustomWord],
    forms: &HongKongForms,
) -> Dictionary {
    // 词、essay 补词与人工词表都按香港字形查 essay；字仍按原字面查，见下。
    let essay_hk = essay_by_hong_kong_form(essay, forms);
    let essay_count = |text: &str| essay_hk.get(text).copied();
    let mut dictionary = Dictionary {
        corpus_scale: corpus_scale(&essay_hk, corpus),
        ..Dictionary::default()
    };
    // Each character's toneless keys, to give an essay word a key when each of its characters has only one.
    let readings = character_readings(characters);
    // 换了字形的单字行：OpenCC 字形本身也是一个字（人名里的「峯」「羣」），仍要打得出，等香港字形的权重定下来后排在它下面一位。
    let mut kept_opencc = Vec::new();
    for row in characters {
        // 字的分数按原字面查：essay 里「參」的分数涵盖它的所有读音，不能拿来给「蔘」那一行。
        let count = essay.get(row.text).copied().unwrap_or(0);
        let weight = row
            .percent
            .map_or(count, |percent| count.saturating_mul(percent) / 100)
            .max(1);
        let key = row.syllables.join(" ");
        let text = hong_kong_text(row.text, forms);
        if text != row.text {
            kept_opencc.push((key.clone(), row.text, text.to_string(), weight));
        }
        dictionary.insert(key, &text, weight);
    }
    // 不取下限 1：「爲」按 1 分会落在 wai 的第二百多位，超出引擎的 SPAN_LIMIT，又看不到了（#10420）。
    for (key, opencc, hong_kong, weight) in kept_opencc {
        let below = dictionary.entries[&(key.clone(), hong_kong)] - 1;
        dictionary.insert(key, opencc, weight.min(below).max(1));
    }
    dictionary.characters = dictionary.entries.len();

    // 已经由词表收录的词（香港字形），essay 补词时跳过。
    let mut listed: HashSet<Cow<str>> = HashSet::new();
    let mut corpus_weighted = HashSet::new();
    // The lightest essay-listed word on each key, and the thinly attested corpus words with their fallback weights, which are held below it once the essay words below are added too.
    let mut lightest_essay: HashMap<String, i64> = HashMap::new();
    let mut thin = Vec::new();
    for row in words {
        let key = row.syllables.join(" ");
        let text = hong_kong_text(row.text, forms);
        let weight = match (essay_count(&text), corpus.get(&*text)) {
            (Some(count), _) => {
                let lightest = lightest_essay.entry(key.clone()).or_insert(count);
                *lightest = (*lightest).min(count);
                count
            }
            (None, Some(&count)) => {
                corpus_weighted.insert(text.clone());
                let fallback = unlisted_word_weight(row.text, essay);
                if count < CORPUS_MIN_COUNT {
                    thin.push((key.clone(), text.to_string(), fallback));
                }
                count.saturating_mul(dictionary.corpus_scale).max(fallback)
            }
            (None, None) => unlisted_word_weight(row.text, essay),
        };
        dictionary.insert(key, &text, weight);
        listed.insert(text);
    }
    dictionary.corpus_words = corpus_weighted.len();
    dictionary.words = dictionary.entries.len() - dictionary.characters;

    // essay 补词：键按原字面的读音推出（「蔘」只读 sam，换成的「參」有好几个读音），写入香港字形；几种写法换成同一个词时先合并，取最大的分数。
    let mut essay_words: BTreeMap<(String, Cow<str>), i64> = BTreeMap::new();
    for (&text, &count) in essay {
        if count < ESSAY_WORD_MIN_COUNT || text.chars().count() < 2 {
            continue;
        }
        let hong_kong = hong_kong_text(text, forms);
        if listed.contains(&hong_kong) {
            continue;
        }
        let keys: Option<Vec<&str>> = text
            .chars()
            .map(|character| {
                let mut buffer = [0; 4];
                let readings = readings.get(&*character.encode_utf8(&mut buffer))?;
                let mut only = readings.iter();
                match (only.next(), only.next()) {
                    (Some(key), None) => Some(key.as_str()),
                    _ => None,
                }
            })
            .collect();
        if let Some(keys) = keys {
            let slot = essay_words
                .entry((keys.join(" "), hong_kong))
                .or_insert(count);
            *slot = (*slot).max(count);
        }
    }
    for ((key, text), count) in essay_words {
        let lightest = lightest_essay.entry(key.clone()).or_insert(count);
        *lightest = (*lightest).min(count);
        dictionary.insert(key, &text, count);
    }
    dictionary.essay_words = dictionary.entries.len() - dictionary.characters - dictionary.words;

    // A reviewed word the essay lists becomes an essay-listed word of its key even below `ESSAY_WORD_MIN_COUNT`, so the thin corpus words of that key are held below it too.
    for word in custom {
        if let Some(count) = essay_count(&hong_kong_text(word.text, forms)) {
            let lightest = lightest_essay.entry(word.key()).or_insert(count);
            *lightest = (*lightest).min(count);
        }
    }

    for (key, text, fallback) in thin {
        let Some(&lightest) = lightest_essay.get(&key) else {
            continue;
        };
        let weight = dictionary
            .entries
            .get_mut(&(key, text))
            .expect("every thin corpus word was inserted");
        let capped = (*weight).min(lightest - 1).max(fallback);
        if capped < *weight {
            *weight = capped;
            dictionary.corpus_capped += 1;
        }
    }

    // 人工审校的词放在最后，只新增或抬高，不压低前面任何来源给的权重。
    for word in custom {
        let text = hong_kong_text(word.text, forms);
        let weight = essay_count(&text).unwrap_or(ESSAY_WORD_MIN_COUNT).max(1);
        let slot = dictionary
            .entries
            .entry((word.key(), text.into_owned()))
            .or_insert(0);
        if *slot < weight {
            *slot = weight;
            dictionary.custom_words += 1;
        }
    }

    dictionary.syllables = dictionary
        .entries
        .keys()
        .flat_map(|(key, _)| key.split(' '))
        .map(str::to_owned)
        .collect();
    dictionary
}

/// Writes `dictionary` to `path` in the engine's language dictionary schema, replacing any earlier file, and freezes it so the same inputs give the same bytes.
pub fn write(dictionary: &Dictionary, path: &Path, source_commit: &str) -> Result<()> {
    crate::language_writer::write(
        path,
        source_commit,
        LICENSE,
        dictionary.syllables.iter().map(String::as_str),
        &dictionary.entries,
    )
}

/// What `verify` counted in a written database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub syllables: usize,
    pub characters: usize,
    pub words: usize,
}

/// Opens a written database the way the engine does and fails when it is below `floors` or a key of `EXPECTED` lacks its text in the top three.
pub fn verify(path: &Path, floors: Floors) -> Result<Counts> {
    let (dictionary, connection) = crate::language_verify::open(path)?;
    let count = |sql| crate::language_verify::count(&connection, sql);
    let counts = Counts {
        syllables: count("SELECT count(*) FROM syllables")?,
        characters: count("SELECT count(*) FROM entries WHERE length(text) = 1")?,
        words: count("SELECT count(*) FROM entries WHERE length(text) > 1")?,
    };
    crate::language_verify::check_floors(
        path,
        &[
            ("syllables", counts.syllables, floors.syllables),
            ("character entries", counts.characters, floors.characters),
            ("word entries", counts.words, floors.words),
        ],
    )?;
    for (key, text) in EXPECTED {
        let top = dictionary
            .lookup(key, 3)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !top.iter().any(|entry| entry.text == text) {
            let found: Vec<&str> = top.iter().map(|entry| entry.text.as_str()).collect();
            bail!(
                "{}: {key:?} offers {found:?} first, without {text}",
                path.display()
            );
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_engine::language_dictionary;

    const HEADER: &str = "# Rime dictionary\n# encoding: utf-8\n\n---\nname: jyut6ping3.chars\nversion: \"2026.08.10\"\nsort: by_weight\n...\n\n";

    fn characters_source() -> String {
        format!("{HEADER}你\tnei5\n妳\tnei5\n好\thou2\n好\thou3\t40%\n號\thou6\t0%\n廣\tgwong2\n東\tdung1\n話\twaa6\t80%\n話\twaa2\t20%\n呢\tne1\n呢\tnei4\t5%\n爲\twai4\n為\twai4\t5%\n因\tjan1\n")
    }

    fn words_source() -> String {
        format!("{HEADER}廣東話\tgwong2 dung1 waa2\n廣東話\tgwong2 dung1 waa6\n你哋\tnei5 dei6\nfi li fe le\tfi4 li1 fe4 le4\n因爲\tjan1 wai4\n")
    }

    const ESSAY_TEXT: &str =
        "你\t7000\n好\t5000\n話\t900\n呢\t2000\n廣東話\t6851\n你好\t21493\n好呢\t3000\n你哋\t40\n號\t10\n你好\t5\n爲\t184291\n因爲\t713185\n";

    fn entries(dictionary: &Dictionary) -> Vec<(&str, &str, i64)> {
        dictionary
            .entries
            .iter()
            .map(|((key, text), weight)| (key.as_str(), text.as_str(), *weight))
            .collect()
    }

    fn sample() -> Dictionary {
        let characters = characters_source();
        let words = words_source();
        build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &parse_essay(ESSAY_TEXT).unwrap(),
            &HashMap::new(),
            &[],
            &sample_forms(),
        )
    }

    /// 只把「爲」换成「為」，让样例库满足 `EXPECTED` 里的 ("wai", "為") 与 ("jan wai", "因為")。
    fn sample_forms() -> HongKongForms {
        HongKongForms::from([(
            '爲',
            HongKongForm {
                hong_kong: '為',
                corpus_count: 1,
            },
        )])
    }

    #[test]
    fn rows_after_the_header_lose_their_tones() {
        let characters = characters_source();
        let rows = parse_characters(&characters).unwrap();
        assert_eq!(rows.len(), 14);
        assert_eq!(
            rows[3],
            Row {
                text: "好",
                syllables: vec!["hou"],
                percent: Some(40)
            }
        );
        let words = words_source();
        let rows = parse_words(&words).unwrap();
        assert_eq!(rows[0].syllables, ["gwong", "dung", "waa"]);
        assert_eq!(rows[3].text, "fi li fe le");
    }

    #[test]
    fn weights_follow_the_essay_and_the_share_of_use() {
        let dictionary = sample();
        assert_eq!(
            entries(&dictionary),
            [
                ("dung", "東", 1),
                ("fi li fe le", "fi li fe le", 1),
                ("gwong", "廣", 1),
                ("gwong dung waa", "廣東話", 6851),
                // 好 is hou2 (5000) and hou3 at 40% (2000); the tones collapse into the larger weight.
                ("hou", "好", 5000),
                // 0% of 10 floors at 1.
                ("hou", "號", 1),
                ("jan", "因", 1),
                // 因爲写成因為，沿用它的 essay 分数。
                ("jan wai", "因為", 713185),
                ("ne", "呢", 2000),
                ("nei", "你", 7000),
                // 5% of 2000.
                ("nei", "呢", 100),
                ("nei", "妳", 1),
                ("nei dei", "你哋", 40),
                // Every character of 你好 has one toneless reading and the essay counts it often enough; the larger of its two counts wins.
                ("nei hou", "你好", 21493),
                // 80% of 900, the larger share of waa2 and waa6.
                ("waa", "話", 720),
                // 為自己的 5% 份额没有 essay 分数，合并后取爲的 184291；爲仍单列，排在為下面一位。
                ("wai", "為", 184291),
                ("wai", "爲", 184290),
            ]
        );
        // 好呢 is frequent but 呢 reads ne or nei, so its key is uncertain and it stays out.
        assert!(!dictionary.entries.keys().any(|(_, text)| text == "好呢"));
        assert_eq!(
            (
                dictionary.characters,
                dictionary.words,
                dictionary.essay_words
            ),
            (12, 4, 1)
        );
    }

    #[test]
    fn unlisted_homophone_words_follow_their_rarest_character() {
        let characters = format!("{HEADER}食\tsik6\n識\tsik1\n咗\tzo2\n");
        let words = format!("{HEADER}識咗\tsik1 zo2\n食咗\tsik6 zo2\n");
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &parse_essay("食\t873763\n識\t379707\n咗\t1995799\n").unwrap(),
            &HashMap::new(),
            &[],
            &HongKongForms::new(),
        );
        // 食 (2^19..2^20) is commoner than 識 (2^18..2^19), so 食咗 outweighs 識咗 although the essay lists neither word.
        assert_eq!(
            dictionary.entries[&("sik zo".to_owned(), "食咗".to_owned())],
            20
        );
        assert_eq!(
            dictionary.entries[&("sik zo".to_owned(), "識咗".to_owned())],
            19
        );
    }

    #[test]
    fn the_corpus_weighs_words_the_essay_lacks_on_the_essay_scale() {
        let characters = format!(
            "{HEADER}我\tngo5\n哋\tdei6\n地\tdei6\n點\tdim2\n解\tgaai2\n佢\tkeoi5\n咩\tme1\n"
        );
        let words = format!("{HEADER}我哋\tngo5 dei6\n我地\tngo5 dei6\n點解\tdim2 gaai2\n佢哋\tkeoi5 dei6\n咩哋\tme1 dei6\n");
        let essay = parse_essay("我\t900000\n哋\t1000000\n地\t800000\n點解\t480000\n我地\t55000\n佢\t500000\n咩\t300000\n").unwrap();
        // 點解 and 我地 are in both lists: their ratios are 480000/200 = 2400 and 55000/50 = 1100, and the lower median of the two is 1100.
        let corpus = crate::hkcancor::parse("點解\t200\n我地\t50\n我哋\t400\n佢哋\t300\n").unwrap();
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &essay,
            &corpus,
            &[],
            &HongKongForms::new(),
        );
        assert_eq!(dictionary.corpus_scale, 1100);
        let weight = |key: &str, text: &str| dictionary.entries[&(key.to_owned(), text.to_owned())];
        // The essay lists 我地 and 點解, so their essay counts stand.
        assert_eq!(weight("ngo dei", "我地"), 55000);
        assert_eq!(weight("dim gaai", "點解"), 480000);
        // 我哋 is missing from the essay; 400 corpus tokens at 1100 each put it ahead of 我地.
        assert_eq!(weight("ngo dei", "我哋"), 440000);
        assert_eq!(weight("keoi dei", "佢哋"), 330000);
        // 咩哋 is in neither list and keeps the fallback, 1 + log2(300000).
        assert_eq!(weight("me dei", "咩哋"), 19);
        assert_eq!(dictionary.corpus_words, 2);
    }

    #[test]
    fn a_thin_corpus_word_stays_below_the_essay_words_of_its_key() {
        let characters = format!(
            "{HEADER}物\tmat6\n證\tzing3\n証\tzing3\n人\tjan4\n印\tjan3\n唔\tm4\n止\tzi2\n只\tzi2\n知\tzi1\n"
        );
        let words = format!(
            "{HEADER}物證\tmat6 zing3\n物証\tmat6 zing3\n人證\tjan4 zing3\n印證\tjan3 zing3\n人証\tjan4 zing3\n唔止\tm4 zi2\n唔知\tm4 zi1\n"
        );
        // 唔只 is not in the words file; the essay adds it under m zi, and it is the key's only essay word besides 唔知.
        let essay = parse_essay(
            "物\t600000\n證\t300000\n証\t2000\n人\t900000\n印\t200000\n唔\t2000000\n止\t400000\n只\t500000\n知\t800000\n物證\t718\n人證\t580\n印證\t889\n唔知\t1518\n唔只\t5606\n點解\t480000\n",
        )
        .unwrap();
        let corpus = crate::hkcancor::parse("點解\t568\n物証\t2\n人証\t2\n唔止\t3\n").unwrap();
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &essay,
            &corpus,
            &[],
            &HongKongForms::new(),
        );
        assert_eq!(dictionary.corpus_scale, 845);
        let weight = |key: &str, text: &str| dictionary.entries[&(key.to_owned(), text.to_owned())];
        // Two tokens would weigh 1690, ahead of 物證; they are held one below it.
        assert_eq!(weight("mat zing", "物証"), 717);
        // jan zing has 人證 (580) and 印證 (889); a thin word stays below both.
        assert_eq!(weight("jan zing", "人証"), 579);
        // Three tokens reach the threshold and keep the corpus weight, which passes 唔知 but not 唔只, the essay word added under the same key.
        assert_eq!(weight("m zi", "唔止"), 2535);
        assert_eq!(weight("m zi", "唔只"), 5606);
        assert_eq!((dictionary.corpus_words, dictionary.corpus_capped), (3, 2));
    }

    #[test]
    fn a_thin_corpus_word_never_drops_below_its_fallback() {
        let characters = format!("{HEADER}噉\tgam2\n解\tgaai2\n禁\tgam3\n戒\tgaai3\n");
        let words = format!("{HEADER}噉解\tgam2 gaai2\n禁戒\tgam3 gaai3\n");
        // 點解 sets the scale to 845. 禁戒 weighs 3 in the essay, below 噉解's fallback of 1 + log2(1024) = 11, so 噉解 drops from 845 to its fallback and no further.
        let essay =
            parse_essay("噉\t1024\n解\t4096\n禁\t9000\n戒\t9000\n禁戒\t3\n點解\t845000\n").unwrap();
        let corpus = crate::hkcancor::parse("噉解\t1\n點解\t1000\n").unwrap();
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &essay,
            &corpus,
            &[],
            &HongKongForms::new(),
        );
        assert_eq!(
            dictionary.entries[&("gam gaai".to_owned(), "噉解".to_owned())],
            11
        );
        assert_eq!(dictionary.corpus_capped, 1);
    }

    #[test]
    fn without_shared_words_the_corpus_scale_is_one() {
        let essay = parse_essay("你\t10\n").unwrap();
        let corpus = crate::hkcancor::parse("我哋\t4\n").unwrap();
        let essay = essay_by_hong_kong_form(&essay, &HongKongForms::new());
        assert_eq!(corpus_scale(&essay, &corpus), 1);
    }

    #[test]
    fn a_unit_character_keeps_its_two_syllables() {
        let characters = format!("{HEADER}兡\tbaak3 hak1\n一\tjat1\n");
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &[],
            &parse_essay("一兡\t1200\n").unwrap(),
            &HashMap::new(),
            &[],
            &HongKongForms::new(),
        );
        assert_eq!(
            entries(&dictionary),
            [
                ("baak hak", "兡", 1),
                ("jat", "一", 1),
                ("jat baak hak", "一兡", 1200)
            ]
        );
    }

    #[test]
    fn rare_essay_words_are_left_out() {
        let characters = characters_source();
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &[],
            &parse_essay("你好\t999\n").unwrap(),
            &HashMap::new(),
            &[],
            &HongKongForms::new(),
        );
        assert_eq!(dictionary.essay_words, 0);
    }

    const CUSTOM_CHARACTERS: &str = "周\tzau1\n杰\tgit6\n傑\tgit6\n倫\tleon4\n你\tnei5\n好\thou2\n";

    #[test]
    fn reviewed_words_take_their_essay_count_or_the_threshold_and_only_raise() {
        let characters = format!("{HEADER}{CUSTOM_CHARACTERS}");
        let characters = parse_characters(&characters).unwrap();
        let readings = character_readings(&characters);
        let source = "\u{feff}# 人工审校的粤语词\n\n周杰倫\tzau1 git6 leon4\n傑倫\tgit6 leon4\n你好\tnei5 hou2\n";
        let custom = parse_custom(source, &readings).unwrap();
        assert_eq!(
            custom[0],
            CustomWord {
                text: "周杰倫",
                reading: "zau1 git6 leon4",
                syllables: vec!["zau", "git", "leon"],
            }
        );
        let dictionary = build(
            &characters,
            &[],
            &parse_essay("周杰倫\t711\n杰倫\t1182\n你好\t21493\n").unwrap(),
            &HashMap::new(),
            &custom,
            &HongKongForms::new(),
        );
        let weight = |key: &str, text: &str| {
            dictionary
                .entries
                .get(&(key.to_owned(), text.to_owned()))
                .copied()
        };
        // essay 给了周杰倫 711 分，不到补词门槛，现在照这个分数收进来。
        assert_eq!(weight("zau git leon", "周杰倫"), Some(711));
        // essay 没有列傑倫，取门槛值。
        assert_eq!(weight("git leon", "傑倫"), Some(ESSAY_WORD_MIN_COUNT));
        // 你好已由 essay 补进来，权重更高，不被压低。
        assert_eq!(weight("nei hou", "你好"), Some(21493));
        assert_eq!(dictionary.custom_words, 2);
        assert!(dictionary.syllables.contains("leon"));
    }

    #[test]
    fn a_thin_corpus_word_stays_below_a_reviewed_essay_word_of_its_key() {
        let characters = format!("{HEADER}{CUSTOM_CHARACTERS}");
        let characters = parse_characters(&characters).unwrap();
        let readings = character_readings(&characters);
        let custom = parse_custom("周杰倫\tzau1 git6 leon4\n", &readings).unwrap();
        let words = format!("{HEADER}周傑倫\tzau1 git6 leon4\n");
        // 點解 sets the scale to 845, so one corpus token of 周傑倫 would weigh 845, ahead of the reviewed 周杰倫 at 711.
        let essay = parse_essay("周杰倫\t711\n點解\t845000\n").unwrap();
        let corpus = crate::hkcancor::parse("周傑倫\t1\n點解\t1000\n").unwrap();
        let dictionary = build(
            &characters,
            &parse_words(&words).unwrap(),
            &essay,
            &corpus,
            &custom,
            &HongKongForms::new(),
        );
        assert_eq!(dictionary.corpus_scale, 845);
        let weight = |text: &str| dictionary.entries[&("zau git leon".to_owned(), text.to_owned())];
        assert_eq!(weight("周杰倫"), 711);
        assert_eq!(weight("周傑倫"), 710);
        assert_eq!(dictionary.corpus_capped, 1);
    }

    #[test]
    fn reviewed_words_must_read_as_the_characters_file_does() {
        let characters = format!("{HEADER}{CUSTOM_CHARACTERS}");
        let characters = parse_characters(&characters).unwrap();
        let readings = character_readings(&characters);
        for (source, expected) in [
            (
                "周杰倫\tzau1 git6 lun4\n",
                "custom/cantonese.txt: line 1: 倫 does not read lun in sources/cantonese/jyut6ping3.chars.dict.yaml, which gives leon",
            ),
            (
                "# 注释\n周杰倫\tzau1 git6\n",
                "line 2: \"周杰倫\" has 3 characters but 2 syllables",
            ),
            ("周\tzau1\n", "\"周\" is a single character"),
            ("周杰倫\tzau git6 leon4\n", "\"zau\" is not a toned"),
            ("周杰倫\n", "expected word<TAB>toned Jyutping"),
            ("周杰倫\tzau1 git6 leon4\t711\n", "expected word<TAB>toned Jyutping"),
            ("\tzau1 git6\n", "the word is empty"),
            ("周杰哦\tzau1 git6 o1\n", "哦 is not in sources/cantonese"),
            (
                "周杰倫\tzau1 git6 leon4\n周杰倫\tzau2 git6 leon4\n",
                "custom/cantonese.txt: line 2 repeats line 1",
            ),
        ] {
            let error = parse_custom(source, &readings).unwrap_err().to_string();
            assert!(error.contains(expected), "{source:?}: {error}");
        }
        // 同一个读音下的另一种写法是另一个条目，不算重复。
        let both = "周杰倫\tzau1 git6 leon4\n周傑倫\tzau1 git6 leon4\n";
        assert_eq!(parse_custom(both, &readings).unwrap().len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn a_checkout_without_the_reviewed_list_builds_without_it() {
        use crate::sources::{Dictionary as Checkout, Lock, Reference, Sources};
        let checkout = tempfile::tempdir().unwrap();
        std::fs::write(
            checkout.path().join(crate::sources::UPSTREAM_LOCK),
            r#"{"version": 1, "upstreams": {}, "files": []}"#,
        )
        .unwrap();
        let lock = Lock {
            references: BTreeMap::new(),
            mozc: Reference {
                repository: "https://github.com/google/mozc".into(),
                commit: "a".repeat(40),
            },
            files: Vec::new(),
        };
        let dictionary = Checkout::open(checkout.path().to_path_buf(), &lock).unwrap();
        let sources = Sources {
            lock,
            repository_inputs: checkout.path().join("unused"),
            cache: checkout.path().join("unused"),
            offline: true,
            dictionary: Some(dictionary),
        };
        assert_eq!(custom_source(&sources).unwrap(), None);
        std::fs::create_dir(checkout.path().join("custom")).unwrap();
        let path = checkout.path().join(CUSTOM);
        std::fs::write(&path, "周杰倫\tzau1 git6 leon4\n").unwrap();
        assert_eq!(custom_source(&sources).unwrap(), Some(path.clone()));
        // 文件存在时照常检查路径：符号链接不读。
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(checkout.path().join(crate::sources::UPSTREAM_LOCK), &path)
            .unwrap();
        assert!(custom_source(&sources).is_err());
    }

    /// `TWVariants.txt` 的真实行，外加一条表里没有的 `說\t説`，用来说明香港语料照用的字形不会被换掉。
    const VARIANTS_TEXT: &str = "# Open Chinese Convert (OpenCC) Dictionary\n\n僞\t偽\n爲\t為\n# @reverse-prefer: 梁\n梁\t梁\n樑\t梁 樑\n着\t著\n衆\t眾\n裏\t裡\n麼\t麼 么\n說\t説\n";

    #[test]
    fn only_forms_the_corpus_uses_instead_of_the_opencc_form_are_replaced() {
        let variants = parse_variants(VARIANTS_TEXT).unwrap();
        assert_eq!(variants[3], ('樑', vec!['梁', '樑']));
        let corpus = crate::hkcancor::parse(
            "因為\t30\n認為\t5\n群眾\t4\n跟着\t3\n睇著\t2\n呢裏\t6\n說話\t6\n梁生\t2\n",
        )
        .unwrap();
        let forms = hong_kong_forms(&variants, &corpus);
        let form = |hong_kong, corpus_count| HongKongForm {
            hong_kong,
            corpus_count,
        };
        // 着、裏、說在语料里照用 OpenCC 字形，不换；僞/偽、麼/么两形语料都没有，也不换；樑有两个值，换成语料用过的梁。
        assert_eq!(
            forms.into_iter().collect::<Vec<_>>(),
            [
                ('樑', form('梁', 2)),
                ('爲', form('為', 35)),
                ('衆', form('眾', 4))
            ]
        );
        assert_eq!(
            hong_kong_text("因爲衆人", &hong_kong_forms(&variants, &corpus)),
            "因為眾人"
        );
        assert!(matches!(
            hong_kong_text("着裏說", &hong_kong_forms(&variants, &corpus)),
            Cow::Borrowed("着裏說")
        ));
    }

    #[test]
    fn malformed_variant_rows_name_their_line() {
        for source in ["爲為\n", "爲\t為 \n", "爲\t為為\n", "爲爲\t為\n", "\t為\n"] {
            let error = parse_variants(source).unwrap_err().to_string();
            assert!(
                error.contains("TWVariants.txt: line 1 is not"),
                "{source:?}: {error}"
            );
        }
    }

    #[test]
    fn opencc_forms_hand_their_weights_to_the_hong_kong_forms() {
        let characters = format!(
            "{HEADER}爲\twai4\n為\twai4\t5%\n為\twai6\t5%\n爲\twai6\t5%\n因\tjan1\n人\tjan4\n着\tzoek6\n裏\tleoi5\n面\tmin6\n說\tsyut3\n僞\tngai6\n偽\tngai6\n"
        );
        let words = format!("{HEADER}因爲\tjan1 wai4\n裏面\tleoi5 min6\n僞造\tngai6 zou6\n");
        let essay = parse_essay(
            "爲\t184291\n因\t50000\n人\t90000\n因爲\t713185\n着\t61756\n裏\t61311\n面\t9000\n裏面\t20000\n說\t100000\n僞\t2624\n爲人\t1500\n為人\t1200\n",
        )
        .unwrap();
        let corpus = crate::hkcancor::parse("因為\t300\n着數\t6\n呢裏\t6\n說話\t6\n").unwrap();
        let forms = hong_kong_forms(&parse_variants(VARIANTS_TEXT).unwrap(), &corpus);
        assert_eq!(forms.keys().collect::<String>(), "爲");
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &essay,
            &corpus,
            &[],
            &forms,
        );
        let weight = |key: &str, text: &str| {
            dictionary
                .entries
                .get(&(key.to_owned(), text.to_owned()))
                .copied()
        };
        // 「為」继承「爲」的 essay 分数；单字「爲」仍保留，紧排在「為」下面一位，不掉到下限 1。
        assert_eq!(weight("wai", "為"), Some(184291));
        assert_eq!(weight("wai", "爲"), Some(184290));
        assert_eq!(weight("jan wai", "因為"), Some(713185));
        assert_eq!(weight("jan wai", "因爲"), None);
        // essay 里两种写法合成一个词，取较大的分数。
        assert_eq!(weight("wai jan", "為人"), Some(1500));
        assert_eq!(weight("wai jan", "爲人"), None);
        // 着、裏、說照旧；僞/偽两形语料都没有，保持原样。
        assert_eq!(weight("zoek", "着"), Some(61756));
        assert_eq!(weight("leoi", "裏"), Some(61311));
        assert_eq!(weight("leoi min", "裏面"), Some(20000));
        assert_eq!(weight("syut", "說"), Some(100000));
        assert_eq!(weight("ngai", "僞"), Some(2624));
        assert_eq!(weight("ngai", "偽"), Some(1));
        assert!(weight("ngai zou", "僞造").is_some());
        // 语料的「因為」对上 essay 的「因爲」，是唯一共有的词：713185 / 300。
        assert_eq!(dictionary.corpus_scale, 2377);
        assert_eq!(dictionary.characters, 10);
    }

    #[test]
    fn both_spellings_of_a_word_merge_at_the_essay_count() {
        let characters = format!("{HEADER}峯\tfung1\n峰\tfung1\n高\tgou1\n夠\tgau3\n");
        // 词表同时列出高峯（essay 600）与高峰（语料只出现 2 次），两行换成同一个香港字形。
        let words = format!("{HEADER}高峯\tgou1 fung1\n高峰\tgou1 fung1\n");
        let essay =
            parse_essay("峯\t4372\n峰\t7741\n高\t90000\n高峯\t600\n點解\t845000\n").unwrap();
        let corpus = crate::hkcancor::parse("高峰\t2\n點解\t1000\n").unwrap();
        let forms = hong_kong_forms(&parse_variants("峯\t峰\n").unwrap(), &corpus);
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &essay,
            &corpus,
            &[],
            &forms,
        );
        let weight = |text: &str| {
            dictionary
                .entries
                .get(&("gou fung".to_owned(), text.to_owned()))
                .copied()
        };
        // 两行都按香港字形查到 essay 的 600，合成一条；高峰不按语料的 2 次加权，也就不会被当成薄语料词压低。
        assert_eq!(weight("高峰"), Some(600));
        assert_eq!(weight("高峯"), None);
        assert_eq!(dictionary.corpus_capped, 0);
        // 字按原字面查 essay，合并后取较大值。
        assert_eq!(
            dictionary.entries[&("fung".to_owned(), "峰".to_owned())],
            7741
        );
        // 单字「峯」本来就比「峰」轻，保留自己的分数。
        assert_eq!(
            dictionary.entries[&("fung".to_owned(), "峯".to_owned())],
            4372
        );
    }

    #[test]
    fn the_inventory_holds_every_syllable_a_key_uses() {
        let dictionary = sample();
        let syllables: Vec<&str> = dictionary.syllables.iter().map(String::as_str).collect();
        // fe, fi, le and li come only from the words file, dei from 你哋.
        assert_eq!(
            syllables,
            [
                "dei", "dung", "fe", "fi", "gwong", "hou", "jan", "le", "li", "ne", "nei", "waa",
                "wai"
            ]
        );
    }

    #[test]
    fn malformed_rows_name_their_file_and_line() {
        for (source, expected) in [
            (format!("{HEADER}你\n"), "chars.dict.yaml: line 10 is not"),
            (
                format!("{HEADER}你好\tnei5\n"),
                "line 10: \"你好\" is not one character",
            ),
            (
                format!("{HEADER}你\tnei\n"),
                "line 10: \"nei\" is not a toned",
            ),
            (
                format!("{HEADER}你\tNei5\n"),
                "line 10: \"Nei5\" is not a toned",
            ),
            (
                format!("{HEADER}你\tnei5\t3\n"),
                "\"3\" is not a percentage",
            ),
            (
                format!("{HEADER}你\tnei5\t101%\n"),
                "\"101%\" is not a percentage",
            ),
            ("你\tnei5\n".to_owned(), "no `...` line"),
        ] {
            let error = parse_characters(&source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        let error = parse_words(&format!("{HEADER}\n你好\tnei5 hou2\t3\n"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("words.dict.yaml: line 11"), "{error}");
        let error = parse_words(&format!("{HEADER}你好\tnei5  hou2\n"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("\"\" is not a toned"), "{error}");
        for essay in ["你好\n", "你好\t-1\n", "\t3\n", "你好\t3\t4\n"] {
            let error = parse_essay(essay).unwrap_err().to_string();
            assert!(error.contains("essay-cantonese.txt: line 1"), "{error}");
        }
    }

    #[test]
    fn only_the_licensed_files_are_read() {
        for excluded in ["maps", "phrase", "lettered"] {
            assert!(
                [CHARACTERS, WORDS, ESSAY, CORPUS, CUSTOM, VARIANTS]
                    .iter()
                    .all(|input| !input.contains(excluded)),
                "{excluded}"
            );
        }
        let lock = crate::sources::Lock::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap();
        assert_eq!(lock.references[REFERENCE].commit.len(), 40);
        for input in [CHARACTERS, WORDS, ESSAY] {
            assert_eq!(
                crate::sources::upstream_reference(input),
                Some(REFERENCE),
                "{input}"
            );
        }
        assert_eq!(
            crate::sources::upstream_reference(CORPUS),
            Some(crate::hkcancor::REFERENCE)
        );
        // 异体字表是 OpenCC 的原样文件，按 OpenCC 的上游记录校验。
        assert_eq!(crate::sources::upstream_reference(VARIANTS), Some("OpenCC"));
        // 人工词表是本项目自己的数据，不按上游记录校验。
        assert_eq!(crate::sources::upstream_reference(CUSTOM), None);
        assert!(!lock
            .files
            .iter()
            .any(|file| file.path.starts_with("sources/cantonese/")));
    }

    fn written(dictionary: &Dictionary) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DATABASE);
        write(
            dictionary,
            &path,
            "259f0e48bba840c3a2e0d117539e96937f3d89bc",
        )
        .unwrap();
        (dir, path)
    }

    #[test]
    fn the_engine_reads_what_is_written_and_the_bytes_repeat() {
        let dictionary = sample();
        let (_dir, path) = written(&dictionary);
        let engine = language_dictionary::open_read_only(&path).unwrap();
        let texts: Vec<String> = engine
            .lookup("nei", 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.text)
            .collect();
        assert_eq!(texts, ["你", "呢", "妳"]);
        assert!(engine.has_syllable("fi").unwrap());
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_SOURCE_COMMIT)
                .unwrap()
                .as_deref(),
            Some("259f0e48bba840c3a2e0d117539e96937f3d89bc")
        );
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_LICENSE)
                .unwrap()
                .as_deref(),
            Some(LICENSE)
        );
        let first = std::fs::read(&path).unwrap();
        // Writing again over the earlier file gives the same bytes.
        write(
            &dictionary,
            &path,
            "259f0e48bba840c3a2e0d117539e96937f3d89bc",
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
    }

    #[test]
    fn verify_enforces_the_floors_and_the_expected_entries() {
        let (_dir, path) = written(&sample());
        let floors = Floors {
            syllables: 13,
            characters: 12,
            words: 5,
        };
        assert_eq!(
            verify(&path, floors).unwrap(),
            Counts {
                syllables: 13,
                characters: 12,
                words: 5
            }
        );
        for (below, what) in [
            (
                Floors {
                    syllables: 14,
                    ..floors
                },
                "13 syllables, below the floor of 14",
            ),
            (
                Floors {
                    characters: 13,
                    ..floors
                },
                "12 character entries, below the floor of 13",
            ),
            (
                Floors { words: 6, ..floors },
                "5 word entries, below the floor of 6",
            ),
        ] {
            let error = verify(&path, below).unwrap_err().to_string();
            assert!(error.contains(what), "{error}");
        }
        let error = verify(&path, FLOORS).unwrap_err().to_string();
        assert!(error.contains("below the floor"), "{error}");

        // Without the essay, 你好 is missing from nei hou.
        let characters = characters_source();
        let words = words_source();
        let (_dir, path) = written(&build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
            &HongKongForms::new(),
        ));
        let error = verify(
            &path,
            Floors {
                syllables: 0,
                characters: 0,
                words: 0,
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("without 你好"), "{error}");
    }
}
