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

pub(crate) fn valid_https_endpoint_and_model(endpoint: &str, model: &str) -> bool {
    if endpoint.len() > 2048
        || model.is_empty()
        || model.len() > 256
        || model.chars().any(char::is_control)
    {
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
