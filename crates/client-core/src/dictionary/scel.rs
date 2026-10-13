//! 搜狗细胞词库（`.scel`）：把用户自己选的文件转成标准 TSV 文本（每个词一行，`词<TAB>音节'音节`），交给现有的文本导入。
//!
//! 上限、去重、权重、分批和导入报告都沿用文本导入，这里只负责读二进制格式。第 k 行就是文件里的第 k 个词，所以下游报告里的「第 N 行」就是「第 N 个词」。读不出读音的词输出成 `词<TAB>`，读不出词的输出成 `<TAB>读音`：这两种行在所有下游路径（桌面分批、iOS 预览、命名词库）上都必定失败，并以同一套计数和行号报出来，不用每个宿主各自把解析器的跳过数并进导入报告。
//!
//! 布局（小端，绝对偏移）按搜狗官方分类词库核对过：`0x000` 是 12 字节魔数，`0x120` 是读音组数，`0x124` 是词数，`0x130` 和 `0x338` 是 UTF-16LE 的词库名和类别，`0x1540` 是本文件自己的拼音表，`0x2628` 起是词条区。每组先是本组词数、读音字节长度和音节序号，再是本组每个词的字节长度、UTF-16LE 词、扩展长度和扩展字节。扩展字段的前 4 字节是词在库里的序号，不是词频，所以不读它。决策与证据见 `.agents/notes/implemented/feature/2026-10-13-scel-import.md`。
//!
//! 文件是不可信输入：全部用 `get` 和 checked 运算读取，任何越界或不一致都整份拒绝，不做部分导入，也不信任文件里声明的数量去预分配。
#![deny(clippy::indexing_slicing)]

use thiserror::Error;

/// 一个细胞词库文件的字节上限。由命名词库请求的 17 MiB 上限反推：8 MiB 经 base64 是约 10.7 MiB，加上宿主选项仍在上限之内。界面和各宿主的同名常量都指向这里。
pub const MAX_SCEL_BYTES: usize = 8 * 1024 * 1024;
/// 转出文本的字节上限，与命名词库一次导入的文本上限相同：超过它的文本没有任何下游会收，不必先把它攒出来。
///
/// 文件大小管不住输出：同一组的每个词都把整条读音再写一遍，一个最多 64 个音节（约 450 字节）的读音配上只占 6 字节的单字词，输出能到文件的 75 倍，不到 8 MiB 的文件就能转出约 620 MB。所以边读边计数，一过上限就整份拒绝。真实的官方词库转出约为文件大小的 0.74 到 1.04 倍，8 MiB 的上限文件也离这里有一倍的余量。
pub const MAX_SCEL_TEXT_BYTES: usize = super::collections::MAX_IMPORT_TEXT_BYTES;

/// 前 4 字节：不是它就不是细胞词库。
const MAGIC: [u8; 4] = [0x40, 0x15, 0x00, 0x00];
/// 接下来 8 字节：已核实的版本。其他版本（例如社区资料提到的第 5 字节为 `E` 的变体）没有样本，按不支持拒绝，不去猜结构。
const VERSION: [u8; 8] = *b"DCS\x01\x01\x00\x00\x00";
const GROUP_COUNT_OFFSET: usize = 0x120;
const ENTRY_COUNT_OFFSET: usize = 0x124;
const NAME_OFFSET: usize = 0x130;
const CATEGORY_OFFSET: usize = 0x338;
const LABEL_BYTES: usize = 520;
const PINYIN_TABLE_OFFSET: usize = 0x1540;
const ENTRIES_OFFSET: usize = 0x2628;
/// 拼音表条数的上限；搜狗的表是 413 条。
const MAX_PINYIN_TABLE: u32 = 512;
/// 一个音节最多 6 个字母（`chuang`），即 12 字节 UTF-16。
const MAX_SYLLABLE_BYTES: usize = 12;
/// 一个词的读音最多 64 个音节，与 Engine 个人词条的音节上限一致。
const MAX_READING_BYTES: usize = 128;
const MAX_WORD_BYTES: usize = 2048;
/// 样本里扩展长度都是 10；留出余量，但拒绝离谱的值，以便发现未知变体。
const MAX_EXTENSION_BYTES: usize = 256;
/// 一组最少的字节数：4 字节组头、2 字节读音、2 字节词长、2 字节词、2 字节扩展长度。
const MIN_GROUP_BYTES: usize = 12;
/// 词库名和类别只用于界面提示，截到这么多字符。
const LABEL_CHARS: usize = 32;

/// 转出的文本和词库的说明信息。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScelText {
    /// 词库名，去掉控制字符并截到 32 个字符，只用于提示，不进词库。
    pub name: String,
    /// 类别，规则同 `name`。
    pub category: String,
    /// 读到的词数，也就是 `text` 的行数。
    pub entries: usize,
    /// 标准 TSV：每个词一行，不带权重列，下游用默认权重。
    pub text: String,
}

