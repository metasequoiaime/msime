//! `msime-others.db`: the emoji, kaomoji and symbol catalogs the emoji panel and the E/M modes read.

use std::collections::HashSet;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use regex::Regex;
use rusqlite::{params, Connection};

use crate::pinyin::Pinyin;
use crate::sqlite;
use crate::text;

const VS16: char = '\u{fe0f}';

/// English function words that carry no search meaning. Directional words that do (up, down, no, not, on, off, out) are deliberately absent.
const EN_STOPWORDS: &[&str] = &[
    "a", "an", "the", "of", "in", "at", "by", "for", "with", "to", "from", "into", "onto", "upon",
    "via", "and", "or", "but", "nor", "yet", "so", "as", "if", "then", "than", "that", "this",
    "these", "those", "be", "is", "are", "was", "were", "am", "been", "being", "it", "its", "all",
    "any", "both", "each", "few", "more", "most", "other", "some", "such", "only", "own", "same",
    "too", "very", "can", "will", "just", "should", "would", "could", "do", "does", "did", "have",
    "has", "had", "there", "here", "which", "who", "whom", "whose",
];

const WORD_PUNCTUATION: &str = ".,:;!?()[]{}<>-/\"'`";

fn is_cjk(text: &str) -> bool {
    text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}

fn strip_vs(text: &str) -> String {
    text.replace(VS16, "")
}

/// A lowercase English search word, or `None` for punctuation-only tokens, non-letters and stopwords.
fn english_word(token: &str) -> Option<&str> {
    let word = text::strip_chars(token, WORD_PUNCTUATION);
    (text::is_alpha(word) && !EN_STOPWORDS.contains(&word)).then_some(word)
}

fn full_and_initials(pinyin: &Pinyin, keyword: &str) -> (String, String) {
    let items = pinyin.lazy(keyword);
    let full = items.concat();
    let initials = items
        .iter()
        .filter_map(|item| item.chars().next())
        .collect();
    (full, initials)
}

/// 注释行：`#` 开头、不含制表符。`#` 开头又带制表符的行只有两种是数据行：首字段恰好是 `#`（英文关键词 `#`），或 `#` 后紧跟 U+FE0F、U+20E3（emoji `#️⃣` 的目录行和关键词行）。以前 `#` 开头一律当注释，这两种行被丢掉：`#️⃣` 落到目录外按 Symbols 排在最后，也少了 `#` 这个关键词。其余 `#` 开头又带制表符的行像是用 `#` 停用的数据行（`#鸡巴<TAB>🐔`），照收会让 `#鸡巴` 成为关键词、拒绝表也拦不住，所以报错：要停用一行就删掉它。
fn is_comment(line: &str) -> Result<bool> {
    let Some(rest) = line.strip_prefix('#') else {
        return Ok(false);
    };
    if !line.contains('\t') {
        return Ok(true);
    }
    if rest.starts_with(['\t', '\u{fe0f}', '\u{20e3}']) {
        return Ok(false);
    }
    bail!("{line:?} starts with # and has a tab, so it is neither a comment nor a # or #\u{fe0f}\u{20e3} data line; delete the line instead of commenting it out")
}

/// 关键词表里的数据行：1 起的行号、关键词、表情或颜文字，都去掉了首尾空白；关键词或表情为空的行跳过。
fn keyword_lines(source: &str) -> Result<Vec<(usize, &str, &str)>> {
    let mut lines = Vec::new();
    for (index, line) in text::splitlines(source).into_iter().enumerate() {
        let stripped = text::strip(line);
        if stripped.is_empty()
            || is_comment(stripped).with_context(|| format!("line {}", index + 1))?
        {
            continue;
        }
        let (keyword, item) = stripped
            .split_once('\t')
            .with_context(|| format!("expected keyword<TAB>item: {line:?}"))?;
        let (keyword, item) = (text::strip(keyword), text::strip(item));
        if keyword.is_empty() || item.is_empty() {
            continue;
        }
        lines.push((index + 1, keyword, item));
    }
    Ok(lines)
}

/// 读取 `keyword<TAB>item` 行，按条目首次出现的顺序分组，每个条目下的关键词去重。
pub fn load_keyword_map(source: &str) -> Result<IndexMap<String, Vec<String>>> {
    let mut mapping: IndexMap<String, Vec<String>> = IndexMap::new();
    for (_, keyword, item) in keyword_lines(source)? {
        let keywords = mapping.entry(item.to_owned()).or_default();
        if !keywords.iter().any(|known| known == keyword) {
            keywords.push(keyword.to_owned());
        }
    }
    Ok(mapping)
}

// ---- emoji ----

