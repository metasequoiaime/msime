use super::*;
use crate::dictionary::import::{self, ImportIssue, ImportKind};

/// 测试用的细胞词库：按 3.1 节的布局现场拼出字节，内容都是自造的词，不提交任何真实词库。
pub(crate) struct ScelBuilder {
    pub magic: [u8; 12],
    pub name: String,
    pub category: String,
    /// 拼音表：（序号, 音节），按给出的顺序写进文件。
    pub table: Vec<(u16, String)>,
    pub groups: Vec<Group>,
    /// 覆盖头部声明的组数；为 `None` 时写实际组数。
    pub declared_groups: Option<u32>,
    pub declared_entries: Option<u32>,
    pub trailing: Vec<u8>,
}

pub(crate) struct Group {
    pub reading: Vec<u16>,
    /// 每个词：UTF-16 单元和扩展字节。
    pub words: Vec<(Vec<u16>, Vec<u8>)>,
}

pub(crate) const SYLLABLES: [&str; 6] = ["ce", "shi", "yang", "li", "ni", "hao"];

impl ScelBuilder {
    /// 两组共三个词：「测试」，以及同音的「实例」「事例」。
    pub fn sample() -> Self {
        Self::new(vec![
            group(&[0, 1], &["测试"]),
            group(&[1, 3], &["实例", "事例"]),
        ])
    }

    pub fn new(groups: Vec<Group>) -> Self {
        Self {
            magic: *b"\x40\x15\x00\x00DCS\x01\x01\x00\x00\x00",
            name: "合成词库".into(),
            category: "测试".into(),
            table: SYLLABLES
                .iter()
                .enumerate()
                .map(|(index, syllable)| (index as u16, (*syllable).to_owned()))
                .collect(),
            groups,
            declared_groups: None,
            declared_entries: None,
            trailing: Vec::new(),
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let mut table = Vec::new();
        for (index, syllable) in &self.table {
            table.extend_from_slice(&index.to_le_bytes());
            table.extend_from_slice(&((syllable.len() * 2) as u16).to_le_bytes());
            table.extend(utf16(syllable));
        }
        let table_end = PINYIN_TABLE_OFFSET + 4 + table.len();
        let mut bytes = vec![0u8; table_end.max(ENTRIES_OFFSET)];
        bytes[..12].copy_from_slice(&self.magic);
        let entries: usize = self.groups.iter().map(|group| group.words.len()).sum();
        put_u32(
            &mut bytes,
            GROUP_COUNT_OFFSET,
            self.declared_groups.unwrap_or(self.groups.len() as u32),
        );
        put_u32(
            &mut bytes,
            ENTRY_COUNT_OFFSET,
            self.declared_entries.unwrap_or(entries as u32),
        );
        let name = utf16(&self.name);
        bytes[NAME_OFFSET..NAME_OFFSET + name.len()].copy_from_slice(&name);
        let category = utf16(&self.category);
        bytes[CATEGORY_OFFSET..CATEGORY_OFFSET + category.len()].copy_from_slice(&category);
        put_u32(&mut bytes, PINYIN_TABLE_OFFSET, self.table.len() as u32);
        bytes[PINYIN_TABLE_OFFSET + 4..table_end].copy_from_slice(&table);
        for group in &self.groups {
            bytes.extend_from_slice(&(group.words.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&((group.reading.len() * 2) as u16).to_le_bytes());
            for index in &group.reading {
                bytes.extend_from_slice(&index.to_le_bytes());
            }
            for (word, extension) in &group.words {
                bytes.extend_from_slice(&((word.len() * 2) as u16).to_le_bytes());
                for unit in word {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes.extend_from_slice(&(extension.len() as u16).to_le_bytes());
                bytes.extend_from_slice(extension);
            }
        }
        bytes.extend_from_slice(&self.trailing);
        bytes
    }
}

pub(crate) fn group(reading: &[u16], words: &[&str]) -> Group {
    Group {
        reading: reading.to_vec(),
        words: words
            .iter()
            .enumerate()
            .map(|(index, word)| (word.encode_utf16().collect(), extension(index as u32 + 1)))
            .collect(),
    }
}

/// 搜狗文件里的扩展字段：4 字节序号加 6 字节 0。
fn extension(ordinal: u32) -> Vec<u8> {
    let mut bytes = ordinal.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[0; 6]);
    bytes
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// 第一组的读音字节长度、第一个词的字节长度和扩展长度在文件里的位置；第一组的读音有 `syllables` 个音节、第一个词有 `units` 个 UTF-16 单元。
fn first_group_offsets(syllables: usize, units: usize) -> (usize, usize, usize) {
    let reading_length = ENTRIES_OFFSET + 2;
    let word_length = ENTRIES_OFFSET + 4 + syllables * 2;
    let extension_length = word_length + 2 + units * 2;
    (reading_length, word_length, extension_length)
}

#[test]
fn scel_round_trips_words_readings_and_labels() {
    let decoded = decode(&ScelBuilder::sample().build()).unwrap();
    assert_eq!(decoded.text, "测试\tce'shi\n实例\tshi'li\n事例\tshi'li\n");
    assert_eq!(decoded.entries, 3);
    assert_eq!(decoded.name, "合成词库");
    assert_eq!(decoded.category, "测试");

    // 转出的文本按标准格式整份可读，行号就是词序，权重是省略时的默认值。
    let report = import::parse(ImportKind::Pinyin, "standard", &decoded.text, usize::MAX).unwrap();
    assert_eq!(report.failed, 0);
    assert!(!report.swapped);
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.line, entry.value.as_str(), entry.weight))
            .collect::<Vec<_>>(),
        [
            (1, "测试", 10_000),
            (2, "实例", 10_000),
            (3, "事例", 10_000)
        ]
    );
}

