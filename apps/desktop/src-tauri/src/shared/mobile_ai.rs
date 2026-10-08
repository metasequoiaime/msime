//! Bounded HTTP transport for the user-configured mobile AI settings.
//!
//! The webview owns the form, but never performs provider requests itself. Keeping
//! this transport in the Rust host gives iOS the same credential and response
//! boundaries as the Android native implementation without exposing secrets to
//! JavaScript logs or browser extensions.

use msime_client_core::{
    is_bounded_chars, is_bounded_chars_with_options, is_bounded_text_with_options,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::time::Duration;

const MAX_ENDPOINT_LENGTH: usize = 2_048;
const MAX_TOKEN_LENGTH: usize = 4_096;
const MAX_MODEL_LENGTH: usize = 512;
const MAX_MODEL_ID_LENGTH: usize = 256;
const MAX_PROMPT_LENGTH: usize = 16 * 1_024;
const MAX_TEXT_CODE_POINTS: usize = 10_000;
const MAX_RESPONSE_BYTES: usize = 1_024 * 1_024;
const MAX_MODELS: usize = 5_000;
const MAX_PAGES: usize = 10;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    Invalid,
    Unavailable,
}

#[derive(Debug, Deserialize)]
struct ModelPage {
    data: Vec<ModelEntry>,
    has_more: Option<bool>,
    last_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    supported_endpoint_types: Option<Vec<String>>,
    chat_completions_bridge: Option<bool>,
    active: Option<bool>,
}

/// 规则在 client-core 的 `ai::endpoint`：https 不限主机，http 只能指向本机或局域网。
fn valid_endpoint(endpoint: &str) -> Result<reqwest::Url, Error> {
    let value = endpoint.trim();
    if value.is_empty() || !is_bounded_text_with_options(value, MAX_ENDPOINT_LENGTH, false) {
        return Err(Error::Invalid);
    }
    msime_client_core::ai::endpoint::validate(value).map_err(|_| Error::Invalid)
}

fn valid_token(token: &str) -> Result<String, Error> {
    let value = token.trim();
    if !is_bounded_text_with_options(value, MAX_TOKEN_LENGTH, false) {
        return Err(Error::Invalid);
    }
    Ok(value.to_owned())
}

// Keep in step with the desktop `ai::ai_models_url`.
fn models_url(endpoint: &str) -> Result<reqwest::Url, Error> {
    Ok(crate::shared::ai_url::models_url(
        valid_endpoint(endpoint)?,
        false,
    ))
}

fn bounded_response(response: reqwest::blocking::Response) -> Result<Vec<u8>, Error> {
    crate::shared::bounded_body::read_bounded(response, MAX_RESPONSE_BYTES).map_err(|error| {
        match error {
            crate::shared::bounded_body::BoundedReadError::TooLarge => Error::Invalid,
            crate::shared::bounded_body::BoundedReadError::Read(_) => Error::Unavailable,
        }
    })
}

fn parse_models(page: ModelPage, models: &mut BTreeSet<String>) -> Result<(), Error> {
    for model in page.data {
        if model.active == Some(false) {
            continue;
        }
        let id = model.id.trim();
        if id.is_empty() || id.chars().count() > MAX_MODEL_ID_LENGTH {
            continue;
        }
        if let Some(endpoints) = model
            .supported_endpoint_types
            .as_deref()
            .filter(|endpoints| !endpoints.is_empty())
        {
            let supported = endpoints.iter().any(|endpoint| endpoint == "openai")
                || model.chat_completions_bridge == Some(true);
            if !supported {
                continue;
            }
        }
        models.insert(id.to_owned());
        if models.len() > MAX_MODELS {
            return Err(Error::Invalid);
        }
    }
    Ok(())
}

/// Fetch the OpenAI-compatible chat models supported by a configured service.
/// Anthropic's model directory is the one supported paginated exception.
pub fn fetch_models(endpoint: &str, token: &str) -> Result<Vec<String>, Error> {
    let token = valid_token(token)?;
    if token.is_empty() {
        return Err(Error::Invalid);
    }
    let base_url = models_url(endpoint)?;
    let anthropic = base_url
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case("api.anthropic.com"));
    // 不跟随重定向；https 地址只走 https，局域网的 http 接口直连、不走代理。
    let client = msime_client_core::ai::endpoint::blocking_client_builder(&base_url)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| Error::Unavailable)?;
    let mut models = BTreeSet::new();
    let mut cursor: Option<String> = None;
    let mut cursors = HashSet::with_capacity(MAX_PAGES);

    for _ in 0..MAX_PAGES {
        let mut url = base_url.clone();
        if anthropic {
            let query = url
                .query_pairs()
                .filter(|(name, _)| name != "limit" && name != "after_id")
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect::<Vec<_>>();
            url.set_query(None);
            {
                let mut pairs = url.query_pairs_mut();
                for (name, value) in query {
                    pairs.append_pair(&name, &value);
                }
                pairs.append_pair("limit", "1000");
                if let Some(cursor) = cursor.as_deref() {
                    pairs.append_pair("after_id", cursor);
                }
            }
        }
        let mut request = client.get(url).header("Accept", "application/json");
        if anthropic {
            request = request
                .header("x-api-key", &token)
                .header("anthropic-version", "2023-06-01");
        } else {
            request = request.bearer_auth(&token);
        }
        let response = request.send().map_err(|_| Error::Unavailable)?;
        if !response.status().is_success() {
            return Err(Error::Unavailable);
        }
        let body = bounded_response(response)?;
        let page: ModelPage = serde_json::from_slice(&body).map_err(|_| Error::Invalid)?;
        let has_more = page.has_more == Some(true);
        let next = page.last_id.clone();
        parse_models(page, &mut models)?;
        if !has_more {
            if models.is_empty() {
                return Err(Error::Invalid);
            }
            let mut result = Vec::with_capacity(models.len());
            result.extend(models);
            return Ok(result);
        }
        if !anthropic {
            return Err(Error::Invalid);
        }
        let next = next
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(Error::Invalid)?;
        if !cursors.insert(next.to_owned()) {
            return Err(Error::Invalid);
        }
        cursor = Some(next.to_owned());
    }
    Err(Error::Invalid)
}

