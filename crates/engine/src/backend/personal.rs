//! A user's personal dictionary, answered statelessly. The server keeps each user's overlay, fixed positions and selection counters in PostgreSQL and streams them into `<scratch>/snapshot.jsonl` before every request; this module writes them into a fresh journal under the scratch directory, replays it over a copy of the shipped dictionaries, and answers from that copy. Nothing survives the request: the server stores whatever the response says changed.
//!
//! The journal is the engine's own (`user_dictionary_operations`, `fixed_candidate_positions`, `candidate_selection_state`), unchanged since the C++ engine, so the rows the server stored through the old bridge replay as they always did.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use super::common::{candidates, scheme, shuangpin_profile, wubi_profile, Roots};
use super::input::registry;
use super::request::{member, Request};
use super::{route, validation, BackendError, Outcome};
use crate::assets;
use crate::local::jianpin::jianpin_ranking_context;
use crate::pinyin::segment::{cut_pinyin_by_mode, join_segments, CutMode};
use crate::types::{CandidateSource, FrequencyAdjustmentMode, SchemeType, WordItem};
use crate::user_dictionary::journal::{ensure_user_database, is_user_inserted, open_database};
use crate::user_dictionary::positions::apply_fixed_positions;
use crate::user_dictionary::ranking::{
    adjust_candidate_ranking, adjust_english_candidate_ranking, RankingRequest,
};
use crate::user_dictionary::removal::delete_dictionary_candidate;
use crate::{prepare_runtime_paths, PersonalDictionaryKind};

/// The file the server writes into the scratch directory before the request.
const SNAPSHOT: &str = "snapshot.jsonl";
/// The longest snapshot line read.
const MAXIMUM_LINE_BYTES: usize = 64 * 1024;
/// A personal query reads this many candidates before fixed positions are applied, whatever the request's limit.
const PERSONAL_QUERY_LIMIT: i64 = 200;
/// The generation id the replayed copy is staged under.
const CONTENT_ID: &str = "backend";

const UPSERT_ENTRY: &str = "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation=excluded.operation,weight=excluded.weight,display=excluded.display,user_inserted=excluded.user_inserted";
const INSERT_POSITION: &str = "INSERT INTO fixed_candidate_positions(context_key,entry_key,value,position) VALUES(?1,?2,?3,?4)";
const INSERT_SELECTION: &str = "INSERT INTO candidate_selection_state(context_key,entry_key,value,selection_count) VALUES(?1,?2,?3,?4)";

/// 服务端存储的词条可能的种类，与服务端数据库 `user_dictionary_entries.kind` 的约束一致。
const STORED_KINDS: [&str; 5] = ["pinyin", "wubi", "wubi98", "english", "quick"];

fn require_roots(roots: Roots) -> Result<(), BackendError> {
    let absolute = |path: &Path| !path.as_os_str().is_empty() && path.is_absolute();
    if !absolute(roots.scratch) || !absolute(roots.resources) {
        return Err(BackendError::ResourcesUnavailable);
    }
    Ok(())
}

/// Each line of the snapshot, parsed. A line that is too long, or a read error, is an engine failure; a line that is not JSON is an invalid request.
fn snapshot_lines(
    scratch: &Path,
) -> Result<impl Iterator<Item = Result<Value, BackendError>>, BackendError> {
    let file = crate::paths::open_file_no_follow(&scratch.join(SNAPSHOT))
        .map_err(|_| BackendError::EngineFailure)?;
    Ok(BufReader::new(file).lines().map(|line| {
        let line = line.map_err(|_| BackendError::EngineFailure)?;
        if line.len() > MAXIMUM_LINE_BYTES {
            return Err(BackendError::EngineFailure);
        }
        serde_json::from_str(&line).map_err(|_| BackendError::InvalidRequest)
    }))
}

/// What the snapshot held besides the journal rows.
struct Snapshot {
    revision: i64,
    /// Any `previous`/`replacement` line: only then does the dictionary differ from the shipped one.
    has_overlay: bool,
}

