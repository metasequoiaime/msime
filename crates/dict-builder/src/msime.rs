//! `msime-pinyin.db`: the quanpin tables, the hand-maintained custom words merged into them, the 86 and 98 wubi tables and the quick phrase table.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use msime_engine::format::{
    build_table_name, quanpin_table, MAXIMUM_NUMBERED_SYLLABLES, SHIPPED_INITIALS,
};
use rusqlite::{params, Connection, OptionalExtension};

use crate::sqlite;
use crate::text;

// The SQL text is kept byte for byte as the Python pipeline wrote it: SQLite stores it in sqlite_master, and the schema is part of what a rebuild must reproduce.
const CREATE_QUANPIN_TABLE: &str = "\ncreate table if not exists {} (\n   \"key\" text, -- 全拼拼音\n   \"jp\" text, -- 全拼简拼\n   \"value\" text, -- 对应的汉字或者词组\n   \"weight\" integer default 0 -- 权重\n);\n";
const CREATE_WUBI_TABLE: &str = "\n            CREATE TABLE wubi86 (\n                \"key\" TEXT NOT NULL,\n                \"value\" TEXT NOT NULL,\n                \"weight\" INTEGER NOT NULL DEFAULT 0,\n                UNIQUE(\"key\", \"value\")\n            )\n            ";
const CREATE_WUBI98_TABLE: &str = "\n            CREATE TABLE wubi98 (\n                \"key\" TEXT NOT NULL,\n                \"value\" TEXT NOT NULL,\n                \"weight\" INTEGER NOT NULL DEFAULT 0,\n                UNIQUE(\"key\", \"value\")\n            )\n            ";
const CREATE_QUICK_PHRASE_TABLE: &str = "\n            CREATE TABLE quick_parases (\n                \"key\" TEXT NOT NULL,\n                \"value\" TEXT NOT NULL,\n                \"weight\" INTEGER NOT NULL DEFAULT 0,\n                UNIQUE(\"key\", \"value\")\n            )\n            ";

/// Characters added to the pinned single-character whitelist. The whitelist only applies to a build that includes unlicensed inputs (a licensed build accepts every single character), but where it applies it must not drop a reading the source has.
pub const WHITELIST_ADDITIONS: &str = "cn/SingleCharWhitelist.additions.txt";

/// Every quanpin table name, numbered buckets first, as `contracts/dictionary/format.json` lists them.
pub fn quanpin_tables() -> Vec<String> {
    (1..=MAXIMUM_NUMBERED_SYLLABLES + 1)
        .flat_map(|count| {
            SHIPPED_INITIALS
                .bytes()
                .filter_map(move |initial| quanpin_table(count, initial))
        })
        .collect()
}

pub fn pinyin_table(key: &str) -> Option<String> {
    let segments: Vec<String> = key.split('\'').map(str::to_owned).collect();
    if segments.iter().any(String::is_empty) {
        return None;
    }
    build_table_name(&segments)
}

fn jianpin(key: &str) -> Result<String> {
    key.split('\'')
        .map(|syllable| {
            syllable
                .chars()
                .next()
                .with_context(|| format!("{key:?} has an empty syllable"))
        })
        .collect()
}

pub struct QuanpinInputs<'a> {
    pub single_chars: &'a Path,
    /// `None` accepts every single character.
    pub whitelist: Option<HashSet<String>>,
    pub phrases: Vec<&'a Path>,
}

/// The whitelist file: one character per line, `#` lines skipped (checked before stripping, as the Python reader did).
pub fn parse_whitelist(text: &str) -> HashSet<String> {
    text::universal_lines(text)
        .into_iter()
        .filter(|line| !text::strip(line).is_empty() && !line.starts_with('#'))
        .map(|line| text::strip(line).to_owned())
        .collect()
}

pub fn build_quanpin(connection: &mut Connection, inputs: &QuanpinInputs) -> Result<usize> {
    let tables = quanpin_tables();
    let transaction = connection.transaction()?;
    for table in &tables {
        transaction.execute_batch(&format!("\ndrop table if exists {table};\n"))?;
        transaction.execute_batch(&CREATE_QUANPIN_TABLE.replace("{}", table))?;
    }
    let mut count =
        insert_quanpin_file(&transaction, inputs.single_chars, inputs.whitelist.as_ref())?;
    for path in &inputs.phrases {
        count += insert_quanpin_file(&transaction, path, None)?;
    }
    for table in &tables {
        let suffix = table.trim_start_matches("tbl_");
        transaction.execute_batch(&format!(
            "\ncreate index idx_key_{suffix} on {table}(key);\n"
        ))?;
        transaction.execute_batch(&format!("\ncreate index idx_jp_{suffix} on {table}(jp);\n"))?;
        // A reading listed by two inputs is one entry: without the licensing record both single-chars.txt and rime-ice.txt list every common character, and dict-v2.0.7 shipped 8740 single-character rows twice. The higher weight stays, the first loaded on a tie, as `parse_word_list` treats a word listed twice. Runs after the key index exists, which it uses.
        count -= transaction.execute(
            &format!("\ndelete from {table} where exists (select 1 from {table} as kept where kept.key = {table}.key and kept.value = {table}.value and (kept.weight > {table}.weight or (kept.weight = {table}.weight and kept.rowid < {table}.rowid)));\n"),
            [],
        )?;
    }
    transaction.commit()?;
    Ok(count)
}