/// `emoji<TAB>category<TAB>sort_order`, regenerated from Unicode's emoji-test.txt when the Unicode version moves.
pub fn read_emoji_catalog(source: &str) -> Result<IndexMap<String, (String, i64)>> {
    let mut catalog = IndexMap::new();
    for line in text::splitlines(source) {
        let stripped = text::strip(line);
        if stripped.is_empty() || is_comment(stripped).context("emoji_catalog.txt")? {
            continue;
        }
        let fields: Vec<&str> = stripped.split('\t').collect();
        let [emoji, category, order] = fields[..] else {
            bail!("emoji_catalog.txt: expected emoji, category and order: {line:?}");
        };
        let order = text::strip(order)
            .parse()
            .with_context(|| format!("emoji_catalog.txt: bad order {order:?}"))?;
        catalog.insert(emoji.to_owned(), (category.to_owned(), order));
    }
    Ok(catalog)
}

fn index_by_stripped(mapping: &IndexMap<String, Vec<String>>) -> IndexMap<String, Vec<String>> {
    let mut index: IndexMap<String, Vec<String>> = IndexMap::new();
    for (emoji, keywords) in mapping {
        let known = index.entry(strip_vs(emoji)).or_default();
        for keyword in keywords {
            if !known.contains(keyword) {
                known.push(keyword.clone());
            }
        }
    }
    index
}

struct KeywordSource<'a> {
    mapping: &'a IndexMap<String, Vec<String>>,
    stripped: IndexMap<String, Vec<String>>,
}

impl<'a> KeywordSource<'a> {
    fn new(mapping: &'a IndexMap<String, Vec<String>>) -> Self {
        Self {
            mapping,
            stripped: index_by_stripped(mapping),
        }
    }

    /// The emoji's own keywords, then those of every spelling that differs only by VS16.
    fn keywords(&self, emoji: &str) -> impl Iterator<Item = &String> {
        self.mapping
            .get(emoji)
            .into_iter()
            .flatten()
            .chain(self.stripped.get(&strip_vs(emoji)).into_iter().flatten())
    }
}

fn dedup<'a>(keywords: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut seen = HashSet::new();
    keywords
        .filter(|keyword| seen.insert(keyword.as_str()))
        .cloned()
        .collect()
}

/// Chinese keywords give their full pinyin and, when different, their initials; English keywords give their words.
fn emoji_search_keys(pinyin: &Pinyin, merged: &[String]) -> Vec<String> {
    let mut keys = Vec::new();
    for keyword in merged {
        if is_cjk(keyword) {
            let (full, initials) = full_and_initials(pinyin, keyword);
            if !full.is_empty() {
                keys.push(full.clone());
            }
            if !initials.is_empty() && initials != full {
                keys.push(initials);
            }
        } else {
            let lowered = keyword.to_lowercase();
            keys.extend(
                text::split_whitespace(&lowered)
                    .filter_map(english_word)
                    .map(str::to_owned),
            );
        }
    }
    keys
}

#[derive(Debug, PartialEq, Eq)]
pub struct EmojiRow {
    pub emoji: String,
    pub category: String,
    pub sort_order: i64,
    pub keywords: String,
    pub pinyin: String,
}

