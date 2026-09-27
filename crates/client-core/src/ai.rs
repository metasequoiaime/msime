//! Host independent contract for asynchronous AI candidate suggestions.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::preferences::AI_PROVIDERS;

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
        || prompt.len() > 16384
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
/// The key the settings page stores an AI token under: `https://host:port`.
///
/// Mirrors `aiCredentialOrigin` in the shared UI, including its refusals, so a
/// token stored by the page is found here and an endpoint the page would not
/// have keyed is not matched by accident.
fn credential_origin(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || !crate::text::is_bounded_text(endpoint, 2048) {
        return None;
    }
    let url = reqwest::Url::parse(endpoint).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    if url.fragment().is_some() {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(format!("https://{host}:{}", url.port().unwrap_or(443)))
}

pub fn chat_completion_http_request(
    config: &crate::preferences::AiAssistantPreferences,
    request: &AiSuggestionRequest,
) -> Result<Option<serde_json::Value>, AiError> {
    if !config.enabled {
        return Ok(None);
    }
    let endpoint = &config.endpoint;
    let url = reqwest::Url::parse(endpoint).map_err(|_| AiError::InvalidConfiguration)?;
    if !crate::text::is_bounded_text(endpoint, 2048)
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || request.candidate_limit != config.candidate_limit
    {
        return Err(AiError::InvalidConfiguration);
    }
    // Two key spaces reach this map and they never intersected. The reference
    // keeps one slot per provider id, and this function has always read that.
    // The settings page, however, stores the token under the endpoint's origin
    // ("https://api.deepseek.com:443") and clears the legacy flat field, so a
    // user who pasted a key got InvalidConfiguration and no request was ever
    // sent. Android reads the origin key, which is why it kept working and why
    // re-keying the page would break it instead. Accept either.
    let origin = credential_origin(&config.endpoint);
    let usable = |token: &&String| {
        let token = token.trim();
        !token.is_empty()
            && !token.starts_with("FAKESECRET_")
            && !(token.starts_with('<') && token.ends_with('>'))
    };
    let token = config
        .tokens
        .get(&config.provider)
        .filter(usable)
        .or_else(|| {
            origin
                .as_deref()
                .and_then(|key| config.tokens.get(key))
                .filter(usable)
        })
        .map(String::as_str)
        .unwrap_or(&config.token)
        .trim();
    if token.is_empty()
        || !crate::text::is_bounded_text(token, 4096)
        || token.starts_with("FAKESECRET_")
        || (token.starts_with('<') && token.ends_with('>'))
    {
        return Err(AiError::InvalidConfiguration);
    }
    let prompt = match config.prompt_id.as_str() {
        "custom_2" => &config.prompt_custom_2,
        "custom_3" => &config.prompt_custom_3,
        _ if !config.prompt_custom_1.is_empty() => &config.prompt_custom_1,
        _ => &config.prompt,
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
            || text.len() > 4096
            || text.chars().any(char::is_control)
            || candidates.iter().any(|candidate| candidate.text == text)
        {
            continue;
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
                .any(|part| part.is_empty() || part.len() > 32)
        {
            return Err(AiError::InvalidSegments);
        }
        if self.context.len() > 16 * 1024 {
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
        if self
            .candidates
            .iter()
            .any(|candidate| candidate.text.is_empty() || candidate.text.len() > 4096)
        {
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
        response.validate(2).unwrap();
        assert!(
            parse_chat_completion_response(&envelope(serde_json::json!({"candidates":[]})), 1)
                .unwrap()
                .candidates
                .is_empty()
        );
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
            endpoint: "https://synthetic.invalid/chat".into(),
            model: "synthetic-model".into(),
            token: "synthetic-legacy".into(),
            prompt: "legacy prompt".into(),
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

        // The provider slot still wins when both are present, which is the
        // shape the reference uses.
        let mut both = origin_only.clone();
        both.tokens
            .insert("deepseek".into(), "synthetic-provider".into());
        assert_eq!(
            descriptor(&both)["headers"]["Authorization"],
            "Bearer synthetic-provider"
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

        // The origin key is derived the same way the page derives it: the
        // default HTTPS port is spelled out, and the host is lowercased.
        assert_eq!(
            credential_origin("https://API.DeepSeek.com/chat/completions").as_deref(),
            Some("https://api.deepseek.com:443")
        );
        assert_eq!(
            credential_origin("https://host.example:8443/v1").as_deref(),
            Some("https://host.example:8443")
        );
        // Anything the page would refuse to key is refused here too.
        assert!(credential_origin("http://api.deepseek.com/x").is_none());
        assert!(credential_origin("https://user:pw@api.deepseek.com/x").is_none());
        assert!(credential_origin("https://api.deepseek.com/x#frag").is_none());
        assert!(credential_origin("").is_none());
        assert!(credential_origin("not a url").is_none());
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
            descriptor(&config)["headers"]["Authorization"],
            "Bearer synthetic-legacy"
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
        assert!(chat_completion_http_request(&config, &request).is_ok());
        config.token = "bad\r\nheader".into();
        assert!(chat_completion_http_request(&config, &request).is_err());
        config.enabled = false;
        assert_eq!(
            chat_completion_http_request(&config, &request).unwrap(),
            None
        );
    }
}