/// `value<TAB>key<TAB>weight` lines. The file is split on `\n` only (the Python reader opened it in binary mode), so a CRLF file keeps its `\r` until the line is stripped.
fn insert_quanpin_file(
    connection: &Connection,
    path: &Path,
    accepted: Option<&HashSet<String>>,
) -> Result<usize> {
    let text = text::read(path)?;
    let mut count = 0;
    for (number, line) in text.split_terminator('\n').enumerate() {
        if line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = text::strip(line).split('\t').collect();
        let location = || format!("{}:{}", path.display(), number + 1);
        let value = fields[0];
        if accepted.is_some_and(|accepted| !accepted.contains(value)) {
            continue;
        }
        let key = fields
            .get(1)
            .with_context(|| format!("{}: expected value, pinyin and weight", location()))?;
        // Filters readings such as ê that do not start with a plain letter.
        if !key.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
            continue;
        }
        let weight = fields
            .get(2)
            .with_context(|| format!("{}: expected value, pinyin and weight", location()))?;
        let table = pinyin_table(key)
            .with_context(|| format!("{}: {key:?} maps to no quanpin table", location()))?;
        let jp = jianpin(key).with_context(location)?;
        // The weight is bound as text, as the Python builder did, so the column's integer affinity converts it exactly the same way.
        connection
            .prepare_cached(&format!("\ninsert into {table} (\n    key,\n    jp,\n    value,\n    weight\n) values (?, ?, ?, ?);\n"))?
            .execute(params![key, jp, value, weight])?;
        count += 1;
    }
    Ok(count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomWord {
    pub value: String,
    pub key: String,
    pub weight: i64,
}

/// `word<TAB>pinyin<TAB>weight`. A malformed line fails the stage: the file is short and hand-edited, so an error is cheaper than a silently dropped word. A word listed twice keeps its higher weight.
pub fn parse_custom_words(text: &str) -> Result<Vec<CustomWord>> {
    parse_word_list(text, "custom/words.txt")
}

/// A `word<TAB>pinyin<TAB>weight` list read like custom/words.txt (blank and `#` lines skipped, a malformed line an error), with errors located in `path`.
pub fn parse_word_list(text: &str, path: &str) -> Result<Vec<CustomWord>> {
    let mut entries: IndexMap<(String, String), CustomWord> = IndexMap::new();
    for (number, line) in text::splitlines(text).into_iter().enumerate() {
        let Some(entry) =
            parse_custom_word(line).with_context(|| format!("{path}:{}", number + 1))?
        else {
            continue;
        };
        match entries.get_mut(&(entry.key.clone(), entry.value.clone())) {
            Some(previous) if previous.weight >= entry.weight => {}
            Some(previous) => *previous = entry,
            None => {
                entries.insert((entry.key.clone(), entry.value.clone()), entry);
            }
        }
    }
    Ok(entries.into_values().collect())
}

/// One line of custom/words.txt: `None` for a blank or `#` comment line, an error naming the problem (without its location) for a malformed one. `check-words` validates contributed lines with this same function.
pub fn parse_custom_word(line: &str) -> Result<Option<CustomWord>> {
    let stripped = text::strip(line);
    if stripped.is_empty() || stripped.starts_with('#') {
        return Ok(None);
    }
    let fields: Vec<&str> = stripped.split('\t').collect();
    let [value, key, weight] = fields[..] else {
        bail!("expected word, pinyin and weight: {line:?}");
    };
    let (value, key, weight) = (text::strip(value), text::strip(key), text::strip(weight));
    if value.is_empty() {
        bail!("the word is empty");
    }
    if !key
        .split('\'')
        .all(|syllable| !syllable.is_empty() && syllable.bytes().all(|b| b.is_ascii_lowercase()))
    {
        bail!("{key:?} is not quanpin separated by \"'\"");
    }
    if pinyin_table(key).is_none() {
        bail!("{key:?} maps to no quanpin table");
    }
    let weight: i64 = weight
        .parse()
        .with_context(|| format!("weight {weight:?} is not an integer"))?;
    if weight < 1 {
        bail!("weight {weight} is below 1");
    }
    Ok(Some(CustomWord {
        value: value.to_owned(),
        key: key.to_owned(),
        weight,
    }))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CustomWordCounts {
    pub inserted: usize,
    pub promoted: usize,
    pub unchanged: usize,
}

/// Weights only go up: a word the general dictionary already ranks higher keeps its rank.
pub fn apply_custom_words(
    connection: &mut Connection,
    entries: &[CustomWord],
) -> Result<CustomWordCounts> {
    let mut counts = CustomWordCounts::default();
    let transaction = connection.transaction()?;
    for entry in entries {
        let table = pinyin_table(&entry.key).context("custom word without a table")?;
        let jp = jianpin(&entry.key)?;
        let existing: Option<i64> = transaction
            .query_row(
                &format!("select weight from {table} where key = ? and value = ?"),
                params![entry.key, entry.value],
                |row| row.get(0),
            )
            .optional()?;
        match existing {
            None => {
                transaction.execute(
                    &format!("insert into {table} (key, jp, value, weight) values (?, ?, ?, ?)"),
                    params![entry.key, jp, entry.value, entry.weight],
                )?;
                counts.inserted += 1;
            }
            Some(weight) if weight < entry.weight => {
                transaction.execute(
                    &format!("update {table} set weight = ?, jp = ? where key = ? and value = ?"),
                    params![entry.weight, jp, entry.key, entry.value],
                )?;
                counts.promoted += 1;
            }
            Some(_) => counts.unchanged += 1,
        }
    }
    transaction.commit()?;
    Ok(counts)
}

/// 所有拼音输入合并之后从全拼表删除的错误读音（`resources/dictionary-sources/`）。上游输入按字节原样保存（msime-dictionary 的 `upstream.lock.json`），所以错误的行不能在原文件里改。
pub const READING_CORRECTIONS: &str = "pinyin-reading-corrections.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingCorrection {
    pub value: String,
    pub wrong: String,
    pub correct: String,
}

/// `word<TAB>wrong pinyin<TAB>correct pinyin`, blank and `#` lines skipped. A malformed line, an entry whose two readings are equal or a word and wrong reading listed twice fails the stage.
pub fn parse_reading_corrections(text: &str) -> Result<Vec<ReadingCorrection>> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for (number, line) in text::splitlines(text).into_iter().enumerate() {
        let location = || format!("{READING_CORRECTIONS}:{}", number + 1);
        let stripped = text::strip(line);
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = stripped.split('\t').map(text::strip).collect();
        let [value, wrong, correct] = fields[..] else {
            bail!(
                "{}: expected word, wrong pinyin and correct pinyin: {line:?}",
                location()
            );
        };
        if value.is_empty() {
            bail!("{}: the word is empty", location());
        }
        for key in [wrong, correct] {
            if !is_quanpin(key) {
                bail!("{}: {key:?} is not quanpin separated by \"'\"", location());
            }
        }
        if wrong == correct {
            bail!(
                "{}: the wrong and correct readings are both {wrong:?}",
                location()
            );
        }
        if !seen.insert((value.to_owned(), wrong.to_owned())) {
            bail!("{}: {value} {wrong} is listed twice", location());
        }
        entries.push(ReadingCorrection {
            value: value.to_owned(),
            wrong: wrong.to_owned(),
            correct: correct.to_owned(),
        });
    }
    Ok(entries)
}