/// Catalog emoji in catalog order, then every emoji the keyword files know that the catalog lacks, as Symbols. Returns the rows and each row's search keys.
pub fn emoji_rows(
    pinyin: &Pinyin,
    catalog: &IndexMap<String, (String, i64)>,
    zh: &IndexMap<String, Vec<String>>,
    en: &IndexMap<String, Vec<String>>,
) -> (Vec<EmojiRow>, Vec<Vec<String>>) {
    let (zh_source, en_source) = (KeywordSource::new(zh), KeywordSource::new(en));
    let row = |emoji: &str, category: &str, sort_order: i64| {
        let merged = dedup(zh_source.keywords(emoji).chain(en_source.keywords(emoji)));
        let pinyin_column = merged
            .iter()
            .filter(|keyword| is_cjk(keyword))
            .map(|keyword| pinyin.lazy(keyword).concat())
            .filter(|full| !full.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let keys = emoji_search_keys(pinyin, &merged);
        let row = EmojiRow {
            emoji: emoji.to_owned(),
            category: category.to_owned(),
            sort_order,
            keywords: merged.join(" "),
            pinyin: pinyin_column,
        };
        (row, keys)
    };
    let mut rows = Vec::new();
    let mut keys = Vec::new();
    let mut known: HashSet<String> = catalog.keys().map(|emoji| strip_vs(emoji)).collect();
    for (emoji, (category, order)) in catalog {
        let (built, search) = row(emoji, category, *order);
        rows.push(built);
        keys.push(search);
    }
    let mut extra_order = catalog
        .values()
        .map(|(_, order)| *order)
        .max()
        .unwrap_or(-1)
        + 1;
    for emoji in zh.keys().chain(en.keys()) {
        if !known.insert(strip_vs(emoji)) {
            continue;
        }
        let (built, search) = row(emoji, "Symbols", extra_order);
        rows.push(built);
        keys.push(search);
        extra_order += 1;
    }
    (rows, keys)
}

pub fn build_emoji(
    connection: &mut Connection,
    rows: &[EmojiRow],
    keys: &[Vec<String>],
) -> Result<usize> {
    let transaction = connection.transaction()?;
    transaction.execute_batch("DROP TABLE IF EXISTS emoji")?;
    transaction.execute_batch("DROP INDEX IF EXISTS idx_emoji_category_order")?;
    transaction.execute_batch("\n            CREATE TABLE emoji (\n                emoji TEXT NOT NULL,\n                category TEXT NOT NULL,\n                sort_order INTEGER NOT NULL,\n                keywords TEXT NOT NULL,\n                pinyin TEXT NOT NULL,\n                PRIMARY KEY (emoji)\n            ) WITHOUT ROWID\n            ")?;
    transaction
        .execute_batch("CREATE INDEX idx_emoji_category_order ON emoji(category, sort_order)")?;
    {
        let mut insert = transaction.prepare("INSERT INTO emoji (emoji, category, sort_order, keywords, pinyin) VALUES (?, ?, ?, ?, ?)")?;
        for row in rows {
            insert.execute(params![
                row.emoji,
                row.category,
                row.sort_order,
                row.keywords,
                row.pinyin
            ])?;
        }
    }
    transaction.execute_batch("DROP TABLE IF EXISTS emoji_pinyin")?;
    transaction.execute_batch("\n            CREATE TABLE emoji_pinyin (\n                key TEXT NOT NULL,\n                emoji TEXT NOT NULL,\n                sort_order INTEGER NOT NULL,\n                PRIMARY KEY (key, emoji)\n            ) WITHOUT ROWID\n            ")?;
    let mut count = 0;
    {
        // One row per search key, so a typed code can prefix-match any keyword of an emoji.
        let mut insert = transaction
            .prepare("INSERT INTO emoji_pinyin (key, emoji, sort_order) VALUES (?, ?, ?)")?;
        let mut seen = HashSet::new();
        for (row, row_keys) in rows.iter().zip(keys) {
            for key in row_keys {
                if !key.is_empty() && seen.insert((key.as_str(), row.emoji.as_str())) {
                    insert.execute(params![key, row.emoji, row.sort_order])?;
                    count += 1;
                }
            }
        }
    }
    sqlite::analyze(&transaction, false)?;
    transaction.commit()?;
    Ok(count)
}

// ---- kaomoji ----

fn kaomoji_columns(pinyin: &Pinyin, keyword: &str) -> (String, String) {
    if is_cjk(keyword) {
        let (full, initials) = full_and_initials(pinyin, keyword);
        let (full, initials) = (full.to_lowercase(), initials.to_lowercase());
        let initials = if initials == full {
            String::new()
        } else {
            initials
        };
        return (full, initials);
    }
    let lowered = keyword.to_lowercase();
    // An ASCII keyword that is already a spaced pinyin code ("zai xiang").
    if keyword.contains(' ') && lowered.bytes().all(|b| b.is_ascii_lowercase() || b == b' ') {
        let syllables: Vec<&str> = text::split_whitespace(&lowered).collect();
        let full = syllables.concat();
        let initials: String = syllables
            .iter()
            .filter_map(|syllable| syllable.chars().next())
            .collect();
        let initials = if initials == full {
            String::new()
        } else {
            initials
        };
        return (full, initials);
    }
    match english_word(&lowered) {
        Some(word) => (word.to_owned(), String::new()),
        None => (String::new(), String::new()),
    }
}

pub fn build_kaomoji(
    connection: &mut Connection,
    pinyin: &Pinyin,
    mapping: &IndexMap<String, Vec<String>>,
) -> Result<(usize, usize)> {
    let transaction = connection.transaction()?;
    transaction.execute_batch("DROP TABLE IF EXISTS kaomoji; DROP TABLE IF EXISTS kaomoji_pinyin; DROP INDEX IF EXISTS idx_kaomoji_jianpin; DROP TABLE IF EXISTS kaomoji_catalog;")?;
    transaction.execute_batch("\n            CREATE TABLE kaomoji (\n                pinyin TEXT NOT NULL,\n                jianpin TEXT NOT NULL,\n                kaomoji TEXT NOT NULL,\n                sort_order INTEGER NOT NULL,\n                PRIMARY KEY (pinyin, jianpin, kaomoji)\n            ) WITHOUT ROWID\n            ")?;
    transaction.execute_batch("CREATE INDEX idx_kaomoji_jianpin ON kaomoji(jianpin)")?;
    let mut rows = 0;
    {
        let mut insert = transaction.prepare(
            "INSERT INTO kaomoji (pinyin, jianpin, kaomoji, sort_order) VALUES (?, ?, ?, ?)",
        )?;
        let mut seen = HashSet::new();
        for (order, (kaomoji, keywords)) in mapping.iter().enumerate() {
            for keyword in keywords {
                let (full, initials) = kaomoji_columns(pinyin, keyword);
                if full.is_empty()
                    || !seen.insert((full.clone(), initials.clone(), kaomoji.as_str()))
                {
                    continue;
                }
                insert.execute(params![full, initials, kaomoji, order as i64])?;
                rows += 1;
            }
        }
    }
    transaction.execute_batch("\n            CREATE TABLE kaomoji_catalog (\n                kaomoji TEXT NOT NULL,\n                sort_order INTEGER NOT NULL,\n                keywords TEXT NOT NULL,\n                PRIMARY KEY (kaomoji)\n            ) WITHOUT ROWID\n            ")?;
    {
        let mut insert = transaction.prepare(
            "INSERT INTO kaomoji_catalog (kaomoji, sort_order, keywords) VALUES (?, ?, ?)",
        )?;
        for (order, (kaomoji, keywords)) in mapping.iter().enumerate() {
            insert.execute(params![kaomoji, order as i64, keywords.join(" ")])?;
        }
    }
    sqlite::analyze(&transaction, false)?;
    transaction.commit()?;
    Ok((rows, mapping.len()))
}

// ---- 关键词拒绝表 ----

/// 表情与颜文字关键词的拒绝表，在 `resources/dictionary-sources/` 下；格式与收词依据见文件头。
pub const KEYWORD_VETOES: &str = "keyword-vetoes.txt";
/// 拒绝表管的三份关键词表，相对 `resources/dictionary-sources/`。
pub const KEYWORD_FILES: [&str; 3] = [
    "emoji/emoji.txt",
    "emoji/emoji_en.txt",
    "kaomoji/kaomoji.txt",
];

/// 拒绝表的一条：`item` 为 `None` 时拒绝这个关键词对应的任何表情或颜文字。
struct KeywordVeto<'a> {
    keyword: &'a str,
    item: Option<&'a str>,
    line: usize,
}

