//! Parsing for user-supplied dictionary files.
//!
//! A realistic exported dictionary contains the occasional unusable row. The
//! earlier parser rejected the whole file on the first one and reported a single
//! opaque string, so a user had to find and fix that row blind. This module
//! skips unusable rows, counts them, and names the first few by line number.
//!
//! Every bound the previous parser enforced is preserved: this reads
//! user-supplied files, so the key alphabets, length limits and control
//! character rules are load-bearing, not cosmetic.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Rows accepted from one file. Beyond this the remainder is reported as
/// truncated rather than silently dropped or rejected wholesale, because each
/// import holds the dictionary maintenance lock.
pub const MAX_ENTRIES: usize = 1000;
const MAX_VALUE_BYTES: usize = 1024;
/// The longest quick phrase, in UTF-16 units.
///
/// Editors count UTF-16 units, and so does the Windows candidate pipe: this is
/// `FanyImePipeLimits::CandidateTextMaxLength` from the Engine's own
/// `contracts/ipc_protocol_limits.h`, which is the field the phrase is finally
/// written into. A phrase longer than the field cannot be delivered, so it is
/// refused where it is entered rather than truncated where it is used.
///
/// One constant for every caller, checked against the contract by
/// `scripts/test-quick-phrase-limit.py`. It was four separate literals across
/// three crates, none of them attached to the header that decides the value.
pub const MAX_QUICK_PHRASE_UTF16: usize = 199;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    Pinyin,
    Wubi,
    QuickPhrase,
    English,
}

impl ImportKind {
    fn key_limit(self) -> usize {
        match self {
            ImportKind::Pinyin => 256,
            ImportKind::Wubi => 4,
            ImportKind::QuickPhrase => 32,
            ImportKind::English => 64,
        }
    }

