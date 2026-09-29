//! Turning a user's CSV/TXT word list into wordbook entries.
//!
//! Shaped after [`crate::dictionary::import`], which solved the same problem for dictionary files:
//! the envelope failing is an error, one bad row is not. A single stray line must not reject a
//! file of five thousand good words, so rows are skipped, counted, and reported by line number.
//!
//! The issue enum is this module's own rather than the dictionary one's, because the questions
//! differ — a wordbook row has no pinyin and no weight, and a dictionary row has no gloss. The
//! count of reported failures is shared, so the two importers tell the user "the first five" and
//! mean the same thing.

use super::wordbook::{self, Wordbook, WordbookEntry};
use crate::dictionary::import::REPORTED_FAILURES;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The most rows examined in one file, matching the dictionary importer's own ceiling.
pub const MAX_ROWS: usize = wordbook::MAX_ENTRIES;

/// Why one row was skipped.
///
/// Describes the shape of the problem and never echoes the row. A word list a user imports is
/// their own material, and the same privacy rule the dictionary importer states applies here.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WordbookImportIssue {
    /// Not two or three columns.
    ColumnCount,
    EmptyWord,
    WordTooLong,
    /// A control character in a field, which would be invisible on the card.
    ControlCharacter,
    EmptyMeaning,
    MeaningTooLong,
    PhoneticTooLong,
    /// The file already had this headword on an earlier line.
    Duplicate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WordbookImportFailure {
    /// 1-based line number in the submitted text, so a user can find the row.
    pub line: usize,
    pub issue: WordbookImportIssue,
}

/// The envelope was unusable, so no row was examined.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum WordbookImportError {
    #[error("wordbook import is empty")]
    Empty,
    #[error("wordbook import is too large")]
    TooLarge,
    #[error("wordbook import contains unsupported control characters")]
    ControlCharacters,
    #[error("wordbook import contains no usable rows")]
    NoUsableRows,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbookImportReport {
    pub entries: Vec<WordbookEntry>,
    /// Rows examined and rejected.
    pub failed: usize,
    /// The first few failures, for a message a user can act on.
    pub first_failures: Vec<WordbookImportFailure>,
    /// Rows beyond [`MAX_ROWS`] were not examined.
    pub truncated: bool,
}

/// How the file separates its columns.
///
/// Decided once for the whole file rather than per line. A file whose delimiter changed halfway
/// would otherwise parse its two halves into different column meanings, which is worse than
/// refusing the rows that do not fit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Delimiter {
    Tab,
    Comma,
}

/// Parse a user-supplied word list into a wordbook.
///
/// `id` and `name` are the host's, not the file's: a word list is rows, and letting a file name
/// its own store key would let an import silently overwrite another book's progress.
///
/// Returns `Err` only when the envelope itself is unusable.
pub fn parse_wordbook(
    id: &str,
    name: &str,
    text: &str,
    max_bytes: usize,
) -> Result<(Wordbook, WordbookImportReport), WordbookImportError> {
    let report = parse(text, max_bytes)?;
    let book = Wordbook {
        id: id.to_owned(),
        name: name.to_owned(),
        entries: report.entries.clone(),
    };
    Ok((book, report))
}

/// Parse a user-supplied word list.
pub fn parse(text: &str, max_bytes: usize) -> Result<WordbookImportReport, WordbookImportError> {
    // A leading U+FEFF is a byte-order mark, not content, and `str::trim` does not remove it. Left
    // in place it becomes part of the first headword, which then stores and displays a word that
    // can never match anything the user types. `crate::dictionary::import::parse` strips it for
    // the same reason and records the full history there.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.is_empty() {
        return Err(WordbookImportError::Empty);
    }
    if text.len() > max_bytes {
        return Err(WordbookImportError::TooLarge);
    }
    if crate::text::has_disallowed_control(text) {
        return Err(WordbookImportError::ControlCharacters);
    }

    let delimiter = if text.contains('\t') {
        Delimiter::Tab
    } else {
        Delimiter::Comma
    };

    let row_capacity = text.lines().take(MAX_ROWS).count();
    let mut report = WordbookImportReport {
        entries: Vec::with_capacity(row_capacity),
        first_failures: Vec::with_capacity(REPORTED_FAILURES),
        ..WordbookImportReport::default()
    };
    let mut seen = std::collections::BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        if report.entries.len() >= MAX_ROWS {
            report.truncated = true;
            break;
        }
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match parse_row(delimiter, line) {
            Ok(entry) => {
                if seen.insert(entry.word.clone()) {
                    report.entries.push(entry);
                } else {
                    record(&mut report, index + 1, WordbookImportIssue::Duplicate);
                }
            }
            Err(issue) => record(&mut report, index + 1, issue),
        }
    }

    if report.entries.is_empty() {
        return Err(WordbookImportError::NoUsableRows);
    }
    Ok(report)
}