fn parse_keyword_vetoes(source: &str) -> Result<Vec<KeywordVeto<'_>>> {
    let mut vetoes: Vec<KeywordVeto> = Vec::new();
    for (index, line) in text::splitlines(text::without_bom(source))
        .into_iter()
        .enumerate()
    {
        let stripped = text::strip(line);
        let at = index + 1;
        if stripped.is_empty()
            || is_comment(stripped).with_context(|| format!("{KEYWORD_VETOES}:{at}"))?
        {
            continue;
        }
        let fields: Vec<&str> = stripped.split('\t').map(text::strip).collect();
        let (keyword, item) = match fields[..] {
            [keyword] => (keyword, None),
            [keyword, item] if !item.is_empty() => (keyword, Some(item)),
            _ => bail!("{KEYWORD_VETOES}:{at}: expected keyword or keyword<TAB>item: {line:?}"),
        };
        if keyword.is_empty() {
            bail!("{KEYWORD_VETOES}:{at}: the keyword is empty");
        }
        if vetoes.iter().any(|known| {
            same_keyword(known.keyword, keyword) && known.item.map(strip_vs) == item.map(strip_vs)
        }) {
            bail!("{KEYWORD_VETOES}:{at}: {stripped:?} is listed twice");
        }
        vetoes.push(KeywordVeto {
            keyword,
            item,
            line: at,
        });
    }
    Ok(vetoes)
}

/// 关键词按原样比较，ASCII 字母不分大小写（`NMSL` 与 `nmsl`）。
fn same_keyword(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// 用拒绝表检查三份关键词表（`files` 是路径和内容）：任何一行命中，或者成对条目里的表情在哪份表里都没有了，都报错并列出全部问题。三份表一起查，单跑 Emoji 或 Kaomoji 阶段时也能判断成对条目是否过期。
pub fn check_keyword_vetoes(vetoes: &str, files: &[(&str, &str)]) -> Result<()> {
    let vetoes = parse_keyword_vetoes(vetoes)?;
    let mut problems = Vec::new();
    let mut items = HashSet::new();
    for (path, source) in files {
        for (line, keyword, item) in keyword_lines(source).with_context(|| path.to_string())? {
            items.insert(strip_vs(item));
            for veto in &vetoes {
                if same_keyword(veto.keyword, keyword)
                    && veto
                        .item
                        .is_none_or(|vetoed| strip_vs(vetoed) == strip_vs(item))
                {
                    problems.push(format!(
                        "{path}:{line}: {keyword} -> {item} is vetoed by {KEYWORD_VETOES}:{}",
                        veto.line
                    ));
                }
            }
        }
    }
    for veto in &vetoes {
        if let Some(item) = veto.item {
            if !items.contains(&strip_vs(item)) {
                problems.push(format!(
                    "{KEYWORD_VETOES}:{}: {item} is in none of the keyword files; drop the stale line",
                    veto.line
                ));
            }
        }
    }
    if !problems.is_empty() {
        bail!("{}", problems.join("; "));
    }
    Ok(())
}

// ---- symbols ----

/// 表情面板符号页的分类与符号，采集自 tw.piliapp.com/symbol/，在 msime-dictionary（经 `--dictionary` 读取）；许可未确认，见词库仓 NOTICE.md 的「待解决」。
pub const PILIAPP_SYMBOLS: &str = "sources/symbols/piliapp-symbols.txt";

static CATEGORY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^# \[piliapp/symbol/([^/\]]+)/\][\s\x1c-\x1f]*(.+)[\s\x1c-\x1f]*$")
        .expect("valid regex")
});

