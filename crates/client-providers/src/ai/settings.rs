//! 设置页 AI 辅助的两个联网操作：「获取模型」列出接口提供的模型，「测试」用润色提示词发一次对话。
//!
//! 自己发请求的桌面宿主共用这里：Tauri 设置在 Windows 和 macOS 上的 `ai_models`、`ai_test` 命令，以及原生 Windows 设置窗口经 `msime_client_ai_models`、`msime_client_ai_test`。Android 和 iOS 走各自平台的 HTTP 栈，Linux 经 provider 服务，都不经过这里。
//!
//! 失败只给稳定的错误码（[`AiSettingsError::code`]），不带服务的回复，也不带 Token。

use reqwest::Url;
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

use super::endpoint;

/// 响应体最多读这么多字节。
pub const MAX_RESPONSE_BYTES: usize = 1_024 * 1_024;
/// 模型列表最多保留这么多项。
pub const MAX_MODELS: usize = 128;
/// Token、提示词和待润色文字的字节上限。
const MAX_TEXT_BYTES: usize = 16 * 1024;
/// 模型名的字节上限。
const MAX_MODEL_BYTES: usize = 256;

/// 失败的原因；[`code`](Self::code) 是 Tauri 命令一直返回的错误码。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AiSettingsError {
    /// 接口地址、Token、模型、提示词或文字不合规，没有发请求。
    Invalid,
    /// 模型列表请求没有成功：连不上、超时或服务返回了错误状态。
    ModelsUnavailable,
    /// 模型列表的响应过大，或不是带模型 id 的 `{data:[{id}]}`。
    ModelsInvalid,
    /// 测试请求没有成功。
    TestUnavailable,
    /// 测试的响应过大，或里面没有回复文字。
    TestInvalid,
}

impl AiSettingsError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Invalid => "ai_invalid",
            Self::ModelsUnavailable => "ai_models_unavailable",
            Self::ModelsInvalid => "ai_models_invalid",
            Self::TestUnavailable => "ai_test_unavailable",
            Self::TestInvalid => "ai_test_invalid",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ResponseBodyError {
    TooLarge,
    Read,
}

/// 接口地址按 [`endpoint::validate`] 检查：https 不限主机，http 只能指向本机或局域网。
pub fn validate_endpoint(value: &str) -> Result<Url, AiSettingsError> {
    endpoint::validate(value).map_err(|_| AiSettingsError::Invalid)
}

/// Token 不能带控制字符或空白控制符，最多 16 KiB。空 Token 表示这个服务不鉴权（Ollama，见 [`super::provider_uses_token`]），请求不带 `Authorization`；其他服务要不要 Token 由设置页把关。
pub fn validate_token(token: &str) -> Result<(), AiSettingsError> {
    if !crate::text::is_bounded_text_with_options(token, MAX_TEXT_BYTES, false) {
        return Err(AiSettingsError::Invalid);
    }
    Ok(())
}

/// 有 Token 才带 Bearer。
fn authorize(
    request: reqwest::blocking::RequestBuilder,
    token: &str,
) -> reqwest::blocking::RequestBuilder {
    if token.is_empty() {
        request
    } else {
        request.bearer_auth(token)
    }
}

/// 提示词和待润色文字可以换行和带制表符，最多 16 KiB；`allow_empty` 为假时不能为空。
pub fn text_is_valid(value: &str, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && crate::text::is_bounded_text_with_options(value, MAX_TEXT_BYTES, true)
}

/// 最多读到上限多一个字节，没有可靠 Content-Length 的响应也不会把设置进程撑大。
pub(crate) fn read_response_body(reader: impl Read) -> Result<Vec<u8>, ResponseBodyError> {
    let mut bytes = Vec::with_capacity(8 * 1024);
    reader
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ResponseBodyError::Read)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(ResponseBodyError::TooLarge);
    }
    Ok(bytes)
}

/// 不跟随重定向（Token 不会被转发到别的主机）；局域网的 http 接口直连、不走代理。
fn client(url: &Url) -> Option<reqwest::blocking::Client> {
    endpoint::blocking_client_builder(url)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .ok()
}

/// 「获取模型」：GET 接口旁边的 `/models`（[`endpoint::models_url`]，去掉查询参数），有 Token 时带 Bearer Token。返回服务列出的模型 id，按服务给的顺序，跳过空的和不合规的，最多 [`MAX_MODELS`] 个；一个也没有时算 [`AiSettingsError::ModelsInvalid`]。会联网，最长约 15 秒，不要在界面线程上调用。
pub fn fetch_models(endpoint: &str, token: &str) -> Result<Vec<String>, AiSettingsError> {
    let endpoint = validate_endpoint(endpoint)?;
    validate_token(token)?;
    let client = client(&endpoint).ok_or(AiSettingsError::ModelsUnavailable)?;
    let response = authorize(client.get(endpoint::models_url(endpoint, true)), token)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|_| AiSettingsError::ModelsUnavailable)?;
    let body = read_response_body(response).map_err(|error| match error {
        ResponseBodyError::TooLarge => AiSettingsError::ModelsInvalid,
        ResponseBodyError::Read => AiSettingsError::ModelsUnavailable,
    })?;
    parse_models(&body)
}

