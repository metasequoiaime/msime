//! msime-dictionary 顶层目录 `curation/` 下由维护者审定的表：拼音否决表、拼音权重覆盖表、不拿来拼整句的粗俗词表和英文否决表。上游文件逐字节锁定、`custom/` 只追加，所以删词、改权重都写成这里的独立表，由构建器在读入之后施加。
//!
//! 这些表都可以不存在：文件不在就是空表，拼音、五笔两库和各生成文件与没有这套机制时逐字节相同，所以构建器可以先于数据合入。`msime-english.db` 例外：构建器内置的英文蔑称（[`BUILT_IN_VETOES`] 和 `english_supplement::OFFENSIVE`）不靠 `curation/` 也总是生效，见 [`EnglishVetoes`]。表里的每一条都必须在本次输入里匹配到东西，一条也匹配不到就让构建失败（照 `pinyin-reading-corrections.txt` 的做法），免得输入变了、表却悄悄过期。格式和各阶段的位置见 `crates/dict-builder/README.md` 的「Curation tables」。

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::msime::{self, CustomWord};
use crate::places_supplement::weighted_rows;
use crate::sources::{sha256_file, Sources};
use crate::text;

/// 拼音否决表：整词删除，或按「词+全拼」删除。
pub const PINYIN_VETOES: &str = "curation/pinyin-vetoes.txt";
/// 拼音权重覆盖表：把一个读音的权重设成给定的值。
pub const PINYIN_WEIGHT_OVERRIDES: &str = "curation/pinyin-weight-overrides.txt";
/// 不拿来拼整句的粗俗词表，写进 `msime-pinyin.db` 的 `sentence_excluded`。
pub const OFFENSIVE_WORDS: &str = "curation/offensive-words.txt";
/// 英文否决表：这些词不进 `english_words`，任何来源、任何大小写都不进。
pub const ENGLISH_VETOES: &str = "curation/english-vetoes.txt";

/// 读 checkout 里的一张表；文件不在时为 `None`。
pub fn read_optional(sources: &Sources, path: &str) -> Result<Option<String>> {
    sources
        .optional(path)?
        .map(|resolved| text::read(&resolved))
        .transpose()
}

/// 存在的表及其 SHA-256，按给出的顺序；生成器把它们写进表头的输入列表，不存在的表不写，没有表时表头与以前相同。
pub fn present_digests(
    sources: &Sources,
    paths: &[&'static str],
) -> Result<Vec<(&'static str, String)>> {
    let mut digests = Vec::new();
    for path in paths {
        if let Some(resolved) = sources.optional(path)? {
            digests.push((*path, sha256_file(&resolved)?));
        }
    }
    Ok(digests)
}

/// `curation/` 里影响拼音和五笔的两张表。
pub const PINYIN_TABLES: [&str; 2] = [PINYIN_VETOES, PINYIN_WEIGHT_OVERRIDES];

/// 读进来的一行，`line` 是 1 起的行号，报错时用。
fn location(path: &str, line: usize) -> String {
    format!("{path}:{line}")
}

/// 表里有内容的行：去掉首尾空白，跳过空行和 `#` 开头的行，带 1 起的行号。
fn entries(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text::splitlines(text::without_bom(text))
        .into_iter()
        .enumerate()
        .map(|(index, line)| (index + 1, text::strip(line)))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
}

/// 否决表的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinyinVeto {
    pub word: String,
    /// `None` 是整词否决：这个词的所有读音，以及五笔码表里的这个词。
    pub key: Option<String>,
    pub line: usize,
}

impl PinyinVeto {
    fn describe(&self) -> String {
        match &self.key {
            Some(key) => format!("{} {key}", self.word),
            None => self.word.clone(),
        }
    }
}

/// `curation/pinyin-vetoes.txt`：一行 `词` 或 `词<TAB>全拼`。
#[derive(Debug, Default)]
pub struct PinyinVetoes {
    pub entries: Vec<PinyinVeto>,
    words: HashSet<String>,
    readings: HashSet<(String, String)>,
}

impl PinyinVetoes {
    /// 整词否决的词至少两个字：删掉一个单字的全部读音不会是本意。读音写成 `'` 分隔、能落进某张全拼表的全拼。同一条写两遍，或者同一个词既整词否决又按读音否决，都算错。
    pub fn parse(text: &str) -> Result<Self> {
        let mut vetoes = Self::default();
        for (line, stripped) in entries(text) {
            let at = || location(PINYIN_VETOES, line);
            let fields: Vec<&str> = stripped.split('\t').map(text::strip).collect();
            let (word, key) = match fields[..] {
                [word] => (word, None),
                [word, key] => (word, Some(key)),
                _ => bail!(
                    "{}: expected a word, or a word and its pinyin: {stripped:?}",
                    at()
                ),
            };
            if word.is_empty() {
                bail!("{}: the word is empty", at());
            }
            match key {
                None => {
                    if word.chars().count() < 2 {
                        bail!("{}: {word} is a single character; veto one of its readings instead of the character", at());
                    }
                    if !vetoes.words.insert(word.to_owned()) {
                        bail!("{}: {word} is listed twice", at());
                    }
                }
                Some(key) => {
                    if !msime::is_quanpin(key) {
                        bail!("{}: {key:?} is not quanpin separated by \"'\"", at());
                    }
                    if !vetoes.readings.insert((word.to_owned(), key.to_owned())) {
                        bail!("{}: {word} {key} is listed twice", at());
                    }
                }
            }
            vetoes.entries.push(PinyinVeto {
                word: word.to_owned(),
                key: key.map(str::to_owned),
                line,
            });
        }
        if let Some(redundant) = vetoes
            .entries
            .iter()
            .find(|entry| entry.key.is_some() && vetoes.words.contains(&entry.word))
        {
            bail!(
                "{}: {} is vetoed as a whole word, so its reading veto is redundant",
                location(PINYIN_VETOES, redundant.line),
                redundant.describe()
            );
        }
        Ok(vetoes)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 这一行拼音（词 `word`，全拼 `key`）是否被否决。
    pub fn vetoes(&self, word: &str, key: &str) -> bool {
        self.words.contains(word) || self.readings.contains(&(word.to_owned(), key.to_owned()))
    }

    /// 整词否决的词：在五笔码表和五笔词组补全里也不出现。
    pub fn vetoes_word(&self, word: &str) -> bool {
        self.words.contains(word)
    }
}

/// 权重覆盖表的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightOverride {
    pub word: String,
    pub key: String,
    pub weight: i64,
    pub line: usize,
}