fn record(report: &mut WordbookImportReport, line: usize, issue: WordbookImportIssue) {
    report.failed += 1;
    if report.first_failures.len() < REPORTED_FAILURES {
        report
            .first_failures
            .push(WordbookImportFailure { line, issue });
    }
}

fn parse_row(delimiter: Delimiter, line: &str) -> Result<WordbookEntry, WordbookImportIssue> {
    let columns = split_columns(delimiter, line);
    let (word, phonetic, meaning) = match columns.len() {
        2 => (columns[0].as_str(), "", columns[1].as_str()),
        3 => (
            columns[0].as_str(),
            columns[1].as_str(),
            columns[2].as_str(),
        ),
        _ => return Err(WordbookImportIssue::ColumnCount),
    };

    let entry = WordbookEntry {
        word: word.trim().to_owned(),
        phonetic: phonetic.trim().to_owned(),
        meaning: meaning.trim().to_owned(),
    };

    // Report the specific reason rather than one blanket failure: "line 12 has no meaning" is
    // something a user can fix, and "line 12 is bad" is not.
    if entry.word.is_empty() {
        return Err(WordbookImportIssue::EmptyWord);
    }
    if entry.word.chars().count() > wordbook::MAX_WORD_CHARS {
        return Err(WordbookImportIssue::WordTooLong);
    }
    if entry.meaning.is_empty() {
        return Err(WordbookImportIssue::EmptyMeaning);
    }
    if entry.meaning.chars().count() > wordbook::MAX_MEANING_CHARS {
        return Err(WordbookImportIssue::MeaningTooLong);
    }
    if entry.phonetic.chars().count() > wordbook::MAX_PHONETIC_CHARS {
        return Err(WordbookImportIssue::PhoneticTooLong);
    }
    // A tab survives the envelope check because it is a legitimate delimiter, so a tab inside a
    // quoted comma-delimited field reaches here. `is_valid` is the one predicate that decides
    // what a card may hold; asking it here keeps the importer and the store in agreement.
    if !entry.is_valid() {
        return Err(WordbookImportIssue::ControlCharacter);
    }
    Ok(entry)
}

