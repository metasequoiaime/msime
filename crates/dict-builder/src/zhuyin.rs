//! `msime-zhuyin.db`：注音方案的注音词库，按 `msime_engine::language_dictionary` 定义的结构写出。数据是 libchewing-data（LGPL-2.1-or-later，见 `resources/licenses/libchewing-data-LGPL-2.1.txt`）在 `resources/dictionary-sources.lock.json` 的 `libchewing-data` 引用所记提交的 `dict/chewing/tsi.csv`、`dict/chewing/word.csv`，由 msime-dictionary 原样收在 `sources/zhuyin/` 下，锁文件按固定提交读取。
//!
//! Three files are read, all `text,frequency,reading` CSV: `tsi.csv` (phrases and characters with their use counts), `word.csv` (every character with each of its readings, all at frequency 0), and the McBopomofo phrase supplement (frequency 0). The scheme types toned syllables, so an entry's key is its syllables joined by one space as the files write them (`ㄋㄧˇ ㄏㄠˇ`): tone 1 is unmarked and ˊ ˇ ˋ ˙ follow the letters. A row appearing more than once keeps its largest frequency, so a `word.csv` character and a supplement phrase weigh 0 unless `tsi.csv` gives the same combination a count.
//!
//! The syllable inventory is every syllable some key uses. A syllable must be at most one initial, one medial and one rime in that order, then an optional tone mark, because that is all the Dachen editor can compose; anything else fails the build. The only rows left out are the four tone marks listed as their own text and reading, which have no letters to type.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use msime_engine::language_dictionary;
use msime_engine::zhuyin::layout::{self, Kind};
use rusqlite::{Connection, OpenFlags};

use crate::sqlite;

pub const PHRASES: &str = "sources/zhuyin/tsi.csv";
pub const CHARACTERS: &str = "sources/zhuyin/word.csv";
pub const SUPPLEMENT: &str = "sources/zhuyin/mcbopomofo-supplement.txt";
/// The sources lock reference whose commit is recorded as the database's `source_commit`.
pub const REFERENCE: &str = "libchewing-data";
/// The SPDX identifier recorded as the database's `license`, as the CSV headers declare it.
pub const LICENSE: &str = "LGPL-2.1-or-later";
pub const DATABASE: &str = "msime-zhuyin.db";
/// The licence text in `resources/licenses/` and the name it ships under beside the database.
pub const LICENSE_SOURCE: &str = "libchewing-data-LGPL-2.1.txt";
pub const LICENSE_NAME: &str = "msime-libchewing_data_LICENSE.txt";

/// The floors `verify` enforces on a release build.
pub const FLOORS: Floors = Floors {
    syllables: 1_300,
    characters: 13_000,
    phrases: 120_000,
};

/// Keys whose first `within` entries must include one of the texts, as a check against gross ranking errors.
pub const EXPECTED: [Expected; 2] = [
    Expected {
        key: "ㄋㄧˇ ㄏㄠˇ",
        texts: &["你好"],
        within: 1,
    },
    Expected {
        key: "ㄊㄞˊ ㄨㄢ",
        texts: &["臺灣", "台灣"],
        within: 2,
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
    pub syllables: usize,
    pub characters: usize,
    pub phrases: usize,
}

/// One row of `tsi.csv` or `word.csv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    /// Toned syllables, one per character of `text`.
    pub syllables: Vec<String>,
    pub frequency: i64,
}

#[derive(Debug, Default)]
pub struct Dictionary {
    pub syllables: BTreeSet<String>,
    /// `(key, text)` to weight.
    pub entries: BTreeMap<(String, String), i64>,
}

/// Where a symbol sits in a syllable: initial, then medial, then rime.
fn slot(symbol: char) -> Option<u8> {
    let (_, _, kind) = layout::DACHEN
        .iter()
        .find(|(_, dachen_symbol, _)| *dachen_symbol == symbol)?;
    Some(match kind {
        Kind::Initial => 0,
        Kind::Medial => 1,
        Kind::Rime => 2,
    })
}

/// Whether the Dachen editor can compose `syllable`: one to three bopomofo letters in initial, medial, rime order, each slot at most once, then an optional tone mark.
fn is_syllable(syllable: &str) -> bool {
    let letters = layout::TONE_MARKS[1..]
        .iter()
        .find_map(|mark| syllable.strip_suffix(mark))
        .unwrap_or(syllable);
    let mut previous = None;
    !letters.is_empty()
        && letters.chars().all(|symbol| {
            let Some(slot) = slot(symbol) else {
                return false;
            };
            let ordered = previous.is_none_or(|previous| slot > previous);
            previous = Some(slot);
            ordered
        })
}

