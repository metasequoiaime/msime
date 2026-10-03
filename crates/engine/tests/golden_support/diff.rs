//! Structural JSON diff with a path per difference, e.g. `steps[3] select {"word":"你好"}: snapshot.candidates[2].word: expected "你好", got "拟好"`.

use serde_json::Value;

/// `CandidateSource::Generated`, whose `weight` is a lattice score rather than a dictionary weight.
const GENERATED_SOURCE: u64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffOptions {
    /// `weight` of Generated rows is a lattice score; compare it only when asked.
    pub compare_generated_weight: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            compare_generated_weight: true,
        }
    }
}

/// Every difference between two recorder documents, step by step, stopping a step's list at a length mismatch so one shifted candidate does not bury the rest.
pub fn diff_documents(expected: &Value, actual: &Value, options: DiffOptions) -> Vec<String> {
    let mut out = Vec::new();
    diff_values(
        "name",
        &expected["name"],
        &actual["name"],
        options,
        &mut out,
    );
    let empty = Vec::new();
    let expected_steps = expected["steps"].as_array().unwrap_or(&empty);
    let actual_steps = actual["steps"].as_array().unwrap_or(&empty);
    if expected_steps.len() != actual_steps.len() {
        out.push(format!(
            "steps: expected {} steps, got {}",
            expected_steps.len(),
            actual_steps.len()
        ));
    }
    for (index, (expected_step, actual_step)) in expected_steps.iter().zip(actual_steps).enumerate()
    {
        let label = step_label(index, expected_step);
        let mut step_out = Vec::new();
        diff_values("", expected_step, actual_step, options, &mut step_out);
        out.extend(step_out.into_iter().map(|line| format!("{label}: {line}")));
    }
    diff_values(
        "final_journal",
        &expected["final_journal"],
        &actual["final_journal"],
        options,
        &mut out,
    );
    out
}

/// `steps[3] select {"word":"你好"}`: the index, the op and the compact argument when there is one.
fn step_label(index: usize, step: &Value) -> String {
    let op = step["op"].as_str().unwrap_or("?");
    match step.get("arg") {
        Some(arg) => format!("steps[{index}] {op} {arg}"),
        None => format!("steps[{index}] {op}"),
    }
}