fn is_quanpin(key: &str) -> bool {
    key.split('\'')
        .all(|syllable| !syllable.is_empty() && syllable.bytes().all(|b| b.is_ascii_lowercase()))
        && pinyin_table(key).is_some()
}

/// Deletes every quanpin row whose word and pinyin match an entry's wrong reading and returns how many rows went. Fails, leaving the tables unchanged, when an entry deletes nothing (the input it corrected has changed, so the entry is stale) or when the word has no row at the correct reading afterwards (the correction would leave the word untypeable).
pub fn apply_reading_corrections(
    connection: &mut Connection,
    entries: &[ReadingCorrection],
) -> Result<usize> {
    let transaction = connection.transaction()?;
    let mut removed = 0;
    let mut stale = Vec::new();
    let mut missing = Vec::new();
    for entry in entries {
        let table = pinyin_table(&entry.wrong).context("reading correction without a table")?;
        let deleted = transaction.execute(
            &format!("delete from {table} where key = ? and value = ?"),
            params![entry.wrong, entry.value],
        )?;
        if deleted == 0 {
            stale.push(format!("{} {}", entry.value, entry.wrong));
        }
        removed += deleted;
    }
    for entry in entries {
        let table = pinyin_table(&entry.correct).context("reading correction without a table")?;
        let present: bool = transaction.query_row(
            &format!("select exists(select 1 from {table} where key = ? and value = ?)"),
            params![entry.correct, entry.value],
            |row| row.get(0),
        )?;
        if !present {
            missing.push(format!("{} {}", entry.value, entry.correct));
        }
    }
    if !stale.is_empty() || !missing.is_empty() {
        let mut problems = Vec::new();
        if !stale.is_empty() {
            problems.push(format!(
                "no row to remove for {} (drop the entry if its input no longer has the wrong reading)",
                stale.join(", ")
            ));
        }
        if !missing.is_empty() {
            problems.push(format!(
                "no row at the correct reading for {} (add it to msime-dictionary's custom/words.txt)",
                missing.join(", ")
            ));
        }
        bail!("{READING_CORRECTIONS}: {}", problems.join("; "));
    }
    transaction.commit()?;
    Ok(removed)
}

/// `value<TAB>code<TAB>weight` (wubi86) or `code<TAB>value<TAB>weight` (quick phrases). Invalid lines are skipped and counted, as the Python importers did.
struct CodeTable {
    name: &'static str,
    create: &'static str,
    indexes: &'static [&'static str],
    code_first: bool,
}

const WUBI86: CodeTable = CodeTable {
    name: "wubi86",
    create: CREATE_WUBI_TABLE,
    // 第二个索引给按词条反查五笔编码用（engine 的 `WubiProvider::reverse_code`），名字与 engine 在代次副本里补建的一致，补建时见到同名索引就跳过。
    indexes: &[
        "CREATE INDEX idx_wubi86_key_weight ON wubi86(\"key\", \"weight\" DESC)",
        "CREATE INDEX idx_wubi86_value ON wubi86(\"value\")",
    ],
    code_first: false,
};

