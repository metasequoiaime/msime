//! Scripted scenario replay (`golden/scenarios/<name>.json` against `golden/expected/<name>.json`): a scratch root per scenario, the fixture staged as `resources`, `prepare_runtime_paths(resources, root/user, root/cache, content_id)`, one `Session`, and every op of the README's op list. `page` and `select_on_page` are harness-side, and the page returns to 0 whenever the editing text or the candidate words change, as a host does.

use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use msime_engine::assets::USER_JOURNAL;
use msime_engine::vietnamese::{InputMethod, ToneStyle};
use msime_engine::{
    CandidateEdge, CandidateSource, Command, FrequencyAdjustmentMode, PersonalDictionaryEntry,
    PersonalDictionaryKind, RuntimePaths, SchemeType, Session, SessionOptions,
    ShuangpinProfileKind,
};
use serde_json::{json, Map, Value};

use super::diff::{diff_documents, DiffOptions};
use super::fixture::stage_fixture;
use super::golden_dir;
use super::snapshot::{dump_journal, query_rows, result_json, scrub, snapshot_json};

/// The file name a scenario fixture stages `cantonese.db` under, beside the resource set's own files.
const CANTONESE_DICTIONARY: &str = "cantonese.db";
/// The file name a scenario fixture stages `zhuyin.db` under, beside the resource set's own files.
const ZHUYIN_DICTIONARY: &str = "zhuyin.db";
/// The file name a scenario fixture stages `stroke.db` under, beside the resource set's own files.
const STROKE_DICTIONARY: &str = "stroke.db";

pub const SCENARIO_ENV: &str = "MSIME_GOLDEN_SCENARIO";

/// Differences printed per scenario before the rest is summarised as a count.
const MAX_DIFFS_PER_SCENARIO: usize = 40;

/// Scenario files in name order, limited by `MSIME_GOLDEN_SCENARIO` when set.
pub fn selected_scenarios() -> Vec<PathBuf> {
    let filter = std::env::var(SCENARIO_ENV).ok();
    selected_scenarios_from(filter.as_deref())
}

/// `selected_scenarios` for an explicit filter (`name[,name...]`); a name without a scenario file fails, so a typo cannot pass as an empty run.
pub fn selected_scenarios_from(filter: Option<&str>) -> Vec<PathBuf> {
    let directory = golden_dir().join("scenarios");
    let mut all: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    all.sort();
    let names: Vec<&str> = filter
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return all;
    }
    names
        .into_iter()
        .map(|name| {
            let path = directory.join(format!("{name}.json"));
            assert!(
                all.contains(&path),
                "{SCENARIO_ENV} names {name}, which has no scenario file"
            );
            path
        })
        .collect()
}

/// Replay each scenario, diff it against its expected file, print every diff and fail if any scenario differs.
pub fn assert_scenarios(scenarios: &[PathBuf]) {
    let work = tempfile::tempdir().unwrap();
    let mut failed = Vec::new();
    for path in scenarios {
        let spec = read_json(path);
        let name = spec["name"].as_str().expect("scenario name").to_owned();
        let expected = read_json(&golden_dir().join("expected").join(format!("{name}.json")));
        // One scenario that panics (an unported engine path, a fixture error) is reported like a diff so the rest still run.
        let replayed =
            panic::catch_unwind(AssertUnwindSafe(|| replay_scenario(&spec, work.path())));
        if replayed.is_err() {
            // The panicking replay skipped its own close, so its connections are still cached. With the engine this far from done the close can panic as well, and that second panic adds nothing to the failure already recorded.
            let _ = panic::catch_unwind(msime_engine::close_cached_databases);
        }
        let diffs = match replayed {
            Ok(actual) => diff_documents(&expected, &actual, DiffOptions::default()),
            Err(payload) => vec![format!("replay panicked: {}", panic_message(&*payload))],
        };
        if diffs.is_empty() {
            continue;
        }
        let mut report = format!("{name}: {} differences", diffs.len());
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
    assert!(
        failed.is_empty(),
        "{} of {} scenarios differ from the reference: {}",
        failed.len(),
        scenarios.len(),
        failed.join(", ")
    );
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

/// The recorder's output document `{name, steps, final_journal?}` for one scenario spec, replayed under `work`.
pub fn replay_scenario(spec: &Value, work: &Path) -> Value {
    let name = spec["name"].as_str().expect("scenario name");
    let root = std::path::absolute(work.join(name)).unwrap();
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    let options_spec = spec.get("options").cloned().unwrap_or_else(|| json!({}));
    let mut options = SessionOptions::new(RuntimePaths::default());
    apply_options(&mut options, &options_spec);
    let mut scenario = Scenario {
        roots: root_spellings(&root),
        resources: root.join("resources"),
        root,
        options,
        content_id: options_spec["content_id"]
            .as_str()
            .unwrap_or("v1")
            .to_owned(),
        page_size: options_spec["page_size"].as_u64().map_or(0, to_usize),
        page: 0,
        session: None,
        paths: RuntimePaths::default(),
    };
    stage_fixture(&spec["fixture"], &scenario.resources);

    scenario.open();
    let mut steps = vec![json!({"op": "open", "snapshot": scenario.snapshot()})];
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
            dump_journal(&scenario.paths.user(USER_JOURNAL)),
        );
    }
    msime_engine::close_cached_databases();
    std::fs::remove_dir_all(&scenario.root).unwrap();
    Value::Object(out)
}

