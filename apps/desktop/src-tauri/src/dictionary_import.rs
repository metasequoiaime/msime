//! A settings-page dictionary request, with an import split into requests the host accepts.
//!
//! `msime_host_api::dictionary_request_json` takes at most 64 KiB per request and reads at most `import::MAX_ENTRIES` rows from one import. That is the contract `msime-client-dictionary` documents, so it stays as it is. The page instead sends a file of up to [`MAX_IMPORT_TEXT_BYTES`] as a run of line-aligned requests under both bounds and answers with one report for the whole file, the way the Windows settings page imports a file whole. Each request carries its own ID, `<id>-<n>`, so the host's per-row receipts still make a repeated import a no-op.

use msime_client_core::dictionary::import::{
    self, ImportError, ImportFailure, ImportFormat, ImportKind, REPORTED_FAILURES,
};
use msime_client_core::dictionary::is_han_character;
use serde_json::{json, Value};

/// The largest file text the page imports. The page refuses files over 32 MiB (`MAX_DICTIONARY_FILE_BYTES` in `packages/ui/src/dictionary/dictionary-file.ts`) before reading them, and a file that size decodes to at most 48 MiB of UTF-8, since a two-byte GBK or UTF-16 character becomes three; this is that bound, so a file the page accepted is never refused here. At 60 KiB a request, text this size is at least 820 requests, and more for a file of short lines, since a request also carries at most `import::MAX_ENTRIES` rows. That takes longer than the 30 seconds input sessions stay released, so every request renews the release before it goes: the lease through `QuiescedHosts::run` on Linux and macOS, the Server's DictionaryQuiesce on Windows.
pub(crate) const MAX_IMPORT_TEXT_BYTES: usize = 3 * 16 * 1024 * 1024;

/// One serialized request, below the host's 65536-byte limit with room to spare.
const BATCH_REQUEST_BYTES: usize = 60 * 1024;

/// The host's reason for a request it will not read at all because of its size; `dictionary_error_code` answers it as `dictionary_too_large`. Used here for a file over [`MAX_IMPORT_TEXT_BYTES`] and for a single line too long for any request.
pub(crate) const TOO_LARGE: &str = "invalid dictionary buffer";

/// The longest request ID the host accepts, and its reason for a longer one.
const MAX_REQUEST_ID: usize = 120;
const INVALID_REQUEST_ID: &str = "invalid dictionary request ID";

/// The host's reason when every parsed row of a request was refused by the Engine.
const IMPORT_REJECTED: &str = "dictionary import rejected";

