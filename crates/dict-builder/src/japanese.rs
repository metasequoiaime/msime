//! `msime-japanese.dat`: the immutable Viterbi model (`MSJPDT1`) the Japanese sentence decoder memory-maps, packed from Mozc's OSS dictionary at a pinned revision.
//!
//! Layout, little-endian: a 56-byte header (`MSJPDT1\0`, version, token count, connection size, reserved, token/connection/string offsets, string bytes), 20-byte token records (reading offset u32, reading length u16, surface offset u32, surface length u16, left id u16, right id u16, cost i32), the `size * size` connection matrix as i16, then the interned UTF-8 strings.

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use regex::Regex;

use crate::text;

pub const MAGIC: &[u8; 8] = b"MSJPDT1\0";
pub const DICTIONARY_FILES: [&str; 10] = [
    "sources/japanese/dictionary00.txt",
    "sources/japanese/dictionary01.txt",
    "sources/japanese/dictionary02.txt",
    "sources/japanese/dictionary03.txt",
    "sources/japanese/dictionary04.txt",
    "sources/japanese/dictionary05.txt",
    "sources/japanese/dictionary06.txt",
    "sources/japanese/dictionary07.txt",
    "sources/japanese/dictionary08.txt",
    "sources/japanese/dictionary09.txt",
];
pub const ID_DEF: &str = "sources/japanese/id.def";
/// Mozc's `src/data/dictionary_oss/aux_dictionary.tsv`: new words that copy the context ids and cost of a word already in the dictionary.
pub const AUX_DICTIONARY: &str = "sources/japanese/aux_dictionary.tsv";
/// Mozc's `src/data/dictionary_oss/dictionary_filter.tsv`: dictionary lines Mozc removes before building its system dictionary.
pub const DICTIONARY_FILTER: &str = "sources/japanese/dictionary_filter.tsv";
/// Mozc's `src/data/dictionary_manual/` word lists, in the order of that directory's `dictionary_manual` filegroup.
pub const MANUAL_WORDS: [&str; 2] = ["sources/japanese/places.tsv", "sources/japanese/words.tsv"];
pub const CONNECTION: &str = "sources/japanese/connection_single_column.txt";
/// Mozc 的 README 包含模型所依据的 IPAdic、ICOT 与冲绳词典说明，因此随模型一同发布。
pub const NOTICE: &str = "sources/japanese/README.txt";
pub const NOTICE_NAME: &str = "msime-mozc_dictionary_oss_README.txt";
/// Mozc's BSD-3-Clause `LICENSE`, which asks for its copyright notice, conditions and disclaimer in the documentation of every binary redistribution; the model is built from Mozc's data files, so the licence ships beside it too.
pub const LICENSE: &str = "sources/japanese/LICENSE";
pub const LICENSE_NAME: &str = "msime-mozc_LICENSE.txt";

const HEADER_SIZE: u64 = 56;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Token {
    pub reading: String,
    pub surface: String,
    pub left: u16,
    pub right: u16,
    pub cost: i32,
}

/// `reading<TAB>left<TAB>right<TAB>cost<TAB>surface[...]` lines that `filter` keeps, plus `extra`, deduplicated and sorted by reading, surface, left, right, cost. Also returns how many lines the filter removed.
pub fn read_tokens(
    sources: &[(&str, String)],
    filter: &DictionaryFilter,
    extra: Vec<Token>,
) -> Result<(Vec<Token>, usize)> {
    let mut seen: HashSet<Token> = extra.into_iter().collect();
    let mut filtered = 0;
    for (name, source) in sources {
        for (number, line) in text::universal_lines(source).into_iter().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if filter.removes(line) {
                filtered += 1;
                continue;
            }
            let columns: Vec<&str> = line.split('\t').collect();
            let [reading, left, right, cost, surface, ..] = columns[..] else {
                bail!(
                    "{name}:{}: expected at least five tab-separated columns",
                    number + 1
                );
            };
            let number = |value: &str| -> Result<i64> {
                text::strip(value)
                    .parse()
                    .with_context(|| format!("{name}:{}: {value:?} is not an integer", number + 1))
            };
            let context = |value: i64| u16::try_from(value).context("context id outside u16");
            seen.insert(Token {
                reading: reading.to_owned(),
                surface: surface.to_owned(),
                left: context(number(left)?)?,
                right: context(number(right)?)?,
                cost: i32::try_from(number(cost)?).context("cost outside i32")?,
            });
        }
    }
    let mut tokens: Vec<Token> = seen.into_iter().collect();
    tokens.sort_unstable();
    Ok((tokens, filtered))
}

