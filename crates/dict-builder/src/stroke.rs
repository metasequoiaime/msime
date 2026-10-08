//! `msime-stroke.db`：笔画方案的笔顺码表，按 `msime_engine::language_dictionary` 定义的结构写出。笔顺码来自 rime-stroke（LGPL-3.0，见 `resources/licenses/rime-stroke-LGPL-3.0.txt`）在提交 `COMMIT` 的 `stroke.dict.yaml`，字频来自 msime-dictionary 的 `sources/pinyin/single-chars.txt`（rime-ice 字频，GPL-3.0；没有记录固定它的大小和 SHA-256，内容只由 `--dictionary` checkout 的提交决定）。
//!
//! `stroke.dict.yaml` 是 Rime 码表：YAML 头以 `...` 一行结束，之后每行 `字<TAB>笔顺码`，`#` 行是注释。码只用 h 横、s 竖、p 撇、n 点（捺）、z 折五个字母，与方案的按键一一对应，所以原样作为 `entries.key`，不加空格。一个字常有几个笔顺码（大陆规范与台湾 CNS11643 的笔顺并列收录，如「小」zpn 与 spn），每个码各成一条。上游没有权重列，Rime 用自己的八股文字频排序；这里改用 `single-chars.txt`：一个字在其中所有读音的权重之和就是它每个笔顺码的权重，表里没有的字权重为 0。
//!
//! 只收基本区（U+4E00–9FFF）和扩展 A 区（U+3400–4DBF）的汉字，其余区段只收在字频表里出现过的字。上游还收了扩展 B 区及以后的七万多个字、西夏文部件、部首与笔画符号，几乎都没有字频；macOS 自带字体不覆盖扩展 B 区及以后，它们在候选窗里是方块，而笔数恰好打满时它们作为精确匹配排在常用字的补全前面。
//!
//! `syllables` 表固定是五个笔画字母，引擎只拿它确认词典非空。上游有三个笔顺码超过引擎的 64 笔上限（最长 84 笔），它们照常写入，只能经前缀补全找到。
//!
//! `msime-stroke.db` 从 `--dictionary` checkout 读取 `sources/stroke/stroke.dict.yaml`，并按它的 `upstream.lock.json` 校验。`languages` 必须传 `--dictionary`，所以 `source` 里按常量校验 `--cache` 文件的兜底分支在命令行上已经走不到，只为不删代码而保留。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use msime_engine::language_dictionary;

use crate::sources::{sha256_file, Sources};
use crate::sqlite;
use crate::text;

pub const SOURCE: &str = "sources/stroke/stroke.dict.yaml";
/// 字频来源，quanpin 阶段也读同一份文件（来自 `--dictionary` checkout，没有记录固定它的摘要）。
pub const FREQUENCIES: &str = "sources/pinyin/single-chars.txt";
/// 锁文件里的上游引用名，它的提交就是 `source_commit`。
pub const REFERENCE: &str = "rime-stroke";
pub const REPOSITORY: &str = "https://github.com/rime/rime-stroke";
/// 许可证覆盖的上游提交，锁文件没有 `rime-stroke` 引用时作为 `source_commit`。
pub const COMMIT: &str = "1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48";
/// `COMMIT` 处原样文件的大小与 SHA-256，msime-dictionary 的 `upstream.lock.json` 记录同样的值。`source` 的缓存兜底分支用它们校验手动放进缓存的文件，但这个分支在命令行上已经走不到。
pub const SOURCE_SIZE: u64 = 3_396_330;
pub const SOURCE_SHA256: &str = "b3e93dce89c185f45c3d6e189b86b3a8626913352cc85e1094c786579a665791";
/// 五种笔画，也是 `syllables` 表的全部内容。
pub const STROKES: [char; 5] = ['h', 's', 'p', 'n', 'z'];
/// 记入数据库 `license` 的 SPDX 表达式：笔顺码是 rime-stroke 的 LGPL-3.0，权重取自 rime-ice 的 GPL-3.0 字频。
pub const LICENSE: &str = "LGPL-3.0-only AND GPL-3.0-only";
pub const DATABASE: &str = "msime-stroke.db";
/// `resources/licenses/` 里的许可证文本，以及它在数据库旁边的文件名。
pub const LICENSE_SOURCE: &str = "rime-stroke-LGPL-3.0.txt";
pub const LICENSE_NAME: &str = "msime-rime_stroke_LICENSE.txt";

