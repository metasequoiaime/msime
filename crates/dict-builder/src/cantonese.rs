//! `msime-cantonese.db`：粤语方案的粤拼词库，按 `msime_engine::language_dictionary` 定义的结构写出。数据是 rime-cantonese（CC BY 4.0，见 `resources/licenses/rime-cantonese-CC-BY-4.0.txt`）在 `resources/dictionary-sources.lock.json` 的 `rime-cantonese` 引用所记提交的文件，由 msime-dictionary 原样收在 `yue/` 下，锁文件按固定提交读取。
//!
//! Three files are read: `jyut6ping3.chars.dict.yaml` (one character, its toned reading and an optional `N%` share of that character's use per row), `jyut6ping3.words.dict.yaml` (one word and its toned readings per row) and `essay-cantonese.txt` (Rime's word frequency list, one `text<TAB>count` per line). `jyut6ping3.maps.dict.yaml` is ODbL and `jyut6ping3.phrase.dict.yaml` has no readings and no clear provenance, so neither is pinned or read; neither is `jyut6ping3.lettered.dict.yaml`.
//!
//! The scheme types toneless Jyutping, so every reading loses its tone digit and an entry's key is its toneless syllables joined by one space (`nei hou`). Readings that differ only in tone collapse into one entry with the largest weight. A word weighs its essay count. A word the essay does not list (about two in five, 食咗 and 我哋 among them) weighs `unlisted_word_weight`: 1 plus the base-2 logarithm of its rarest character's essay count, which orders such homophones by how common their characters are while staying far below the counts the essay gives the words it lists (most of them at least 100). A character weighs its essay count scaled by the row's share when the row gives one, never below 1.
//!
//! The words file lacks some of the commonest words because Rime composes them from characters at typing time (你好 is one), and the engine does not compose sentences. So an essay word the words file does not list becomes an entry too when the essay counts it at least `ESSAY_WORD_MIN_COUNT` times and each of its characters has exactly one toneless reading in the characters file, which makes its key certain.
//!
//! The syllable inventory is every toneless syllable some key uses, so every entry stays typable; the words file uses a few colloquial syllables (`fi`, `za`, `gwek`) that no character row has.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use msime_engine::language_dictionary;
use rusqlite::{Connection, OpenFlags};

use crate::sqlite;

pub const CHARACTERS: &str = "yue/jyut6ping3.chars.dict.yaml";
pub const WORDS: &str = "yue/jyut6ping3.words.dict.yaml";
pub const ESSAY: &str = "yue/essay-cantonese.txt";
/// The sources lock reference whose commit is recorded as the database's `source_commit`.
pub const REFERENCE: &str = "rime-cantonese";
/// The SPDX identifier recorded as the database's `license`.
pub const LICENSE: &str = "CC-BY-4.0";
pub const DATABASE: &str = "msime-cantonese.db";
/// The licence text in `resources/licenses/` and the name it ships under beside the database.
pub const LICENSE_SOURCE: &str = "rime-cantonese-CC-BY-4.0.txt";
pub const LICENSE_NAME: &str = "msime-rime_cantonese_LICENSE.txt";

/// The essay count from which a word missing from the words file is added. The essay's counts are scaled scores rather than raw counts (the median word of the words file scores 900), and 1000 keeps about five thousand frequent words such as 你好 while leaving out the long tail of novel names and segmentation fragments the essay also lists.
pub const ESSAY_WORD_MIN_COUNT: i64 = 1000;

/// The floors `verify` enforces on a release build.
pub const FLOORS: Floors = Floors {
    syllables: 600,
    characters: 25_000,
    words: 90_000,
};

/// Keys whose top three entries must include the text, as a check against gross ranking errors.
pub const EXPECTED: [(&str, &str); 2] = [("nei hou", "你好"), ("gwong dung waa", "廣東話")];

#[derive(Debug, Clone, Copy)]
pub struct Floors {
    pub syllables: usize,
    pub characters: usize,
    pub words: usize,
}