/// `curation/pinyin-weight-overrides.txt`：一行 `词<TAB>全拼<TAB>权重`。
#[derive(Debug, Default)]
pub struct WeightOverrides {
    pub entries: Vec<WeightOverride>,
    weights: HashMap<(String, String), i64>,
}

impl WeightOverrides {
    /// 权重是不小于 0 的整数；同一个词和读音写两遍算错。
    pub fn parse(text: &str) -> Result<Self> {
        let mut overrides = Self::default();
        for (line, stripped) in entries(text) {
            let at = || location(PINYIN_WEIGHT_OVERRIDES, line);
            let fields: Vec<&str> = stripped.split('\t').map(text::strip).collect();
            let [word, key, weight] = fields[..] else {
                bail!("{}: expected word, pinyin and weight: {stripped:?}", at());
            };
            if word.is_empty() {
                bail!("{}: the word is empty", at());
            }
            if !msime::is_quanpin(key) {
                bail!("{}: {key:?} is not quanpin separated by \"'\"", at());
            }
            let weight: i64 = weight
                .parse()
                .with_context(|| format!("{}: weight {weight:?} is not an integer", at()))?;
            if weight < 0 {
                bail!("{}: weight {weight} is below 0", at());
            }
            if overrides
                .weights
                .insert((word.to_owned(), key.to_owned()), weight)
                .is_some()
            {
                bail!("{}: {word} {key} is listed twice", at());
            }
            overrides.entries.push(WeightOverride {
                word: word.to_owned(),
                key: key.to_owned(),
                weight,
                line,
            });
        }
        Ok(overrides)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, word: &str, key: &str) -> Option<i64> {
        self.weights
            .get(&(word.to_owned(), key.to_owned()))
            .copied()
    }
}

/// 拼音的两张表，构建的 `word-vetoes`、`weight-overrides` 阶段和五笔、区划补充表的生成器读的是同一份。
#[derive(Debug, Default)]
pub struct PinyinCuration {
    pub vetoes: PinyinVetoes,
    pub overrides: WeightOverrides,
}

impl PinyinCuration {
    /// 一个读音不能既被否决又被改权重：否决在前，覆盖就永远匹配不到。
    pub fn parse(vetoes: &str, overrides: &str) -> Result<Self> {
        let curation = Self {
            vetoes: PinyinVetoes::parse(vetoes)?,
            overrides: WeightOverrides::parse(overrides)?,
        };
        if let Some(conflict) = curation
            .overrides
            .entries
            .iter()
            .find(|entry| curation.vetoes.vetoes(&entry.word, &entry.key))
        {
            bail!(
                "{}: {} {} is also vetoed in {PINYIN_VETOES}; keep one of the two",
                location(PINYIN_WEIGHT_OVERRIDES, conflict.line),
                conflict.word,
                conflict.key
            );
        }
        Ok(curation)
    }

    /// 从 `--dictionary` checkout 读两张表，不在的按空表。
    pub fn load(sources: &Sources) -> Result<Self> {
        Self::parse(
            &read_optional(sources, PINYIN_VETOES)?.unwrap_or_default(),
            &read_optional(sources, PINYIN_WEIGHT_OVERRIDES)?.unwrap_or_default(),
        )
    }

    /// 像 [`weighted_rows`] 那样读 `value<TAB>key<TAB>weight` 行，去掉被否决的行，覆盖表里有的读音换成覆盖后的权重。生成器看到的就是构建在 `word-vetoes`、`weight-overrides` 之后的样子。
    pub fn rows<'a>(
        &'a self,
        source: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a str, i64)> + 'a {
        weighted_rows(source)
            .filter(|(value, key, _)| !self.vetoes.vetoes(value, key))
            .map(|(value, key, weight)| {
                (value, key, self.overrides.get(value, key).unwrap_or(weight))
            })
    }

    /// 整词否决的词，见 [`PinyinVetoes::vetoes_word`]。
    pub fn vetoes_word(&self, word: &str) -> bool {
        self.vetoes.vetoes_word(word)
    }
}

