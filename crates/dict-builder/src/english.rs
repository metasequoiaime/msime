//! `msime-english.db`: the English prefix-candidate table, the bidirectional glosses derived from ECDICT, and the hand-maintained translation overrides.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use regex::Regex;
use rusqlite::{params, Connection};

use crate::curation::{EnglishVetoes, GlossVetoes};
use crate::sqlite;
use crate::text;

const CREATE_ENGLISH_WORDS: &str = "\n            CREATE TABLE english_words (\n                word TEXT COLLATE BINARY NOT NULL,\n                display TEXT NOT NULL,\n                weight INTEGER NOT NULL DEFAULT 0,\n                PRIMARY KEY (word, display)\n            ) WITHOUT ROWID\n            ";

fn is_ascii_word(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphabetic())
}

/// `str.isdigit` for the weight column: every character a decimal digit.
fn is_digits(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|c| {
            unicode_general_category::get_general_category(c)
                == unicode_general_category::GeneralCategory::DecimalNumber
        })
}

/// OALDPE headwords: one lowercase ASCII word per line, no duplicates.
pub fn parse_oaldpe_words(text: &str) -> Result<BTreeSet<String>> {
    let mut words = BTreeSet::new();
    for (number, line) in text::universal_lines(text).into_iter().enumerate() {
        let word = text::strip(line);
        if !is_ascii_word(word) || word.bytes().any(|b| b.is_ascii_uppercase()) {
            bail!(
                "oaldpe-words.txt:{}: expected a lowercase ASCII word, got {word:?}",
                number + 1
            );
        }
        if !words.insert(word.to_owned()) {
            bail!("oaldpe-words.txt:{}: duplicate word {word:?}", number + 1);
        }
    }
    if words.is_empty() {
        bail!("oaldpe-words.txt: file is empty");
    }
    Ok(words)
}

/// Every display casing of each lowercase word, in the order the source first lists them. Each casing becomes its own `english_words` row; [`build_english_words`] decides which of them leads.
pub type EnglishWords = BTreeMap<String, Vec<String>>;

fn add_display(words: &mut EnglishWords, display: &str) {
    let displays = words.entry(display.to_ascii_lowercase()).or_default();
    if !displays.iter().any(|known| known == display) {
        displays.push(display.to_owned());
    }
}

/// `display input-code [weight]` lines from rime-ice's English dictionary. The input code is ignored: prefix lookup uses the display word itself. Returns every casing the source writes for a lowercase word (China and china, PostgreSQL and postgresql), each kept as its own display.
pub fn parse_base_dict_words(text: &str) -> Result<EnglishWords> {
    let mut words = EnglishWords::new();
    for (number, line) in text::universal_lines(text).into_iter().enumerate() {
        let stripped = text::strip(line);
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        let mut fields: Vec<&str> = text::split_whitespace(stripped).collect();
        if fields.len() < 2 {
            bail!(
                "rime-ice-en.txt:{}: expected display input-code [weight]",
                number + 1
            );
        }
        if fields.len() >= 3 && is_digits(fields[fields.len() - 1]) {
            fields.pop();
        }
        fields.pop();
        let display = fields.join(" ");
        if is_ascii_word(&display) {
            add_display(&mut words, &display);
        }
    }
    if words.is_empty() {
        bail!("rime-ice-en.txt: no pure English words found");
    }
    Ok(words)
}

/// A generated word list such as `sources/english/scowl-words.txt`: one display word per line after `#` header lines. Every line has to be an ASCII word and appear once, since the generator only writes such lines; anything else means the file is not what the lock pinned.
pub fn parse_word_list(text: &str, name: &str) -> Result<EnglishWords> {
    let mut words = EnglishWords::new();
    let mut seen = HashSet::new();
    for (number, line) in text::universal_lines(text).into_iter().enumerate() {
        let stripped = text::strip(line);
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        if !is_ascii_word(stripped) {
            bail!(
                "{name}:{}: expected one ASCII word, got {stripped:?}",
                number + 1
            );
        }
        if !seen.insert(stripped.to_owned()) {
            bail!("{name}:{}: duplicate word {stripped:?}", number + 1);
        }
        add_display(&mut words, stripped);
    }
    if words.is_empty() {
        bail!("{name}: no words found");
    }
    Ok(words)
}

/// rime-ice 英文补充表按上游写法收词（PuTTY、Playwright、Musk），其中有些小写词形本身是普通英文词（putty 油灰、playwright 剧作家、musk 麝香）。SCOWL 补充表按小写词形对照补充表，不会再收这些词，于是表里只剩大写写法，打 putty 只能上屏 PuTTY。这里给只有非小写写法、而 SCOWL 拼写词典（`attested`）收了小写词形的词补上小写写法，返回补了几个词。小写写法排在前面由 [`leading_display`] 决定，与 China、china 并存时相同。
pub fn add_attested_lowercase(words: &mut EnglishWords, attested: &HashSet<String>) -> usize {
    let mut added = 0;
    for (word, displays) in words.iter_mut() {
        if !displays.iter().any(|display| display == word) && attested.contains(word) {
            displays.push(word.clone());
            added += 1;
        }
    }
    added
}

/// Adds the displays of `other` to `words`; returns how many lowercase words were new.
pub fn merge_words(words: &mut EnglishWords, other: EnglishWords) -> usize {
    let mut added = 0;
    for (word, displays) in other {
        if !words.contains_key(&word) {
            added += 1;
        }
        for display in displays {
            add_display(words, &display);
        }
    }
    added
}

/// The display that leads when a word has several casings. SCOWL's spelling dictionary (`attested`, its letter-only forms) decides where it has a say: when it lists the word in some of the source's casings but not in lowercase, the first of those leads, since the word is a name (Wikipedia, Ukraine, Islam, Skype) whose lowercase spelling rime-ice also carries. Otherwise (SCOWL has the lowercase form too, as for china and China, may and May, or none of them) the all-lowercase display leads when the source has it, as the form the user typed, and the first the source lists when it does not. rime-ice's own order says nothing here: it lists Go before go but japan before Japan.
fn leading_display<'a>(
    word: &str,
    displays: &'a [String],
    attested: &HashSet<String>,
) -> Option<&'a str> {
    let lowercase = displays.iter().find(|display| display.as_str() == word);
    let attested_first = displays
        .iter()
        .find(|display| attested.contains(display.as_str()));
    let chosen = match attested_first {
        Some(display) if !attested.contains(word) => Some(display),
        _ => lowercase.or_else(|| displays.first()),
    };
    chosen.map(String::as_str)
}

/// Google's unigram counts (`word<TAB>count`), which order the words once several of them match a prefix. A later line for the same word wins.
pub fn parse_google_counts(text: &str) -> HashMap<String, i64> {
    let mut counts = HashMap::new();
    for line in text::universal_lines(text) {
        let stripped = text::strip(line);
        let (word, count) = stripped.split_once('\t').unwrap_or((stripped, ""));
        if word.is_empty() || !is_digits(count) {
            continue;
        }
        if let Ok(count) = count.parse() {
            counts.insert(word.to_lowercase(), count);
        }
    }
    counts
}

/// What [`build_english_words`] wrote: the words of the base lexicons, and how the custom rows landed on them.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EnglishWordCounts {
    /// Lowercase words of the base lexicons.
    pub base: usize,
    /// Base rows: one per display, so a word with several casings has several.
    pub base_rows: usize,
    /// Custom (word, display) pairs the base lexicons did not have.
    pub custom_added: usize,
    /// Custom pairs that replaced a base row with the same word and display.
    pub custom_replaced: usize,
    /// 被否决（[`EnglishVetoes`]）而不写入的小写词和 custom 行。
    pub vetoed: usize,
}

/// The weight a custom row is stored with. Base rows are weighted by the word's Google unigram count, 0 when Google never counted it, and the Engine orders a prefix's completions by that weight after the exact word. A custom row takes the same count when there is one, so a custom word sits where its real frequency puts it and never above a word people type more often. A word Google never counted takes its file weight capped just below the smallest count (`ceiling`), which places it after every counted word and ahead of the uncounted base words at 0: reachable from a prefix once the common words are out of the way, always first on its own full spelling, and unable to outrank a common word whatever weight the file gives it. The file weight is therefore only an ordering among uncounted custom words. Never lower than the base row it replaces, since that row already carries the count.
fn custom_english_weight(entry: &CustomEnglishWord, count: i64, ceiling: i64) -> i64 {
    count.max(entry.weight.min(ceiling))
}

