//! Operations on a piece of text rather than on typed input: helpcodes of Chinese text, the pinyin reading of a Chinese word, and Japanese conversion of romaji.

use serde_json::{json, Value};

use super::common::{candidates, scheme, Roots};
use super::request::Request;
use super::{BackendError, Outcome};
use crate::dictionary::hanzi::hanzi_to_pinyin;
use crate::helpcode::{compute_helpcodes, load_helpcode_keymap};
use crate::japanese::decoder::JapaneseDictionary;
use crate::japanese::provider::JapaneseProvider;
use crate::japanese::romaji::convert_romaji;
use crate::japanese::scheme::JapaneseRomajiScheme;
use crate::{
    assets, validate_personal_dictionary_entry, PersonalDictionaryEntry, PersonalDictionaryKind,
};

/// The helpcode schemas the server offers. The engine also knows `jiajia` and user-supplied `custom/` tables, which have no place on a shared server.
const HELPCODE_SCHEMAS: [&str; 5] = ["lantian", "ziranma", "shouyou2_0", "shouyouplus", "xiaohe"];

/// `{"text": "(rN)", "schema": "lantian"}`: the helpcode of each character of `text` under `schema`.
pub(super) fn helpcode(request: &Request, roots: Roots) -> Outcome {
    scheme(request)?;
    roots.require_resources()?;
    let schema = request.string_or("schema", "lantian")?;
    if !HELPCODE_SCHEMAS.contains(&schema) {
        return Err(BackendError::InvalidRequest);
    }
    let keymap = load_helpcode_keymap(roots.resources, schema)
        .map_err(|_| BackendError::ResourcesUnavailable)?;
    if keymap.is_empty() {
        return Err(BackendError::ResourcesUnavailable);
    }
    Ok(json!({
        "text": compute_helpcodes(request.text(), false, &keymap),
        "schema": schema,
    }))
}

/// `{"code": "chong'qing'yin'hang", "word": "重庆银行"}`. The reading is the dictionary's own key for the word when it has one, otherwise each character's most frequent reading; it is then checked as a personal pinyin entry, so the code the server stores is one a client accepts.
pub(super) fn annotate(request: &Request, roots: Roots) -> Outcome {
    scheme(request)?;
    roots.require_resources()?;
    let dictionary = roots.resource(assets::MAIN_DICTIONARY);
    if !dictionary.is_file() {
        return Err(BackendError::ResourcesUnavailable);
    }
    let text = request.text();
    let code = hanzi_to_pinyin(&dictionary, text);
    if code.is_empty() {
        return Err(BackendError::InvalidRequest);
    }
    let validated = validate_personal_dictionary_entry(&PersonalDictionaryEntry {
        kind: PersonalDictionaryKind::Pinyin,
        key: code,
        value: text.to_owned(),
        weight: 10,
    })
    .map_err(|_| BackendError::InvalidDictionaryEntry)?;
    Ok(json!({ "code": validated.key, "word": text }))
}

/// `{"words": [...]}` → `{"entries": [{code, word}]}`; the first word without a reading decides the answer.
pub(super) fn annotate_batch(request: &Request, roots: Roots) -> Outcome {
    let words = request.batch("words")?;
    let mut entries = Vec::with_capacity(words.len());
    for word in words {
        let single = json!({ "operation": "annotate", "text": word.as_str().ok_or(BackendError::InvalidRequest)? });
        entries.push(annotate(&Request::new(&single)?, roots)?);
    }
    Ok(json!({ "entries": entries }))
}

/// Japanese candidates for romaji `text`, with the hiragana it spells and the letters still pending.
pub(super) fn japanese(request: &Request, roots: Roots) -> Outcome {
    roots.require_resources()?;
    let model = roots.resource(assets::JAPANESE_MODEL);
    if JapaneseDictionary::shared(&model).is_none() {
        return Err(BackendError::ResourcesUnavailable);
    }
    let text = request.text();
    let mut input = JapaneseRomajiScheme::new();
    input.set_raw_input(text, text);
    let query = input.build_request();
    if !query.valid {
        return Err(BackendError::InvalidRequest);
    }
    let mut items = JapaneseProvider::new(&model).query(&query);
    items.truncate(request.limit());
    let conversion = convert_romaji(text);
    let mut response = candidates(&items);
    response["hiragana"] = Value::from(conversion.hiragana);
    response["pending"] = Value::from(conversion.pending);
    response["complete"] = Value::from(conversion.complete);
    Ok(response)
}

/// `{"text": "頭髮", "conversion": "s2t"}`: OpenCC's Simplified to Traditional conversion.
pub(super) fn convert(
    request: &Request,
    roots: Roots,
    simplified_to_traditional: super::SimplifiedToTraditional,
) -> Outcome {
    scheme(request)?;
    roots.require_resources()?;
    Ok(json!({
        "text": simplified_to_traditional(request.text()),
        "conversion": "s2t",
    }))
}