/// Send one dictionary action through `send`, which delivers one serialized request to the host. Every action other than an import, and an import that fits one request, is sent exactly as it was given. A larger import is sent in batches and the host's reports are added up, with each failure's line counted from the start of the whole file.
pub(crate) fn send_dictionary_action(
    options: &Value,
    action: &Value,
    mut send: impl FnMut(&[u8]) -> Result<Value, String>,
) -> Result<Value, String> {
    let whole = serde_json::to_vec(&json!({ "options": options, "action": action }))
        .map_err(|error| error.to_string())?;
    let Some(request) = ImportRequest::from_action(action) else {
        return send(&whole);
    };
    if request.text.len() > MAX_IMPORT_TEXT_BYTES {
        return Err(TOO_LARGE.into());
    }
    // Everything the action carries except the text, with the longest batch ID it can be given, so the budget below holds for every batch.
    let mut template = action.clone();
    template["text"] = Value::String(String::new());
    template["request_id"] = Value::String(format!("{}-{}", request.id, usize::MAX));
    let overhead = serde_json::to_vec(&json!({ "options": options, "action": template }))
        .map_err(|error| error.to_string())?
        .len();
    let batches = split_import(
        request.text,
        request.format == "rime",
        BATCH_REQUEST_BYTES.saturating_sub(overhead),
    )
    .ok_or(TOO_LARGE)?;
    if batches.len() == 1 && whole.len() <= BATCH_REQUEST_BYTES {
        return send(&whole);
    }
    // The host takes IDs of at most 120 characters. Shortening one to make room for the batch number could give two imports the same receipts, and the second would then write nothing, so an ID without the room is refused as the host would refuse it.
    if format!("{}-{}", request.id, batches.len()).len() > MAX_REQUEST_ID {
        return Err(INVALID_REQUEST_ID.into());
    }

    let mut total = ImportTotal::default();
    // The rows as one request would have read them: the column order the whole file is in, decided once, and each batch parsed in it. The host only tries the other order for a request with nothing usable in the asked one, so a batch that happened to hold only bad rows would otherwise be read the other way round on its own.
    let kind = serde_json::from_value::<ImportKind>(request.kind.clone());
    let rows = match kind {
        Ok(kind) if request.format != "hans" => {
            // A file the host would refuse as a whole - an unsupported format, a control character anywhere, nothing usable in either column order - is refused before any part of it is written, as one request would have been.
            let report = import::parse(kind, request.format, request.text, usize::MAX)
                .map_err(|error| error.to_string())?;
            total.swapped = report.swapped;
            let format = match (request.format, report.swapped) {
                ("standard", true) => "windows",
                ("windows", true) => "standard",
                (format, _) => format,
            };
            let parsed = ImportFormat::parse(format)
                .ok_or_else(|| ImportError::UnsupportedFormat.to_string())?;
            template["format"] = Value::String(format.to_owned());
            Some((kind, parsed))
        }
        // `hans` is all or nothing in the host, which parses it alone. What it refuses without the Engine is refused here for the whole file, so a bad line late in it cannot fail its batch after earlier ones landed.
        Ok(_) if !hans_text_is_acceptable(request.text) => return Err(INVALID_IMPORT.into()),
        Ok(_) => None,
        // A kind the host does not know. It refuses the request for that before reading any row, so the first part of the file is enough for it to say so.
        Err(_) => {
            template["text"] = Value::String(batches[0].text.clone());
            template["request_id"] = Value::String(format!("{}-0", request.id));
            let bytes = serde_json::to_vec(&json!({ "options": options, "action": template }))
                .map_err(|error| error.to_string())?;
            return send(&bytes);
        }
    };
    let mut refusals = Vec::with_capacity(batches.len());
    let mut index = 0;
    for batch in batches {
        if let Some((kind, format)) = rows {
            let text = batch.text.strip_prefix('\u{feff}').unwrap_or(&batch.text);
            let local = import::parse_rows(kind, format, text);
            // Nothing in this part of the file can be imported, so there is nothing to send; its rows are reported as the host would have reported them.
            if local.entries.is_empty() {
                total.failed += local.failed as u64;
                total.add_failures(local.first_failures, batch.lines_before);
                continue;
            }
        } else if batch.rows == 0 {
            continue;
        }
        template["text"] = Value::String(batch.text.clone());
        template["request_id"] = Value::String(format!("{}-{index}", request.id));
        index += 1;
        let bytes = serde_json::to_vec(&json!({ "options": options, "action": template }))
            .map_err(|error| error.to_string())?;
        match send(&bytes) {
            Ok(report) => total.add_report(&report, batch.lines_before),
            // Every row of this batch parsed and the Engine refused them all: part of the file failing row by row, not the file failing. Name those rows as a single request would have.
            Err(reason) if reason == IMPORT_REJECTED => {
                total.add_refused_batch(rows, &batch.text, batch.rows, batch.lines_before);
                refusals.push(reason);
            }
            // Anything else is about the request or the store, and the next batch would meet it too.
            Err(reason) => return Err(reason),
        }
    }
    // Nothing landed and the Engine refused everything: a failed import, as a single request would have said.
    if total.applied == 0 {
        if let Some(reason) = refusals.into_iter().next() {
            return Err(reason);
        }
    }
    Ok(total.into_report())
}

/// The host's reason for a `hans` file it will not read.
const INVALID_IMPORT: &str = "invalid dictionary import";

/// Whether the host's `hans` parser would read `text` as a whole, short of asking the Engine for the readings: no control character but line breaks, at least one word, and every word at most 1024 bytes of Han characters alone. The host's own count limit is per request and the batches keep to it. These are `parse_hans_import`'s checks in `msime-host-api`, which still makes them for every batch.
fn hans_text_is_acceptable(text: &str) -> bool {
    if msime_client_core::has_disallowed_control_with_line_breaks(text) {
        return false;
    }
    let mut words = text
        .lines()
        .map(str::trim)
        .filter(|word| !word.is_empty() && !word.starts_with('#'))
        .peekable();
    words.peek().is_some()
        && words.all(|word| word.len() <= 1024 && word.chars().all(is_han_character))
}

struct ImportRequest<'a> {
    kind: &'a Value,
    format: &'a str,
    text: &'a str,
    id: &'a str,
}

impl<'a> ImportRequest<'a> {
    /// An import with the fields batching needs. Anything else goes to the host unchanged, and the host refuses it as it always has.
    fn from_action(action: &'a Value) -> Option<Self> {
        (action.get("operation")?.as_str()? == "import").then_some(())?;
        Some(Self {
            kind: action.get("kind")?,
            format: action.get("format")?.as_str()?,
            text: action.get("text")?.as_str()?,
            id: action.get("request_id")?.as_str()?,
        })
    }
}

/// One request's share of the file.
#[derive(Debug)]
pub(crate) struct ImportBatch {
    pub(crate) text: String,
    /// Lines of the file before this batch, which is what the host's line numbers are offset by.
    pub(crate) lines_before: usize,
    /// Lines the host will read as rows: not blank, not a comment, not part of a Rime header.
    pub(crate) rows: usize,
    /// Size of `text` once escaped into a JSON string.
    bytes: usize,
}