    fn key_is_well_formed(self, key: &str, format: ImportFormat) -> bool {
        match self {
            ImportKind::Pinyin => {
                super::pinyin_code_is_well_formed(key, format == ImportFormat::Rime)
            }
            ImportKind::Wubi => super::wubi_code_is_well_formed(key),
            ImportKind::QuickPhrase => super::quick_phrase_code_is_well_formed(key),
            ImportKind::English => super::english_code_is_well_formed(key),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    Standard,
    Windows,
    Rime,
}

impl ImportFormat {
    /// The same layout with the two columns exchanged, or `None` for a format that has only one.
    ///
    /// Rime files are word-first by definition - the code is the second column of a `dict.yaml`
    /// row - so there is no other order to try.
    fn flipped(self) -> Option<Self> {
        match self {
            ImportFormat::Standard => Some(ImportFormat::Windows),
            ImportFormat::Windows => Some(ImportFormat::Standard),
            ImportFormat::Rime => None,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(ImportFormat::Standard),
            "windows" => Some(ImportFormat::Windows),
            "rime" => Some(ImportFormat::Rime),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportEntry {
    pub key: String,
    pub value: String,
    pub weight: i64,
    /// 1-based line in the submitted text. Carried so a row the Engine refuses
    /// later can be reported by line, exactly as a parse failure is.
    #[serde(default)]
    pub line: usize,
}

/// Why one row was skipped. Deliberately describes the shape of the problem and
/// never echoes the row, which may be private user text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportIssue {
    ColumnCount,
    EmptyKey,
    KeyTooLong,
    KeyAlphabet,
    Pinyin,
    EmptyValue,
    ValueTooLong,
    QuickPhraseTooLong,
    Weight,
    /// Parsed cleanly, but the Engine refused it - typically a jianpin code, or
    /// a syllable count that does not match the number of Han characters.
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportFailure {
    /// 1-based line number in the submitted text, so a user can find the row.
    pub line: usize,
    pub issue: ImportIssue,
}

/// The envelope was unusable, so no row was examined.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ImportError {
    #[error("dictionary import format is not supported")]
    UnsupportedFormat,
    #[error("dictionary import is empty")]
    Empty,
    #[error("dictionary import is too large")]
    TooLarge,
    #[error("dictionary import contains unsupported control characters")]
    ControlCharacters,
    #[error("dictionary import contains no usable rows")]
    NoUsableRows,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    pub entries: Vec<ImportEntry>,
    /// Rows examined and rejected.
    pub failed: usize,
    /// The first few failures, for a message a user can act on.
    pub first_failures: Vec<ImportFailure>,
    /// Rows beyond [`MAX_ENTRIES`] were not examined.
    pub truncated: bool,
    /// The file was read with the columns the other way round from the format that was asked for.
    ///
    /// The two coded layouts differ only in which column comes first, and which one a file uses is
    /// a property of the tool that wrote it rather than of the platform it ran on: the reference
    /// exports pinyin and wubi word-first and English and quick phrases code-first, from the same
    /// settings page. Asking the user to know that - and telling them "no usable rows" when they
    /// guess wrong, for a file that is perfectly good - is a puzzle with one answer. So a file
    /// that yields nothing in the order requested is read once more the other way, and this says
    /// so, because a silent reinterpretation of which column is the word would be worse than the
    /// puzzle.
    #[serde(default)]
    pub swapped: bool,
}

pub const REPORTED_FAILURES: usize = 5;

impl ImportReport {
    /// Fold rows the Engine refused into this report.
    ///
    /// The parser only checks the key alphabet and length; the Engine also
    /// demands complete pinyin syllables and one syllable per Han character,
    /// so ordinary real files contain rows that parse but are then refused.
    /// Those belong in the same counters the user already sees, named by line,
    /// rather than aborting an import that has already written part of itself.
    pub fn record_rejected(&mut self, lines: &[usize]) {
        self.failed += lines.len();
        for line in lines {
            if self.first_failures.len() >= REPORTED_FAILURES {
                break;
            }
            self.first_failures.push(ImportFailure {
                line: *line,
                issue: ImportIssue::Rejected,
            });
        }
        // Parse failures are reported in line order; keep the combined list in
        // line order too, so "first appeared at line N" stays true.
        self.first_failures.sort_by_key(|failure| failure.line);
    }
}

/// Parse a user-supplied dictionary file.
///
/// Returns `Err` only when the envelope itself is unusable. Individual bad rows
/// are skipped and counted, so one stray line cannot reject an entire file.
pub fn parse(
    kind: ImportKind,
    format: &str,
    text: &str,
    max_bytes: usize,
) -> Result<ImportReport, ImportError> {
    let format = ImportFormat::parse(format).ok_or(ImportError::UnsupportedFormat)?;
    // A leading U+FEFF is a byte-order mark, not content - and every Windows tool that writes a
    // dictionary puts one there, including this application's own exporter. It is stripped here
    // rather than left to callers because `str::trim` does not remove it: U+FEFF has not been a
    // White_Space character for a long time, so it survives into the first row and lands wherever
    // that format puts column one. In the word-first formats that is the word, which then parses
    // cleanly and is stored with an invisible prefix that can never match what a user types; in
    // the Windows format it is the code, which is refused as a bad alphabet and reported as one
    // failed line with nothing a reader could act on. Both are silent in their own way, and one
    // of them corrupts data.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.is_empty() {
        return Err(ImportError::Empty);
    }
    if text.len() > max_bytes {
        return Err(ImportError::TooLarge);
    }
    // A disallowed control byte means the file is not the text format claimed; examining rows
    // from it would be guesswork.
    if crate::text::has_disallowed_control(text) {
        return Err(ImportError::ControlCharacters);
    }

    let report = parse_rows(kind, format, text);
    if !report.entries.is_empty() {
        return Ok(report);
    }
    // Nothing was usable. Before refusing the file, read it in the other column order: see
    // `ImportReport::swapped` for why a user cannot be expected to know which one their file is.
    if let Some(other) = format.flipped() {
        let mut retry = parse_rows(kind, other, text);
        if !retry.entries.is_empty() {
            retry.swapped = true;
            return Ok(retry);
        }
    }
    Err(ImportError::NoUsableRows)
}

/// Every row of the text in one column order, with unusable ones counted rather than fatal.
///
/// Unlike [`parse`] this never refuses the text: it neither checks the envelope nor tries the other column order. A caller that already had the host refuse part of a larger file uses it to name that part's rows the way a single request would have.
pub fn parse_rows(kind: ImportKind, format: ImportFormat, text: &str) -> ImportReport {
    let mut report = ImportReport {
        entries: Vec::new(),
        failed: 0,
        first_failures: Vec::new(),
        truncated: false,
        swapped: false,
    };
    let mut in_yaml_header = false;
    for (index, line) in text.lines().enumerate() {
        if report.entries.len() >= MAX_ENTRIES {
            report.truncated = true;
            break;
        }
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if format == ImportFormat::Rime {
            if trimmed == "---" {
                in_yaml_header = true;
                continue;
            }
            if trimmed == "..." {
                in_yaml_header = false;
                continue;
            }
            if in_yaml_header {
                continue;
            }
        }
        match parse_row(kind, format, line) {
            Ok(mut entry) => {
                entry.line = index + 1;
                report.entries.push(entry);
            }
            Err(issue) => {
                report.failed += 1;
                if report.first_failures.len() < REPORTED_FAILURES {
                    report.first_failures.push(ImportFailure {
                        line: index + 1,
                        issue,
                    });
                }
            }
        }
    }
    report
}

fn parse_row(
    kind: ImportKind,
    format: ImportFormat,
    line: &str,
) -> Result<ImportEntry, ImportIssue> {
    let columns: Vec<_> = line.split('\t').collect();
    if !(2..=3).contains(&columns.len()) {
        return Err(ImportIssue::ColumnCount);
    }
    // Windows exports put the code first; the other formats put the word first.
    let (word, key) = if format == ImportFormat::Windows {
        (columns[1].trim(), columns[0].trim())
    } else {
        (columns[0].trim(), columns[1].trim())
    };
    let key = key.to_ascii_lowercase();
    let weight = match columns.get(2).map(|value| value.trim()) {
        None | Some("") => 10000,
        // Rime carries metadata such as `c=3` in the third column.
        Some(value) if format == ImportFormat::Rime && value.contains('=') => 10000,
        Some(value) => value.parse::<i64>().map_err(|_| ImportIssue::Weight)?,
    };

    if key.is_empty() {
        return Err(ImportIssue::EmptyKey);
    }
    if key.len() > kind.key_limit() {
        return Err(ImportIssue::KeyTooLong);
    }
    if !kind.key_is_well_formed(&key, format) {
        return Err(ImportIssue::KeyAlphabet);
    }
    if word.is_empty() {
        return Err(ImportIssue::EmptyValue);
    }
    if !crate::text::is_bounded_text(word, MAX_VALUE_BYTES) {
        return Err(ImportIssue::ValueTooLong);
    }
    if kind == ImportKind::QuickPhrase && !crate::is_bounded_utf16(word, MAX_QUICK_PHRASE_UTF16) {
        return Err(ImportIssue::QuickPhraseTooLong);
    }
    if weight < 0 {
        return Err(ImportIssue::Weight);
    }
    // The Engine stores nothing below 1, and a file that says 0 is ordinary: it is what the
    // reference writes for an imported English row whose line carried no weight. Keeping the row
    // at the Engine's floor loses a rank difference of one; refusing it loses the word.
    let weight = weight.max(1);
    Ok(ImportEntry {
        key,
        value: word.to_owned(),
        weight,
        line: 0, // Filled in by the caller, which knows the line number.
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: usize = 1 << 20;

    fn parse_ok(kind: ImportKind, format: &str, text: &str) -> ImportReport {
        parse(kind, format, text, LIMIT).expect("envelope is usable")
    }

    #[test]
    fn standard_and_windows_rows_use_opposite_column_order() {
        let standard = parse_ok(
            ImportKind::Pinyin,
            "standard",
            "你好\tni'hao\t7\n# comment\n西安\txi'an\n",
        );
        assert_eq!(standard.entries.len(), 2);
        assert_eq!(standard.entries[0].key, "ni'hao");
        assert_eq!(standard.entries[0].value, "你好");
        assert_eq!(standard.entries[0].weight, 7);
        // An omitted weight uses the shared default.
        assert_eq!(standard.entries[1].weight, 10000);
        assert_eq!(standard.failed, 0);

        let windows = parse_ok(ImportKind::Wubi, "windows", "wq\t你好\t9\n");
        assert_eq!(windows.entries[0].key, "wq");
        assert_eq!(windows.entries[0].value, "你好");
    }

    #[test]
    fn one_unusable_row_no_longer_rejects_the_whole_file() {
        // The previous parser returned Err here and imported nothing.
        let report = parse_ok(
            ImportKind::Pinyin,
            "standard",
            "你好\tni'hao\n坏行没有制表符\n世界\tshi'jie\n再见\tZAI JIAN\n",
        );
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.failed, 2);
        assert_eq!(
            report.first_failures,
            vec![
                ImportFailure {
                    line: 2,
                    issue: ImportIssue::ColumnCount
                },
                ImportFailure {
                    line: 4,
                    issue: ImportIssue::KeyAlphabet
                },
            ]
        );
        assert!(!report.truncated);
    }

    // A file written the other way round is read anyway. The reference's own settings page exports
    // pinyin and wubi word-first and English and quick phrases code-first, so "which order is my
    // file" is a question its users cannot answer from where the file came from.
    #[test]
    fn a_file_in_the_other_column_order_is_read_rather_than_refused() {
        // Asked for word-first, given code-first.
        let report = parse_ok(ImportKind::Wubi, "standard", "ggg\t三\t7\nhhh\t四\n");
        assert!(report.swapped);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].key, "ggg");
        assert_eq!(report.entries[0].value, "三");
        assert_eq!(report.entries[1].weight, 10000);

        // And the other way: asked for code-first, given word-first.
        let report = parse_ok(ImportKind::Wubi, "windows", "三\tggg\t7\n");
        assert!(report.swapped);
        assert_eq!(report.entries[0].key, "ggg");

        // A file that is already in the order asked for is not flagged, and the retry never runs.
        let report = parse_ok(ImportKind::Wubi, "standard", "三\tggg\t7\n");
        assert!(!report.swapped);
        assert_eq!(report.entries[0].key, "ggg");

        // A file that is unusable in both orders is still unusable, and says so once.
        assert_eq!(
            parse(ImportKind::Wubi, "standard", "三\t四\n", LIMIT),
            Err(ImportError::NoUsableRows)
        );
    }

    // A file that writes 0 in the weight column is ordinary - it is what the reference produces
    // for an English row whose line carried no weight - and the Engine stores nothing below 1.
    // Keeping the row at the floor costs a rank difference of one; refusing it costs the word.
    #[test]
    fn a_zero_weight_row_is_kept_at_the_floor_rather_than_refused() {
        let report = parse_ok(
            ImportKind::QuickPhrase,
            "standard",
            "在家等\tzjd\t0\n企鹅\tqq\t5\n",
        );
        assert_eq!(report.failed, 0);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].weight, 1);
        assert_eq!(report.entries[1].weight, 5);
        // A negative weight is still a malformed file rather than a small number.
        let refused = parse_ok(
            ImportKind::QuickPhrase,
            "standard",
            "在家等\tzjd\t-1\n企鹅\tqq\t5\n",
        );
        assert_eq!(refused.failed, 1);
        assert_eq!(refused.entries.len(), 1);
    }

