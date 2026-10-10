//! Host independent contract for asynchronous AI candidate suggestions.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::preferences::AI_PROVIDERS;

pub mod endpoint;

#[derive(Debug, Clone, Error, Eq, PartialEq)]
pub enum AiError {
    #[error("segmented pinyin is empty or too large")]
    InvalidSegments,
    #[error("context is too large")]
    ContextTooLarge,
    #[error("candidate limit is invalid")]
    InvalidLimit,
    #[error("candidate text is empty or too large")]
    InvalidCandidate,
    #[error("AI provider request configuration is invalid")]
    InvalidConfiguration,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiSuggestionRequest {
    pub segmented_pinyin: Vec<String>,
    pub context: String,
    pub candidate_limit: u8,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub text: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiSuggestionResponse {
    pub candidates: Vec<AiSuggestion>,
}

/// The system prompt a blank prompt slot stands for. It used to exist only in the Windows installer's default configuration, so every other host that never wrote a prompt sent an empty system message and got replies the JSON parser rejects.
pub const DEFAULT_CANDIDATE_PROMPT: &str = "你是一个中文全拼输入法联想引擎。输入为已经切分好的拼音数组、前文上下文和候选数量。\n\n优先生成与拼音严格对应的中文候选：若有 N 段拼音，首选必须尽量为 N 个汉字，每段拼音对应一个汉字，不得随意增删或改变读音。结合上下文、常用程度、语义完整性和固定搭配排序。\n\n若去掉分词后能明显组成更合理的英文单词、缩写、产品名或技术术语，如 `deep + seek → DeepSeek`、`git + hub → GitHub`，可优先返回英文；不要生造英文或做牵强匹配。\n\n只输出合法 JSON，不要解释或输出 Markdown：\n\n{\n\"candidates\": [\n{\n\"text\": \"候选内容\",\n\"type\": \"chinese或english\",\n\"confidence\": 0.98\n}\n]\n}\n\n候选按推荐程度降序排列，数量不超过指定上限；没有合理结果时返回空数组。";

pub trait AiSuggestor {
    fn suggest(&self, request: &AiSuggestionRequest) -> Result<AiSuggestionResponse, AiError>;
}

/// Pure Windows-compatible non-streaming request body. The host owns endpoint,
/// credentials, timeout and cancellation. Never log prompts or request contents.
pub fn chat_completion_body(
    request: &AiSuggestionRequest,
    provider: &str,
    model: &str,
    prompt: &str,
) -> Result<serde_json::Value, AiError> {
    request.validate()?;
    if !AI_PROVIDERS.contains(&provider)
        || model.is_empty()
        || !crate::text::is_bounded_text(model, 256)
        || !crate::text::is_bounded_text_with_options(prompt, 16384, true)
    {
        return Err(AiError::InvalidConfiguration);
    }
    let mut body = serde_json::json!({
        "model":model, "stream":false, "temperature":0.2, "max_tokens":512,
        "response_format":{"type":"json_object"},
        "messages":[{"role":"system","content":prompt},
            {"role":"user","content":serde_json::to_string(request).map_err(|_| AiError::InvalidConfiguration)?}]
    });
    if provider == "deepseek" {
        body["thinking"] = serde_json::json!({"type":"disabled"});
    }
    Ok(body)
}

/// Resolve local provider credentials into a native HTTP descriptor. Contains a
/// bearer token and private input: never log or persist this descriptor.
///
/// 接口地址按 [`endpoint::validate`] 检查：https 不限主机，http 只能指向本机或局域网。
pub fn chat_completion_http_request(
    config: &crate::preferences::AiAssistantPreferences,
    request: &AiSuggestionRequest,
) -> Result<Option<serde_json::Value>, AiError> {
    if !config.enabled {
        return Ok(None);
    }
    let endpoint = &config.endpoint;
    if endpoint::validate(endpoint).is_err() || request.candidate_limit != config.candidate_limit {
        return Err(AiError::InvalidConfiguration);
    }
    let token = credential_for_endpoint(config).ok_or(AiError::InvalidConfiguration)?;
    let prompt = match config.prompt_id.as_str() {
        "custom_2" => &config.prompt_custom_2,
        "custom_3" => &config.prompt_custom_3,
        _ => &config.prompt_custom_1,
    };
    let prompt = if prompt.trim().is_empty() {
        DEFAULT_CANDIDATE_PROMPT
    } else {
        prompt
    };
    let body = chat_completion_body(request, &config.provider, &config.model, prompt)?;
    if serde_json::to_vec(&body)
        .map_err(|_| AiError::InvalidConfiguration)?
        .len()
        > 65536
    {
        return Err(AiError::InvalidConfiguration);
    }
    Ok(Some(serde_json::json!({"url":endpoint,"method":"POST",
        "headers":{"Content-Type":"application/json","Authorization":format!("Bearer {token}")},
        "body":body,"timeout_ms":8000,"connect_timeout_ms":2500,"max_response_bytes":1048576})))
}

/// Resolve a secret only for the configured endpoint's origin. Never log the result.
/// Origin-scoped entries also record an explicit clear, so even an empty entry
/// must not fall back to a stale provider or flat token.
pub fn credential_for_endpoint(
    config: &crate::preferences::AiAssistantPreferences,
) -> Option<&str> {
    let origin = endpoint::credential_origin(config.endpoint.trim())?;
    if let Some(token) = config.tokens.get(&origin) {
        return usable_ai_token(token);
    }
    if legacy_provider_origin(&config.provider) != Some(origin.as_str()) {
        return None;
    }
    if let Some(token) = config.tokens.get(&config.provider) {
        return usable_ai_token(token);
    }
    (config.provider == "deepseek")
        .then_some(config.token.as_str())
        .and_then(usable_ai_token)
}

fn usable_ai_token(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()
        && crate::text::is_bounded_text(value, 4096)
        && !value.starts_with("FAKESECRET_")
        && !(value.starts_with('<') && value.ends_with('>')))
    .then_some(value)
}

/// Provider slots predate origin-scoped storage. They are safe to read only
/// while the endpoint still belongs to that provider's built-in service.
fn legacy_provider_origin(provider: &str) -> Option<&'static str> {
    Some(match provider {
        "everyapi" => "https://api.everyapi.ai:443",
        "openai" => "https://api.openai.com:443",
        "anthropic" => "https://api.anthropic.com:443",
        "gemini" => "https://generativelanguage.googleapis.com:443",
        "deepseek" => "https://api.deepseek.com:443",
        "qwen" => "https://dashscope.aliyuncs.com:443",
        "kimi" => "https://api.moonshot.cn:443",
        "zhipu" => "https://open.bigmodel.cn:443",
        "siliconflow" => "https://api.siliconflow.cn:443",
        "groq" => "https://api.groq.com:443",
        "openrouter" => "https://openrouter.ai:443",
        _ => return None,
    })
}

