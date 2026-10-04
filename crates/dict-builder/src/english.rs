//! `msime-english.db`: the English prefix-candidate table, the bidirectional glosses derived from ECDICT, and the hand-maintained translation overrides.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use regex::Regex;
use rusqlite::{params, Connection};

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
}

/// The weight a custom row is stored with. Base rows are weighted by the word's Google unigram count, 0 when Google never counted it, and the Engine orders a prefix's completions by that weight after the exact word. A custom row takes the same count when there is one, so a custom word sits where its real frequency puts it and never above a word people type more often. A word Google never counted takes its file weight capped just below the smallest count (`ceiling`), which places it after every counted word and ahead of the uncounted base words at 0: reachable from a prefix once the common words are out of the way, always first on its own full spelling, and unable to outrank a common word whatever weight the file gives it. The file weight is therefore only an ordering among uncounted custom words. Never lower than the base row it replaces, since that row already carries the count.
fn custom_english_weight(entry: &CustomEnglishWord, count: i64, ceiling: i64) -> i64 {
    count.max(entry.weight.min(ceiling))
}

pub fn build_english_words(
    connection: &mut Connection,
    oaldpe: &BTreeSet<String>,
    base: &EnglishWords,
    counts: &HashMap<String, i64>,
    attested: &HashSet<String>,
    custom: &[CustomEnglishWord],
) -> Result<EnglishWordCounts> {
    let words: BTreeSet<&String> = oaldpe.iter().chain(base.keys()).collect();
    let mut result = EnglishWordCounts {
        base: words.len(),
        ..EnglishWordCounts::default()
    };
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

/// Short Chinese headwords from an ECDICT translation, in reading order.
pub fn extract_gloss_terms(translation: &str) -> Vec<GlossTerm> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    // ECDICT stores many line breaks as the two characters `\n`.
    let translation = translation.replace("\\n", "\n");
    for (line_index, raw_line) in text::splitlines(&translation).into_iter().enumerate() {
        let mut line = text::strip(raw_line);
        if line.is_empty() {
            continue;
        }
        let mut domain_specific = false;
        let mut reverse_domain_allowed = false;
        while let Some(found) = LEADING_DOMAIN.find(line) {
            domain_specific = true;
            let label = found.as_str();
            reverse_domain_allowed =
                reverse_domain_allowed || label.contains('计') || label.contains("网络");
            line = &line[found.end()..];
        }
        let line = match LEADING_POS.find(line) {
            Some(found) => text::strip(&line[found.end()..]),
            None => text::strip(line),
        };
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
    terms
}

/// Up to two terms joined with `；`, general senses before domain senses, then by how common the term is as a Chinese candidate, then by position.
pub fn choose_chinese_gloss(terms: &[GlossTerm], weights: &HashMap<String, i64>) -> String {
    let texts: HashSet<&str> = terms.iter().map(|term| term.text.as_str()).collect();
    let mut ranked: Vec<&GlossTerm> = terms
        .iter()
        .filter(|term| {
            !term
                .text
                .strip_suffix('的')
                .is_some_and(|stem| texts.contains(stem))
        })
        .collect();
    ranked.sort_by(|a, b| {
        let key = |term: &GlossTerm| {
            (
                term.domain_specific,
                -weights.get(&term.text).copied().unwrap_or(0),
                term.line_index,
                term.item_index,
                term.text.chars().count(),
            )
        };
        key(a).cmp(&key(b)).then_with(|| a.text.cmp(&b.text))
    });
    ranked
        .iter()
        .take(2)
        .map(|term| term.text.as_str())
        .collect::<Vec<_>>()
        .join("；")
}

struct EnglishEntry {
    reverse_english: String,
    quality: i64,
    terms: Vec<GlossTerm>,
}

pub struct Glosses {
    pub en_zh: BTreeMap<String, String>,
    pub zh_en: BTreeMap<String, String>,
}

fn english_candidates(connection: &Connection) -> Result<HashSet<String>> {
    let mut candidates = HashSet::new();
    let mut statement = connection.prepare("SELECT DISTINCT word FROM english_words")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if let Some(word) = row.get::<_, Option<String>>(0)? {
            let word = word.to_lowercase();
            if is_lowercase_ascii_word(&word) {
                candidates.insert(word);
            }
        }
    }
    Ok(candidates)
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

/// Intersects ECDICT with the English candidates and derives both gloss directions. Only general senses of words with a corpus or core-vocabulary signal feed the Chinese-to-English index, and only for Chinese terms the pinyin tables can produce. Words in `reverse_excluded` get English-to-Chinese glosses but neither feed the Chinese-to-English index nor stand in for an inflection there: the build passes the words only SCOWL brings, so that index keeps choosing among the curated word lists (SCOWL's long tail would otherwise win terms such as 体现 → impersonate).
pub fn derive_glosses(
    ecdict: &Path,
    english: &Connection,
    msime: &Connection,
    reverse_excluded: &HashSet<String>,
) -> Result<Glosses> {
    let candidates = english_candidates(english)?;
    let bytes = std::fs::read(ecdict).with_context(|| format!("reading {}", ecdict.display()))?;
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
        let english = text::strip(row.word).to_lowercase();
        if !candidates.contains(&english) {
            continue;
        }
        let translation = text::strip(row.translation);
        if translation.is_empty() {
            continue;
        }
        let terms = extract_gloss_terms(translation);
        if terms.is_empty() {
            continue;
        }
        let quality = vocabulary_quality(&row);
        // A lemma listed in `exchange` as `0:<lemma>` stands in for its inflections in the reverse index.
        let reverse_english = row
            .exchange
            .split('/')
            .filter_map(|item| item.strip_prefix("0:"))
            .map(|lemma| text::strip(lemma).to_lowercase())
            .find(|lemma| {
                candidates.contains(lemma)
                    && !reverse_excluded.contains(lemma)
                    && is_lowercase_ascii_word(lemma)
            })
            .unwrap_or_else(|| english.clone());
        let replace = entries.get(&english).is_none_or(|previous| {
            (quality, terms.len()) > (previous.quality, previous.terms.len())
        });
        if replace {
            entries.insert(
                english,
                EnglishEntry {
                    reverse_english,
                    quality,
                    terms,
                },
            );
        }
    }

    // Reverse candidates are built only after duplicate English rows are resolved, so a duplicated source row cannot distort the ranking.
    let mut reverse: HashMap<String, HashMap<String, i64>> = HashMap::new();
    for (english, entry) in &entries {
        if entry.quality <= 0 || reverse_excluded.contains(english) {
            continue;
        }
        for term in entry.terms.iter().filter(|term| term.reverse_eligible()) {
            let position_bonus =
                (30_000 - term.line_index as i64 * 2_000 - term.item_index as i64 * 500).max(0);
            let score =
                entry.quality + position_bonus - entry.reverse_english.chars().count() as i64 * 10;
            let best = reverse
                .entry(term.text.clone())
                .or_default()
                .entry(entry.reverse_english.clone())
                .or_insert(score);
            *best = (*best).max(score);
        }
    }

    let all_terms: HashSet<&str> = entries
        .values()
        .flat_map(|entry| entry.terms.iter().map(|term| term.text.as_str()))
        .collect();
    let weights = chinese_term_weights(msime, &all_terms)?;
    let en_zh = entries
        .iter()
        .map(|(english, entry)| {
            (
                english.clone(),
                choose_chinese_gloss(&entry.terms, &weights),
            )
        })
        .collect();
    let zh_en = reverse
        .into_iter()
        .filter(|(chinese, _)| weights.contains_key(chinese))
        .filter_map(|(chinese, candidates)| {
            finalize_reverse_gloss(candidates).map(|gloss| (chinese, gloss))
        })
        .collect();
    Ok(Glosses { en_zh, zh_en })
}