/// The canonical spelling and the one the harness built, so `/private/var/...` and its `/var/...` alias both scrub.
fn root_spellings(root: &Path) -> Vec<String> {
    let mut roots = vec![root.to_string_lossy().into_owned()];
    if let Ok(canonical) = root.canonicalize() {
        roots.push(canonical.to_string_lossy().into_owned());
    }
    roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
    roots.dedup();
    roots
}

fn to_usize(value: u64) -> usize {
    usize::try_from(value).expect("index fits in usize")
}

/// Only the reference `SessionOptions` fields, as the recorder's `apply_options`; `learning_undo` and unknown fields fail the scenario.
pub fn apply_options(options: &mut SessionOptions, value: &Value) {
    let Some(fields) = value.as_object() else {
        panic!("options must be an object, got {value}");
    };
    for (key, v) in fields {
        match key.as_str() {
            "scheme" => options.scheme = scheme_from(as_str(v)),
            "shuangpin_profile" => {
                options.shuangpin_profile = ShuangpinProfileKind::from_name(as_str(v))
                    .unwrap_or_else(|| panic!("unknown shuangpin profile {v}"));
            }
            "shuangpin_preedit_uses_raw" => options.shuangpin_preedit_uses_raw = as_bool(v),
            "vietnamese_input_method" => {
                options.vietnamese_input_method = match as_str(v) {
                    "telex" => InputMethod::Telex,
                    "vni" => InputMethod::Vni,
                    other => panic!("unknown vietnamese input method {other}"),
                };
            }
            "vietnamese_tone_style" => {
                options.vietnamese_tone_style = match as_str(v) {
                    "modern" => ToneStyle::Modern,
                    "classic" => ToneStyle::Classic,
                    other => panic!("unknown vietnamese tone style {other}"),
                };
            }
            "helpcode_schema" => options.helpcode_schema = as_str(v).to_owned(),
            "autocorrect_types" => options.autocorrect_types = as_u32(v),
            "helpcode" => options.helpcode = as_bool(v),
            "chinese_punctuation" => options.chinese_punctuation = as_bool(v),
            "paired_punctuation" => options.paired_punctuation = as_bool(v),
            "punctuation_lock" => options.punctuation_lock = as_i32(v),
            "learning" => options.learning = as_bool(v),
            "sentence_alternatives" => options.sentence_alternatives = as_bool(v),
            "fuzzy_pinyin_rules" => options.fuzzy_pinyin.rules = as_u32(v),
            "frequency" => {
                if let Some(mode) = v.get("mode") {
                    options.frequency.mode = FrequencyAdjustmentMode::from_name(as_str(mode))
                        .unwrap_or_else(|| panic!("unknown frequency mode {mode}"));
                }
                if let Some(count) = v.get("trigger_count") {
                    options.frequency.trigger_count = as_i32(count);
                }
                if let Some(step) = v.get("linear_step") {
                    options.frequency.linear_step = as_i32(step);
                }
            }
            "local_modes" => {
                let modes = &mut options.local_modes;
                for (name, flag) in v.as_object().expect("local_modes is an object") {
                    let on = as_bool(flag);
                    match name.as_str() {
                        "unicode" => modes.unicode = on,
                        "date_time" => modes.date_time = on,
                        "quick_phrase" => modes.quick_phrase = on,
                        "emoji" => modes.emoji = on,
                        "kaomoji" => modes.kaomoji = on,
                        "super_jianpin" => modes.super_jianpin = on,
                        "temporary_english" => modes.temporary_english = on,
                        "temporary_japanese" => modes.temporary_japanese = on,
                        _ => panic!("unknown local mode {name}"),
                    }
                }
            }
            "english" => {
                if let Some(mixed) = v.get("mixed_candidates") {
                    options.english.mixed_candidates = as_bool(mixed);
                }
                if let Some(prefix) = v.get("minimum_prefix") {
                    options.english.minimum_prefix =
                        to_usize(prefix.as_u64().expect("minimum_prefix"));
                }
            }
            "expressive" => {
                if let Some(emoji) = v.get("emoji_candidates") {
                    options.expressive.emoji_candidates = as_bool(emoji);
                }
                if let Some(kaomoji) = v.get("kaomoji_candidates") {
                    options.expressive.kaomoji_candidates = as_bool(kaomoji);
                }
            }
            "wubi_mixed_pinyin" => options.wubi.mixed_pinyin = as_bool(v),
            "personal_context" => options.personal_context = as_bool(v),
            // Harness-side: the page view and the generation name.
            "page_size" | "content_id" => {}
            _ => panic!("unknown or unsupported option {key}"),
        }
    }
}