/// Parse a bounded successful HTTP body containing JSON-mode chat content.
/// Preserve provider order, omit invalid/duplicate entries, and honor the caller's
/// configured limit. None denotes an invalid envelope; an empty list is no result.
pub fn parse_chat_completion_response(body: &[u8], limit: u8) -> Option<AiSuggestionResponse> {
    if body.len() > 1024 * 1024 || !(1..=10).contains(&limit) {
        return None;
    }
    let outer: serde_json::Value = serde_json::from_slice(body).ok()?;
    if outer.get("error").is_some_and(|error| !error.is_null()) {
        return None;
    }
    let content = outer
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()?;
    let inner: serde_json::Value = serde_json::from_str(content).ok()?;
    let entries = inner.get("candidates")?.as_array()?;
    let mut candidates: Vec<AiSuggestion> = Vec::new();
    for entry in entries {
        let Some(text) = entry.get("text").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if text.trim().is_empty()
            || !crate::text::is_bounded_text(text, 4096)
            || candidates.iter().any(|candidate| candidate.text == text)
        {
            continue;
        }
        if candidates.is_empty() {
            candidates.reserve_exact(usize::from(limit));
        }
        candidates.push(AiSuggestion { text: text.into() });
        if candidates.len() == usize::from(limit) {
            break;
        }
    }
    Some(AiSuggestionResponse { candidates })
}

pub fn suggest<S: AiSuggestor>(
    suggestor: &S,
    request: &AiSuggestionRequest,
) -> Result<AiSuggestionResponse, AiError> {
    request.validate()?;
    let response = suggestor.suggest(request)?;
    response.validate(request.candidate_limit)?;
    Ok(response)
}

impl AiSuggestionRequest {
    pub fn validate(&self) -> Result<(), AiError> {
        if self.segmented_pinyin.is_empty()
            || self.segmented_pinyin.len() > 128
            || self
                .segmented_pinyin
                .iter()
                .any(|part| part.is_empty() || !crate::text::is_bounded_text(part, 32))
        {
            return Err(AiError::InvalidSegments);
        }
        if !crate::text::is_bounded_text_with_options(&self.context, 16 * 1024, true) {
            return Err(AiError::ContextTooLarge);
        }
        if !(1..=10).contains(&self.candidate_limit) {
            return Err(AiError::InvalidLimit);
        }
        Ok(())
    }
}

