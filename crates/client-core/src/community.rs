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
    offset <= MAXIMUM_OFFSET && crate::text::is_bounded_chars(search, MAXIMUM_SEARCH_CHARACTERS)
}

pub mod resource;
pub mod resource_library;