/// `text,frequency,reading` rows of the file `name`, skipping `#` lines and naming the 1-based line of a malformed row.
pub fn parse(name: &str, source: &str) -> Result<Vec<Row>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(source.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.with_context(|| format!("reading {name}"))?;
        // `#` lines are skipped here rather than through the reader's comment option, which leaves them out of its line count.
        if record.get(0).is_some_and(|first| first.starts_with('#')) {
            continue;
        }
        let line = record.position().map_or(0, csv::Position::line);
        let [text, frequency, reading] = record.iter().collect::<Vec<_>>()[..] else {
            bail!("{name}: line {line} is not text,frequency,reading");
        };
        // The tone marks list themselves so chewing can type them; the editor has no letterless syllable to reach them with.
        if text == reading && layout::TONE_MARKS[1..].contains(&reading) {
            continue;
        }
        let frequency = frequency
            .parse::<i64>()
            .ok()
            .filter(|frequency| *frequency >= 0)
            .with_context(|| format!("{name}: line {line}: {frequency:?} is not a frequency"))?;
        // Two tsi.csv rows (博客來, 新媒體) start their reading with a space, so syllables split on any run of spaces.
        let syllables: Vec<String> = reading.split_whitespace().map(str::to_owned).collect();
        if let Some(bad) = syllables.iter().find(|syllable| !is_syllable(syllable)) {
            bail!("{name}: line {line}: {bad:?} is not a bopomofo syllable");
        }
        if text.is_empty() || text.chars().count() != syllables.len() {
            bail!("{name}: line {line}: {text:?} does not have one syllable per character in {reading:?}");
        }
        rows.push(Row {
            text: text.to_owned(),
            syllables,
            frequency,
        });
    }
    Ok(rows)
}

/// The dictionary of the parsed rows of both files. A `(key, text)` pair keeps the largest frequency any row gives it.
pub fn build(rows: &[Row]) -> Dictionary {
    let mut dictionary = Dictionary::default();
    for row in rows {
        dictionary.syllables.extend(row.syllables.iter().cloned());
        let weight = dictionary
            .entries
            .entry((row.syllables.join(" "), row.text.clone()))
            .or_insert(row.frequency);
        *weight = (*weight).max(row.frequency);
    }
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
    pub phrases: usize,
}