fn valid_text(value: &str, maximum: usize, require_non_empty: bool) -> bool {
    (!require_non_empty || !value.trim().is_empty())
        && is_bounded_chars_with_options(value, maximum, true)
}

fn parse_completion(body: &[u8]) -> Result<String, Error> {
    let document: Value = serde_json::from_slice(body).map_err(|_| Error::Invalid)?;
    let content = document
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or(Error::Invalid)?;
    if !valid_text(content, MAX_TEXT_CODE_POINTS, true) {
        return Err(Error::Invalid);
    }
    Ok(content.to_owned())
}

/// Send one non-streaming OpenAI-compatible chat completion for the settings test.
pub fn polish(
    endpoint: &str,
    model: &str,
    prompt: &str,
    token: &str,
    text: &str,
) -> Result<String, Error> {
    let endpoint = valid_endpoint(endpoint)?;
    let model = model.trim();
    let prompt = prompt.trim();
    let token = valid_token(token)?;
    if model.is_empty()
        || !is_bounded_chars(model, MAX_MODEL_LENGTH)
        || prompt.is_empty()
        || !is_bounded_chars_with_options(prompt, MAX_PROMPT_LENGTH, true)
        || !valid_text(text, MAX_TEXT_CODE_POINTS, true)
    {
        return Err(Error::Invalid);
    }
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": text}
        ],
        "stream": false
    });
    let client = msime_client_core::ai::endpoint::blocking_client_builder(&endpoint)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| Error::Unavailable)?;
    let mut request = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body);
    if !token.is_empty() {
        request = request.bearer_auth(token);
    }
    let response = request.send().map_err(|_| Error::Unavailable)?;
    if !response.status().is_success() {
        return Err(Error::Unavailable);
    }
    parse_completion(&bounded_response(response)?)
}

#[cfg(test)]
mod tests {
    use super::{models_url, parse_completion, valid_endpoint, valid_text};

    #[test]
    fn model_directory_replaces_known_chat_suffix() {
        assert_eq!(
            models_url("https://fixture.invalid/v1/chat/completions")
                .unwrap()
                .as_str(),
            "https://fixture.invalid/v1/models"
        );
        assert_eq!(
            models_url("https://fixture.invalid/v1/").unwrap().as_str(),
            "https://fixture.invalid/v1/models"
        );
        assert_eq!(
            models_url("http://localhost:1234/v1/chat/completions")
                .unwrap()
                .as_str(),
            "http://localhost:1234/v1/models"
        );
    }

    #[test]
    fn endpoint_and_text_boundaries_match_the_mobile_contract() {
        assert!(valid_endpoint("https://fixture.invalid/api").is_ok());
        assert!(valid_endpoint("https:///api").is_err());
        assert!(valid_endpoint("http://fixture.invalid/api").is_err());
        // 局域网里的本地模型服务（如 LM Studio）可以用 http，公网 IP 不行。
        assert!(valid_endpoint(" http://192.168.1.20:1234/v1/chat/completions ").is_ok());
        assert!(valid_endpoint("http://8.8.8.8/v1/chat/completions").is_err());
        assert!(valid_endpoint("https://user:pass@fixture.invalid/api").is_err());
        assert!(valid_text("合成文本", 10_000, true));
        assert!(!valid_text("\u{0000}", 10_000, true));
        assert!(!valid_text("x".repeat(10_001).as_str(), 10_000, true));
    }

    #[test]
    fn completion_parser_exposes_only_message_content() {
        let body = serde_json::to_vec(&serde_json::json!({
            "choices": [{"message": {"content": "合成结果"}}]
        }))
        .unwrap();
        assert_eq!(parse_completion(&body).unwrap(), "合成结果");
        assert!(parse_completion(br#"{"error":"private detail"}"#).is_err());
    }
}
