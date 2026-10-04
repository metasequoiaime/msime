use super::*;
use msime_client_core::cloud::dictionary::MAX_IMPORT_BYTES;
use msime_client_core::dictionary::import::{ImportEntry, ImportIssue, MAX_ENTRIES};
use std::collections::{BTreeSet, HashSet};

/// The host's `import` operation as far as batching can see it: the 64 KiB request bound, the shared parser with its row and byte limits, one Engine receipt per row that makes a replayed request a no-op, and the whole-request refusal when the Engine takes none of the rows.
struct FakeHost {
    refuses: fn(&ImportEntry) -> bool,
    actions: Vec<Value>,
    sizes: Vec<usize>,
    receipts: HashSet<String>,
    store: BTreeSet<(String, String)>,
}

impl FakeHost {
    fn new() -> Self {
        Self::refusing(|_| false)
    }

    fn refusing(refuses: fn(&ImportEntry) -> bool) -> Self {
        Self {
            refuses,
            actions: Vec::new(),
            sizes: Vec::new(),
            receipts: HashSet::new(),
            store: BTreeSet::new(),
        }
    }

    fn send(&mut self, bytes: &[u8]) -> Result<Value, String> {
        if bytes.len() > 65536 {
            return Err(TOO_LARGE.into());
        }
        let request: Value = serde_json::from_slice(bytes).unwrap();
        let action = request["action"].clone();
        self.actions.push(action.clone());
        self.sizes.push(bytes.len());
        if action["operation"] != "import" {
            return Ok(json!({ "entries": [] }));
        }
        let kind: ImportKind = serde_json::from_value(action["kind"].clone())
            .map_err(|_| "invalid dictionary request".to_owned())?;
        let format = action["format"].as_str().unwrap();
        if format == "hans" {
            let text = action["text"].as_str().unwrap();
            if !hans_text_is_acceptable(text)
                || text.lines().filter(|line| !line.trim().is_empty()).count() > MAX_ENTRIES
            {
                return Err(INVALID_IMPORT.into());
            }
            return Ok(
                json!({ "applied": text.lines().filter(|line| !line.trim().is_empty()).count() }),
            );
        }
        let mut report = import::parse(
            kind,
            format,
            action["text"].as_str().unwrap(),
            MAX_IMPORT_BYTES,
        )
        .map_err(|error| error.to_string())?;
        let id = action["request_id"].as_str().unwrap();
        assert!(
            id.len() <= 120
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        );
        let mut applied = 0;
        let mut rejected = Vec::new();
        for (index, entry) in report.entries.iter().enumerate() {
            if (self.refuses)(entry) {
                rejected.push(entry.line);
                continue;
            }
            if self.receipts.insert(format!("{id}-{index}")) {
                self.store.insert((entry.key.clone(), entry.value.clone()));
            }
            applied += 1;
        }
        if applied == 0 && !rejected.is_empty() {
            return Err(IMPORT_REJECTED.into());
        }
        report.record_rejected(&rejected);
        Ok(json!({
            "applied": applied,
            "failed": report.failed,
            "truncated": report.truncated,
            "swapped": report.swapped,
            "first_failures": report.first_failures,
        }))
    }

    fn import(&mut self, kind: &str, format: &str, text: &str, id: &str) -> Result<Value, String> {
        let action = json!({ "operation": "import", "kind": kind, "format": format, "text": text, "request_id": id });
        send_dictionary_action(&options(), &action, |bytes| self.send(bytes))
    }

    fn ids(&self) -> Vec<String> {
        self.actions
            .iter()
            .map(|action| action["request_id"].as_str().unwrap().to_owned())
            .collect()
    }
}

fn options() -> Value {
    json!({
        "api_version": 1,
        "resources": "/data/resources",
        "user_data": "/data/user",
        "cache": "/data/cache",
        "dictionaries": "/data/dictionaries",
        "preferences": {},
    })
}

