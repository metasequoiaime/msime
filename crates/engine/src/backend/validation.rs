//! Personal dictionary entry validation for the server's dictionary endpoints and community resources. The rules are the engine's own (`validate_personal_dictionary_entry`), so an entry the server stores is one every client accepts on replay.

use serde_json::{json, Value};

use super::request::Request;
use super::{BackendError, Outcome};
use crate::{validate_personal_dictionary_entry, PersonalDictionaryEntry, PersonalDictionaryKind};

/// 服务端存储的词条种类，与服务端数据库 `user_dictionary_entries.kind` 的约束一致；`wubi` 是 86 版五笔，`wubi98` 是 98 版。
fn kind(name: &str) -> Result<PersonalDictionaryKind, BackendError> {
    match name {
        "pinyin" => Ok(PersonalDictionaryKind::Pinyin),
        "wubi" => Ok(PersonalDictionaryKind::Wubi),
        "wubi98" => Ok(PersonalDictionaryKind::Wubi98),
        "quick" => Ok(PersonalDictionaryKind::QuickPhrase),
        "english" => Ok(PersonalDictionaryKind::English),
        _ => Err(BackendError::InvalidRequest),
    }
}

/// `{"kind","code","text","weight"?}` → `{"kind","code","word","weight"}` with the code and word normalised.
pub(super) fn validate_dictionary(request: &Request) -> Outcome {
    let name = request.string("kind")?;
    let weight = match request.field("weight") {
        None => PersonalDictionaryEntry::DEFAULT_WEIGHT,
        Some(value) => value.as_i64().ok_or(BackendError::InvalidRequest)?,
    };
    let entry = PersonalDictionaryEntry {
        kind: kind(name)?,
        key: request.string("code")?.to_owned(),
        value: request.text().to_owned(),
        weight,
    };
    let validated = validate_personal_dictionary_entry(&entry)
        .map_err(|_| BackendError::InvalidDictionaryEntry)?;
    Ok(json!({
        "kind": name,
        "code": validated.key,
        "word": validated.value,
        "weight": validated.weight,
    }))
}

/// `{"entries":[{kind,code,text,weight}]}` → `{"entries":[...]}`; the first entry that fails decides the answer.
pub(super) fn validate_dictionary_batch(request: &Request) -> Outcome {
    let entries = request.batch("entries")?;
    let mut validated = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut single = entry.clone();
        single
            .as_object_mut()
            .ok_or(BackendError::InvalidRequest)?
            .insert("operation".into(), Value::from("validate_dictionary"));
        validated.push(validate_dictionary(&Request::new(&single)?)?);
    }
    Ok(json!({ "entries": validated }))
}