/// The token list of Mozc's OSS system dictionary: `aux_tokens` computed from the unfiltered base (the Bazel `:aux_dictionary` rule reads `:base_dictionary_data`), then the base lines that `filter` keeps plus those aux tokens, which the filter never touches (only `:filtered_dictionary` applies `dictionary_filter.tsv`). Returns the tokens, the number of aux tokens and the number of filtered base lines.
pub fn system_tokens(
    dictionaries: &[(&str, String)],
    id_def: &str,
    aux_tsv: (&str, &str),
    word_lists: &[(&str, String)],
    filter: &DictionaryFilter,
) -> Result<(Vec<Token>, usize, usize)> {
    let aux = aux_tokens(dictionaries, id_def, aux_tsv, word_lists)?;
    let added = aux.len();
    let (tokens, filtered) = read_tokens(dictionaries, filter, aux)?;
    Ok((tokens, added, filtered))
}

/// `dictionary_filter.tsv` as `gen_filtered_dictionary.py` applies it to the base dictionary lines: each `key<TAB>value` row is a pair of regular expressions, and a line is removed when it fully matches `{key}\t\d+\t\d+\t\d+\t{value}(\t.*)?`. The pieces are concatenated without grouping, exactly as the script does.
pub struct DictionaryFilter(Vec<Regex>);

impl DictionaryFilter {
    pub fn parse(name: &str, source: &str) -> Result<Self> {
        let mut patterns = Vec::new();
        for (number, line) in text::universal_lines(source).into_iter().enumerate() {
            if line.starts_with('#') {
                continue;
            }
            let [key, value] = line.split('\t').collect::<Vec<_>>()[..] else {
                bail!("{name}:{}: expected key<TAB>value", number + 1);
            };
            let pattern = format!(r"^(?:{key}\t\d+\t\d+\t\d+\t{value}(\t.*)?)$");
            patterns.push(
                Regex::new(&pattern)
                    .with_context(|| format!("{name}:{}: bad pattern", number + 1))?,
            );
        }
        Ok(Self(patterns))
    }

    fn removes(&self, line: &str) -> bool {
        self.0.iter().any(|pattern| pattern.is_match(line))
    }
}

/// A dictionary line as `gen_aux_dictionary.py` reads it: `line.rstrip().split('\t')`, context ids kept as text.
struct AuxBase<'a> {
    key: &'a str,
    left: &'a str,
    right: &'a str,
    cost: i64,
    value: &'a str,
}