/// 模型列表响应 `{data:[{id}]}` 里的模型 id，规则见 [`fetch_models`]。
pub fn parse_models(body: &[u8]) -> Result<Vec<String>, AiSettingsError> {
    let document: Value =
        serde_json::from_slice(body).map_err(|_| AiSettingsError::ModelsInvalid)?;
    let data = document
        .get("data")
        .and_then(Value::as_array)
        .ok_or(AiSettingsError::ModelsInvalid)?;
    let models: Vec<String> = data
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .filter(|id| !id.is_empty() && crate::text::is_bounded_text(id, MAX_MODEL_BYTES))
        .take(MAX_MODELS)
        .map(str::to_owned)
        .collect();
    if models.is_empty() {
        return Err(AiSettingsError::ModelsInvalid);
    }
    Ok(models)
}

/// 测试请求的 JSON 体：不流式、温度 0.2，提示词作 system、待润色文字作 user。
pub fn test_body(model: &str, prompt: &str, text: &str) -> Value {
    serde_json::json!({
        "model": model,
        "stream": false,
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": text}
        ]
    })
}

/// 「测试」：用 `prompt` 让模型润色 `text`，POST 到接口地址本身，返回回复文字（去掉首尾空白）。模型不能为空、最多 256 字节；提示词可以为空，文字不能为空。会联网，最长约 15 秒，不要在界面线程上调用。
pub fn test_polish(
    endpoint: &str,
    model: &str,
    prompt: &str,
    token: &str,
    text: &str,
) -> Result<String, AiSettingsError> {
    let endpoint = validate_endpoint(endpoint)?;
    validate_token(token)?;
    if model.is_empty()
        || !crate::text::is_bounded_text(model, MAX_MODEL_BYTES)
        || !text_is_valid(prompt, true)
        || !text_is_valid(text, false)
    {
        return Err(AiSettingsError::Invalid);
    }
    let client = client(&endpoint).ok_or(AiSettingsError::TestUnavailable)?;
    let response = authorize(client.post(endpoint), token)
        .json(&test_body(model, prompt, text))
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|_| AiSettingsError::TestUnavailable)?;
    let body = read_response_body(response).map_err(|error| match error {
        ResponseBodyError::TooLarge => AiSettingsError::TestInvalid,
        ResponseBodyError::Read => AiSettingsError::TestUnavailable,
    })?;
    parse_test_reply(&body)
}