fn as_str(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("expected a string, got {value}"))
}

fn as_bool(value: &Value) -> bool {
    value
        .as_bool()
        .unwrap_or_else(|| panic!("expected a bool, got {value}"))
}

fn as_i32(value: &Value) -> i32 {
    value
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .unwrap_or_else(|| panic!("expected an i32, got {value}"))
}

fn as_u32(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .unwrap_or_else(|| panic!("expected a u32, got {value}"))
}

fn as_index(value: &Value) -> usize {
    to_usize(
        value
            .as_u64()
            .unwrap_or_else(|| panic!("expected an index, got {value}")),
    )
}

/// The first byte of a one-character string argument, as the recorder's `.at(0)`.
fn first_byte(value: &Value) -> u8 {
    *as_str(value)
        .as_bytes()
        .first()
        .unwrap_or_else(|| panic!("expected a non-empty string, got {value}"))
}

fn scheme_from(name: &str) -> SchemeType {
    match name {
        "quanpin" => SchemeType::Quanpin,
        "shuangpin" => SchemeType::Shuangpin,
        "wubi" => SchemeType::Wubi,
        "japanese" => SchemeType::JapaneseRomaji,
        "korean" => SchemeType::Korean,
        "cantonese" => SchemeType::Cantonese,
        "vietnamese" => SchemeType::Vietnamese,
        "tibetan" => SchemeType::Tibetan,
        "zhuyin" => SchemeType::Zhuyin,
        "stroke" => SchemeType::Stroke,
        _ => panic!("unknown scheme {name}"),
    }
}

fn source_from(value: &Value) -> CandidateSource {
    if let Some(number) = value.as_u64() {
        return u8::try_from(number)
            .ok()
            .and_then(CandidateSource::from_u8)
            .unwrap_or_else(|| panic!("unknown candidate source {number}"));
    }
    match as_str(value) {
        "cloud" => CandidateSource::CloudSuggestion,
        "ai" => CandidateSource::AiSuggestion,
        other => panic!("unknown online source {other}"),
    }
}

/// The recorder's kind names; quick phrases are `quick_phrase` here, unlike the journal's `quick`.
fn kind_from(name: &str) -> PersonalDictionaryKind {
    match name {
        "pinyin" => PersonalDictionaryKind::Pinyin,
        "wubi" => PersonalDictionaryKind::Wubi,
        "quick_phrase" => PersonalDictionaryKind::QuickPhrase,
        "english" => PersonalDictionaryKind::English,
        _ => panic!("unknown dictionary kind {name}"),
    }
}

fn kind_name(kind: PersonalDictionaryKind) -> &'static str {
    match kind {
        PersonalDictionaryKind::Pinyin => "pinyin",
        PersonalDictionaryKind::Wubi => "wubi",
        PersonalDictionaryKind::QuickPhrase => "quick_phrase",
        PersonalDictionaryKind::English => "english",
        _ => panic!("dictionary kind {kind:?} has no recorder name"),
    }
}