/// The tokens `gen_aux_dictionary.py --strict` writes to `aux_dictionary.txt`, which Mozc's OSS build adds to the filtered base dictionary.
///
/// `aux_dictionary.tsv` rows (`key, value, base_key, base_value, cost_offset`) copy the context ids of every base entry with `base_key`/`base_value`, at that entry's cost plus the offset, unless the new word already exists with those ids; a missing base entry is an error. Then each word list (`key, value, pos`) adds its words not already in the base or the aux rows, with the POS alias mapped through id.def and the cost set to the median cost of the base entries whose left and right ids are both that POS id. The base is the unfiltered dictionary, as in the Bazel rule; its zip-code part is not in the OSS sources, and zip-code entries use context id 0, which no alias names, so leaving it out changes no median.
pub fn aux_tokens(
    dictionaries: &[(&str, String)],
    id_def: &str,
    aux_tsv: (&str, &str),
    word_lists: &[(&str, String)],
) -> Result<Vec<Token>> {
    let mut bases: Vec<AuxBase<'_>> = Vec::new();
    for (name, source) in dictionaries {
        for (number, line) in text::universal_lines(source).into_iter().enumerate() {
            let columns: Vec<&str> = line.trim_end_matches(text::is_space).split('\t').collect();
            let [key, left, right, cost, value, ..] = columns[..] else {
                bail!(
                    "{name}:{}: expected at least five tab-separated columns",
                    number + 1
                );
            };
            let cost = text::strip(cost)
                .parse()
                .with_context(|| format!("{name}:{}: {cost:?} is not an integer", number + 1))?;
            bases.push(AuxBase {
                key,
                left,
                right,
                cost,
                value,
            });
        }
    }
    let mut by_word: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    let mut existing: HashSet<(&str, &str, &str, &str)> = HashSet::new();
    let mut costs_by_pos: HashMap<&str, Vec<i64>> = HashMap::new();
    for (index, base) in bases.iter().enumerate() {
        by_word
            .entry((base.key, base.value))
            .or_default()
            .push(index);
        existing.insert((base.left, base.right, base.key, base.value));
        if base.left == base.right {
            costs_by_pos.entry(base.left).or_default().push(base.cost);
        }
    }

    let token = |key: &str, left: &str, right: &str, cost: i64, value: &str| -> Result<Token> {
        let id = |value: &str| -> Result<u16> {
            value
                .parse()
                .with_context(|| format!("context id {value:?} outside u16"))
        };
        Ok(Token {
            reading: key.to_owned(),
            surface: value.to_owned(),
            left: id(left)?,
            right: id(right)?,
            cost: i32::try_from(cost).context("cost outside i32")?,
        })
    };

    let mut tokens = Vec::new();
    let mut added: HashSet<(String, String, String, String)> = HashSet::new();
    let (aux_name, aux_source) = aux_tsv;
    for (number, line) in text::universal_lines(aux_source).into_iter().enumerate() {
        if line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.trim_end_matches(text::is_space).split('\t').collect();
        let [key, value, base_key, base_value, offset] = columns[..] else {
            bail!(
                "{aux_name}:{}: expected five tab-separated columns",
                number + 1
            );
        };
        let offset: i64 = text::strip(offset)
            .parse()
            .with_context(|| format!("{aux_name}:{}: bad cost offset", number + 1))?;
        let Some(indices) = by_word.get(&(base_key, base_value)) else {
            bail!(
                "{aux_name}:{}: {base_key} and {base_value} are not in the dictionary",
                number + 1
            );
        };
        for &index in indices {
            let base = &bases[index];
            if existing.contains(&(base.left, base.right, key, value)) {
                continue;
            }
            tokens.push(token(
                key,
                base.left,
                base.right,
                base.cost + offset,
                value,
            )?);
            added.insert((
                base.left.to_owned(),
                base.right.to_owned(),
                key.to_owned(),
                value.to_owned(),
            ));
        }
    }

    let pos_ids = manual_pos_ids(id_def)?;
    let mut medians: HashMap<&str, i64> = HashMap::new();
    for (name, source) in word_lists {
        for (number, line) in text::universal_lines(source).into_iter().enumerate() {
            if line.starts_with('#') || line.trim_end_matches(text::is_space).is_empty() {
                continue;
            }
            let columns: Vec<&str> = line.trim_end_matches(text::is_space).split('\t').collect();
            let [key, value, pos] = columns[..] else {
                bail!("{name}:{}: expected key<TAB>value<TAB>pos", number + 1);
            };
            let Some(&id) = pos_ids.get(pos) else {
                bail!("{name}:{}: {pos} is an invalid pos", number + 1);
            };
            if existing.contains(&(id, id, key, value))
                || added.contains(&(
                    id.to_owned(),
                    id.to_owned(),
                    key.to_owned(),
                    value.to_owned(),
                ))
            {
                continue;
            }
            let cost = match medians.get(id) {
                Some(&cost) => cost,
                None => {
                    let mut costs = costs_by_pos
                        .get(id)
                        .with_context(|| format!("no dictionary entry has context ids {id}/{id}"))?
                        .clone();
                    costs.sort_unstable();
                    // `EntryList.AtRatio(0.5)`: index int((n - 1) * 0.5) of the costs in ascending order.
                    let cost = costs[(costs.len() - 1) / 2];
                    medians.insert(id, cost);
                    cost
                }
            };
            tokens.push(token(key, id, id, cost, value)?);
        }
    }
    Ok(tokens)
}