/// Leaf-level differences under `path`.
pub fn diff_values(
    path: &str,
    expected: &Value,
    actual: &Value,
    options: DiffOptions,
    out: &mut Vec<String>,
) {
    match (expected, actual) {
        (Value::Object(expected_map), Value::Object(actual_map)) => {
            // A Generated row's weight is a lattice score; the recorder README lets replayers skip it.
            let skip_weight = !options.compare_generated_weight
                && expected_map.get("source").and_then(Value::as_u64) == Some(GENERATED_SOURCE);
            let mut keys: Vec<&String> = expected_map.keys().chain(actual_map.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                if skip_weight && key == "weight" {
                    continue;
                }
                let child = join_key(path, key);
                match (expected_map.get(key), actual_map.get(key)) {
                    (Some(e), Some(a)) => diff_values(&child, e, a, options, out),
                    (Some(e), None) => out.push(format!("{child}: expected {e}, got <absent>")),
                    (None, Some(a)) => out.push(format!("{child}: expected <absent>, got {a}")),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(expected_items), Value::Array(actual_items)) => {
            if expected_items.len() != actual_items.len() {
                out.push(format!(
                    "{}: expected {} items {}, got {} items {}",
                    display_path(path),
                    expected_items.len(),
                    summary(expected_items),
                    actual_items.len(),
                    summary(actual_items)
                ));
                return;
            }
            for (index, (e, a)) in expected_items.iter().zip(actual_items).enumerate() {
                diff_values(&format!("{path}[{index}]"), e, a, options, out);
            }
        }
        _ => {
            if expected != actual {
                out.push(format!(
                    "{}: expected {expected}, got {actual}",
                    display_path(path)
                ));
            }
        }
    }
}

fn join_key(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

fn display_path(path: &str) -> &str {
    if path.is_empty() {
        "<root>"
    } else {
        path
    }
}

/// A one-line view of a list for a length mismatch: candidates by word, anything else as compact JSON cut to a readable width.
fn summary(items: &[Value]) -> String {
    const WIDTH: usize = 60;
    let parts: Vec<String> = items
        .iter()
        .map(|item| match item.get("word").and_then(Value::as_str) {
            Some(word) => word.to_owned(),
            None => {
                let text = item.to_string();
                if text.chars().count() > WIDTH {
                    format!("{}…", text.chars().take(WIDTH).collect::<String>())
                } else {
                    text
                }
            }
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden_support::golden_dir;
    use serde_json::json;

    fn expected_documents() -> Vec<(String, Value)> {
        let mut paths: Vec<_> = std::fs::read_dir(golden_dir().join("expected"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                let text = std::fs::read_to_string(&path).unwrap();
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    serde_json::from_str(&text).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn every_expected_document_round_trips_without_a_diff() {
        let documents = expected_documents();
        assert_eq!(documents.len(), 299);
        for (name, document) in documents {
            let reparsed: Value = serde_json::from_str(&document.to_string()).unwrap();
            let diffs = diff_documents(&document, &reparsed, DiffOptions::default());
            assert!(diffs.is_empty(), "{name}: {diffs:?}");
        }
    }

    #[test]
    fn a_changed_candidate_word_is_reported_with_its_step_and_field_path() {
        let (_, expected) = expected_documents()
            .into_iter()
            .find(|(name, _)| name == "qp_paging_and_page_selection.json")
            .unwrap();
        let mut actual = expected.clone();
        actual["steps"][1]["snapshot"]["candidates"][1]["word"] = json!("你好吗");
        assert_eq!(
            diff_documents(&expected, &actual, DiffOptions::default()),
            vec![
                "steps[1] type \"nihao\": snapshot.candidates[1].word: expected \"拟好\", got \"你好吗\""
                    .to_owned()
            ]
        );
    }

    #[test]
    fn an_inserted_candidate_reports_the_length_once_instead_of_every_shifted_row() {
        let (_, expected) = expected_documents()
            .into_iter()
            .find(|(name, _)| name == "qp_paging_and_page_selection.json")
            .unwrap();
        let mut actual = expected.clone();
        let snapshot = &mut actual["steps"][1]["snapshot"];
        let extra = snapshot["candidates"][0].clone();
        snapshot["candidates"]
            .as_array_mut()
            .unwrap()
            .insert(0, extra);
        snapshot["candidate_count"] = json!(6);
        let diffs = diff_documents(&expected, &actual, DiffOptions::default());
        assert_eq!(diffs.len(), 2, "{diffs:?}");
        assert_eq!(
            diffs[0],
            "steps[1] type \"nihao\": snapshot.candidate_count: expected 5, got 6"
        );
        assert!(
            diffs[1].starts_with(
                "steps[1] type \"nihao\": snapshot.candidates: expected 5 items [你好, 拟好, "
            ),
            "{}",
            diffs[1]
        );
        assert!(
            diffs[1].contains("got 6 items [你好, 你好, 拟好"),
            "{}",
            diffs[1]
        );
    }

    #[test]
    fn missing_extra_and_step_count_differences_are_reported() {
        let expected = json!({"name": "s", "steps": [
            {"op": "open", "snapshot": {"caret": 0}},
            {"op": "char", "arg": "a", "result": {"handled": true, "commit": "a"}},
        ]});
        let actual = json!({"name": "s", "steps": [
            {"op": "open", "snapshot": {"caret": 0, "fuzzy": true}},
            {"op": "char", "arg": "a", "result": {"handled": true}},
            {"op": "command", "arg": "Cancel"},
        ], "final_journal": {}});
        assert_eq!(
            diff_documents(&expected, &actual, DiffOptions::default()),
            vec![
                "steps: expected 2 steps, got 3".to_owned(),
                "steps[0] open: snapshot.fuzzy: expected <absent>, got true".to_owned(),
                "steps[1] char \"a\": result.commit: expected \"a\", got <absent>".to_owned(),
                "final_journal: expected null, got {}".to_owned(),
            ]
        );
    }

    #[test]
    fn generated_weights_are_skipped_only_when_asked() {
        let expected = json!([{"word": "特乐好", "source": 8, "weight": -1234}, {"word": "你好", "source": 0, "weight": 200}]);
        let actual = json!([{"word": "特乐好", "source": 8, "weight": -1300}, {"word": "你好", "source": 0, "weight": 150}]);
        let mut out = Vec::new();
        diff_values(
            "candidates",
            &expected,
            &actual,
            DiffOptions {
                compare_generated_weight: false,
            },
            &mut out,
        );
        assert_eq!(
            out,
            vec!["candidates[1].weight: expected 200, got 150".to_owned()]
        );
        out.clear();
        diff_values(
            "candidates",
            &expected,
            &actual,
            DiffOptions::default(),
            &mut out,
        );
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn integers_and_floats_are_distinct() {
        let mut out = Vec::new();
        diff_values(
            "rows",
            &json!([[1]]),
            &json!([[1.0]]),
            DiffOptions::default(),
            &mut out,
        );
        assert_eq!(out, vec!["rows[0][0]: expected 1, got 1.0".to_owned()]);
    }
}
