//! `msime-japanese.dat`: the immutable Viterbi model (`MSJPDT1`) the Japanese sentence decoder memory-maps, packed from Mozc's OSS dictionary at a pinned revision.
//!
//! Layout, little-endian: a 56-byte header (`MSJPDT1\0`, version, token count, connection size, reserved, token/connection/string offsets, string bytes), 20-byte token records (reading offset u32, reading length u16, surface offset u32, surface length u16, left id u16, right id u16, cost i32), the `size * size` connection matrix as i16, then the interned UTF-8 strings.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::text;

pub const MAGIC: &[u8; 8] = b"MSJPDT1\0";
pub const DICTIONARY_FILES: [&str; 10] = [
    "ja/mozc/dictionary00.txt",
    "ja/mozc/dictionary01.txt",
    "ja/mozc/dictionary02.txt",
    "ja/mozc/dictionary03.txt",
    "ja/mozc/dictionary04.txt",
    "ja/mozc/dictionary05.txt",
    "ja/mozc/dictionary06.txt",
    "ja/mozc/dictionary07.txt",
    "ja/mozc/dictionary08.txt",
    "ja/mozc/dictionary09.txt",
];
pub const ID_DEF: &str = "ja/mozc/id.def";
pub const CONNECTION: &str = "ja/mozc/connection_single_column.txt";
/// Mozc 的 README 包含模型所依据的 IPAdic、ICOT 与冲绳词典说明，因此随模型一同发布。
pub const NOTICE: &str = "ja/mozc/README.txt";
pub const NOTICE_NAME: &str = "mozc_dictionary_oss_README.txt";

const HEADER_SIZE: u64 = 56;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Token {
    pub reading: String,
    pub surface: String,
    pub left: u16,
    pub right: u16,
    pub cost: i32,
}

/// `reading<TAB>left<TAB>right<TAB>cost<TAB>surface[...]` lines, deduplicated and sorted by reading, surface, left, right, cost.
pub fn read_tokens(sources: &[(&str, String)]) -> Result<Vec<Token>> {
    let mut seen = HashSet::new();
    for (name, source) in sources {
        for (number, line) in text::universal_lines(source).into_iter().enumerate() {
            if line.is_empty() || line.starts_with('#') {
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
    Ok(tokens)
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

pub fn write_model(output: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = output.with_extension("dat.tmp");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let tokens = read_tokens(
            &sources
                .iter()
                .map(|(name, text)| (*name, text.clone()))
                .collect::<Vec<_>>(),
        )
        .unwrap();
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

    #[test]
    fn a_connection_file_that_is_not_square_fails() {
        assert!(read_connection("0 a\n", "1\n2\n3\n").is_err());
        assert_eq!(read_connection("0 a\n", "1\n2\n3\n4\n").unwrap().0, 2);
    }
}