/// Opens a written database the way the engine does and fails when it is below `floors` or a key of `expected` lacks its text among its first entries.
pub fn verify(path: &Path, floors: Floors, expected: &[Expected]) -> Result<Counts> {
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
        phrases: count("SELECT count(*) FROM entries WHERE length(text) > 1")?,
    };
    for (what, found, floor) in [
        ("syllables", counts.syllables, floors.syllables),
        ("character entries", counts.characters, floors.characters),
        ("phrase entries", counts.phrases, floors.phrases),
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

    const HEADER: &str = "# dc:title,內建詞庫,\n# dc:rights,Copyright (c) 2025 libchewing Core Team,\n# dc:license,LGPL-2.1-or-later,\n# dc:identifier,2025.11.17,\n";

    fn phrases_source() -> String {
        format!("{HEADER}ˇ,1,ˇ\nㄅ,0,ㄅ\n你,89941,ㄋㄧˇ\n妳,16127,ㄋㄧˇ\n好,30909,ㄏㄠˇ\n好,309,ㄏㄠˋ\n你好,1227,ㄋㄧˇ ㄏㄠˇ\n妳好,427,ㄋㄧˇ ㄏㄠˇ\n台灣,124258,ㄊㄞˊ ㄨㄢ\n臺灣,15908,ㄊㄞˊ ㄨㄢ\n博客來,6,  ㄅㄛˊ ㄎㄜˋ ㄌㄞˊ\n你好,1000,ㄋㄧˇ ㄏㄠˇ\n")
    }

    fn characters_source() -> String {
        format!("{HEADER}ˋ,0,ˋ\n你,0,ㄋㄧˇ\n妳,0,ㄋㄞˇ\n兒,0,ㄦ\n誒,0,ㄝˋ\n")
    }

    fn sample() -> Dictionary {
        let mut rows = parse(PHRASES, &phrases_source()).unwrap();
        rows.extend(parse(CHARACTERS, &characters_source()).unwrap());
        build(&rows)
    }

    fn entries(dictionary: &Dictionary) -> Vec<(&str, &str, i64)> {
        dictionary
            .entries
            .iter()
            .map(|((key, text), weight)| (key.as_str(), text.as_str(), *weight))
            .collect()
    }

    #[test]
    fn rows_after_the_header_keep_their_toned_syllables() {
        let rows = parse(PHRASES, &phrases_source()).unwrap();
        // The tone mark row is left out.
        assert_eq!(rows.len(), 11);
        assert_eq!(
            rows[0],
            Row {
                text: "ㄅ".to_owned(),
                syllables: vec!["ㄅ".to_owned()],
                frequency: 0
            }
        );
        assert_eq!(rows[5].syllables, ["ㄋㄧˇ", "ㄏㄠˇ"]);
        // Tone 1 is unmarked.
        assert_eq!(rows[7].syllables, ["ㄊㄞˊ", "ㄨㄢ"]);
        // The leading spaces of the reading do not make an empty syllable.
        assert_eq!(rows[9].syllables, ["ㄅㄛˊ", "ㄎㄜˋ", "ㄌㄞˊ"]);
    }

    #[test]
    fn a_row_keeps_its_largest_frequency_across_both_files() {
        assert_eq!(
            entries(&sample()),
            [
                ("ㄅ", "ㄅ", 0),
                ("ㄅㄛˊ ㄎㄜˋ ㄌㄞˊ", "博客來", 6),
                ("ㄊㄞˊ ㄨㄢ", "台灣", 124258),
                ("ㄊㄞˊ ㄨㄢ", "臺灣", 15908),
                // word.csv gives 妳 this reading only, so it keeps the word.csv weight of 0.
                ("ㄋㄞˇ", "妳", 0),
                // tsi.csv counts 你 under the reading word.csv also lists, so the count wins over 0.
                ("ㄋㄧˇ", "你", 89941),
                ("ㄋㄧˇ", "妳", 16127),
                // 你好 is listed twice; the larger count wins.
                ("ㄋㄧˇ ㄏㄠˇ", "你好", 1227),
                ("ㄋㄧˇ ㄏㄠˇ", "妳好", 427),
                ("ㄏㄠˇ", "好", 30909),
                ("ㄏㄠˋ", "好", 309),
                ("ㄝˋ", "誒", 0),
                ("ㄦ", "兒", 0),
            ]
        );
    }

    #[test]
    fn the_inventory_holds_every_syllable_a_key_uses() {
        let dictionary = sample();
        let syllables: Vec<&str> = dictionary.syllables.iter().map(String::as_str).collect();
        assert_eq!(
            syllables,
            [
                "ㄅ", "ㄅㄛˊ", "ㄊㄞˊ", "ㄋㄞˇ", "ㄋㄧˇ", "ㄌㄞˊ", "ㄎㄜˋ", "ㄏㄠˇ", "ㄏㄠˋ",
                "ㄝˋ", "ㄦ", "ㄨㄢ"
            ]
        );
    }

    #[test]
    fn syllables_follow_the_slots_the_editor_fills() {
        for syllable in [
            "ㄅ",
            "ㄧ",
            "ㄚ",
            "ㄓ",
            "ㄋㄧˇ",
            "ㄓㄨㄤ",
            "ㄌㄩㄝˋ",
            "ㄇㄚ˙",
            "ㄦˊ",
        ] {
            assert!(is_syllable(syllable), "{syllable}");
        }
        for syllable in [
            "",
            "ˇ",
            "ㄧㄋ",
            "ㄋㄏ",
            "ㄚㄛ",
            "ㄋㄧˇˇ",
            "ˇㄋㄧ",
            "ㄋㄧˉ",
            "ㄋa",
        ] {
            assert!(!is_syllable(syllable), "{syllable}");
        }
    }

    #[test]
    fn malformed_rows_name_their_file_and_line() {
        for (source, expected) in [
            (
                format!("{HEADER}你,1\n"),
                "tsi.csv: line 5 is not text,frequency,reading",
            ),
            (format!("{HEADER}你,1,ㄋㄧˇ,x\n"), "line 5 is not"),
            (
                format!("{HEADER}你,-1,ㄋㄧˇ\n"),
                "line 5: \"-1\" is not a frequency",
            ),
            (
                format!("{HEADER}你,many,ㄋㄧˇ\n"),
                "\"many\" is not a frequency",
            ),
            (
                format!("{HEADER}你,1,ni3\n"),
                "line 5: \"ni3\" is not a bopomofo syllable",
            ),
            (
                format!("{HEADER}你,1,ㄧㄋˇ\n"),
                "\"ㄧㄋˇ\" is not a bopomofo syllable",
            ),
            (
                format!("{HEADER}你好,1,ㄋㄧˇ\n"),
                "line 5: \"你好\" does not have one syllable per character",
            ),
            (format!("{HEADER},1,\n"), "\"\" does not have one syllable"),
            // A tone mark whose text is something else is not the tone mark listing itself.
            (
                format!("{HEADER}啊,1,ˇ\n"),
                "\"ˇ\" is not a bopomofo syllable",
            ),
        ] {
            let error = parse(PHRASES, &source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn only_the_pinned_files_are_read() {
        let lock = crate::sources::Lock::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap();
        assert_eq!(lock.references[REFERENCE].commit.len(), 40);
        let mut pinned: Vec<&str> = lock
            .files
            .iter()
            .filter(|file| file.path.starts_with("sources/zhuyin/"))
            .map(|file| {
                crate::sources::assert_dictionary_repository_file(file);
                file.path.as_str()
            })
            .collect();
        pinned.sort_unstable();
        assert_eq!(pinned, [SUPPLEMENT, PHRASES, CHARACTERS]);
    }

    fn written(dictionary: &Dictionary) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DATABASE);
        write(
            dictionary,
            &path,
            "c44e81aef24b06f1509f19e1be54c99812d0c43f",
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
            .lookup("ㄋㄧˇ", 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.text)
            .collect();
        assert_eq!(texts, ["你", "妳"]);
        assert!(engine.has_syllable("ㄨㄢ").unwrap());
        assert!(!engine.has_syllable("ˇ").unwrap());
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_SOURCE_COMMIT)
                .unwrap()
                .as_deref(),
            Some("c44e81aef24b06f1509f19e1be54c99812d0c43f")
        );
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_LICENSE)
                .unwrap()
                .as_deref(),
            Some(LICENSE)
        );
        let first = std::fs::read(&path).unwrap();
        write(
            &dictionary,
            &path,
            "c44e81aef24b06f1509f19e1be54c99812d0c43f",
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
    }

    #[test]
    fn verify_enforces_the_floors_and_the_expected_entries() {
        let (_dir, path) = written(&sample());
        let floors = Floors {
            syllables: 12,
            characters: 8,
            phrases: 5,
        };
        assert_eq!(
            verify(&path, floors, &EXPECTED).unwrap(),
            Counts {
                syllables: 12,
                characters: 8,
                phrases: 5
            }
        );
        for (below, what) in [
            (
                Floors {
                    syllables: 13,
                    ..floors
                },
                "12 syllables, below the floor of 13",
            ),
            (
                Floors {
                    characters: 9,
                    ..floors
                },
                "8 character entries, below the floor of 9",
            ),
            (
                Floors {
                    phrases: 6,
                    ..floors
                },
                "5 phrase entries, below the floor of 6",
            ),
        ] {
            let error = verify(&path, below, &EXPECTED).unwrap_err().to_string();
            assert!(error.contains(what), "{error}");
        }
        let error = verify(&path, FLOORS, &EXPECTED).unwrap_err().to_string();
        assert!(error.contains("below the floor"), "{error}");

        // 妳好 outranking 你好 fails the first-place check.
        let mut rows = parse(PHRASES, &phrases_source()).unwrap();
        rows.push(Row {
            text: "妳好".to_owned(),
            syllables: vec!["ㄋㄧˇ".to_owned(), "ㄏㄠˇ".to_owned()],
            frequency: 5000,
        });
        let (_dir, path) = written(&build(&rows));
        let none = Floors {
            syllables: 0,
            characters: 0,
            phrases: 0,
        };
        let error = verify(&path, none, &EXPECTED).unwrap_err().to_string();
        assert!(error.contains("[\"妳好\"] first, without 你好"), "{error}");
        // Either spelling of Taiwan within the first two passes.
        assert!(verify(&path, none, &EXPECTED[1..]).is_ok());
    }
}