/// Parent tabs in navigation order; each PiliApp category slug with its English section heading, in heading order.
const PARENT_TABS: &[(&str, &[(&str, &str)])] = &[
    (
        "Stars and shapes",
        &[
            ("star", "Stars"),
            ("asterisk", "Asterisks"),
            ("circle", "Circles"),
            ("triangle", "Triangles"),
            ("square", "Squares"),
            ("other-shapes", "Other shapes"),
            ("bullet-point", "Bullet points"),
        ],
    ),
    (
        "Arrows and lines",
        &[
            ("arrow", "Arrows"),
            ("line", "Lines"),
            ("random-lines", "Random lines"),
        ],
    ),
    (
        "Punctuation",
        &[
            ("punctuation", "Punctuation"),
            ("brackets", "Brackets"),
            ("quotation-mark", "Quotation marks"),
            ("pilcrow", "Pilcrow"),
            ("tick", "Check marks"),
            ("x-mark", "X marks"),
        ],
    ),
    (
        "Math",
        &[
            ("math", "Math"),
            ("number", "Numbers"),
            ("fraction", "Fractions"),
            ("pi", "Pi"),
            ("subscript-superscript", "Super/subscripts"),
        ],
    ),
    (
        "Currency",
        &[
            ("currency", "Currency"),
            ("business", "Business"),
            ("unit", "Units"),
        ],
    ),
    (
        "Hearts",
        &[
            ("heart", "Hearts"),
            ("eye", "Eyes"),
            ("monochrome", "Monochrome"),
        ],
    ),
    (
        "Letters",
        &[
            ("latin", "Latin letters"),
            ("latin-extended", "Latin extended"),
            ("greek", "Greek letters"),
            ("kana", "Japanese kana"),
            ("braille", "Braille"),
            ("korean", "Korean"),
        ],
    ),
    (
        "Games",
        &[
            ("card-suit", "Card suits"),
            ("chess", "Chess"),
            ("dice", "Dice"),
        ],
    ),
    (
        "Culture",
        &[
            ("culture", "Religion and culture"),
            ("cross", "Crosses"),
            ("zodiac", "Zodiac"),
            ("gender", "Gender"),
            ("totem", "Totems"),
        ],
    ),
    (
        "Animals and nature",
        &[
            ("animals", "Animals"),
            ("flower", "Flowers"),
            ("weather", "Weather"),
        ],
    ),
    (
        "People and activity",
        &[("activity", "People and activity")],
    ),
    (
        "More",
        &[
            ("music", "Music"),
            ("tech", "Technical"),
            ("menu", "Menu"),
            ("misc", "Miscellaneous"),
            ("confidential", "Block elements"),
            ("mashup", "Mashup"),
        ],
    ),
];

/// 符号表的注释行：单独一个 `#`，或 `# ` 加说明。符号表一行一个符号、没有制表符，不能套用关键词表的 `is_comment`；以前 `#` 开头一律当注释，项目清单分类里的 `#️⃣` 因此被丢掉，同一分类的 `*️⃣` 却在。
fn is_piliapp_comment(line: &str) -> bool {
    line == "#" || line.starts_with("# ")
}