/// `vetoes` 对全部来源一视同仁：OALDPE 词头、`base`（rime-ice 两张词表和 SCOWL 补充表合并后的结果）和 `custom` 里被否决的小写词，任何大小写都不写入。`curation/english-vetoes.txt` 的词在这些来源里一个也没有时失败：表过期了。
pub fn build_english_words(
    connection: &mut Connection,
    oaldpe: &BTreeSet<String>,
    base: &EnglishWords,
    counts: &HashMap<String, i64>,
    attested: &HashSet<String>,
    custom: &[CustomEnglishWord],
    vetoes: &EnglishVetoes,
) -> Result<EnglishWordCounts> {
    let present: HashSet<&str> = oaldpe
        .iter()
        .chain(base.keys())
        .map(String::as_str)
        .chain(custom.iter().map(|entry| entry.word.as_str()))
        .collect();
    let stale = vetoes.stale(&present);
    if !stale.is_empty() {
        bail!(
            "{}: {} match no English word of any source (drop the entries)",
            crate::curation::ENGLISH_VETOES,
            stale.join(", ")
        );
    }
    let all: BTreeSet<&String> = oaldpe.iter().chain(base.keys()).collect();
    let words: BTreeSet<&String> = all
        .iter()
        .copied()
        .filter(|word| !vetoes.vetoes(word))
        .collect();
    let kept_custom: Vec<&CustomEnglishWord> = custom
        .iter()
        .filter(|entry| !vetoes.vetoes(&entry.word))
        .collect();
    let mut result = EnglishWordCounts {
        base: words.len(),
        vetoed: all.len() - words.len() + custom.len() - kept_custom.len(),
        ..EnglishWordCounts::default()
    };
    let custom = kept_custom;
    let transaction = connection.transaction()?;
    transaction.execute_batch("DROP TABLE IF EXISTS english_words")?;
    transaction.execute_batch(CREATE_ENGLISH_WORDS)?;
    {
        let mut insert = transaction
            .prepare("INSERT INTO english_words(word, display, weight) VALUES (?, ?, ?)")?;
        for word in &words {
            let count = counts.get(*word).copied().unwrap_or(0);
            let displays = base.get(*word).map(Vec::as_slice).unwrap_or_default();
            let Some(leading) = leading_display(word, displays, attested) else {
                // An OALDPE headword the base lexicons lack: its lowercase spelling is the display.
                insert.execute(params![word, word, count])?;
                result.base_rows += 1;
                continue;
            };
            // The leading casing takes the count and the others one below it, so the Engine, which orders a prefix's rows by weight and then by display bytes, shows the leading one first. A word without a count would tie at zero and fall to byte order, which puts every capitalised form first, so its leading casing gets 1 instead.
            let leading_weight = if displays.len() > 1 {
                count.max(1)
            } else {
                count
            };
            for display in displays {
                let weight = if display == leading {
                    leading_weight
                } else {
                    leading_weight - 1
                };
                insert.execute(params![word, display, weight])?;
                result.base_rows += 1;
            }
        }
    }
    // Every base word has at least one display and every (word, display) pair is written once; custom rows may add more displays.
    let (base_rows, distinct): (i64, i64) = transaction.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT word) FROM english_words",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if base_rows == 0
        || base_rows != i64::try_from(result.base_rows)?
        || distinct != i64::try_from(result.base)?
    {
        bail!(
            "english_words: unexpected row counts: rows={base_rows}, distinct_words={distinct}, expected {} rows of {} words",
            result.base_rows,
            result.base
        );
    }
    {
        let ceiling = counts
            .values()
            .copied()
            .filter(|count| *count > 0)
            .min()
            .map_or(i64::MAX, |smallest| smallest - 1);
        let mut exists = transaction
            .prepare("SELECT EXISTS(SELECT 1 FROM english_words WHERE word = ? AND display = ?)")?;
        let mut insert = transaction.prepare(
            "INSERT OR REPLACE INTO english_words(word, display, weight) VALUES (?, ?, ?)",
        )?;
        for entry in custom {
            let replaced: bool =
                exists.query_row(params![entry.word, entry.display], |row| row.get(0))?;
            let count = counts.get(&entry.word).copied().unwrap_or(0);
            insert.execute(params![
                entry.word,
                entry.display,
                custom_english_weight(entry, count, ceiling)
            ])?;
            if replaced {
                result.custom_replaced += 1;
            } else {
                result.custom_added += 1;
            }
        }
    }
    transaction.commit()?;

    // The Python verify_db.py step, including its ANALYZE: it runs before the gloss tables exist, so the shipped statistics cover english_words only.
    sqlite::integrity_check(connection)?;
    let primary_key: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info('english_words') WHERE pk > 0 ORDER BY pk")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    if primary_key != ["word", "display"] {
        bail!("english_words must use PRIMARY KEY(word, display); got {primary_key:?}");
    }
    let rows: i64 =
        connection.query_row("SELECT COUNT(*) FROM english_words", [], |row| row.get(0))?;
    let expected = base_rows + i64::try_from(result.custom_added)?;
    if rows != expected {
        bail!(
            "english_words: {rows} rows, expected {base_rows} base rows plus {} custom ones",
            result.custom_added
        );
    }
    sqlite::analyze(connection, true)?;
    Ok(result)
}

/// The licence notices of the word lists in `english_words` whose terms ask for the notice in every copy (SCOWL's), as `(source, notice)` rows of `source_notices`. The Engine never reads the table; it only travels with the database.
pub fn write_notices(connection: &mut Connection, notices: &[(&str, &str)]) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch("DROP TABLE IF EXISTS source_notices; CREATE TABLE source_notices (source TEXT NOT NULL PRIMARY KEY, notice TEXT NOT NULL) WITHOUT ROWID;")?;
    for (source, notice) in notices {
        transaction.execute(
            "INSERT INTO source_notices(source, notice) VALUES (?, ?)",
            params![source, notice],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

// ---- custom English words ----

pub const CUSTOM_ENGLISH: &str = "custom/english.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomEnglishWord {
    /// The lowercase key a prefix is matched against.
    pub word: String,
    /// What the candidate shows and commits.
    pub display: String,
    pub weight: i64,
}

/// `word<TAB>display<TAB>weight`. One word may list several displays (webview, Webview, webview2), each its own row. A malformed line fails the stage, as custom/words.txt does. A (word, display) pair listed twice keeps its last weight, which is what the INSERT OR REPLACE that stores it does too.
pub fn parse_custom_english(text: &str) -> Result<Vec<CustomEnglishWord>> {
    let mut entries: indexmap::IndexMap<(String, String), CustomEnglishWord> =
        indexmap::IndexMap::new();
    for (number, line) in text::splitlines(text::without_bom(text))
        .into_iter()
        .enumerate()
    {
        let Some(entry) = parse_custom_english_line(line)
            .with_context(|| format!("{CUSTOM_ENGLISH}:{}", number + 1))?
        else {
            continue;
        };
        entries.insert((entry.word.clone(), entry.display.clone()), entry);
    }
    Ok(entries.into_values().collect())
}

/// One line of custom/english.txt: `None` for a blank or `#` comment line, an error naming the problem (without its location) for a malformed one. `check-words` validates contributed lines with this same function.
pub fn parse_custom_english_line(line: &str) -> Result<Option<CustomEnglishWord>> {
    let stripped = text::strip(line);
    if stripped.is_empty() || stripped.starts_with('#') {
        return Ok(None);
    }
    let fields: Vec<&str> = stripped.split('\t').collect();
    let [word, display, weight] = fields[..] else {
        bail!("expected word, display and weight: {line:?}");
    };
    let (word, display, weight) = (text::strip(word), text::strip(display), text::strip(weight));
    // The Engine only looks up lowercase a-z prefixes, so any other key could never be reached.
    if !is_lowercase_ascii_word(word) {
        bail!("{word:?} is not a lowercase ASCII word");
    }
    if display.is_empty() {
        bail!("the display is empty");
    }
    let weight: i64 = weight
        .parse()
        .with_context(|| format!("weight {weight:?} is not an integer"))?;
    if weight < 1 {
        bail!("weight {weight} is below 1");
    }
    Ok(Some(CustomEnglishWord {
        word: word.to_owned(),
        display: display.to_owned(),
        weight,
    }))
}

// ---- ECDICT glosses ----

// Python's `\s` also matches the ASCII information separators, which Rust's does not.
macro_rules! ws {
    () => {
        r"[\s\x1c-\x1f]"
    };
}

static LEADING_DOMAIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        "^",
        ws!(),
        r"*(?:\[[^\]]+\]|【[^】]+】)",
        ws!(),
        "*"
    ))
    .expect("valid regex")
});
static LEADING_POS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        "(?i)^",
        ws!(),
        r"*(?:(?:interj|abbr|modal|aux|adj|adv|prep|pron|conj|num|art|sing|pref|suff|vt|vi|ad|pl|int|n|v|a)\.?",
        ws!(),
        "*)+"
    ))
    .expect("valid regex")
});
static LEADING_PAREN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!("^", ws!(), r"*[（(][^）)]*[）)]", ws!(), "*")).expect("valid regex")
});
static TRAILING_PAREN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(ws!(), r"*[（(][^）)]*[）)]", ws!(), r"*\z")).expect("valid regex")
});
static INTERNAL_PAREN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[（(].*[）)]").expect("valid regex"));
static SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[,，;；、]+").expect("valid regex"));

const TRIM_PUNCTUATION: &str = " \t\r\n.:：!?！？'\"“”‘’·•-—–_/\\";