/// The POS aliases `gen_aux_dictionary.py` accepts in the word lists, mapped to their id.def ids.
fn manual_pos_ids(id_def: &str) -> Result<HashMap<&'static str, &str>> {
    const ALIASES: [(&str, &str); 18] = [
        ("名詞", "名詞,一般,*,*,*,*,*"),
        ("固有名詞", "名詞,固有名詞,一般,*,*,*,*"),
        ("人名", "名詞,固有名詞,人名,一般,*,*,*"),
        ("姓", "名詞,固有名詞,人名,姓,*,*,*"),
        ("名", "名詞,固有名詞,人名,名,*,*,*"),
        ("組織", "名詞,固有名詞,組織,*,*,*,*"),
        ("地名", "名詞,固有名詞,地域,一般,*,*,*"),
        ("名詞サ変", "名詞,サ変接続,*,*,*,*,*"),
        ("名詞形動", "名詞,形容動詞語幹,*,*,*,*,*"),
        ("副詞", "副詞,一般,*,*,*,*,*"),
        ("連体詞", "連体詞,*,*,*,*,*,*"),
        ("接続詞", "接続詞,*,*,*,*,*,*"),
        ("感動詞", "感動詞,*,*,*,*,*,*"),
        ("接頭語", "接頭詞,名詞接続,*,*,*,*,*"),
        ("助数詞", "名詞,接尾,助数詞,*,*,*,*"),
        ("接尾一般", "名詞,接尾,一般,*,*,*,*"),
        ("接尾人名", "名詞,接尾,人名,*,*,*,*"),
        ("接尾地名", "名詞,接尾,地域,*,*,*,*"),
    ];
    let mut ids = HashMap::new();
    for (number, line) in text::universal_lines(id_def).into_iter().enumerate() {
        let [id, name] = line
            .trim_end_matches(text::is_space)
            .split(' ')
            .collect::<Vec<_>>()[..]
        else {
            bail!("id.def:{}: expected <id> <name>", number + 1);
        };
        ids.insert(name, id);
    }
    ALIASES
        .iter()
        .map(|&(alias, name)| {
            let id = ids
                .get(name)
                .with_context(|| format!("id.def has no {name}"))?;
            Ok((alias, *id))
        })
        .collect()
}

/// The connection matrix, costs clamped to i16. The single-column file may start with the matrix size.
pub fn read_connection(id_def: &str, connection: &str) -> Result<(usize, Vec<i16>)> {
    let mut largest = None;
    for line in text::universal_lines(id_def) {
        if text::strip(line).is_empty() || line.starts_with('#') {
            continue;
        }
        let id: usize = text::split_whitespace(line)
            .next()
            .unwrap_or_default()
            .parse()
            .context("id.def: bad id")?;
        largest = largest.max(Some(id));
    }
    let mut size = largest.context("id.def lists no ids")? + 1;
    let mut costs = Vec::new();
    for line in text::universal_lines(connection) {
        let value = text::strip(line);
        if value.is_empty() || value.starts_with('#') {
            continue;
        }
        let cost: i64 = text::split_whitespace(value)
            .next()
            .unwrap_or_default()
            .parse()
            .context("connection: bad cost")?;
        costs.push(cost.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16);
    }
    if costs.len() == size * size + 1 && usize::try_from(costs[0]).ok() == Some(size) {
        costs.remove(0);
    }
    if costs.len() != size * size {
        let inferred = costs.len().isqrt();
        if inferred * inferred != costs.len() {
            bail!(
                "connection cost count {} is not a square matrix",
                costs.len()
            );
        }
        eprintln!("warning: id.def size {size}; using inferred connection size {inferred}");
        size = inferred;
    }
    Ok((size, costs))
}

fn intern<'a>(
    strings: &mut Vec<u8>,
    locations: &mut HashMap<&'a str, (u32, u16)>,
    value: &'a str,
) -> Result<(u32, u16)> {
    if let Some(location) = locations.get(value) {
        return Ok(*location);
    }
    let length = u16::try_from(value.len()).context("dictionary string exceeds u16 length")?;
    let location = (
        u32::try_from(strings.len()).context("string table exceeds u32")?,
        length,
    );
    strings.extend_from_slice(value.as_bytes());
    locations.insert(value, location);
    Ok(location)
}