/// Built by [`build_wubi98`] rather than from `value<TAB>code<TAB>weight` lines: the pinned 98 table has no weights.
const WUBI98: CodeTable = CodeTable {
    name: "wubi98",
    create: CREATE_WUBI98_TABLE,
    indexes: &[
        "CREATE INDEX idx_wubi98_key_weight ON wubi98(\"key\", \"weight\" DESC)",
        "CREATE INDEX idx_wubi98_value ON wubi98(\"value\")",
    ],
    code_first: false,
};

/// Weight step between neighbouring candidates of one 98 code, the step the 86 table uses for its lowest ranks.
const WUBI98_WEIGHT_STEP: i64 = 10;

const QUICK_PHRASES: CodeTable = CodeTable {
    name: "quick_parases",
    create: CREATE_QUICK_PHRASE_TABLE,
    indexes: &[
        "CREATE INDEX idx_quick_parases_key_weight ON quick_parases(\"key\", \"weight\" DESC)",
    ],
    code_first: true,
};

pub(crate) fn parse_code_line(line: &str, code_first: bool) -> Option<(String, &str, i64)> {
    let comment = if code_first {
        line.trim_start_matches(text::is_space)
    } else {
        line
    };
    if line.is_empty() || comment.starts_with('#') {
        return None;
    }
    let columns: Vec<&str> = line.split('\t').collect();
    let (key, value, weight) = if code_first {
        let [key, value, weight] = columns[..] else {
            return None;
        };
        (key, value, weight)
    } else {
        let [value, key, weight, ..] = columns[..] else {
            return None;
        };
        (key, value, weight)
    };
    let key = text::strip(key).to_lowercase();
    if value.is_empty() || key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let weight: i64 = text::strip(weight).parse().ok()?;
    (weight >= 0).then_some((key, value, weight))
}

fn build_code_table(
    connection: &mut Connection,
    table: &CodeTable,
    path: &Path,
) -> Result<(usize, usize)> {
    let source = text::read(path)?;
    let rows = text::universal_lines(text::without_bom(&source))
        .into_iter()
        .map(|line| parse_code_line(line, table.code_first));
    write_code_table(connection, table, rows)
}

/// Recreates `table` and inserts the parsed rows; `None` marks a skipped line. Returns the imported and skipped counts.
fn write_code_table<'a>(
    connection: &mut Connection,
    table: &CodeTable,
    rows: impl Iterator<Item = Option<(String, &'a str, i64)>>,
) -> Result<(usize, usize)> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(&format!("DROP TABLE IF EXISTS {}", table.name))?;
    transaction.execute_batch(table.create)?;
    for index in table.indexes {
        transaction.execute_batch(index)?;
    }
    let (mut imported, mut skipped) = (0, 0);
    {
        let mut insert = transaction.prepare(&format!(
            "\nINSERT INTO {0} (\"key\", \"value\", \"weight\")\nVALUES (?, ?, ?)\nON CONFLICT(\"key\", \"value\") DO UPDATE SET\n    \"weight\" = MAX(\"weight\", excluded.\"weight\")\n",
            table.name
        ))?;
        for row in rows {
            match row {
                Some((key, value, weight)) => {
                    insert.execute(params![key, value, weight])?;
                    imported += 1;
                }
                None => skipped += 1,
            }
        }
    }
    transaction.commit()?;
    Ok((imported, skipped))
}

/// Builds `wubi86` from the jidian table and then the generated supplement (`wubi86_supplement`), in that order: the provider breaks equal weights by rowid, so a supplement row of a code stays after the jidian rows of the same weight. Rows naming a character outside the basic CJK set ([`outside_basic_cjk`]) are left out. Returns the imported, skipped (blank, comment or invalid) and left-out counts.
pub fn build_wubi(connection: &mut Connection, paths: &[&Path]) -> Result<(usize, usize, usize)> {
    let sources = paths
        .iter()
        .map(|path| text::read(path))
        .collect::<Result<Vec<_>>>()?;
    let mut outside = 0;
    let rows = sources
        .iter()
        .flat_map(|source| {
            text::universal_lines(text::without_bom(source))
                .into_iter()
                .map(|line| parse_code_line(line, false))
        })
        .filter_map(|row| match row {
            Some((_, value, _)) if outside_basic_cjk(value) => {
                outside += 1;
                None
            }
            row => Some(row),
        });
    let (imported, skipped) = write_code_table(connection, &WUBI86, rows)?;
    Ok((imported, skipped, outside))
}

/// The jidian table ends with its large character set: about 49 000 rows, nearly all at weight 0, of CJK Extension A (U+3400-U+4DBF) and of the extensions beyond the Basic Multilingual Plane. They became candidates when dict-v2.0.6 switched 86 Wubi to this table: 25 000 codes, `dui` among them, then offered such a character first, the candidate window's fonts drew it as a missing-glyph box, and with mixed pinyin it pushed the pinyin rows of the same letters (对 for `dui`) off the first page. The table before had none of them, so they are left out again.
pub(crate) fn outside_basic_cjk(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(u32::from(character), 0x3400..=0x4DBF | 0x10000..))
}

