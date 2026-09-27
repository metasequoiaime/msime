//! Validation and probing for the third-party service credentials the user
//! supplies. Nothing here stores or logs a secret; the probes report only
//! whether a credential was accepted.

pub mod asr;
pub mod doubao;
pub mod doubao_auth;
pub mod probe;
pub mod translation;

pub(crate) fn usable_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 8192
        && !value.chars().any(char::is_control)
        && !value.starts_with('<')
        && !value.chars().all(|character| character == '*')
}
