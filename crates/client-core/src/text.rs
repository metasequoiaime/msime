//! Shared text predicates used by client-core input boundaries.

/// Whether `value` contains a control character other than the line-oriented controls accepted
/// by multi-line text fields.
pub(crate) fn has_disallowed_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}