#[test]
fn scel_reads_each_files_own_pinyin_table() {
    let ordered = ScelBuilder::sample();
    // 同样的音节换一种顺序：序号跟着音节走，读出来的拼音不变。
    let mut shuffled = ScelBuilder::new(vec![
        group(&[5, 4], &["测试"]),
        group(&[4, 2], &["实例", "事例"]),
    ]);
    shuffled.table = [
        (5, "ce"),
        (4, "shi"),
        (3, "yang"),
        (2, "li"),
        (1, "ni"),
        (0, "hao"),
    ]
    .into_iter()
    .map(|(index, syllable)| (index, syllable.to_owned()))
    .collect();
    assert_eq!(
        decode(&ordered.build()).unwrap().text,
        decode(&shuffled.build()).unwrap().text
    );
}

#[test]
fn scel_turns_unreadable_entries_into_failing_rows_without_shifting_lines() {
    let mut builder = ScelBuilder::new(vec![
        group(&[0, 1], &["测试"]),
        // 音节序号不在拼音表里：读音读不出。
        group(&[0, 99], &["越界"]),
        group(
            &[2, 3],
            &[
                "样\n例",
                "样\t例",
                "\u{feff}样例",
                "#样例",
                " 样例",
                "样例 ",
                "样例",
            ],
        ),
        group(&[4, 5], &["你好"]),
    ]);
    // 孤立代理：词读不出。
    builder.groups[2]
        .words
        .push((vec![0xd800, 0x4f8b], extension(8)));
    // 词和读音都读不出。
    builder.groups.push(Group {
        reading: vec![200],
        words: vec![(vec![0xdc00], extension(1))],
    });
    let decoded = decode(&builder.build()).unwrap();
    assert_eq!(decoded.entries, 12);
    let lines: Vec<&str> = decoded.text.lines().collect();
    assert_eq!(
        lines,
        [
            "测试\tce'shi",
            "越界\t",
            "\tyang'li",
            "\tyang'li",
            "\tyang'li",
            "\tyang'li",
            "\tyang'li",
            "\tyang'li",
            "样例\tyang'li",
            "\tyang'li",
            "你好\tni'hao",
            "\t'",
        ]
    );
    for line in &lines {
        assert_eq!(line.split('\t').count(), 2, "{line:?}");
    }

    // 下游按同一套行号报出每个坏词条，后面的行没有错位。
    let report = import::parse(ImportKind::Pinyin, "standard", &decoded.text, usize::MAX).unwrap();
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| (entry.line, entry.value.as_str()))
            .collect::<Vec<_>>(),
        [(1, "测试"), (9, "样例"), (11, "你好")]
    );
    assert_eq!(report.failed, 9);
    assert_eq!(report.first_failures[0].line, 2);
    assert_eq!(report.first_failures[0].issue, ImportIssue::EmptyKey);
    assert!(report.first_failures[1..]
        .iter()
        .all(|failure| failure.issue == ImportIssue::EmptyValue));
}

