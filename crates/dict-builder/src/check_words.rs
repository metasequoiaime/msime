//! `check-words`: the gate for changes to the hand-edited files in the dictionary source repository (metasequoiaime/msime-dictionary): custom/words.txt, custom/translations.txt and custom/english.txt. A change to any of them may only append lines. Every appended entry must pass the parser the build uses and be new: not repeated within the change and not already in the file. Weighted entries (words, English words) must also keep their weight within the range the file already uses, and an entry already in a shipped database (a word in msime-pinyin.db's quanpin table for its pinyin, an English word and display in msime-english.db's english_words) is rejected when that database is given. A translation may override an existing source with a different gloss, as the build's last-line-wins does, but repeating the same source and gloss is a duplicate. Appended blank and `#` comment lines are skipped, as the build skips them.

use std::collections::HashMap;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Serialize;

use crate::english::{
    parse_custom_english_line, parse_custom_translation, CUSTOM_ENGLISH, CUSTOM_TRANSLATIONS,
};
use crate::msime::{parse_custom_word, pinyin_table};
use crate::text;

/// Which custom file a base/head pair is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Kind {
    Words,
    Translations,
    English,
}

impl Kind {
    /// The file's path in msime-dictionary, as reports name it.
    pub fn path(self) -> &'static str {
        match self {
            Kind::Words => "custom/words.txt",
            Kind::Translations => CUSTOM_TRANSLATIONS,
            Kind::English => CUSTOM_ENGLISH,
        }
    }

    /// The bare file name the rejection reasons use ("words.txt").
    fn name(self) -> &'static str {
        let path = self.path();
        path.rsplit('/').next().unwrap_or(path)
    }

    /// Splits the file into lines the way the build reads it: words.txt as it always has, the other two after dropping a byte-order mark, as their parsers do.
    fn lines(self, contents: &str) -> Vec<&str> {
        match self {
            Kind::Words => text::splitlines(contents),
            Kind::Translations | Kind::English => text::splitlines(text::without_bom(contents)),
        }
    }

    fn parse(self, line: &str) -> Result<Option<Entry>> {
        Ok(match self {
            Kind::Words => parse_custom_word(line)?.map(|entry| Entry::Word {
                word: entry.value,
                pinyin: entry.key,
                weight: entry.weight,
            }),
            Kind::Translations => parse_custom_translation(line)?.map(|entry| Entry::Translation {
                source: entry.source,
                gloss: entry.gloss,
            }),
            Kind::English => parse_custom_english_line(line)?.map(|entry| Entry::English {
                word: entry.word,
                display: entry.display,
                weight: entry.weight,
            }),
        })
    }
}

/// One parsed entry. Serialized without a tag: the report's `file` says which kind it is.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Entry {
    Word {
        word: String,
        pinyin: String,
        weight: i64,
    },
    Translation {
        source: String,
        gloss: String,
    },
    English {
        word: String,
        display: String,
        weight: i64,
    },
}

impl Entry {
    /// What makes two entries the same: a word under the same pinyin, a translation with the same source and gloss, an English word with the same display. The build keeps one row per key.
    fn key(&self) -> (String, String) {
        match self {
            Entry::Word { word, pinyin, .. } => (pinyin.clone(), word.clone()),
            Entry::Translation { source, gloss } => (source.clone(), gloss.clone()),
            Entry::English { word, display, .. } => (word.clone(), display.clone()),
        }
    }

    fn weight(&self) -> Option<i64> {
        match self {
            Entry::Word { weight, .. } | Entry::English { weight, .. } => Some(*weight),
            Entry::Translation { .. } => None,
        }
    }
}

/// One file to check: its contents before and after the change.
pub struct Input<'a> {
    pub kind: Kind,
    pub base: &'a str,
    pub head: &'a str,
}