    #[test]
    fn rime_skips_its_yaml_header_and_metadata_weights() {
        let report = parse_ok(
            ImportKind::Pinyin,
            "rime",
            "---\nname: demo\n...\n你好\tni hao\tc=3\n世界\tshi jie\n",
        );
        assert_eq!(report.entries.len(), 2);
        // Metadata in the weight column falls back to the default.
        assert_eq!(report.entries[0].weight, 10000);
        // Rime keys may contain spaces; the other formats may not.
        assert_eq!(report.entries[0].key, "ni hao");
        assert_eq!(report.failed, 0);

        let standard = parse(ImportKind::Pinyin, "standard", "你好\tni hao\n", LIMIT);
        assert_eq!(standard, Err(ImportError::NoUsableRows));
    }

    #[test]
    fn every_key_alphabet_and_length_bound_is_preserved() {
        // Wubi keys are at most four lowercase letters.
        assert_eq!(
            parse(ImportKind::Wubi, "windows", "abcde\t你好\n", LIMIT),
            Err(ImportError::NoUsableRows)
        );
        // A digit is not part of an English code. Both columns have to be unusable as a key,
        // because a file whose columns are the other way round is read that way rather than
        // refused.
        assert_eq!(
            parse(ImportKind::English, "standard", "h3llo\th3llo\n", LIMIT),
            Err(ImportError::NoUsableRows)
        );
        // A hyphen and an apostrophe are, and the word beside the code is its own text: `dont`
        // types out `don't`, which is the row the reference's own importer exists to accept.
        assert_eq!(
            parse_ok(
                ImportKind::English,
                "windows",
                "dont\tdon't\ne-mail\te-mail\n"
            )
            .entries
            .iter()
            .map(|entry| (entry.key.as_str(), entry.value.as_str()))
            .collect::<Vec<_>>(),
            vec![("dont", "don't"), ("e-mail", "e-mail")]
        );
        // Quick phrase keys are letters only, as in the reference.
        assert_eq!(
            parse(ImportKind::QuickPhrase, "standard", "你好\tnh1\n", LIMIT),
            Err(ImportError::NoUsableRows)
        );
        // An uppercase quick phrase key is folded, not refused.
        assert_eq!(
            parse_ok(ImportKind::QuickPhrase, "standard", "你好\tNH\n")
                .entries
                .iter()
                .map(|entry| entry.key.as_str())
                .collect::<Vec<_>>(),
            vec!["nh"]
        );
        // Negative weights are refused.
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "你好\tni\t-1\n", LIMIT),
            Err(ImportError::NoUsableRows)
        );
        // A non-numeric weight is refused.
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "你好\tni\tmuch\n", LIMIT),
            Err(ImportError::NoUsableRows)
        );
        // Quick phrases are bounded in UTF-16 units.
        let long = "字".repeat(MAX_QUICK_PHRASE_UTF16 + 1);
        assert_eq!(
            parse(
                ImportKind::QuickPhrase,
                "standard",
                &format!("{long}\tnh\n"),
                LIMIT
            ),
            Err(ImportError::NoUsableRows)
        );
        let allowed = "字".repeat(MAX_QUICK_PHRASE_UTF16);
        assert_eq!(
            parse_ok(
                ImportKind::QuickPhrase,
                "standard",
                &format!("{allowed}\tnh\n")
            )
            .entries
            .len(),
            1
        );
    }

    #[test]
    fn envelope_problems_are_distinguished_from_row_problems() {
        assert_eq!(
            parse(ImportKind::Pinyin, "hans", "你好\tni'hao\n", LIMIT),
            Err(ImportError::UnsupportedFormat)
        );
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "", LIMIT),
            Err(ImportError::Empty)
        );
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "你好\tni'hao\n", 4),
            Err(ImportError::TooLarge)
        );
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "你好\tni\u{7}hao\n", LIMIT),
            Err(ImportError::ControlCharacters)
        );
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "\0", LIMIT),
            Err(ImportError::ControlCharacters)
        );
        // A file of comments examines no rows at all.
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "# only comments\n", LIMIT),
            Err(ImportError::NoUsableRows)
        );
    }

    #[test]
    fn rows_beyond_the_cap_are_reported_rather_than_rejected() {
        // The previous parser failed the entire import at this point.
        let mut text = String::new();
        for index in 0..(MAX_ENTRIES + 10) {
            text.push_str(&format!("词{index}\tni'hao\n"));
        }
        let report = parse_ok(ImportKind::Pinyin, "standard", &text);
        assert_eq!(report.entries.len(), MAX_ENTRIES);
        assert!(report.truncated);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn a_page_holds_only_rows_of_the_requested_kind() {
        // The defect: a user with more than a page of pinyin words who selects
        // 五笔 saw an empty page 1, because the client filtered a page it had
        // already fetched instead of the server selecting the right rows.
        let rows: Vec<(bool, &str)> = (0..250).map(|index| (index % 50 == 0, "wq")).collect();
        let mut selector = PageSelector::new(0, 5);
        let mut taken = 0;
        for (is_wubi, key) in &rows {
            if dictionary_row_matches(*is_wubi, ImportKind::Wubi, key, "") && selector.accept() {
                taken += 1;
            }
        }
        assert_eq!(taken, 5);
        assert!(selector.full());
        assert_eq!(selector.taken(), 5);
    }

    #[test]
    fn the_offset_counts_matching_rows_not_scanned_ones() {
        let mut selector = PageSelector::new(2, 2);
        let accepted: Vec<bool> = (0..6).map(|_| selector.accept()).collect();
        // Skip two matches, take two, refuse the rest.
        assert_eq!(accepted, vec![false, false, true, true, false, false]);
        assert_eq!(selector.taken(), 2);
        assert!(selector.full());
    }

    #[test]
    fn a_page_shorter_than_the_limit_is_not_full() {
        let mut selector = PageSelector::new(0, 10);
        for _ in 0..3 {
            assert!(selector.accept());
        }
        assert!(!selector.full());
        assert_eq!(selector.taken(), 3);
    }

    #[test]
    fn the_code_prefix_is_matched_case_insensitively() {
        assert!(dictionary_row_matches(true, ImportKind::Wubi, "wq", "w"));
        assert!(dictionary_row_matches(true, ImportKind::Wubi, "WQ", "w"));
        assert!(dictionary_row_matches(true, ImportKind::Wubi, "wq", "WQ"));
        assert!(dictionary_row_matches(true, ImportKind::Wubi, " wq ", "wq"));
        // A prefix, not a substring: the user is typing a code from the start.
        assert!(!dictionary_row_matches(true, ImportKind::Wubi, "awq", "wq"));
        // Longer than the key cannot match.
        assert!(!dictionary_row_matches(true, ImportKind::Wubi, "wq", "wqx"));
        // An empty prefix matches everything of the right kind.
        assert!(dictionary_row_matches(
            true,
            ImportKind::Wubi,
            "anything",
            ""
        ));
        // The kind still gates it, whatever the prefix.
        assert!(!dictionary_row_matches(false, ImportKind::Wubi, "wq", ""));
        assert!(!dictionary_row_matches(false, ImportKind::Wubi, "wq", "wq"));
    }

    #[test]
    fn a_pinyin_prefix_ignores_syllable_separators() {
        let pinyin =
            |key: &str, prefix: &str| dictionary_row_matches(true, ImportKind::Pinyin, key, prefix);
        // Stored keys are separated; users type without separators, with spaces, or stop inside a syllable.
        for prefix in ["nihao", "nih", "ni'hao", "ni hao", "NiHao", "n"] {
            assert!(pinyin("ni'hao", prefix), "{prefix:?}");
        }
        // A key stored with spaces is compared the same way.
        assert!(pinyin("ni hao", "nihao"));
        // Still a prefix from the start of the key, and never longer than it.
        assert!(!pinyin("ni'hao", "hao"));
        assert!(!pinyin("ni'hao", "nihaoma"));
        assert!(!pinyin("ni'hao", "nhao"));
        // The accepted overmatch: without separators a prefix can cross a syllable boundary.
        assert!(pinyin("xi'an", "xian"));
        assert!(pinyin("xian", "xi'an"));
        // The kind still gates it.
        assert!(!dictionary_row_matches(
            false,
            ImportKind::Pinyin,
            "ni'hao",
            "nihao"
        ));
        // Other kinds keep separators significant: an apostrophe is part of an English code.
        assert!(!dictionary_row_matches(
            true,
            ImportKind::English,
            "don't",
            "dont"
        ));
    }

    #[test]
    fn engine_rejections_join_the_same_report() {
        let mut report = parse_ok(
            ImportKind::Pinyin,
            "standard",
            "你好	ni'hao
坏行没有制表符
世界	shi'jie
",
        );
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.failed, 1);
        // Every accepted row knows which line it came from, so a row the Engine
        // refuses later can be named the same way a parse failure is.
        assert_eq!(report.entries[0].line, 1);
        assert_eq!(report.entries[1].line, 3);

        report.record_rejected(&[3]);
        assert_eq!(report.failed, 2);
        // Both kinds of failure sit in one list, in line order.
        assert_eq!(
            report.first_failures,
            vec![
                ImportFailure {
                    line: 2,
                    issue: ImportIssue::ColumnCount
                },
                ImportFailure {
                    line: 3,
                    issue: ImportIssue::Rejected
                },
            ]
        );
    }

    #[test]
    fn rejections_respect_the_reporting_cap() {
        let mut report = parse_ok(
            ImportKind::Pinyin,
            "standard",
            "你好	ni'hao
",
        );
        let lines: Vec<usize> = (1..=REPORTED_FAILURES + 4).collect();
        report.record_rejected(&lines);
        // Every rejection is counted...
        assert_eq!(report.failed, REPORTED_FAILURES + 4);
        // ...but only the first few are named, as with parse failures.
        assert_eq!(report.first_failures.len(), REPORTED_FAILURES);
        assert!(report
            .first_failures
            .iter()
            .all(|failure| failure.issue == ImportIssue::Rejected));
    }

    #[test]
    fn only_the_first_few_failures_are_named() {
        let mut text = String::from("你好\tni'hao\n");
        for _ in 0..(REPORTED_FAILURES + 3) {
            text.push_str("没有制表符\n");
        }
        let report = parse_ok(ImportKind::Pinyin, "standard", &text);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.failed, REPORTED_FAILURES + 3);
        assert_eq!(report.first_failures.len(), REPORTED_FAILURES);
    }

    #[test]
    fn the_code_prefix_is_case_insensitive_and_starts_at_the_key() {
        assert!(dictionary_row_matches(true, ImportKind::Wubi, "wq", "w"));
        assert!(dictionary_row_matches(true, ImportKind::Wubi, "WQ", "w"));
        assert!(dictionary_row_matches(true, ImportKind::Wubi, "wq", "WQ"));
        assert!(dictionary_row_matches(true, ImportKind::Wubi, " wq ", "wq"));
        assert!(!dictionary_row_matches(true, ImportKind::Wubi, "awq", "wq"));
        assert!(!dictionary_row_matches(true, ImportKind::Wubi, "wq", "wqx"));
        assert!(dictionary_row_matches(
            true,
            ImportKind::Wubi,
            "anything",
            ""
        ));
        assert!(!dictionary_row_matches(false, ImportKind::Wubi, "wq", ""));
        assert!(!dictionary_row_matches(false, ImportKind::Wubi, "wq", "wq"));
    }

    #[test]
    fn a_byte_order_mark_belongs_to_the_envelope_and_not_to_the_first_row() {
        // Exactly what this application's own exporter writes, and what every Windows text tool
        // writes: U+FEFF, then the rows.
        let standard = parse_ok(
            ImportKind::Pinyin,
            "standard",
            "\u{feff}你好\tni'hao\t7\n世界\tshi'jie\n",
        );
        assert_eq!(standard.failed, 0);
        // Without stripping this reads back as "\u{feff}你好": it parses, it is stored, and it
        // can never match anything the user types, with nothing reported.
        assert_eq!(standard.entries[0].value, "你好");
        assert_eq!(standard.entries[0].key, "ni'hao");
        assert_eq!(standard.entries.len(), 2);

        // In the code-first format the mark lands on the code instead, where it is refused for
        // its alphabet - one failed line and no way for a reader to tell why.
        let windows = parse_ok(
            ImportKind::Pinyin,
            "windows",
            "\u{feff}ni'hao\t你好\t7\nshi'jie\t世界\n",
        );
        assert_eq!(windows.failed, 0);
        assert_eq!(windows.entries.len(), 2);
        assert_eq!(windows.entries[0].key, "ni'hao");

        // Only the leading one is an envelope marker. A U+FEFF anywhere else is content the row
        // rules judge for themselves, and the row it appears in still fails on its own terms.
        let inner = parse_ok(
            ImportKind::Pinyin,
            "standard",
            "你好\tni'hao\n世\u{feff}界\tshi'jie\n",
        );
        assert_eq!(inner.entries.len(), 2);
        assert_eq!(inner.entries[1].value, "世\u{feff}界");

        // A file that is nothing but a mark has no rows, and says so as an empty file rather than
        // as one with nothing usable in it.
        assert_eq!(
            parse(ImportKind::Pinyin, "standard", "\u{feff}", LIMIT),
            Err(ImportError::Empty)
        );
    }
}