/// 整份文件为什么不能读。只有形状，不带任何词条内容。
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ScelError {
    /// 前 4 字节不是细胞词库的魔数。
    #[error("file is not a Sogou cell dictionary")]
    NotScel,
    /// 是细胞词库，但不是已核实的版本。
    #[error("Sogou cell dictionary version is not supported")]
    Unsupported,
    /// 文件超过 [`MAX_SCEL_BYTES`]，或转出的文本超过 [`MAX_SCEL_TEXT_BYTES`]。
    #[error("Sogou cell dictionary is too large")]
    TooLarge,
    /// 结构越界或前后不一致，包括下载不完整的文件。
    #[error("Sogou cell dictionary is truncated or malformed")]
    Malformed,
    /// 声明的读音组数为 0。
    #[error("Sogou cell dictionary has no entries")]
    Empty,
}

/// 读一个细胞词库，转成标准 TSV 文本。
pub fn decode(bytes: &[u8]) -> Result<ScelText, ScelError> {
    if bytes.len() > MAX_SCEL_BYTES {
        return Err(ScelError::TooLarge);
    }
    if bytes.get(..MAGIC.len()) != Some(&MAGIC[..]) {
        return Err(ScelError::NotScel);
    }
    match bytes.get(MAGIC.len()..MAGIC.len() + VERSION.len()) {
        None => return Err(ScelError::Malformed),
        Some(version) if version != VERSION => return Err(ScelError::Unsupported),
        Some(_) => {}
    }
    if bytes.len() < ENTRIES_OFFSET + 4 {
        return Err(ScelError::Malformed);
    }
    let syllables = pinyin_table(bytes)?;

    let groups = u32_at(bytes, GROUP_COUNT_OFFSET)? as usize;
    if groups == 0 {
        return Err(ScelError::Empty);
    }
    let remaining = bytes.len() - ENTRIES_OFFSET;
    if groups > remaining / MIN_GROUP_BYTES {
        return Err(ScelError::Malformed);
    }
    // 声明的词数只用来预留容量，而且不超过剩余字节能装下的词数，预留的总量也不超过文本上限：一个伪造的大数不会换来一次巨大的分配。
    let declared = u32_at(bytes, ENTRY_COUNT_OFFSET)? as usize;
    let rows = declared.min(remaining / 8);
    let mut text = String::with_capacity(rows.saturating_mul(24).min(MAX_SCEL_TEXT_BYTES));
    let mut entries = 0usize;
    let mut reading = String::with_capacity(MAX_READING_BYTES * 4);

    let mut reader = Reader::new(bytes, ENTRIES_OFFSET);
    for _ in 0..groups {
        let words = reader.u16()?;
        if words == 0 {
            return Err(ScelError::Malformed);
        }
        let reading_bytes = usize::from(reader.u16()?);
        if !(2..=MAX_READING_BYTES).contains(&reading_bytes) || reading_bytes % 2 != 0 {
            return Err(ScelError::Malformed);
        }
        let indices = reader.take(reading_bytes)?;
        let readable = read_reading(indices, &syllables, &mut reading);
        for _ in 0..words {
            let word_bytes = usize::from(reader.u16()?);
            if !(2..=MAX_WORD_BYTES).contains(&word_bytes) || word_bytes % 2 != 0 {
                return Err(ScelError::Malformed);
            }
            let word = reader.take(word_bytes)?;
            let extension = usize::from(reader.u16()?);
            if extension > MAX_EXTENSION_BYTES {
                return Err(ScelError::Malformed);
            }
            reader.take(extension)?;
            push_row(&mut text, word, readable.then_some(reading.as_str()));
            if text.len() > MAX_SCEL_TEXT_BYTES {
                return Err(ScelError::TooLarge);
            }
            entries += 1;
        }
    }
    // 声明的组数之后的字节（社区资料提到的被删词表等）不读。

    Ok(ScelText {
        name: label(bytes, NAME_OFFSET),
        category: label(bytes, CATEGORY_OFFSET),
        entries,
        text,
    })
}

/// 本文件自己的拼音表：不同文件的表顺序不同（同一批官方词库里就有两种），不能内置一张。表坏了整份文件的读音都不可信，所以任何一条不合规都整份拒绝。
fn pinyin_table(bytes: &[u8]) -> Result<Vec<String>, ScelError> {
    let count = u32_at(bytes, PINYIN_TABLE_OFFSET)?;
    if !(1..=MAX_PINYIN_TABLE).contains(&count) {
        return Err(ScelError::Malformed);
    }
    let count = count as usize;
    // 表必须在词条区之前结束，所以只在这一段里读。
    let table = bytes.get(..ENTRIES_OFFSET).ok_or(ScelError::Malformed)?;
    let mut reader = Reader::new(table, PINYIN_TABLE_OFFSET + 4);
    let mut syllables = vec![String::new(); count];
    for _ in 0..count {
        let index = usize::from(reader.u16()?);
        let length = usize::from(reader.u16()?);
        if !(2..=MAX_SYLLABLE_BYTES).contains(&length) || length % 2 != 0 {
            return Err(ScelError::Malformed);
        }
        let units = reader.take(length)?;
        let slot = syllables.get_mut(index).ok_or(ScelError::Malformed)?;
        if !slot.is_empty() {
            return Err(ScelError::Malformed);
        }
        for unit in units.chunks_exact(2) {
            let unit = u16_from(unit)?;
            let letter = u8::try_from(unit).map_err(|_| ScelError::Malformed)?;
            if !letter.is_ascii_lowercase() {
                return Err(ScelError::Malformed);
            }
            slot.push(char::from(letter));
        }
    }
    // 序号小于条数且互不重复，条数个序号恰好填满整张表。
    Ok(syllables)
}