/// The shipped databases additions must not repeat; either may be absent.
#[derive(Default, Clone, Copy)]
pub struct Shipped<'a> {
    /// msime-pinyin.db, for custom/words.txt.
    pub msime: Option<&'a Connection>,
    /// msime-english.db, for custom/english.txt.
    pub english: Option<&'a Connection>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// True when nothing was rejected.
    pub accepted: bool,
    /// words.txt's line counts and weight range, kept at the top level as the words-only report had them; null when words.txt was not checked. `files` has the same for every checked file.
    pub base_lines: Option<usize>,
    pub head_lines: Option<usize>,
    pub weight_range: Option<(i64, i64)>,
    pub files: Vec<FileSummary>,
    pub added: Vec<Added>,
    pub rejected: Vec<Rejected>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileSummary {
    pub file: &'static str,
    pub base_lines: usize,
    pub head_lines: usize,
    /// The lowest and highest weight of the entries already in the file; `None` when it has none or its entries carry no weight.
    pub weight_range: Option<(i64, i64)>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Added {
    /// The file's path in msime-dictionary.
    pub file: &'static str,
    /// 1-based line number in the head file.
    pub line: usize,
    #[serde(flatten)]
    pub entry: Entry,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Rejected {
    /// The file's path in msime-dictionary.
    pub file: &'static str,
    /// 1-based line number: in the head file, or in the base file for a removed line.
    pub line: usize,
    pub text: String,
    pub reason: String,
}

/// Checks each head against its base. An error means the check itself could not run (a base line the build would reject, a shipped database without the expected table); a rejected contribution is reported, not returned as an error.
pub fn check(inputs: &[Input], shipped: Shipped) -> Result<Report> {
    let mut report = Report {
        accepted: true,
        base_lines: None,
        head_lines: None,
        weight_range: None,
        files: Vec::new(),
        added: Vec::new(),
        rejected: Vec::new(),
    };
    for input in inputs {
        let summary = check_file(input, shipped, &mut report)?;
        if input.kind == Kind::Words {
            report.base_lines = Some(summary.base_lines);
            report.head_lines = Some(summary.head_lines);
            report.weight_range = summary.weight_range;
        }
        report.files.push(summary);
    }
    report.accepted = report.rejected.is_empty();
    Ok(report)
}

fn check_file(input: &Input, shipped: Shipped, report: &mut Report) -> Result<FileSummary> {
    let kind = input.kind;
    let (file, name) = (kind.path(), kind.name());
    let base_lines = kind.lines(input.base);
    let head_lines = kind.lines(input.head);
    let mut summary = FileSummary {
        file,
        base_lines: base_lines.len(),
        head_lines: head_lines.len(),
        weight_range: None,
    };

    let mut existing: HashMap<(String, String), usize> = HashMap::new();
    for (index, line) in base_lines.iter().enumerate() {
        let Some(entry) = kind
            .parse(line)
            .with_context(|| format!("base {name}:{}", index + 1))?
        else {
            continue;
        };
        if let Some(weight) = entry.weight() {
            summary.weight_range = Some(match summary.weight_range {
                None => (weight, weight),
                Some((low, high)) => (low.min(weight), high.max(weight)),
            });
        }
        existing.entry(entry.key()).or_insert(index + 1);
    }

    if let Some(index) = (0..base_lines.len()).find(|&i| head_lines.get(i) != Some(&base_lines[i]))
    {
        let (text, what) = match head_lines.get(index) {
            Some(changed) => (*changed, "changed"),
            None => (base_lines[index], "removed"),
        };
        report.rejected.push(Rejected {
            file,
            line: index + 1,
            text: text.to_owned(),
            reason: format!(
                "line {} of {name} was {what}; only appending new lines is allowed",
                index + 1
            ),
        });
        return Ok(summary);
    }

    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    for (index, line) in head_lines.iter().enumerate().skip(base_lines.len()) {
        let number = index + 1;
        let reject = |reason: String| Rejected {
            file,
            line: number,
            text: (*line).to_owned(),
            reason,
        };
        let entry = match kind.parse(line) {
            Ok(Some(entry)) => entry,
            Ok(None) => continue,
            Err(error) => {
                report.rejected.push(reject(format!("{error:#}")));
                continue;
            }
        };
        let key = entry.key();
        let previous = seen.insert(key.clone(), number);
        let out_of_range =
            entry
                .weight()
                .zip(summary.weight_range)
                .and_then(|(weight, (low, high))| {
                    (!(low..=high).contains(&weight)).then_some((weight, low, high))
                });
        let reason = if let Some((weight, low, high)) = out_of_range {
            Some(format!(
                "weight {weight} is outside the range {name} uses ({low} to {high})"
            ))
        } else if let Some(previous) = previous {
            Some(format!("duplicates line {previous} of this change"))
        } else if let Some(line) = existing.get(&key) {
            Some(format!("already in {name} at line {line}"))
        } else {
            in_shipped(&entry, shipped)?
        };
        match reason {
            Some(reason) => report.rejected.push(reject(reason)),
            None => report.added.push(Added {
                file,
                line: number,
                entry,
            }),
        }
    }
    Ok(summary)
}

/// The rejection reason when a shipped database already has the entry.
fn in_shipped(entry: &Entry, shipped: Shipped) -> Result<Option<String>> {
    Ok(match (entry, shipped) {
        (
            Entry::Word { word, pinyin, .. },
            Shipped {
                msime: Some(connection),
                ..
            },
        ) => in_shipped_quanpin(connection, pinyin, word)?
            .then(|| "already in the shipped msime-pinyin.db for this pinyin".to_owned()),
        (
            Entry::English { word, display, .. },
            Shipped {
                english: Some(connection),
                ..
            },
        ) => in_shipped_english(connection, word, display)?.then(|| {
            "already in the shipped msime-english.db for this word and display".to_owned()
        }),
        _ => None,
    })
}

fn in_shipped_quanpin(connection: &Connection, key: &str, value: &str) -> Result<bool> {
    let table = pinyin_table(key).context("a parsed custom word maps to a quanpin table")?;
    connection
        .query_row(
            &format!("select exists(select 1 from {table} where key = ?1 and value = ?2)"),
            [key, value],
            |row| row.get(0),
        )
        .with_context(|| {
            format!("looking up {value:?} in the shipped msime-pinyin.db table {table}")
        })
}

fn in_shipped_english(connection: &Connection, word: &str, display: &str) -> Result<bool> {
    connection
        .query_row(
            "select exists(select 1 from english_words where word = ?1 and display = ?2)",
            [word, display],
            |row| row.get(0),
        )
        .with_context(|| {
            format!("looking up {display:?} in the shipped msime-english.db english_words")
        })
}

/// A short summary for a pull-request comment or a job summary. Contributed text is escaped so it renders literally.
pub fn markdown(report: &Report) -> String {
    let mut out = format!(
        "## Custom words check\n\n{} added, {} rejected.\n",
        report.added.len(),
        report.rejected.len()
    );
    if !report.rejected.is_empty() {
        out.push_str(
            "\n### Rejected\n\n| File | Line | Entry | Reason |\n| --- | ---: | --- | --- |\n",
        );
        for rejected in &report.rejected {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                escape(rejected.file),
                rejected.line,
                escape(&rejected.text),
                escape(&rejected.reason)
            ));
        }
    }
    for summary in &report.files {
        let added: Vec<&Added> = report
            .added
            .iter()
            .filter(|added| added.file == summary.file)
            .collect();
        let Some(first) = added.first() else {
            continue;
        };
        let header = match first.entry {
            Entry::Word { .. } => "| Line | Word | Pinyin | Weight |\n| ---: | --- | --- | ---: |",
            Entry::Translation { .. } => "| Line | Source | Gloss |\n| ---: | --- | --- |",
            Entry::English { .. } => {
                "| Line | Word | Display | Weight |\n| ---: | --- | --- | ---: |"
            }
        };
        out.push_str(&format!(
            "\n### Added to {}\n\n{header}\n",
            escape(summary.file)
        ));
        for added in added {
            let row = match &added.entry {
                Entry::Word {
                    word,
                    pinyin,
                    weight,
                } => format!("{} | {} | {weight}", escape(word), escape(pinyin)),
                Entry::Translation { source, gloss } => {
                    format!("{} | {}", escape(source), escape(gloss))
                }
                Entry::English {
                    word,
                    display,
                    weight,
                } => format!("{} | {} | {weight}", escape(word), escape(display)),
            };
            out.push_str(&format!("| {} | {row} |\n", added.line));
        }
    }
    out
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' | '|' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("&#9;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "# words\n你好\tni'hao\t1\n宣传\txuan'chuan\t100\n";

    fn words(base: &str, head: &str, msime: Option<&Connection>) -> Result<Report> {
        let input = Input {
            kind: Kind::Words,
            base,
            head,
        };
        check(
            &[input],
            Shipped {
                msime,
                english: None,
            },
        )
    }

    fn run(head: &str) -> Report {
        words(BASE, head, None).unwrap()
    }

    fn reasons(report: &Report) -> Vec<(usize, &str)> {
        report
            .rejected
            .iter()
            .map(|rejected| (rejected.line, rejected.reason.as_str()))
            .collect()
    }

    #[test]
    fn appended_valid_lines_are_added() {
        let report = run(&format!("{BASE}新词\txin'ci\t5\n\n# note\n"));
        assert!(report.accepted);
        assert_eq!(report.weight_range, Some((1, 100)));
        assert_eq!(
            report.added,
            [Added {
                file: "custom/words.txt",
                line: 4,
                entry: Entry::Word {
                    word: "新词".into(),
                    pinyin: "xin'ci".into(),
                    weight: 5
                }
            }]
        );
        assert!(report.rejected.is_empty());
    }

    #[test]
    fn an_unchanged_file_adds_nothing() {
        let report = run(BASE);
        assert!(report.accepted);
        assert!(report.added.is_empty());
    }

    #[test]
    fn a_base_without_a_final_newline_can_still_be_appended_to() {
        let base = BASE.trim_end_matches('\n');
        let report = words(base, &format!("{base}\n新词\txin'ci\t5\n"), None).unwrap();
        assert!(report.accepted, "{:?}", report.rejected);
        assert_eq!(report.added.len(), 1);
    }

    #[test]
    fn edits_deletions_and_reordering_are_rejected() {
        let edited = run("# words\n你好\tni'hao\t2\n宣传\txuan'chuan\t100\n新词\txin'ci\t5\n");
        assert!(!edited.accepted);
        assert_eq!(edited.rejected.len(), 1);
        assert_eq!(edited.rejected[0].line, 2);
        assert!(edited.rejected[0].reason.contains("changed"));
        assert!(edited.added.is_empty());

        let removed = run("# words\n你好\tni'hao\t1\n");
        assert_eq!(reasons(&removed).len(), 1);
        assert_eq!(removed.rejected[0].line, 3);
        assert!(removed.rejected[0].reason.contains("removed"));

        let reordered = run("# words\n宣传\txuan'chuan\t100\n你好\tni'hao\t1\n");
        assert!(!reordered.accepted);
        assert_eq!(reordered.rejected[0].line, 2);

        let inserted = run("# words\n新词\txin'ci\t5\n你好\tni'hao\t1\n宣传\txuan'chuan\t100\n");
        assert!(!inserted.accepted);
        assert_eq!(inserted.rejected[0].line, 2);
    }

    #[test]
    fn appended_lines_the_build_would_reject_are_rejected() {
        let report = run(&format!(
            "{BASE}词\tci\n词\tCi\t1\n词\tci\tx\n\tci\t1\n词\tci'\t1\n好词\thao'ci\t6\n"
        ));
        assert!(!report.accepted);
        let lines: Vec<usize> = report.rejected.iter().map(|r| r.line).collect();
        assert_eq!(lines, [4, 5, 6, 7, 8]);
        assert!(report.rejected[0]
            .reason
            .contains("expected word, pinyin and weight"));
        assert!(report.rejected[1].reason.contains("not quanpin"));
        assert!(report.rejected[2].reason.contains("not an integer"));
        assert!(report.rejected[3]
            .reason
            .contains("expected word, pinyin and weight"));
        assert_eq!(report.added.len(), 1);
        assert_eq!(report.added[0].line, 9);
    }

    #[test]
    fn weights_outside_the_existing_range_are_rejected() {
        let report = run(&format!("{BASE}低\tdi\t0\n高\tgao\t101\n中\tzhong\t100\n"));
        assert_eq!(
            reasons(&report),
            [
                (4, "weight 0 is below 1"),
                (
                    5,
                    "weight 101 is outside the range words.txt uses (1 to 100)"
                )
            ]
        );
        assert_eq!(report.added.len(), 1);
    }

    #[test]
    fn duplicates_within_the_change_and_against_words_txt_are_rejected() {
        let report = run(&format!(
            "{BASE}新词\txin'ci\t5\n新词\txin'ci\t6\n宣传\txuan'chuan\t3\n新词\txin'ci'a\t5\n"
        ));
        assert_eq!(
            reasons(&report),
            [
                (5, "duplicates line 4 of this change"),
                (6, "already in words.txt at line 3")
            ]
        );
        // The same word under another pinyin is a different entry.
        assert_eq!(report.added.len(), 2);
    }

    #[test]
    fn duplicates_of_the_shipped_quanpin_table_are_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "create table tbl_2_x (key text, jp text, value text, weight integer);
                 insert into tbl_2_x values ('xin''ci', 'xc', '新词', 7);",
            )
            .unwrap();
        let head = format!("{BASE}新词\txin'ci\t5\n心词\txin'ci\t5\n");
        let report = words(BASE, &head, Some(&connection)).unwrap();
        assert_eq!(
            reasons(&report),
            [(4, "already in the shipped msime-pinyin.db for this pinyin")]
        );
        assert!(matches!(&report.added[0].entry, Entry::Word { word, .. } if word == "心词"));
    }

    #[test]
    fn a_malformed_base_is_an_error_not_a_rejection() {
        assert!(words("词\tci\n", "词\tci\n", None).is_err());
    }

    #[test]
    fn the_report_serializes_and_the_summary_escapes_contributed_text() {
        let report = run(&format!("{BASE}<b>|\tbu\t5\n新词\txin'ci\t5\n"));
        let json: serde_json::Value = serde_json::to_value(&report).unwrap();
        assert_eq!(json["accepted"], true);
        assert_eq!(json["added"][0]["word"], "<b>|");
        assert_eq!(json["weight_range"], serde_json::json!([1, 100]));

        let summary = markdown(&report);
        assert!(summary.contains("2 added, 0 rejected."), "{summary}");
        assert!(summary.contains("| 4 | \\<b\\>\\| | bu | 5 |"), "{summary}");
        assert!(!summary.contains("### Rejected"));
        assert_eq!(json["added"][0]["file"], "custom/words.txt");
        assert_eq!(json["files"][0]["file"], "custom/words.txt");
        assert_eq!(json["base_lines"], 3);
        assert!(
            summary.contains("### Added to custom/words.txt"),
            "{summary}"
        );
    }

    const TRANSLATIONS: &str = "\u{feff}# glosses\n水杉\tdawn redwood\nredwood\t红杉\n";

    fn translations(head: &str) -> Report {
        let input = Input {
            kind: Kind::Translations,
            base: TRANSLATIONS,
            head,
        };
        check(&[input], Shipped::default()).unwrap()
    }

    #[test]
    fn translations_are_append_only_and_parse_as_the_build_parses_them() {
        let report = translations(&format!(
            "{TRANSLATIONS}银杏\tginkgo\n水杉\tmetasequoia\n水杉\tdawn redwood\n银杏\tginkgo\n只有来源\n来源\t \textra\n"
        ));
        assert_eq!(
            reasons(&report),
            [
                (6, "already in translations.txt at line 2"),
                (7, "duplicates line 4 of this change"),
                (8, "expected source<TAB>gloss, got \"只有来源\""),
                (9, "empty source or gloss"),
            ]
        );
        // A different gloss for an existing source overrides it, as the build's last line does.
        assert_eq!(
            report
                .added
                .iter()
                .map(|added| &added.entry)
                .collect::<Vec<_>>(),
            [
                &Entry::Translation {
                    source: "银杏".into(),
                    gloss: "ginkgo".into()
                },
                &Entry::Translation {
                    source: "水杉".into(),
                    gloss: "metasequoia".into()
                },
            ]
        );
        assert!(report
            .added
            .iter()
            .all(|added| added.file == "custom/translations.txt"));
        assert_eq!(report.base_lines, None);
        assert_eq!(report.files[0].weight_range, None);

        let edited = translations("\u{feff}# glosses\n水杉\tredwood\nredwood\t红杉\n");
        assert_eq!(
            reasons(&edited),
            [(
                2,
                "line 2 of translations.txt was changed; only appending new lines is allowed"
            )]
        );
    }

    const ENGLISH: &str = "figma\tfigma\t1\nasr\tASR\t3\nwebview\tWebview\t2\n";

    fn english(head: &str, shipped: Option<&Connection>) -> Report {
        let input = Input {
            kind: Kind::English,
            base: ENGLISH,
            head,
        };
        let shipped = Shipped {
            msime: None,
            english: shipped,
        };
        check(&[input], shipped).unwrap()
    }

    #[test]
    fn english_words_are_checked_for_format_weight_and_repeats() {
        let report = english(
            &format!(
                "{ENGLISH}rust\tRust\t2\nrust\tRust\t3\nasr\tASR\t1\ncargo\tcargo\t4\nCargo\tCargo\t1\ncargo\t\t1\nwebview\twebview2\t1\n"
            ),
            None,
        );
        assert_eq!(
            reasons(&report),
            [
                (5, "duplicates line 4 of this change"),
                (6, "already in english.txt at line 2"),
                (7, "weight 4 is outside the range english.txt uses (1 to 3)"),
                (8, "\"Cargo\" is not a lowercase ASCII word"),
                (9, "the display is empty"),
            ]
        );
        assert_eq!(
            report
                .added
                .iter()
                .map(|added| &added.entry)
                .collect::<Vec<_>>(),
            [
                &Entry::English {
                    word: "rust".into(),
                    display: "Rust".into(),
                    weight: 2
                },
                &Entry::English {
                    word: "webview".into(),
                    display: "webview2".into(),
                    weight: 1
                },
            ]
        );
        assert_eq!(report.files[0].weight_range, Some((1, 3)));
    }

    #[test]
    fn english_words_already_in_the_shipped_english_db_are_rejected() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "create table english_words (word text, display text, weight integer, primary key (word, display));
                 insert into english_words values ('rust', 'rust', 900);",
            )
            .unwrap();
        let report = english(
            &format!("{ENGLISH}rust\trust\t1\nrust\tRust\t1\n"),
            Some(&connection),
        );
        assert_eq!(
            reasons(&report),
            [(
                4,
                "already in the shipped msime-english.db for this word and display"
            )]
        );
        assert_eq!(report.added.len(), 1);
    }

    #[test]
    fn several_files_share_one_report_that_names_each_file() {
        let inputs = [
            Input {
                kind: Kind::Words,
                base: BASE,
                head: &format!("{BASE}新词\txin'ci\t500\n"),
            },
            Input {
                kind: Kind::Translations,
                base: TRANSLATIONS,
                head: &format!("{TRANSLATIONS}银杏\tginkgo\n"),
            },
            Input {
                kind: Kind::English,
                base: ENGLISH,
                head: &format!("{ENGLISH}rust\tRust\t2\n"),
            },
        ];
        let report = check(&inputs, Shipped::default()).unwrap();
        assert!(!report.accepted);
        assert_eq!(report.rejected.len(), 1);
        assert_eq!(report.rejected[0].file, "custom/words.txt");
        assert_eq!(
            report
                .files
                .iter()
                .map(|file| file.file)
                .collect::<Vec<_>>(),
            [
                "custom/words.txt",
                "custom/translations.txt",
                "custom/english.txt"
            ]
        );
        assert_eq!(report.base_lines, Some(3));
        assert_eq!(report.weight_range, Some((1, 100)));

        let json: serde_json::Value = serde_json::to_value(&report).unwrap();
        assert_eq!(json["added"][0]["file"], "custom/translations.txt");
        assert_eq!(json["added"][0]["source"], "银杏");
        assert_eq!(json["added"][1]["display"], "Rust");
        assert_eq!(json["rejected"][0]["file"], "custom/words.txt");

        let summary = markdown(&report);
        assert!(summary.contains("2 added, 1 rejected."), "{summary}");
        assert!(summary.contains("| custom/words.txt | 4 | 新词&#9;xin'ci&#9;500 | weight 500 is outside the range words.txt uses (1 to 100) |"), "{summary}");
        assert!(
            summary.contains("### Added to custom/translations.txt\n\n| Line | Source | Gloss |"),
            "{summary}"
        );
        assert!(summary.contains("| 4 | 银杏 | ginkgo |"), "{summary}");
        assert!(
            summary
                .contains("### Added to custom/english.txt\n\n| Line | Word | Display | Weight |"),
            "{summary}"
        );
        assert!(summary.contains("| 4 | rust | Rust | 2 |"), "{summary}");
        assert!(
            !summary.contains("### Added to custom/words.txt"),
            "{summary}"
        );
    }
}