/// Server-side paging for the dictionary browser.
///
/// The Engine pages the whole user store in one sequence, so the client used to
/// ask for 100 rows and then drop everything that was not the selected kind.
/// With more than a page of pinyin words, selecting 五笔 showed an empty list on
/// page 1 even though wubi entries existed, and the status line counted the
/// filtered rows against the unfiltered page. Filtering here instead means a
/// page always holds `limit` rows of what the user actually asked for.
#[derive(Debug, Clone, Copy)]
pub struct PageSelector {
    offset: usize,
    limit: usize,
    skipped: usize,
    taken: usize,
}

impl PageSelector {
    pub fn new(offset: usize, limit: usize) -> Self {
        Self {
            offset,
            limit,
            skipped: 0,
            taken: 0,
        }
    }

    /// Does a row that already matched the filter belong on this page?
    ///
    /// Call once per matching row, in order. Returns false while skipping to
    /// `offset`, then true until `limit` rows have been taken.
    pub fn accept(&mut self) -> bool {
        if self.skipped < self.offset {
            self.skipped += 1;
            return false;
        }
        if self.taken >= self.limit {
            return false;
        }
        self.taken += 1;
        true
    }

    /// True once the page is full. A further match means there is more to show.
    pub fn full(&self) -> bool {
        self.taken >= self.limit
    }

