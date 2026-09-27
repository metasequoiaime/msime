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
        && crate::text::is_bounded_text(value, 8192)
        && !value.starts_with('<')
        && !value.chars().all(|character| character == '*')
}

pub(crate) fn valid_https_endpoint_and_model(endpoint: &str, model: &str) -> bool {
    if endpoint.len() > 2048 || model.is_empty() || !crate::text::is_bounded_text(model, 256) {
        return false;
    }
    reqwest::Url::parse(endpoint).ok().is_some_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}
