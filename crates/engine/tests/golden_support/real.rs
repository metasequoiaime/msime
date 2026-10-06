//! Real-dictionary sets (`golden/real/*.jsonl`): one session with the product options (`prepare_options` defaults, `sentence_alternatives = true`, learning off) types every row, then `Cancel` and `reset_context()`. Rows compare `n`, `unhandled` and the first nine `[word, source, corrected_from?]`.

use std::path::{Path, PathBuf};

use msime_engine::{
    Command, EnglishInputOptions, FrequencyAdjustmentMode, FrequencyAdjustmentOptions,
    LocalModeOptions, MixedExpressiveOptions, RuntimePaths, SchemeType, Session, SessionOptions,
    ShuangpinProfileKind,
};
use serde_json::{Map, Value};

use super::diff::{diff_values, DiffOptions};
use super::golden_dir;

pub const RESOURCES_ENV: &str = "MSIME_EVAL_RESOURCES";

/// Candidates kept per row: the runtime's page size cap (`crates/input-runtime/src/runtime.rs:478`), the recorder's `--top` default.
const TOP: usize = 9;

/// Differing rows printed before the rest is summarised as a count.
const MAX_REPORTED_ROWS: usize = 50;

/// The resource directory, or `None` with the skip reason printed.
pub fn resources() -> Option<PathBuf> {
    match std::env::var_os(RESOURCES_ENV) {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => {
            eprintln!("skipping real-dictionary golden: {RESOURCES_ENV} is not set (needs dict-v2.0.1 without dict_pinyin.dat and the sentence models)");
            None
        }
    }
}

/// Replay one set and fail with the differing rows; returns without running when the resources are absent.
pub fn assert_real_set(file_name: &str) {
    let Some(resources) = resources() else {
        return;
    };
    assert!(
        has_recorded_resources(&resources),
        "{RESOURCES_ENV}={} lacks msime-pinyin.db, msime-english.db or the n-gram tables",
        resources.display()
    );
    let failures = replay_real_set(file_name, &resources);
    let mut shown = failures
        .iter()
        .take(MAX_REPORTED_ROWS)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if failures.len() > MAX_REPORTED_ROWS {
        shown.push_str(&format!(
            "\n... and {} more",
            failures.len() - MAX_REPORTED_ROWS
        ));
    }
    assert!(
        failures.is_empty(),
        "{file_name}: {} rows differ\n{shown}",
        failures.len()
    );
}

/// The recorded rows of one set, in file order.
pub fn expected_rows(file_name: &str) -> Vec<Value> {
    let path = golden_dir().join("real").join(file_name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("{}: bad line {line}: {error}", path.display()))
        })
        .collect()
}

/// Row-by-row differences, each with the row id, input and both candidate lists.
pub fn replay_real_set(file_name: &str, resources: &std::path::Path) -> Vec<String> {
    let rows = expected_rows(file_name);
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("real");
    let resources = std::path::absolute(resources).unwrap();
    let mut options = SessionOptions::new(RuntimePaths::default());
    product_options(&mut options);
    options.paths = msime_engine::prepare_runtime_paths(
        &resources,
        &root.join("user"),
        &root.join("cache"),
        "real",
    )
    .unwrap_or_else(|error| panic!("prepare_runtime_paths failed: {error}"));
    let mut session =
        Session::new(options).unwrap_or_else(|error| panic!("Session::new failed: {error}"));
    let mut failures = Vec::new();
    for expected in &rows {
        let id = expected["id"].as_str().expect("row id");
        let input = expected["input"].as_str().expect("row input");
        let gold = expected["gold"].as_str().expect("row gold");
        // Every letter is typed even after one is refused, as the recorder did.
        let mut handled = true;
        for byte in input.bytes() {
            handled = session.character(byte, false).handled && handled;
        }
        let actual = row_json(id, input, gold, handled, &session);
        if let Some(failure) = diff_row(expected, &actual) {
            failures.push(failure);
        }
        session.command(Command::Cancel);
        session.reset_context();
    }
    drop(session);
    msime_engine::flush_personal_learning();
    msime_engine::close_cached_databases();
    failures
}

/// One report line for a row that differs, or `None`: the field paths first, then both candidate lists as `word/source`.
pub fn diff_row(expected: &Value, actual: &Value) -> Option<String> {
    let mut diffs = Vec::new();
    diff_values("", expected, actual, DiffOptions::default(), &mut diffs);
    if diffs.is_empty() {
        return None;
    }
    Some(format!(
        "{} {}: {}\n    expected: {}\n    got:      {}",
        expected["id"].as_str().unwrap_or("?"),
        expected["input"].as_str().unwrap_or("?"),
        diffs.join("; "),
        candidate_list(&expected["c"]),
        candidate_list(&actual["c"])
    ))
}