/// Write the snapshot into `journal` in one transaction. `{"previous": E}` is a tombstone for E, `{"replacement": E}` its final state; `fixed` and `selection` lines are the user's positions and counters.
fn write_snapshot(journal: &Path, scratch: &Path) -> Result<Snapshot, BackendError> {
    let mut connection = open_database(journal, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_| BackendError::EngineFailure)?;
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| BackendError::EngineFailure)?;
    let mut snapshot = Snapshot {
        revision: 0,
        has_overlay: false,
    };
    {
        let failure = |_| BackendError::EngineFailure;
        let mut entry = transaction.prepare(UPSERT_ENTRY).map_err(failure)?;
        let mut position = transaction.prepare(INSERT_POSITION).map_err(failure)?;
        let mut selection = transaction.prepare(INSERT_SELECTION).map_err(failure)?;
        let mut write_entry = |value: &Value, deleted: bool| -> Result<(), BackendError> {
            if value.is_null() {
                return Ok(());
            }
            let kind = member(value, "kind")?;
            if !STORED_KINDS.contains(&kind) {
                return Err(BackendError::EngineFailure);
            }
            let word = member(value, "word")?;
            let weight = if deleted {
                0
            } else {
                value
                    .get("weight")
                    .and_then(Value::as_i64)
                    .ok_or(BackendError::InvalidRequest)?
            };
            let display = if !deleted && kind == "english" {
                word
            } else {
                ""
            };
            let inserted = !deleted
                && match value.get("user_inserted") {
                    None => true,
                    Some(flag) => flag.as_bool().ok_or(BackendError::InvalidRequest)?,
                };
            entry
                .execute(params![
                    kind,
                    member(value, "code")?,
                    word,
                    if deleted { "delete" } else { "upsert" },
                    weight,
                    display,
                    inserted,
                ])
                .map_err(|_| BackendError::EngineFailure)?;
            Ok(())
        };
        let keyed = |statement: &mut rusqlite::Statement, value: &Value, number: &str| {
            let number = value
                .get(number)
                .and_then(Value::as_i64)
                .ok_or(BackendError::InvalidRequest)?;
            statement
                .execute(params![
                    member(value, "context")?,
                    member(value, "code")?,
                    member(value, "word")?,
                    number,
                ])
                .map(|_| ())
                .map_err(|_| BackendError::EngineFailure)
        };
        for line in snapshot_lines(scratch)? {
            let change = line?;
            if let Some(revision) = change.get("snapshot_revision") {
                snapshot.revision = revision.as_i64().ok_or(BackendError::InvalidRequest)?;
            } else if let Some(value) = change.get("selection") {
                keyed(&mut selection, value, "count")?;
            } else if let Some(value) = change.get("fixed") {
                keyed(&mut position, value, "position")?;
            } else {
                snapshot.has_overlay = true;
                let previous = change.get("previous").ok_or(BackendError::InvalidRequest)?;
                let replacement = change
                    .get("replacement")
                    .ok_or(BackendError::InvalidRequest)?;
                write_entry(previous, true)?;
                write_entry(replacement, false)?;
            }
        }
    }
    transaction
        .commit()
        .map_err(|_| BackendError::EngineFailure)?;
    Ok(snapshot)
}

/// The key a candidate is stored under: the dictionary's complete key when the row has one, else the code it matched.
fn stored_code(item: &WordItem) -> &str {
    if item.canonical_pinyin.is_empty() {
        &item.pinyin
    } else {
        &item.canonical_pinyin
    }
}