/// 对话响应里 `choices[0].message.content` 的文字，去掉首尾空白后不能为空、最多 16 KiB。
pub fn parse_test_reply(body: &[u8]) -> Result<String, AiSettingsError> {
    let document: Value = serde_json::from_slice(body).map_err(|_| AiSettingsError::TestInvalid)?;
    document
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= MAX_TEXT_BYTES)
        .map(str::to_owned)
        .ok_or(AiSettingsError::TestInvalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_validation_accepts_http_api_urls_and_rejects_unsafe_urls() {
        for endpoint in [
            "https://api.example.test/v1/chat/completions",
            "http://127.0.0.1:8080/v1/chat/completions?tenant=fixture",
            "http://192.168.1.20:1234/v1/chat/completions",
            "http://studio.local:1234/v1",
        ] {
            assert!(validate_endpoint(endpoint).is_ok(), "{endpoint}");
        }
        for endpoint in [
            "file:///tmp/models",
            "http://api.example.test/v1/chat/completions",
            "http://8.8.8.8/v1/chat/completions",
            "https:///v1/chat/completions",
            "https://user:password@example.test/v1/chat/completions",
            "https://example.test/v1/chat/completions#fragment",
            "https://example.test/v1/chat/\ncompletions",
        ] {
            assert_eq!(
                validate_endpoint(endpoint).unwrap_err(),
                AiSettingsError::Invalid,
                "{endpoint}"
            );
        }
    }

    #[test]
    fn credentials_and_text_reject_empty_or_unsafe_values() {
        assert!(validate_token("fixture-token").is_ok());
        // 空 Token 是不鉴权的服务（Ollama），请求不带 Authorization。
        assert!(validate_token("").is_ok());
        assert!(validate_token("fixture\n-token").is_err());
        assert!(validate_token(&"x".repeat(MAX_TEXT_BYTES + 1)).is_err());
        assert!(text_is_valid("多行\nfixture text\t", false));
        assert!(text_is_valid("", true));
        assert!(!text_is_valid("", false));
        assert!(!text_is_valid("fixture\0text", false));
    }

    #[test]
    fn response_body_limit_rejects_oversized_streams() {
        let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        assert_eq!(
            read_response_body(std::io::Cursor::new(body)).unwrap_err(),
            ResponseBodyError::TooLarge
        );
        let body = vec![b'x'; MAX_RESPONSE_BYTES];
        assert_eq!(
            read_response_body(std::io::Cursor::new(body))
                .unwrap()
                .len(),
            MAX_RESPONSE_BYTES
        );
    }

    // Everything below is rejected before a client is built, so nothing reaches the network.
    #[test]
    fn invalid_requests_fail_before_the_network() {
        let endpoint = "https://fixture.invalid/v1/chat/completions";
        assert_eq!(
            fetch_models("http://api.example.test/v1", "token").unwrap_err(),
            AiSettingsError::Invalid
        );
        assert_eq!(
            fetch_models(endpoint, "fixture\n-token").unwrap_err(),
            AiSettingsError::Invalid
        );
        for (model, prompt, token, text) in [
            ("m", "p", "fixture\n-token", "t"),
            ("", "p", "token", "t"),
            ("m\n", "p", "token", "t"),
            ("m", "p", "token", ""),
            ("m", "p\0", "token", "t"),
        ] {
            assert_eq!(
                test_polish(endpoint, model, prompt, token, text).unwrap_err(),
                AiSettingsError::Invalid
            );
        }
        assert_eq!(
            test_polish("ftp://fixture.invalid", "m", "", "token", "t").unwrap_err(),
            AiSettingsError::Invalid
        );
    }

    #[test]
    fn model_list_keeps_valid_ids_in_order_up_to_the_limit() {
        let body =
            br#"{"data":[{"id":"b"},{"id":""},{"name":"x"},{"id":"a\u0000"},{"id":"a"},{"id":7}]}"#;
        assert_eq!(parse_models(body).unwrap(), ["b", "a"]);
        let many: Vec<Value> = (0..200)
            .map(|index| serde_json::json!({"id": format!("model-{index}")}))
            .collect();
        let body = serde_json::to_vec(&serde_json::json!({ "data": many })).unwrap();
        let models = parse_models(&body).unwrap();
        assert_eq!(models.len(), MAX_MODELS);
        assert_eq!(models[0], "model-0");
        for body in [
            &br#"{"data":[]}"#[..],
            br#"{"data":[{"id":""}]}"#,
            br#"{"models":[{"id":"a"}]}"#,
            b"not json",
        ] {
            assert_eq!(
                parse_models(body).unwrap_err(),
                AiSettingsError::ModelsInvalid
            );
        }
    }

    #[test]
    fn test_reply_exposes_only_the_trimmed_message_content() {
        let body =
            r#"{"choices":[{"message":{"content":"  润色后的文字\n"}}],"usage":{}}"#.as_bytes();
        assert_eq!(parse_test_reply(body).unwrap(), "润色后的文字");
        let long = serde_json::to_vec(&serde_json::json!({
            "choices": [{"message": {"content": "x".repeat(MAX_TEXT_BYTES + 1)}}]
        }))
        .unwrap();
        for body in [
            &br#"{"choices":[{"message":{"content":"   "}}]}"#[..],
            br#"{"choices":[]}"#,
            br#"{"error":{"message":"denied"}}"#,
            &long,
        ] {
            assert_eq!(
                parse_test_reply(body).unwrap_err(),
                AiSettingsError::TestInvalid
            );
        }
    }

    #[test]
    fn test_body_sends_the_prompt_as_system_and_the_text_as_user() {
        let body = test_body("model-a", "提示词", "文字");
        assert_eq!(body["model"], "model-a");
        assert_eq!(body["stream"], false);
        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "提示词");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "文字");
    }

    #[test]
    fn error_codes_are_the_ones_the_tauri_commands_return() {
        assert_eq!(AiSettingsError::Invalid.code(), "ai_invalid");
        assert_eq!(
            AiSettingsError::ModelsUnavailable.code(),
            "ai_models_unavailable"
        );
        assert_eq!(AiSettingsError::ModelsInvalid.code(), "ai_models_invalid");
        assert_eq!(
            AiSettingsError::TestUnavailable.code(),
            "ai_test_unavailable"
        );
        assert_eq!(AiSettingsError::TestInvalid.code(), "ai_test_invalid");
    }
}