/// A reverse-index item starting with one of these is usually an explanation rather than a Chinese headword.
const REVERSE_EXPLANATION_PREFIXES: &[&str] = &[
    "表示", "用于", "用来", "用作", "指代", "即为", "一种", "一个", "某种", "某个",
];

const TAG_BONUS: &[(&str, i64)] = &[
    ("zk", 80_000),
    ("gk", 75_000),
    ("cet4", 70_000),
    ("cet6", 60_000),
    ("ky", 55_000),
    ("ielts", 50_000),
    ("toefl", 45_000),
    ("gre", 30_000),
];

fn is_cjk_term(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'))
}

fn is_lowercase_ascii_word(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_lowercase())
}

/// 一行开头（标签、词性之后）写着「X的…」的屈折说明（`be的现在式第三人称`、`eat的过去式`），X 是原形。
static INFLECTION_NOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A([A-Za-z]+)的").expect("valid regex"));

/// 英译中选第一个义项时，第一条通用义项行（ECDICT 常把名词行排在动词行前面：give 的第一行是「n. 弹性」）的首项权重按这个倍数算：别的行的首项要常用到它的这么多倍才排到前面（give 的「给」，look 的「看」）。
const FIRST_LINE_BIAS: i64 = 10;
/// 英译中的冷僻义项：权重（[`rarity_weight`]）低于这个值、或不到这个词最常用的通用义项的 1/[`RARE_TERM_RATIO`] 的义项，排在所有行的常用义项后面，不影响行的先后（may 的「愿能」810；post 的「柱」「杆」「准星」不到「岗位」的 1/50，排在「邮件」「职位」后面；school 的「鱼群」）。
const RARE_TERM_WEIGHT: i64 = 1_000;
/// 见 [`RARE_TERM_WEIGHT`]。用相对值而不是一个更高的绝对门槛：冷僻英文词的中文义项大多不常用，绝对门槛（比较过 2 万、5 万）会把这些词的首义换成后面某一行碰巧常用的词（generous 成了「大量的」），2026-11-04 比较英文候选词前 1500 个和随机抽样时 50 倍改动最少、错得也最少。
const RARE_TERM_RATIO: i64 = 50;
/// 中译英的打分：义项在行内每靠后一位乘 7/10，不在这个英文词主行（[`order_terms`] 排出的第一项所在的行）上再乘 7/10。
const POSITION_DECAY: (i64, i64) = (7, 10);
/// 中译英的第二个英文词至少要有第一名的这个百分比。
const SECOND_GLOSS_PERCENT: i64 = 60;
/// 中译英得分最高的英文词，这个中文义项在它那里的位置（行内序号，不在主行上再加 1）超过这个数时，这个中文词不给英文释义：排在第五位以后的义项多是成语或引申义的碎片（ghost 的「一点」来自 the ghost of a chance），按 2026-11-04 的抽样，这样留下来的释义约一半是错的。
const MAX_REVERSE_STEPS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossTerm {
    pub text: String,
    pub line_index: usize,
    pub item_index: usize,
    pub domain_specific: bool,
    pub reverse_domain_allowed: bool,
}

impl GlossTerm {
    fn reverse_eligible(&self) -> bool {
        let length = self.text.chars().count();
        if (self.domain_specific && !self.reverse_domain_allowed) || !(2..=6).contains(&length) {
            return false;
        }
        !REVERSE_EXPLANATION_PREFIXES
            .iter()
            .any(|prefix| self.text.starts_with(prefix))
    }
}

fn parse_positive_int(value: &str) -> i64 {
    text::strip(value)
        .parse::<i64>()
        .map_or(0, |parsed| parsed.max(0))
}

pub struct EcdictRow<'a> {
    pub word: &'a str,
    pub translation: &'a str,
    pub collins: &'a str,
    pub oxford: &'a str,
    pub tag: &'a str,
    pub bnc: &'a str,
    pub frq: &'a str,
    pub exchange: &'a str,
}

pub fn vocabulary_quality(row: &EcdictRow) -> i64 {
    let collins = parse_positive_int(row.collins).min(5);
    let oxford = i64::from(parse_positive_int(row.oxford) != 0);
    let lowered = row.tag.to_lowercase();
    let tag_bonus = text::split_whitespace(&lowered)
        .map(|tag| {
            TAG_BONUS
                .iter()
                .find(|(name, _)| *name == tag)
                .map_or(0, |(_, bonus)| *bonus)
        })
        .max()
        .unwrap_or(0);
    let frq = parse_positive_int(row.frq);
    let bnc = parse_positive_int(row.bnc);
    let frq_bonus = if frq != 0 {
        (50_000 - frq.min(50_000)).max(0)
    } else {
        0
    };
    let bnc_bonus = if bnc != 0 {
        (25_000 - bnc.min(50_000) / 2).max(0)
    } else {
        0
    };
    collins * 100_000 + oxford * 80_000 + tag_bonus + frq_bonus + bnc_bonus
}

/// Removes leading and trailing qualifiers in parentheses; `None` when an explanation in parentheses sits inside the item.
fn strip_parenthetical_qualifiers(value: &str) -> Option<String> {
    let mut value = value.to_owned();
    loop {
        let next = TRAILING_PAREN
            .replace(&LEADING_PAREN.replace(&value, ""), "")
            .into_owned();
        if next == value {
            break;
        }
        value = next;
    }
    (!INTERNAL_PAREN.is_match(&value)).then_some(value)
}

/// 去掉行首的领域标签（`[计]`、`[俚]`、`【建】`），带了标签的行算领域义项；只有计算机标签（含「计」字）的行还能进中译英。`[网络]` 是网上抓来的译名，多是专名的音译或干脆是错的（xxxvii 译成「炒杂碎」，pussy 的「小猫」），不进中译英。
fn strip_domain_labels<'a>(
    mut line: &'a str,
    domain_specific: &mut bool,
    reverse_allowed: &mut bool,
) -> &'a str {
    while let Some(found) = LEADING_DOMAIN.find(line) {
        *domain_specific = true;
        *reverse_allowed = *reverse_allowed || found.as_str().contains('计');
        line = &line[found.end()..];
    }
    line
}

/// 一条 ECDICT 释义拆出来的东西：短的中文词条（阅读顺序）和屈折说明里的原形（[`INFLECTION_NOTE`]）。
struct ParsedTranslation {
    terms: Vec<GlossTerm>,
    inflection_of: Option<String>,
}

/// 按出现顺序提取 ECDICT 译文中的短中文词头。构建走 [`parse_translation`]（连同屈折说明一起拿），这个只给测试用。
#[cfg(test)]
pub fn extract_gloss_terms(translation: &str) -> Vec<GlossTerm> {
    parse_translation(translation).terms
}

/// 领域标签在词性前后都算（`[计] vt. 存款`、`n. [俚] 黑鬼`）。同一个词条在一行里只取一次，在不同的行里各算各的：keep 的「保持」在名词行和动词行都有，动词行不能因为名词行先出现就少了它。
fn parse_translation(translation: &str) -> ParsedTranslation {
    let mut terms = Vec::new();
    let mut inflection_of = None;
    // ECDICT stores many line breaks as the two characters `\n`.
    let translation = translation.replace("\\n", "\n");
    for (line_index, raw_line) in text::splitlines(&translation).into_iter().enumerate() {
        let line = text::strip(raw_line);
        if line.is_empty() {
            continue;
        }
        let mut domain_specific = false;
        let mut reverse_domain_allowed = false;
        let line = strip_domain_labels(line, &mut domain_specific, &mut reverse_domain_allowed);
        let line = match LEADING_POS.find(line) {
            Some(found) => text::strip(&line[found.end()..]),
            None => text::strip(line),
        };
        let line = text::strip(strip_domain_labels(
            line,
            &mut domain_specific,
            &mut reverse_domain_allowed,
        ));
        if !domain_specific && inflection_of.is_none() {
            inflection_of = INFLECTION_NOTE
                .captures(line)
                .map(|captures| captures[1].to_ascii_lowercase());
        }
        let mut seen = HashSet::new();
        for (item_index, raw_item) in SPLIT.split(line).enumerate() {
            let Some(item) =
                strip_parenthetical_qualifiers(text::strip_chars(raw_item, TRIM_PUNCTUATION))
            else {
                continue;
            };
            let item = text::strip_chars(&item, TRIM_PUNCTUATION);
            if item.chars().count() > 8 || !is_cjk_term(item) || !seen.insert(item.to_owned()) {
                continue;
            }
            terms.push(GlossTerm {
                text: item.to_owned(),
                line_index,
                item_index,
                domain_specific,
                reverse_domain_allowed,
            });
        }
    }
    ParsedTranslation {
        terms,
        inflection_of,
    }
}

/// 判断义项冷僻与否用的权重：拼音表里这个词的权重；「X的」再看 X 的权重，取较高的。
fn rarity_weight(term: &str, weights: &HashMap<String, i64>) -> i64 {
    let own = weights.get(term).copied().unwrap_or(0);
    term.strip_suffix('的')
        .filter(|stem| !stem.is_empty())
        .and_then(|stem| weights.get(stem))
        .map_or(own, |stem| own.max(*stem))
}