/// One row of the characters or words file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<'a> {
    pub text: &'a str,
    /// Toneless syllables.
    pub syllables: Vec<&'a str>,
    /// The row's share of the character's use in percent, when the row gives one.
    pub percent: Option<i64>,
}

#[derive(Debug, Default)]
pub struct Dictionary {
    pub syllables: BTreeSet<String>,
    /// `(key, text)` to weight.
    pub entries: BTreeMap<(String, String), i64>,
    /// Entries that came from the characters file, the words file and the essay.
    pub characters: usize,
    pub words: usize,
    pub essay_words: usize,
}

/// The rows after the YAML header, which ends at the `...` line, with their 1-based line numbers. Blank lines and `#` comments are skipped.
fn body<'a>(name: &str, source: &'a str) -> Result<impl Iterator<Item = (usize, &'a str)>> {
    let mut lines = source.lines().enumerate();
    if !lines.by_ref().any(|(_, line)| line == "...") {
        bail!("{name}: no `...` line ends the YAML header");
    }
    Ok(lines
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(index, line)| (index + 1, line)))
}

/// The toneless syllables of a space-separated toned reading such as `nei5 hou2`.
fn toneless<'a>(name: &str, line: usize, reading: &'a str) -> Result<Vec<&'a str>> {
    reading
        .split(' ')
        .map(|syllable| {
            let letters = syllable
                .strip_suffix(|c: char| ('1'..='6').contains(&c))
                .filter(|letters| {
                    !letters.is_empty() && letters.bytes().all(|b| b.is_ascii_lowercase())
                });
            letters.with_context(|| {
                format!("{name}: line {line}: {syllable:?} is not a toned Jyutping syllable")
            })
        })
        .collect()
}

/// `jyut6ping3.chars.dict.yaml`: `character<TAB>reading[<TAB>N%]`.
pub fn parse_characters(source: &str) -> Result<Vec<Row<'_>>> {
    let mut rows = Vec::new();
    for (line, row) in body(CHARACTERS, source)? {
        let fields: Vec<&str> = row.split('\t').collect();
        let (text, reading, percent) = match fields[..] {
            [text, reading] => (text, reading, None),
            [text, reading, percent] => (text, reading, Some(percent)),
            _ => bail!("{CHARACTERS}: line {line} is not character<TAB>reading[<TAB>N%]"),
        };
        if text.chars().count() != 1 {
            bail!("{CHARACTERS}: line {line}: {text:?} is not one character");
        }
        // A few unit characters read as two syllables (兡 baak3 hak1, a hectogram).
        let syllables = toneless(CHARACTERS, line, reading)?;
        let percent = percent
            .map(|percent| {
                percent
                    .strip_suffix('%')
                    .and_then(|number| number.parse::<i64>().ok())
                    .filter(|number| (0..=100).contains(number))
                    .with_context(|| {
                        format!("{CHARACTERS}: line {line}: {percent:?} is not a percentage")
                    })
            })
            .transpose()?;
        rows.push(Row {
            text,
            syllables,
            percent,
        });
    }
    Ok(rows)
}

/// `jyut6ping3.words.dict.yaml`: `word<TAB>reading`.
pub fn parse_words(source: &str) -> Result<Vec<Row<'_>>> {
    let mut rows = Vec::new();
    for (line, row) in body(WORDS, source)? {
        let fields: Vec<&str> = row.split('\t').collect();
        let [text, reading] = fields[..] else {
            bail!("{WORDS}: line {line} is not word<TAB>reading");
        };
        if text.is_empty() {
            bail!("{WORDS}: line {line} has no word");
        }
        rows.push(Row {
            text,
            syllables: toneless(WORDS, line, reading)?,
            percent: None,
        });
    }
    Ok(rows)
}