/// 五笔原表（86 极点表、两张 98 表）的每一行：`(文件, 编码, 词)`，按文件和行的先后，按构建读它们的方式解析。整词否决在这些表里也生效：`word-vetoes` 判断只在五笔里出现的整词否决是否过期、`cleanse-report` 找五笔原表里的粗俗碎片，读的都是它。新世纪表只收单字，整词否决不收单字，所以不读它。
pub fn wubi_source_rows(sources: &Sources) -> Result<Vec<(&'static str, String, String)>> {
    use crate::{
        wubi86_supplement::JIDIAN, wubi98_supplement::WUBI98, wubi98_supplement::WUBI98_FCITX,
    };
    let mut rows = Vec::new();
    let jidian = text::read(&sources.pinned(JIDIAN)?)?;
    for line in text::universal_lines(text::without_bom(&jidian)) {
        if let Some((code, value, _)) = msime::parse_code_line(line, false) {
            rows.push((JIDIAN, code, value.to_owned()));
        }
    }
    let wubi98_path = sources.pinned(WUBI98)?;
    let wubi98 = msime::decode_utf16le(&crate::sources::read_private(&wubi98_path)?)
        .with_context(|| format!("decoding {}", wubi98_path.display()))?;
    for line in text::universal_lines(text::without_bom(&wubi98)) {
        if let Some((code, value)) = msime::parse_wubi98_line(line) {
            rows.push((WUBI98, code, value.to_owned()));
        }
    }
    let fcitx = text::read(&sources.pinned(WUBI98_FCITX)?)?;
    for line in text::universal_lines(&fcitx) {
        if let Some((code, value)) = msime::parse_fcitx_wubi98_line(line) {
            rows.push((WUBI98_FCITX, code.to_owned(), value));
        }
    }
    Ok(rows)
}

/// `word-vetoes` 阶段的结果。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct VetoCounts {
    /// 从全拼表删掉的行。
    pub removed: usize,
    /// 全拼表里没有、只在五笔码表里出现的整词否决。
    pub wubi_only: usize,
}

/// 从全拼表删掉否决表里的行。整词否决删这个词的所有读音；按读音否决只删那一张表里的那一行。一条否决在全拼表里一行都没删到，而且（整词否决时）五笔码表里也没有这个词（`wubi_words` 只在需要时才算），就是过期了，阶段失败，表不动。
pub fn apply_word_vetoes(
    connection: &mut Connection,
    vetoes: &PinyinVetoes,
    wubi_words: impl FnOnce() -> Result<HashSet<String>>,
) -> Result<VetoCounts> {
    let mut counts = VetoCounts::default();
    let transaction = connection.transaction()?;
    let mut matched: HashSet<usize> = HashSet::new();
    for (index, entry) in vetoes.entries.iter().enumerate() {
        let Some(key) = &entry.key else {
            continue;
        };
        let table = msime::pinyin_table(key).context("a parsed veto maps to a quanpin table")?;
        let deleted = transaction.execute(
            &format!("delete from {table} where key = ? and value = ?"),
            params![key, entry.word],
        )?;
        if deleted > 0 {
            matched.insert(index);
        }
        counts.removed += deleted;
    }
    let whole: Vec<(usize, &PinyinVeto)> = vetoes
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.key.is_none())
        .collect();
    if !whole.is_empty() {
        transaction.execute_batch(
            "DROP TABLE IF EXISTS temp.curation_vetoed; CREATE TEMP TABLE curation_vetoed(value TEXT PRIMARY KEY) WITHOUT ROWID;",
        )?;
        {
            let mut insert =
                transaction.prepare("INSERT INTO temp.curation_vetoed(value) VALUES (?)")?;
            for (_, entry) in &whole {
                insert.execute([&entry.word])?;
            }
        }
        let mut found: HashSet<String> = HashSet::new();
        for table in msime::quanpin_tables() {
            {
                let mut statement = transaction.prepare(&format!(
                    "SELECT DISTINCT value FROM {table} WHERE value IN (SELECT value FROM temp.curation_vetoed)"
                ))?;
                let values = statement.query_map([], |row| row.get::<_, String>(0))?;
                for value in values {
                    found.insert(value?);
                }
            }
            counts.removed += transaction.execute(
                &format!(
                    "DELETE FROM {table} WHERE value IN (SELECT value FROM temp.curation_vetoed)"
                ),
                [],
            )?;
        }
        transaction.execute_batch("DROP TABLE temp.curation_vetoed;")?;
        for (index, entry) in &whole {
            if found.contains(&entry.word) {
                matched.insert(*index);
            }
        }
        if whole.iter().any(|(index, _)| !matched.contains(index)) {
            let wubi = wubi_words()?;
            for (index, entry) in &whole {
                if !matched.contains(index) && wubi.contains(&entry.word) {
                    matched.insert(*index);
                    counts.wubi_only += 1;
                }
            }
        }
    }
    let stale: Vec<String> = vetoes
        .entries
        .iter()
        .enumerate()
        .filter(|(index, _)| !matched.contains(index))
        .map(|(_, entry)| format!("{} (line {})", entry.describe(), entry.line))
        .collect();
    if !stale.is_empty() {
        bail!(
            "{PINYIN_VETOES}: nothing to remove for {} (drop the entry if no input has the word or reading any more)",
            stale.join(", ")
        );
    }
    transaction.commit()?;
    Ok(counts)
}

/// `weight-overrides` 阶段的结果。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct OverrideCounts {
    pub changed: usize,
    /// 行已经是这个权重。
    pub unchanged: usize,
}