/// Split one line into columns.
///
/// Comma-delimited files get RFC 4180 double quoting, because a gloss routinely contains a comma
/// (`adj. 无处不在的, 普遍存在的`) and every tool that exports one quotes the field. Without it the
/// commonest real file would parse as four columns and be rejected wholesale. A doubled quote
/// inside a quoted field is one literal quote. Tab-delimited files need none of this: a gloss does
/// not contain a tab, and the exporters that emit tabs do not quote.
fn split_columns(delimiter: Delimiter, line: &str) -> Vec<String> {
    let separator = match delimiter {
        Delimiter::Tab => '\t',
        Delimiter::Comma => ',',
    };
    if delimiter == Delimiter::Tab {
        return line.split(separator).map(str::to_owned).collect();
    }

    let mut columns = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' if quoted => {
                if characters.peek() == Some(&'"') {
                    characters.next();
                    current.push('"');
                } else {
                    quoted = false;
                }
            }
            '"' if current.trim().is_empty() => {
                // An opening quote only counts at the start of a field; a quote in the middle of
                // unquoted text is an apostrophe-style character, not a delimiter.
                current.clear();
                quoted = true;
            }
            c if c == separator && !quoted => columns.push(std::mem::take(&mut current)),
            c => current.push(c),
        }
    }
    columns.push(current);
    columns
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: usize = 64 * 1024;

    #[test]
    fn reads_two_and_three_column_comma_files() {
        let report = parse(
            "ubiquitous,/juːˈbɪkwɪtəs/,adj. 无处不在的\nephemeral,adj. 短暂的\n",
            LIMIT,
        )
        .unwrap();
        assert_eq!(report.failed, 0);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].word, "ubiquitous");
        assert_eq!(report.entries[0].phonetic, "/juːˈbɪkwɪtəs/");
        assert_eq!(report.entries[0].meaning, "adj. 无处不在的");
        assert_eq!(
            report.entries[1].phonetic, "",
            "two columns means no phonetic"
        );
        assert_eq!(report.entries[1].meaning, "adj. 短暂的");
    }

    #[test]
    fn reads_tab_separated_files() {
        let report = parse("ubiquitous\t/juː/\tadj. 无处不在的\n", LIMIT).unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].meaning, "adj. 无处不在的");
    }

    #[test]
    fn a_quoted_gloss_keeps_its_commas() {
        // The commonest real file. Without RFC 4180 quoting this row is four columns and the whole
        // file is rejected.
        let report = parse("ubiquitous,/juː/,\"adj. 无处不在的, 普遍存在的\"\n", LIMIT).unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].meaning, "adj. 无处不在的, 普遍存在的");
    }

    #[test]
    fn a_doubled_quote_inside_a_quoted_field_is_one_quote() {
        let report = parse("quote,\"say \"\"hello\"\" loudly\"\n", LIMIT).unwrap();
        assert_eq!(report.entries[0].meaning, "say \"hello\" loudly");
    }

    #[test]
    fn a_leading_byte_order_mark_is_not_part_of_the_first_word() {
        let report = parse("\u{feff}ubiquitous,adj. 无处不在的\n", LIMIT).unwrap();
        assert_eq!(
            report.entries[0].word, "ubiquitous",
            "a BOM left in place stores a word that can never be matched"
        );
    }

    #[test]
    fn blank_lines_and_comments_are_skipped_without_counting_as_failures() {
        let report = parse(
            "# CET-4\n\nubiquitous,adj. 无处不在的\n\n# end\nephemeral,adj. 短暂的\n",
            LIMIT,
        )
        .unwrap();
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn one_bad_row_does_not_reject_the_file_and_is_reported_by_line() {
        let report = parse(
            "ubiquitous,adj. 无处不在的\nonlyoneColumn\n,adj. 空词\nephemeral,\nlast,adj. 好的\n",
            LIMIT,
        )
        .unwrap();
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.failed, 3);
        assert_eq!(
            report.first_failures,
            vec![
                WordbookImportFailure {
                    line: 2,
                    issue: WordbookImportIssue::ColumnCount
                },
                WordbookImportFailure {
                    line: 3,
                    issue: WordbookImportIssue::EmptyWord
                },
                WordbookImportFailure {
                    line: 4,
                    issue: WordbookImportIssue::EmptyMeaning
                },
            ]
        );
    }

    #[test]
    fn only_the_first_few_failures_are_reported() {
        let mut text = String::new();
        for _ in 0..20 {
            text.push_str("badrow\n");
        }
        text.push_str("good,adj. 好的\n");
        let report = parse(&text, LIMIT).unwrap();
        assert_eq!(report.failed, 20);
        assert_eq!(report.first_failures.len(), REPORTED_FAILURES);
    }

    #[test]
    fn a_repeated_headword_is_reported_rather_than_silently_overwritten() {
        let report = parse(
            "ubiquitous,adj. 无处不在的\nubiquitous,adj. 另一个释义\n",
            LIMIT,
        )
        .unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].meaning, "adj. 无处不在的");
        assert_eq!(
            report.first_failures,
            vec![WordbookImportFailure {
                line: 2,
                issue: WordbookImportIssue::Duplicate
            }]
        );
    }

    #[test]
    fn oversized_fields_are_named_individually() {
        let long_word = "a".repeat(wordbook::MAX_WORD_CHARS + 1);
        let long_meaning = "词".repeat(wordbook::MAX_MEANING_CHARS + 1);
        let long_phonetic = "iː".repeat(wordbook::MAX_PHONETIC_CHARS);
        let text = format!(
            "{long_word},adj. 好的\ngood,{long_meaning}\ngood2,{long_phonetic},adj. 好的\nok,adj. 好的\n"
        );
        let report = parse(&text, LIMIT).unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(
            report
                .first_failures
                .iter()
                .map(|failure| failure.issue)
                .collect::<Vec<_>>(),
            vec![
                WordbookImportIssue::WordTooLong,
                WordbookImportIssue::MeaningTooLong,
                WordbookImportIssue::PhoneticTooLong,
            ]
        );
    }

    #[test]
    fn the_envelope_is_refused_when_it_is_empty_oversized_or_binary() {
        assert_eq!(parse("", LIMIT), Err(WordbookImportError::Empty));
        assert_eq!(
            parse("ubiquitous,adj. 无处不在的\n", 4),
            Err(WordbookImportError::TooLarge)
        );
        assert_eq!(
            parse("ubiquitous,adj.\0无处不在的\n", LIMIT),
            Err(WordbookImportError::ControlCharacters)
        );
        assert_eq!(
            parse("# only a comment\n\n", LIMIT),
            Err(WordbookImportError::NoUsableRows)
        );
    }

    #[test]
    fn rows_past_the_ceiling_are_not_examined_and_the_report_says_so() {
        let mut text = String::new();
        for index in 0..(MAX_ROWS + 10) {
            text.push_str(&format!("word{index},adj. 好的\n"));
        }
        let report = parse(&text, 64 * 1024 * 1024).unwrap();
        assert_eq!(report.entries.len(), MAX_ROWS);
        assert!(report.truncated);
    }

    #[test]
    fn a_parsed_book_takes_its_identity_from_the_host_not_the_file() {
        let (book, report) = parse_wordbook(
            "user-cet-4",
            "我的四级词表",
            "ubiquitous,adj. 无处不在的\n",
            LIMIT,
        )
        .unwrap();
        assert_eq!(book.id, "user-cet-4");
        assert_eq!(book.name, "我的四级词表");
        assert_eq!(report.entries.len(), 1);
        assert!(book.is_valid());
    }
}