#[test]
fn scel_refuses_every_truncation_without_panicking() {
    let bytes = ScelBuilder::sample().build();
    for length in 0..bytes.len() {
        let error = decode(&bytes[..length]).unwrap_err();
        assert!(
            matches!(error, ScelError::Malformed | ScelError::NotScel),
            "{length}: {error:?}"
        );
        assert_eq!(error == ScelError::NotScel, length < 4, "{length}");
    }
}

#[test]
fn scel_survives_random_corruption() {
    let original = ScelBuilder::sample().build();
    // 固定种子的 xorshift，结果可复现；不为此引入随机数依赖。
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for flips in 1..=8 {
        for _ in 0..2000 {
            let mut bytes = original.clone();
            for _ in 0..flips {
                // 一半落在头部和拼音表，一半落在词条区，两边都会被打到。
                let position = if next() % 2 == 0 {
                    (next() % ENTRIES_OFFSET as u64) as usize
                } else {
                    ENTRIES_OFFSET + (next() % (bytes.len() - ENTRIES_OFFSET) as u64) as usize
                };
                bytes[position] = next() as u8;
            }
            if let Ok(decoded) = decode(&bytes) {
                assert_eq!(decoded.text.lines().count(), decoded.entries);
                for line in decoded.text.lines() {
                    assert_eq!(line.split('\t').count(), 2, "{line:?}");
                }
            }
        }
    }
}

#[test]
fn scel_refuses_out_of_range_structure() {
    let refused = |builder: &ScelBuilder, patch: &dyn Fn(&mut Vec<u8>)| {
        let mut bytes = builder.build();
        patch(&mut bytes);
        decode(&bytes).unwrap_err()
    };
    let sample = ScelBuilder::sample();
    let (reading_length, word_length, extension_length) = first_group_offsets(2, 2);

    let mut empty = ScelBuilder::sample();
    empty.declared_groups = Some(0);
    assert_eq!(decode(&empty.build()).unwrap_err(), ScelError::Empty);
    let mut huge = ScelBuilder::sample();
    huge.declared_groups = Some(u32::MAX);
    assert_eq!(decode(&huge.build()).unwrap_err(), ScelError::Malformed);
    // 声明的词数只用于预留容量：再大也不会照着分配，也不影响读出的内容。
    let mut declared = ScelBuilder::sample();
    declared.declared_entries = Some(u32::MAX);
    assert_eq!(decode(&declared.build()).unwrap().entries, 3);

    for length in [0u16, 3, 130] {
        assert_eq!(
            refused(&sample, &|bytes| put_u16(bytes, reading_length, length)),
            ScelError::Malformed,
            "reading length {length}"
        );
    }
    for length in [0u16, 3, 2050] {
        assert_eq!(
            refused(&sample, &|bytes| put_u16(bytes, word_length, length)),
            ScelError::Malformed,
            "word length {length}"
        );
    }
    assert_eq!(
        refused(&sample, &|bytes| put_u16(bytes, extension_length, 257)),
        ScelError::Malformed
    );
    assert_eq!(
        refused(&sample, &|bytes| put_u16(bytes, ENTRIES_OFFSET, 0)),
        ScelError::Malformed,
        "a group without words"
    );

    for count in [0u32, 513] {
        assert_eq!(
            refused(&sample, &|bytes| put_u32(bytes, PINYIN_TABLE_OFFSET, count)),
            ScelError::Malformed,
            "table count {count}"
        );
    }
    for syllable in ["Ce", "c3", "", "chuangg"] {
        let mut builder = ScelBuilder::sample();
        builder.table[0].1 = syllable.to_owned();
        assert_eq!(
            decode(&builder.build()).unwrap_err(),
            ScelError::Malformed,
            "{syllable:?}"
        );
    }
    let mut repeated = ScelBuilder::sample();
    repeated.table[1].0 = 0;
    assert_eq!(decode(&repeated.build()).unwrap_err(), ScelError::Malformed);
    let mut outside = ScelBuilder::sample();
    outside.table[1].0 = 6;
    assert_eq!(decode(&outside.build()).unwrap_err(), ScelError::Malformed);
    // 表越过词条区的起点。
    let mut overrun = ScelBuilder::sample();
    overrun.table = (0..400).map(|index| (index, "zhuang".to_owned())).collect();
    assert_eq!(decode(&overrun.build()).unwrap_err(), ScelError::Malformed);

    let mut version = ScelBuilder::sample();
    version.magic[4] = b'E';
    assert_eq!(
        decode(&version.build()).unwrap_err(),
        ScelError::Unsupported
    );
    let mut foreign = ScelBuilder::sample();
    foreign.magic[0] = 0x41;
    assert_eq!(decode(&foreign.build()).unwrap_err(), ScelError::NotScel);
    assert_eq!(decode(b"").unwrap_err(), ScelError::NotScel);
    assert_eq!(
        decode(&vec![0; MAX_SCEL_BYTES + 1]).unwrap_err(),
        ScelError::TooLarge
    );
}