/// 发布构建时 `verify` 要求的下限。固定提交经 `kept` 过滤后的实际值是 47095 条、27588 个字、7678 个有字频的字。
pub const FLOORS: Floors = Floors {
    entries: 45_000,
    characters: 27_000,
    weighted: 7_000,
};

/// 这些码的前 `within` 条里必须有对应的字，用来发现排序上的大错。
pub const EXPECTED: [Expected; 8] = [
    Expected {
        key: "h",
        texts: &["一"],
        within: 1,
    },
    Expected {
        key: "hh",
        texts: &["二"],
        within: 1,
    },
    Expected {
        key: "hhh",
        texts: &["三"],
        within: 1,
    },
    Expected {
        key: "hs",
        texts: &["十"],
        within: 1,
    },
    Expected {
        key: "szh",
        texts: &["口"],
        within: 1,
    },
    Expected {
        key: "pn",
        texts: &["人"],
        within: 1,
    },
    // 「小」的大陆笔顺 spn 与台湾笔顺 zpn 都要排在首位。
    Expected {
        key: "spn",
        texts: &["小"],
        within: 1,
    },
    Expected {
        key: "zpn",
        texts: &["小"],
        within: 1,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct Expected {
    pub key: &'static str,
    pub texts: &'static [&'static str],
    pub within: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Floors {
    pub entries: usize,
    pub characters: usize,
    pub weighted: usize,
}

/// `stroke.dict.yaml` 的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<'a> {
    pub text: &'a str,
    /// 只含 `STROKES` 字母的笔顺码。
    pub code: &'a str,
}

#[derive(Debug, Default)]
pub struct Dictionary {
    /// `(key, text)` 到权重。
    pub entries: BTreeMap<(String, String), i64>,
}

/// 构建读取的 `stroke.dict.yaml`：有 `--dictionary` checkout 时按 `Sources::pinned` 读（按 checkout 的 `upstream.lock.json` 校验），文件缺失或不符都报错。`languages` 必须传 `--dictionary`，所以命令行构建总是走这一支；后面的缓存兜底（只接受 `--cache` 下大小与 SHA-256 等于 `SOURCE_SIZE`、`SOURCE_SHA256` 的文件，都没有时返回 `None`）只有不带 checkout 的 `Sources` 才会走到，目前只有单元测试这样构造。
pub fn source(sources: &Sources) -> Result<Option<PathBuf>> {
    if sources.lock.files.iter().any(|file| file.path == SOURCE)
        || sources.checkout_file(SOURCE).is_some()
    {
        return sources.pinned(SOURCE).map(Some);
    }
    let cached = sources.cache.join(SOURCE);
    if !cached.exists() {
        return Ok(None);
    }
    check_cached(&cached, SOURCE_SIZE, SOURCE_SHA256)?;
    Ok(Some(cached))
}

fn check_cached(path: &Path, size: u64, sha256: &str) -> Result<()> {
    if !path.is_file() {
        bail!(
            "{SOURCE} is not in the cache at {} and no --dictionary checkout was given; download {REPOSITORY}/raw/{COMMIT}/stroke.dict.yaml there",
            path.display()
        );
    }
    let found_size = std::fs::metadata(path)
        .with_context(|| format!("reading {}", path.display()))?
        .len();
    let found_sha256 = sha256_file(path)?;
    if found_size != size || found_sha256 != sha256 {
        bail!(
            "{}: {found_size} bytes with sha256 {found_sha256}, rime-stroke {COMMIT} has {size} bytes with {sha256}",
            path.display()
        );
    }
    Ok(())
}

/// 记为 `source_commit` 的上游提交：锁文件的 `rime-stroke` 引用，未固定时是 `COMMIT`。
pub fn source_commit(sources: &Sources) -> &str {
    sources
        .lock
        .references
        .get(REFERENCE)
        .map_or(COMMIT, |reference| reference.commit.as_str())
}

/// YAML 头之后的 `字<TAB>笔顺码` 行。空行与 `#` 注释跳过；其他格式（权重列、多字词条、笔画以外的字母）都让构建失败，并写明行号。
pub fn parse(source: &str) -> Result<Vec<Row<'_>>> {
    let mut lines = source.lines().enumerate();
    if !lines.by_ref().any(|(_, line)| line == "...") {
        bail!("{SOURCE}: no `...` line ends the YAML header");
    }
    let mut rows = Vec::new();
    for (index, line) in lines {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        let [text, code] = fields[..] else {
            bail!("{SOURCE}: line {number} is not character<TAB>strokes");
        };
        if text.chars().count() != 1 {
            bail!("{SOURCE}: line {number}: {text:?} is not one character");
        }
        if code.is_empty() || !code.chars().all(|stroke| STROKES.contains(&stroke)) {
            bail!("{SOURCE}: line {number}: {code:?} is not a sequence of h, s, p, n and z");
        }
        rows.push(Row { text, code });
    }
    Ok(rows)
}