/// `custom/words.txt` 从已有的行提了权的读音：词和全拼 → （提权前的权重，custom 的权重）。只看 `listed` 里的条目（覆盖表也列了的那些），`earlier` 是 custom-words 阶段之前并入全拼表的输入（单字表带它的白名单、词组输入、区划补充表），按构建读它们的方式取同词同音的最高权重。custom 新插入的词（之前的输入没有这一行）和没有提权的词（之前的权重不低于 custom 的）都不在结果里。
pub fn custom_promotions<'a>(
    listed: impl IntoIterator<Item = &'a CustomWord>,
    earlier: &[(&str, Option<&HashSet<String>>)],
) -> HashMap<(String, String), (i64, i64)> {
    let listed: HashMap<(&str, &str), i64> = listed
        .into_iter()
        .map(|entry| ((entry.value.as_str(), entry.key.as_str()), entry.weight))
        .collect();
    let mut before: HashMap<(&str, &str), i64> = HashMap::new();
    for (text, accepted) in earlier {
        for (value, key, weight) in weighted_rows(text) {
            if accepted.is_some_and(|accepted| !accepted.contains(value)) {
                continue;
            }
            if let Some((&listed_key, _)) = listed.get_key_value(&(value, key)) {
                let slot = before.entry(listed_key).or_insert(weight);
                *slot = (*slot).max(weight);
            }
        }
    }
    listed
        .iter()
        .filter_map(|(&(value, key), &custom)| {
            let earlier = *before.get(&(value, key))?;
            (earlier < custom).then(|| ((value.to_owned(), key.to_owned()), (earlier, custom)))
        })
        .collect()
}

/// 把覆盖表里每个读音的权重设成给定的值，可降可升。全拼表里没有这一行（过期了），或者这一行是 `custom/words.txt` 从已有的行提了权的（`promoted`，见 [`custom_promotions`]；人工词条的提权只升不降，覆盖表不能把它改回去，要改就走 custom 的流程）时，阶段失败，表不动。custom 新插入的词可以覆盖：它们的权重是投稿时定的，越位时（计划 H11 的那批）正要靠这里降下来。
pub fn apply_weight_overrides(
    connection: &mut Connection,
    overrides: &WeightOverrides,
    promoted: &HashMap<(String, String), (i64, i64)>,
) -> Result<OverrideCounts> {
    let mut counts = OverrideCounts::default();
    let (mut stale, mut boosted) = (Vec::new(), Vec::new());
    let transaction = connection.transaction()?;
    for entry in &overrides.entries {
        let table =
            msime::pinyin_table(&entry.key).context("a parsed override maps to a quanpin table")?;
        let current: Option<i64> = transaction
            .query_row(
                &format!("select max(weight) from {table} where key = ? and value = ? having count(*) > 0"),
                params![entry.key, entry.word],
                |row| row.get(0),
            )
            .optional()?;
        let Some(current) = current else {
            stale.push(format!(
                "{} {} (line {})",
                entry.word, entry.key, entry.line
            ));
            continue;
        };
        if let Some((earlier, weight)) = promoted.get(&(entry.word.clone(), entry.key.clone())) {
            boosted.push(format!(
                "{} {} (line {}, raised from {earlier} to {weight} by custom/words.txt)",
                entry.word, entry.key, entry.line
            ));
            continue;
        }
        if current == entry.weight {
            counts.unchanged += 1;
            continue;
        }
        transaction.execute(
            &format!("update {table} set weight = ? where key = ? and value = ?"),
            params![entry.weight, entry.key, entry.word],
        )?;
        counts.changed += 1;
    }
    if !stale.is_empty() || !boosted.is_empty() {
        let mut problems = Vec::new();
        if !stale.is_empty() {
            problems.push(format!(
                "no row to override for {} (drop the entry if no input has the reading any more)",
                stale.join(", ")
            ));
        }
        if !boosted.is_empty() {
            problems.push(format!(
                "custom/words.txt raised {}, and its entries only raise weights; leave those rows out of the overrides",
                boosted.join(", ")
            ));
        }
        bail!("{PINYIN_WEIGHT_OVERRIDES}: {}", problems.join("; "));
    }
    transaction.commit()?;
    Ok(counts)
}

/// 读 `curation/offensive-words.txt`：格式与引擎内嵌的 `offensive_words.txt` 相同，用引擎的同一个解析器（`msime_engine::parse_offensive_words`）读，规则也相同。
pub fn parse_offensive_words(text: &str) -> Result<Vec<msime_engine::OffensiveEntry>> {
    msime_engine::parse_offensive_words(text)
        .map_err(|error| anyhow::anyhow!("{OFFENSIVE_WORDS}: {error}"))
}

