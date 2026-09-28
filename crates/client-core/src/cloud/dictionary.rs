//! Host-independent validation and data contracts for account-backed dictionaries.
//! Network and credentials remain injected by the platform host.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryKind {
    Pinyin,
    Wubi,
    Quick,
    English,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DictionaryValue {
    pub code: String,
    pub word: String,
    pub weight: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DictionaryEntry {
    pub id: String,
    pub kind: DictionaryKind,
    pub value: DictionaryValue,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DictionaryChange {
    pub revision: i64,
    pub previous: Option<DictionaryEntry>,
    pub replacement: Option<DictionaryEntry>,
}

pub fn validate_change_page(
    after: i64,
    changes: &[DictionaryChange],
    next: i64,
    has_more: bool,
) -> bool {
    if after < 0 || next < after {
        return false;
    }
    let mut cursor = after;
    for change in changes {
        if change.revision <= cursor {
            return false;
        }
        cursor = change.revision;
    }
    cursor == next && (!has_more || !changes.is_empty())
}

pub const MAX_IMPORT_BYTES: usize = 64 * 1024;

pub fn dictionary_path(kind: DictionaryKind, offset: usize, search: &str) -> Option<String> {
    if offset > 1_000_000 || !valid_bounded_text(search, 1024) {
        return None;
    }
    let kind = match kind {
        DictionaryKind::Pinyin => "pinyin",
        DictionaryKind::Wubi => "wubi",
        DictionaryKind::Quick => "quick",
        DictionaryKind::English => "english",
    };
    Some(format!(
        "/v1/users/me/dictionaries/{kind}?q={}&offset={offset}&limit=100",
        encode(search)
    ))
}

pub fn mutation_path(kind: DictionaryKind, operation: &str) -> Option<String> {
    let base = match kind {
        DictionaryKind::Pinyin => "pinyin",
        DictionaryKind::Wubi => "wubi",
        DictionaryKind::Quick => "quick",
        DictionaryKind::English => "english",
    };
    match operation {
        "add" | "import" | "import-hans" | "export" => {
            Some(format!("/v1/users/me/dictionaries/{base}/{operation}"))
        }
        _ => None,
    }
}

pub fn entry_path(entry: &DictionaryEntry) -> Option<String> {
    if entry.revision <= 0
        || entry.id.len() != 64
        || !entry
            .id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let kind = match entry.kind {
        DictionaryKind::Pinyin => "pinyin",
        DictionaryKind::Wubi => "wubi",
        DictionaryKind::Quick => "quick",
        DictionaryKind::English => "english",
    };
    Some(format!("/v1/users/me/dictionaries/{kind}/{}", entry.id))
}

pub fn changes_path(after: i64, limit: usize) -> Option<String> {
    if after < 0 || !(1..=100).contains(&limit) {
        return None;
    }
    Some(format!(
        "/v1/users/me/dictionary/changes?after={after}&limit={limit}"
    ))
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                format!("{}", b as char)
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
}

pub fn validate_value(value: &DictionaryValue) -> Result<(), &'static str> {
    if value.code.is_empty()
        || value.word.is_empty()
        || !valid_bounded_text(&value.code, 256)
        || !valid_bounded_text(&value.word, 1024)
    {
        return Err("invalid dictionary value");
    }
    if value.weight < 0 {
        return Err("invalid dictionary weight");
    }
    Ok(())
}

pub fn validate_import(text: &str) -> Result<(), &'static str> {
    if text.is_empty()
        || text.len() > MAX_IMPORT_BYTES
        || text.contains('\0')
        || crate::text::has_disallowed_control(text)
    {
        return Err("invalid dictionary import");
    }
    Ok(())
}

pub fn valid_candidate_query(
    text: &str,
    kind: &str,
    scheme: &str,
    profile: &str,
    limit: usize,
) -> bool {
    !text.is_empty()
        && valid_bounded_text(text, 256)
        && matches!(kind, "pinyin" | "jianpin" | "wubi" | "quick" | "english")
        && matches!(scheme, "pinyin" | "shuangpin")
        && matches!(profile, "xiaohe" | "ziranma" | "microsoft" | "shoudao")
        && (1..=100).contains(&limit)
}

pub fn valid_candidate_value(code: &str, word: &str) -> bool {
    !code.is_empty()
        && !word.is_empty()
        && valid_bounded_text(code, 256)
        && valid_bounded_text(word, 1024)
}

pub fn valid_bounded_text(value: &str, maximum_bytes: usize) -> bool {
    crate::text::is_bounded_text(value, maximum_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dictionary_values() {
        let value = DictionaryValue {
            code: "nihao".into(),
            word: "你好".into(),
            weight: 100,
        };
        assert!(validate_value(&value).is_ok());
        assert!(validate_value(&DictionaryValue {
            weight: -1,
            ..value.clone()
        })
        .is_err());
        assert!(validate_value(&DictionaryValue {
            code: "".into(),
            ..value
        })
        .is_err());
    }

    #[test]
    fn validates_bounded_import_text() {
        assert!(validate_import("你好\tnihao\n").is_ok());
        assert!(validate_import("bad\0").is_err());
        assert!(validate_import(&"x".repeat(MAX_IMPORT_BYTES + 1)).is_err());
    }

    #[test]
    fn builds_credential_free_dictionary_path() {
        assert_eq!(
            dictionary_path(DictionaryKind::Pinyin, 2, "ni hao").unwrap(),
            "/v1/users/me/dictionaries/pinyin?q=ni%20hao&offset=2&limit=100"
        );
        assert!(dictionary_path(DictionaryKind::Wubi, 0, "bad\n").is_none());
    }

    #[test]
    fn builds_mutation_paths_without_credentials() {
        assert_eq!(
            mutation_path(DictionaryKind::Quick, "import"),
            Some("/v1/users/me/dictionaries/quick/import".into())
        );
        assert!(mutation_path(DictionaryKind::Pinyin, "delete").is_none());
    }

    #[test]
    fn validates_versioned_entry_path() {
        let entry = DictionaryEntry {
            id: "a".repeat(64),
            kind: DictionaryKind::Pinyin,
            value: DictionaryValue {
                code: "ni".into(),
                word: "你".into(),
                weight: 1,
            },
            revision: 2,
        };
        assert_eq!(
            entry_path(&entry).unwrap(),
            format!("/v1/users/me/dictionaries/pinyin/{}", "a".repeat(64))
        );
        assert!(entry_path(&DictionaryEntry {
            revision: 0,
            ..entry
        })
        .is_none());
    }

    #[test]
    fn validates_change_feed_cursor() {
        assert_eq!(
            changes_path(4, 50),
            Some("/v1/users/me/dictionary/changes?after=4&limit=50".into())
        );
        assert!(changes_path(-1, 1).is_none());
        assert!(changes_path(0, 101).is_none());
    }

    #[test]
    fn validates_monotonic_change_pages() {
        let change = DictionaryChange {
            revision: 3,
            previous: None,
            replacement: None,
        };
        assert!(validate_change_page(
            1,
            std::slice::from_ref(&change),
            3,
            true
        ));
        assert!(!validate_change_page(3, &[change], 3, false));
        assert!(!validate_change_page(1, &[], 1, true));
    }
}
