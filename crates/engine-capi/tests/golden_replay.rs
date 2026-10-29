// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The engine's golden scenarios replayed through the C ABI and compared, step by step, with the same scenario replayed through the core Rust API, so the C library cannot drift from the engine without a failing test. msime's own hosts do not link this ABI (they use host-api's `msime_client.h`), so this replay is what exercises it the way an adopter would.
//!
//! The core side is the golden harness itself (`crates/engine/tests/golden_support`, included by path): `replay_scenario_observed` replays the scenario exactly as `crates/engine/tests/golden.rs` does and hands over the session after every step, where the test reads `display_snapshot`, the snapshot `msime_engine_state` promises. The C side stages the same fixture in its own root, prepares the generation with `msime_engine_prepare_runtime`, builds the session from the version 1 options JSON and drives every op through the exported functions. The two sides run one after the other, never interleaved, because the personal learning store and the connection cache are process-wide.
//!
//! After every step both sides are reduced to what the C ABI shows: the `msime_engine_state` fields, every `msime_engine_candidate`, the step's `msime_engine_result` in the recorder's shape, and the journal, SQL rows and personal dictionary pages the scenario reads. Scenarios whose options or ops the ABI does not take are skipped and counted by reason; a full run fails when fewer than `REPLAYED_FLOOR` scenarios replay, so coverage cannot quietly collapse. The real-dictionary sets run with `MSIME_EVAL_RESOURCES`, like the engine's golden tests.

// The test drives the library through its `extern "C"` functions, whose pointer arguments are the contract under test; the exemption is scoped to this test crate, like the crate's own `#![allow(unsafe_code)]`.
#![allow(unsafe_code)]

// The golden harness is shared, not copied. This replay uses only part of it, and its own unit tests run here as well as in the engine.
#[allow(dead_code)]
#[path = "../../engine/tests/golden_support/mod.rs"]
mod golden_support;

use std::collections::BTreeMap;
use std::ffi::{c_char, CStr};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, MutexGuard};

use golden_support::diff::{diff_documents, diff_values, DiffOptions};
use golden_support::fixture::stage_fixture;
use golden_support::real;
use golden_support::replay::{
    self, apply_options, keeps_page, moved_page, panic_message, read_json, reference_options,
    root_spellings, CANTONESE_DICTIONARY, STROKE_DICTIONARY, ZHUYIN_DICTIONARY,
};
use golden_support::snapshot::{journal_tables, query_rows, scrub};
use msime_engine::assets::USER_JOURNAL;
use msime_engine::vietnamese::{InputMethod, ToneStyle};
use msime_engine::{
    autocorrect_type, fuzzy_rule, CandidateSource, Command, FrequencyAdjustmentMode, KoreanLayout,
    LocalInputMode, LocalModeOptions, RuntimePaths, SchemeType, Session, SessionOptions,
    ShuangpinProfileKind, WubiProfileKind,
};
use msime_engine_capi::*;
use serde_json::{json, Map, Value};

/// 完整运行至少要经 C ABI 回放的场景数。落地时（2026-10-29）是 201，ABI 1.1 补上调频、混排、本地模式开关、`candidate_key`、`validate_entry` 和五笔混输的运行时开关后是 240；ABI 承载的选项或 op 变多时上调，不能为了让失败通过而下调。
const REPLAYED_FLOOR: usize = 240;

/// Differences printed per scenario before the rest is summarised as a count.
const MAX_DIFFS_PER_SCENARIO: usize = 40;

/// Differing rows printed per real set before the rest is summarised as a count.
const MAX_REPORTED_ROWS: usize = 50;

/// The personal learning store and the connection cache are process-wide, and each replay closes them when it ends, so the tests that open sessions take turns, as in the engine's `golden.rs`.
static ENGINE: Mutex<()> = Mutex::new(());

fn engine_lock() -> MutexGuard<'static, ()> {
    ENGINE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---- Scripted scenarios ----

#[test]
fn scripted_scenarios_match_the_core_api_through_the_c_abi() {
    let _engine = engine_lock();
    let scenarios = replay::selected_scenarios();
    let full_run = scenarios.len() == replay::selected_scenarios_from(None).len();
    let work = tempfile::tempdir().unwrap();
    let mut replayed = 0;
    let mut skipped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut failed = Vec::new();
    for path in &scenarios {
        let spec = read_json(path);
        let name = spec["name"].as_str().expect("scenario name").to_owned();
        if let Some(reason) = unsupported(&spec) {
            skipped.entry(reason).or_default().push(name);
            continue;
        }
        replayed += 1;
        let diffs = compare_scenario(&spec, work.path());
        if diffs.is_empty() {
            continue;
        }
        let mut report = format!(
            "{name}: {} differences (expected = core API, got = C ABI)",
            diffs.len()
        );
        for line in diffs.iter().take(MAX_DIFFS_PER_SCENARIO) {
            report.push_str("\n  ");
            report.push_str(line);
        }
        if diffs.len() > MAX_DIFFS_PER_SCENARIO {
            report.push_str(&format!(
                "\n  ... and {} more",
                diffs.len() - MAX_DIFFS_PER_SCENARIO
            ));
        }
        eprintln!("{report}");
        failed.push(name);
    }
    let summary = coverage_summary(replayed, scenarios.len(), &skipped);
    eprintln!("{summary}");
    assert!(
        failed.is_empty(),
        "{} of {replayed} replayed scenarios differ between the core API and the C ABI: {}",
        failed.len(),
        failed.join(", ")
    );
    if full_run {
        assert!(
            replayed >= REPLAYED_FLOOR,
            "only {replayed} scenarios replayed through the C ABI, below the floor of {REPLAYED_FLOOR}; an option or op the replay used to take is now skipped\n{summary}"
        );
    }
}

/// `C ABI golden replay: N of M scenarios replayed, K skipped`, then one line per reason with its count and scenario names.
fn coverage_summary(
    replayed: usize,
    total: usize,
    skipped: &BTreeMap<String, Vec<String>>,
) -> String {
    let skipped_count: usize = skipped.values().map(Vec::len).sum();
    let mut summary = format!(
        "C ABI golden replay: {replayed} of {total} scenarios replayed, {skipped_count} skipped"
    );
    let mut reasons: Vec<_> = skipped.iter().collect();
    reasons.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    for (reason, names) in reasons {
        summary.push_str(&format!(
            "\n  {:>3} {reason}: {}",
            names.len(),
            names.join(", ")
        ));
    }
    summary
}

/// C ABI 没有对应函数的 op，以及宿主能改用的办法（有的话）。
fn unsupported_op(op: &str) -> Option<&'static str> {
    Some(match op {
        "balance_paired_punctuation_after_auto_close" => {
            "op balance_paired_punctuation_after_auto_close"
        }
        "set_nine_key_enabled" | "set_key_grid" | "grid_key" | "choose_nine_key_spelling" => {
            "nine-key and key grids (not in ABI v1)"
        }
        "apply_online_candidates" => "online candidates (not in ABI v1)",
        "reset_cache" => "op reset_cache (msime_engine_release_caches is process-wide only)",
        _ => return None,
    })
}

