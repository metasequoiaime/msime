//! Display-only glosses and English completions (api-contract §1c, bridge.cpp:149-237, 554-572, 1138-1236).

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::assets;
use crate::diagnostics;
use crate::dictionary::english::{load_custom_translations, upsert_gloss, EnglishDictionary};
use crate::error::{EngineError, Result};
use crate::types::CandidateSource;

/// Senses shown beside a candidate; the rest of a dictionary gloss is cut (bridge.cpp:209).
const MAXIMUM_GLOSS_SENSES: usize = 2;
const FULLWIDTH_SENSE_DELIMITER: &str = "；";
const MAXIMUM_ENGLISH_COMPLETIONS: usize = 32;

/// ASCII letters only, `limit` 1..=32, lowercased; empty prefix gives nothing (`INVALID_ENGLISH_COMPLETION_*`, `ENGLISH_DICTIONARY_UNAVAILABLE`).
pub fn english_completions(resources: &str, prefix: &str, limit: usize) -> Result<Vec<String>> {
    if limit == 0 || limit > MAXIMUM_ENGLISH_COMPLETIONS {
        return Err(EngineError::invalid(
            diagnostics::INVALID_ENGLISH_COMPLETION_LIMIT,
        ));
    }
    if !prefix.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(EngineError::invalid(
            diagnostics::INVALID_ENGLISH_COMPLETION_PREFIX,
        ));
    }
    if prefix.is_empty() {
        return Ok(Vec::new());
    }
    let dictionary = EnglishDictionary::open(
        &Path::new(resources).join(assets::ENGLISH_DICTIONARY),
        None,
        None,
    );
    if !dictionary.ready() {
        return Err(EngineError::failed(
            diagnostics::ENGLISH_DICTIONARY_UNAVAILABLE,
        ));
    }
    // `word` is the display spelling, which is what the C++ `WordItem::word` returned too.
    Ok(dictionary
        .query_prefix(&prefix.to_ascii_lowercase(), limit)
        .into_iter()
        .map(|item| item.word)
        .collect())
}

/// Parallel to `candidates`; empty for ineligible rows (emoji, kaomoji, neither Latin nor Han).
pub fn candidate_glosses(resources: &str, candidates: &[(String, u8)]) -> Result<Vec<String>> {
    candidate_glosses_with_user(resources, "", candidates)
}