/// 文件不大、输出却很大的形状：一组 6 万个单字词共用一条 64 个音节的读音，每个词在文件里只占 6 字节，在文本里却要 300 多字节。
#[test]
fn scel_refuses_output_that_outgrows_the_text_limit() {
    let reading = vec![2u16; 64];
    let word: Vec<u16> = "样".encode_utf16().collect();
    let amplified = ScelBuilder::new(vec![Group {
        reading,
        words: vec![(word, Vec::new()); 60_000],
    }])
    .build();
    assert!(amplified.len() < MAX_SCEL_BYTES / 16);
    assert_eq!(decode(&amplified).unwrap_err(), ScelError::TooLarge);
}

#[test]
fn scel_ignores_bytes_after_the_declared_groups() {
    let mut builder = ScelBuilder::sample();
    builder.trailing = vec![0xff; 37];
    assert_eq!(
        decode(&builder.build()).unwrap().text,
        decode(&ScelBuilder::sample().build()).unwrap().text
    );
}

#[test]
fn scel_labels_drop_controls_and_stop_at_thirty_two_characters() {
    let mut builder = ScelBuilder::sample();
    builder.name = format!(" 化学\u{1}词汇{} ", "长".repeat(40));
    builder.category = String::new();
    let decoded = decode(&builder.build()).unwrap();
    assert_eq!(decoded.name.chars().count(), 32);
    assert!(decoded.name.starts_with("化学词汇长"));
    assert_eq!(decoded.category, "");
}

/// 用真实下载的细胞词库核对解析器：只在本机运行，样本不进仓库。`MSIME_SCEL_SAMPLES` 指向放 `.scel` 文件的目录。
#[test]
#[ignore = "reads user-supplied sample files named by MSIME_SCEL_SAMPLES"]
fn scel_reads_local_samples() {
    let Some(directory) = std::env::var_os("MSIME_SCEL_SAMPLES") else {
        eprintln!("MSIME_SCEL_SAMPLES is not set; nothing to check");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "scel")
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .scel files in MSIME_SCEL_SAMPLES");
    let mut total = 0;
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let decoded =
            decode(&bytes).unwrap_or_else(|error| panic!("{}: {error:?}", path.display()));
        let declared = u32_at(&bytes, ENTRY_COUNT_OFFSET).unwrap() as usize;
        assert_eq!(decoded.entries, declared, "{}", path.display());
        let failing = decoded
            .text
            .lines()
            .filter(|line| line.starts_with('\t') || line.ends_with('\t'))
            .count();
        assert_eq!(failing, 0, "{}", path.display());
        total += decoded.entries;
        eprintln!(
            "{}: {} entries, {:?} / {:?}",
            path.display(),
            decoded.entries,
            decoded.name,
            decoded.category
        );
    }
    eprintln!("{} files, {total} entries", files.len());
}
