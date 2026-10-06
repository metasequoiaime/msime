//! The stateless input modes the server offers under `/v1/input/{operation}`: Unicode, date and time, romaji, emoji, kaomoji, jianpin, quick phrases, English completion and glosses. Each is the engine's own query for that mode, called directly rather than through a session, so the request's `limit` applies as given.

use std::path::Path;

use serde_json::{json, Value};

use super::common::{candidates, scheme, shuangpin_profile, Roots};
use super::request::Request;
use super::{BackendError, Outcome};
use crate::assets;
use crate::dictionary::english::EnglishDictionary;
use crate::japanese::romaji::{convert_romaji, hiragana_to_katakana, hiragana_to_romaji};
use crate::local::date_time::query_date_time_with_limit;
use crate::local::emoji::{query_emoji, query_kaomoji};
use crate::local::jianpin::query_jianpin;
use crate::local::quick_phrase::query_quick_phrases_with_limit;
use crate::local::unicode::query_unicode;
use crate::local::LocalQueryResult;
use crate::LocalDateTime;

/// The keywords the date and time mode answers.
const DATE_TIME_KEYWORDS: [&str; 9] = [
    "rq", "riqi", "date", "sj", "shijian", "time", "xq", "xingqi", "week",
];

pub(super) fn unicode(request: &Request) -> Outcome {
    let mut items = query_unicode(request.text());
    items.truncate(request.limit());
    Ok(candidates(&items))
}

/// `date` is the caller's local time, `{year, month, day, weekday (0 = Sunday), hour, minute, second}`; the server builds it in the user's time zone.
pub(super) fn date_time(request: &Request) -> Outcome {
    let date = request.required("date")?;
    let field = |name: &str| -> Result<i64, BackendError> {
        date.get(name)
            .and_then(Value::as_i64)
            .ok_or(BackendError::InvalidRequest)
    };
    let (year, month, day, weekday) = (
        field("year")?,
        field("month")?,
        field("day")?,
        field("weekday")?,
    );
    let (hour, minute, second) = (field("hour")?, field("minute")?, field("second")?);
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..=6).contains(&weekday)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
        || !DATE_TIME_KEYWORDS.contains(&request.text())
    {
        return Err(BackendError::InvalidRequest);
    }
    // Every value is range-checked above, so the narrowing conversions are exact.
    let now = LocalDateTime {
        year: year as i32,
        month: month as u32,
        day: day as u32,
        weekday: weekday as u32,
        hour: hour as u32,
        minute: minute as u32,
        second: second as u32,
    };
    Ok(candidates(&query_date_time_with_limit(
        request.text(),
        &now,
        request.limit(),
    )))
}

/// `direction`: `romaji-hiragana` (the default, with the letters that do not form kana yet), `hiragana-katakana` or `kana-romaji`.
pub(super) fn romaji(request: &Request) -> Outcome {
    let text = request.text();
    match request.string_or("direction", "romaji-hiragana")? {
        "romaji-hiragana" => {
            let conversion = convert_romaji(text);
            Ok(json!({
                "text": conversion.hiragana,
                "pending": conversion.pending,
                "complete": conversion.complete,
            }))
        }
        "hiragana-katakana" => Ok(json!({ "text": hiragana_to_katakana(text) })),
        "kana-romaji" => Ok(json!({ "text": hiragana_to_romaji(text) })),
        _ => Err(BackendError::InvalidRequest),
    }
}

fn require_file(path: &Path) -> Result<(), BackendError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(BackendError::ResourcesUnavailable)
    }
}

fn local_result(result: LocalQueryResult) -> Outcome {
    if result.diagnostic.is_some() {
        return Err(BackendError::ResourcesUnavailable);
    }
    Ok(candidates(&result.candidates))
}

/// `emoji`, `kaomoji`, `jianpin` and `quick`. Jianpin and quick phrases read the main dictionary, which a personal query replaces with the user's replayed copy; emoji and kaomoji read the shipped catalog.
pub(super) fn local_mode(request: &Request, roots: Roots) -> Outcome {
    let scheme = scheme(request)?;
    let profile = shuangpin_profile(request)?;
    roots.require_resources()?;
    let (text, limit) = (request.text(), request.limit());
    let result = match request.operation() {
        "emoji" | "kaomoji" => {
            let path = roots.resource(assets::OTHER_DICTIONARY);
            require_file(&path)?;
            if request.operation() == "emoji" {
                query_emoji(text, scheme, &path, limit, profile)
            } else {
                query_kaomoji(text, scheme, &path, limit, profile)
            }
        }
        operation => {
            let path = roots.dictionary(assets::MAIN_DICTIONARY);
            require_file(&path)?;
            if operation == "jianpin" {
                query_jianpin(text, scheme, &path, limit, profile)
            } else {
                query_quick_phrases_with_limit(text, &path, limit)
            }
        }
    };
    local_result(result)
}

fn english_dictionary(roots: Roots) -> Result<EnglishDictionary, BackendError> {
    roots.require_resources()?;
    let path = roots.dictionary(assets::ENGLISH_DICTIONARY);
    require_file(&path)?;
    let dictionary = EnglishDictionary::open(&path, None, None);
    if !dictionary.ready() {
        return Err(BackendError::ResourcesUnavailable);
    }
    Ok(dictionary)
}

/// English words starting with `text`, most frequent first.
pub(super) fn english(request: &Request, roots: Roots) -> Outcome {
    scheme(request)?;
    let dictionary = english_dictionary(roots)?;
    Ok(candidates(
        &dictionary.query_prefix(request.text(), request.limit()),
    ))
}

/// `direction`: `en-zh` (the default) gives the Chinese gloss of an English word, `zh-en` the English gloss of a Chinese word.
pub(super) fn gloss(request: &Request, roots: Roots) -> Outcome {
    scheme(request)?;
    let dictionary = english_dictionary(roots)?;
    let text = match request.string_or("direction", "en-zh")? {
        "en-zh" => dictionary.query_chinese_gloss(request.text()),
        "zh-en" => dictionary.query_english_gloss(request.text()),
        _ => return Err(BackendError::InvalidRequest),
    };
    Ok(json!({ "text": text }))
}