    pub fn taken(&self) -> usize {
        self.taken
    }
}

/// Does this row belong to the requested kind and code prefix?
///
/// The prefix is compared case-insensitively over ASCII because every code alphabet here is ASCII and users type codes in either case.
///
/// A pinyin key is stored with its syllables separated (`ni'hao`), but users search the way they type: `nihao`, `ni hao` or a prefix that ends inside a syllable such as `nih`. Separators are therefore ignored on both sides for pinyin rows, the way the Windows reference accepts an unseparated search by normalizing it before it queries. The cost is that a prefix can span a syllable boundary either way - `xian` finds both `xi'an` and `xian` - which is what a prefix search should do anyway.
pub fn dictionary_row_matches(
    kind_matches: bool,
    row_kind: ImportKind,
    key: &str,
    prefix: &str,
) -> bool {
    if !kind_matches {
        return false;
    }
    if prefix.is_empty() {
        return true;
    }
    if row_kind == ImportKind::Pinyin {
        let not_separator = |byte: &u8| !matches!(byte, b'\'' | b' ');
        let mut key = key.bytes().filter(not_separator);
        return prefix.bytes().filter(not_separator).all(|wanted| {
            key.next()
                .is_some_and(|byte| byte.eq_ignore_ascii_case(&wanted))
        });
    }
    let key = key.trim();
    if key.len() < prefix.len() {
        return false;
    }
    key.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}