fn entry_from(value: &Value) -> Option<PersonalDictionaryEntry> {
    if value.is_null() {
        return None;
    }
    Some(PersonalDictionaryEntry {
        kind: kind_from(as_str(&value["kind"])),
        key: as_str(&value["key"]).to_owned(),
        value: as_str(&value["value"]).to_owned(),
        weight: value
            .get("weight")
            .map_or(PersonalDictionaryEntry::DEFAULT_WEIGHT, |weight| {
                weight
                    .as_i64()
                    .unwrap_or_else(|| panic!("expected an i64 weight, got {weight}"))
            }),
    })
}

fn entry_json(entry: &PersonalDictionaryEntry) -> Value {
    json!({"kind": kind_name(entry.kind), "key": entry.key, "value": entry.value, "weight": entry.weight})
}

/// Ops after which the recorder keeps the page: they only move the view or read data.
fn keeps_page(op: &str) -> bool {
    matches!(
        op,
        "page" | "dump_journal" | "query" | "validate_entry" | "dict_list"
    )
}

/// Ops that record no snapshot because they cannot change the session.
fn read_only(op: &str) -> bool {
    matches!(
        op,
        "dump_journal" | "query" | "validate_entry" | "dict_list"
    )
}

/// `page` moves: `next`, `prev` (never below 0) or an absolute index.
fn moved_page(current: usize, arg: &Value) -> usize {
    match arg.as_str() {
        Some("next") => current + 1,
        Some("prev") => current.saturating_sub(1),
        _ => as_index(arg),
    }
}

struct Scenario {
    root: PathBuf,
    resources: PathBuf,
    roots: Vec<String>,
    options: SessionOptions,
    content_id: String,
    page_size: usize,
    page: usize,
    session: Option<Session>,
    paths: RuntimePaths,
}

impl Scenario {
    fn open(&mut self) {
        self.paths = msime_engine::prepare_runtime_paths(
            &self.resources,
            &self.root.join("user"),
            &self.root.join("cache"),
            &self.content_id,
        )
        .unwrap_or_else(|error| {
            panic!(
                "prepare_runtime_paths failed: {}",
                scrub(&error.to_string(), &self.roots)
            )
        });
        let mut options = self.options.clone();
        options.paths = self.paths.clone();
        // `cantonese.db`, `zhuyin.db` and `stroke.db` ship beside the resource set, so a fixture that stages one hands its path to the session as a host would.
        let cantonese = self.resources.join(CANTONESE_DICTIONARY);
        if cantonese.exists() {
            options.cantonese_dictionary = cantonese;
        }
        let zhuyin = self.resources.join(ZHUYIN_DICTIONARY);
        if zhuyin.exists() {
            options.zhuyin_dictionary = zhuyin;
        }
        let stroke = self.resources.join(STROKE_DICTIONARY);
        if stroke.exists() {
            options.stroke_dictionary = stroke;
        }
        self.session = Some(Session::new(options).unwrap_or_else(|error| {
            panic!(
                "Session::new failed: {}",
                scrub(&error.to_string(), &self.roots)
            )
        }));
        self.page = 0;
    }

    /// Drop the session, then write the delayed personal-context rows, as the recorder's `close`.
    fn close(&mut self) {
        self.session = None;
        msime_engine::flush_personal_learning();
    }

    fn session(&mut self) -> &mut Session {
        self.session.as_mut().expect("an open session")
    }

    fn snapshot(&self) -> Value {
        let session = self.session.as_ref().expect("an open session");
        let page = (self.page_size != 0).then_some((self.page, self.page_size));
        snapshot_json(session, page)
    }

    /// Editing text and candidate words: what a host watches to send the page back to 0.
    fn view_key(&self) -> (String, Vec<String>) {
        let view = self.session.as_ref().expect("an open session").snapshot();
        let words = view.candidates.into_iter().map(|item| item.word).collect();
        (view.editing_text, words)
    }

    /// `{"word": ...}` selects by word like the reference tests' candidate_index helpers; a number is the index itself.
    fn index_arg(&self, arg: &Value) -> usize {
        if arg.is_u64() {
            return as_index(arg);
        }
        let word = as_str(&arg["word"]);
        let view = self.session.as_ref().expect("an open session").snapshot();
        view.candidates
            .iter()
            .position(|item| item.word == word)
            .unwrap_or_else(|| panic!("step selects a word that is not a candidate: {word}"))
    }