/// 把粗俗词表写进 `msime-pinyin.db` 的 `sentence_excluded`（`msime_engine::format::SENTENCE_EXCLUDED_SCHEMA`）。每个词必须是全拼表里某一行的词，每条豁免写法必须出现在全拼表某一行的词里，否则它在这次的词库里什么也不管，表过期了，阶段失败、不写表。返回词和豁免写法的条数。
pub fn write_sentence_excluded(
    connection: &mut Connection,
    entries: &[msime_engine::OffensiveEntry],
) -> Result<(usize, usize)> {
    let listed: HashSet<&str> = entries
        .iter()
        .filter(|entry| !entry.exempt)
        .map(|entry| entry.text.as_str())
        .collect();
    let exempt: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.exempt)
        .map(|entry| entry.text.as_str())
        .collect();
    let mut found: HashSet<&str> = HashSet::new();
    let mut covered = vec![false; exempt.len()];
    for table in msime::quanpin_tables() {
        let mut statement = connection.prepare(&format!("SELECT DISTINCT value FROM {table}"))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let Some(value) = row.get::<_, Option<String>>(0)? else {
                continue;
            };
            if let Some(word) = listed.get(value.as_str()) {
                found.insert(*word);
            }
            for (index, pattern) in exempt.iter().enumerate() {
                if !covered[index] && value.contains(pattern) {
                    covered[index] = true;
                }
            }
        }
    }
    let mut stale: Vec<String> = entries
        .iter()
        .filter(|entry| !entry.exempt && !found.contains(entry.text.as_str()))
        .map(|entry| entry.text.clone())
        .collect();
    stale.extend(
        exempt
            .iter()
            .zip(&covered)
            .filter(|(_, covered)| !**covered)
            .map(|(pattern, _)| format!("!{pattern}")),
    );
    if !stale.is_empty() {
        bail!(
            "{OFFENSIVE_WORDS}: {} match no word of the quanpin tables (a listed word must be a dictionary word, an exempt pattern part of one); drop them",
            stale.join(", ")
        );
    }
    let mut sorted: Vec<&msime_engine::OffensiveEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.text.cmp(&b.text));
    let table = msime_engine::format::SENTENCE_EXCLUDED_TABLE;
    let transaction = connection.transaction()?;
    transaction.execute_batch(&format!("DROP TABLE IF EXISTS {table};"))?;
    transaction.execute_batch(msime_engine::format::SENTENCE_EXCLUDED_SCHEMA)?;
    {
        let mut insert =
            transaction.prepare(&format!("INSERT INTO {table}(value, exempt) VALUES (?, ?)"))?;
        for entry in sorted {
            insert.execute(params![entry.text, i64::from(entry.exempt)])?;
        }
    }
    transaction.commit()?;
    Ok((listed.len(), exempt.len()))
}

/// 去掉 `--out` 里上一次构建留下的 `sentence_excluded`：这次的 checkout 没有 `curation/offensive-words.txt`，引擎应当用内嵌表。没有这张表时不碰数据库。返回是否删了。
pub fn drop_sentence_excluded(connection: &Connection) -> Result<bool> {
    let table = msime_engine::format::SENTENCE_EXCLUDED_TABLE;
    let present: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )?;
    if present {
        connection.execute_batch(&format!("DROP TABLE {table};"))?;
    }
    Ok(present)
}

/// 英文的否决集合：构建器内置的蔑称（`english_supplement::OFFENSIVE` 加上 [`BUILT_IN_VETOES`]），再加 `curation/english-vetoes.txt`。对全部英文来源一视同仁：rime-ice 的两张词表、SCOWL 补充表、OALDPE 词头和 `custom/english.txt`，按小写词否决，所有大小写都不进；`custom/translations.txt` 里英文一侧含这些词的条目也不写（[`EnglishVetoes::vetoed_translation`]）。内置的一组不靠 `curation/` 也生效，所以换上这个构建器之后 `msime-english.db` 就会变。
#[derive(Debug, Default)]
pub struct EnglishVetoes {
    /// `curation/english-vetoes.txt` 的词和行号。
    pub curated: Vec<(String, usize)>,
    words: HashSet<String>,
}

/// rime-ice 自己的英文词表收了、而 `english_supplement::OFFENSIVE` 只管 SCOWL 补充表所以没拦住的蔑称。小写。
pub const BUILT_IN_VETOES: [&str; 3] = ["beyatch", "nigger", "tranny"];

impl EnglishVetoes {
    /// 只有内置的词。
    pub fn built_in() -> Self {
        Self {
            curated: Vec::new(),
            words: crate::english_supplement::OFFENSIVE
                .iter()
                .chain(BUILT_IN_VETOES.iter())
                .map(|word| (*word).to_owned())
                .collect(),
        }
    }

    /// 内置的词加上 `text`（一行一个小写 ASCII 词）。重复的、内置表已有的都算错。
    pub fn parse(text: &str) -> Result<Self> {
        let mut vetoes = Self::built_in();
        for (line, word) in entries(text) {
            let at = || location(ENGLISH_VETOES, line);
            if !word.bytes().all(|byte| byte.is_ascii_lowercase()) {
                bail!("{}: {word:?} is not a lowercase ASCII word", at());
            }
            if vetoes.curated.iter().any(|(listed, _)| listed == word) {
                bail!("{}: {word} is listed twice", at());
            }
            if !vetoes.words.insert(word.to_owned()) {
                bail!(
                    "{}: {word} is already vetoed by the builder's built-in list",
                    at()
                );
            }
            vetoes.curated.push((word.to_owned(), line));
        }
        Ok(vetoes)
    }

    /// 从 `--dictionary` checkout 读，不在的按只有内置的词。
    pub fn load(sources: &Sources) -> Result<Self> {
        Self::parse(&read_optional(sources, ENGLISH_VETOES)?.unwrap_or_default())
    }

    /// `word`（任意大小写）是否被否决。
    pub fn vetoes(&self, word: &str) -> bool {
        self.words.contains(&word.to_ascii_lowercase())
    }