/// 英译中的义项顺序，义项位置为主：ECDICT 一行之内的义项按常用程度排，所以行内按位置；行与行之间是按词性分的（常把名词行放在最前，give 是「n. 弹性」、keep 是「n. 生计」），位置说明不了哪个词性常用，就按各行首项在拼音表里的权重排，第一条通用行占 [`FIRST_LINE_BIAS`] 倍的优势。先把领域行排在通用行后面，再把冷僻义项（[`RARE_TERM_WEIGHT`]、[`RARE_TERM_RATIO`]）按行的先后全局移到所有常用义项后面，因此常用领域义项可能领先于冷僻通用义项。「X的」在 X 也是义项时不要，同一个词只出现一次；第一项是通用义项时只留通用义项，不拿领域义项凑第二个（TV 不配「转移向量」）。
fn order_terms<'a>(terms: &[&'a GlossTerm], weights: &HashMap<String, i64>) -> Vec<&'a GlossTerm> {
    let weight = |term: &GlossTerm| weights.get(&term.text).copied().unwrap_or(0);
    let texts: HashSet<&str> = terms.iter().map(|term| term.text.as_str()).collect();
    let mut lines: Vec<Vec<&'a GlossTerm>> = Vec::new();
    for &term in terms {
        if term
            .text
            .strip_suffix('的')
            .is_some_and(|stem| texts.contains(stem))
        {
            continue;
        }
        match lines.last_mut() {
            Some(line) if line[0].line_index == term.line_index => line.push(term),
            _ => lines.push(vec![term]),
        }
    }
    let first_general = lines.iter().position(|line| !line[0].domain_specific);
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by_key(|&index| {
        let lead = lines[index][0];
        let bias = if Some(index) == first_general {
            FIRST_LINE_BIAS
        } else {
            1
        };
        (
            lead.domain_specific,
            std::cmp::Reverse(weight(lead).saturating_mul(bias)),
            lead.line_index,
        )
    });
    let strongest = lines
        .iter()
        .flatten()
        .filter(|term| !term.domain_specific)
        .map(|term| rarity_weight(&term.text, weights))
        .max()
        .unwrap_or(0);
    let threshold = RARE_TERM_WEIGHT.max(strongest / RARE_TERM_RATIO);
    let (common, rare): (Vec<&'a GlossTerm>, Vec<&'a GlossTerm>) = order
        .iter()
        .flat_map(|&index| lines[index].iter().copied())
        .partition(|term| rarity_weight(&term.text, weights) >= threshold);
    let mut emitted = HashSet::new();
    let mut ordered = Vec::new();
    for term in common.into_iter().chain(rare) {
        if emitted.insert(term.text.as_str()) {
            ordered.push(term);
        }
    }
    if ordered.first().is_some_and(|term| !term.domain_specific) {
        ordered.retain(|term| !term.domain_specific);
    }
    ordered
}

/// 英译中释义：[`order_terms`] 的前两项，用「；」连接。
pub fn choose_chinese_gloss(terms: &[&GlossTerm], weights: &HashMap<String, i64>) -> String {
    order_terms(terms, weights)
        .iter()
        .take(2)
        .map(|term| term.text.as_str())
        .collect::<Vec<_>>()
        .join("；")
}

struct EnglishEntry {
    /// ECDICT 的词头原样（去掉首尾空白），中译英显示专名的大写从这里来。
    headword: String,
    reverse_english: String,
    quality: i64,
    terms: Vec<GlossTerm>,
    /// 屈折说明（`be的现在式第三人称`）里的原形，它也是英文候选词时才记。
    inflection_of: Option<String>,
}

#[derive(Debug)]
pub struct Glosses {
    pub en_zh: BTreeMap<String, String>,
    pub zh_en: BTreeMap<String, String>,
}

/// 英文候选词（`english_words.word` 的小写 ASCII 词）和它们在表里的全部写法（`display`）。
fn english_candidates(connection: &Connection) -> Result<HashMap<String, BTreeSet<String>>> {
    let mut candidates: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut statement = connection.prepare("SELECT word, display FROM english_words")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if let Some(word) = row.get::<_, Option<String>>(0)? {
            let word = word.to_lowercase();
            if is_lowercase_ascii_word(&word) {
                let displays = candidates.entry(word).or_default();
                if let Some(display) = row.get::<_, Option<String>>(1)? {
                    displays.insert(display);
                }
            }
        }
    }
    Ok(candidates)
}

/// 中译英里一个英文词显示成什么样，大小写只从 ECDICT 词头来，不借 `english_words` 的写法（putty 在表里只有 PuTTY，ram 只有 RAM，「淡灰色」「公羊」仍要显示 putty、ram）：
///
/// - 词头是小写：一律小写。
/// - 词头有大写：SCOWL 拼写词典（`attested`）只收小写词形时用小写（ECDICT 把 core、fax、maxim 的词头写成 CORE、FAX、Maxim）；只收词头的写法时用词头（Africa、Monday、English，`english_words` 里可能只有 africa）；两种都收时（China 和 china）只有词头的第一个义项和以它开头的义项用大写（「中国」「中国的」，不是「瓷器」）。
/// - SCOWL 两种都不收时（缩写、品牌），`english_words` 没有小写写法或这是词头的第一个义项时用词头，否则小写。
///
/// 释义只用来显示，不会作为英文候选上屏，所以词头的写法不必在 `english_words` 里。
fn display_for(
    candidates: &HashMap<String, BTreeSet<String>>,
    attested: &HashSet<String>,
    headword: &str,
    english: &str,
    proper_sense: bool,
) -> String {
    if headword == english {
        return english.to_owned();
    }
    let capitalised = match (attested.contains(headword), attested.contains(english)) {
        (true, true) => proper_sense,
        (true, false) => true,
        (false, true) => false,
        (false, false) => {
            proper_sense
                || !candidates
                    .get(english)
                    .is_some_and(|displays| displays.contains(english))
        }
    };
    if capitalised {
        headword.to_owned()
    } else {
        english.to_owned()
    }
}

/// Every pinyin-table value in `terms`, with its best weight (floored at zero).
fn chinese_term_weights(msime: &Connection, terms: &HashSet<&str>) -> Result<HashMap<String, i64>> {
    let table_pattern = Regex::new(r"\Atbl_(?:[1-7]|others)_[a-z]\z").expect("valid regex");
    let tables: Vec<String> = msime
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name GLOB 'tbl_*_[a-z]' ORDER BY name")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?
        .into_iter()
        .filter(|name| table_pattern.is_match(name))
        .collect();
    let mut weights = HashMap::new();
    for table in tables {
        let mut statement = msime.prepare(&format!("SELECT value,weight FROM \"{table}\""))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let Some(value) = row.get::<_, Option<String>>(0)? else {
                continue;
            };
            if terms.contains(value.as_str()) {
                let weight = row.get::<_, Option<i64>>(1)?.unwrap_or(0);
                let best = weights.entry(value).or_insert(0);
                *best = (*best).max(weight);
            }
        }
    }
    Ok(weights)
}