impl AiSuggestionResponse {
    pub fn validate(&self, limit: u8) -> Result<(), AiError> {
        if self.candidates.len() > limit as usize {
            return Err(AiError::InvalidLimit);
        }
        if self.candidates.iter().any(|candidate| {
            candidate.text.is_empty() || !crate::text::is_bounded_text(&candidate.text, 4096)
        }) {
            return Err(AiError::InvalidCandidate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Stub;
    impl AiSuggestor for Stub {
        fn suggest(&self, _: &AiSuggestionRequest) -> Result<AiSuggestionResponse, AiError> {
            Ok(AiSuggestionResponse {
                candidates: vec![AiSuggestion {
                    text: "你好".into(),
                }],
            })
        }
    }
    #[test]
    fn validates_and_dispatches_suggestions() {
        let request = AiSuggestionRequest {
            segmented_pinyin: vec!["ni".into(), "hao".into()],
            context: String::new(),
            candidate_limit: 3,
        };
        assert_eq!(suggest(&Stub, &request).unwrap().candidates[0].text, "你好");
    }
    #[test]
    fn rejects_invalid_bounds() {
        let mut request = AiSuggestionRequest {
            segmented_pinyin: vec!["ni".into()],
            context: String::new(),
            candidate_limit: 0,
        };
        assert_eq!(request.validate(), Err(AiError::InvalidLimit));
        request.candidate_limit = 1;
        request.segmented_pinyin[0] = String::new();
        assert_eq!(request.validate(), Err(AiError::InvalidSegments));
    }

    #[test]
    fn rejects_control_characters_in_model_inputs() {
        let mut request = AiSuggestionRequest {
            segmented_pinyin: vec!["ni\u{0}".into()],
            context: String::new(),
            candidate_limit: 1,
        };
        assert_eq!(request.validate(), Err(AiError::InvalidSegments));
        request.segmented_pinyin[0] = "ni".into();
        request.context = "context\u{0}".into();
        assert_eq!(request.validate(), Err(AiError::ContextTooLarge));
        request.context.clear();
        assert_eq!(
            chat_completion_body(&request, "openai", "model", "prompt\u{0}"),
            Err(AiError::InvalidConfiguration)
        );
    }

    #[test]
    fn rejects_control_characters_in_suggestions() {
        let response = AiSuggestionResponse {
            candidates: vec![AiSuggestion {
                text: "候选\u{0}".into(),
            }],
        };
        assert_eq!(response.validate(1), Err(AiError::InvalidCandidate));
    }

    #[test]
    fn request_body_matches_windows_shape_without_credentials() {
        let request = AiSuggestionRequest {
            segmented_pinyin: vec!["ni".into(), "hao".into()],
            context: "合成上下文\n\"引用\"".into(),
            candidate_limit: 3,
        };
        for provider in AI_PROVIDERS {
            let body =
                chat_completion_body(&request, provider, "synthetic-model", "synthetic\nprompt")
                    .unwrap();
            assert_eq!(body["stream"], false);
            assert_eq!(body["temperature"], 0.2);
            assert_eq!(body["max_tokens"], 512);
            assert_eq!(body["response_format"]["type"], "json_object");
            assert_eq!(body["messages"][0]["content"], "synthetic\nprompt");
            let decoded: AiSuggestionRequest =
                serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
            assert_eq!(decoded, request);
            assert_eq!(body.get("thinking").is_some(), provider == "deepseek");
            assert!(body.get("token").is_none());
        }
        for (provider, model, prompt) in [("unknown", "model", "prompt"), ("openai", "", "prompt")]
        {
            assert_eq!(
                chat_completion_body(&request, provider, model, prompt),
                Err(AiError::InvalidConfiguration)
            );
        }
    }

    fn envelope(inner: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(
            &serde_json::json!({"choices":[{"message":{"content":inner.to_string()}}]}),
        )
        .unwrap()
    }
    #[test]
    fn response_preserves_order_filters_and_limits() {
        let body = envelope(serde_json::json!({"candidates":[null,{}, {"text":12},
            {"text":""},{"text":"   "},{"text":"bad\ntext"},{"text":"甲"},{"text":"甲"},{"text":"乙"},{"text":"丙"}]}));
        let response = parse_chat_completion_response(&body, 2).unwrap();
        assert_eq!(
            response
                .candidates
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>(),
            vec!["甲", "乙"]
        );
        assert_eq!(response.candidates.capacity(), 2);
        response.validate(2).unwrap();
        let empty =
            parse_chat_completion_response(&envelope(serde_json::json!({"candidates":[]})), 1)
                .unwrap();
        assert!(empty.candidates.is_empty());
        assert_eq!(empty.candidates.capacity(), 0);
    }
    #[test]
    fn response_rejects_malformed_envelopes_and_bounds() {
        let mut maximum = envelope(serde_json::json!({"candidates":[{"text":"x".repeat(4096)}]}));
        maximum.resize(1024 * 1024, b' ');
        assert_eq!(
            parse_chat_completion_response(&maximum, 1)
                .unwrap()
                .candidates[0]
                .text
                .len(),
            4096
        );
        maximum.push(b' ');
        assert!(parse_chat_completion_response(&maximum, 1).is_none());
        for body in [
            b"not json".to_vec(),
            vec![0xff],
            b"{}".to_vec(),
            br#"{"choices":[]}"#.to_vec(),
            br#"{"choices":[{"message":{"content":{}}}]}"#.to_vec(),
            envelope(serde_json::json!({"candidates":{}})),
            vec![b'x'; 1024 * 1024 + 1],
        ] {
            assert!(parse_chat_completion_response(&body, 3).is_none());
        }
        let body = envelope(
            serde_json::json!({"candidates":[{"text":"字".repeat(1366)},{"text":"valid"}]}),
        );
        assert_eq!(
            parse_chat_completion_response(&body, 1).unwrap().candidates[0].text,
            "valid"
        );
        for limit in [0, 11, 255] {
            assert!(parse_chat_completion_response(&body, limit).is_none());
        }
        let mut error: serde_json::Value = serde_json::from_slice(&body).unwrap();
        error["error"] = serde_json::json!({"message":"synthetic"});
        assert!(parse_chat_completion_response(&serde_json::to_vec(&error).unwrap(), 1).is_none());
    }

    #[test]
    fn http_descriptor_resolves_private_slots_and_prompt_selection() {
        let mut config = crate::preferences::AiAssistantPreferences {
            enabled: true,
            endpoint: "https://api.deepseek.com/chat/completions".into(),
            model: "synthetic-model".into(),
            token: "synthetic-legacy".into(),
            prompt_custom_2: "second prompt".into(),
            ..Default::default()
        };
        let request = AiSuggestionRequest {
            segmented_pinyin: vec!["ni".into()],
            context: String::new(),
            candidate_limit: 3,
        };
        let descriptor = |config: &crate::preferences::AiAssistantPreferences| {
            chat_completion_http_request(config, &request)
                .unwrap()
                .unwrap()
        };
        assert_eq!(
            descriptor(&config)["headers"]["Authorization"],
            "Bearer synthetic-legacy"
        );
        config
            .tokens
            .insert("deepseek".into(), " synthetic-slot ".into());
        config
            .tokens
            .insert("openai".into(), "synthetic-other".into());
        config.prompt_id = "custom_2".into();
        let value = descriptor(&config);
        assert_eq!(value["headers"]["Authorization"], "Bearer synthetic-slot");

        // The settings page stores the token under the endpoint origin and
        // clears the flat field. Reading only the provider slot meant a user
        // who pasted a key got InvalidConfiguration and no request was sent.
        let mut origin_only = config.clone();
        origin_only.tokens.clear();
        origin_only.token = String::new();
        origin_only.provider = "deepseek".into();
        origin_only.endpoint = "https://api.deepseek.com/chat/completions".into();
        origin_only.tokens.insert(
            "https://api.deepseek.com:443".into(),
            "synthetic-origin".into(),
        );
        assert_eq!(
            descriptor(&origin_only)["headers"]["Authorization"],
            "Bearer synthetic-origin"
        );

        // The current endpoint's origin wins over a legacy provider slot.
        let mut both = origin_only.clone();
        both.tokens
            .insert("deepseek".into(), "synthetic-provider".into());
        assert_eq!(
            descriptor(&both)["headers"]["Authorization"],
            "Bearer synthetic-origin"
        );

        // A placeholder in the provider slot must not shadow a real origin key.
        let mut placeholder = origin_only.clone();
        placeholder
            .tokens
            .insert("deepseek".into(), "<YOUR_TOKEN>".into());
        assert_eq!(
            descriptor(&placeholder)["headers"]["Authorization"],
            "Bearer synthetic-origin"
        );

        // An origin key recorded for a different endpoint is not accepted.
        let mut mismatched = origin_only.clone();
        mismatched.tokens.clear();
        mismatched.tokens.insert(
            "https://api.openai.com:443".into(),
            "synthetic-other".into(),
        );
        assert!(matches!(
            chat_completion_http_request(&mismatched, &request),
            Err(AiError::InvalidConfiguration)
        ));

        // 来源键的推导与设置页一致，逐条用例见 `endpoint` 模块读取的 shared/contracts/ai-endpoint/cases.json。
        // 局域网的 http 接口按 `http://host:port` 存 Token，这里要能找到它。
        let mut local = origin_only.clone();
        local.tokens.clear();
        local.endpoint = "http://192.168.1.20:1234/v1/chat/completions".into();
        local
            .tokens
            .insert("http://192.168.1.20:1234".into(), "synthetic-local".into());
        assert_eq!(
            descriptor(&local)["headers"]["Authorization"],
            "Bearer synthetic-local"
        );
        assert_eq!(
            descriptor(&local)["url"],
            "http://192.168.1.20:1234/v1/chat/completions"
        );
        assert_eq!(value["body"]["messages"][0]["content"], "second prompt");
        assert_eq!(value["timeout_ms"], 8000);
        assert_eq!(value["connect_timeout_ms"], 2500);
        assert_eq!(value["max_response_bytes"], 1048576);
        config.prompt_id = "custom_3".into();
        assert_eq!(
            descriptor(&config)["body"]["messages"][0]["content"],
            DEFAULT_CANDIDATE_PROMPT
        );
        config
            .tokens
            .insert("deepseek".into(), "<placeholder>".into());
        assert_eq!(
            chat_completion_http_request(&config, &request),
            Err(AiError::InvalidConfiguration)
        );
        for endpoint in [
            "file:///synthetic",
            "https://user:pass@synthetic.invalid",
            "https://synthetic.invalid/#fragment",
            "https://synthetic.invalid/\n",
        ] {
            config.endpoint = endpoint.into();
            assert!(chat_completion_http_request(&config, &request).is_err());
        }
        config.endpoint = "http://localhost:8080/chat".into();
        config
            .tokens
            .insert("http://localhost:8080".into(), "synthetic-local".into());
        assert!(chat_completion_http_request(&config, &request).is_ok());
        config.endpoint = "http://api.deepseek.com/chat".into();
        assert!(chat_completion_http_request(&config, &request).is_err());
        config.endpoint = "http://localhost.example/chat".into();
        assert!(chat_completion_http_request(&config, &request).is_err());
        config.endpoint = "http://[::1]:8080/chat".into();
        config
            .tokens
            .insert("http://[::1]:8080".into(), "synthetic-local".into());
        assert!(chat_completion_http_request(&config, &request).is_ok());
        config.endpoint = "http://10.0.0.8:1234/v1/chat/completions".into();
        config
            .tokens
            .insert("http://10.0.0.8:1234".into(), "synthetic-local".into());
        assert!(chat_completion_http_request(&config, &request).is_ok());
        config.endpoint = "http://8.8.8.8/v1/chat/completions".into();
        assert!(chat_completion_http_request(&config, &request).is_err());
        config.endpoint = "https://api.deepseek.com/chat/completions".into();
        config.tokens.clear();
        config.token = "bad\r\nheader".into();
        assert!(chat_completion_http_request(&config, &request).is_err());
        config.enabled = false;
        assert_eq!(
            chat_completion_http_request(&config, &request).unwrap(),
            None
        );
    }

    #[test]
    fn request_uses_only_credentials_bound_to_the_current_custom_origin() {
        let mut config = crate::preferences::AiAssistantPreferences {
            enabled: true,
            provider: "deepseek".into(),
            endpoint: "https://custom.invalid/v1/chat/completions".into(),
            model: "synthetic-model".into(),
            token: "synthetic-flat-legacy".into(),
            ..Default::default()
        };
        config
            .tokens
            .insert("deepseek".into(), "synthetic-provider-legacy".into());
        let request = AiSuggestionRequest {
            segmented_pinyin: vec!["ni".into()],
            context: String::new(),
            candidate_limit: config.candidate_limit,
        };
        assert_eq!(
            chat_completion_http_request(&config, &request),
            Err(AiError::InvalidConfiguration)
        );
        config.tokens.insert(
            "https://custom.invalid:443".into(),
            "synthetic-current-origin".into(),
        );
        let descriptor = chat_completion_http_request(&config, &request)
            .unwrap()
            .unwrap();
        assert_eq!(
            descriptor["headers"]["Authorization"],
            "Bearer synthetic-current-origin"
        );
        config
            .tokens
            .insert("https://custom.invalid:443".into(), "  ".into());
        assert_eq!(
            chat_completion_http_request(&config, &request),
            Err(AiError::InvalidConfiguration)
        );
    }
}
