//! The community library: resources published by other users.
//!
//! Community skins live under [`crate::skin`] instead, next to the other skin
//! sources, because callers reach for them by what they are rather than by
//! where they came from.

pub(crate) fn valid_text(value: &str, minimum: usize, maximum: usize, multiline: bool) -> bool {
    let count = value.chars().count();
    let allowed_controls: &[char] = if multiline { &['\n', '\t'] } else { &[] };
    (minimum..=maximum).contains(&count)
        && !crate::has_disallowed_control_with_allowed(value, allowed_controls)
}

pub(crate) const MAXIMUM_OFFSET: usize = 1_000_000;
pub(crate) const MAXIMUM_SEARCH_CHARACTERS: usize = 128;
pub(crate) const MAXIMUM_PAGE_ITEMS: usize = 20;
pub(crate) const MAXIMUM_JAVASCRIPT_INTEGER: u64 = 9_007_199_254_740_991;

pub(crate) fn valid_query(offset: usize, search: &str) -> bool {
    offset <= MAXIMUM_OFFSET && crate::text::is_bounded_chars(search, MAXIMUM_SEARCH_CHARACTERS)
}

pub(crate) fn valid_name(value: &str) -> bool {
    valid_text(value, 1, 32, false) && value.trim() == value
}

pub(crate) fn valid_description(value: &str) -> bool {
    valid_text(value, 0, 280, true)
}

pub(crate) fn valid_author(value: &str) -> bool {
    valid_text(value, 1, 128, false) && value.trim() == value
}

pub(crate) fn valid_rating(rating_count: u64, rating_average: f64, my_rating: u8) -> bool {
    rating_count <= MAXIMUM_JAVASCRIPT_INTEGER
        && my_rating <= 5
        && rating_average.is_finite()
        && (0.0..=5.0).contains(&rating_average)
        && (rating_count != 0 || rating_average == 0.0)
}

pub mod resource;
pub mod resource_library;