/// The best English words for one Chinese term: at most two, each within 45% of the top score.
fn finalize_reverse_gloss(candidates: HashMap<String, i64>) -> Option<String> {
    let mut ranked: Vec<(String, i64)> = candidates.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(a.0.chars().count().cmp(&b.0.chars().count()))
            .then_with(|| a.0.cmp(&b.0))
    });
    let top = ranked.first()?.1;
    let selected: Vec<String> = ranked
        .into_iter()
        .filter(|(_, score)| score * 100 >= top * 45)
        .take(2)
        .map(|(english, _)| english)
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

pub const CUSTOM_TRANSLATIONS: &str = "custom/translations.txt";

/// `source<TAB>gloss`; a source containing a character from U+3400 up is Chinese-to-English, anything else English-to-Chinese. A later line for the same source wins.
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

/// One line of custom/translations.txt: `None` for a blank or `#` comment line, an error naming the problem (without its location) for a malformed one. Fields after the gloss are ignored. `check-words` validates contributed lines with this same function.
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
        chinese_to_english: source.chars().any(|c| c >= '\u{3400}'),
        source: source.to_owned(),
        gloss: gloss.to_owned(),
    }))
}

pub fn apply_custom_translations(
    connection: &mut Connection,
    entries: &[CustomTranslation],
) -> Result<()> {
    let transaction = connection.transaction()?;
    for entry in entries {
        let sql = if entry.chinese_to_english {
            "INSERT OR REPLACE INTO zh_en_glosses(chinese,english_gloss) VALUES(?1,?2)"
        } else {
            "INSERT OR REPLACE INTO en_zh_glosses(english,chinese_gloss) VALUES(?1,?2)"
        };
        transaction.execute(sql, params![entry.source, entry.gloss])?;
    }
    transaction.commit()?;
    Ok(())
}

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
            build_english_words(&mut connection, &oaldpe, &base, &counts, &attested, &[]).unwrap(),
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
        )
        .unwrap();
        assert_eq!(
            counts_written,
            EnglishWordCounts {
                base: 5,
                base_rows: 5,
                custom_added: 5,
                custom_replaced: 1
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

    #[test]
    fn the_chinese_gloss_prefers_common_general_terms() {
        let terms = extract_gloss_terms("n. 未来的, 未来, 前途, 期货");
        let weights = HashMap::from([("前途".to_owned(), 10), ("未来".to_owned(), 5)]);
        assert_eq!(choose_chinese_gloss(&terms, &weights), "前途；未来");
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
        let glosses = derive_glosses(&csv, &english, &msime, &HashSet::new()).unwrap();
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
        let excluded =
            derive_glosses(&csv, &english, &msime, &HashSet::from(["bank".to_owned()])).unwrap();
        assert_eq!(excluded.en_zh, glosses.en_zh);
        assert_eq!(
            excluded.zh_en.get("银行").map(String::as_str),
            Some("banks")
        );

        let mut english = english;
        write_glosses(&mut english, &glosses).unwrap();
        let entries = parse_custom_translations("\u{feff}# c\n华科\tHUST\nbank\t河岸\n").unwrap();
        assert!(entries[0].chinese_to_english && !entries[1].chinese_to_english);
        apply_custom_translations(&mut english, &entries).unwrap();
        let bank: String = english
            .query_row(
                "SELECT chinese_gloss FROM en_zh_glosses WHERE english='bank'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bank, "河岸");
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
}