fn candidate_list(candidates: &Value) -> String {
    candidates
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| match item.as_array().map(Vec::as_slice) {
                    Some([word, source]) => format!("{}/{source}", word.as_str().unwrap_or("?")),
                    Some([word, source, corrected]) => format!(
                        "{}/{source}<-{}",
                        word.as_str().unwrap_or("?"),
                        corrected.as_str().unwrap_or("?")
                    ),
                    _ => item.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// The product options the real sets were recorded with (recorder.cpp `apply_product_options`).
pub fn product_options(options: &mut SessionOptions) {
    options.scheme = SchemeType::Quanpin;
    options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
    options.learning = false;
    options.autocorrect_types = 0;
    options.fuzzy_pinyin.rules = 0;
    options.helpcode = true;
    options.helpcode_schema = "ziranma".to_owned();
    options.chinese_punctuation = true;
    options.paired_punctuation = true;
    options.punctuation_lock = 0;
    options.frequency = FrequencyAdjustmentOptions {
        mode: FrequencyAdjustmentMode::Promote,
        trigger_count: 1,
        linear_step: 1,
    };
    options.english = EnglishInputOptions {
        mixed_candidates: true,
        minimum_prefix: 5,
    };
    options.expressive = MixedExpressiveOptions {
        emoji_candidates: false,
        kaomoji_candidates: false,
    };
    options.local_modes = LocalModeOptions {
        unicode: true,
        date_time: true,
        quick_phrase: true,
        emoji: true,
        kaomoji: true,
        super_jianpin: true,
        temporary_english: true,
        temporary_japanese: true,
        expression: false,
        command: false,
        mention: false,
    };
    options.wubi.mixed_pinyin = false;
    options.sentence_alternatives = true;
}

/// `{id, input, gold, unhandled?, n, c}` for the current snapshot, as the recorder writes it.
pub fn row_json(
    id: &str,
    input: &str,
    gold: &str,
    handled: bool,
    session: &msime_engine::Session,
) -> Value {
    let view = session.snapshot();
    let candidates = view
        .candidates
        .iter()
        .take(TOP)
        .map(|item| {
            let mut entry = vec![
                Value::from(item.word.as_str()),
                Value::from(item.source as u8),
            ];
            if !item.corrected_from.is_empty() {
                entry.push(Value::from(item.corrected_from.as_str()));
            }
            Value::Array(entry)
        })
        .collect();
    let mut line = Map::new();
    line.insert("id".into(), Value::from(id));
    line.insert("input".into(), Value::from(input));
    line.insert("gold".into(), Value::from(gold));
    if !handled {
        line.insert("unhandled".into(), Value::from(true));
    }
    line.insert("n".into(), Value::from(view.candidates.len()));
    line.insert("c".into(), Value::Array(candidates));
    Value::Object(line)
}

/// Whether `path` holds the files the real sets were recorded with; lets a caller tell a wrong directory from an engine difference.
pub fn has_recorded_resources(path: &Path) -> bool {
    [
        msime_engine::assets::MAIN_DICTIONARY,
        msime_engine::assets::ENGLISH_DICTIONARY,
        msime_engine::assets::BIGRAM_TABLE,
        msime_engine::assets::TRIGRAM_TABLE,
    ]
    .iter()
    .all(|name| path.join(name).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_recorded_sets_have_the_documented_row_counts_and_shape() {
        for (file, count) in [
            ("sentences-v1.jsonl", 60),
            ("sentences-neutral-v1.jsonl", 1105),
            ("sentences-v2.jsonl", 312),
            ("quanpin-words-v1.subset3000.jsonl", 3000),
        ] {
            let rows = expected_rows(file);
            assert_eq!(rows.len(), count, "{file}");
            for row in rows {
                assert!(
                    row["input"].as_str().is_some_and(|input| input.is_ascii()),
                    "{row}"
                );
                let candidates = row["c"].as_array().unwrap();
                assert!(candidates.len() <= TOP, "{row}");
                assert!(
                    row["n"].as_u64().unwrap() as usize >= candidates.len(),
                    "{row}"
                );
                assert!(
                    candidates
                        .iter()
                        .all(|item| matches!(item.as_array().map(Vec::len), Some(2 | 3))),
                    "{row}"
                );
                assert_eq!(diff_row(&row, &row), None);
            }
        }
    }

    #[test]
    fn a_differing_row_reports_the_field_and_both_lists() {
        let expected = json!({"id": "s-001", "input": "nihao", "gold": "你好", "n": 3, "c": [["你好", 0], ["拟好", 0], ["你", 0, "nihoa"]]});
        let mut actual = expected.clone();
        actual["n"] = json!(4);
        actual["c"][1][0] = json!("泥好");
        assert_eq!(
            diff_row(&expected, &actual).unwrap(),
            "s-001 nihao: c[1][0]: expected \"拟好\", got \"泥好\"; n: expected 3, got 4\n    expected: 你好/0 拟好/0 你/0<-nihoa\n    got:      你好/0 泥好/0 你/0<-nihoa"
        );
        let mut unhandled = expected.clone();
        unhandled["unhandled"] = json!(true);
        assert!(diff_row(&expected, &unhandled)
            .unwrap()
            .contains("unhandled: expected <absent>, got true"));
    }

    #[test]
    fn product_options_follow_the_bridge_defaults() {
        let mut options = SessionOptions::new(RuntimePaths::default());
        product_options(&mut options);
        assert_eq!(options.scheme, SchemeType::Quanpin);
        assert!(!options.learning && options.sentence_alternatives && options.helpcode);
        assert_eq!(options.helpcode_schema, "ziranma");
        assert_eq!(options.frequency.mode, FrequencyAdjustmentMode::Promote);
        assert_eq!(
            options.english,
            EnglishInputOptions {
                mixed_candidates: true,
                minimum_prefix: 5
            }
        );
        assert_eq!(options.autocorrect_types, 0);
        assert_eq!(options.fuzzy_pinyin.rules, 0);
        assert!(!options.expressive.emoji_candidates && !options.expressive.kaomoji_candidates);
        assert_eq!(options.local_modes, LocalModeOptions::default());
        // Everything apply_product_options leaves alone keeps the reference default.
        assert!(options.personal_context);
    }
}