    /// `text` 里第一个被否决的英文词（连续的 ASCII 字母算一个词，任意大小写），没有时为 `None`。`custom/translations.txt` 的一条按它的英文一侧查：英译中查源词，中译英查译文。
    pub fn vetoed_word_in<'a>(&self, text: &'a str) -> Option<&'a str> {
        text.split(|c: char| !c.is_ascii_alphabetic())
            .find(|word| !word.is_empty() && self.vetoes(word))
    }

    /// `custom/translations.txt` 的一条（`source<TAB>gloss`）是否因为英文一侧有被否决的词而不写入，返回那个词。哪一侧是英文按 `english::is_chinese_source` 定，与构建相同。
    pub fn vetoed_translation<'a>(&self, source: &'a str, gloss: &'a str) -> Option<&'a str> {
        self.vetoed_word_in(if crate::english::is_chinese_source(source) {
            gloss
        } else {
            source
        })
    }

    /// `curation/english-vetoes.txt` 里在 `words`（全部英文来源的小写词）中一个也没有的词，报错用。
    pub fn stale<'a>(&'a self, words: &HashSet<&str>) -> Vec<&'a str> {
        self.curated
            .iter()
            .filter(|(word, _)| !words.contains(word.as_str()))
            .map(|(word, _)| word.as_str())
            .collect()
    }

    /// 否决来自哪里，check-words 的拒绝理由用。
    pub fn source(&self, word: &str) -> Option<&'static str> {
        let word = word.to_ascii_lowercase();
        if !self.words.contains(&word) {
            return None;
        }
        Some(if self.curated.iter().any(|(listed, _)| *listed == word) {
            ENGLISH_VETOES
        } else {
            "the builder's built-in slur list"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quanpin(rows: &[(&str, &str, i64)]) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        for table in msime::quanpin_tables() {
            connection
                .execute_batch(&format!(
                    "CREATE TABLE {table}(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
                ))
                .unwrap();
        }
        for (value, key, weight) in rows {
            let table = msime::pinyin_table(key).unwrap();
            connection
                .execute(
                    &format!("INSERT INTO {table}(key, jp, value, weight) VALUES (?, '', ?, ?)"),
                    params![key, value, weight],
                )
                .unwrap();
        }
        connection
    }

    fn weight(connection: &Connection, value: &str, key: &str) -> Option<i64> {
        let table = msime::pinyin_table(key).unwrap();
        connection
            .query_row(
                &format!("SELECT weight FROM {table} WHERE key = ? AND value = ?"),
                params![key, value],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
    }

    #[test]
    fn vetoes_parse_whole_words_and_readings() {
        let vetoes = PinyinVetoes::parse("# 注释\n\n别鸡巴\n上调\tshang'diao\n").unwrap();
        assert_eq!(vetoes.entries.len(), 2);
        assert!(vetoes.vetoes("别鸡巴", "bie'ji'ba"));
        assert!(vetoes.vetoes_word("别鸡巴"));
        assert!(vetoes.vetoes("上调", "shang'diao"));
        assert!(!vetoes.vetoes("上调", "shang'tiao"));
        assert!(!vetoes.vetoes_word("上调"));
        assert!(PinyinVetoes::parse("").unwrap().is_empty());
    }

    #[test]
    fn malformed_veto_lines_fail() {
        for (text, problem) in [
            ("鸡\n", "single character"),
            ("别鸡巴\n别鸡巴\n", "twice"),
            ("上调\tshang'diao\n上调\tshang'diao\n", "twice"),
            ("上调\tShangDiao\n", "not quanpin"),
            ("上调\tshang'diao\textra\n", "expected a word"),
            ("别鸡巴\n别鸡巴\tbie'ji'ba\n", "redundant"),
        ] {
            let error = PinyinVetoes::parse(text).unwrap_err().to_string();
            assert!(error.contains(problem), "{text:?}: {error}");
            assert!(error.contains(PINYIN_VETOES), "{error}");
        }
    }

    #[test]
    fn overrides_parse_and_conflict_with_vetoes() {
        let overrides = WeightOverrides::parse("车件\tche'jian\t100\n").unwrap();
        assert_eq!(overrides.get("车件", "che'jian"), Some(100));
        assert_eq!(overrides.get("车件", "che'jia"), None);
        for (text, problem) in [
            ("车件\tche'jian\n", "expected word"),
            ("车件\tche'jian\t-1\n", "below 0"),
            ("车件\tche'jian\tmany\n", "not an integer"),
            ("车件\tche'jian\t1\n车件\tche'jian\t2\n", "twice"),
        ] {
            let error = WeightOverrides::parse(text).unwrap_err().to_string();
            assert!(error.contains(problem), "{text:?}: {error}");
        }
        let error = PinyinCuration::parse("车件\n", "车件\tche'jian\t1\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("also vetoed"), "{error}");
    }

    /// 生成器看到的行：否决的去掉，覆盖的换权重，其余原样，格式与 `weighted_rows` 相同。
    #[test]
    fn curated_rows_drop_vetoed_rows_and_apply_overrides() {
        let curation =
            PinyinCuration::parse("别鸡巴\n上调\tshang'diao\n", "车件\tche'jian\t100\n").unwrap();
        let source = "# 头\n别鸡巴\tbie'ji'ba\t100\n上调\tshang'diao\t96310\n上调\tshang'tiao\t50000\n车件\tche'jian\t5000\n车检\tche'jian\t4000\n";
        let rows: Vec<(&str, &str, i64)> = curation.rows(source).collect();
        assert_eq!(
            rows,
            [
                ("上调", "shang'tiao", 50000),
                ("车件", "che'jian", 100),
                ("车检", "che'jian", 4000)
            ]
        );
        let empty = PinyinCuration::default();
        assert_eq!(
            empty.rows(source).collect::<Vec<_>>(),
            weighted_rows(source).collect::<Vec<_>>()
        );
    }

    #[test]
    fn word_vetoes_delete_every_reading_or_one_reading() {
        let mut connection = quanpin(&[
            ("别鸡巴", "bie'ji'ba", 100),
            ("上调", "shang'diao", 96310),
            ("上调", "shang'tiao", 50000),
            ("上吊", "shang'diao", 2000),
        ]);
        let vetoes = PinyinVetoes::parse("别鸡巴\n上调\tshang'diao\n").unwrap();
        let counts = apply_word_vetoes(&mut connection, &vetoes, || {
            unreachable!("every veto matched")
        })
        .unwrap();
        assert_eq!(
            counts,
            VetoCounts {
                removed: 2,
                wubi_only: 0
            }
        );
        assert_eq!(weight(&connection, "别鸡巴", "bie'ji'ba"), None);
        assert_eq!(weight(&connection, "上调", "shang'diao"), None);
        assert_eq!(weight(&connection, "上调", "shang'tiao"), Some(50000));
        assert_eq!(weight(&connection, "上吊", "shang'diao"), Some(2000));
    }

    /// 只在五笔码表里有的整词否决不算过期；全拼表和五笔码表里都没有的，构建失败，表不动。
    #[test]
    fn a_veto_that_matches_nothing_fails_and_changes_nothing() {
        let mut connection = quanpin(&[("别鸡巴", "bie'ji'ba", 100)]);
        let vetoes = PinyinVetoes::parse("别鸡巴\n狗日\n").unwrap();
        let counts = apply_word_vetoes(&mut connection, &vetoes, || {
            Ok(HashSet::from(["狗日".to_owned()]))
        })
        .unwrap();
        assert_eq!(
            counts,
            VetoCounts {
                removed: 1,
                wubi_only: 1
            }
        );

        let mut connection = quanpin(&[("别鸡巴", "bie'ji'ba", 100)]);
        let vetoes = PinyinVetoes::parse("别鸡巴\n狗日\n上调\tshang'diao\n").unwrap();
        let error = apply_word_vetoes(&mut connection, &vetoes, || Ok(HashSet::new()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("狗日 (line 2)"), "{error}");
        assert!(error.contains("上调 shang'diao (line 3)"), "{error}");
        assert_eq!(weight(&connection, "别鸡巴", "bie'ji'ba"), Some(100));
    }

    #[test]
    fn overrides_set_weights_both_ways() {
        let mut connection = quanpin(&[
            ("车件", "che'jian", 5000),
            ("车检", "che'jian", 4000),
            ("闷的", "men'de", 100),
            ("门的", "men'de", 435135),
        ]);
        let overrides = WeightOverrides::parse(
            "车件\tche'jian\t3999\n闷的\tmen'de\t5000\n车检\tche'jian\t4000\n",
        )
        .unwrap();
        let counts = apply_weight_overrides(&mut connection, &overrides, &HashMap::new()).unwrap();
        assert_eq!(
            counts,
            OverrideCounts {
                changed: 2,
                unchanged: 1
            }
        );
        assert_eq!(weight(&connection, "车件", "che'jian"), Some(3999));
        assert_eq!(weight(&connection, "闷的", "men'de"), Some(5000));
    }

    /// 覆盖不到的行（过期）和被 custom/words.txt 从已有的行提了权的行都让阶段失败，表不动；custom 新插入的行（越位的品牌名）和 custom 列了但没有提权的行可以覆盖。
    #[test]
    fn overrides_refuse_stale_rows_and_custom_promotions_only() {
        let custom = |value: &str, key: &str, weight: i64| CustomWord {
            value: value.to_owned(),
            key: key.to_owned(),
            weight,
        };
        // 小红书 只在 custom 里（新插入）；车件 在输入里是 5000，custom 写的 10 没有提权；掌纹 在输入里是 100，custom 提到 20000。
        let listed = [
            custom("小红书", "xiao'hong'shu", 20000),
            custom("车件", "che'jian", 10),
            custom("掌纹", "zhang'wen", 20000),
        ];
        let whitelist = HashSet::from(["甲".to_owned()]);
        let promoted = custom_promotions(
            &listed,
            &[
                ("甲\tjia\t1000\n掌\tzhang\t50\n", Some(&whitelist)),
                (
                    "车件\tche'jian\t5000\n掌纹\tzhang'wen\t100\n# 掌纹\tzhang'wen\t30000\n",
                    None,
                ),
                ("掌纹\tzhang'wen\t90\n", None),
            ],
        );
        assert_eq!(
            promoted,
            HashMap::from([(("掌纹".to_owned(), "zhang'wen".to_owned()), (100, 20000))])
        );
        // 白名单外的单字不算之前就有：custom 写的 掌 是新插入。
        assert!(custom_promotions(
            &[custom("掌", "zhang", 9000)],
            &[("掌\tzhang\t50\n", Some(&whitelist))]
        )
        .is_empty());

        let mut connection = quanpin(&[
            ("小红书", "xiao'hong'shu", 20000),
            ("车件", "che'jian", 5000),
            ("掌纹", "zhang'wen", 20000),
        ]);
        let overrides = WeightOverrides::parse(
            "掌纹\tzhang'wen\t100\n不存在\tbu'cun'zai\t1\n小红书\txiao'hong'shu\t100\n",
        )
        .unwrap();
        let error = apply_weight_overrides(&mut connection, &overrides, &promoted)
            .unwrap_err()
            .to_string();
        assert!(error.contains("不存在 bu'cun'zai (line 2)"), "{error}");
        assert!(
            error.contains("掌纹 zhang'wen (line 1, raised from 100 to 20000"),
            "{error}"
        );
        assert!(!error.contains("小红书"), "{error}");
        assert_eq!(weight(&connection, "掌纹", "zhang'wen"), Some(20000));
        assert_eq!(weight(&connection, "小红书", "xiao'hong'shu"), Some(20000));

        let overrides =
            WeightOverrides::parse("小红书\txiao'hong'shu\t100\n车件\tche'jian\t100\n").unwrap();
        let counts = apply_weight_overrides(&mut connection, &overrides, &promoted).unwrap();
        assert_eq!(counts.changed, 2);
        assert_eq!(weight(&connection, "小红书", "xiao'hong'shu"), Some(100));
        assert_eq!(weight(&connection, "车件", "che'jian"), Some(100));
    }

    #[test]
    fn the_offensive_table_is_written_sorted_and_checked_against_the_dictionary() {
        let mut connection = quanpin(&[
            ("巨乳", "ju'ru", 3174),
            ("乳交", "ru'jiao", 1196),
            ("水乳交融", "shui'ru'jiao'rong", 3008),
            ("走路", "zou'lu", 100),
        ]);
        let entries = parse_offensive_words("巨乳\n乳交\n!水乳交\n").unwrap();
        assert_eq!(
            write_sentence_excluded(&mut connection, &entries).unwrap(),
            (2, 1)
        );
        let rows: Vec<(String, i64)> = connection
            .prepare("SELECT value, exempt FROM sentence_excluded")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            rows,
            [
                ("乳交".to_owned(), 0),
                ("巨乳".to_owned(), 0),
                ("水乳交".to_owned(), 1)
            ]
        );
        // 引擎读得懂这张表，格式版本不变。
        let table = msime_engine::OffensiveWords::from_entries(entries.iter().cloned()).unwrap();
        assert!(table.is_offensive("我们有巨乳"));
        assert!(!table.is_offensive("水乳交融"));

        let entries = parse_offensive_words("巨乳\n鸡巴\n!水乳交\n!内射模\n").unwrap_err();
        assert!(
            entries.to_string().contains("contains no listed word"),
            "{entries}"
        );
        let entries = parse_offensive_words("巨乳\n鸡巴\n!巨乳症\n").unwrap();
        let error = write_sentence_excluded(&mut connection, &entries)
            .unwrap_err()
            .to_string();
        assert!(error.contains("鸡巴, !巨乳症"), "{error}");
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM sentence_excluded", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 3, "a failed write leaves the earlier table alone");

        assert!(drop_sentence_excluded(&connection).unwrap());
        assert!(!drop_sentence_excluded(&connection).unwrap());
    }

    #[test]
    fn english_vetoes_cover_built_in_slurs_and_the_curated_file() {
        let built_in = EnglishVetoes::built_in();
        for word in ["nigger", "Tranny", "beyatch", "chinks"] {
            assert!(built_in.vetoes(word), "{word}");
        }
        assert!(!built_in.vetoes("china"));
        let vetoes = EnglishVetoes::parse("# 垃圾 token\nasdfgh\n").unwrap();
        assert!(vetoes.vetoes("ASDFGH"));
        assert_eq!(vetoes.source("asdfgh"), Some(ENGLISH_VETOES));
        assert_eq!(
            vetoes.source("nigger"),
            Some("the builder's built-in slur list")
        );
        assert_eq!(vetoes.source("china"), None);
        assert_eq!(vetoes.stale(&HashSet::from(["china"])), ["asdfgh"]);
        assert!(vetoes.stale(&HashSet::from(["asdfgh"])).is_empty());
        assert_eq!(vetoes.vetoed_word_in("a Nigger, or"), Some("Nigger"));
        assert_eq!(vetoes.vetoed_word_in("niggardly; China"), None);
        assert_eq!(
            vetoes.vetoed_translation("黑鬼", "nigger (offensive)"),
            Some("nigger")
        );
        assert_eq!(
            vetoes.vetoed_translation("tranny", "变性人"),
            Some("tranny")
        );
        assert_eq!(vetoes.vetoed_translation("redwood", "红杉"), None);
        // 中文一侧不查：英译中的译文里有英文字母也不算。
        assert_eq!(
            vetoes.vetoed_translation("redwood", "红杉（不是 nigger）"),
            None
        );
        for (text, problem) in [
            ("Asdf\n", "lowercase"),
            ("asdf\nasdf\n", "twice"),
            ("nigger\n", "built-in"),
        ] {
            let error = EnglishVetoes::parse(text).unwrap_err().to_string();
            assert!(error.contains(problem), "{text:?}: {error}");
        }
    }
}