/// `single-chars.txt` 的 `字<TAB>拼音<TAB>权重` 行（CRLF 换行，`#` 行跳过），每个字所有读音的权重之和。
pub fn parse_frequencies(source: &str) -> Result<HashMap<&str, i64>> {
    let mut weights: HashMap<&str, i64> = HashMap::new();
    for (index, line) in source.lines().enumerate() {
        let line = text::strip(line);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        let [text, _, weight] = fields[..] else {
            bail!("{FREQUENCIES}: line {number} is not character<TAB>pinyin<TAB>weight");
        };
        let weight = weight
            .parse::<i64>()
            .ok()
            .filter(|weight| *weight >= 0)
            .with_context(|| format!("{FREQUENCIES}: line {number}: {weight:?} is not a weight"))?;
        let total = weights.entry(text).or_insert(0);
        *total = total.checked_add(weight).with_context(|| {
            format!("{FREQUENCIES}: line {number}: the weights of {text:?} overflow")
        })?;
    }
    Ok(weights)
}

/// 每个 `(笔顺码, 字)` 一条，权重是该字的字频之和；同一行重复出现只留一条。`kept` 不收的字跳过。
pub fn build(rows: &[Row], frequencies: &HashMap<&str, i64>) -> Dictionary {
    let mut dictionary = Dictionary::default();
    for row in rows {
        let weight = frequencies.get(row.text).copied().unwrap_or(0);
        if !kept(row.text, weight) {
            continue;
        }
        dictionary
            .entries
            .insert((row.code.to_owned(), row.text.to_owned()), weight);
    }
    dictionary
}

/// 基本区和扩展 A 区的汉字都收；其他字符（扩展 B 区及以后、兼容汉字、部首、笔画符号、西夏文部件）只在有字频时收。
fn kept(text: &str, weight: i64) -> bool {
    let common = text.chars().next().is_some_and(
        |character| matches!(character, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}'),
    );
    common || weight > 0
}

/// 按引擎的语言词典结构把 `dictionary` 写到 `path`（先删掉旧文件），最后 freeze，同样的输入得到同样的字节。
pub fn write(dictionary: &Dictionary, path: &Path, source_commit: &str) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    let mut connection = sqlite::open(path)?;
    connection.execute_batch(language_dictionary::SCHEMA)?;
    let transaction = connection.transaction()?;
    {
        let mut metadata = transaction.prepare("INSERT INTO metadata VALUES (?1, ?2)")?;
        for (name, value) in [
            (
                language_dictionary::METADATA_FORMAT_VERSION,
                language_dictionary::FORMAT_VERSION.to_string().as_str(),
            ),
            (language_dictionary::METADATA_SOURCE_COMMIT, source_commit),
            (language_dictionary::METADATA_LICENSE, LICENSE),
        ] {
            metadata.execute((name, value))?;
        }
        let mut syllables = transaction.prepare("INSERT INTO syllables VALUES (?1)")?;
        let strokes: BTreeSet<String> = STROKES.iter().map(char::to_string).collect();
        for stroke in &strokes {
            syllables.execute((stroke,))?;
        }
        let mut entries = transaction.prepare("INSERT INTO entries VALUES (?1, ?2, ?3)")?;
        for ((key, text), weight) in &dictionary.entries {
            entries.execute((key, text, weight))?;
        }
    }
    transaction.commit()?;
    sqlite::analyze(&connection, false)?;
    sqlite::integrity_check(&connection)?;
    drop(connection);
    sqlite::freeze(path)
}

/// `verify` 在写好的数据库里数出的值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub syllables: usize,
    pub entries: usize,
    pub characters: usize,
    /// 有字频（权重大于 0）的字数。
    pub weighted: usize,
}