/// `essay-cantonese.txt`: `text<TAB>count`. A text listed twice keeps its larger count.
pub fn parse_essay(source: &str) -> Result<HashMap<&str, i64>> {
    let mut counts = HashMap::new();
    for (index, row) in source.lines().enumerate() {
        if row.is_empty() {
            continue;
        }
        let parsed = row.split_once('\t').and_then(|(text, count)| {
            Some((text, count.parse::<i64>().ok().filter(|count| *count >= 0)?))
        });
        let Some((text, count)) = parsed.filter(|(text, _)| !text.is_empty()) else {
            bail!("{ESSAY}: line {} is not text<TAB>count", index + 1);
        };
        let kept = counts.entry(text).or_insert(count);
        *kept = (*kept).max(count);
    }
    Ok(counts)
}

impl Dictionary {
    fn insert(&mut self, key: String, text: &str, weight: i64) {
        let slot = self.entries.entry((key, text.to_owned())).or_insert(weight);
        *slot = (*slot).max(weight);
    }
}

/// The weight of a word the essay does not list: 1 plus the base-2 logarithm of the essay count of its rarest character, or 1 when one of its characters is not counted at all. Real counts top out near 2^24, so this stays below 64 and under almost every count the essay gives a listed word.
fn unlisted_word_weight(text: &str, essay: &HashMap<&str, i64>) -> i64 {
    let mut buffer = [0; 4];
    let rarest = text
        .chars()
        .map(|character| {
            essay
                .get(&*character.encode_utf8(&mut buffer))
                .copied()
                .unwrap_or(0)
        })
        .min()
        .unwrap_or(0);
    1 + rarest.checked_ilog2().map_or(0, i64::from)
}

/// The dictionary of the three parsed inputs.
pub fn build(characters: &[Row], words: &[Row], essay: &HashMap<&str, i64>) -> Dictionary {
    let mut dictionary = Dictionary::default();
    // Each character's toneless keys, to give an essay word a key when each of its characters has only one.
    let mut readings: HashMap<&str, HashSet<String>> = HashMap::new();
    for row in characters {
        let count = essay.get(row.text).copied().unwrap_or(0);
        let weight = row
            .percent
            .map_or(count, |percent| count.saturating_mul(percent) / 100)
            .max(1);
        let key = row.syllables.join(" ");
        readings.entry(row.text).or_default().insert(key.clone());
        dictionary.insert(key, row.text, weight);
    }
    dictionary.characters = dictionary.entries.len();

    let mut listed = HashSet::new();
    for row in words {
        let weight = essay
            .get(row.text)
            .copied()
            .unwrap_or_else(|| unlisted_word_weight(row.text, essay));
        dictionary.insert(row.syllables.join(" "), row.text, weight);
        listed.insert(row.text);
    }
    dictionary.words = dictionary.entries.len() - dictionary.characters;

    for (&text, &count) in essay {
        if count < ESSAY_WORD_MIN_COUNT || text.chars().count() < 2 || listed.contains(text) {
            continue;
        }
        let keys: Option<Vec<&str>> = text
            .chars()
            .map(|character| {
                let mut buffer = [0; 4];
                let readings = readings.get(&*character.encode_utf8(&mut buffer))?;
                let mut only = readings.iter();
                match (only.next(), only.next()) {
                    (Some(key), None) => Some(key.as_str()),
                    _ => None,
                }
            })
            .collect();
        if let Some(keys) = keys {
            dictionary.insert(keys.join(" "), text, count);
        }
    }
    dictionary.essay_words = dictionary.entries.len() - dictionary.characters - dictionary.words;

    dictionary.syllables = dictionary
        .entries
        .keys()
        .flat_map(|(key, _)| key.split(' '))
        .map(str::to_owned)
        .collect();
    dictionary
}

/// Writes `dictionary` to `path` in the engine's language dictionary schema, replacing any earlier file, and freezes it so the same inputs give the same bytes.
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
        for syllable in &dictionary.syllables {
            syllables.execute((syllable,))?;
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

/// What `verify` counted in a written database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub syllables: usize,
    pub characters: usize,
    pub words: usize,
}