fn failures(report: &Value) -> Vec<(u64, String)> {
    report["first_failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|failure| {
            (
                failure["line"].as_u64().unwrap(),
                failure["issue"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// `n` spelled with one letter per decimal digit, `a` for 0 through `j` for 9: a quick phrase code
/// unique to its line and exactly as long as the number, since a code is letters only
/// (`quick_phrase_code_is_well_formed`) and several tests depend on the file's byte size.
fn letters(n: usize) -> String {
    n.to_string()
        .bytes()
        .map(|digit| char::from(b'a' + (digit - b'0')))
        .collect()
}

/// The quick phrase code the fixtures give line `n`.
fn code(n: usize) -> String {
    format!("q{}", letters(n))
}

/// One quick phrase per line, word first, with `special` put in place of the lines it names (1-based).
fn phrases(count: usize, special: &[(usize, &str)]) -> String {
    (1..=count)
        .map(|line| match special.iter().find(|(at, _)| *at == line) {
            Some((_, text)) => format!("{text}\n"),
            None => format!("短语{line}\t{}\t100\n", code(line)),
        })
        .collect()
}

#[test]
fn five_thousand_rows_land_in_batches_and_failures_keep_their_file_lines() {
    let mut host = FakeHost::refusing(|entry| entry.key == "refused");
    let text = phrases(
        5003,
        &[
            (1500, "no tab here"),
            (3000, "拒绝\trefused\t100"),
            (4200, "短语\tNOT-LOWER\t1"),
        ],
    );
    assert!(text.len() > 65536);
    let report = host
        .import("quick_phrase", "standard", &text, "ui-import-7")
        .unwrap();
    assert_eq!(report["applied"], 5000);
    assert_eq!(report["failed"], 3);
    assert_eq!(report["truncated"], false);
    assert_eq!(report["swapped"], false);
    assert_eq!(
        failures(&report),
        [
            (1500, "column_count".to_owned()),
            (3000, "rejected".to_owned()),
            (4200, "key_alphabet".to_owned())
        ]
    );
    assert_eq!(host.store.len(), 5000);
    // Every request fits the host with room to spare, reads no more rows than the host does, and carries its own ID.
    let batches = host.actions.len();
    assert!(batches >= 5, "{batches}");
    assert!(host.sizes.iter().all(|size| *size <= BATCH_REQUEST_BYTES));
    let ids: Vec<String> = (0..batches)
        .map(|index| format!("ui-import-7-{index}"))
        .collect();
    assert_eq!(host.ids(), ids);
    // Each batch starts on a line of its own: the batches are the file, cut at line ends.
    let sent: String = host
        .actions
        .iter()
        .map(|action| action["text"].as_str().unwrap())
        .collect();
    assert_eq!(sent, text);
}

#[test]
fn a_file_over_two_mebibytes_adds_up_across_its_batches_and_keeps_its_file_lines() {
    let mut host = FakeHost::refusing(|entry| entry.key == "refused");
    let count = 100_000;
    let text = phrases(
        count,
        &[
            (7, "no tab here"),
            (54_321, "拒绝\trefused\t100"),
            (count - 1, "短语\tNOT-LOWER\t1"),
        ],
    );
    // Over the 1 MiB the page used to stop at, and well within what it reads now.
    assert!(text.len() > 2 * 1024 * 1024 && text.len() < MAX_IMPORT_TEXT_BYTES);
    let report = host
        .import("quick_phrase", "standard", &text, "ui-import-2m")
        .unwrap();
    assert_eq!(report["applied"], count - 3);
    assert_eq!(report["failed"], 3);
    assert_eq!(report["truncated"], false);
    assert_eq!(
        failures(&report),
        [
            (7, "column_count".to_owned()),
            (54_321, "rejected".to_owned()),
            ((count - 1) as u64, "key_alphabet".to_owned())
        ]
    );
    assert_eq!(host.store.len(), count - 3);
    // Every batch is within both host bounds, and together they are the file.
    let batches = host.actions.len();
    assert!(batches >= count / MAX_ENTRIES, "{batches}");
    assert!(host.sizes.iter().all(|size| *size <= BATCH_REQUEST_BYTES));
    assert!(host.actions.iter().all(|action| {
        let rows = action["text"]
            .as_str()
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        rows <= MAX_ENTRIES
    }));
    let ids: Vec<String> = (0..batches)
        .map(|index| format!("ui-import-2m-{index}"))
        .collect();
    assert_eq!(host.ids(), ids);
    let sent: String = host
        .actions
        .iter()
        .map(|action| action["text"].as_str().unwrap())
        .collect();
    assert_eq!(sent, text);
}

#[test]
fn replaying_a_batched_import_writes_nothing_again() {
    let mut host = FakeHost::new();
    let text = phrases(2500, &[]);
    assert_eq!(
        host.import("quick_phrase", "standard", &text, "ui-import-8")
            .unwrap()["applied"],
        2500
    );
    let first_ids = host.ids();
    // The user deletes one entry; a retried request with the same ID must not bring it back.
    let deleted = (code(7), "短语7".to_owned());
    assert!(host.store.remove(&deleted));
    host.actions.clear();
    let replayed = host
        .import("quick_phrase", "standard", &text, "ui-import-8")
        .unwrap();
    assert_eq!(replayed["applied"], 2500);
    assert_eq!(host.ids(), first_ids);
    assert_eq!(host.store.len(), 2499);
    assert!(!host.store.contains(&deleted));
    // A new import is a new request and does write it again.
    host.import("quick_phrase", "standard", &text, "ui-import-9")
        .unwrap();
    assert!(host.store.contains(&deleted));
}

#[test]
fn an_overflowing_batch_report_is_rejected_without_panicking() {
    let text = phrases(5000, &[]);
    let action = json!({
        "operation": "import",
        "kind": "quick_phrase",
        "format": "standard",
        "text": text,
        "request_id": "ui-overflow"
    });
    let result = send_dictionary_action(&options(), &action, |_bytes| {
        Ok(json!({
            "applied": u64::MAX,
            "failed": 0,
            "truncated": false,
            "swapped": false,
            "first_failures": []
        }))
    });
    assert_eq!(result, Err("invalid dictionary report".to_owned()));
}

#[test]
fn a_rime_header_longer_than_one_batch_is_never_read_as_rows() {
    let mut host = FakeHost::new();
    let mut text = String::from(
        "\u{feff}# Rime dictionary\n---\nname: user\nversion: \"1\"\nimport_tables:\n",
    );
    for index in 0..4000 {
        text.push_str(&format!("  - extra_dictionary_{index}\n"));
    }
    text.push_str("...\n");
    let header_lines = text.lines().count();
    for index in 1..=1500 {
        text.push_str(&format!("短语{index}\t{}\t100\n", code(index)));
    }
    text.push_str("短语\tNOT-LOWER\n");
    assert!(text.len() > 2 * BATCH_REQUEST_BYTES);
    let report = host
        .import("quick_phrase", "rime", &text, "ui-rime")
        .unwrap();
    assert_eq!(report["applied"], 1500);
    assert_eq!(report["failed"], 1);
    assert_eq!(
        failures(&report),
        [((header_lines + 1501) as u64, "key_alphabet".to_owned())]
    );
    // The header reaches the host only as the empty lines that keep every row at its own number.
    assert!(host.actions.iter().all(|action| !action["text"]
        .as_str()
        .unwrap()
        .contains("extra_dictionary")));
    assert_eq!(host.ids(), ["ui-rime-0", "ui-rime-1"]);
    // Cut the same way without knowing it is Rime, the second piece would begin inside the header.
    let naive = split_import(&text, false, 60_000).unwrap();
    assert!(naive[1].text.starts_with("  - extra_dictionary_"));
}

#[test]
fn a_file_in_the_other_column_order_is_read_that_way_throughout() {
    let mut host = FakeHost::new();
    let text: String = (1..=2500)
        .map(|index| format!("{}\t短语{index}\t100\n", code(index)))
        .collect();
    let report = host
        .import("quick_phrase", "standard", &text, "ui-swap")
        .unwrap();
    assert_eq!(report["applied"], 2500);
    assert_eq!(report["swapped"], true);
    assert!(host
        .actions
        .iter()
        .all(|action| action["format"] == "windows"));
    assert!(host.store.contains(&(code(2500), "短语2500".to_owned())));
}

#[test]
fn a_batch_of_bad_rows_is_not_read_in_the_other_order_on_its_own() {
    let mut host = FakeHost::new();
    // A run of rows written the other way round, long enough to fill a batch by itself. One request would have counted them as failures, since the file as a whole is in the asked order.
    let text: String = (1..=3000)
        .map(|index| {
            if (1001..=2000).contains(&index) {
                format!("{}\t短语{index}\t100\n", code(index))
            } else {
                format!("短语{index}\t{}\t100\n", code(index))
            }
        })
        .collect();
    let report = host
        .import("quick_phrase", "standard", &text, "ui-mixed")
        .unwrap();
    assert_eq!(report["applied"], 2000);
    assert_eq!(report["failed"], 1000);
    assert_eq!(report["swapped"], false);
    assert_eq!(failures(&report)[0], (1001, "key_alphabet".to_owned()));
    assert!(host
        .actions
        .iter()
        .all(|action| action["format"] == "standard"));
    assert!(!host.store.contains(&(code(1500), "短语1500".to_owned())));
}

#[test]
fn a_batch_the_engine_refuses_whole_is_counted_row_by_row() {
    let mut host = FakeHost::refusing(|entry| entry.key.starts_with('x'));
    let text: String = (1..=2500)
        .map(|index| {
            let key = if (1001..=2000).contains(&index) {
                "x"
            } else {
                "q"
            };
            format!("短语{index}\t{key}{}\t100\n", letters(index))
        })
        .collect();
    let report = host
        .import("quick_phrase", "standard", &text, "ui-refused")
        .unwrap();
    assert_eq!(report["applied"], 1500);
    assert_eq!(report["failed"], 1000);
    assert_eq!(failures(&report)[0], (1001, "rejected".to_owned()));
    assert_eq!(failures(&report).len(), REPORTED_FAILURES);
    // Refused everywhere is a failed import, as one request would have said.
    let mut host = FakeHost::refusing(|_| true);
    assert_eq!(
        host.import("quick_phrase", "standard", &text, "ui-refused")
            .unwrap_err(),
        IMPORT_REJECTED
    );
}

#[test]
fn a_file_the_host_would_refuse_whole_is_refused_before_anything_is_written() {
    let mut host = FakeHost::new();
    let control = phrases(3000, &[(2800, "短语\tq\u{7}")]);
    assert_eq!(
        host.import("quick_phrase", "standard", &control, "ui-bad")
            .unwrap_err(),
        ImportError::ControlCharacters.to_string()
    );
    let nothing: String = (1..=3000)
        .map(|index| format!("no columns {index}\n"))
        .collect();
    assert_eq!(
        host.import("quick_phrase", "standard", &nothing, "ui-bad")
            .unwrap_err(),
        ImportError::NoUsableRows.to_string()
    );
    let hans: String = (1..=3000)
        .map(|index| {
            if index == 2900 {
                "latin\n".to_owned()
            } else {
                "你好\n".to_owned()
            }
        })
        .collect();
    assert_eq!(
        host.import("pinyin", "hans", &hans, "ui-bad").unwrap_err(),
        INVALID_IMPORT
    );
    assert!(host.actions.is_empty());
    assert!(host.store.is_empty());
}

#[test]
fn a_hans_file_is_sent_in_batches_the_host_will_read() {
    let mut host = FakeHost::new();
    let text = "你好\n\n".repeat(2500);
    let report = host.import("pinyin", "hans", &text, "ui-hans").unwrap();
    assert_eq!(report["applied"], 2500);
    assert_eq!(host.ids(), ["ui-hans-0", "ui-hans-1", "ui-hans-2"]);
}

#[test]
fn an_import_that_fits_one_request_and_every_other_action_go_as_they_are() {
    let mut host = FakeHost::new();
    let action = json!({ "operation": "import", "kind": "quick_phrase", "format": "standard", "text": "短语\tq\t1\n", "request_id": "ui-small" });
    let whole = serde_json::to_vec(&json!({ "options": options(), "action": action })).unwrap();
    let mut sent = Vec::new();
    send_dictionary_action(&options(), &action, |bytes| {
        sent.push(bytes.to_vec());
        host.send(bytes)
    })
    .unwrap();
    assert_eq!(sent, [whole]);
    assert_eq!(host.ids(), ["ui-small"]);
    let list = json!({ "operation": "list", "kind": "pinyin", "offset": 0, "limit": 10 });
    let whole = serde_json::to_vec(&json!({ "options": options(), "action": list })).unwrap();
    let mut sent = Vec::new();
    send_dictionary_action(&options(), &list, |bytes| {
        sent.push(bytes.to_vec());
        Ok(Value::Null)
    })
    .unwrap();
    assert_eq!(sent, [whole]);
}

#[test]
fn a_file_too_large_to_send_is_refused_as_too_large() {
    let mut host = FakeHost::new();
    let oversized = phrases(MAX_IMPORT_TEXT_BYTES / 20, &[]);
    assert!(oversized.len() > MAX_IMPORT_TEXT_BYTES);
    assert_eq!(
        host.import("quick_phrase", "standard", &oversized, "ui-big")
            .unwrap_err(),
        TOO_LARGE
    );
    // One line no request could carry is the same refusal, not a partial import.
    let long_line = phrases(100, &[(50, &format!("短语\tq\t{}", "1".repeat(70_000)))]);
    assert_eq!(
        host.import("quick_phrase", "standard", &long_line, "ui-big")
            .unwrap_err(),
        TOO_LARGE
    );
    assert!(host.actions.is_empty());
    assert_eq!(
        crate::dictionary_error_code(TOO_LARGE),
        "dictionary_too_large"
    );
    // The parser's own size refusal only reaches Android's unbatched path, whose 64 KiB limit the 32 MB message would misstate.
    assert_ne!(
        crate::dictionary_error_code(&ImportError::TooLarge.to_string()),
        "dictionary_too_large"
    );
}

#[test]
fn batches_are_cut_at_line_ends_within_both_host_bounds() {
    let text = format!("{}{}", "a\"\\\t\n".repeat(30_000), "\n#\n".repeat(3000));
    let batches = split_import(&text, false, 60_000).unwrap();
    assert_eq!(
        batches
            .iter()
            .map(|batch| batch.text.as_str())
            .collect::<String>(),
        text
    );
    let mut lines = 0;
    for batch in &batches {
        assert_eq!(batch.lines_before, lines);
        assert!(batch.rows <= MAX_ENTRIES);
        assert!(serde_json::to_string(&batch.text).unwrap().len() - 2 <= 60_000);
        assert!(batch.text.ends_with('\n'));
        lines += batch.text.lines().count();
    }
    assert_eq!(
        batches.iter().map(|batch| batch.rows).sum::<usize>(),
        30_000
    );
    assert_eq!(
        json_escaped_len("\"\\\u{1}\u{8}é你"),
        serde_json::to_string("\"\\\u{1}\u{8}é你").unwrap().len() - 2
    );
}

#[test]
fn zero_batch_budget_is_rejected_before_reserving_batches() {
    assert!(split_import("", false, 0).is_none());
}

#[test]
fn a_large_import_with_the_longest_request_id_still_fits() {
    let mut host = FakeHost::new();
    let text = phrases(4000, &[]);
    let id = "i".repeat(118);
    assert_eq!(
        host.import("quick_phrase", "standard", &text, &id).unwrap()["applied"],
        4000
    );
    assert!(host.sizes.iter().all(|size| *size <= BATCH_REQUEST_BYTES));
    assert!(host
        .ids()
        .iter()
        .all(|batch| batch.starts_with(&id) && batch.len() <= 120));
    // An ID the host accepts on its own but with no room for a batch number is refused before anything is written, not shortened into one another import might share.
    host.actions.clear();
    let id = "i".repeat(119);
    assert_eq!(
        host.import("quick_phrase", "standard", &text, &id)
            .unwrap_err(),
        INVALID_REQUEST_ID
    );
    assert!(host.actions.is_empty());
    assert_eq!(
        serde_json::to_value(ImportIssue::Rejected).unwrap(),
        "rejected"
    );
}