    fn run_step(&mut self, step: &Value) -> Value {
        let op = as_str(&step["op"]).to_owned();
        let before = self.view_key();
        let arg = step.get("arg").cloned().unwrap_or(Value::Null);
        let mut out = Map::new();
        out.insert("op".into(), Value::from(op.as_str()));
        if !arg.is_null() {
            out.insert("arg".into(), arg.clone());
        }
        let result = self.dispatch(&op, &arg, step, &mut out);
        if !keeps_page(&op) && self.view_key() != before {
            self.page = 0;
        }
        if let Some(result) = result {
            out.insert("result".into(), result);
        }
        if !read_only(&op) {
            out.insert("snapshot".into(), self.snapshot());
        }
        Value::Object(out)
    }

    /// Run one op; the return value is the step's `result`, and ops with output of their own (`journal`, `rows`) write it into `out`.
    fn dispatch(
        &mut self,
        op: &str,
        arg: &Value,
        step: &Value,
        out: &mut Map<String, Value>,
    ) -> Option<Value> {
        let roots = self.roots.clone();
        let keyed = |result: msime_engine::KeyResult| Some(result_json(&result, &roots));
        match op {
            "type" => Some(self.type_text(as_str(arg))),
            "char" => {
                let text = as_str(arg);
                assert_eq!(text.len(), 1, "char takes one byte");
                let shift = step["shift"].as_bool().unwrap_or(false);
                keyed(self.session().character(text.as_bytes()[0], shift))
            }
            "command" => {
                let command = Command::from_name(as_str(arg))
                    .unwrap_or_else(|| panic!("unknown command {arg}"));
                keyed(self.session().command(command))
            }
            "candidate_key" => {
                let key = first_byte(arg);
                keyed(self.session().candidate_key(key))
            }
            "punctuation" => {
                let key = first_byte(arg);
                keyed(self.session().punctuation(key))
            }
            "select" => {
                let index = self.index_arg(arg);
                keyed(self.session().select(index))
            }
            "select_on_page" => {
                assert!(
                    self.page_size != 0,
                    "select_on_page needs options.page_size"
                );
                let index = self.page * self.page_size + as_index(arg);
                keyed(self.session().select(index))
            }
            "select_edge" => {
                let index = self.index_arg(arg);
                let edge = if as_str(&step["edge"]) == "first" {
                    CandidateEdge::FirstHan
                } else {
                    CandidateEdge::LastHan
                };
                keyed(self.session().select_edge(index, edge))
            }
            "pin" => {
                let index = self.index_arg(arg);
                keyed(self.session().pin(index))
            }
            "remove" => {
                let index = self.index_arg(arg);
                keyed(self.session().remove(index))
            }
            "fix_position" => {
                let index = self.index_arg(arg);
                let position = as_i32(&step["position"]);
                keyed(self.session().fix_position(index, position))
            }
            "clear_position" => {
                let index = self.index_arg(arg);
                keyed(self.session().clear_position(index))
            }
            "finish" => {
                // `Session::finish()` without an index is `finish(0)` (session.cpp:183-186).
                let index = if arg.is_null() { 0 } else { as_index(arg) };
                keyed(self.session().finish(index))
            }
            "page" => {
                // Paging is host behaviour (input-runtime); the engine hands back the whole list. This only moves the recorded view so page-relative selections can be scripted.
                assert!(self.page_size != 0, "page needs options.page_size");
                self.page = moved_page(self.page, arg);
                None
            }
            "switch_scheme" => {
                // A scheme whose dictionary cannot be opened is refused and the session stays where it was; the refusal is the step's result.
                match self.session().switch_scheme(scheme_from(as_str(arg))) {
                    Ok(()) => None,
                    Err(error) => Some(json!({"error": scrub(&error.to_string(), &self.roots)})),
                }
            }
            "set_helpcode_schema" => {
                let accepted = self.session().set_helpcode_schema(as_str(arg));
                Some(json!({"accepted": accepted}))
            }
            "set_helpcode_enabled" => {
                self.session().set_helpcode_enabled(as_bool(arg));
                None
            }
            "set_dedicated_english" => {
                self.session().set_dedicated_english(as_bool(arg));
                None
            }
            "set_wubi_mixed_pinyin" => {
                self.session().set_wubi_mixed_pinyin(as_bool(arg));
                None
            }
            "set_chinese_punctuation_enabled" => {
                self.session().set_chinese_punctuation_enabled(as_bool(arg));
                None
            }
            "set_punctuation_lock" => {
                // The reference throws for a lock outside 0..=2, which failed the recording; the same panic fails the replay.
                self.session()
                    .set_punctuation_lock(as_i32(arg))
                    .unwrap_or_else(|error| panic!("set_punctuation_lock: {error}"));
                None
            }
            "set_paired_punctuation_enabled" => {
                self.session().set_paired_punctuation_enabled(as_bool(arg));
                None
            }
            "balance_paired_punctuation_after_auto_close" => {
                let opening = first_byte(arg);
                self.session()
                    .balance_paired_punctuation_after_auto_close(opening);
                None
            }
            "set_nine_key_enabled" => {
                self.session().set_nine_key_enabled(as_bool(arg));
                None
            }
            "choose_nine_key_spelling" => {
                let index = if arg.is_u64() {
                    as_index(arg)
                } else {
                    // {"spelling": "ni"} picks by text, like the reference test's std::find over nine_key_spellings.
                    let spelling = as_str(&arg["spelling"]);
                    self.session()
                        .snapshot()
                        .nine_key_spellings
                        .iter()
                        .position(|offered| offered == spelling)
                        .unwrap_or_else(|| panic!("spelling not offered: {spelling}"))
                };
                keyed(self.session().choose_nine_key_spelling(index))
            }
            "set_personal_context_enabled" => {
                self.session().set_personal_context_enabled(as_bool(arg));
                None
            }
            "reset_cache" => {
                self.session().reset_cache();
                None
            }
            "reset_context" => {
                self.session().reset_context();
                None
            }
            "expand_initial_candidates" => {
                let grew = self.session().expand_initial_candidates();
                Some(json!({"grew": grew}))
            }
            "apply_online_candidates" => {
                // Uses the live query, as a host does when its request returns before the composition changes.
                let Some(query) = self.session().online_query() else {
                    return Some(json!({"applied": false, "no_query": true}));
                };
                let words: Vec<String> = arg
                    .as_array()
                    .expect("apply_online_candidates takes a word list")
                    .iter()
                    .map(|word| as_str(word).to_owned())
                    .collect();
                let source = source_from(&step["source"]);
                let applied = self
                    .session()
                    .apply_online_candidates(&query, &words, source);
                Some(json!({"applied": applied}))
            }
            "reopen" => {
                // A new Session on the same user data: the same generation is re-prepared, which replays the journal. An `options` object on the step changes those session options for the new Session (harness-side, for options with no live setter such as the Vietnamese tone style).
                self.close();
                if let Some(changed) = step.get("options") {
                    apply_options(&mut self.options, changed);
                }
                self.open();
                None
            }
            "new_generation" => {
                self.close();
                self.content_id = as_str(arg).to_owned();
                self.open();
                None
            }
            "dump_journal" => {
                out.insert(
                    "journal".into(),
                    dump_journal(&self.paths.user(USER_JOURNAL)),
                );
                None
            }
            "query" => {
                // Read-only SQL against the live generation copy of a dictionary (msime.db / english.db) or the journal.
                msime_engine::flush_personal_learning();
                let db = as_str(&step["db"]);
                let path = if db == USER_JOURNAL {
                    self.paths.user(db)
                } else {
                    self.paths.dictionary(db)
                };
                out.insert("rows".into(), query_rows(&path, as_str(arg)));
                None
            }
            "validate_entry" => {
                let entry = entry_from(arg).expect("validate_entry takes an entry");
                Some(
                    match msime_engine::validate_personal_dictionary_entry(&entry) {
                        Ok(entry) => json!({"entry": entry_json(&entry)}),
                        Err(error) => json!({"error": error.to_string()}),
                    },
                )
            }
            "dict_edit" => {
                let previous = entry_from(&arg["previous"]);
                let replacement = entry_from(&arg["replacement"]);
                let request_id = arg["request_id"].as_str().unwrap_or_default();
                let edit = msime_engine::edit_personal_dictionary(
                    &self.paths,
                    previous.as_ref(),
                    replacement.as_ref(),
                    request_id,
                );
                Some(match edit {
                    Ok(()) => json!({"success": true}),
                    Err(error) => {
                        json!({"success": false, "error": scrub(&error.to_string(), &self.roots)})
                    }
                })
            }
            "dict_list" => {
                let offset = arg["offset"].as_u64().map_or(0, to_usize);
                let limit = arg["limit"].as_u64().map_or(100, to_usize);
                let include_learned = arg["include_learned"].as_bool().unwrap_or(false);
                Some(
                    match msime_engine::personal_dictionary_entries(
                        &self.paths,
                        offset,
                        limit,
                        include_learned,
                    ) {
                        Ok(page) => json!({
                            "entries": page.entries.iter().map(entry_json).collect::<Vec<_>>(),
                            "has_more": page.has_more,
                        }),
                        Err(error) => json!({
                            "entries": [],
                            "has_more": false,
                            "error": scrub(&error.to_string(), &self.roots),
                        }),
                    },
                )
            }
            _ => panic!("unknown op {op}"),
        }
    }