/// 像引擎那样打开写好的数据库；`syllables` 不是五个笔画、数量低于 `floors`，或 `expected` 的码前几条里没有对应的字时失败。
pub fn verify(path: &Path, floors: Floors, expected: &[Expected]) -> Result<Counts> {
    let dictionary = language_dictionary::open_read_only(path)
        .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
    let connection =
        sqlite::open_read_only(path).with_context(|| format!("opening {}", path.display()))?;
    let count = |sql: &str| -> Result<usize> {
        let count: i64 = connection.query_row(sql, [], |row| row.get(0))?;
        Ok(usize::try_from(count)?)
    };
    let counts = Counts {
        syllables: count("SELECT count(*) FROM syllables")?,
        entries: count("SELECT count(*) FROM entries")?,
        characters: count("SELECT count(DISTINCT text) FROM entries")?,
        weighted: count("SELECT count(DISTINCT text) FROM entries WHERE weight > 0")?,
    };
    for stroke in STROKES {
        let stroke = stroke.to_string();
        let found = dictionary
            .has_syllable(&stroke)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !found {
            bail!("{}: the stroke {stroke:?} is not listed", path.display());
        }
    }
    if counts.syllables != STROKES.len() {
        bail!(
            "{}: {} syllables, expected the {} strokes",
            path.display(),
            counts.syllables,
            STROKES.len()
        );
    }
    for (what, found, floor) in [
        ("entries", counts.entries, floors.entries),
        ("characters", counts.characters, floors.characters),
        ("weighted characters", counts.weighted, floors.weighted),
    ] {
        if found < floor {
            bail!(
                "{}: {found} {what}, below the floor of {floor}",
                path.display()
            );
        }
    }
    for check in expected {
        let top = dictionary
            .lookup(check.key, check.within)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !top
            .iter()
            .any(|entry| check.texts.contains(&entry.text.as_str()))
        {
            let found: Vec<&str> = top.iter().map(|entry| entry.text.as_str()).collect();
            bail!(
                "{}: {:?} offers {found:?} first, without {}",
                path.display(),
                check.key,
                check.texts.join(" or ")
            );
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "# Rime dictionary: stroke\n# encoding: utf-8\n#\n# h,s,p,n,z\n\n---\nname: stroke\nversion: \"2.0\"\nsort: by_weight\n...\n\n";

    fn stroke_source() -> String {
        format!(
            "{HEADER}甲\th\n乙\th\n丙\thh\n丁\thsp\n戊\tzpn\n戊\tspn\n# 注释行\n己\tzpn\n庚\tzpn\n"
        )
    }

    // 合成字频：乙有两个读音，权重相加；丁、庚不在表里，权重为 0。
    const FREQUENCIES_SOURCE: &str =
        "#  字表\r\n甲\tjia\t50\r\n乙\tyi\t30\r\n乙\tzhe\t40\r\n丙\tbing\t9\r\n戊\twu\t100\r\n己\tji\t60\r\n辛\txin\t5\r\n";

    fn sample() -> Dictionary {
        let source = stroke_source();
        build(
            &parse(&source).unwrap(),
            &parse_frequencies(FREQUENCIES_SOURCE).unwrap(),
        )
    }

    fn entries(dictionary: &Dictionary) -> Vec<(&str, &str, i64)> {
        dictionary
            .entries
            .iter()
            .map(|((key, text), weight)| (key.as_str(), text.as_str(), *weight))
            .collect()
    }

    #[test]
    fn rows_after_the_header_keep_their_stroke_letters() {
        let source = stroke_source();
        let rows = parse(&source).unwrap();
        assert_eq!(rows.len(), 8);
        assert_eq!(
            rows[0],
            Row {
                text: "甲",
                code: "h"
            }
        );
        // 同一个字的第二个笔顺码是独立的一行。
        assert_eq!(
            rows[5],
            Row {
                text: "戊",
                code: "spn"
            }
        );
    }

    #[test]
    fn frequencies_sum_every_reading_of_a_character() {
        let frequencies = parse_frequencies(FREQUENCIES_SOURCE).unwrap();
        assert_eq!(frequencies["乙"], 70);
        assert_eq!(frequencies["甲"], 50);
        assert_eq!(frequencies.len(), 6);
    }

    #[test]
    fn only_common_blocks_and_characters_with_a_frequency_are_kept() {
        // 基本区与扩展 A 区：没有字频也收。
        assert!(kept("口", 0));
        assert!(kept("\u{3AD1}", 0));
        // 扩展 B 区及以后、西夏文部件、部首、笔画符号、兼容汉字：没有字频就不收。
        for text in [
            "\u{20BB9}",
            "\u{31480}",
            "\u{18800}",
            "\u{2E8A}",
            "\u{31C0}",
            "\u{F900}",
        ] {
            assert!(!kept(text, 0), "{text}");
        }
        // 有字频的字照收，不论区段。
        assert!(kept("\u{20BB9}", 1));
        let rows = [
            Row {
                text: "口",
                code: "szh",
            },
            Row {
                text: "\u{20BB9}",
                code: "szhhsp",
            },
            Row {
                text: "\u{3AD1}",
                code: "szhhzp",
            },
        ];
        let dictionary = build(&rows, &HashMap::new());
        let texts: Vec<&str> = dictionary
            .entries
            .keys()
            .map(|(_, text)| text.as_str())
            .collect();
        assert_eq!(texts, ["口", "\u{3AD1}"]);
    }

    #[test]
    fn every_code_of_a_character_carries_its_frequency() {
        assert_eq!(
            entries(&sample()),
            [
                ("h", "乙", 70),
                ("h", "甲", 50),
                ("hh", "丙", 9),
                // 不在字频表里的字权重为 0。
                ("hsp", "丁", 0),
                ("spn", "戊", 100),
                ("zpn", "己", 60),
                ("zpn", "庚", 0),
                ("zpn", "戊", 100),
            ]
        );
    }

    #[test]
    fn malformed_rows_name_their_line() {
        for (source, expected) in [
            ("name: stroke\n甲\th\n".to_owned(), "no `...` line"),
            (
                format!("{HEADER}甲\n"),
                "line 12 is not character<TAB>strokes",
            ),
            (
                format!("{HEADER}甲\th\t100\n"),
                "line 12 is not character<TAB>strokes",
            ),
            (
                format!("{HEADER}甲乙\th\n"),
                "line 12: \"甲乙\" is not one character",
            ),
            (format!("{HEADER}\th\n"), "\"\" is not one character"),
            (
                format!("{HEADER}甲\thx\n"),
                "line 12: \"hx\" is not a sequence of h, s, p, n and z",
            ),
            (format!("{HEADER}甲\t12\n"), "\"12\" is not a sequence"),
            (format!("{HEADER}甲\t\n"), "\"\" is not a sequence"),
        ] {
            let error = parse(&source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        for (source, expected) in [
            ("甲\tjia\n", "line 1 is not character<TAB>pinyin<TAB>weight"),
            ("甲\tjia\t-1\n", "line 1: \"-1\" is not a weight"),
            ("甲\tjia\tmany\n", "\"many\" is not a weight"),
            (
                "甲\tjia\t9223372036854775807\r\n甲\tjia\t1\r\n",
                "line 2: the weights of \"甲\" overflow",
            ),
        ] {
            let error = parse_frequencies(source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn the_cached_source_must_match_the_upstream_digest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SOURCE);
        let error = check_cached(&path, 7, "0").unwrap_err().to_string();
        assert!(
            error.contains(&format!("{REPOSITORY}/raw/{COMMIT}/stroke.dict.yaml")),
            "{error}"
        );
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"fixture").unwrap();
        let digest = sha256_file(&path).unwrap();
        check_cached(&path, 7, &digest).unwrap();
        let error = check_cached(&path, 8, &digest).unwrap_err().to_string();
        assert!(error.contains("7 bytes with sha256"), "{error}");
        let error = check_cached(&path, 7, &"0".repeat(64))
            .unwrap_err()
            .to_string();
        assert!(error.contains(&digest), "{error}");
    }

    /// 锁文件没固定、缓存里也没有源文件时不构建笔画词库，而不是让整个 `languages` 失败；缓存里放了不对的文件仍然报错。
    #[test]
    fn an_unpinned_and_uncached_source_is_skipped() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut lock =
            crate::sources::Lock::load(&root.join("resources/dictionary-sources.lock.json"))
                .unwrap();
        lock.files.retain(|file| file.path != SOURCE);
        let dir = tempfile::tempdir().unwrap();
        let sources = Sources {
            lock,
            repository_inputs: root.join("resources/dictionary-sources"),
            cache: dir.path().to_path_buf(),
            offline: true,
            dictionary: None,
        };
        assert!(source(&sources).unwrap().is_none());
        let cached = dir.path().join(SOURCE);
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
        std::fs::write(&cached, b"not rime-stroke").unwrap();
        assert!(source(&sources).is_err());
    }

    /// 锁文件的 `rime-stroke` 引用正是许可证覆盖的提交，锁文件不固定 `sources/stroke/` 下的任何文件，`stroke.dict.yaml` 是 msime 认定的 rime-stroke 上游数据。
    #[test]
    fn the_lock_references_the_covered_commit_and_pins_no_source() {
        let lock = crate::sources::Lock::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap();
        assert_eq!(lock.references[REFERENCE].commit, COMMIT);
        assert!(!lock
            .files
            .iter()
            .any(|file| file.path.starts_with("sources/stroke/")));
        assert_eq!(crate::sources::upstream_reference(SOURCE), Some(REFERENCE));
    }

    fn written(dictionary: &Dictionary) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DATABASE);
        write(dictionary, &path, COMMIT).unwrap();
        (dir, path)
    }

    #[test]
    fn the_engine_reads_what_is_written_and_the_bytes_repeat() {
        let dictionary = sample();
        let (_dir, path) = written(&dictionary);
        let engine = language_dictionary::open_read_only(&path).unwrap();
        let texts: Vec<String> = engine
            .lookup("zpn", 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.text)
            .collect();
        assert_eq!(texts, ["戊", "己", "庚"]);
        // 笔顺码没有空格，前缀补全覆盖以它开头的所有码。
        let completions: Vec<(String, String)> = engine
            .lookup_completions("h", 10)
            .unwrap()
            .into_iter()
            .map(|(key, entry)| (key, entry.text))
            .collect();
        assert_eq!(
            completions,
            [
                ("h".to_owned(), "乙".to_owned()),
                ("h".to_owned(), "甲".to_owned()),
                ("hh".to_owned(), "丙".to_owned()),
                ("hsp".to_owned(), "丁".to_owned()),
            ]
        );
        assert_eq!(engine.syllables().unwrap().len(), 5);
        assert!(engine.has_syllable("z").unwrap());
        assert!(!engine.has_syllable("x").unwrap());
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_SOURCE_COMMIT)
                .unwrap()
                .as_deref(),
            Some(COMMIT)
        );
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_LICENSE)
                .unwrap()
                .as_deref(),
            Some(LICENSE)
        );
        let first = std::fs::read(&path).unwrap();
        write(&dictionary, &path, COMMIT).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
    }

    #[test]
    fn verify_enforces_the_floors_and_the_expected_entries() {
        let (_dir, path) = written(&sample());
        let floors = Floors {
            entries: 8,
            characters: 7,
            weighted: 5,
        };
        let expected = [
            Expected {
                key: "h",
                texts: &["乙"],
                within: 1,
            },
            Expected {
                key: "zpn",
                texts: &["己", "庚"],
                within: 2,
            },
        ];
        assert_eq!(
            verify(&path, floors, &expected).unwrap(),
            Counts {
                syllables: 5,
                entries: 8,
                characters: 7,
                weighted: 5
            }
        );
        for (below, what) in [
            (
                Floors {
                    entries: 9,
                    ..floors
                },
                "8 entries, below the floor of 9",
            ),
            (
                Floors {
                    characters: 8,
                    ..floors
                },
                "7 characters, below the floor of 8",
            ),
            (
                Floors {
                    weighted: 6,
                    ..floors
                },
                "5 weighted characters, below the floor of 6",
            ),
        ] {
            let error = verify(&path, below, &expected).unwrap_err().to_string();
            assert!(error.contains(what), "{error}");
        }
        let error = verify(&path, FLOORS, &expected).unwrap_err().to_string();
        assert!(error.contains("below the floor"), "{error}");

        // 甲 不是 h 的首选，首位检查失败。
        let check = [Expected {
            key: "h",
            texts: &["甲"],
            within: 1,
        }];
        let error = verify(&path, floors, &check).unwrap_err().to_string();
        assert!(
            error.contains("\"h\" offers [\"乙\"] first, without 甲"),
            "{error}"
        );
    }
}
