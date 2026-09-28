//! Shared text predicates used by client-core input boundaries.

/// Whether `value` contains a control character other than the line-oriented controls accepted
/// by multi-line text fields.
pub(crate) fn has_disallowed_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

/// Whether a single-line field fits its byte bound and contains no controls.
pub(crate) fn is_bounded_text(value: &str, maximum_bytes: usize) -> bool {
    value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

/// Whether a single-line field fits its character bound and contains no controls.
pub(crate) fn is_bounded_chars(value: &str, maximum_characters: usize) -> bool {
    value.chars().count() <= maximum_characters && !value.chars().any(char::is_control)
}