/// slug -> (Chinese title, symbols). HTML entities such as `&dollar;` are decoded; a symbol repeated within a category is kept once.
pub fn parse_piliapp(source: &str) -> IndexMap<String, (String, Vec<String>)> {
    let mut categories: IndexMap<String, (String, Vec<String>)> = IndexMap::new();
    let mut current: Option<(String, String, Vec<String>, HashSet<String>)> = None;
    let mut flush = |current: &mut Option<(String, String, Vec<String>, HashSet<String>)>| {
        if let Some((slug, title, symbols, _)) = current.take() {
            categories.insert(slug, (title, symbols));
        }
    };
    for raw in text::splitlines(source) {
        let line = text::strip(raw);
        if line.is_empty() {
            continue;
        }
        if let Some(captures) = CATEGORY_LINE.captures(line) {
            flush(&mut current);
            let slug = text::strip(&captures[1]).to_owned();
            let title = text::strip(&captures[2]).to_owned();
            current = Some((slug, title, Vec::new(), HashSet::new()));
            continue;
        }
        if is_piliapp_comment(line) {
            continue;
        }
        let Some((_, _, symbols, seen)) = current.as_mut() else {
            continue;
        };
        let decoded = html_escape::decode_html_entities(line);
        let symbol = text::strip(&decoded);
        if !symbol.is_empty() && seen.insert(symbol.to_owned()) {
            symbols.push(symbol.to_owned());
        }
    }
    flush(&mut current);
    categories
}

#[derive(Debug, PartialEq, Eq)]
pub struct SymbolRow {
    pub symbol: String,
    pub category: String,
    pub parent: String,
    pub sort_order: i64,
    pub keywords: String,
}

pub fn symbol_rows(
    pinyin: &Pinyin,
    categories: &IndexMap<String, (String, Vec<String>)>,
) -> Result<Vec<SymbolRow>> {
    let mut rows = Vec::new();
    let mut used = HashSet::new();
    for (parent, slugs) in PARENT_TABS {
        for (slug, category) in *slugs {
            used.insert(*slug);
            let Some((chinese, symbols)) = categories
                .get(*slug)
                .filter(|(_, symbols)| !symbols.is_empty())
            else {
                continue;
            };
            let mut parts = vec![
                category.to_string(),
                chinese.clone(),
                slug.replace('-', " "),
                parent.to_string(),
            ];
            if is_cjk(chinese) {
                let (full, initials) = full_and_initials(pinyin, chinese);
                let (full, initials) = (full.to_lowercase(), initials.to_lowercase());
                if !full.is_empty() {
                    parts.push(full.clone());
                }
                if !initials.is_empty() && initials != full {
                    parts.push(initials);
                }
            }
            let keywords = parts
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            for symbol in symbols {
                rows.push(SymbolRow {
                    symbol: symbol.clone(),
                    category: category.to_string(),
                    parent: parent.to_string(),
                    sort_order: rows.len() as i64,
                    keywords: keywords.clone(),
                });
            }
        }
    }
    let leftovers: Vec<&String> = categories
        .iter()
        .filter(|(slug, (_, symbols))| !used.contains(slug.as_str()) && !symbols.is_empty())
        .map(|(slug, _)| slug)
        .collect();
    if !leftovers.is_empty() {
        bail!("unmapped PiliApp categories: {leftovers:?}");
    }
    Ok(rows)
}