/// 取 ECDICT 与英文候选的交集，派生两个方向的释义。中译英索引只收有语料或核心词汇信号的词的通用义项与获准的计算机领域义项，并要求中文词能由拼音表产出。`reverse_excluded` 中的词有英译中释义，但不进入中译英索引，也不代替其中的屈折形式；构建把只有 SCOWL 收录的词传入这里，使索引仍从人工整理的词表中选择（否则 SCOWL 长尾会胜出，例如 体现 → impersonate）。
///
/// 两个方向都以义项位置为主（[`order_terms`]）：英译中取排好的前两项；只有屈折说明、没有通用义项的词（is 只有「be的现在式第三人称」和一行 `[计]`）用原形的义项。中译英给每个（中文义项，英文词）打分：英文词的常用度（[`vocabulary_quality`]）在行内每靠后一位乘 [`POSITION_DECAY`]，不在这个词的主行上再乘一次，所以 affect 的首项「影响」压过 win 第二行第三项的「影响」；取最高分，第二个英文词要有它的 [`SECOND_GLOSS_PERCENT`]%，最高分的义项位置太靠后（[`MAX_REVERSE_STEPS`]）时宁可不给。`vetoes` 里成对否决的义项两个方向都不用；有一对在 ECDICT 的候选词里匹配不到就失败。
pub fn derive_glosses(
    ecdict: &Path,
    english: &Connection,
    msime: &Connection,
    reverse_excluded: &HashSet<String>,
    attested: &HashSet<String>,
    vetoes: &GlossVetoes,
) -> Result<Glosses> {
    let candidates = english_candidates(english)?;
    let bytes = crate::sources::read_private(ecdict)
        .with_context(|| format!("reading {}", ecdict.display()))?;
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
    let headers = reader.headers()?.clone();
    let column = |name: &str| headers.iter().position(|header| header == name);
    let required = [
        "word",
        "translation",
        "collins",
        "oxford",
        "tag",
        "bnc",
        "frq",
    ];
    let missing: Vec<_> = required
        .iter()
        .filter(|name| column(name).is_none())
        .collect();
    if !missing.is_empty() {
        bail!("ECDICT CSV is missing columns: {missing:?}");
    }
    let [word, translation, collins, oxford, tag, bnc, frq] =
        required.map(|name| column(name).unwrap_or_default());
    let exchange = column("exchange");

    let mut entries: BTreeMap<String, EnglishEntry> = BTreeMap::new();
    let mut record = csv::StringRecord::new();
    while reader.read_record(&mut record)? {
        let field = |index: usize| record.get(index).unwrap_or("");
        let row = EcdictRow {
            word: field(word),
            translation: field(translation),
            collins: field(collins),
            oxford: field(oxford),
            tag: field(tag),
            bnc: field(bnc),
            frq: field(frq),
            exchange: exchange.map_or("", field),
        };
        let headword = text::strip(row.word);
        let english = headword.to_lowercase();
        if !candidates.contains_key(&english) {
            continue;
        }
        let translation = text::strip(row.translation);
        if translation.is_empty() {
            continue;
        }
        let parsed = parse_translation(translation);
        let inflection_of = parsed
            .inflection_of
            .filter(|lemma| *lemma != english && candidates.contains_key(lemma));
        if parsed.terms.is_empty() && inflection_of.is_none() {
            continue;
        }
        let quality = vocabulary_quality(&row);
        // `exchange` 以 `0:<lemma>` 列出的原形在反向索引中代替其屈折形式。只看小写词头：大写的专名（States 的「美国」）不是 state 的屈折形式。
        let reverse_english = if headword == english {
            row.exchange
                .split('/')
                .filter_map(|item| item.strip_prefix("0:"))
                .map(|lemma| text::strip(lemma).to_lowercase())
                .find(|lemma| {
                    candidates.contains_key(lemma)
                        && !reverse_excluded.contains(lemma)
                        && is_lowercase_ascii_word(lemma)
                })
                .unwrap_or_else(|| english.clone())
        } else {
            english.clone()
        };
        let replace = entries.get(&english).is_none_or(|previous| {
            (quality, parsed.terms.len()) > (previous.quality, previous.terms.len())
        });
        if replace {
            entries.insert(
                english,
                EnglishEntry {
                    headword: headword.to_owned(),
                    reverse_english,
                    quality,
                    terms: parsed.terms,
                    inflection_of,
                },
            );
        }
    }

    // 成对否决：一个义项对这个英文词或它代表的原形被否决，就从这个词的义项里拿掉。
    let mut matched: HashSet<(String, String)> = HashSet::new();
    let mut kept: HashMap<&str, Vec<&GlossTerm>> = HashMap::new();
    for (english, entry) in &entries {
        let mut terms = Vec::new();
        for term in &entry.terms {
            let mut vetoed = false;
            for listed in [english.as_str(), entry.reverse_english.as_str()] {
                if vetoes.vetoes(&term.text, listed) {
                    matched.insert((term.text.clone(), listed.to_owned()));
                    vetoed = true;
                }
            }
            if !vetoed {
                terms.push(term);
            }
        }
        kept.insert(english.as_str(), terms);
    }
    let stale = vetoes.stale(&matched);
    if !stale.is_empty() {
        bail!(
            "{}: {} match no ECDICT sense of an English candidate (drop the entries)",
            crate::curation::GLOSS_VETOES,
            stale.join(", ")
        );
    }

    // 「X的」的 X 也要权重，[`rarity_weight`] 用。
    let all_terms: HashSet<&str> = entries
        .values()
        .flat_map(|entry| entry.terms.iter().map(|term| term.text.as_str()))
        .flat_map(|text| {
            [
                Some(text),
                text.strip_suffix('的').filter(|stem| !stem.is_empty()),
            ]
        })
        .flatten()
        .collect();
    let weights = chinese_term_weights(msime, &all_terms)?;
    let has_general = |terms: &[&GlossTerm]| terms.iter().any(|term| !term.domain_specific);

    let mut en_zh = BTreeMap::new();
    for (english, entry) in &entries {
        let mut terms = &kept[english.as_str()];
        if !has_general(terms) {
            if let Some(lemma_terms) = entry
                .inflection_of
                .as_deref()
                .and_then(|lemma| kept.get(lemma))
                .filter(|lemma_terms| has_general(lemma_terms))
            {
                terms = lemma_terms;
            }
        }
        let gloss = choose_chinese_gloss(terms, &weights);
        if !gloss.is_empty() {
            en_zh.insert(english.clone(), gloss);
        }
    }

    // 先处理重复英文行，再构建反向候选，避免重复源行扭曲排序。
    let mut reverse: HashMap<&str, HashMap<&str, (i64, String, usize)>> = HashMap::new();
    for (english, entry) in &entries {
        if entry.quality <= 0 || reverse_excluded.contains(english) {
            continue;
        }
        let terms = &kept[english.as_str()];
        let primary = order_terms(terms, &weights)
            .first()
            .map(|term| term.line_index);
        for term in terms.iter().filter(|term| term.reverse_eligible()) {
            // 屈折形式的领域义项多是同形缩写（is 的 `[计]` 是 IS，信息系统），不能记到原形名下。
            if term.domain_specific && entry.reverse_english != *english {
                continue;
            }
            let steps = term.item_index + usize::from(Some(term.line_index) != primary);
            let mut score = entry.quality;
            for _ in 0..steps {
                score = score * POSITION_DECAY.0 / POSITION_DECAY.1;
            }
            score -= entry.reverse_english.chars().count() as i64 * 10;
            let proper_sense = term.text.starts_with(entry.terms[0].text.as_str());
            let shown = if entry.reverse_english == *english {
                display_for(
                    &candidates,
                    attested,
                    &entry.headword,
                    english,
                    proper_sense,
                )
            } else {
                display_for(
                    &candidates,
                    attested,
                    &entry.reverse_english,
                    &entry.reverse_english,
                    proper_sense,
                )
            };
            let slot = reverse
                .entry(term.text.as_str())
                .or_default()
                .entry(entry.reverse_english.as_str());
            match slot {
                std::collections::hash_map::Entry::Occupied(mut best) => {
                    if score > best.get().0 {
                        best.insert((score, shown, steps));
                    }
                }
                std::collections::hash_map::Entry::Vacant(vacant) => {
                    vacant.insert((score, shown, steps));
                }
            }
        }
    }
    let zh_en = reverse
        .into_iter()
        .filter(|(chinese, _)| weights.contains_key(*chinese))
        .filter_map(|(chinese, candidates)| {
            finalize_reverse_gloss(candidates).map(|gloss| (chinese.to_owned(), gloss))
        })
        .collect();
    Ok(Glosses { en_zh, zh_en })
}

/// 一个中文词的英文释义：按分数从高到低（同分时短词、字母序在前），最多两个，第二个要有第一名的 [`SECOND_GLOSS_PERCENT`]%；最高分不到 1、或者第一名的位置超过 [`MAX_REVERSE_STEPS`] 时不给。
fn finalize_reverse_gloss(candidates: HashMap<&str, (i64, String, usize)>) -> Option<String> {
    let mut ranked: Vec<(&str, (i64, String, usize))> = candidates.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1 .0
            .cmp(&a.1 .0)
            .then(a.0.chars().count().cmp(&b.0.chars().count()))
            .then_with(|| a.0.cmp(b.0))
    });
    let (top, top_steps) = ranked
        .first()
        .map(|(_, (score, _, steps))| (*score, *steps))?;
    if top <= 0 || top_steps > MAX_REVERSE_STEPS {
        return None;
    }
    let selected: Vec<String> = ranked
        .into_iter()
        .filter(|(_, (score, _, _))| score * 100 >= top * SECOND_GLOSS_PERCENT)
        .take(2)
        .map(|(_, (_, shown, _))| shown)
        .collect();
    Some(selected.join("; "))
}

const CREATE_GLOSS_TABLES: &str = "\n            DROP TABLE IF EXISTS en_zh_glosses_new;\n            DROP TABLE IF EXISTS zh_en_glosses_new;\n            CREATE TABLE en_zh_glosses_new (\n                english TEXT COLLATE BINARY PRIMARY KEY,\n                chinese_gloss TEXT NOT NULL\n            ) WITHOUT ROWID;\n            CREATE TABLE zh_en_glosses_new (\n                chinese TEXT COLLATE BINARY PRIMARY KEY,\n                english_gloss TEXT NOT NULL\n            ) WITHOUT ROWID;\n            ";
const SWAP_GLOSS_TABLES: &str = "\n            DROP TABLE IF EXISTS en_zh_glosses;\n            ALTER TABLE en_zh_glosses_new RENAME TO en_zh_glosses;\n            DROP TABLE IF EXISTS zh_en_glosses;\n            ALTER TABLE zh_en_glosses_new RENAME TO zh_en_glosses;\n            PRAGMA user_version=3;\n            ";