/// Builds `wubi98` from the 98 wubi group's table as upstream ships it: UTF-16LE with a byte-order mark, `value<TAB>code` lines, no weights. Candidates of one code are listed best first, so each gets [`WUBI98_WEIGHT_STEP`] times the number of candidates after it plus one: the last of a code weighs one step, as the 86 table's lowest rank does.
/// 从主 UTF-16 表和完整的补充表构建 98 五笔。补充表作为独立来源保留，不写成手工特例；重复的“编码、词语”去重，主表保持原有权重和顺序。`generated` 是生成的补充表（`wubi98_supplement`，`value<TAB>code<TAB>weight` 行），按顺序插在两张 98 表之后：provider 同权重时按 rowid 排序，所以补充行排在同权重的原有行之后。
pub fn build_wubi98_sources(
    connection: &mut Connection,
    path: &Path,
    supplements: &[&Path],
    generated: &[&Path],
) -> Result<(usize, usize)> {
    let bytes = crate::sources::read_private(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let source = decode_utf16le(&bytes).with_context(|| format!("decoding {}", path.display()))?;
    let supplement_sources = supplements
        .iter()
        .map(|path| text::read(path))
        .collect::<Result<Vec<_>>>()?;
    let supplement_texts: Vec<&str> = supplement_sources.iter().map(String::as_str).collect();
    let (weighted, mut skipped) = wubi98_rows(&source, &supplement_texts);
    let generated_sources = generated
        .iter()
        .map(|path| text::read(path))
        .collect::<Result<Vec<_>>>()?;
    let rows = weighted
        .iter()
        .map(|(key, value, weight)| Some((key.clone(), value.as_str(), *weight)))
        .chain(generated_sources.iter().flat_map(|source| {
            text::universal_lines(text::without_bom(source))
                .into_iter()
                .map(|line| parse_code_line(line, false))
        }));
    let (imported, generated_skipped) = write_code_table(connection, &WUBI98, rows)?;
    skipped += generated_skipped;
    Ok((imported, skipped))
}

/// 两张 98 表按构建的规则得到的行 `(code, value, weight)`，以及跳过的行数：主表 `primary`（已从 UTF-16LE 解码）按编码内的先后给 [`WUBI98_WEIGHT_STEP`] 的倍数，补充表（Fcitx 格式）里主表没有的“编码、词语”一律给 1，重复的去掉。构建和 `wubi98_supplement` 都用它，生成器看到的权重就是构建写进表里的权重。
pub(crate) fn wubi98_rows(
    primary: &str,
    supplements: &[&str],
) -> (Vec<(String, String, i64)>, usize) {
    let lines = text::universal_lines(text::without_bom(primary));
    let mut parsed = Vec::new();
    let mut seen = HashSet::new();
    let mut skipped = 0;
    for line in lines {
        match parse_wubi98_line(line) {
            Some((key, value)) if seen.insert((key.to_owned(), value.to_owned())) => {
                parsed.push((key.to_owned(), value.to_owned()));
            }
            _ => skipped += 1,
        }
    }
    let mut remaining: HashMap<&str, i64> = HashMap::new();
    for (key, _) in &parsed {
        *remaining.entry(key.as_str()).or_default() += 1;
    }
    let mut weighted = Vec::with_capacity(parsed.len());
    for (key, value) in &parsed {
        let left = remaining
            .get_mut(key.as_str())
            .expect("every parsed code was counted");
        let weight = *left * WUBI98_WEIGHT_STEP;
        *left -= 1;
        weighted.push((key.clone(), value.clone(), weight));
    }
    for supplement in supplements {
        for line in text::universal_lines(supplement) {
            match parse_fcitx_wubi98_line(line) {
                Some((key, value)) if seen.insert((key.to_owned(), value.to_owned())) => {
                    weighted.push((key.to_owned(), value, 1));
                }
                _ => skipped += 1,
            }
        }
    }
    (weighted, skipped)
}

pub(crate) fn decode_utf16le(bytes: &[u8]) -> Result<String> {
    if !bytes.len().is_multiple_of(2) || !bytes.starts_with(&[0xff, 0xfe]) {
        bail!("not UTF-16LE with a byte-order mark");
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Ok(String::from_utf16(&units)?)
}

/// `value<TAB>code`, the code one to four of the letters a to y (z is the wildcard and the pinyin fallback, never a code).
pub(crate) fn parse_wubi98_line(line: &str) -> Option<(String, &str)> {
    let [value, key] = line.split('\t').collect::<Vec<_>>()[..] else {
        return None;
    };
    let valid_key = (1..=4).contains(&key.len()) && key.bytes().all(|b| (b'a'..=b'y').contains(&b));
    (!value.is_empty() && valid_key).then(|| (key.to_owned(), value))
}

/// 解析 Fcitx5 table-extra 的 UTF-8 98 五笔表中的“编码 空格 词语”行。表头和规则区忽略，只接受由一到四个小写字母组成的编码。
pub(crate) fn parse_fcitx_wubi98_line(line: &str) -> Option<(&str, String)> {
    let (key, value) = line.split_once(' ')?;
    let key = key.trim();
    let value = value.trim();
    let valid = (1..=4).contains(&key.len())
        && key.bytes().all(|byte| byte.is_ascii_lowercase())
        && !value.is_empty();
    valid.then_some((key, value.to_owned()))
}

/// Builds the quick phrase table, then checks it and refreshes the planner statistics of the whole database (the Python `04verify_db.py` step, which ran after quanpin and wubi).
pub fn build_quick_phrases(connection: &mut Connection, path: &Path) -> Result<(usize, usize)> {
    let counts = build_code_table(connection, &QUICK_PHRASES, path)?;
    sqlite::integrity_check(connection)?;
    let (rows, distinct): (i64, i64) = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM quick_parases), (SELECT COUNT(*) FROM (SELECT 1 FROM quick_parases GROUP BY \"key\", \"value\"))",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if rows == 0 || rows != distinct {
        bail!("quick_parases: unexpected row counts: rows={rows}, distinct_entries={distinct}");
    }
    sqlite::analyze(connection, true)?;
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(connection: &Connection, sql: &str) -> Vec<(String, String, String, i64)> {
        let mut statement = connection.prepare(sql).unwrap();
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn table_names_follow_the_format_contract() {
        let tables = quanpin_tables();
        assert_eq!(tables.len(), 8 * 23);
        assert_eq!(tables[0], "tbl_1_a");
        assert_eq!(tables.last().unwrap(), "tbl_others_z");
        assert_eq!(pinyin_table("ni'hao").as_deref(), Some("tbl_2_n"));
        assert_eq!(
            pinyin_table("a'b'c'd'e'f'g'h").as_deref(),
            Some("tbl_others_a")
        );
        assert_eq!(pinyin_table("ni''hao"), None);
    }

    const SINGLE_CHARS: &str =
        "#  字表\r\n昊\thao\t100\r\n昍\txuan\t0\r\n宣\txuan\t500\r\n欸\tê\t3\r\n";

    fn quanpin_fixture(whitelist: Option<HashSet<String>>) -> Connection {
        let dir = tempfile::tempdir().unwrap();
        let single = write(dir.path(), "single.txt", SINGLE_CHARS);
        let phrases = write(
            dir.path(),
            "phrases.txt",
            "# 词\r\n你好\tni'hao\t9000\r\n宣传\txuan'chuan\t12\r\n",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        build_quanpin(
            &mut connection,
            &QuanpinInputs {
                single_chars: &single,
                whitelist,
                phrases: vec![&phrases],
            },
        )
        .unwrap();
        connection
    }

    #[test]
    fn quanpin_rows_land_in_their_tables_with_jianpin() {
        let connection = quanpin_fixture(None);
        assert_eq!(
            rows(
                &connection,
                "select key, jp, value, weight from tbl_1_x order by weight"
            ),
            [
                ("xuan".into(), "x".into(), "昍".into(), 0),
                ("xuan".into(), "x".into(), "宣".into(), 500)
            ]
        );
        assert_eq!(
            rows(&connection, "select key, jp, value, weight from tbl_2_n"),
            [("ni'hao".into(), "nh".into(), "你好".into(), 9000)]
        );
        let weight_type: String = connection
            .query_row("select typeof(weight) from tbl_2_x", [], |row| row.get(0))
            .unwrap();
        assert_eq!(weight_type, "integer");
        let indexes: i64 = connection
            .query_row(
                "select count(*) from sqlite_master where type='index'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 2 * 8 * 23);
    }

    #[test]
    fn a_reading_listed_by_two_inputs_is_stored_once() {
        let dir = tempfile::tempdir().unwrap();
        let single = write(dir.path(), "single.txt", SINGLE_CHARS);
        // Like rime-ice.txt, the phrase input repeats single characters: 宣 lower, 昊 equal, 昍 higher.
        let phrases = write(
            dir.path(),
            "phrases.txt",
            "宣\txuan\t300\n昊\thao\t100\n昍\txuan\t7\n你好\tni'hao\t9000\n你好\tni'hao\t9000\n",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        let count = build_quanpin(
            &mut connection,
            &QuanpinInputs {
                single_chars: &single,
                whitelist: None,
                phrases: vec![&phrases],
            },
        )
        .unwrap();
        assert_eq!(
            rows(
                &connection,
                "select key, jp, value, weight from tbl_1_x order by weight"
            ),
            [
                ("xuan".into(), "x".into(), "昍".into(), 7),
                ("xuan".into(), "x".into(), "宣".into(), 500)
            ]
        );
        assert_eq!(
            rows(&connection, "select key, jp, value, weight from tbl_1_h"),
            [("hao".into(), "h".into(), "昊".into(), 100)]
        );
        assert_eq!(
            rows(&connection, "select key, jp, value, weight from tbl_2_n"),
            [("ni'hao".into(), "nh".into(), "你好".into(), 9000)]
        );
        assert_eq!(count, 4, "the row count reports what is stored");
    }

    #[test]
    fn the_whitelist_keeps_xuan_for_its_added_character() {
        // The pinned whitelist omits 昍; the repository's additions put it back.
        let mut whitelist = parse_whitelist("# comment\n昊\n宣\n");
        let additions = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources")
                .join(WHITELIST_ADDITIONS),
        )
        .unwrap();
        whitelist.extend(parse_whitelist(&additions));
        let connection = quanpin_fixture(Some(whitelist));
        let values: Vec<_> = rows(
            &connection,
            "select key, jp, value, weight from tbl_1_x order by weight",
        )
        .into_iter()
        .map(|(key, _, value, _)| (key, value))
        .collect();
        assert_eq!(
            values,
            [
                ("xuan".to_owned(), "昍".to_owned()),
                ("xuan".to_owned(), "宣".to_owned())
            ]
        );

        let without = quanpin_fixture(Some(parse_whitelist("昊\n宣\n")));
        let count: i64 = without
            .query_row(
                "select count(*) from tbl_1_x where value = '昍'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn custom_words_only_raise_weights() {
        let mut connection = quanpin_fixture(None);
        let words = parse_custom_words(
            "# c\n你好\tni'hao\t1\n新词\txin'ci\t5\n新词\txin'ci\t7\n宣传\txuan'chuan\t20\n",
        )
        .unwrap();
        assert_eq!(words.len(), 3);
        assert_eq!(words[1].weight, 7);
        let counts = apply_custom_words(&mut connection, &words).unwrap();
        assert_eq!(
            counts,
            CustomWordCounts {
                inserted: 1,
                promoted: 1,
                unchanged: 1
            }
        );
        assert_eq!(
            rows(
                &connection,
                "select key, jp, value, weight from tbl_2_x order by key"
            ),
            [
                ("xin'ci".into(), "xc".into(), "新词".into(), 7),
                ("xuan'chuan".into(), "xc".into(), "宣传".into(), 20)
            ]
        );
    }

    #[test]
    fn a_malformed_custom_word_fails_the_stage() {
        for bad in ["词\tci", "词\tCi\t1", "词\tci\t0", "\tci\t1", "词\tci\tx"] {
            assert!(parse_custom_words(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn reading_corrections_remove_every_row_of_the_wrong_reading() {
        let mut connection = quanpin_fixture(None);
        // The same word and wrong reading from two inputs gives two rows; both go.
        apply_custom_words(
            &mut connection,
            &parse_custom_words("重绘\tzhong'hui\t5\n重绘\tchong'hui\t5\n").unwrap(),
        )
        .unwrap();
        connection
            .execute(
                "insert into tbl_2_z (key, jp, value, weight) values ('zhong''hui', 'zh', '重绘', 9)",
                [],
            )
            .unwrap();
        let entries = parse_reading_corrections("# c\n\n重绘\tzhong'hui\tchong'hui\n").unwrap();
        assert_eq!(
            apply_reading_corrections(&mut connection, &entries).unwrap(),
            2
        );
        assert_eq!(
            rows(
                &connection,
                "select key, jp, value, weight from tbl_2_z union all select key, jp, value, weight from tbl_2_c"
            ),
            [("chong'hui".into(), "ch".into(), "重绘".into(), 5)]
        );
    }

    #[test]
    fn a_stale_or_unbacked_reading_correction_fails_and_changes_nothing() {
        let mut connection = quanpin_fixture(None);
        let count = |connection: &Connection| -> i64 {
            connection
                .query_row("select count(*) from tbl_2_n", [], |row| row.get(0))
                .unwrap()
        };
        // 你好 has no row at ni'hao'a: the entry is stale.
        let stale = parse_reading_corrections("你好\tni'hao'a\tni'hao\n").unwrap();
        let error = apply_reading_corrections(&mut connection, &stale).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no row to remove for 你好 ni'hao'a"),
            "{error}"
        );
        // Removing ni'hao would leave 你好 without the correct reading.
        let unbacked = parse_reading_corrections("你好\tni'hao\tnin'hao\n").unwrap();
        let error = apply_reading_corrections(&mut connection, &unbacked).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no row at the correct reading for 你好 nin'hao"),
            "{error}"
        );
        assert_eq!(count(&connection), 1);
    }

    #[test]
    fn a_malformed_reading_correction_fails_the_stage() {
        for bad in [
            "词\tci",
            "词\tci\tci",
            "词\tCi\tci'a",
            "\tci\tcha",
            "词\tci\tcha\t1",
            "词\tci\tcha\n词\tci\tchi",
        ] {
            assert!(parse_reading_corrections(bad).is_err(), "{bad:?}");
        }
        let error = parse_reading_corrections("# c\n\n词\tci\tcha\n词\tci").unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with(&format!("{READING_CORRECTIONS}:4:")),
            "{error}"
        );
    }

    #[test]
    fn the_repository_reading_corrections_parse() {
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources")
                .join(READING_CORRECTIONS),
        )
        .unwrap();
        assert!(!parse_reading_corrections(&text).unwrap().is_empty());
    }

    #[test]
    fn the_98_table_is_decoded_from_utf16_and_weighted_by_line_order() {
        let dir = tempfile::tempdir().unwrap();
        let text = "工\ta\r\n戈\ta\r\n五\tgg\r\n工\taaaa\r\n藏匿\taaaa\r\n恭恭敬敬\taaaa\r\n坏\tz\r\n坏\tAA\r\n\tgg\r\n长\tabcde\r\n只有一列\r\n";
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        let path = dir.path().join("wubi98.txt");
        std::fs::write(&path, bytes).unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(
            build_wubi98_sources(&mut connection, &path, &[], &[]).unwrap(),
            (6, 5)
        );
        let rows: Vec<(String, String, i64)> = connection
            .prepare("select key, value, weight from wubi98 order by key, weight desc")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            [
                ("a".into(), "工".into(), 20),
                ("a".into(), "戈".into(), 10),
                ("aaaa".into(), "工".into(), 30),
                ("aaaa".into(), "藏匿".into(), 20),
                ("aaaa".into(), "恭恭敬敬".into(), 10),
                ("gg".into(), "五".into(), 10),
            ]
        );
        let indexes: i64 = connection
            .query_row(
                "select count(*) from sqlite_master where name in ('idx_wubi98_key_weight', 'idx_wubi98_value')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 2);
    }

    #[test]
    fn the_98_table_must_be_utf16le_with_a_byte_order_mark() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "wubi98.txt", "工\ta\r\n");
        let mut connection = Connection::open_in_memory().unwrap();
        assert!(build_wubi98_sources(&mut connection, &path, &[], &[]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn the_98_table_rejects_a_fifo_without_blocking() {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        use std::sync::mpsc;
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wubi98.txt");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());

        let (done, result) = mpsc::channel();
        let worker_path = path.clone();
        let worker = std::thread::spawn(move || {
            let mut connection = Connection::open_in_memory().unwrap();
            done.send(build_wubi98_sources(&mut connection, &worker_path, &[], &[]).is_err())
                .unwrap();
        });
        let rejected = match result.recv_timeout(Duration::from_millis(100)) {
            Ok(rejected) => rejected,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut writer = std::fs::OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&path)
                    .unwrap();
                let mut bytes = vec![0xff, 0xfe];
                bytes.extend("工\ta\r\n".encode_utf16().flat_map(u16::to_le_bytes));
                writer.write_all(&bytes).unwrap();
                drop(writer);
                result.recv_timeout(Duration::from_secs(1)).unwrap()
            }
            Err(error) => panic!("98 table reader failed to report: {error}"),
        };
        worker.join().unwrap();
        assert!(rejected, "FIFO 98 table must be rejected without blocking");
    }

    #[test]
    fn the_98_table_merges_a_complete_fcitx_source() {
        let dir = tempfile::tempdir().unwrap();
        let text = "部门\tukuy\r\n";
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        let primary = dir.path().join("wubi98.txt");
        std::fs::write(&primary, bytes).unwrap();
        let supplement = write(
            dir.path(),
            "wubi98-fcitx.txt",
            "[Data]\nukuy 部门\nukuy 冲凉\n",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(
            build_wubi98_sources(&mut connection, &primary, &[&supplement], &[]).unwrap(),
            (2, 2)
        );
        let rows: Vec<(String, String, i64)> = connection
            .prepare("select key, value, weight from wubi98 order by weight desc")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            [
                ("ukuy".into(), "部门".into(), 10),
                ("ukuy".into(), "冲凉".into(), 1)
            ]
        );
    }

    #[test]
    fn wubi86_leaves_out_extension_characters() {
        let dir = tempfile::tempdir().unwrap();
        // 𡗜 (U+215DC) and 𥒜 (U+2549C) are the jidian rows that answered `dui` and `duiy`; 䔍 (U+450D) is Extension A. 磁浮 and 一 stay.
        let wubi = write(
            dir.path(),
            "wubi.txt",
            "𡗜\tdui\t0\n磁浮\tduie\t10\n𥒜\tduiy\t0\n䔍\tacu\t0\n一\tg\t100\n𡗜子\tdubb\t5\n",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(build_wubi(&mut connection, &[&wubi]).unwrap(), (2, 0, 4));
        let values: Vec<String> = connection
            .prepare("select value from wubi86 order by key")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(values, ["磁浮", "一"]);
    }

    #[test]
    fn code_tables_skip_invalid_lines_and_keep_the_higher_weight() {
        let dir = tempfile::tempdir().unwrap();
        let wubi = write(
            dir.path(),
            "wubi.txt",
            "\u{feff}工\ta\t20\r\n工\tA\t30\r\n戈\ta\t10\n# c\nx\t1a\t1\n戒\taa\n戒\taa\t-1\n",
        );
        let mut connection = Connection::open_in_memory().unwrap();
        assert_eq!(build_wubi(&mut connection, &[&wubi]).unwrap(), (3, 4, 0));
        let wubi_rows: Vec<(String, String, i64)> = connection
            .prepare("select key, value, weight from wubi86 order by weight")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            wubi_rows,
            [("a".into(), "戈".into(), 10), ("a".into(), "工".into(), 30)]
        );
        // 反查按词条找编码，发布的表要带上这个索引，否则每次都扫整张表。
        let plan: String = connection
            .query_row(
                "explain query plan select key from wubi86 where value = '工'",
                [],
                |row| row.get(3),
            )
            .unwrap();
        assert!(plan.contains("idx_wubi86_value"), "{plan}");

        let phrases = write(
            dir.path(),
            "quick.txt",
            "  # c\naddr\t示例\t10\nPhone\t138\t10\nbad key\tx\t1\nx\ty\n",
        );
        assert_eq!(
            build_quick_phrases(&mut connection, &phrases).unwrap(),
            (2, 3)
        );
        let keys: Vec<String> = connection
            .prepare("select key from quick_parases order by key")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(keys, ["addr", "phone"]);
        let stats: i64 = connection
            .query_row(
                "select count(*) from sqlite_stat1 where tbl = 'quick_parases'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stats > 0);
    }
}