pub fn build_symbols(connection: &mut Connection, rows: &[SymbolRow]) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch("DROP TABLE IF EXISTS symbol_catalog")?;
    transaction.execute_batch("\n            CREATE TABLE symbol_catalog (\n                symbol TEXT NOT NULL,\n                category TEXT NOT NULL,\n                parent_category TEXT NOT NULL,\n                sort_order INTEGER NOT NULL,\n                keywords TEXT NOT NULL,\n                PRIMARY KEY (symbol, category)\n            ) WITHOUT ROWID\n            ")?;
    transaction.execute_batch("CREATE INDEX idx_symbol_catalog_parent_order ON symbol_catalog(parent_category, sort_order)")?;
    {
        let mut insert = transaction
            .prepare("INSERT INTO symbol_catalog (symbol, category, parent_category, sort_order, keywords) VALUES (?, ?, ?, ?, ?)")?;
        for row in rows {
            insert.execute(params![
                row.symbol,
                row.category,
                row.parent,
                row.sort_order,
                row.keywords
            ])?;
        }
    }
    sqlite::analyze(&transaction, false)?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pinyin() -> Pinyin {
        Pinyin::with_overrides("鱼眼\tyu\tyan\n").unwrap()
    }

    #[test]
    fn emoji_rows_merge_keywords_across_vs16_spellings() {
        let catalog = read_emoji_catalog("# emoji<TAB>category<TAB>sort_order\n😀\tSmileys and emotion\t0\n☺\u{fe0f}\tSmileys and emotion\t1\n").unwrap();
        let zh = load_keyword_map("# c\n笑脸\t😀\n微笑\t☺\n扭曲\t🫪\n鱼眼\t🫪\n笑脸\t😀\n").unwrap();
        let en = load_keyword_map("grinning face\t😀\nsmiling face\t☺\u{fe0f}\nthe\t🫪\n").unwrap();
        let (rows, keys) = emoji_rows(&pinyin(), &catalog, &zh, &en);
        assert_eq!(
            rows[0],
            EmojiRow {
                emoji: "😀".into(),
                category: "Smileys and emotion".into(),
                sort_order: 0,
                keywords: "笑脸 grinning face".into(),
                pinyin: "xiaolian".into()
            }
        );
        assert_eq!(keys[0], ["xiaolian", "xl", "grinning", "face"]);
        assert_eq!(rows[1].keywords, "微笑 smiling face");
        assert_eq!(rows[2].emoji, "🫪");
        assert_eq!(
            (rows[2].category.as_str(), rows[2].sort_order),
            ("Symbols", 2)
        );
        assert_eq!(keys[2], ["niuqu", "nq", "yuyan", "yy"]);
        assert_eq!(rows.len(), 3);

        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(build_emoji(&mut connection, &rows, &keys).unwrap(), 12);
    }

    /// `#` 开头的数据行（emoji `#️⃣` 的目录行、英文关键词 `#`）照常读进来，只有不含制表符的 `#` 行是注释。
    #[test]
    fn hash_led_data_lines_are_not_comments() {
        let catalog = read_emoji_catalog(
            "# emoji<TAB>category<TAB>sort_order\n#\u{fe0f}\u{20e3}\tSymbols\t0\n*\u{fe0f}\u{20e3}\tSymbols\t1\n",
        )
        .unwrap();
        assert_eq!(
            catalog.keys().map(String::as_str).collect::<Vec<_>>(),
            ["#\u{fe0f}\u{20e3}", "*\u{fe0f}\u{20e3}"]
        );
        let en = load_keyword_map(
            "# Format: keyword<TAB>emoji\n#\t#\u{fe0f}\u{20e3}\nkeycap: #\t#\u{fe0f}\u{20e3}\n",
        )
        .unwrap();
        assert_eq!(en["#\u{fe0f}\u{20e3}"], ["#", "keycap: #"]);
    }

    /// 用 `#` 停用的数据行不会作为 `#鸡巴` 这样的关键词收进来，三份关键词表、目录和拒绝表都报错；`#` 开头不带制表符的照旧是注释。
    #[test]
    fn commented_out_data_lines_are_rejected() {
        let error = load_keyword_map("# c\n#鸡巴\t🐔\n鸡\t🐔\n").unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("line 2"), "{error}");
        assert!(error.contains("delete the line"), "{error}");
        assert!(load_keyword_map("#  鸡巴\t🐔\n").is_err());
        assert!(read_emoji_catalog("#😀\tSmileys and emotion\t0\n").is_err());
        assert!(check_keyword_vetoes("#nmsl\t🤬\n", &[]).is_err());
        let error = check_keyword_vetoes("nmsl\n", &[("emoji.txt", "#草泥马\t🦙\n")]).unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("emoji.txt"), "{error}");
        assert_eq!(
            load_keyword_map("#鸡巴 🐔\n# keyword<TAB>emoji\n")
                .unwrap()
                .len(),
            0
        );
    }

    /// 仓库里的 #️⃣ 按目录的位置排，不再落到目录外排最后。
    #[test]
    fn the_repository_catalog_keeps_the_keycap_number_sign() {
        let catalog = read_emoji_catalog(&repository_file("emoji/emoji_catalog.txt")).unwrap();
        let (category, order) = &catalog["#\u{fe0f}\u{20e3}"];
        assert_eq!((category.as_str(), *order), ("Symbols", 1558));
    }

    fn repository_file(path: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources")
                .join(path),
        )
        .unwrap()
    }

    #[test]
    fn keyword_vetoes_reject_listed_keywords_and_pairs() {
        let vetoes = "# c\nnmsl\n白色小方块\t▪\n";
        let clean = "鸡\t🐔\n黑色小方块\t▪\n白色小方块\t▫\n";
        check_keyword_vetoes(vetoes, &[("emoji.txt", clean)]).unwrap();

        // 只拒绝关键词的条目不管表情，ASCII 不分大小写。
        let error = check_keyword_vetoes(
            vetoes,
            &[("emoji.txt", clean), ("kaomoji.txt", "NMSL\t(-_-)\n")],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("kaomoji.txt:1: NMSL -> (-_-)"), "{error}");
        assert!(error.contains("keyword-vetoes.txt:2"), "{error}");

        // 成对的条目只拒绝那一对，比较时忽略 U+FE0F。
        let error = check_keyword_vetoes(vetoes, &[("emoji.txt", "白色小方块\t▪\u{fe0f}\n")])
            .unwrap_err()
            .to_string();
        assert!(error.contains("emoji.txt:1: 白色小方块"), "{error}");

        // 成对条目里的表情哪份表都没有了，是过期的条目。
        let error = check_keyword_vetoes(vetoes, &[("emoji.txt", "鸡\t🐔\n")])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("keyword-vetoes.txt:3: ▪ is in none"),
            "{error}"
        );

        assert!(check_keyword_vetoes("nmsl\nNMSL\n", &[]).is_err());
        assert!(check_keyword_vetoes("a\tb\tc\n", &[]).is_err());
        assert!(check_keyword_vetoes("a\t\tb\n", &[]).is_err());
    }

    /// 仓库里的三份关键词表过得了拒绝表：删掉的粗口关键词和改正过的方块颜色没有回来。
    #[test]
    fn the_repository_keyword_files_pass_the_vetoes() {
        let files: Vec<(&str, String)> = KEYWORD_FILES
            .iter()
            .map(|path| (*path, repository_file(path)))
            .collect();
        let files: Vec<(&str, &str)> = files
            .iter()
            .map(|(path, source)| (*path, source.as_str()))
            .collect();
        check_keyword_vetoes(&repository_file(KEYWORD_VETOES), &files).unwrap();
    }

    #[test]
    fn kaomoji_keywords_become_pinyin_initials_or_words() {
        let pinyin = pinyin();
        assert_eq!(
            kaomoji_columns(&pinyin, "害羞"),
            ("haixiu".into(), "hx".into())
        );
        assert_eq!(
            kaomoji_columns(&pinyin, "剪jj"),
            ("jianjj".into(), "jj".into())
        );
        assert_eq!(
            kaomoji_columns(&pinyin, "zai xiang"),
            ("zaixiang".into(), "zx".into())
        );
        assert_eq!(
            kaomoji_columns(&pinyin, "Kiss!"),
            ("kiss".into(), String::new())
        );
        assert_eq!(
            kaomoji_columns(&pinyin, "the"),
            (String::new(), String::new())
        );
        assert_eq!(kaomoji_columns(&pinyin, "哭"), ("ku".into(), "k".into()));
        assert_eq!(
            kaomoji_columns(&pinyin, "a"),
            (String::new(), String::new())
        );

        let mapping =
            load_keyword_map("kiss\t!(*￣(￣　*)\nqian\t$_$\n贪心\t$_$\nthe\t$_$\n").unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(
            build_kaomoji(&mut connection, &pinyin, &mapping).unwrap(),
            (3, 2)
        );
        let order: i64 = connection
            .query_row(
                "SELECT sort_order FROM kaomoji WHERE pinyin='tanxin'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(order, 1);
        let keywords: String = connection
            .query_row(
                "SELECT keywords FROM kaomoji_catalog WHERE kaomoji='$_$'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(keywords, "qian 贪心 the");
    }

    #[test]
    fn symbols_decode_entities_and_group_by_parent_tab() {
        let source = "\u{feff}# 特殊符号表\n\n# [piliapp/symbol/currency/] 货币符号\n&dollar;\n€\n€\n# [piliapp/symbol/fraction/] 分数\n&frac13;\n# [piliapp/symbol/star/] 星星\n*\n# [piliapp/symbol/tick/] 勾\n";
        let categories = parse_piliapp(source);
        assert_eq!(
            categories["currency"],
            ("货币符号".to_owned(), vec!["$".to_owned(), "€".to_owned()])
        );
        assert_eq!(categories["fraction"].1, ["⅓"]);
        let rows = symbol_rows(&pinyin(), &categories).unwrap();
        let summary: Vec<_> = rows
            .iter()
            .map(|row| (row.symbol.as_str(), row.parent.as_str(), row.sort_order))
            .collect();
        assert_eq!(
            summary,
            [
                ("*", "Stars and shapes", 0),
                ("⅓", "Math", 1),
                ("$", "Currency", 2),
                ("€", "Currency", 3)
            ]
        );
        assert_eq!(
            rows[2].keywords,
            "Currency 货币符号 currency Currency huobifuhao hbfh"
        );
        let mut connection = Connection::open_in_memory().unwrap();
        build_symbols(&mut connection, &rows).unwrap();

        let unmapped = parse_piliapp("# [piliapp/symbol/new-slug/] 新\nx\n");
        assert!(symbol_rows(&pinyin(), &unmapped).is_err());
    }

    /// 符号表里 `#` 开头的符号（项目清单的 `#️⃣`）照常收，`#` 和 `# 说明` 才是注释。
    #[test]
    fn piliapp_keeps_hash_led_symbols() {
        let categories = parse_piliapp(
            "# 特殊符号表\n#\n# [piliapp/symbol/bullet-point/] 项目清单\n#\u{fe0f}\u{20e3}\n# 说明\n#\n*\u{fe0f}\u{20e3}\n",
        );
        assert_eq!(
            categories["bullet-point"].1,
            ["#\u{fe0f}\u{20e3}", "*\u{fe0f}\u{20e3}"]
        );
    }
}
