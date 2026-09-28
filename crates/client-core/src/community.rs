//! The community library: resources published by other users.
//!
//! Community skins live under [`crate::skin`] instead, next to the other skin
//! sources, because callers reach for them by what they are rather than by
//! where they came from.

pub(crate) fn valid_text(value: &str, minimum: usize, maximum: usize, multiline: bool) -> bool {
    let count = value.chars().count();
    (minimum..=maximum).contains(&count)
        && value.chars().all(|character| {
            !character.is_control() || (multiline && matches!(character, '\n' | '\t'))
        })
}

pub(crate) const MAXIMUM_OFFSET: usize = 1_000_000;
pub(crate) const MAXIMUM_SEARCH_CHARACTERS: usize = 128;

pub(crate) fn valid_query(offset: usize, search: &str) -> bool {
    offset <= MAXIMUM_OFFSET
        && search.chars().count() <= MAXIMUM_SEARCH_CHARACTERS
        && !search.chars().any(char::is_control)
}

pub(crate) fn encode_query(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

pub mod resource;
pub mod resource_library;