    /// One `character` call per byte; the step reports the combined outcome.
    fn type_text(&mut self, text: &str) -> Value {
        let mut unhandled = Vec::new();
        let mut commit: Option<String> = None;
        let mut diagnostics = Vec::new();
        for (index, byte) in text.bytes().enumerate() {
            let result = self.session().character(byte, false);
            if !result.handled {
                unhandled.push(index);
            }
            if let Some(text) = result.commit {
                commit.get_or_insert_with(String::new).push_str(&text);
            }
            if let Some(diagnostic) = result.diagnostic {
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
        Value::Object(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_is_selected_in_name_order_without_a_filter() {
        let all = selected_scenarios_from(None);
        assert_eq!(all.len(), 299);
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(all, sorted);
        assert_eq!(selected_scenarios_from(Some(" , ")), all);
    }

    #[test]
    fn the_filter_keeps_only_the_named_scenarios() {
        let picked = selected_scenarios_from(Some("remove_wubi, qp_select_edge"));
        let names: Vec<_> = picked
            .iter()
            .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["remove_wubi", "qp_select_edge"]);
    }

    #[test]
    #[should_panic(expected = "names qp_nope, which has no scenario file")]
    fn a_filter_naming_no_scenario_fails() {
        selected_scenarios_from(Some("qp_nope"));
    }

    #[test]
    fn every_scenario_uses_only_known_options_and_ops() {
        const OPS: &[&str] = &[
            "type",
            "char",
            "command",
            "candidate_key",
            "punctuation",
            "select",
            "select_on_page",
            "select_edge",
            "pin",
            "remove",
            "fix_position",
            "clear_position",
            "finish",
            "page",
            "switch_scheme",
            "set_helpcode_schema",
            "set_helpcode_enabled",
            "set_dedicated_english",
            "set_wubi_mixed_pinyin",
            "set_chinese_punctuation_enabled",
            "set_punctuation_lock",
            "set_paired_punctuation_enabled",
            "balance_paired_punctuation_after_auto_close",
            "set_nine_key_enabled",
            "choose_nine_key_spelling",
            "set_personal_context_enabled",
            "reset_cache",
            "reset_context",
            "expand_initial_candidates",
            "apply_online_candidates",
            "reopen",
            "new_generation",
            "dump_journal",
            "query",
            "validate_entry",
            "dict_edit",
            "dict_list",
        ];
        for path in selected_scenarios_from(None) {
            let spec = read_json(&path);
            let mut options = SessionOptions::new(RuntimePaths::default());
            apply_options(&mut options, &spec["options"]);
            for step in spec["steps"].as_array().unwrap() {
                let op = step["op"].as_str().unwrap();
                assert!(OPS.contains(&op), "{}: unknown op {op}", path.display());
                if op == "command" {
                    assert!(Command::from_name(step["arg"].as_str().unwrap()).is_some());
                }
            }
        }
    }

    #[test]
    fn options_map_onto_the_reference_fields() {
        let mut options = SessionOptions::new(RuntimePaths::default());
        apply_options(
            &mut options,
            &json!({
                "scheme": "shuangpin", "shuangpin_profile": "microsoft", "shuangpin_preedit_uses_raw": false,
                "helpcode_schema": "xiaohe", "autocorrect_types": 3, "helpcode": false,
                "chinese_punctuation": false, "paired_punctuation": false, "punctuation_lock": 2,
                "learning": false, "sentence_alternatives": true, "fuzzy_pinyin_rules": 5,
                "frequency": {"mode": "linear", "trigger_count": 3, "linear_step": 4},
                "local_modes": {"emoji": false, "temporary_japanese": false},
                "english": {"mixed_candidates": true, "minimum_prefix": 4},
                "expressive": {"emoji_candidates": true},
                "wubi_mixed_pinyin": true, "personal_context": false, "page_size": 2, "content_id": "v2",
            }),
        );
        assert_eq!(options.scheme, SchemeType::Shuangpin);
        assert_eq!(options.shuangpin_profile, ShuangpinProfileKind::Microsoft);
        assert!(!options.shuangpin_preedit_uses_raw);
        assert_eq!(options.helpcode_schema, "xiaohe");
        assert_eq!(options.autocorrect_types, 3);
        assert!(!options.helpcode && !options.chinese_punctuation && !options.paired_punctuation);
        assert_eq!(options.punctuation_lock, 2);
        assert!(!options.learning && options.sentence_alternatives);
        assert_eq!(options.fuzzy_pinyin.rules, 5);
        assert_eq!(options.frequency.mode, FrequencyAdjustmentMode::Linear);
        assert_eq!(
            (
                options.frequency.trigger_count,
                options.frequency.linear_step
            ),
            (3, 4)
        );
        assert!(!options.local_modes.emoji && !options.local_modes.temporary_japanese);
        assert!(options.local_modes.unicode && options.local_modes.kaomoji);
        assert!(options.english.mixed_candidates);
        assert_eq!(options.english.minimum_prefix, 4);
        assert!(options.expressive.emoji_candidates && !options.expressive.kaomoji_candidates);
        assert!(options.wubi.mixed_pinyin && !options.personal_context);
    }

    #[test]
    fn vietnamese_options_map_onto_the_session_fields() {
        let mut options = SessionOptions::new(RuntimePaths::default());
        assert_eq!(options.vietnamese_input_method, InputMethod::Telex);
        assert_eq!(options.vietnamese_tone_style, ToneStyle::Modern);
        apply_options(
            &mut options,
            &json!({"scheme": "vietnamese", "vietnamese_input_method": "vni", "vietnamese_tone_style": "classic"}),
        );
        assert_eq!(options.scheme, SchemeType::Vietnamese);
        assert_eq!(options.vietnamese_input_method, InputMethod::Vni);
        assert_eq!(options.vietnamese_tone_style, ToneStyle::Classic);
    }

    #[test]
    #[should_panic(expected = "unknown or unsupported option learning_undo")]
    fn learning_undo_is_refused() {
        let mut options = SessionOptions::new(RuntimePaths::default());
        apply_options(&mut options, &json!({"learning_undo": true}));
    }

    #[test]
    fn pages_move_and_clamp_at_zero() {
        assert_eq!(moved_page(0, &json!("next")), 1);
        assert_eq!(moved_page(0, &json!("prev")), 0);
        assert_eq!(moved_page(3, &json!("prev")), 2);
        assert_eq!(moved_page(1, &json!(4)), 4);
        assert!(keeps_page("page") && keeps_page("query") && !keeps_page("reopen"));
        assert!(read_only("dict_list") && !read_only("dict_edit") && !read_only("page"));
    }

    #[test]
    fn personal_entries_use_the_recorder_names_and_default_weight() {
        let entry =
            entry_from(&json!({"kind": "quick_phrase", "key": "k1", "value": "v"})).unwrap();
        assert_eq!(entry.kind, PersonalDictionaryKind::QuickPhrase);
        assert_eq!(entry.weight, PersonalDictionaryEntry::DEFAULT_WEIGHT);
        assert_eq!(
            entry_json(&entry),
            json!({"kind": "quick_phrase", "key": "k1", "value": "v", "weight": 100000})
        );
        assert!(entry_from(&Value::Null).is_none());
        assert_eq!(source_from(&json!("ai")), CandidateSource::AiSuggestion);
        assert_eq!(source_from(&json!(2)), CandidateSource::CloudSuggestion);
    }
}