/// Why the C ABI cannot replay this scenario, or `None` when it can: an option the version 1 JSON does not carry, an op without a C function, or an argument the C function refuses by contract.
fn unsupported(spec: &Value) -> Option<String> {
    let mut options = reference_options();
    apply_options(&mut options, spec.get("options").unwrap_or(&json!({})));
    if let Err(reason) = capi_options(&options) {
        return Some(reason);
    }
    for step in spec["steps"].as_array().expect("scenario steps") {
        let op = step["op"].as_str().expect("op");
        if let Some(reason) = unsupported_op(op) {
            return Some(reason.to_owned());
        }
        let arg = &step["arg"];
        match op {
            "type" | "char" => {
                if let Some(byte) = arg
                    .as_str()
                    .unwrap_or_default()
                    .bytes()
                    .find(|byte| !takes_key_char(*byte))
                {
                    return Some(format!("key_char refuses byte {byte:#04x}"));
                }
            }
            "punctuation" => {
                let byte = arg.as_str().unwrap_or_default().as_bytes()[0];
                if !byte.is_ascii_punctuation() {
                    return Some(format!("key_punct refuses byte {byte:#04x}"));
                }
            }
            "candidate_key" => {
                let byte = arg.as_str().unwrap_or_default().as_bytes()[0];
                if !byte.is_ascii_digit() {
                    return Some(format!("candidate_key refuses byte {byte:#04x}"));
                }
            }
            "reopen" => {
                if let Some(changed) = step.get("options") {
                    apply_options(&mut options, changed);
                    if let Err(reason) = capi_options(&options) {
                        return Some(format!("{reason} (on reopen)"));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// `msime_engine_key_char` takes printable ASCII and Tab.
fn takes_key_char(byte: u8) -> bool {
    byte == b'\t' || (0x20..=0x7e).contains(&byte)
}

/// Both replays of one scenario as comparable documents, diffed with the golden differ; a panic on either side is one difference.
fn compare_scenario(spec: &Value, work: &Path) -> Vec<String> {
    let core = panic::catch_unwind(AssertUnwindSafe(|| core_document(spec, &work.join("core"))));
    let core = match core {
        Ok(document) => document,
        Err(payload) => {
            let _ = panic::catch_unwind(msime_engine::close_cached_databases);
            return vec![format!(
                "core replay panicked: {}",
                panic_message(&*payload)
            )];
        }
    };
    let capi = panic::catch_unwind(AssertUnwindSafe(|| capi_document(spec, &work.join("capi"))));
    let capi = match capi {
        Ok(document) => document,
        Err(payload) => {
            let _ = panic::catch_unwind(msime_engine::close_cached_databases);
            return vec![format!(
                "C ABI replay panicked: {}",
                panic_message(&*payload)
            )];
        }
    };
    diff_documents(&core, &capi, DiffOptions::default())
}

/// The golden replay of the scenario, each step reduced to its op, argument, result and read data, plus the session's view after it.
fn core_document(spec: &Value, work: &Path) -> Value {
    let mut views = Vec::new();
    let recorded =
        replay::replay_scenario_observed(spec, work, |session| views.push(core_view(session)));
    let steps = recorded["steps"].as_array().expect("recorded steps");
    assert_eq!(steps.len(), views.len(), "one view per recorded step");
    let steps = steps
        .iter()
        .zip(views)
        .map(|(step, view)| {
            let mut out = Map::new();
            for key in ["op", "arg", "result", "journal", "rows"] {
                if let Some(value) = step.get(key) {
                    out.insert(key.into(), value.clone());
                }
            }
            out.insert("view".into(), view);
            Value::Object(out)
        })
        .collect();
    let mut document = Map::new();
    document.insert("name".into(), recorded["name"].clone());
    document.insert("steps".into(), Value::Array(steps));
    if let Some(journal) = recorded.get("final_journal") {
        document.insert("final_journal".into(), journal.clone());
    }
    Value::Object(document)
}

// ---- The comparable view ----

/// What the C ABI shows of a session, read from the core: `display_snapshot` (the snapshot `msime_engine_state` is documented to read), `takes_local_separator`, and the enum values mapped to the ABI's frozen constants here rather than cast, so a renumbered engine enum shows up as a difference.
fn core_view(session: &Session) -> Value {
    let view = session.display_snapshot();
    let candidates: Vec<Value> = view
        .candidates
        .iter()
        .enumerate()
        .map(|(index, item)| {
            candidate_json(CandidateView {
                word: item.word.as_str().to_owned(),
                code: item.pinyin.as_str().to_owned(),
                annotation: view
                    .candidate_annotations
                    .get(index)
                    .cloned()
                    .unwrap_or_default(),
                corrected_from: item.corrected_from.as_str().to_owned(),
                source: source_constant(item.source),
                scheme: scheme_constant(item.scheme),
                fixed_position: item.fixed_position,
                fuzzy: item.fuzzy,
                answers_key: view
                    .candidate_answers_key
                    .get(index)
                    .copied()
                    .unwrap_or(false),
                sentence: item.sentence_association,
            })
        })
        .collect();
    json!({
        "scheme": scheme_constant(view.scheme),
        "local_mode": local_mode_constant(view.local_mode),
        "prediction": view.prediction,
        "dedicated_english": view.dedicated_english,
        "candidate_list_open": view.candidate_list_open,
        "takes_separator": session.takes_local_separator(),
        "caret_byte": view.caret_position,
        "conversion_focus_scalar": [view.conversion_focus.0, view.conversion_focus.1],
        "preedit": view.preedit,
        "editing_text": view.editing_text,
        "conversion": view.conversion,
        "spelling_symbols": view.spelling_symbols,
        "candidate_count": view.candidates.len(),
        "candidates": candidates,
    })
}

struct CandidateView {
    word: String,
    code: String,
    annotation: String,
    corrected_from: String,
    source: u32,
    scheme: u32,
    fixed_position: i32,
    fuzzy: bool,
    answers_key: bool,
    sentence: bool,
}

fn candidate_json(candidate: CandidateView) -> Value {
    json!({
        "word": candidate.word,
        "code": candidate.code,
        "annotation": candidate.annotation,
        "corrected_from": candidate.corrected_from,
        "source": candidate.source,
        "scheme": candidate.scheme,
        "fixed_position": candidate.fixed_position,
        "fuzzy": candidate.fuzzy,
        "answers_key": candidate.answers_key,
        "sentence": candidate.sentence,
    })
}

fn scheme_constant(scheme: SchemeType) -> u32 {
    match scheme {
        SchemeType::Quanpin => MSIME_ENGINE_SCHEME_QUANPIN,
        SchemeType::Shuangpin => MSIME_ENGINE_SCHEME_SHUANGPIN,
        SchemeType::Wubi => MSIME_ENGINE_SCHEME_WUBI,
        SchemeType::JapaneseRomaji => MSIME_ENGINE_SCHEME_JAPANESE,
        SchemeType::Korean => MSIME_ENGINE_SCHEME_KOREAN,
        SchemeType::Cantonese => MSIME_ENGINE_SCHEME_CANTONESE,
        SchemeType::Zhuyin => MSIME_ENGINE_SCHEME_ZHUYIN,
        SchemeType::Vietnamese => MSIME_ENGINE_SCHEME_VIETNAMESE,
        SchemeType::Tibetan => MSIME_ENGINE_SCHEME_TIBETAN,
        SchemeType::Stroke => MSIME_ENGINE_SCHEME_STROKE,
    }
}

fn local_mode_constant(mode: LocalInputMode) -> u32 {
    match mode {
        LocalInputMode::None => MSIME_ENGINE_LOCAL_MODE_NONE,
        LocalInputMode::Unicode => MSIME_ENGINE_LOCAL_MODE_UNICODE,
        LocalInputMode::DateTime => MSIME_ENGINE_LOCAL_MODE_DATE_TIME,
        LocalInputMode::QuickPhrase => MSIME_ENGINE_LOCAL_MODE_QUICK_PHRASE,
        LocalInputMode::Emoji => MSIME_ENGINE_LOCAL_MODE_EMOJI,
        LocalInputMode::Kaomoji => MSIME_ENGINE_LOCAL_MODE_KAOMOJI,
        LocalInputMode::SuperJianpin => MSIME_ENGINE_LOCAL_MODE_SUPER_JIANPIN,
        LocalInputMode::TemporaryEnglish => MSIME_ENGINE_LOCAL_MODE_TEMPORARY_ENGLISH,
        LocalInputMode::TemporaryJapanese => MSIME_ENGINE_LOCAL_MODE_TEMPORARY_JAPANESE,
        LocalInputMode::Expression => MSIME_ENGINE_LOCAL_MODE_EXPRESSION,
        LocalInputMode::Command => MSIME_ENGINE_LOCAL_MODE_COMMAND,
        LocalInputMode::Mention => MSIME_ENGINE_LOCAL_MODE_MENTION,
        LocalInputMode::Url => MSIME_ENGINE_LOCAL_MODE_URL,
        LocalInputMode::Chaizi => MSIME_ENGINE_LOCAL_MODE_CHAIZI,
        LocalInputMode::Symbol => MSIME_ENGINE_LOCAL_MODE_SYMBOL,
    }
}

fn source_constant(source: CandidateSource) -> u32 {
    match source {
        CandidateSource::Database => MSIME_ENGINE_SOURCE_DATABASE,
        CandidateSource::UserDatabase => MSIME_ENGINE_SOURCE_USER_DATABASE,
        CandidateSource::CloudSuggestion => MSIME_ENGINE_SOURCE_CLOUD_SUGGESTION,
        CandidateSource::AiSuggestion => MSIME_ENGINE_SOURCE_AI_SUGGESTION,
        CandidateSource::EnglishDictionary => MSIME_ENGINE_SOURCE_ENGLISH_DICTIONARY,
        CandidateSource::QuickPhrase => MSIME_ENGINE_SOURCE_QUICK_PHRASE,
        CandidateSource::Emoji => MSIME_ENGINE_SOURCE_EMOJI,
        CandidateSource::Kaomoji => MSIME_ENGINE_SOURCE_KAOMOJI,
        CandidateSource::Generated => MSIME_ENGINE_SOURCE_GENERATED,
        CandidateSource::Fallback => MSIME_ENGINE_SOURCE_FALLBACK,
        CandidateSource::NeuralKeyboard => MSIME_ENGINE_SOURCE_NEURAL_KEYBOARD,
        CandidateSource::AiPending => MSIME_ENGINE_SOURCE_AI_PENDING,
        CandidateSource::MixedJapanese => MSIME_ENGINE_SOURCE_MIXED_JAPANESE,
        CandidateSource::Plugin => MSIME_ENGINE_SOURCE_PLUGIN,
        CandidateSource::CloudPending => MSIME_ENGINE_SOURCE_CLOUD_PENDING,
        CandidateSource::Contact => MSIME_ENGINE_SOURCE_CONTACT,
        CandidateSource::Collocation => MSIME_ENGINE_SOURCE_COLLOCATION,
        CandidateSource::Multilingual => MSIME_ENGINE_SOURCE_MULTILINGUAL,
    }
}

fn command_constant(command: Command) -> u32 {
    match command {
        Command::Backspace => MSIME_ENGINE_COMMAND_BACKSPACE,
        Command::CommitCandidate => MSIME_ENGINE_COMMAND_COMMIT_CANDIDATE,
        Command::CommitRaw => MSIME_ENGINE_COMMAND_COMMIT_RAW,
        Command::Cancel => MSIME_ENGINE_COMMAND_CANCEL,
        Command::MoveLeft => MSIME_ENGINE_COMMAND_MOVE_LEFT,
        Command::MoveRight => MSIME_ENGINE_COMMAND_MOVE_RIGHT,
        Command::MoveHome => MSIME_ENGINE_COMMAND_MOVE_HOME,
        Command::MoveEnd => MSIME_ENGINE_COMMAND_MOVE_END,
        Command::DeleteForward => MSIME_ENGINE_COMMAND_DELETE_FORWARD,
        Command::CycleKanaVariant => MSIME_ENGINE_COMMAND_CYCLE_KANA_VARIANT,
        Command::CommitReading => MSIME_ENGINE_COMMAND_COMMIT_READING,
        Command::ConvertHanja => MSIME_ENGINE_COMMAND_OPEN_CANDIDATE_LIST,
        Command::ConversionLeft => MSIME_ENGINE_COMMAND_CONVERSION_LEFT,
        Command::ConversionRight => MSIME_ENGINE_COMMAND_CONVERSION_RIGHT,
        Command::CommitRawWithoutLearning => MSIME_ENGINE_COMMAND_COMMIT_RAW_WITHOUT_LEARNING,
    }
}

// ---- Options: `SessionOptions` to the version 1 JSON ----

/// The version 1 options JSON that builds the same session as `options`, with every key the JSON has written out rather than left to the library's defaults, or the reason it cannot: every field the JSON does not carry must still hold the `SessionOptions::new` value, which is what the C library starts from.
fn capi_options(options: &SessionOptions) -> Result<Value, String> {
    let defaults = SessionOptions::new(options.paths.clone());
    let mut rest = options.clone();
    rest.scheme = defaults.scheme;
    rest.enabled_schemes = defaults.enabled_schemes;
    rest.shuangpin_profile = defaults.shuangpin_profile;
    rest.shuangpin_preedit_uses_raw = defaults.shuangpin_preedit_uses_raw;
    rest.wubi = defaults.wubi;
    rest.vietnamese_input_method = defaults.vietnamese_input_method;
    rest.vietnamese_tone_style = defaults.vietnamese_tone_style;
    rest.korean_layout = defaults.korean_layout;
    if !Session::is_supported_helpcode_schema(&options.helpcode_schema) {
        return Err(format!(
            "helpcode schema {:?} (the options JSON refuses names it does not know)",
            options.helpcode_schema
        ));
    }
    rest.helpcode = defaults.helpcode;
    rest.helpcode_schema = defaults.helpcode_schema.clone();
    rest.chinese_punctuation = defaults.chinese_punctuation;
    rest.paired_punctuation = defaults.paired_punctuation;
    if (0..=2).contains(&options.punctuation_lock) {
        rest.punctuation_lock = defaults.punctuation_lock;
    }
    rest.learning = defaults.learning;
    if (1..=10).contains(&options.frequency.trigger_count)
        && (1..=10).contains(&options.frequency.linear_step)
    {
        rest.frequency = defaults.frequency;
    }
    rest.expressive = defaults.expressive;
    // `/` 和 `@` 两个模式不在选项 JSON 里，其余开关都在。
    rest.local_modes = LocalModeOptions {
        command: options.local_modes.command,
        mention: options.local_modes.mention,
        ..defaults.local_modes
    };
    rest.learn_english_on_enter = defaults.learn_english_on_enter;
    rest.learn_typed_spellings_on_enter = defaults.learn_typed_spellings_on_enter;
    rest.personal_context = defaults.personal_context;
    rest.next_word_prediction = defaults.next_word_prediction;
    if options.fuzzy_pinyin.rules & !fuzzy_rule::ALL == 0 {
        rest.fuzzy_pinyin = defaults.fuzzy_pinyin;
    }
    if options.autocorrect_types & !AUTOCORRECT_ALL == 0 {
        rest.autocorrect_types = defaults.autocorrect_types;
    }
    if (1..=8).contains(&options.english.minimum_prefix)
        && (1..=3).contains(&options.english.first_page_count)
    {
        rest.english = defaults.english;
    }
    if (1..=255).contains(&options.first_page_rows) {
        rest.first_page_rows = defaults.first_page_rows;
    }
    rest.cantonese_dictionary = defaults.cantonese_dictionary.clone();
    rest.zhuyin_dictionary = defaults.zhuyin_dictionary.clone();
    rest.stroke_dictionary = defaults.stroke_dictionary.clone();
    rest.japanese_dictionary = defaults.japanese_dictionary.clone();
    if rest != defaults {
        return Err(format!(
            "option {} (not in the options JSON)",
            first_differing_field(&rest, &defaults)
        ));
    }
    let path = |path: &Path| path.to_str().expect("UTF-8 path").to_owned();
    let mut languages = Map::new();
    for (key, dictionary) in [
        ("cantonese", &options.cantonese_dictionary),
        ("zhuyin", &options.zhuyin_dictionary),
        ("stroke", &options.stroke_dictionary),
        ("japanese", &options.japanese_dictionary),
    ] {
        if !dictionary.as_os_str().is_empty() {
            languages.insert(key.into(), Value::from(path(dictionary)));
        }
    }
    let enabled: Vec<&str> = (0..=u8::MAX)
        .filter_map(SchemeType::from_u8)
        .filter(|scheme| options.enabled_schemes.contains(*scheme))
        .map(scheme_name)
        .collect();
    Ok(json!({
        "version": 1,
        "paths": {
            "resources": path(&options.paths.resources),
            "user_data": path(&options.paths.user_data),
            "cache": path(&options.paths.cache),
            "dictionaries": path(&options.paths.dictionaries),
        },
        "scheme": scheme_name(options.scheme),
        "enabled_schemes": enabled,
        "shuangpin": {
            "profile": shuangpin_profile_name(options.shuangpin_profile),
            "preedit_shows_keys": options.shuangpin_preedit_uses_raw,
        },
        "wubi": {
            "profile": match options.wubi.profile {
                WubiProfileKind::Wubi86 => "wubi86",
                WubiProfileKind::Wubi98 => "wubi98",
            },
            "mixed_pinyin": options.wubi.mixed_pinyin,
            "z_wildcard": options.wubi.z_wildcard,
        },
        "vietnamese": {
            "input_method": match options.vietnamese_input_method {
                InputMethod::Telex => "telex",
                InputMethod::Vni => "vni",
            },
            "tone_style": match options.vietnamese_tone_style {
                ToneStyle::Modern => "modern",
                ToneStyle::Classic => "classic",
            },
        },
        "korean": {
            "layout": match options.korean_layout {
                KoreanLayout::Dubeolsik => "dubeolsik",
                KoreanLayout::Romaja => "romaja",
            },
        },
        "helpcode": {"enabled": options.helpcode, "schema": options.helpcode_schema},
        "punctuation": {
            "chinese": options.chinese_punctuation,
            "paired": options.paired_punctuation,
            "lock": PUNCTUATION_LOCKS[options.punctuation_lock as usize],
        },
        "learning": options.learning,
        "personal_context": options.personal_context,
        "next_word_prediction": options.next_word_prediction,
        "fuzzy_pinyin": FUZZY_RULES
            .iter()
            .filter(|(bit, _)| options.fuzzy_pinyin.rules & bit != 0)
            .map(|(_, name)| *name)
            .collect::<Vec<_>>(),
        "autocorrect": AUTOCORRECT_TYPES
            .iter()
            .filter(|(bit, _)| options.autocorrect_types & bit != 0)
            .map(|(_, name)| *name)
            .collect::<Vec<_>>(),
        "english": {
            "mixed_candidates": options.english.mixed_candidates,
            "minimum_prefix": options.english.minimum_prefix,
            "first_page_count": options.english.first_page_count,
            "learn_on_enter": options.learn_english_on_enter,
            "learn_typed_spellings_on_enter": options.learn_typed_spellings_on_enter,
        },
        "first_page_rows": options.first_page_rows,
        "language_dictionaries": languages,
        "frequency": {
            "mode": options.frequency.mode.name(),
            "trigger_count": options.frequency.trigger_count,
            "linear_step": options.frequency.linear_step,
        },
        "expressive": {
            "emoji_candidates": options.expressive.emoji_candidates,
            "kaomoji_candidates": options.expressive.kaomoji_candidates,
            "symbol_candidates": options.expressive.symbol_candidates,
        },
        "local_modes": local_modes_json(&options.local_modes),
    }))
}

/// 选项 JSON 的 `local_modes`：除 `/`、`@` 以外的每个开关都写出来。
fn local_modes_json(modes: &LocalModeOptions) -> Value {
    json!({
        "unicode": modes.unicode,
        "date_time": modes.date_time,
        "date_time_inline": modes.date_time_inline,
        "date_time_menu": modes.date_time_menu,
        "quick_phrase": modes.quick_phrase,
        "emoji": modes.emoji,
        "kaomoji": modes.kaomoji,
        "super_jianpin": modes.super_jianpin,
        "temporary_english": modes.temporary_english,
        "temporary_japanese": modes.temporary_japanese,
        "expression": modes.expression,
        "chaizi": modes.chaizi,
        "symbol": modes.symbol,
        "shift_letter_entry": modes.shift_letter_entry,
    })
}

/// `punctuation.lock` by `SessionOptions::punctuation_lock`.
const PUNCTUATION_LOCKS: [&str; 3] = ["follow", "chinese", "ascii"];

const FUZZY_RULES: [(u32, &str); 11] = [
    (fuzzy_rule::Z_ZH, "z_zh"),
    (fuzzy_rule::C_CH, "c_ch"),
    (fuzzy_rule::S_SH, "s_sh"),
    (fuzzy_rule::N_L, "n_l"),
    (fuzzy_rule::F_H, "f_h"),
    (fuzzy_rule::R_L, "r_l"),
    (fuzzy_rule::AN_ANG, "an_ang"),
    (fuzzy_rule::EN_ENG, "en_eng"),
    (fuzzy_rule::IN_ING, "in_ing"),
    (fuzzy_rule::IAN_IANG, "ian_iang"),
    (fuzzy_rule::UAN_UANG, "uan_uang"),
];

const AUTOCORRECT_TYPES: [(u32, &str); 4] = [
    (autocorrect_type::TRANSPOSITION, "transposition"),
    (autocorrect_type::NEIGHBOR, "neighbor"),
    (autocorrect_type::DELETION, "deletion"),
    (autocorrect_type::INSERTION, "insertion"),
];

const AUTOCORRECT_ALL: u32 = autocorrect_type::TRANSPOSITION
    | autocorrect_type::NEIGHBOR
    | autocorrect_type::DELETION
    | autocorrect_type::INSERTION;

fn scheme_name(scheme: SchemeType) -> &'static str {
    match scheme {
        SchemeType::Quanpin => "quanpin",
        SchemeType::Shuangpin => "shuangpin",
        SchemeType::Wubi => "wubi",
        SchemeType::JapaneseRomaji => "japanese",
        SchemeType::Korean => "korean",
        SchemeType::Cantonese => "cantonese",
        SchemeType::Zhuyin => "zhuyin",
        SchemeType::Vietnamese => "vietnamese",
        SchemeType::Tibetan => "tibetan",
        SchemeType::Stroke => "stroke",
    }
}

fn shuangpin_profile_name(profile: ShuangpinProfileKind) -> &'static str {
    match profile {
        ShuangpinProfileKind::Xiaohe => "xiaohe",
        ShuangpinProfileKind::Ziranma => "ziranma",
        ShuangpinProfileKind::Shoudao => "shoudao",
        ShuangpinProfileKind::Microsoft => "microsoft",
        ShuangpinProfileKind::Custom => "custom",
        ShuangpinProfileKind::SmartAbc => "smart_abc",
    }
}

/// The top-level `SessionOptions` field where `a` first differs from `b`, read off their pretty `Debug` forms (four-space indented `name: value` lines), so a field added to the engine later is named without this test listing every field.
fn first_differing_field(a: &SessionOptions, b: &SessionOptions) -> String {
    let a = format!("{a:#?}");
    let b = format!("{b:#?}");
    let mut field = "?";
    for (line_a, line_b) in a.lines().zip(b.lines()) {
        if let Some(name) = line_a
            .strip_prefix("    ")
            .filter(|rest| !rest.starts_with(' '))
            .and_then(|rest| rest.split_once(':'))
            .map(|(name, _)| name)
        {
            field = name;
        }
        if line_a != line_b {
            break;
        }
    }
    field.to_owned()
}

// ---- The C side ----

fn borrowed(text: &str) -> msime_engine_str {
    msime_engine_str {
        ptr: text.as_ptr().cast(),
        len: text.len(),
    }
}

fn empty_str() -> msime_engine_str {
    msime_engine_str {
        ptr: ptr::null(),
        len: 0,
    }
}

/// A string the library lent out: `len` bytes of UTF-8, valid until the session's next state-changing call.
fn lent(text: msime_engine_str) -> String {
    if text.len == 0 {
        return String::new();
    }
    // SAFETY: the library lends `len` readable bytes, and the caller reads them before the next state-changing call.
    let bytes = unsafe { std::slice::from_raw_parts(text.ptr.cast::<u8>(), text.len) };
    String::from_utf8(bytes.to_vec()).expect("the library lends UTF-8")
}

fn last_error() -> String {
    // SAFETY: the library returns a NUL-terminated string valid until this thread's next call into it.
    unsafe { CStr::from_ptr(msime_engine_last_error()) }
        .to_string_lossy()
        .into_owned()
}

/// Takes ownership of a string the library allocated and frees it.
fn take_string(text: *mut c_char) -> String {
    assert!(!text.is_null(), "the library returned no string");
    // SAFETY: a NUL-terminated string from the library, read once and freed once.
    let owned = unsafe { CStr::from_ptr(text) }
        .to_str()
        .expect("UTF-8")
        .to_owned();
    // SAFETY: as above.
    unsafe { msime_engine_string_free(text) };
    owned
}

fn new_result() -> msime_engine_result {
    msime_engine_result {
        struct_size: size_of::<msime_engine_result>() as u32,
        handled: 0,
        has_commit: 0,
        diagnostic_code: 0,
        revision: 0,
        commit: empty_str(),
        diagnostic: empty_str(),
    }
}

/// The four roots as the C functions take them.
struct CapiPaths {
    resources: String,
    user_data: String,
    cache: String,
    dictionaries: String,
}

impl CapiPaths {
    fn abi(&self) -> msime_engine_paths {
        msime_engine_paths {
            struct_size: size_of::<msime_engine_paths>() as u32,
            resources: borrowed(&self.resources),
            user_data: borrowed(&self.user_data),
            cache: borrowed(&self.cache),
            dictionaries: borrowed(&self.dictionaries),
        }
    }

    fn runtime(&self) -> RuntimePaths {
        RuntimePaths {
            resources: PathBuf::from(&self.resources),
            user_data: PathBuf::from(&self.user_data),
            cache: PathBuf::from(&self.cache),
            dictionaries: PathBuf::from(&self.dictionaries),
        }
    }

    /// `msime_engine_prepare_runtime` on the first three roots; fills `dictionaries` with the generation it hands back.
    fn prepare(&mut self, content_id: &str) -> Result<(), String> {
        let mut directory: *mut c_char = ptr::null_mut();
        // SAFETY: the paths, the id and the output pointer all live across the call.
        let status = unsafe {
            msime_engine_prepare_runtime(
                &self.abi(),
                content_id.as_ptr().cast(),
                content_id.len(),
                &mut directory,
            )
        };
        if status != MSIME_ENGINE_OK {
            return Err(last_error());
        }
        self.dictionaries = take_string(directory);
        Ok(())
    }
}

fn utf8(path: &Path) -> String {
    path.to_str().expect("UTF-8 path").to_owned()
}

/// A session handle that is freed when dropped, so a replay that panics still gives back its dictionary lease.
struct CapiSession(*mut msime_engine_session);

impl CapiSession {
    fn new(options: &Value) -> Result<Self, String> {
        let text = options.to_string();
        let mut session = ptr::null_mut();
        // SAFETY: the JSON and the output pointer live across the call.
        let status =
            unsafe { msime_engine_session_new(text.as_ptr().cast(), text.len(), &mut session) };
        if status != MSIME_ENGINE_OK {
            return Err(format!("status {status}: {}", last_error()));
        }
        Ok(Self(session))
    }

    fn state(&self) -> msime_engine_state {
        let mut state = msime_engine_state {
            struct_size: size_of::<msime_engine_state>() as u32,
            scheme: 0,
            revision: 0,
            candidate_count: 0,
            flags: 0,
            local_mode: 0,
            caret_byte: 0,
            conversion_focus_start_scalar: 0,
            conversion_focus_end_scalar: 0,
            preedit: empty_str(),
            editing_text: empty_str(),
            conversion: empty_str(),
            spelling_symbols: empty_str(),
        };
        // SAFETY: a live handle and a full-size output struct.
        let status = unsafe { msime_engine_state_get(self.0, &mut state) };
        assert_eq!(status, MSIME_ENGINE_OK, "state_get: {}", last_error());
        state
    }

    fn candidate(&self, revision: u64, index: u32) -> msime_engine_candidate {
        let mut candidate = msime_engine_candidate {
            struct_size: size_of::<msime_engine_candidate>() as u32,
            index: 0,
            source: 0,
            scheme: 0,
            flags: 0,
            fixed_position: 0,
            word: empty_str(),
            code: empty_str(),
            annotation: empty_str(),
            corrected_from: empty_str(),
        };
        // SAFETY: a live handle and a full-size output struct.
        let status = unsafe { msime_engine_candidate_get(self.0, revision, index, &mut candidate) };
        assert_eq!(
            status,
            MSIME_ENGINE_OK,
            "candidate_get({index}): {}",
            last_error()
        );
        assert_eq!(candidate.index, index, "candidate_get echoes the index");
        candidate
    }

    /// The same shape as [`core_view`], read through `msime_engine_state_get` and `msime_engine_candidate_get`.
    fn view(&self) -> Value {
        let state = self.state();
        let known = MSIME_ENGINE_STATE_PREDICTION
            | MSIME_ENGINE_STATE_DEDICATED_ENGLISH
            | MSIME_ENGINE_STATE_CANDIDATE_LIST_OPEN
            | MSIME_ENGINE_STATE_TAKES_SEPARATOR;
        assert_eq!(state.flags & !known, 0, "state flags outside the ABI");
        let candidates: Vec<Value> = (0..state.candidate_count)
            .map(|index| {
                let candidate = self.candidate(state.revision, index);
                let known = MSIME_ENGINE_CANDIDATE_FUZZY
                    | MSIME_ENGINE_CANDIDATE_ANSWERS_KEY
                    | MSIME_ENGINE_CANDIDATE_SENTENCE;
                assert_eq!(
                    candidate.flags & !known,
                    0,
                    "candidate flags outside the ABI"
                );
                candidate_json(CandidateView {
                    word: lent(candidate.word),
                    code: lent(candidate.code),
                    annotation: lent(candidate.annotation),
                    corrected_from: lent(candidate.corrected_from),
                    source: candidate.source,
                    scheme: candidate.scheme,
                    fixed_position: candidate.fixed_position,
                    fuzzy: candidate.flags & MSIME_ENGINE_CANDIDATE_FUZZY != 0,
                    answers_key: candidate.flags & MSIME_ENGINE_CANDIDATE_ANSWERS_KEY != 0,
                    sentence: candidate.flags & MSIME_ENGINE_CANDIDATE_SENTENCE != 0,
                })
            })
            .collect();
        json!({
            "scheme": state.scheme,
            "local_mode": state.local_mode,
            "prediction": state.flags & MSIME_ENGINE_STATE_PREDICTION != 0,
            "dedicated_english": state.flags & MSIME_ENGINE_STATE_DEDICATED_ENGLISH != 0,
            "candidate_list_open": state.flags & MSIME_ENGINE_STATE_CANDIDATE_LIST_OPEN != 0,
            "takes_separator": state.flags & MSIME_ENGINE_STATE_TAKES_SEPARATOR != 0,
            "caret_byte": state.caret_byte,
            "conversion_focus_scalar": [state.conversion_focus_start_scalar, state.conversion_focus_end_scalar],
            "preedit": lent(state.preedit),
            "editing_text": lent(state.editing_text),
            "conversion": lent(state.conversion),
            "spelling_symbols": lent(state.spelling_symbols),
            "candidate_count": state.candidate_count,
            "candidates": candidates,
        })
    }

    /// Editing text and candidate words, which a host watches to send its page back to 0.
    fn view_key(&self) -> (String, Vec<String>) {
        let state = self.state();
        let words = (0..state.candidate_count)
            .map(|index| lent(self.candidate(state.revision, index).word))
            .collect();
        (lent(state.editing_text), words)
    }

    fn word_index(&self, word: &str) -> u32 {
        let state = self.state();
        (0..state.candidate_count)
            .find(|index| lent(self.candidate(state.revision, *index).word) == word)
            .unwrap_or_else(|| panic!("step selects a word that is not a candidate: {word}"))
    }

    /// Runs one result-returning call and checks the revision it reports against `state_get`.
    fn keyed(
        &self,
        call: impl FnOnce(*mut msime_engine_session, *mut msime_engine_result) -> i32,
    ) -> Result<msime_engine_result, Value> {
        let mut result = new_result();
        let status = call(self.0, &mut result);
        if status != MSIME_ENGINE_OK {
            return Err(json!({"status": status, "error": last_error()}));
        }
        assert_eq!(
            result.revision,
            self.state().revision,
            "result.revision is the state's revision"
        );
        Ok(result)
    }

    fn set_int(&self, option: u32, value: i64) -> Option<Value> {
        // SAFETY: a live handle.
        let status = unsafe { msime_engine_set_int(self.0, option, value) };
        (status != MSIME_ENGINE_OK).then(|| json!({"status": status, "error": last_error()}))
    }
}

impl Drop for CapiSession {
    fn drop(&mut self) {
        // SAFETY: the handle came from `msime_engine_session_new` and is freed once, here.
        let status = unsafe { msime_engine_session_free(self.0) };
        // A replay that is already unwinding reports its own panic; a second one would abort the test binary.
        if !std::thread::panicking() {
            assert_eq!(status, MSIME_ENGINE_OK, "session_free: {}", last_error());
        }
    }
}

fn flush_learning() {
    assert_eq!(msime_engine_flush_learning(0), MSIME_ENGINE_OK);
}

/// `{handled, commit?, diagnostic?}` from a C result, in the recorder's shape.
fn result_json(result: &msime_engine_result, roots: &[String]) -> Value {
    let mut out = Map::new();
    out.insert("handled".into(), Value::from(result.handled != 0));
    if result.has_commit != 0 {
        out.insert("commit".into(), Value::from(lent(result.commit)));
    }
    let diagnostic = lent(result.diagnostic);
    if !diagnostic.is_empty() {
        out.insert("diagnostic".into(), Value::from(scrub(&diagnostic, roots)));
    }
    Value::Object(out)
}

/// The scenario replayed through the C ABI, mirroring the golden harness's `Scenario`: the same root layout, fixture, generation and options, and every op through the exported functions.
struct CapiScenario {
    root: PathBuf,
    resources: PathBuf,
    roots: Vec<String>,
    options: SessionOptions,
    content_id: String,
    page_size: usize,
    page: usize,
    paths: CapiPaths,
    session: Option<CapiSession>,
}

fn capi_document(spec: &Value, work: &Path) -> Value {
    let name = spec["name"].as_str().expect("scenario name");
    let root = std::path::absolute(work.join(name)).unwrap();
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let options_spec = spec.get("options").cloned().unwrap_or_else(|| json!({}));
    let mut options = reference_options();
    apply_options(&mut options, &options_spec);
    let resources = root.join("resources");
    let mut scenario = CapiScenario {
        roots: root_spellings(&root),
        paths: CapiPaths {
            resources: utf8(&resources),
            user_data: utf8(&root.join("user")),
            cache: utf8(&root.join("cache")),
            dictionaries: String::new(),
        },
        resources,
        root,
        options,
        content_id: options_spec["content_id"]
            .as_str()
            .unwrap_or("v1")
            .to_owned(),
        page_size: options_spec["page_size"]
            .as_u64()
            .map_or(0, |size| size as usize),
        page: 0,
        session: None,
    };
    stage_fixture(&spec["fixture"], &scenario.resources);
    scenario.open();
    let mut steps = vec![json!({"op": "open", "view": scenario.session().view()})];
    for step in spec["steps"].as_array().expect("scenario steps") {
        steps.push(scenario.run_step(step));
    }
    scenario.close();
    let mut out = Map::new();
    out.insert("name".into(), Value::from(name));
    out.insert("steps".into(), Value::Array(steps));
    if spec["dump_journal_at_end"].as_bool().unwrap_or(false) {
        out.insert(
            "final_journal".into(),
            journal_tables(&scenario.paths.runtime().user(USER_JOURNAL)),
        );
    }
    assert_eq!(msime_engine_close_databases(), MSIME_ENGINE_OK);
    std::fs::remove_dir_all(&scenario.root).unwrap();
    Value::Object(out)
}

impl CapiScenario {
    /// `msime_engine_prepare_runtime`, then `msime_engine_session_new` with the options JSON, as the harness's `open` does with the core calls.
    fn open(&mut self) {
        self.paths
            .prepare(&self.content_id)
            .unwrap_or_else(|error| {
                panic!(
                    "prepare_runtime_paths failed: {}",
                    scrub(&error, &self.roots)
                )
            });
        let mut options = self.options.clone();
        options.paths = self.paths.runtime();
        for (name, slot) in [
            (CANTONESE_DICTIONARY, &mut options.cantonese_dictionary),
            (ZHUYIN_DICTIONARY, &mut options.zhuyin_dictionary),
            (STROKE_DICTIONARY, &mut options.stroke_dictionary),
        ] {
            let path = self.resources.join(name);
            if path.exists() {
                *slot = path;
            }
        }
        let json = capi_options(&options).expect("options checked before the replay");
        self.session =
            Some(CapiSession::new(&json).unwrap_or_else(|error| {
                panic!("Session::new failed: {}", scrub(&error, &self.roots))
            }));
        self.page = 0;
    }

    /// Free the session, then write the delayed personal-context rows, as the harness's `close`.
    fn close(&mut self) {
        self.session = None;
        flush_learning();
    }

    fn session(&self) -> &CapiSession {
        self.session.as_ref().expect("an open session")
    }

    fn index_arg(&self, arg: &Value) -> u32 {
        match arg.as_u64() {
            Some(index) => u32::try_from(index).expect("index fits in u32"),
            None => self
                .session()
                .word_index(arg["word"].as_str().expect("{word}")),
        }
    }

    fn run_step(&mut self, step: &Value) -> Value {
        let op = step["op"].as_str().expect("op").to_owned();
        let before = self.session().view_key();
        let arg = step.get("arg").cloned().unwrap_or(Value::Null);
        let mut out = Map::new();
        out.insert("op".into(), Value::from(op.as_str()));
        if !arg.is_null() {
            out.insert("arg".into(), arg.clone());
        }
        let result = self.dispatch(&op, &arg, step, &mut out);
        if !keeps_page(&op) && self.session().view_key() != before {
            self.page = 0;
        }
        if let Some(result) = result {
            out.insert("result".into(), result);
        }
        out.insert("view".into(), self.session().view());
        Value::Object(out)
    }

    /// A selection-type call on the row the step names, at the current revision.
    fn on_row(
        &self,
        index: u32,
        call: impl FnOnce(*mut msime_engine_session, u64, u32, *mut msime_engine_result) -> i32,
    ) -> Option<Value> {
        let revision = self.session().state().revision;
        let outcome = self
            .session()
            .keyed(|session, result| call(session, revision, index, result));
        Some(match outcome {
            Ok(result) => result_json(&result, &self.roots),
            Err(error) => error,
        })
    }

    fn dispatch(
        &mut self,
        op: &str,
        arg: &Value,
        step: &Value,
        out: &mut Map<String, Value>,
    ) -> Option<Value> {
        let text = || arg.as_str().expect("a string argument");
        let flag = || i64::from(arg.as_bool().expect("a bool argument"));
        match op {
            "type" => Some(self.type_text(text())),
            "char" => {
                assert_eq!(text().len(), 1, "char takes one byte");
                let flags = if step["shift"].as_bool().unwrap_or(false) {
                    MSIME_ENGINE_KEY_SHIFT_ONLY
                } else {
                    0
                };
                let byte = text().as_bytes()[0];
                // SAFETY: a live handle and the result the helper passes.
                let outcome = self.session().keyed(|session, result| unsafe {
                    msime_engine_key_char(session, byte, flags, result)
                });
                Some(outcome.map_or_else(|error| error, |result| result_json(&result, &self.roots)))
            }
            "punctuation" => {
                let byte = text().as_bytes()[0];
                // SAFETY: as above.
                let outcome = self.session().keyed(|session, result| unsafe {
                    msime_engine_key_punct(session, byte, result)
                });
                Some(outcome.map_or_else(|error| error, |result| result_json(&result, &self.roots)))
            }
            "command" => {
                let command = command_constant(
                    Command::from_name(text()).unwrap_or_else(|| panic!("unknown command {arg}")),
                );
                // SAFETY: as above.
                let outcome = self.session().keyed(|session, result| unsafe {
                    msime_engine_command(session, command, result)
                });
                Some(outcome.map_or_else(|error| error, |result| result_json(&result, &self.roots)))
            }
            "candidate_key" => {
                let byte = text().as_bytes()[0];
                // SAFETY: 句柄有效，结果结构由 `keyed` 交来。
                let outcome = self.session().keyed(|session, result| unsafe {
                    msime_engine_candidate_key(session, byte, result)
                });
                Some(outcome.map_or_else(|error| error, |result| result_json(&result, &self.roots)))
            }
            "select" => {
                let index = self.index_arg(arg);
                // SAFETY: a live handle and the result the helper passes.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_select(s, revision, index, result)
                })
            }
            "select_on_page" => {
                assert!(
                    self.page_size != 0,
                    "select_on_page needs options.page_size"
                );
                let index = u32::try_from(
                    self.page * self.page_size + arg.as_u64().expect("index") as usize,
                )
                .expect("index fits in u32");
                // SAFETY: as above.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_select(s, revision, index, result)
                })
            }
            "select_edge" => {
                let index = self.index_arg(arg);
                let edge = if step["edge"].as_str() == Some("first") {
                    MSIME_ENGINE_EDGE_FIRST_HAN
                } else {
                    MSIME_ENGINE_EDGE_LAST_HAN
                };
                // SAFETY: as above.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_select_edge(s, revision, index, edge, result)
                })
            }
            "pin" => {
                let index = self.index_arg(arg);
                // SAFETY: as above.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_pin(s, revision, index, result)
                })
            }
            "remove" => {
                let index = self.index_arg(arg);
                // SAFETY: as above.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_remove(s, revision, index, result)
                })
            }
            "fix_position" => {
                let index = self.index_arg(arg);
                let position = i32::try_from(step["position"].as_i64().expect("position"))
                    .expect("position fits in i32");
                // SAFETY: as above.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_fix_position(s, revision, index, position, result)
                })
            }
            "clear_position" => {
                let index = self.index_arg(arg);
                // SAFETY: as above.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_clear_position(s, revision, index, result)
                })
            }
            "finish" => {
                let index = if arg.is_null() {
                    0
                } else {
                    self.index_arg(arg)
                };
                // SAFETY: as above.
                self.on_row(index, |s, revision, index, result| unsafe {
                    msime_engine_finish(s, revision, index, result)
                })
            }
            "page" => {
                assert!(self.page_size != 0, "page needs options.page_size");
                self.page = moved_page(self.page, arg);
                None
            }
            "switch_scheme" => {
                let scheme = (0..=u8::MAX)
                    .filter_map(SchemeType::from_u8)
                    .find(|scheme| scheme_name(*scheme) == text())
                    .unwrap_or_else(|| panic!("unknown scheme {arg}"));
                // SAFETY: a live handle.
                let status = unsafe {
                    msime_engine_switch_scheme(self.session().0, scheme_constant(scheme))
                };
                (status != MSIME_ENGINE_OK)
                    .then(|| json!({"error": scrub(&last_error(), &self.roots)}))
            }
            "set_helpcode_schema" => {
                let schema = text();
                // SAFETY: a live handle and a borrowed string.
                let status = unsafe {
                    msime_engine_set_string(
                        self.session().0,
                        MSIME_ENGINE_OPTION_HELPCODE_SCHEMA,
                        schema.as_ptr().cast(),
                        schema.len(),
                    )
                };
                // The C function refuses an unknown name (INVALID_ARGUMENT) or a table it cannot load (FAILED); both are the core's `false`.
                Some(match status {
                    MSIME_ENGINE_OK => json!({"accepted": true}),
                    MSIME_ENGINE_FAILED | MSIME_ENGINE_INVALID_ARGUMENT => {
                        json!({"accepted": false})
                    }
                    _ => json!({"status": status, "error": last_error()}),
                })
            }
            "set_helpcode_enabled" => self
                .session()
                .set_int(MSIME_ENGINE_OPTION_HELPCODE_ENABLED, flag()),
            "set_dedicated_english" => self
                .session()
                .set_int(MSIME_ENGINE_OPTION_DEDICATED_ENGLISH, flag()),
            "set_wubi_mixed_pinyin" => self
                .session()
                .set_int(MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN, flag()),
            "set_chinese_punctuation_enabled" => self
                .session()
                .set_int(MSIME_ENGINE_OPTION_CHINESE_PUNCT, flag()),
            "set_paired_punctuation_enabled" => self
                .session()
                .set_int(MSIME_ENGINE_OPTION_PAIRED_PUNCT, flag()),
            "set_punctuation_lock" => self.session().set_int(
                MSIME_ENGINE_OPTION_PUNCT_LOCK,
                arg.as_i64().expect("an integer lock"),
            ),
            "set_personal_context_enabled" => self
                .session()
                .set_int(MSIME_ENGINE_OPTION_PERSONAL_CONTEXT, flag()),
            "reset_context" => {
                // SAFETY: a live handle.
                let status = unsafe { msime_engine_reset_context(self.session().0) };
                (status != MSIME_ENGINE_OK)
                    .then(|| json!({"status": status, "error": last_error()}))
            }
            "expand_initial_candidates" => {
                let mut grew = 0;
                // SAFETY: a live handle and a writable flag.
                let status = unsafe { msime_engine_expand_candidates(self.session().0, &mut grew) };
                Some(if status == MSIME_ENGINE_OK {
                    json!({"grew": grew != 0})
                } else {
                    json!({"status": status, "error": last_error()})
                })
            }
            "reopen" => {
                self.close();
                if let Some(changed) = step.get("options") {
                    apply_options(&mut self.options, changed);
                }
                self.open();
                None
            }
            "new_generation" => {
                self.close();
                self.content_id = text().to_owned();
                self.open();
                None
            }
            "dump_journal" => {
                flush_learning();
                out.insert(
                    "journal".into(),
                    journal_tables(&self.paths.runtime().user(USER_JOURNAL)),
                );
                None
            }
            "query" => {
                flush_learning();
                let db = step["db"].as_str().expect("db");
                let paths = self.paths.runtime();
                let path = if db == USER_JOURNAL {
                    paths.user(db)
                } else {
                    paths.dictionary(db)
                };
                out.insert("rows".into(), query_rows(&path, text()));
                None
            }
            "validate_entry" => {
                let entry = arg.to_string();
                let mut text: *mut c_char = ptr::null_mut();
                // SAFETY: 词条字符串和输出参数在调用期间都活着。
                let status = unsafe {
                    msime_engine_user_dict_validate(entry.as_ptr().cast(), entry.len(), &mut text)
                };
                // 与录制的形状一致：合法时是规范化后的词条，不合法时是引擎的原因。
                Some(if status == MSIME_ENGINE_OK {
                    json!({"entry": serde_json::from_str::<Value>(&take_string(text)).expect("the entry is JSON")})
                } else {
                    json!({"error": last_error()})
                })
            }
            "dict_edit" => {
                // Maintenance takes the exclusive dictionary lease, which an open C session's shared lease blocks (README「生命周期」), so a host releases the session, edits and opens a new one. The core replay edits beside its open session, which holds no lease.
                self.close();
                let edit = json!({
                    "previous": arg["previous"],
                    "replacement": arg["replacement"],
                    "request_id": arg["request_id"].as_str().unwrap_or_default(),
                })
                .to_string();
                // SAFETY: the paths and the JSON live across the call.
                let status = unsafe {
                    msime_engine_user_dict_edit(&self.paths.abi(), edit.as_ptr().cast(), edit.len())
                };
                let result = if status == MSIME_ENGINE_OK {
                    json!({"success": true})
                } else {
                    json!({"success": false, "error": scrub(&last_error(), &self.roots)})
                };
                self.open();
                Some(result)
            }
            "dict_list" => {
                let offset = arg["offset"].as_u64().map_or(0, |n| n as u32);
                let limit = arg["limit"].as_u64().map_or(100, |n| n as u32);
                let flags = if arg["include_learned"].as_bool().unwrap_or(false) {
                    MSIME_ENGINE_USER_DICT_INCLUDE_LEARNED
                } else {
                    0
                };
                let mut text: *mut c_char = ptr::null_mut();
                // SAFETY: the paths and the output pointer live across the call.
                let status = unsafe {
                    msime_engine_user_dict_list(&self.paths.abi(), offset, limit, flags, &mut text)
                };
                Some(if status == MSIME_ENGINE_OK {
                    serde_json::from_str(&take_string(text)).expect("the list is JSON")
                } else {
                    json!({"entries": [], "has_more": false, "error": scrub(&last_error(), &self.roots)})
                })
            }
            _ => panic!("op {op} reached the C replay; `unsupported` should have skipped it"),
        }
    }

    /// One `msime_engine_key_char` per byte, combined as the harness's `type_text` combines them.
    fn type_text(&self, text: &str) -> Value {
        let mut unhandled = Vec::new();
        let mut commit: Option<String> = None;
        let mut diagnostics = Vec::new();
        let mut errors = Vec::new();
        for (index, byte) in text.bytes().enumerate() {
            // SAFETY: a live handle and the result the helper passes.
            let outcome = self.session().keyed(|session, result| unsafe {
                msime_engine_key_char(session, byte, 0, result)
            });
            let result = match outcome {
                Ok(result) => result,
                Err(error) => {
                    errors.push(error);
                    continue;
                }
            };
            if result.handled == 0 {
                unhandled.push(index);
            }
            if result.has_commit != 0 {
                commit
                    .get_or_insert_with(String::new)
                    .push_str(&lent(result.commit));
            }
            let diagnostic = lent(result.diagnostic);
            if !diagnostic.is_empty() {
                diagnostics.push(Value::from(scrub(&diagnostic, &self.roots)));
            }
        }
        let mut out = Map::new();
        out.insert("handled".into(), Value::from(unhandled.is_empty()));
        if !unhandled.is_empty() {
            out.insert("unhandled_at".into(), Value::from(unhandled));
        }
        if let Some(commit) = commit {
            out.insert("commit".into(), Value::from(commit));
        }
        if !diagnostics.is_empty() {
            out.insert("diagnostics".into(), Value::Array(diagnostics));
        }
        if !errors.is_empty() {
            out.insert("errors".into(), Value::Array(errors));
        }
        Value::Object(out)
    }
}