/// Replaces both gloss tables atomically (built under `_new` names and renamed, as the runtime may hold the file open during a local rebuild).
pub fn write_glosses(connection: &mut Connection, glosses: &Glosses) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(CREATE_GLOSS_TABLES)?;
    {
        let mut insert = transaction
            .prepare("INSERT INTO en_zh_glosses_new(english,chinese_gloss) VALUES(?1,?2)")?;
        for (english, gloss) in &glosses.en_zh {
            insert.execute(params![english, gloss])?;
        }
        let mut insert = transaction
            .prepare("INSERT INTO zh_en_glosses_new(chinese,english_gloss) VALUES(?1,?2)")?;
        for (chinese, gloss) in &glosses.zh_en {
            insert.execute(params![chinese, gloss])?;
        }
    }
    transaction.execute_batch(SWAP_GLOSS_TABLES)?;
    transaction.commit()?;
    sqlite::integrity_check(connection)
}

// ---- custom translations ----

#[derive(Debug, PartialEq, Eq)]
pub struct CustomTranslation {
    pub chinese_to_english: bool,
    pub source: String,
    pub gloss: String,
}

/// 人工译文写成这一个字符时表示删掉这个源词的释义（ECDICT 生成的和更早的人工行都删），不是把释义改成它。
pub const DELETE_GLOSS: &str = "-";

impl CustomTranslation {
    /// 这一行是不是删除标记（[`DELETE_GLOSS`]）。
    pub fn deletes(&self) -> bool {
        self.gloss == DELETE_GLOSS
    }
}

pub const CUSTOM_TRANSLATIONS: &str = "custom/translations.txt";