/// 把一组的音节序号拼成 `音节'音节`，写进 `reading`；有序号不在本文件的拼音表里时返回 `false`。
fn read_reading(indices: &[u8], syllables: &[String], reading: &mut String) -> bool {
    reading.clear();
    for unit in indices.chunks_exact(2) {
        let Ok(index) = u16_from(unit) else {
            return false;
        };
        let Some(syllable) = syllables.get(usize::from(index)) else {
            return false;
        };
        if !reading.is_empty() {
            reading.push('\'');
        }
        reading.push_str(syllable);
    }
    true
}

/// 写一行。坏词条写成必定失败的一行而不是跳过，这样它在报告里有计数和行号，后面的行号也不会错位。
fn push_row(text: &mut String, word: &[u8], reading: Option<&str>) {
    let start = text.len();
    let word_ok = push_word(text, word);
    if !word_ok {
        // 词读不出：把可能写了一半的词去掉，留空。
        text.truncate(start);
    }
    text.push('\t');
    match (word_ok, reading) {
        (_, Some(reading)) => text.push_str(reading),
        // 词和读音都读不出：一个只有分隔符的行会被下游当成空行静默跳过，所以写一个撇号，让它按「词为空」失败。
        (false, None) => text.push('\''),
        // 读音读不出：编码列为空，下游按「编码为空」失败。
        (true, None) => {}
    }
    text.push('\n');
}

/// 把一个 UTF-16LE 词追加到 `text`，词能作为一行里的一列时返回 `true`。
///
/// 挡掉的东西都是为了守住「一行一个词」：孤立代理读不出；控制字符（含 TAB、CR、LF）会拆行或拆列，让后面的行号错位；U+FEFF 在开头会被当成 BOM 剥掉；首尾空白会被下游裁掉；`#` 开头的行会被下游当成注释静默跳过，不出现在失败计数里。
fn push_word(text: &mut String, word: &[u8]) -> bool {
    let start = text.len();
    for character in char::decode_utf16(utf16_units(word)) {
        let Ok(character) = character else {
            return false;
        };
        if character.is_control() || character == '\u{feff}' {
            return false;
        }
        text.push(character);
    }
    let Some(pushed) = text.get(start..) else {
        return false;
    };
    let (Some(first), Some(last)) = (pushed.chars().next(), pushed.chars().next_back()) else {
        return false;
    };
    !(first.is_whitespace() || last.is_whitespace() || first == '#')
}

/// 词库名或类别：UTF-16LE，到第一个 NUL 为止；读不出的单元换成替换字符，去掉控制字符和首尾空白，截到 32 个字符。
fn label(bytes: &[u8], offset: usize) -> String {
    let Some(region) = offset
        .checked_add(LABEL_BYTES)
        .and_then(|end| bytes.get(offset..end))
    else {
        return String::new();
    };
    let units = utf16_units(region).take_while(|unit| *unit != 0);
    let decoded: String = char::decode_utf16(units)
        .map(|character| character.unwrap_or(char::REPLACEMENT_CHARACTER))
        .filter(|character| !character.is_control())
        .collect();
    decoded
        .trim()
        .chars()
        .take(LABEL_CHARS)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// 一段 UTF-16LE 字节的代码单元；调用方给的长度都是偶数。
fn utf16_units(bytes: &[u8]) -> impl Iterator<Item = u16> + '_ {
    bytes.chunks_exact(2).filter_map(|unit| u16_from(unit).ok())
}

fn u16_from(unit: &[u8]) -> Result<u16, ScelError> {
    let pair: [u8; 2] = unit.try_into().map_err(|_| ScelError::Malformed)?;
    Ok(u16::from_le_bytes(pair))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, ScelError> {
    let end = offset.checked_add(4).ok_or(ScelError::Malformed)?;
    let quad: [u8; 4] = bytes
        .get(offset..end)
        .ok_or(ScelError::Malformed)?
        .try_into()
        .map_err(|_| ScelError::Malformed)?;
    Ok(u32::from_le_bytes(quad))
}

/// 顺序读取；任何一次越过末尾都是 `Malformed`。
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ScelError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ScelError::Malformed)?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or(ScelError::Malformed)?;
        self.position = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, ScelError> {
        u16_from(self.take(2)?)
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
pub(crate) mod tests;