// ---- Real-dictionary sets ----

/// 真实词库的几个集合在同一个代次上经核心 API 和 C ABI 逐行并排打字：取集合的产品选项，只去掉选项 JSON 不承载的 `sentence_alternatives`（ABI 固定为假）。学习关着，两个会话可以共用代次。和引擎的真实 golden 集合一样，只在设了 `MSIME_EVAL_RESOURCES` 时跑。
#[test]
fn real_sets_match_the_core_api_through_the_c_abi() {
    let _engine = engine_lock();
    let Some(resources) = real::resources() else {
        return;
    };
    assert!(
        real::has_recorded_resources(&resources),
        "{}={} lacks msime-pinyin.db, msime-english.db or the n-gram tables",
        real::RESOURCES_ENV,
        resources.display()
    );
    let mut failures = Vec::new();
    for file in [
        "sentences-v1.jsonl",
        "sentences-neutral-v1.jsonl",
        "sentences-v2.jsonl",
        "quanpin-words-v1.subset3000.jsonl",
    ] {
        let (rows, with_candidates, differing) = compare_real_set(file, &resources);
        eprintln!(
            "C ABI real set {file}: {rows} rows replayed, {with_candidates} with candidates, {} differ",
            differing.len()
        );
        // A wrong resource directory gives two empty lists that agree; the sets are all pinyin that the dictionaries answer.
        assert!(
            with_candidates * 2 > rows,
            "{file}: only {with_candidates} of {rows} rows have candidates; is {} the right directory?",
            resources.display()
        );
        let mut shown: Vec<String> = differing.iter().take(MAX_REPORTED_ROWS).cloned().collect();
        if differing.len() > MAX_REPORTED_ROWS {
            shown.push(format!(
                "... and {} more",
                differing.len() - MAX_REPORTED_ROWS
            ));
        }
        if !differing.is_empty() {
            failures.push(format!(
                "{file}: {} rows differ (expected = core API, got = C ABI)\n{}",
                differing.len(),
                shown.join("\n")
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `(rows, rows with candidates, differing rows)` for one set.
fn compare_real_set(file_name: &str, resources: &Path) -> (usize, usize, Vec<String>) {
    let work = tempfile::tempdir().unwrap();
    let root = std::path::absolute(work.path().join("real")).unwrap();
    let mut paths = CapiPaths {
        resources: utf8(&std::path::absolute(resources).unwrap()),
        user_data: utf8(&root.join("user")),
        cache: utf8(&root.join("cache")),
        dictionaries: String::new(),
    };
    paths
        .prepare("real")
        .unwrap_or_else(|error| panic!("prepare_runtime failed: {error}"));
    let mut options = SessionOptions::new(paths.runtime());
    real::product_options(&mut options);
    options.sentence_alternatives = false;
    let json = capi_options(&options)
        .unwrap_or_else(|reason| panic!("the real sets' options must fit the C ABI: {reason}"));
    let mut core =
        Session::new(options).unwrap_or_else(|error| panic!("Session::new failed: {error}"));
    let capi =
        CapiSession::new(&json).unwrap_or_else(|error| panic!("session_new failed: {error}"));
    let rows = real::expected_rows(file_name);
    let mut with_candidates = 0;
    let mut failures = Vec::new();
    for row in &rows {
        let input = row["input"].as_str().expect("row input");
        let mut core_handled = true;
        let mut capi_handled = true;
        for byte in input.bytes() {
            core_handled = core.character(byte, false).handled && core_handled;
            // SAFETY: a live handle and the result the helper passes.
            let result = capi
                .keyed(|session, result| unsafe { msime_engine_key_char(session, byte, 0, result) })
                .unwrap_or_else(|error| panic!("key_char: {error}"));
            capi_handled = result.handled != 0 && capi_handled;
        }
        let expected = json!({"handled": core_handled, "view": core_view(&core)});
        if expected["view"]["candidate_count"].as_u64() > Some(0) {
            with_candidates += 1;
        }
        let actual = json!({"handled": capi_handled, "view": capi.view()});
        let mut diffs = Vec::new();
        diff_values("", &expected, &actual, DiffOptions::default(), &mut diffs);
        if !diffs.is_empty() {
            failures.push(format!(
                "{} {input}: {}",
                row["id"].as_str().unwrap_or("?"),
                diffs.join("; ")
            ));
        }
        core.command(Command::Cancel);
        core.reset_context();
        // SAFETY: a live handle and the result the helper passes.
        capi.keyed(|session, result| unsafe {
            msime_engine_command(session, MSIME_ENGINE_COMMAND_CANCEL, result)
        })
        .unwrap_or_else(|error| panic!("command: {error}"));
        // SAFETY: a live handle.
        assert_eq!(
            unsafe { msime_engine_reset_context(capi.0) },
            MSIME_ENGINE_OK
        );
    }
    drop(core);
    drop(capi);
    flush_learning();
    assert_eq!(msime_engine_close_databases(), MSIME_ENGINE_OK);
    (rows.len(), with_candidates, failures)
}

// ---- The replay's own rules ----

#[test]
fn options_the_json_cannot_carry_skip_the_scenario_by_name() {
    let mut options = reference_options();
    assert!(capi_options(&options).is_ok());
    options.japanese_mixing = true;
    assert_eq!(
        capi_options(&options).unwrap_err(),
        "option japanese_mixing (not in the options JSON)"
    );
    let mut options = reference_options();
    options.local_modes.command = true;
    assert_eq!(
        capi_options(&options).unwrap_err(),
        "option local_modes (not in the options JSON)"
    );
    let mut options = reference_options();
    apply_options(&mut options, &json!({"sentence_alternatives": true}));
    assert_eq!(
        capi_options(&options).unwrap_err(),
        "option sentence_alternatives (not in the options JSON)"
    );
    let mut options = reference_options();
    apply_options(&mut options, &json!({"fuzzy_pinyin_rules": 0x800}));
    assert_eq!(
        capi_options(&options).unwrap_err(),
        "option fuzzy_pinyin (not in the options JSON)"
    );
}

#[test]
fn every_option_the_json_carries_is_written_out() {
    let mut options = reference_options();
    apply_options(
        &mut options,
        &json!({
            "scheme": "shuangpin", "shuangpin_profile": "microsoft", "shuangpin_preedit_uses_raw": false,
            "autocorrect_types": 3, "fuzzy_pinyin_rules": 5, "learning": false,
            "english": {"mixed_candidates": true, "minimum_prefix": 5},
            "wubi_mixed_pinyin": true, "vietnamese_input_method": "vni",
            "frequency": {"mode": "linear", "trigger_count": 3, "linear_step": 4},
            "expressive": {"kaomoji_candidates": true},
            "local_modes": {"emoji": false, "temporary_japanese": false},
        }),
    );
    let json = capi_options(&options).unwrap();
    assert_eq!(json["scheme"], "shuangpin");
    assert_eq!(json["enabled_schemes"].as_array().unwrap().len(), 10);
    assert_eq!(
        json["shuangpin"],
        json!({"profile": "microsoft", "preedit_shows_keys": false})
    );
    assert_eq!(json["autocorrect"], json!(["transposition", "neighbor"]));
    assert_eq!(json["fuzzy_pinyin"], json!(["z_zh", "s_sh"]));
    assert_eq!(
        json["helpcode"],
        json!({"enabled": true, "schema": "lantian"})
    );
    assert_eq!(json["wubi"]["mixed_pinyin"], true);
    assert_eq!(json["vietnamese"]["input_method"], "vni");
    assert_eq!(
        json["english"],
        json!({
            "mixed_candidates": true,
            "minimum_prefix": 5,
            "first_page_count": 1,
            "learn_on_enter": true,
            "learn_typed_spellings_on_enter": false,
        })
    );
    assert_eq!(json["punctuation"]["lock"], "follow");
    assert_eq!(json["learning"], false);
    assert_eq!(
        json["frequency"],
        json!({"mode": "linear", "trigger_count": 3, "linear_step": 4})
    );
    assert_eq!(
        FrequencyAdjustmentMode::from_name(json["frequency"]["mode"].as_str().unwrap()),
        Some(FrequencyAdjustmentMode::Linear)
    );
    assert_eq!(
        json["expressive"],
        json!({"emoji_candidates": false, "kaomoji_candidates": true, "symbol_candidates": false})
    );
    assert_eq!(json["local_modes"]["emoji"], false);
    assert_eq!(json["local_modes"]["temporary_japanese"], false);
    assert_eq!(json["local_modes"]["unicode"], true);
    assert_eq!(json["local_modes"].as_object().unwrap().len(), 14);
}