/// `personal_query`, `personal_rank` and `personal_delete`: `query` is an ordinary `candidates`, `english`, `quick`, `jianpin` or `dictionary` request, answered against the user's dictionary; rank and delete then apply `action` (`{code, word, mode?, linear_step?, trigger_count?, force_top?}`) to the candidate it names.
pub(super) fn personal(request: &Request, roots: Roots) -> Outcome {
    require_roots(roots)?;
    let user = roots.scratch.join("user");
    std::fs::create_dir_all(&user).map_err(|_| BackendError::EngineFailure)?;
    let journal = user.join(assets::USER_JOURNAL);
    ensure_user_database(&journal).map_err(|_| BackendError::EngineFailure)?;
    let snapshot = write_snapshot(&journal, roots.scratch)?;

    let mut nested = request.required("query")?.clone();
    let nested_fields = nested.as_object_mut().ok_or(BackendError::InvalidRequest)?;
    let operation = nested_fields
        .get("operation")
        .and_then(Value::as_str)
        .ok_or(BackendError::InvalidRequest)?
        .to_owned();
    if !["candidates", "english", "quick", "jianpin", "dictionary"].contains(&operation.as_str()) {
        return Err(BackendError::InvalidRequest);
    }
    let changes_ranking = request.operation() != "personal_query";
    let generation: PathBuf = if snapshot.has_overlay || changes_ranking {
        prepare_runtime_paths(
            roots.resources,
            &user,
            &roots.scratch.join("cache"),
            CONTENT_ID,
        )
        .map_err(|_| BackendError::EngineFailure)?
        .dictionaries
    } else {
        roots.resources.to_path_buf()
    };
    let projected = Roots {
        dictionaries: &generation,
        ..roots
    };
    let requested_limit = match nested_fields.get("limit") {
        None => 20,
        Some(value) => value.as_i64().ok_or(BackendError::InvalidRequest)?,
    };
    if operation != "dictionary" {
        nested_fields.insert("limit".into(), Value::from(PERSONAL_QUERY_LIMIT));
    }
    let nested = Request::new(&nested)?;
    let mut response = route(&nested, projected)?;
    if operation == "dictionary" {
        response["revision"] = Value::from(snapshot.revision);
        return Ok(response);
    }

    let text = nested.text();
    let scheme = scheme(&nested)?;
    let profile = shuangpin_profile(&nested)?;
    let wubi = wubi_profile(&nested)?;
    let mut ranking_context = String::new();
    let context = match operation.as_str() {
        "english" => format!("english:{text}"),
        "jianpin" => jianpin_ranking_context(text, scheme, profile),
        "candidates" => {
            let mut context = response["normalized_segmentation"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            ranking_context = if scheme == SchemeType::Wubi {
                text.to_owned()
            } else {
                context.clone()
            };
            if scheme != SchemeType::Wubi && text.len() != 1 {
                let plain: String = context
                    .chars()
                    .filter(|&character| character != '\'')
                    .collect();
                if let Some(first) = cut_pinyin_by_mode(&plain, CutMode::Correction).first() {
                    context = join_segments(first);
                }
            }
            context
        }
        _ => String::new(),
    };
    let source = if operation == "english" {
        CandidateSource::EnglishDictionary
    } else {
        CandidateSource::Database
    };
    let mut items = response["candidates"]
        .as_array()
        .ok_or(BackendError::EngineFailure)?
        .iter()
        .map(|item| -> Result<WordItem, BackendError> {
            Ok(WordItem::new(
                member(item, "code")?,
                member(item, "word")?,
                item["weight"].as_i64().ok_or(BackendError::EngineFailure)?,
                source,
                item["canonical_pinyin"].as_str().unwrap_or_default(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if changes_ranking {
        if operation == "quick" {
            return Err(BackendError::InvalidRequest);
        }
        let kind = if operation == "english" {
            PersonalDictionaryKind::English
        } else if scheme == SchemeType::Wubi {
            wubi.dictionary_kind()
        } else {
            PersonalDictionaryKind::Pinyin
        };
        let dictionary = projected.dictionary(if kind == PersonalDictionaryKind::English {
            assets::ENGLISH_DICTIONARY
        } else {
            assets::MAIN_DICTIONARY
        });
        let context = if operation == "candidates" {
            &ranking_context
        } else {
            &context
        };
        let mut result = apply_action(
            request.required("action")?,
            request.operation() == "personal_delete",
            context,
            &items,
            kind,
            &dictionary,
            &journal,
        )?;
        result["revision"] = Value::from(snapshot.revision);
        return Ok(result);
    }

    let include_missing =
        operation == "candidates" && scheme != SchemeType::Wubi && text.len() == 1;
    if include_missing {
        let providers = registry(projected, profile, wubi)?;
        let mut find = |key: &str, word: &str| providers.find_candidate(scheme, key, word);
        apply_fixed_positions(&journal, &context, &mut items, true, Some(&mut find), false);
    } else {
        apply_fixed_positions(
            &journal,
            &context,
            &mut items,
            false,
            None,
            operation == "english",
        );
    }
    items.truncate(usize::try_from(requested_limit).unwrap_or(0));
    response["candidates"] = candidates(&items)["candidates"].take();
    response["context"] = Value::from(context);
    response["revision"] = Value::from(snapshot.revision);
    Ok(response)
}

/// Delete the named candidate, or record a pick of it and let the engine move it, then report what the server must store: the deleted entry, or every candidate whose journal weight now differs from the one it was shown with, and the pick counter.
fn apply_action(
    action: &Value,
    remove: bool,
    context: &str,
    items: &[WordItem],
    kind: PersonalDictionaryKind,
    dictionary: &Path,
    journal: &Path,
) -> Outcome {
    let key = member(action, "code")?;
    let word = member(action, "word")?;
    let selected = items
        .iter()
        .find(|item| stored_code(item) == key && item.word == word)
        .ok_or(BackendError::InvalidRequest)?;
    if context.is_empty() {
        return Err(BackendError::InvalidRequest);
    }
    let kind_name = kind.journal_name();
    if remove {
        // A single character is never removed from the pinyin or wubi dictionary: it may be the only way to type it.
        if kind != PersonalDictionaryKind::English && word.chars().count() <= 1 {
            return Err(BackendError::InvalidRequest);
        }
        let inserted = is_user_inserted(journal, kind, key, word);
        delete_dictionary_candidate(dictionary, journal, kind, key, word)
            .map_err(|_| BackendError::EngineFailure)?;
        return Ok(json!({
            "deleted": {
                "kind": kind_name,
                "code": key,
                "word": word,
                "weight": selected.weight,
                "user_inserted": inserted,
            },
            "changed": true,
        }));
    }

    let integer = |name: &str, default: i64| match action.get(name) {
        None => Ok(default),
        Some(value) => value.as_i64().ok_or(BackendError::InvalidRequest),
    };
    let mode = match action.get("mode") {
        None => FrequencyAdjustmentMode::Pin,
        Some(value) => value
            .as_str()
            .and_then(FrequencyAdjustmentMode::from_name)
            .ok_or(BackendError::InvalidRequest)?,
    };
    let linear_step = integer("linear_step", 1)?;
    let trigger_count = integer("trigger_count", 1)?;
    let force_top = match action.get("force_top") {
        None => false,
        Some(value) => value.as_bool().ok_or(BackendError::InvalidRequest)?,
    };
    if !(1..=100).contains(&linear_step) || !(1..=10).contains(&trigger_count) {
        return Err(BackendError::InvalidRequest);
    }
    let ranking = RankingRequest {
        main_db: dictionary,
        user_db: journal,
        context_key: context,
        ordered: items,
        entry_key: key,
        value: word,
        mode,
        // Both are range-checked above.
        linear_step: linear_step as i32,
        trigger_count: trigger_count as i32,
        force_top,
        kind,
    };
    let weight_changed = if kind == PersonalDictionaryKind::English {
        adjust_english_candidate_ranking(&ranking).map(|()| false)
    } else {
        adjust_candidate_ranking(&ranking)
    }
    .map_err(|_| BackendError::InvalidRequest)?;

    let connection = open_database(journal, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| BackendError::EngineFailure)?;
    let count = connection
        .query_row(
            "SELECT selection_count FROM candidate_selection_state WHERE context_key=?1 AND entry_key=?2 AND value=?3",
            params![context, key, word],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let mut weights = connection.prepare(
        "SELECT weight FROM user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3 AND operation='upsert'",
    )?;
    let mut updates = Vec::new();
    for item in items {
        let code = stored_code(item);
        let Some(updated) = weights
            .query_row(params![kind_name, code, item.word], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?
        else {
            continue;
        };
        if updated != item.weight {
            updates.push(json!({
                "kind": kind_name,
                "code": code,
                "word": item.word,
                "weight": updated,
                "user_inserted": is_user_inserted(journal, kind, code, &item.word),
            }));
        }
    }
    // English ranking does not say whether it wrote; any changed weight is the answer.
    let changed = weight_changed || !updates.is_empty();
    Ok(json!({
        "updates": updates,
        "selection": {"context": context, "code": key, "word": word, "count": count},
        "changed": changed,
    }))
}

/// `validate_snapshot`: every `entry` and `overlay` record of an exported snapshot (`<scratch>/snapshot.jsonl`, `{"type", "data", "deleted"?}`) must be a valid entry whose code and word are already normalised. Deleted overlay rows carry no weight of their own and are checked at weight 10.
pub(super) fn validate_snapshot(roots: Roots) -> Outcome {
    if roots.scratch.as_os_str().is_empty() {
        return Err(BackendError::EngineFailure);
    }
    for line in snapshot_lines(roots.scratch)? {
        let record = line?;
        let kind = member(&record, "type")?;
        if kind != "entry" && kind != "overlay" {
            continue;
        }
        let data = record.get("data").ok_or(BackendError::InvalidRequest)?;
        let deleted = kind == "overlay"
            && record
                .get("deleted")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let weight = data
            .get("weight")
            .and_then(Value::as_i64)
            .ok_or(BackendError::InvalidRequest)?;
        let code = member(data, "code")?;
        let word = member(data, "word")?;
        let single = json!({
            "operation": "validate_dictionary",
            "kind": member(data, "kind")?,
            "code": code,
            "text": word,
            "weight": if deleted { 10 } else { weight },
        });
        let validated = Request::new(&single)
            .and_then(|request| validation::validate_dictionary(&request))
            .map_err(|_| BackendError::InvalidRequest)?;
        if validated["code"] != code || validated["word"] != word {
            return Err(BackendError::InvalidRequest);
        }
    }
    Ok(json!({ "valid": true }))
}
