//! Shared text predicates used by client-core input boundaries.

/// Whether `value` contains a control character other than the line-oriented controls accepted
/// by multi-line text fields.
pub(crate) fn has_disallowed_control(value: &str) -> bool {
    has_disallowed_control_with_options(value, true)
}

/// Whether `value` contains a disallowed control character with an explicit line-whitespace policy.
pub fn has_disallowed_control_with_options(value: &str, allow_whitespace: bool) -> bool {
    value.chars().any(|character| {
        character.is_control() && !(allow_whitespace && matches!(character, '\n' | '\r' | '\t'))
    })
}

/// Whether a single-line field fits its byte bound and contains no controls.
pub fn is_bounded_text(value: &str, maximum_bytes: usize) -> bool {
    value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

pub fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether a single-line field fits its character bound and contains no controls.
pub(crate) fn is_bounded_chars(value: &str, maximum_characters: usize) -> bool {
    value.chars().count() <= maximum_characters && !value.chars().any(char::is_control)
}