pub fn pack(tokens: &[Token], size: usize, costs: &[i16]) -> Result<Vec<u8>> {
    let mut strings = Vec::new();
    let mut locations = HashMap::new();
    let mut records = Vec::with_capacity(tokens.len() * 20);
    for token in tokens {
        if usize::from(token.left) >= size || usize::from(token.right) >= size {
            bail!(
                "context id outside connection matrix: {}, {}",
                token.left,
                token.right
            );
        }
        let reading = intern(&mut strings, &mut locations, &token.reading)?;
        let surface = intern(&mut strings, &mut locations, &token.surface)?;
        records.extend_from_slice(&reading.0.to_le_bytes());
        records.extend_from_slice(&reading.1.to_le_bytes());
        records.extend_from_slice(&surface.0.to_le_bytes());
        records.extend_from_slice(&surface.1.to_le_bytes());
        records.extend_from_slice(&token.left.to_le_bytes());
        records.extend_from_slice(&token.right.to_le_bytes());
        records.extend_from_slice(&token.cost.to_le_bytes());
    }
    let token_offset = HEADER_SIZE;
    let connection_offset = token_offset + records.len() as u64;
    let string_offset = connection_offset + costs.len() as u64 * 2;
    let mut output = Vec::with_capacity(string_offset as usize + strings.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&1u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(tokens.len())?.to_le_bytes());
    output.extend_from_slice(&u32::try_from(size)?.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    for value in [
        token_offset,
        connection_offset,
        string_offset,
        strings.len() as u64,
    ] {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output.extend_from_slice(&records);
    for cost in costs {
        output.extend_from_slice(&cost.to_le_bytes());
    }
    output.extend_from_slice(&strings);
    Ok(output)
}

/// The inverse of [`pack`]: the tokens in file order, the connection matrix size and its costs. Fails on a file `pack` could not have written (bad magic or version, offsets past the end, strings that are not UTF-8 at a token's bounds).
pub fn unpack(bytes: &[u8]) -> Result<(Vec<Token>, usize, Vec<i16>)> {
    fn read<const N: usize>(bytes: &[u8], at: u64) -> Result<[u8; N]> {
        let at = usize::try_from(at)?;
        bytes
            .get(at..at + N)
            .and_then(|slice| slice.try_into().ok())
            .context("MSJPDT1 file is truncated")
    }
    if bytes.len() < HEADER_SIZE as usize || &bytes[..8] != MAGIC {
        bail!("not an MSJPDT1 file");
    }
    let version = u32::from_le_bytes(read(bytes, 8)?);
    if version != 1 {
        bail!("MSJPDT1 version {version} is not supported");
    }
    let token_count = u64::from(u32::from_le_bytes(read(bytes, 12)?));
    let size = u64::from(u32::from_le_bytes(read(bytes, 16)?));
    let token_offset = u64::from_le_bytes(read(bytes, 24)?);
    let connection_offset = u64::from_le_bytes(read(bytes, 32)?);
    let string_offset = u64::from_le_bytes(read(bytes, 40)?);
    let string_size = u64::from_le_bytes(read(bytes, 48)?);
    let token_bytes = token_count
        .checked_mul(20)
        .context("MSJPDT1 token table size overflows")?;
    let token_end = token_offset
        .checked_add(token_bytes)
        .context("MSJPDT1 token table range overflows")?;
    let connection_entries = size
        .checked_mul(size)
        .context("MSJPDT1 connection matrix size overflows")?;
    let connection_bytes = connection_entries
        .checked_mul(2)
        .context("MSJPDT1 connection matrix byte size overflows")?;
    let connection_end = connection_offset
        .checked_add(connection_bytes)
        .context("MSJPDT1 connection matrix range overflows")?;
    let string_end = string_offset
        .checked_add(string_size)
        .context("MSJPDT1 string table range overflows")?;
    let file_size = u64::try_from(bytes.len())?;
    if token_end > file_size || connection_end > file_size || string_end > file_size {
        bail!("MSJPDT1 section is out of bounds");
    }
    let strings = bytes
        .get(usize::try_from(string_offset)?..usize::try_from(string_end)?)
        .context("MSJPDT1 string table is out of bounds")?;
    let text = |offset: u32, length: u16| -> Result<String> {
        let start = usize::try_from(offset)?;
        let end = start
            .checked_add(usize::from(length))
            .context("MSJPDT1 token string range overflows")?;
        let slice = strings
            .get(start..end)
            .context("MSJPDT1 token string is out of bounds")?;
        Ok(std::str::from_utf8(slice)
            .context("MSJPDT1 token string is not UTF-8")?
            .to_owned())
    };
    let mut tokens = Vec::with_capacity(usize::try_from(token_count)?);
    for index in 0..token_count {
        let at = token_offset + index * 20;
        let reading_offset = u32::from_le_bytes(read(bytes, at)?);
        let reading_length = u16::from_le_bytes(read(bytes, at + 4)?);
        let surface_offset = u32::from_le_bytes(read(bytes, at + 6)?);
        let surface_length = u16::from_le_bytes(read(bytes, at + 10)?);
        tokens.push(Token {
            reading: text(reading_offset, reading_length)?,
            surface: text(surface_offset, surface_length)?,
            left: u16::from_le_bytes(read(bytes, at + 12)?),
            right: u16::from_le_bytes(read(bytes, at + 14)?),
            cost: i32::from_le_bytes(read(bytes, at + 16)?),
        });
    }
    let mut costs = Vec::with_capacity(usize::try_from(connection_entries)?);
    for index in 0..connection_entries {
        costs.push(i16::from_le_bytes(read(
            bytes,
            connection_offset + index * 2,
        )?));
    }
    Ok((tokens, usize::try_from(size)?, costs))
}

pub fn write_model(output: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = output.with_extension("dat.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn model_staging_symlink_is_not_truncated() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let output = directory.path().join("model.dat");
        let target = outside.path().join("outside.dat");
        std::fs::write(&target, b"keep").unwrap();
        symlink(&target, directory.path().join("model.dat.tmp")).unwrap();

        assert!(write_model(&output, b"replacement").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
    }

    #[test]
    fn unpack_reads_back_what_pack_wrote() {
        let tokens = vec![
            Token {
                reading: "かな".to_owned(),
                surface: "仮名".to_owned(),
                left: 1,
                right: 0,
                cost: 500,
            },
            Token {
                reading: "かな".to_owned(),
                surface: "かな".to_owned(),
                left: 0,
                right: 1,
                cost: 400,
            },
        ];
        let costs = [0, -3, 7, 12];
        let bytes = pack(&tokens, 2, &costs).unwrap();
        let (read, size, read_costs) = unpack(&bytes).unwrap();
        assert_eq!(read, tokens);
        assert_eq!(size, 2);
        assert_eq!(read_costs, costs);
        assert!(unpack(&bytes[..40]).is_err());
        assert!(unpack(b"not a model at all, but long enough for a header.......").is_err());
    }

    #[test]
    fn unpack_rejects_overflowing_string_range() {
        let mut bytes = vec![0; HEADER_SIZE as usize];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        bytes[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
        bytes[48..56].copy_from_slice(&1u64.to_le_bytes());

        assert!(unpack(&bytes).is_err());
    }

    #[test]
    fn tokens_are_deduplicated_sorted_and_packed() {
        let sources = [
            (
                "d0",
                "# c\nかな\t1\t1\t500\t仮名\nあ\t0\t1\t10\t亜\tjunk\n".to_owned(),
            ),
            (
                "d1",
                "あ\t0\t1\t10\t亜\r\nかな\t1\t0\t400\tかな\r\n".to_owned(),
            ),
        ];
        let (tokens, filtered) = read_tokens(
            &sources
                .iter()
                .map(|(name, text)| (*name, text.clone()))
                .collect::<Vec<_>>(),
            &DictionaryFilter::parse("empty", "").unwrap(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(filtered, 0);
        let summary: Vec<_> = tokens
            .iter()
            .map(|token| (token.reading.as_str(), token.surface.as_str(), token.cost))
            .collect();
        assert_eq!(
            summary,
            [
                ("あ", "亜", 10),
                ("かな", "かな", 400),
                ("かな", "仮名", 500)
            ]
        );

        let (size, costs) = read_connection("0 BOS\n1 名詞\n", "2\n5\n-40000\n7\n40000\n").unwrap();
        assert_eq!((size, costs.clone()), (2, vec![5, i16::MIN, 7, i16::MAX]));

        let bytes = pack(&tokens, size, &costs).unwrap();
        assert_eq!(&bytes[..8], MAGIC);
        let u32_at =
            |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let u64_at =
            |offset: usize| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        assert_eq!(
            (u32_at(8), u32_at(12), u32_at(16), u32_at(20)),
            (1, 3, 2, 0)
        );
        assert_eq!(
            (u64_at(24), u64_at(32), u64_at(40)),
            (56, 56 + 60, 56 + 60 + 8)
        );
        // あ, 亜, かな (interned once, shared by the second token's surface), 仮名.
        let strings = &bytes[(56 + 60 + 8) as usize..];
        assert_eq!(std::str::from_utf8(strings).unwrap(), "あ亜かな仮名");
        assert_eq!(u64_at(48), strings.len() as u64);
        let second = &bytes[56 + 20..56 + 40];
        assert_eq!(u32::from_le_bytes(second[0..4].try_into().unwrap()), 6);
        assert_eq!(u32::from_le_bytes(second[6..10].try_into().unwrap()), 6);

        let out_of_range = [Token {
            reading: "a".into(),
            surface: "b".into(),
            left: 2,
            right: 0,
            cost: 0,
        }];
        assert!(pack(&out_of_range, size, &costs).is_err());
    }

    // gen_filtered_dictionary.py: the key and value are regular expressions over the reading and surface columns, the line must match in full, and a sixth column is allowed.
    #[test]
    fn the_filter_removes_fully_matching_base_lines_only() {
        let filter = DictionaryFilter::parse(
            "dictionary_filter.tsv",
            "# key\tvalue\nみいだ[さ-そ].*\t見い出[さ-そ].*\nのろける?\t惚気ける?\nよね([づず]げ|ずけ)んし\t米津玄師\n",
        )
        .unwrap();
        let sources = [(
            "d",
            concat!(
                "みいだす\t837\t837\t5530\t見い出す\n",
                "みいだす\t837\t837\t5000\t見出す\n",
                "のろけ\t694\t694\t6805\t惚気け\n",
                "のろけた\t694\t694\t6805\t惚気けた\n",
                "よねずけんし\t1921\t1921\t4175\t米津玄師\tSPELLING_CORRECTION\n",
                "よねづけんし\t1921\t1921\t4175\t米津玄師\n",
                "よねづげんし\t1921\t1921\t4000\t米津玄師\n",
            )
            .to_owned(),
        )];
        let (tokens, filtered) = read_tokens(&sources, &filter, Vec::new()).unwrap();
        assert_eq!(filtered, 4);
        let kept: Vec<_> = tokens
            .iter()
            .map(|token| (token.reading.as_str(), token.surface.as_str()))
            .collect();
        assert_eq!(
            kept,
            [
                ("のろけた", "惚気けた"),
                ("みいだす", "見出す"),
                ("よねづけんし", "米津玄師")
            ]
        );
        assert!(DictionaryFilter::parse("f", "only-one-column\n").is_err());
        assert!(DictionaryFilter::parse("f", "(\tx\n").is_err());
    }

    // gen_aux_dictionary.py: aux rows copy every base entry's ids at cost + offset unless already present; word-list rows take the median cost of their POS and are skipped when the base or the aux rows already have them.
    #[test]
    fn aux_rows_and_word_lists_follow_gen_aux_dictionary() {
        let id_def = "0 BOS/EOS,*,*,*,*,*,*\n5 名詞,一般,*,*,*,*,*\n6 名詞,固有名詞,地域,一般,*,*,*\n7 動詞,自立,*,*,五段・カ行イ音便,基本形,*\n";
        let id_def = ALIAS_NAMES_FOR_TEST
            .iter()
            .enumerate()
            .fold(id_def.to_owned(), |text, (index, name)| {
                format!("{text}{} {name}\n", 100 + index)
            });
        let dictionaries = [(
            "d",
            concat!(
                "みにおぼえ\t5\t7\t7000\t見に覚え\n",
                "みにおぼえ\t5\t5\t6000\t見に覚え \n",
                "みにおぼえ\t5\t7\t6500\t身に覚え\n",
                "かな\t5\t5\t100\t仮名\n",
                "かな\t5\t5\t300\t仮名\n",
                "まち\t6\t6\t4000\t町\n",
            )
            .to_owned(),
        )];
        let aux = "# key\tvalue\tbase_key\tbase_value\tcost_offset\nみにおぼえ\t身に覚え\tみにおぼえ\t見に覚え\t-1\n";
        let words = [
            (
                "places.tsv",
                "# key\tvalue\tpos\nあきのくに\t安芸国\t地名\nまち\t町\t地名\n".to_owned(),
            ),
            (
                "words.tsv",
                "\nみにおぼえ\t身に覚え\t名詞\nかんじ\t漢字\t名詞\nあきのくに\t安芸国\t地名\n"
                    .to_owned(),
            ),
        ];
        let tokens =
            aux_tokens(&dictionaries, &id_def, ("aux_dictionary.tsv", aux), &words).unwrap();
        let summary: Vec<_> = tokens
            .iter()
            .map(|token| {
                (
                    token.reading.as_str(),
                    token.surface.as_str(),
                    token.left,
                    token.right,
                    token.cost,
                )
            })
            .collect();
        // 見に覚え has (5, 7) and, after rstrip, (5, 5); 身に覚え already exists as (5, 7), so only (5, 5) is added. 町 already exists. The words.tsv 身に覚え is now an aux row and is skipped; 漢字 takes the 名詞 median, the middle of 100, 300 and 6000; 安芸国 is added once per list, as the script does.
        assert_eq!(
            summary,
            [
                ("みにおぼえ", "身に覚え", 5, 5, 5999),
                ("あきのくに", "安芸国", 6, 6, 4000),
                ("かんじ", "漢字", 5, 5, 300),
                ("あきのくに", "安芸国", 6, 6, 4000),
            ]
        );

        let missing = "みにおぼえ\t身に覚え\tない\tない\t-1\n";
        assert!(aux_tokens(&dictionaries, &id_def, ("aux_dictionary.tsv", missing), &[]).is_err());
        let bad_pos = [("words.tsv", "かんじ\t漢字\t動詞\n".to_owned())];
        assert!(aux_tokens(&dictionaries, &id_def, ("aux_dictionary.tsv", ""), &bad_pos).is_err());
    }

    // The aux dictionary is built from the unfiltered base and is not filtered itself: an aux row whose base entry the filter removes still resolves, and a filter row matching an aux or word-list token leaves that token in place.
    #[test]
    fn aux_tokens_come_from_the_unfiltered_base_and_skip_the_filter() {
        let id_def = ALIAS_NAMES_FOR_TEST.iter().enumerate().fold(
            "0 BOS/EOS,*,*,*,*,*,*\n5 名詞,一般,*,*,*,*,*\n6 名詞,固有名詞,地域,一般,*,*,*\n"
                .to_owned(),
            |text, (index, name)| format!("{text}{} {name}\n", 100 + index),
        );
        let dictionaries = [(
            "d",
            "おみ\t5\t5\t5000\tお見\nみる\t5\t5\t3000\t見る\nまち\t6\t6\t4000\t町\n".to_owned(),
        )];
        let aux = "# key\tvalue\tbase_key\tbase_value\tcost_offset\nおみ\t御見\tおみ\tお見\t10\n";
        let words = [("places.tsv", "あきのくに\t安芸国\t地名\n".to_owned())];
        // Removes the base お見 (the aux row's base), and would remove 御見 and 安芸国 if it applied to them.
        let filter = DictionaryFilter::parse(
            "dictionary_filter.tsv",
            "おみ\tお見\nおみ\t御見\nあきのくに\t安芸国\n",
        )
        .unwrap();
        let (tokens, added, filtered) = system_tokens(
            &dictionaries,
            &id_def,
            ("aux_dictionary.tsv", aux),
            &words,
            &filter,
        )
        .unwrap();
        assert_eq!((added, filtered), (2, 1));
        let summary: Vec<_> = tokens
            .iter()
            .map(|token| (token.reading.as_str(), token.surface.as_str(), token.cost))
            .collect();
        assert_eq!(
            summary,
            [
                ("あきのくに", "安芸国", 4000),
                ("おみ", "御見", 5010),
                ("まち", "町", 4000),
                ("みる", "見る", 3000),
            ]
        );
    }

    /// The id.def names of the aliases the test id.def does not already give an id.
    const ALIAS_NAMES_FOR_TEST: [&str; 16] = [
        "名詞,固有名詞,一般,*,*,*,*",
        "名詞,固有名詞,人名,一般,*,*,*",
        "名詞,固有名詞,人名,姓,*,*,*",
        "名詞,固有名詞,人名,名,*,*,*",
        "名詞,固有名詞,組織,*,*,*,*",
        "名詞,サ変接続,*,*,*,*,*",
        "名詞,形容動詞語幹,*,*,*,*,*",
        "副詞,一般,*,*,*,*,*",
        "連体詞,*,*,*,*,*,*",
        "接続詞,*,*,*,*,*,*",
        "感動詞,*,*,*,*,*,*",
        "接頭詞,名詞接続,*,*,*,*,*",
        "名詞,接尾,助数詞,*,*,*,*",
        "名詞,接尾,一般,*,*,*,*",
        "名詞,接尾,人名,*,*,*,*",
        "名詞,接尾,地域,*,*,*,*",
    ];

    #[test]
    fn a_connection_file_that_is_not_square_fails() {
        assert!(read_connection("0 a\n", "1\n2\n3\n").is_err());
        assert_eq!(read_connection("0 a\n", "1\n2\n3\n4\n").unwrap().0, 2);
    }
}