/// Opens a written database the way the engine does and fails when it is below `floors` or a key of `EXPECTED` lacks its text in the top three.
pub fn verify(path: &Path, floors: Floors) -> Result<Counts> {
    let dictionary = language_dictionary::open_read_only(path)
        .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {}", path.display()))?;
    let count = |sql: &str| -> Result<usize> {
        let count: i64 = connection.query_row(sql, [], |row| row.get(0))?;
        Ok(usize::try_from(count)?)
    };
    let counts = Counts {
        syllables: count("SELECT count(*) FROM syllables")?,
        characters: count("SELECT count(*) FROM entries WHERE length(text) = 1")?,
        words: count("SELECT count(*) FROM entries WHERE length(text) > 1")?,
    };
    for (what, found, floor) in [
        ("syllables", counts.syllables, floors.syllables),
        ("character entries", counts.characters, floors.characters),
        ("word entries", counts.words, floors.words),
    ] {
        if found < floor {
            bail!(
                "{}: {found} {what}, below the floor of {floor}",
                path.display()
            );
        }
    }
    for (key, text) in EXPECTED {
        let top = dictionary
            .lookup(key, 3)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !top.iter().any(|entry| entry.text == text) {
            let found: Vec<&str> = top.iter().map(|entry| entry.text.as_str()).collect();
            bail!(
                "{}: {key:?} offers {found:?} first, without {text}",
                path.display()
            );
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "# Rime dictionary\n# encoding: utf-8\n\n---\nname: jyut6ping3.chars\nversion: \"2026.08.10\"\nsort: by_weight\n...\n\n";

    fn characters_source() -> String {
        format!("{HEADER}你\tnei5\n妳\tnei5\n好\thou2\n好\thou3\t40%\n號\thou6\t0%\n廣\tgwong2\n東\tdung1\n話\twaa6\t80%\n話\twaa2\t20%\n呢\tne1\n呢\tnei4\t5%\n")
    }

    fn words_source() -> String {
        format!("{HEADER}廣東話\tgwong2 dung1 waa2\n廣東話\tgwong2 dung1 waa6\n你哋\tnei5 dei6\nfi li fe le\tfi4 li1 fe4 le4\n")
    }

    const ESSAY_TEXT: &str =
        "你\t7000\n好\t5000\n話\t900\n呢\t2000\n廣東話\t6851\n你好\t21493\n好呢\t3000\n你哋\t40\n號\t10\n你好\t5\n";

    fn entries(dictionary: &Dictionary) -> Vec<(&str, &str, i64)> {
        dictionary
            .entries
            .iter()
            .map(|((key, text), weight)| (key.as_str(), text.as_str(), *weight))
            .collect()
    }

    fn sample() -> Dictionary {
        let characters = characters_source();
        let words = words_source();
        build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &parse_essay(ESSAY_TEXT).unwrap(),
        )
    }

    #[test]
    fn rows_after_the_header_lose_their_tones() {
        let characters = characters_source();
        let rows = parse_characters(&characters).unwrap();
        assert_eq!(rows.len(), 11);
        assert_eq!(
            rows[3],
            Row {
                text: "好",
                syllables: vec!["hou"],
                percent: Some(40)
            }
        );
        let words = words_source();
        let rows = parse_words(&words).unwrap();
        assert_eq!(rows[0].syllables, ["gwong", "dung", "waa"]);
        assert_eq!(rows[3].text, "fi li fe le");
    }

    #[test]
    fn weights_follow_the_essay_and_the_share_of_use() {
        let dictionary = sample();
        assert_eq!(
            entries(&dictionary),
            [
                ("dung", "東", 1),
                ("fi li fe le", "fi li fe le", 1),
                ("gwong", "廣", 1),
                ("gwong dung waa", "廣東話", 6851),
                // 好 is hou2 (5000) and hou3 at 40% (2000); the tones collapse into the larger weight.
                ("hou", "好", 5000),
                // 0% of 10 floors at 1.
                ("hou", "號", 1),
                ("ne", "呢", 2000),
                ("nei", "你", 7000),
                // 5% of 2000.
                ("nei", "呢", 100),
                ("nei", "妳", 1),
                ("nei dei", "你哋", 40),
                // Every character of 你好 has one toneless reading and the essay counts it often enough; the larger of its two counts wins.
                ("nei hou", "你好", 21493),
                // 80% of 900, the larger share of waa2 and waa6.
                ("waa", "話", 720),
            ]
        );
        // 好呢 is frequent but 呢 reads ne or nei, so its key is uncertain and it stays out.
        assert!(!dictionary.entries.keys().any(|(_, text)| text == "好呢"));
        assert_eq!(
            (
                dictionary.characters,
                dictionary.words,
                dictionary.essay_words
            ),
            (9, 3, 1)
        );
    }

    #[test]
    fn unlisted_homophone_words_follow_their_rarest_character() {
        let characters = format!("{HEADER}食\tsik6\n識\tsik1\n咗\tzo2\n");
        let words = format!("{HEADER}識咗\tsik1 zo2\n食咗\tsik6 zo2\n");
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &parse_essay("食\t873763\n識\t379707\n咗\t1995799\n").unwrap(),
        );
        // 食 (2^19..2^20) is commoner than 識 (2^18..2^19), so 食咗 outweighs 識咗 although the essay lists neither word.
        assert_eq!(
            dictionary.entries[&("sik zo".to_owned(), "食咗".to_owned())],
            20
        );
        assert_eq!(
            dictionary.entries[&("sik zo".to_owned(), "識咗".to_owned())],
            19
        );
    }

    #[test]
    fn a_unit_character_keeps_its_two_syllables() {
        let characters = format!("{HEADER}兡\tbaak3 hak1\n一\tjat1\n");
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &[],
            &parse_essay("一兡\t1200\n").unwrap(),
        );
        assert_eq!(
            entries(&dictionary),
            [
                ("baak hak", "兡", 1),
                ("jat", "一", 1),
                ("jat baak hak", "一兡", 1200)
            ]
        );
    }

    #[test]
    fn rare_essay_words_are_left_out() {
        let characters = characters_source();
        let dictionary = build(
            &parse_characters(&characters).unwrap(),
            &[],
            &parse_essay("你好\t999\n").unwrap(),
        );
        assert_eq!(dictionary.essay_words, 0);
    }

    #[test]
    fn the_inventory_holds_every_syllable_a_key_uses() {
        let dictionary = sample();
        let syllables: Vec<&str> = dictionary.syllables.iter().map(String::as_str).collect();
        // fe, fi, le and li come only from the words file, dei from 你哋.
        assert_eq!(
            syllables,
            ["dei", "dung", "fe", "fi", "gwong", "hou", "le", "li", "ne", "nei", "waa"]
        );
    }

    #[test]
    fn malformed_rows_name_their_file_and_line() {
        for (source, expected) in [
            (format!("{HEADER}你\n"), "chars.dict.yaml: line 10 is not"),
            (
                format!("{HEADER}你好\tnei5\n"),
                "line 10: \"你好\" is not one character",
            ),
            (
                format!("{HEADER}你\tnei\n"),
                "line 10: \"nei\" is not a toned",
            ),
            (
                format!("{HEADER}你\tNei5\n"),
                "line 10: \"Nei5\" is not a toned",
            ),
            (
                format!("{HEADER}你\tnei5\t3\n"),
                "\"3\" is not a percentage",
            ),
            (
                format!("{HEADER}你\tnei5\t101%\n"),
                "\"101%\" is not a percentage",
            ),
            ("你\tnei5\n".to_owned(), "no `...` line"),
        ] {
            let error = parse_characters(&source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        let error = parse_words(&format!("{HEADER}\n你好\tnei5 hou2\t3\n"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("words.dict.yaml: line 11"), "{error}");
        let error = parse_words(&format!("{HEADER}你好\tnei5  hou2\n"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("\"\" is not a toned"), "{error}");
        for essay in ["你好\n", "你好\t-1\n", "\t3\n", "你好\t3\t4\n"] {
            let error = parse_essay(essay).unwrap_err().to_string();
            assert!(error.contains("essay-cantonese.txt: line 1"), "{error}");
        }
    }

    #[test]
    fn only_the_licensed_files_are_read_and_pinned() {
        for excluded in ["maps", "phrase", "lettered"] {
            assert!(
                [CHARACTERS, WORDS, ESSAY]
                    .iter()
                    .all(|input| !input.contains(excluded)),
                "{excluded}"
            );
        }
        let lock = crate::sources::Lock::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap();
        assert_eq!(lock.references[REFERENCE].commit.len(), 40);
        let mut pinned: Vec<&str> = lock
            .files
            .iter()
            .filter(|file| file.path.starts_with("yue/"))
            .map(|file| {
                crate::sources::assert_dictionary_repository_file(file);
                file.path.as_str()
            })
            .collect();
        pinned.sort_unstable();
        let mut inputs = vec![CHARACTERS, WORDS, ESSAY];
        inputs.sort_unstable();
        assert_eq!(pinned, inputs);
    }

    fn written(dictionary: &Dictionary) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DATABASE);
        write(
            dictionary,
            &path,
            "259f0e48bba840c3a2e0d117539e96937f3d89bc",
        )
        .unwrap();
        (dir, path)
    }

    #[test]
    fn the_engine_reads_what_is_written_and_the_bytes_repeat() {
        let dictionary = sample();
        let (_dir, path) = written(&dictionary);
        let engine = language_dictionary::open_read_only(&path).unwrap();
        let texts: Vec<String> = engine
            .lookup("nei", 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.text)
            .collect();
        assert_eq!(texts, ["你", "呢", "妳"]);
        assert!(engine.has_syllable("fi").unwrap());
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_SOURCE_COMMIT)
                .unwrap()
                .as_deref(),
            Some("259f0e48bba840c3a2e0d117539e96937f3d89bc")
        );
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_LICENSE)
                .unwrap()
                .as_deref(),
            Some(LICENSE)
        );
        let first = std::fs::read(&path).unwrap();
        // Writing again over the earlier file gives the same bytes.
        write(
            &dictionary,
            &path,
            "259f0e48bba840c3a2e0d117539e96937f3d89bc",
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
    }

    #[test]
    fn verify_enforces_the_floors_and_the_expected_entries() {
        let (_dir, path) = written(&sample());
        let floors = Floors {
            syllables: 11,
            characters: 9,
            words: 4,
        };
        assert_eq!(
            verify(&path, floors).unwrap(),
            Counts {
                syllables: 11,
                characters: 9,
                words: 4
            }
        );
        for (below, what) in [
            (
                Floors {
                    syllables: 12,
                    ..floors
                },
                "11 syllables, below the floor of 12",
            ),
            (
                Floors {
                    characters: 10,
                    ..floors
                },
                "9 character entries, below the floor of 10",
            ),
            (
                Floors { words: 5, ..floors },
                "4 word entries, below the floor of 5",
            ),
        ] {
            let error = verify(&path, below).unwrap_err().to_string();
            assert!(error.contains(what), "{error}");
        }
        let error = verify(&path, FLOORS).unwrap_err().to_string();
        assert!(error.contains("below the floor"), "{error}");

        // Without the essay, 你好 is missing from nei hou.
        let characters = characters_source();
        let words = words_source();
        let (_dir, path) = written(&build(
            &parse_characters(&characters).unwrap(),
            &parse_words(&words).unwrap(),
            &HashMap::new(),
        ));
        let error = verify(
            &path,
            Floors {
                syllables: 0,
                characters: 0,
                words: 0,
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("without 你好"), "{error}");
    }
}
