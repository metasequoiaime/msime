//! Shared text predicates used by client-core input boundaries.

/// Whether `value` contains a control character other than the line-oriented controls accepted
/// by multi-line text fields.
pub(crate) fn has_disallowed_control(value: &str) -> bool {
    has_disallowed_control_with_options(value, true)
}

/// Whether `value` contains a disallowed control character with an explicit line-whitespace policy.
pub fn has_disallowed_control_with_options(value: &str, allow_whitespace: bool) -> bool {
    let allowed = if allow_whitespace {
        &['\n', '\r', '\t'][..]
    } else {
        &[]
    };
    has_disallowed_control_with_allowed(value, allowed)
}

/// Whether `value` contains a control character outside the supplied allowed set.
pub fn has_disallowed_control_with_allowed(value: &str, allowed: &[char]) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && !allowed.contains(&character))
}

/// Whether `value` contains a control character other than line-feed or carriage-return.
pub fn has_disallowed_control_with_line_breaks(value: &str) -> bool {
    has_disallowed_control_with_allowed(value, &['\n', '\r'])
}

/// Whether a single-line field fits its byte bound and contains no controls.
pub fn is_bounded_text(value: &str, maximum_bytes: usize) -> bool {
    is_bounded_text_with_options(value, maximum_bytes, false)
}

/// Whether `value` fits its byte bound and contains no control characters outside the selected whitespace policy.
pub fn is_bounded_text_with_options(
    value: &str,
    maximum_bytes: usize,
    allow_whitespace: bool,
) -> bool {
    value.len() <= maximum_bytes && !has_disallowed_control_with_options(value, allow_whitespace)
}

/// Whether `value` fits both byte and Unicode scalar bounds and contains no controls.
pub fn is_bounded_text_with_chars(
    value: &str,
    maximum_bytes: usize,
    maximum_characters: usize,
) -> bool {
    is_bounded_text(value, maximum_bytes) && value.chars().count() <= maximum_characters
}

/// Whether `value` fits a UTF-16 code-unit bound.
pub fn is_bounded_utf16(value: &str, maximum_units: usize) -> bool {
    value.encode_utf16().count() <= maximum_units
}

/// Validate an ASCII hexadecimal value with an exact byte length.
pub fn is_ascii_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Validate a CSS-style hexadecimal color with one of the accepted digit lengths.
pub fn is_hex_color(value: &str, digits: &[usize]) -> bool {
    value.strip_prefix('#').is_some_and(|hex| {
        digits.contains(&hex.len()) && hex.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

/// Validate a non-empty ASCII digit string.
pub fn is_ascii_digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

/// Whether a value contains only ASCII letters.
pub fn is_ascii_alphabetic(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// Whether a value contains only visible ASCII characters (U+0021 through U+007E).
pub fn is_ascii_graphic(value: &str) -> bool {
    value.bytes().all(|byte| (33..=126).contains(&byte))
}

/// Whether a value contains only lowercase ASCII letters.
pub fn is_ascii_lowercase(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// Whether a value contains only ASCII letters, digits, and dashes.
pub fn is_ascii_alphanumeric_dash(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// Whether a value contains only ASCII letters, digits, dashes, and underscores.
pub fn is_ascii_identifier(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Whether a value contains only ASCII letters, digits, dots, dashes, and underscores.
pub fn is_ascii_identifier_with_dots(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// Whether a value contains only lowercase ASCII letters, digits, dots, dashes, and underscores.
pub fn is_ascii_lowercase_identifier_with_dots(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
    })
}

pub fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether a single-line field fits its character bound and contains no controls.
pub fn is_bounded_chars(value: &str, maximum_characters: usize) -> bool {
    is_bounded_chars_with_options(value, maximum_characters, false)
}

/// Whether `value` fits its character bound and contains no control characters outside the selected whitespace policy.
pub fn is_bounded_chars_with_options(
    value: &str,
    maximum_characters: usize,
    allow_whitespace: bool,
) -> bool {
    value.chars().count() <= maximum_characters
        && !has_disallowed_control_with_options(value, allow_whitespace)
}

/// Whether `value` fits a character bound and contains no NUL character.
pub fn is_bounded_chars_without_nul(value: &str, maximum_characters: usize) -> bool {
    value.chars().count() <= maximum_characters && !value.contains('\0')
}

/// Whether an ASCII byte is an RFC 3986 URI unreserved character.
pub(crate) fn is_ascii_uri_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}