/// Hand-written translations outrank learned glosses, which outrank packaged ones; at most two senses shown. Empty `resources` means the learned store only.
pub fn candidate_glosses_with_user(
    resources: &str,
    user_data: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>> {
    // An empty resource path requests only the user overlay, never a relative `english.db` (bridge.cpp:1161-1162).
    let packaged = if resources.is_empty() {
        None
    } else {
        open_gloss_dictionary(&Path::new(resources).join(assets::ENGLISH_DICTIONARY))
    };
    // The user's own `custom_translations.txt` is read directly. The bridge reached it only as the learned store's sidecar (bridge.cpp:1147-1166), so before the first online gloss created `translation-glosses.db` the file Settings writes was ignored.
    let custom = if user_data.is_empty() {
        Default::default()
    } else {
        load_custom_translations(&Path::new(user_data).join(assets::TRANSLATIONS))
    };
    let learned = if user_data.is_empty() {
        None
    } else {
        open_gloss_dictionary(&Path::new(user_data).join(assets::LEARNED_GLOSSES))
    };
    if packaged.is_none() && learned.is_none() && custom.en_zh.is_empty() && custom.zh_en.is_empty()
    {
        return Err(EngineError::failed(
            diagnostics::CANDIDATE_GLOSS_UNAVAILABLE,
        ));
    }
    let lookup = |dictionary: &EnglishDictionary, key: &str, chinese_to_english: bool| {
        candidate_gloss_display(&if chinese_to_english {
            dictionary.query_english_gloss(key)
        } else {
            dictionary.query_chinese_gloss(key)
        })
    };
    Ok(candidates
        .iter()
        .map(|(text, source)| {
            let Some((key, chinese_to_english)) = candidate_gloss_key(text, *source) else {
                return String::new();
            };
            let hand_written = if chinese_to_english {
                custom.zh_en.get(&key)
            } else {
                custom.en_zh.get(&key)
            };
            if let Some(gloss) = hand_written {
                return candidate_gloss_display(gloss);
            }
            let gloss = learned
                .as_ref()
                .map(|dictionary| lookup(dictionary, &key, chinese_to_english))
                .unwrap_or_default();
            if !gloss.is_empty() {
                return gloss;
            }
            packaged
                .as_ref()
                .map(|dictionary| lookup(dictionary, &key, chinese_to_english))
                .unwrap_or_default()
        })
        .collect())
}

pub fn save_candidate_gloss(
    user_data: &str,
    chinese_to_english: bool,
    key: &str,
    gloss: &str,
) -> bool {
    upsert_gloss(
        &Path::new(user_data).join(assets::LEARNED_GLOSSES),
        chinese_to_english,
        key,
        gloss,
    )
}

/// Chinese rows only, from an offline `zh-<lang>.db` at `user_version = 1` whose `meta.target_language` matches; never creates the file.
pub fn candidate_target_glosses(
    database_path: &str,
    target_language: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>> {
    // An empty path would open a private temporary database rather than fail.
    if database_path.is_empty() {
        return Err(EngineError::failed(diagnostics::OFFLINE_GLOSS_UNAVAILABLE));
    }
    let connection = Connection::open_with_flags(
        database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .map_err(|_| EngineError::failed(diagnostics::OFFLINE_GLOSS_UNAVAILABLE))?;
    // bridge.cpp:1194 set no busy timeout; rusqlite's default 5 s wait is not the reference's behaviour.
    connection
        .busy_timeout(std::time::Duration::ZERO)
        .map_err(|_| EngineError::failed(diagnostics::OFFLINE_GLOSS_UNAVAILABLE))?;
    let unreadable = |_| EngineError::failed(diagnostics::OFFLINE_GLOSS_UNREADABLE);
    // SQLite opens lazily, so a file that is not a database first fails here, as it did in the C++ prepare (bridge.cpp:1198-1206).
    let version = connection
        .prepare("PRAGMA user_version")
        .map_err(unreadable)?
        .query_row((), |row| row.get::<_, i64>(0))
        // A step that fails reads as a version the host cannot use, as the C++ check did (bridge.cpp:1207-1208).
        .ok();
    if version != Some(1) {
        return Err(EngineError::failed(
            diagnostics::OFFLINE_GLOSS_VERSION_UNSUPPORTED,
        ));
    }
    let stored = connection
        .prepare("SELECT value FROM meta WHERE key = 'target_language'")
        .map_err(unreadable)?
        .query_row((), |row| row.get::<_, Option<String>>(0))
        .ok()
        .flatten();
    if stored.as_deref() != Some(target_language) {
        return Err(EngineError::failed(
            diagnostics::OFFLINE_GLOSS_LANGUAGE_MISMATCH,
        ));
    }
    let mut lookup = connection
        .prepare("SELECT gloss FROM zh_glosses WHERE chinese = ?1")
        .map_err(unreadable)?;
    let mut output = Vec::with_capacity(candidates.len());
    for (text, source) in candidates {
        let gloss = match candidate_gloss_key(text, *source) {
            Some((key, true)) => lookup
                .query_row((key,), |row| row.get::<_, Option<String>>(0))
                .optional()
                .map_err(|_| EngineError::failed(diagnostics::OFFLINE_GLOSS_READ_FAILED))?
                .flatten()
                .map(|gloss| candidate_gloss_display(&gloss))
                .unwrap_or_default(),
            _ => String::new(),
        };
        output.push(gloss);
    }
    Ok(output)
}

/// `open_dictionary` (bridge.cpp:1150-1158): only a regular file whose prefix statement prepares.
fn open_gloss_dictionary(path: &Path) -> Option<EnglishDictionary> {
    if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    Some(EnglishDictionary::open(path, None, None)).filter(EnglishDictionary::ready)
}

/// `candidate_gloss_key` (bridge.cpp:149-188): `(key, chinese_to_english)`, or `None` for a row that gets no gloss. Emoji and kaomoji never do; text of ASCII letters, spaces, `-` and `'` with at least one letter is looked up lowercased as English; otherwise any Han character makes the whole text a Chinese key. `&str` is valid UTF-8, so the C++ rejection of malformed text cannot arise.
pub(super) fn candidate_gloss_key(text: &str, source: u8) -> Option<(String, bool)> {
    if source == CandidateSource::Emoji as u8 || source == CandidateSource::Kaomoji as u8 {
        return None;
    }
    let latin = text
        .bytes()
        .all(|byte| byte.is_ascii_alphabetic() || matches!(byte, b' ' | b'-' | b'\''));
    if latin && text.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return Some((text.to_ascii_lowercase(), false));
    }
    if text.chars().any(is_gloss_han) {
        return Some((text.to_owned(), true));
    }
    None
}

/// The bridge's Han ranges (bridge.cpp:143-148), narrower than `text::is_han`: no U+3007 and nothing past U+2FA1F.
fn is_gloss_han(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

/// `candidate_gloss_display` (bridge.cpp:206-237): senses split on `;` or `；`, ASCII whitespace collapsed, empty senses dropped, at most two joined with `"; "`. A control character anywhere in the result withholds the whole gloss, because a learned gloss came from the network and must not reach the candidate window unfiltered.
pub(super) fn candidate_gloss_display(text: &str) -> String {
    // Joining two senses adds at most one byte beyond the source's separators.
    let mut output = String::with_capacity(text.len() + 1);
    let mut count = 0;
    let mut rest = text;
    while count < MAXIMUM_GLOSS_SENSES {
        let ascii = rest.find(';');
        let fullwidth = rest.find(FULLWIDTH_SENSE_DELIMITER);
        let (end, width) = match (ascii, fullwidth) {
            (_, Some(wide)) if ascii.is_none_or(|narrow| wide < narrow) => {
                (Some(wide), FULLWIDTH_SENSE_DELIMITER.len())
            }
            (narrow, _) => (narrow, 1),
        };
        let sense = collapse_ascii_whitespace(&rest[..end.unwrap_or(rest.len())]);
        if !sense.is_empty() {
            if !output.is_empty() {
                output.push_str("; ");
            }
            output.push_str(&sense);
            count += 1;
        }
        let Some(end) = end else {
            break;
        };
        rest = &rest[end + width..];
    }
    if output
        .chars()
        .any(|character| matches!(character as u32, 0..=0x1F | 0x7F..=0x9F))
    {
        return String::new();
    }
    output
}

/// bridge.cpp:189-205: runs of space, tab, CR and LF become one space; leading and trailing runs vanish.
fn collapse_ascii_whitespace(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if matches!(character, ' ' | '\t' | '\r' | '\n') {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);
    }
    output
}