/// 每行 `source<TAB>gloss`；源词含 U+3400 及以上字符时按中译英处理，其余按英译中处理。同一源词以后出现的行优先。译文是单独一个 `-`（[`DELETE_GLOSS`]）的行删掉这个源词的释义，后面的行可以再给它一个。
pub fn parse_custom_translations(text: &str) -> Result<Vec<CustomTranslation>> {
    let mut entries = Vec::new();
    for (number, line) in text::splitlines(text::without_bom(text))
        .into_iter()
        .enumerate()
    {
        if let Some(entry) = parse_custom_translation(line)
            .with_context(|| format!("{CUSTOM_TRANSLATIONS}:{}", number + 1))?
        {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// 人工译文的源词是否含 U+3400 起的字符；含有时按中译英处理。
pub fn is_chinese_source(source: &str) -> bool {
    source.chars().any(|c| c >= '\u{3400}')
}

/// 解析 `custom/translations.txt` 的一行：空行或 `#` 注释返回 `None`，格式错误返回不带位置的问题说明，释义之后的字段忽略。`check-words` 用同一函数检查投稿行。
pub fn parse_custom_translation(line: &str) -> Result<Option<CustomTranslation>> {
    let stripped = text::strip(line);
    if stripped.is_empty() || stripped.starts_with('#') {
        return Ok(None);
    }
    let fields: Vec<&str> = stripped.split('\t').collect();
    if fields.len() < 2 {
        bail!("expected source<TAB>gloss, got {line:?}");
    }
    let (source, gloss) = (text::strip(fields[0]), text::strip(fields[1]));
    if source.is_empty() || gloss.is_empty() {
        bail!("empty source or gloss");
    }
    Ok(Some(CustomTranslation {
        chinese_to_english: is_chinese_source(source),
        source: source.to_owned(),
        gloss: gloss.to_owned(),
    }))
}

/// 按文件顺序写人工译文：删除标记删掉这个源词（原样，不改大小写）的那一行，其余的写入或替换。返回删除标记实际删掉的行数；源词本来就没有释义的删除标记不算错（ECDICT 换了版本时它可能已经没有这个释义，`custom/` 又只能追加）。
pub fn apply_custom_translations(
    connection: &mut Connection,
    entries: &[CustomTranslation],
) -> Result<usize> {
    let transaction = connection.transaction()?;
    let mut deleted = 0;
    for entry in entries {
        if entry.deletes() {
            let sql = if entry.chinese_to_english {
                "DELETE FROM zh_en_glosses WHERE chinese=?1"
            } else {
                "DELETE FROM en_zh_glosses WHERE english=?1"
            };
            deleted += transaction.execute(sql, params![entry.source])?;
            continue;
        }
        let sql = if entry.chinese_to_english {
            "INSERT OR REPLACE INTO zh_en_glosses(chinese,english_gloss) VALUES(?1,?2)"
        } else {
            "INSERT OR REPLACE INTO en_zh_glosses(english,chinese_gloss) VALUES(?1,?2)"
        };
        transaction.execute(sql, params![entry.source, entry.gloss])?;
    }
    transaction.commit()?;
    Ok(deleted)
}

#[cfg(test)]
mod gloss_regression_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn displays(words: &EnglishWords, word: &str) -> Vec<String> {
        words.get(word).cloned().unwrap_or_default()
    }

    fn prefix_rows(connection: &Connection, prefix: &str) -> Vec<(String, i64)> {
        connection
            .prepare("SELECT display, weight FROM english_words WHERE word >= ?1 AND word < ?1 || '{' ORDER BY CASE WHEN word = ?1 THEN 0 ELSE 1 END, weight DESC, length(word), word, display")
            .unwrap()
            .query_map([prefix], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn english_words_keep_every_casing_and_merge_counts() {
        let base = parse_base_dict_words("AA AA\r\naaa aaa\r\n# aac aac\r\nAaliyah Aaliyah\r\nHello hello 3\r\nhello hello\r\nice cream icecream\r\nJan jan 12\r\nDOS DOS\r\nDoS DoS\r\nDOS DOS\r\nwikipedia wikipedia\r\nWikipedia Wikipedia\r\nRaq raq\r\nraq raq\r\n").unwrap();
        assert_eq!(displays(&base, "aa"), ["AA"]);
        assert_eq!(displays(&base, "aaliyah"), ["Aaliyah"]);
        assert_eq!(
            displays(&base, "hello"),
            ["Hello", "hello"],
            "each casing is kept, in source order"
        );
        assert_eq!(displays(&base, "jan"), ["Jan"]);
        assert_eq!(
            displays(&base, "dos"),
            ["DOS", "DoS"],
            "a repeated casing is kept once"
        );
        assert!(!base.contains_key("ice cream"));

        let counts =
            parse_google_counts("the\t100\nHello\t7\nbad\tx\n\t5\ndos\t40\nwikipedia\t50\n");
        // The casings SCOWL lists: Wikipedia only capitalised, hello in both casings, neither DOS nor raq.
        let attested: HashSet<String> = ["Wikipedia", "hello", "Hello"].map(str::to_owned).into();
        assert_eq!(counts.get("hello"), Some(&7));
        assert!(!counts.contains_key("bad"));

        let oaldpe = parse_oaldpe_words("a\nzebra\nhello\n").unwrap();
        assert!(parse_oaldpe_words("a\na\n").is_err());
        assert!(parse_oaldpe_words("Abc\n").is_err());

        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(
            build_english_words(
                &mut connection,
                &oaldpe,
                &base,
                &counts,
                &attested,
                &[],
                &EnglishVetoes::default()
            )
            .unwrap(),
            EnglishWordCounts {
                base: 10,
                base_rows: 14,
                ..EnglishWordCounts::default()
            }
        );
        // SCOWL has hello in lowercase too, so the lowercase casing leads with the word's count although the source lists Hello first; the other casing follows one below.
        assert_eq!(
            prefix_rows(&connection, "hello"),
            [("hello".to_owned(), 7), ("Hello".to_owned(), 6)]
        );
        // Without a lowercase casing the first one the source lists leads.
        assert_eq!(
            prefix_rows(&connection, "dos"),
            [("DOS".to_owned(), 40), ("DoS".to_owned(), 39)]
        );
        // SCOWL lists the name only capitalised, so it leads although the source lists the lowercase spelling first.
        assert_eq!(
            prefix_rows(&connection, "wikipedia"),
            [("Wikipedia".to_owned(), 50), ("wikipedia".to_owned(), 49)]
        );
        // Without a count the leading casing is lifted to 1, so the zero tie does not fall to byte order (Raq before raq).
        assert_eq!(
            prefix_rows(&connection, "raq"),
            [("raq".to_owned(), 1), ("Raq".to_owned(), 0)]
        );
        assert_eq!(prefix_rows(&connection, "zebra"), [("zebra".to_owned(), 0)]);
        let stats: Vec<String> = connection
            .prepare("SELECT tbl FROM sqlite_stat1")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(stats, ["english_words"]);

        write_notices(
            &mut connection,
            &[("SCOWL", "Copyright 2000-2026 by Kevin Atkinson")],
        )
        .unwrap();
        write_notices(
            &mut connection,
            &[("SCOWL", "Copyright 2000-2026 by Kevin Atkinson")],
        )
        .unwrap();
        let notices: Vec<(String, String)> = connection
            .prepare("SELECT source, notice FROM source_notices")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            notices,
            [(
                "SCOWL".to_owned(),
                "Copyright 2000-2026 by Kevin Atkinson".to_owned()
            )],
            "rewriting the notices replaces them"
        );
    }

    #[test]
    fn custom_english_lines_parse_as_the_file_is_written() {
        let entries = parse_custom_english(
            "\u{feff}# custom\nfigma\tfigma\t1\nfigma\tFigma\t1\n\nwebview\twebview2\t2\nwebview\twebview2\t3",
        )
        .unwrap();
        let pairs: Vec<_> = entries
            .iter()
            .map(|entry| (entry.word.as_str(), entry.display.as_str(), entry.weight))
            .collect();
        assert_eq!(
            pairs,
            [
                ("figma", "figma", 1),
                ("figma", "Figma", 1),
                ("webview", "webview2", 3)
            ],
            "one word keeps several displays; a repeated pair keeps its last weight"
        );

        for (line, problem) in [
            ("figma\tFigma", "expected word, display and weight"),
            (
                "figma\tFigma\t1\textra",
                "expected word, display and weight",
            ),
            ("Figma\tFigma\t1", "not a lowercase ASCII word"),
            ("web view\tweb view\t1", "not a lowercase ASCII word"),
            ("figma\t \t1", "the display is empty"),
            ("figma\tFigma\tone", "not an integer"),
            ("figma\tFigma\t0", "below 1"),
        ] {
            let error = parse_custom_english_line(line).unwrap_err().to_string();
            assert!(error.contains(problem), "{line:?}: {error}");
        }
        let error = parse_custom_english("figma\tfigma\t1\nbad\n")
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("custom/english.txt:2"), "{error}");
    }

    #[test]
    fn custom_english_rows_merge_below_common_words_and_above_uncounted_ones() {
        let base = parse_base_dict_words(
            "asr asr\nfig fig\nfigure figure\nfigwort figwort\nwebview webview\n",
        )
        .unwrap();
        // The smallest count is 12711, as in google-word-counts.txt; figwort and figma were never counted.
        let counts = parse_google_counts(
            "figure\t9000000\nfig\t400000\nasr\t779429\nwebview\t79275\nzzz\t12711\n",
        );
        let custom = parse_custom_english("figma\tfigma\t1\nfigma\tFigma\t1\nasr\tASR\t1\nwebview\twebview\t1\nwebview\tWebview2\t1\nloud\tLOUD\t999999999\n").unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        let counts_written = build_english_words(
            &mut connection,
            &BTreeSet::new(),
            &base,
            &counts,
            &HashSet::new(),
            &custom,
            &EnglishVetoes::default(),
        )
        .unwrap();
        assert_eq!(
            counts_written,
            EnglishWordCounts {
                base: 5,
                base_rows: 5,
                custom_added: 5,
                custom_replaced: 1,
                vetoed: 0
            }
        );
        let rows = |prefix: &str| -> Vec<(String, i64)> {
            connection
                .prepare("SELECT display, weight FROM english_words WHERE word >= ?1 AND word < ?1 || '{' ORDER BY CASE WHEN word = ?1 THEN 0 ELSE 1 END, weight DESC, length(word), word, display")
                .unwrap()
                .query_map([prefix], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        // The Engine's prefix order: counted words first, then the uncounted custom word, then the uncounted base word.
        assert_eq!(
            rows("fig"),
            [
                ("fig".to_owned(), 400_000),
                ("figure".to_owned(), 9_000_000),
                ("Figma".to_owned(), 1),
                ("figma".to_owned(), 1),
                ("figwort".to_owned(), 0)
            ]
        );
        // A counted custom word takes its count, beside the base row that already had it.
        assert_eq!(
            rows("asr"),
            [("ASR".to_owned(), 779_429), ("asr".to_owned(), 779_429)]
        );
        // Replacing the base row does not demote it; a second display joins it.
        assert_eq!(
            rows("webview"),
            [
                ("Webview2".to_owned(), 79_275),
                ("webview".to_owned(), 79_275)
            ]
        );
        // An uncounted word's file weight stays below the least common counted word.
        assert_eq!(rows("loud"), [("LOUD".to_owned(), 12_710)]);
    }

    #[test]
    fn gloss_terms_drop_labels_parts_of_speech_and_explanations() {
        let terms =
            extract_gloss_terms("n. 银行, 堤(河岸)\\n[计] vt. 存款；（口）储蓄\\n[医] 一种病");
        let texts: Vec<_> = terms
            .iter()
            .map(|term| {
                (
                    term.text.as_str(),
                    term.line_index,
                    term.item_index,
                    term.domain_specific,
                )
            })
            .collect();
        assert_eq!(
            texts,
            [
                ("银行", 0, 0, false),
                ("堤", 0, 1, false),
                ("存款", 1, 0, true),
                ("储蓄", 1, 1, true),
                ("一种病", 2, 0, true)
            ]
        );
        assert!(terms[2].reverse_domain_allowed);
        assert!(terms[2].reverse_eligible());
        assert!(!terms[1].reverse_eligible(), "one character is too short");
        assert!(!terms[4].reverse_eligible());
        assert!(extract_gloss_terms("a. 很长很长很长很长的词语").is_empty());
    }

    /// 词性后面的标签也算领域标签（`n. [俚] 黑鬼` 不是通用义项）；`[网络]` 行是领域义项、不进中译英；同一个词在不同的行里各算一次；行首的「X的…」屈折说明记下原形。
    #[test]
    fn labels_after_the_part_of_speech_and_inflection_notes() {
        let terms = extract_gloss_terms(
            "n. 黑人, 黑色\\nn. [俚] 黑鬼\\n[网络] 深棕色\\nvt. [计] 页面, 黑色",
        );
        let texts: Vec<_> = terms
            .iter()
            .map(|term| {
                (
                    term.text.as_str(),
                    term.line_index,
                    term.domain_specific,
                    term.reverse_eligible(),
                )
            })
            .collect();
        assert_eq!(
            texts,
            [
                ("黑人", 0, false, true),
                ("黑色", 0, false, true),
                ("黑鬼", 1, true, false),
                ("深棕色", 2, true, false),
                ("页面", 3, true, true),
                ("黑色", 3, true, true)
            ]
        );
        let parsed = parse_translation("be的现在式第三人称\\n[计] 信息系统");
        assert_eq!(parsed.inflection_of.as_deref(), Some("be"));
        assert_eq!(parsed.terms.len(), 1);
        assert_eq!(
            parse_translation("[计] eat的过去式\\nv. 吃").inflection_of,
            None,
            "a note on a domain line is not the word's inflection"
        );
    }

    /// 英译中按位置：一行之内按义项顺序，权重低于门槛的词排到本行后面；第一条通用行占 10 倍优势，别的行首项更常用十倍以上才排到前面；第一项是通用义项时不拿领域义项凑数。
    #[test]
    fn the_chinese_gloss_follows_sense_position() {
        let terms = extract_gloss_terms("n. 未来的, 未来, 前途, 期货");
        let weights = HashMap::from([("前途".to_owned(), 900_000), ("未来".to_owned(), 500_000)]);
        assert_eq!(
            choose_chinese_gloss(&terms.iter().collect::<Vec<_>>(), &weights),
            "未来；前途"
        );

        let may = extract_gloss_terms("n. 五月\\naux. 愿能, 可以, 愿意");
        let weights = HashMap::from([
            ("五月".to_owned(), 42_900),
            ("愿能".to_owned(), 810),
            ("可以".to_owned(), 13_378_152),
            ("愿意".to_owned(), 770_758),
        ]);
        // 五月 不到 可以 的 1/50，算冷僻义项，排在 aux. 行的常用义项后面。
        assert_eq!(
            choose_chinese_gloss(&may.iter().collect::<Vec<_>>(), &weights),
            "可以；愿意"
        );

        // 「X的」不在拼音表里时看 X：自然的 自己的权重不到 物质的 的 1/50，按 自然 算就不是冷僻义项。
        let natural = extract_gloss_terms("a. 自然的, 物质的");
        let weights = HashMap::from([
            ("自然的".to_owned(), 28_446),
            ("自然".to_owned(), 1_712_884),
            ("物质的".to_owned(), 2_000_000),
        ]);
        assert_eq!(
            choose_chinese_gloss(&natural.iter().collect::<Vec<_>>(), &weights),
            "自然的；物质的"
        );

        let keep = extract_gloss_terms("n. 生计, 维持, 保持\\nvt. 保持, 保存");
        let weights = HashMap::from([
            ("生计".to_owned(), 22_411),
            ("保持".to_owned(), 1_000_000),
            ("保存".to_owned(), 300_000),
        ]);
        assert_eq!(
            choose_chinese_gloss(&keep.iter().collect::<Vec<_>>(), &weights),
            "保持；保存"
        );

        let tv = extract_gloss_terms("电视\\n[计] 电视, 转移向量");
        assert_eq!(
            choose_chinese_gloss(&tv.iter().collect::<Vec<_>>(), &HashMap::new()),
            "电视"
        );
        let domain_only = extract_gloss_terms("[计] 存取管理程序, 寻址方式");
        assert_eq!(
            choose_chinese_gloss(&domain_only.iter().collect::<Vec<_>>(), &HashMap::new()),
            "存取管理程序；寻址方式"
        );
    }

    /// 大小写只看 ECDICT 词头和 SCOWL：小写词头一律小写，不借 `english_words` 的品牌写法（putty 只有 PuTTY）；大写词头由 SCOWL 定，SCOWL 两种都不收时才看 `english_words` 有没有小写写法。
    #[test]
    fn chinese_to_english_keeps_proper_names_capitalised() {
        let set = |words: &[&str]| -> BTreeSet<String> {
            words.iter().map(|word| (*word).to_owned()).collect()
        };
        let candidates = HashMap::from([
            ("china".to_owned(), set(&["China", "china"])),
            ("irish".to_owned(), set(&["Irish"])),
            ("putty".to_owned(), set(&["PuTTY"])),
            ("core".to_owned(), set(&["core"])),
            ("africa".to_owned(), set(&["africa"])),
            ("nasa".to_owned(), set(&["NASA"])),
            ("admin".to_owned(), set(&["Admin", "admin"])),
        ]);
        let attested: HashSet<String> = ["China", "china", "Irish", "putty", "core", "Africa"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let shown = |headword: &str, proper_sense: bool| {
            display_for(
                &candidates,
                &attested,
                headword,
                &headword.to_lowercase(),
                proper_sense,
            )
        };
        assert_eq!(shown("China", true), "China");
        assert_eq!(shown("China", false), "china");
        assert_eq!(shown("Irish", false), "Irish");
        assert_eq!(shown("putty", true), "putty");
        assert_eq!(shown("CORE", true), "core");
        assert_eq!(shown("Africa", false), "Africa");
        assert_eq!(shown("NASA", false), "NASA");
        assert_eq!(shown("ADMIN", true), "ADMIN");
        assert_eq!(shown("ADMIN", false), "admin");
    }

    /// 补充表里只有大写写法、SCOWL 收了小写词形的词补上小写写法；SCOWL 不收小写的品牌名和已有小写写法的词不动。
    #[test]
    fn supplement_words_get_the_lowercase_scowl_attests() {
        let mut words = parse_base_dict_words(
            "PuTTY putty
AdGuard adguard
China china
china china
",
        )
        .unwrap();
        let attested: HashSet<String> = ["putty", "China", "china"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(add_attested_lowercase(&mut words, &attested), 1);
        assert_eq!(displays(&words, "putty"), ["PuTTY", "putty"]);
        assert_eq!(displays(&words, "adguard"), ["AdGuard"]);
        assert_eq!(displays(&words, "china"), ["China", "china"]);
        let leading = leading_display("putty", &words["putty"], &attested);
        assert_eq!(leading, Some("putty"));
    }

    #[test]
    fn quality_combines_collins_oxford_tags_and_frequency() {
        let row = EcdictRow {
            word: "bank",
            translation: "",
            collins: "7",
            oxford: "1",
            tag: "zk GK cet4",
            bnc: "1000",
            frq: "",
            exchange: "",
        };
        assert_eq!(vocabulary_quality(&row), 500_000 + 80_000 + 80_000 + 24_500);
    }

    fn fixture_databases() -> (Connection, Connection) {
        let english = Connection::open_in_memory().unwrap();
        english.execute_batch("CREATE TABLE english_words(word, display, weight); INSERT INTO english_words VALUES ('bank','bank',0),('banks','banks',0),('run','run',0),('rare','rare',0);").unwrap();
        let msime = Connection::open_in_memory().unwrap();
        msime.execute_batch("CREATE TABLE tbl_2_y(key, jp, value, weight); INSERT INTO tbl_2_y VALUES ('yin''hang','yh','银行',900); CREATE TABLE tbl_2_p(key, jp, value, weight); INSERT INTO tbl_2_p VALUES ('pao''bu','pb','跑步',50);").unwrap();
        (english, msime)
    }

    #[test]
    fn glosses_intersect_ecdict_with_both_dictionaries() {
        let (english, msime) = fixture_databases();
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("ecdict.csv");
        std::fs::write(
            &csv,
            "\u{feff}word,phonetic,definition,translation,pos,collins,oxford,tag,bnc,frq,exchange,detail,audio\n\
             bank,,,\"n. 银行, 堤\",,3,1,zk,500,400,s:banks,,\n\
             banks,,,n. 银行,,,,,,100,0:bank,,\n\
             run,,,\"v. 跑步\\nn. 运行\",,5,1,,100,100,,,\n\
             rare,,,a. 稀有的,,,,,,,,,\n\
             absent,,,n. 缺席,,5,1,,1,1,,,\n",
        )
        .unwrap();
        let glosses = derive_glosses(
            &csv,
            &english,
            &msime,
            &HashSet::new(),
            &HashSet::new(),
            &GlossVetoes::default(),
        )
        .unwrap();
        assert_eq!(
            glosses.en_zh.get("bank").map(String::as_str),
            Some("银行；堤")
        );
        assert_eq!(glosses.en_zh.get("banks").map(String::as_str), Some("银行"));
        assert!(!glosses.en_zh.contains_key("absent"));
        // 稀有 has no quality signal, 运行 and 堤 are not pinyin-table values.
        assert_eq!(glosses.zh_en.keys().collect::<Vec<_>>(), ["跑步", "银行"]);
        assert_eq!(glosses.zh_en.get("银行").map(String::as_str), Some("bank"));
        // An excluded word keeps its English-to-Chinese gloss but neither feeds the reverse index nor stands in for its inflections there.
        let excluded = derive_glosses(
            &csv,
            &english,
            &msime,
            &HashSet::from(["bank".to_owned()]),
            &HashSet::new(),
            &GlossVetoes::default(),
        )
        .unwrap();
        assert_eq!(excluded.en_zh, glosses.en_zh);
        assert_eq!(
            excluded.zh_en.get("银行").map(String::as_str),
            Some("banks")
        );

        let mut english = english;
        write_glosses(&mut english, &glosses).unwrap();
        let entries = parse_custom_translations("\u{feff}# c\n华科\tHUST\nbank\t河岸\n").unwrap();
        assert!(entries[0].chinese_to_english && !entries[1].chinese_to_english);
        assert_eq!(
            apply_custom_translations(&mut english, &entries).unwrap(),
            0
        );
        let bank: String = english
            .query_row(
                "SELECT chinese_gloss FROM en_zh_glosses WHERE english='bank'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bank, "河岸");
        // 删除标记删掉 ECDICT 生成的和更早的人工行，后面的行还能再写；源词本来没有释义的删除标记不算错。
        let entries =
            parse_custom_translations("银行\t-\nbank\t-\n华科\t-\n华科\tHUST\nabsent\t-\n")
                .unwrap();
        assert!(entries[0].deletes() && !entries[3].deletes());
        assert_eq!(
            apply_custom_translations(&mut english, &entries).unwrap(),
            3
        );
        let glosses_of = |sql: &str| -> Vec<String> {
            english
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        assert!(
            glosses_of("SELECT chinese_gloss FROM en_zh_glosses WHERE english='bank'").is_empty()
        );
        assert!(
            glosses_of("SELECT english_gloss FROM zh_en_glosses WHERE chinese='银行'").is_empty()
        );
        assert_eq!(
            glosses_of("SELECT english_gloss FROM zh_en_glosses WHERE chinese='华科'"),
            ["HUST"]
        );
        let version: i64 = english
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);
        let sql: String = english
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='zh_en_glosses'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(sql.starts_with("CREATE TABLE \"zh_en_glosses\" ("), "{sql}");
        assert!(parse_custom_translations("only-one-field\n").is_err());
    }

    /// 一个否决集合管全部英文来源：rime-ice 词表（base）、OALDPE 词头和 custom/english.txt 里的蔑称都不写入，任何大小写都不写；curation 文件里的词一个来源也没有时构建失败。
    #[test]
    fn english_vetoes_apply_to_every_source() {
        let base = parse_base_dict_words(
            "tranny tranny\nTranny Tranny\nnigger nigger\nhello hello\nasdfgh asdfgh\n",
        )
        .unwrap();
        let oaldpe = parse_oaldpe_words("beyatch\nzebra\n").unwrap();
        let custom = parse_custom_english("chinks\tChinks\t1\nfigma\tFigma\t1\n").unwrap();
        let counts = parse_google_counts("hello\t100\ntranny\t50\n");
        let vetoes = EnglishVetoes::parse("asdfgh\n").unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        let written = build_english_words(
            &mut connection,
            &oaldpe,
            &base,
            &counts,
            &HashSet::new(),
            &custom,
            &vetoes,
        )
        .unwrap();
        assert_eq!(written.vetoed, 5);
        let mut displays: Vec<String> = connection
            .prepare("SELECT display FROM english_words ORDER BY display")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        displays.sort();
        assert_eq!(displays, ["Figma", "hello", "zebra"]);

        let stale = EnglishVetoes::parse("qwertyu\n").unwrap();
        let error = build_english_words(
            &mut Connection::open_in_memory().unwrap(),
            &oaldpe,
            &base,
            &counts,
            &HashSet::new(),
            &custom,
            &stale,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("qwertyu"), "{error}");
    }
}
