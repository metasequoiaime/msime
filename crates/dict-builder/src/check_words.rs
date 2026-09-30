//! `check-words`: the gate for changes to custom/words.txt in the dictionary source repository (metasequoiaime/msime-dictionary). A change may only append lines. Every appended entry must pass the parser the build uses, keep its weight within the range the file already uses, and be new: not repeated within the change, not already in words.txt, and, when a shipped msime.db is given, not already in the quanpin table for its pinyin. Appended blank and `#` comment lines are skipped, as the build skips them.

use std::collections::HashMap;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::Serialize;

use crate::msime::{parse_custom_word, pinyin_table};
use crate::text;

#[derive(Debug, Serialize)]
pub struct Report {
    /// True when nothing was rejected.
    pub accepted: bool,
    pub base_lines: usize,
    pub head_lines: usize,
    /// The lowest and highest weight of the entries already in words.txt; `None` when it has none.
    pub weight_range: Option<(i64, i64)>,
    pub added: Vec<Added>,
    pub rejected: Vec<Rejected>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Added {
    /// 1-based line number in the head file.
    pub line: usize,
    pub word: String,
    pub pinyin: String,
    pub weight: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Rejected {
    /// 1-based line number: in the head file, or in the base file for a removed line.
    pub line: usize,
    pub text: String,
    pub reason: String,
}

/// Checks `head` against `base`. An error means the check itself could not run (a base line the build would reject, a shipped database without the expected table); a rejected contribution is reported, not returned as an error.
pub fn check(base: &str, head: &str, shipped: Option<&Connection>) -> Result<Report> {
    let base_lines = text::splitlines(base);
    let head_lines = text::splitlines(head);
    let mut report = Report {
        accepted: true,
        base_lines: base_lines.len(),
        head_lines: head_lines.len(),
        weight_range: None,
        added: Vec::new(),
        rejected: Vec::new(),
    };

    let mut existing: HashMap<(String, String), usize> = HashMap::new();
    for (index, line) in base_lines.iter().enumerate() {
        let Some(entry) =
            parse_custom_word(line).with_context(|| format!("base words.txt:{}", index + 1))?
        else {
            continue;
        };
        report.weight_range = Some(match report.weight_range {
            None => (entry.weight, entry.weight),
            Some((low, high)) => (low.min(entry.weight), high.max(entry.weight)),
        });
        existing
            .entry((entry.key, entry.value))
            .or_insert(index + 1);
    }

    if let Some(index) = (0..base_lines.len()).find(|&i| head_lines.get(i) != Some(&base_lines[i]))
    {
        let rejected = match head_lines.get(index) {
            Some(changed) => Rejected {
                line: index + 1,
                text: (*changed).to_owned(),
                reason: format!(
                    "line {} of words.txt was changed; only appending new lines is allowed",
                    index + 1
                ),
            },
            None => Rejected {
                line: index + 1,
                text: base_lines[index].to_owned(),
                reason: format!(
                    "line {} of words.txt was removed; only appending new lines is allowed",
                    index + 1
                ),
            },
        };
        report.rejected.push(rejected);
        report.accepted = false;
        return Ok(report);
    }

    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    for (index, line) in head_lines.iter().enumerate().skip(base_lines.len()) {
        let number = index + 1;
        let reject = |reason: String| Rejected {
            line: number,
            text: (*line).to_owned(),
            reason,
        };
        let entry = match parse_custom_word(line) {
            Ok(Some(entry)) => entry,
            Ok(None) => continue,
            Err(error) => {
                report.rejected.push(reject(format!("{error:#}")));
                continue;
            }
        };
        let key = (entry.key.clone(), entry.value.clone());
        let previous = seen.insert(key.clone(), number);
        let reason = if let Some((low, high)) = report
            .weight_range
            .filter(|(low, high)| !(*low..=*high).contains(&entry.weight))
        {
            Some(format!(
                "weight {} is outside the range words.txt uses ({low} to {high})",
                entry.weight
            ))
        } else if let Some(previous) = previous {
            Some(format!("duplicates line {previous} of this change"))
        } else if let Some(line) = existing.get(&key) {
            Some(format!("already in words.txt at line {line}"))
        } else if let Some(connection) = shipped {
            in_shipped_table(connection, &entry.key, &entry.value)?
                .then(|| "already in the shipped msime.db for this pinyin".to_owned())
        } else {
            None
        };
        match reason {
            Some(reason) => report.rejected.push(reject(reason)),
            None => report.added.push(Added {
                line: number,
                word: entry.value,
                pinyin: entry.key,
                weight: entry.weight,
            }),
        }
    }
    report.accepted = report.rejected.is_empty();
    Ok(report)
}

fn in_shipped_table(connection: &Connection, key: &str, value: &str) -> Result<bool> {
    let table = pinyin_table(key).context("a parsed custom word maps to a quanpin table")?;
    connection
        .query_row(
            &format!("select exists(select 1 from {table} where key = ?1 and value = ?2)"),
            [key, value],
            |row| row.get(0),
        )
        .with_context(|| format!("looking up {value:?} in the shipped msime.db table {table}"))
}

/// A short summary for a pull-request comment or a job summary. Contributed text is escaped so it renders literally.
pub fn markdown(report: &Report) -> String {
    let mut out = format!(
        "## Custom words check\n\n{} added, {} rejected.\n",
        report.added.len(),
        report.rejected.len()
    );
    if !report.rejected.is_empty() {
        out.push_str("\n### Rejected\n\n| Line | Entry | Reason |\n| ---: | --- | --- |\n");
        for rejected in &report.rejected {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                rejected.line,
                escape(&rejected.text),
                escape(&rejected.reason)
            ));
        }
    }
    if !report.added.is_empty() {
        out.push_str(
            "\n### Added\n\n| Line | Word | Pinyin | Weight |\n| ---: | --- | --- | ---: |\n",
        );
        for added in &report.added {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                added.line,
                escape(&added.word),
                escape(&added.pinyin),
                added.weight
            ));
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

    fn run(head: &str) -> Report {
        check(BASE, head, None).unwrap()
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
                line: 4,
                word: "新词".into(),
                pinyin: "xin'ci".into(),
                weight: 5
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
        let report = check(base, &format!("{base}\n新词\txin'ci\t5\n"), None).unwrap();
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
        let report = check(BASE, &head, Some(&connection)).unwrap();
        assert_eq!(
            reasons(&report),
            [(4, "already in the shipped msime.db for this pinyin")]
        );
        assert_eq!(report.added[0].word, "心词");
    }

    #[test]
    fn a_malformed_base_is_an_error_not_a_rejection() {
        assert!(check("词\tci\n", "词\tci\n", None).is_err());
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
    }
}
