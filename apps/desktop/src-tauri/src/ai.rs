//! The bring-your-own-endpoint AI assistant: URL and token validation, the
//! model listing, and the one-shot polish request.
//!
//! Only the two desktop hosts that expose it build this. Android and iOS answer
//! the same two commands from their own platform modules, because those requests
//! go out through the platform's HTTP stack rather than reqwest.

use crate::CommandError;
use reqwest::Url;
use serde_json::Value;
use std::io::Read;

pub(crate) const MAX_RESPONSE_BYTES: usize = 1_024 * 1_024;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AiResponseBodyError {
    TooLarge,
    Read,
}

/// Read at most one byte past the response limit so streams without a reliable
/// Content-Length cannot grow the settings process without bound.
pub(crate) fn read_ai_response_body(reader: impl Read) -> Result<Vec<u8>, AiResponseBodyError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| AiResponseBodyError::Read)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(AiResponseBodyError::TooLarge);
    }
    Ok(bytes)
}

pub(crate) fn validate_ai_endpoint(value: &str) -> Result<Url, CommandError> {
    if value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(CommandError { code: "ai_invalid" });
    }
    let url = Url::parse(value).map_err(|_| CommandError { code: "ai_invalid" })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(CommandError { code: "ai_invalid" });
    }
    Ok(url)
}

pub(crate) fn validate_ai_token(token: &str) -> Result<(), CommandError> {
    if token.is_empty() || token.len() > 16 * 1024 || token.chars().any(char::is_control) {
        return Err(CommandError { code: "ai_invalid" });
    }
    Ok(())
}

pub(crate) fn ai_text_is_valid(value: &str, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && value.len() <= 16 * 1024
        && !value.chars().any(|character| {
            character == '\0'
                || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        })
}

/// The listing sits next to the chat endpoint, whatever its version prefix
/// (`/v1`, `/v1beta/openai`, `/api/paas/v4`). Keep in step with
/// `mobile_ai::models_url`.
pub(crate) fn ai_models_url(endpoint: &Url) -> Url {
    crate::shared::ai_url::models_url(endpoint.clone(), true)
}

pub(crate) fn ai_models_request(endpoint: &str, token: &str) -> Result<Vec<String>, CommandError> {
    let endpoint = validate_ai_endpoint(endpoint)?;
    validate_ai_token(token)?;
    let client = reqwest::blocking::Client::builder()
        // The configured endpoint receives the user's bearer token. A redirect could replay it
        // to a different origin, so this test request must stop at the first response.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| CommandError {
            code: "ai_models_unavailable",
        })?;
    let response = client
        .get(ai_models_url(&endpoint))
        .bearer_auth(token)
        .send()
        .map_err(|_| CommandError {
            code: "ai_models_unavailable",
        })?
        .error_for_status()
        .map_err(|_| CommandError {
            code: "ai_models_unavailable",
        })?;
    let body = read_ai_response_body(response).map_err(|error| match error {
        AiResponseBodyError::TooLarge => CommandError {
            code: "ai_models_invalid",
        },
        AiResponseBodyError::Read => CommandError {
            code: "ai_models_unavailable",
        },
    })?;
    let document: Value = serde_json::from_slice(&body).map_err(|_| CommandError {
        code: "ai_models_invalid",
    })?;
    let models = document
        .get("data")
        .and_then(Value::as_array)
        .ok_or(CommandError {
            code: "ai_models_invalid",
        })?
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .filter(|id| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control))
        .take(128)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if models.is_empty() {
        return Err(CommandError {
            code: "ai_models_invalid",
        });
    }
    Ok(models)
}

pub(crate) fn ai_test_request(
    endpoint: &str,
    model: &str,
    prompt: &str,
    token: &str,
    text: &str,
) -> Result<String, CommandError> {
    let endpoint = validate_ai_endpoint(endpoint)?;
    validate_ai_token(token)?;
    if model.is_empty()
        || model.len() > 256
        || model.chars().any(char::is_control)
        || !ai_text_is_valid(prompt, true)
        || !ai_text_is_valid(text, false)
    {
        return Err(CommandError { code: "ai_invalid" });
    }
    let body = serde_json::json!({
        "model": model,
        "stream": false,
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": text}
        ]
    });
    let client = reqwest::blocking::Client::builder()
        // The configured endpoint receives the user's bearer token. A redirect could replay it
        // to a different origin, so this test request must stop at the first response.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| CommandError {
            code: "ai_test_unavailable",
        })?;
    let response = client
        .post(endpoint)
        .bearer_auth(token)
        .json(&body)
        .send()
        .map_err(|_| CommandError {
            code: "ai_test_unavailable",
        })?
        .error_for_status()
        .map_err(|_| CommandError {
            code: "ai_test_unavailable",
        })?;
    let body = read_ai_response_body(response).map_err(|error| match error {
        AiResponseBodyError::TooLarge => CommandError {
            code: "ai_test_invalid",
        },
        AiResponseBodyError::Read => CommandError {
            code: "ai_test_unavailable",
        },
    })?;
    let document: Value = serde_json::from_slice(&body).map_err(|_| CommandError {
        code: "ai_test_invalid",
    })?;
    let output = document
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 16 * 1024)
        .ok_or(CommandError {
            code: "ai_test_invalid",
        })?;
    Ok(output.to_owned())
}

// Linux routes these two through the provider socket instead - see the
// same-named commands in lib.rs - and `#[tauri::command]` declares a crate-level
// `macro_rules! __cmd__<name>`, so two commands sharing a name collide however
// separate their modules are. Gate the definitions the way their registration is
// already gated rather than leaving both to exist on Linux.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub(crate) async fn ai_models(
    endpoint: String,
    token: String,
) -> Result<Vec<String>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || ai_models_request(&endpoint, &token))
        .await
        .map_err(|_| CommandError {
            code: "ai_models_unavailable",
        })?
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub(crate) async fn ai_test(
    endpoint: String,
    model: String,
    prompt: String,
    token: String,
    text: String,
) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        ai_test_request(&endpoint, &model, &prompt, &token, &text)
    })
    .await
    .map_err(|_| CommandError {
        code: "ai_test_unavailable",
    })?
}