/// Cut `text` at line ends into batches of at most `budget` bytes once escaped into JSON and at most `import::MAX_ENTRIES` rows each, which is all the host reads from one import. `None` when a single line is over the budget on its own.
///
/// A Rime file's YAML header, `---` through `...`, is replaced by empty lines rather than carried: the parser only skips it when it sees the opening `---`, so a batch that began inside it would read the rest as rows. Keeping the lines, empty, keeps every later line at its own number.
pub(crate) fn split_import(text: &str, rime: bool, budget: usize) -> Option<Vec<ImportBatch>> {
    let empty = |lines_before| ImportBatch {
        text: String::new(),
        lines_before,
        rows: 0,
        bytes: 0,
    };
    // The parser strips a leading byte-order mark before it looks for the header, and so does this. The mark stays in the text the host receives unless the header opens on its line, which is blanked with it.
    let first_line_start = if rime && text.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let mut batches = Vec::new();
    let mut current = empty(0);
    let mut in_header = false;
    for (index, piece) in text.split_inclusive('\n').enumerate() {
        let trimmed = if index == 0 {
            piece[first_line_start..].trim()
        } else {
            piece.trim()
        };
        // The same rules, in the same order, as the shared parser's `parse_rows`.
        let content = !trimmed.is_empty() && !trimmed.starts_with('#');
        let header = rime
            && content
            && match trimmed {
                "---" => {
                    in_header = true;
                    true
                }
                "..." => {
                    in_header = false;
                    true
                }
                _ => in_header,
            };
        let row = content && !header;
        let piece = if header { "\n" } else { piece };
        let bytes = json_escaped_len(piece);
        if bytes > budget {
            return None;
        }
        if current.bytes + bytes > budget || (row && current.rows == import::MAX_ENTRIES) {
            batches.push(std::mem::replace(&mut current, empty(index)));
        }
        current.text.push_str(piece);
        current.bytes += bytes;
        if row {
            current.rows += 1;
        }
    }
    batches.push(current);
    Some(batches)
}

/// The length serde_json writes `text` in, between the quotes of a JSON string.
fn json_escaped_len(text: &str) -> usize {
    text.chars()
        .map(|character| match character {
            '"' | '\\' | '\u{8}' | '\u{c}' | '\n' | '\r' | '\t' => 2,
            '\0'..='\u{1f}' => 6,
            character => character.len_utf8(),
        })
        .sum()
}

struct ImportTotal {
    applied: u64,
    failed: u64,
    truncated: bool,
    swapped: bool,
    failures: Vec<ImportFailure>,
}

impl Default for ImportTotal {
    fn default() -> Self {
        Self {
            applied: 0,
            failed: 0,
            truncated: false,
            swapped: false,
            failures: Vec::with_capacity(REPORTED_FAILURES),
        }
    }
}

impl ImportTotal {
    fn add_report(&mut self, report: &Value, lines_before: usize) {
        self.applied += report["applied"].as_u64().unwrap_or(0);
        self.failed += report["failed"].as_u64().unwrap_or(0);
        self.truncated |= report["truncated"].as_bool() == Some(true);
        self.swapped |= report["swapped"].as_bool() == Some(true);
        let failures: Vec<ImportFailure> =
            serde_json::from_value(report["first_failures"].clone()).unwrap_or_default();
        self.add_failures(failures, lines_before);
    }

    /// A batch whose rows the Engine refused, every one. The shared parser says which way each row failed, and the rows it could read are the ones the Engine refused.
    fn add_refused_batch(
        &mut self,
        parsed: Option<(ImportKind, ImportFormat)>,
        text: &str,
        rows: usize,
        lines_before: usize,
    ) {
        let Some((kind, format)) = parsed else {
            // `hans` has no parsed report to name its rows from; they are still counted.
            self.failed += rows as u64;
            return;
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut report = import::parse_rows(kind, format, text);
        let refused: Vec<usize> = report.entries.drain(..).map(|entry| entry.line).collect();
        report.record_rejected(&refused);
        self.failed += report.failed as u64;
        self.add_failures(report.first_failures, lines_before);
    }

    fn add_failures(&mut self, failures: Vec<ImportFailure>, lines_before: usize) {
        self.failures
            .extend(failures.into_iter().map(|failure| ImportFailure {
                // Line 0 is a row the host could not place, and stays unplaced.
                line: if failure.line == 0 {
                    0
                } else {
                    failure.line + lines_before
                },
                ..failure
            }));
    }

    fn into_report(mut self) -> Value {
        self.failures.sort_by_key(|failure| failure.line);
        self.failures.truncate(REPORTED_FAILURES);
        json!({
            "applied": self.applied,
            "failed": self.failed,
            "truncated": self.truncated,
            "swapped": self.swapped,
            "first_failures": self.failures,
        })
    }
}

#[cfg(test)]
mod tests;
