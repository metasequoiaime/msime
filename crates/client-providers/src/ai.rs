//! Host independent contract for asynchronous AI candidate suggestions.
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::preferences::AI_PROVIDERS;

pub mod endpoint;
pub mod jev;
pub mod provider_catalog;
pub mod settings;

#[derive(Debug, Clone, Error, Eq, PartialEq)]
pub enum AiError {
    #[error("segmented pinyin is empty or too large")]
    InvalidSegments,
    #[error("English input is empty, too large or contains non-letters")]
    InvalidEnglishInput,
    #[error("context is too large")]
    ContextTooLarge,
    #[error("candidate limit is invalid")]
    InvalidLimit,
    #[error("candidate text is empty or too large")]
    InvalidCandidate,
    #[error("AI provider request configuration is invalid")]
    InvalidConfiguration,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiInputMode {
    #[default]
    Pinyin,
    English,
}

impl AiInputMode {
    fn is_pinyin(&self) -> bool {
        *self == Self::Pinyin
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiSuggestionRequest {
    #[serde(default, skip_serializing_if = "AiInputMode::is_pinyin")]
    pub mode: AiInputMode,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub input: String,
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiTranslationRequest {
    pub text: String,
    pub source_language: String,
    pub target_language: String,
}

pub const DEFAULT_TRANSLATION_PROMPT: &str = "你是一个机器翻译服务。把用户给出的文本从 source_language 翻译成 target_language。只翻译文本本身，保留原意和标点，不要解释、注释或改写。只输出合法 JSON，不要输出 Markdown：{\"translation\":\"译文\"}。没有合适译文时输出空字符串。";

/// The system prompt a blank prompt slot stands for. It used to exist only in the Windows installer's default configuration, so every other host that never wrote a prompt sent an empty system message and got replies the JSON parser rejects.
pub const DEFAULT_CANDIDATE_PROMPT: &str = "你是一个中文全拼输入法联想引擎。输入为已经切分好的拼音数组、前文上下文和候选数量。\n\n优先生成与拼音严格对应的中文候选：若有 N 段拼音，首选必须尽量为 N 个汉字，每段拼音对应一个汉字，不得随意增删或改变读音。结合上下文、常用程度、语义完整性和固定搭配排序。\n\n若去掉分词后能明显组成更合理的英文单词、缩写、产品名或技术术语，如 `deep + seek → DeepSeek`、`git + hub → GitHub`，可优先返回英文；不要生造英文或做牵强匹配。\n\n只输出合法 JSON，不要解释或输出 Markdown：\n\n{\n\"candidates\": [\n{\n\"text\": \"候选内容\",\n\"type\": \"chinese或english\",\n\"confidence\": 0.98\n}\n]\n}\n\n候选按推荐程度降序排列，数量不超过指定上限；没有合理结果时返回空数组。";

/// 英文模式按原始拼写与前文补全、纠错，不将字母拆成拼音。
pub const DEFAULT_ENGLISH_CANDIDATE_PROMPT: &str = "你是一个英文输入法补全和拼写纠错服务。用户提供 mode=english、原始字母串 input、前文 context 和候选数量 candidate_limit。结合前文返回最可能的完整英文单词，保留合理的大小写，纠正常见拼写错误。不要把字母当作拼音，不翻译成中文，不解释。只输出合法 JSON：{\"candidates\":[{\"text\":\"英文单词\"}]}。按推荐程度降序，数量不超过 candidate_limit；没有合理结果返回空数组。";

pub trait AiSuggestor {
    fn suggest(&self, request: &AiSuggestionRequest) -> Result<AiSuggestionResponse, AiError>;
}

/// DeepSeek 的思考强度档位：`low`/`high`/`max` 在请求体里写成 `reasoning_effort`。
pub fn is_valid_thinking_level(level: &str) -> bool {
    matches!(level, "disabled" | "low" | "high" | "max")
}

/// 应答服务的归属方：DeepSeek 的 `thinking.type`、SiliconFlow 的 `enable_thinking` 这类厂商字段必须按它判断而不是按设置页的服务商标签——一个槽位可以装任何 OpenAI 兼容地址，给不支持的服务发 `thinking.type` 会被整个拒绝（MiniMax 报 requires adaptive thinking）。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ChatVendor {
    Other,
    DeepSeek,
    SiliconFlow,
    Groq,
    OpenAi,
}

/// scheme://[user[:pass]@]host[:port]/path 里只有主机说明厂商；子域名也算归属。
fn vendor_from_chat_endpoint(endpoint: &str) -> ChatVendor {
    let Some(rest) = endpoint.split_once("://").map(|(_, rest)| rest) else {
        return ChatVendor::Other;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let host = host_port
        .split(':')
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let owned_by = |domain: &str| {
        host == domain
            || (host.len() > domain.len()
                && host.ends_with(domain)
                && host.as_bytes()[host.len() - domain.len() - 1] == b'.')
    };
    if owned_by("deepseek.com") {
        ChatVendor::DeepSeek
    } else if owned_by("siliconflow.cn") {
        ChatVendor::SiliconFlow
    } else if owned_by("groq.com") {
        ChatVendor::Groq
    } else if owned_by("openai.com") {
        ChatVendor::OpenAi
    } else {
        ChatVendor::Other
    }
}

/// 端点真正归属的厂商；标签说的和地址不一致时以地址为准。只有地址缺失才让标签说话——自定义槽位指着真实 DeepSeek 时 thinking 依然可用，标着 deepseek 却指向别家的槽位不会发出厂商字段。
fn chat_vendor(provider: &str, endpoint: &str) -> ChatVendor {
    let actual = vendor_from_chat_endpoint(endpoint);
    if actual != ChatVendor::Other {
        return actual;
    }
    if endpoint.contains("://") {
        return ChatVendor::Other;
    }
    match provider {
        "deepseek" => ChatVendor::DeepSeek,
        "siliconflow" => ChatVendor::SiliconFlow,
        "groq" => ChatVendor::Groq,
        "openai" => ChatVendor::OpenAi,
        _ => ChatVendor::Other,
    }
}

/// DeepSeek 厂商且档位不是 disabled；其余一律不开发送思考。
fn uses_deepseek_thinking(provider: &str, endpoint: &str, level: &str) -> bool {
    chat_vendor(provider, endpoint) == ChatVendor::DeepSeek
        && is_valid_thinking_level(level)
        && level != "disabled"
}

/// Pure Windows-compatible non-streaming request body. The host owns endpoint,
/// credentials, timeout and cancellation. Never log prompts or request contents.
pub fn chat_completion_body(
    request: &AiSuggestionRequest,
    provider: &str,
    endpoint: &str,
    model: &str,
    prompt: &str,
    thinking: &str,
) -> Result<serde_json::Value, AiError> {
    request.validate()?;
    if !AI_PROVIDERS.contains(&provider)
        || model.is_empty()
        || !crate::text::is_bounded_text(model, 256)
        || !crate::text::is_bounded_text_with_options(prompt, 16384, true)
    {
        return Err(AiError::InvalidConfiguration);
    }
    // JSON 模式要求对话里出现 "json" 这个词。把 Chat Completions 转成 Responses API 的网关（如 EveryAPI 上的 GPT-6 系列）会把 system 消息放进 `instructions`，只检查 input，于是只在 system 提示里写 JSON 的请求一律 400。在 user 消息里带上 `output_format` 字段，任何提示都能满足这个检查。
    #[derive(Serialize)]
    struct UserMessage<'a> {
        #[serde(flatten)]
        request: &'a AiSuggestionRequest,
        output_format: &'static str,
    }
    let user = UserMessage {
        request,
        output_format: "json",
    };
    let uses_thinking = uses_deepseek_thinking(provider, endpoint, thinking);
    let mut body = serde_json::json!({
        "model":model, "stream":false, "temperature":0.2,
        // 思考 token 计入输出上限：开着思考还按 512 给会挤掉正文 JSON 的空间。
        "max_tokens": if uses_thinking { 8192 } else { 512 },
        "response_format":{"type":"json_object"},
        "messages":[{"role":"system","content":prompt},
            {"role":"user","content":serde_json::to_string(&user).map_err(|_| AiError::InvalidConfiguration)?}]
    });
    if uses_thinking {
        body["thinking"] = serde_json::json!({"type":"enabled"});
        body["reasoning_effort"] = serde_json::Value::String(thinking.to_owned());
    } else if chat_vendor(provider, endpoint) == ChatVendor::DeepSeek {
        // DeepSeek 的思考会给联想多出几百个推理 token、明显变慢；其他服务拿普通请求。
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
    let headers = request_headers(config)?;
    let prompt = match config.prompt_id.as_str() {
        "custom_2" => &config.prompt_custom_2,
        "custom_3" => &config.prompt_custom_3,
        _ => &config.prompt_custom_1,
    };
    let prompt = if prompt.trim().is_empty() {
        match request.mode {
            AiInputMode::Pinyin => DEFAULT_CANDIDATE_PROMPT,
            AiInputMode::English => DEFAULT_ENGLISH_CANDIDATE_PROMPT,
        }
    } else {
        prompt
    };
    let thinking = uses_deepseek_thinking(&config.provider, endpoint, &config.thinking);
    let body = chat_completion_body(
        request,
        &config.provider,
        endpoint,
        &config.model,
        prompt,
        &config.thinking,
    )?;
    if serde_json::to_vec(&body)
        .map_err(|_| AiError::InvalidConfiguration)?
        .len()
        > 65536
    {
        return Err(AiError::InvalidConfiguration);
    }
    // 开着思考的 DeepSeek 回复要等几十秒，8 秒的普通预算会把它全部判超时。
    Ok(Some(serde_json::json!({"url":endpoint,"method":"POST",
        "headers":headers,
        "body":body,"timeout_ms":if thinking {30000} else {8000},"connect_timeout_ms":2500,"max_response_bytes":1048576})))
}

/// 构造与候选请求相同的带凭据 Chat Completions 描述器。
/// 翻译提示词固定，避免用户自定义联想提示词让模型返回键盘会显示为释义的任意文本。
pub fn chat_translation_http_request(
    config: &crate::preferences::AiAssistantPreferences,
    request: &AiTranslationRequest,
) -> Result<Option<serde_json::Value>, AiError> {
    if !config.enabled {
        return Ok(None);
    }
    request.validate()?;
    if !AI_PROVIDERS.contains(&config.provider.as_str())
        || config.model.is_empty()
        || !crate::text::is_bounded_text(&config.model, 256)
    {
        return Err(AiError::InvalidConfiguration);
    }
    let headers = request_headers(config)?;
    let user = serde_json::json!({
        "source_language": request.source_language,
        "target_language": request.target_language,
        "text": request.text,
        "output_format": "json",
    });
    let mut body = serde_json::json!({
        "model": config.model,
        "stream": false,
        "temperature": 0.0,
        "max_tokens": 512,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": DEFAULT_TRANSLATION_PROMPT},
            {"role": "user", "content": serde_json::to_string(&user).map_err(|_| AiError::InvalidConfiguration)?},
        ]
    });
    if config.provider == "deepseek" {
        body["thinking"] = serde_json::json!({"type": "disabled"});
    }
    if serde_json::to_vec(&body)
        .map_err(|_| AiError::InvalidConfiguration)?
        .len()
        > 65536
    {
        return Err(AiError::InvalidConfiguration);
    }
    Ok(Some(serde_json::json!({
        "url": config.endpoint,
        "method": "POST",
        "headers": headers,
        "body": body,
        "timeout_ms": 8000,
        "connect_timeout_ms": 2500,
        "max_response_bytes": 1048576,
    })))
}

pub fn parse_chat_translation_response(body: &[u8]) -> Option<String> {
    if body.len() > 1024 * 1024 {
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
    let inner: serde_json::Value = serde_json::from_str(strip_code_fence(content)).ok()?;
    let text = inner.get("translation")?.as_str()?.trim();
    if text.is_empty() || text.chars().count() > 4096 || text.chars().any(char::is_control) {
        return None;
    }
    Some(text.to_owned())
}

/// 判断 Chat Completions 翻译回复是服务失败，还是没有可用译文的合法回答。
/// 空的 `translation` 是合法的否定回答，可以缓存；JSON 损坏、错误信封或字段无效的回复需要重试。
pub fn chat_translation_response_failed(body: &[u8]) -> bool {
    if body.len() > 1024 * 1024 {
        return true;
    }
    let Ok(outer) = serde_json::from_slice::<serde_json::Value>(body) else {
        return true;
    };
    if outer.get("error").is_some_and(|error| !error.is_null()) {
        return true;
    }
    let Some(content) = outer
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
    else {
        return true;
    };
    let Ok(inner) = serde_json::from_str::<serde_json::Value>(strip_code_fence(content)) else {
        return true;
    };
    let Some(text) = inner.get("translation").and_then(serde_json::Value::as_str) else {
        return true;
    };
    let text = text.trim();
    text.chars().count() > 4096 || text.chars().any(char::is_control)
}

/// Ollama 跑在用户自己的机器或局域网里，不做鉴权：设置页选它时不显示 Token，请求不读存下的 Token，也不带 `Authorization`。其他服务都要 Token。
pub fn provider_uses_token(provider: &str) -> bool {
    provider != "ollama"
}

/// 发出请求的条件里和凭据有关的那一条：不用 Token 的服务总是满足，其余要有这个接口地址可用的 Token。
pub fn has_request_credential(config: &crate::preferences::AiAssistantPreferences) -> bool {
    !provider_uses_token(&config.provider) || credential_for_endpoint(config).is_some()
}

/// 候选与翻译请求共用的请求头。Contains the bearer token when the provider uses one: never log it.
fn request_headers(
    config: &crate::preferences::AiAssistantPreferences,
) -> Result<serde_json::Value, AiError> {
    let mut headers = serde_json::json!({"Content-Type": "application/json"});
    if provider_uses_token(&config.provider) {
        let token = credential_for_endpoint(config).ok_or(AiError::InvalidConfiguration)?;
        headers["Authorization"] = serde_json::Value::String(format!("Bearer {token}"));
    }
    Ok(headers)
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
        "minimax" => "https://api.minimax.cn:443",
        "volcengine" => "https://ark.cn-beijing.volces.com:443",
        "siliconflow" => "https://api.siliconflow.cn:443",
        "groq" => "https://api.groq.com:443",
        "openrouter" => "https://openrouter.ai:443",
        "jev" => "https://api.typesafe.ai:443",
        _ => return None,
    })
}

/// 去掉包在正文外面的 Markdown 代码块。不支持 `response_format` 的模型（如经网关调用的 Claude）会无视 JSON 模式，回答写成 ```json ... ```，里面仍是合法 JSON。
fn strip_code_fence(content: &str) -> &str {
    let trimmed = content.trim();
    let Some(fenced) = trimmed
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
    else {
        return trimmed;
    };
    // 第一行是信息串（如 `json`），只有一行时没有信息串。
    fenced.split_once('\n').map_or(fenced, |(_, json)| json)
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
    let inner: serde_json::Value = serde_json::from_str(strip_code_fence(content)).ok()?;
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
        match self.mode {
            AiInputMode::Pinyin => {
                if !self.input.is_empty()
                    || self.segmented_pinyin.is_empty()
                    || self.segmented_pinyin.len() > 128
                    || self
                        .segmented_pinyin
                        .iter()
                        .any(|part| part.is_empty() || !crate::text::is_bounded_text(part, 32))
                {
                    return Err(AiError::InvalidSegments);
                }
            }
            AiInputMode::English => {
                if !self.segmented_pinyin.is_empty()
                    || self.input.is_empty()
                    || self.input.len() > 256
                    || !self.input.bytes().all(|byte| byte.is_ascii_alphabetic())
                {
                    return Err(AiError::InvalidEnglishInput);
                }
            }
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

impl AiTranslationRequest {
    pub fn validate(&self) -> Result<(), AiError> {
        if !msime_client_core::translation::is_valid_source_text(&self.text) {
            return Err(AiError::InvalidCandidate);
        }
        if !msime_client_core::translation::is_supported_translation_pair(
            &self.source_language,
            &self.target_language,
        ) || self.source_language == self.target_language
        {
            return Err(AiError::InvalidConfiguration);
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

    #[test]
    fn english_online_request_carries_mode_spelling_context_and_prompt() {
        let mut ai = crate::preferences::Preferences::default().ai_assistant;
        ai.enabled = true;
        ai.endpoint = "https://synthetic.invalid/chat".into();
        ai.model = "synthetic-model".into();
        ai.tokens.insert(
            "https://synthetic.invalid:443".into(),
            "synthetic-token".into(),
        );
        let mut request = AiSuggestionRequest {
            mode: AiInputMode::English,
            input: "accessability".into(),
            segmented_pinyin: vec![],
            context: "Synthetic preceding sentence.".into(),
            candidate_limit: ai.candidate_limit,
        };
        let descriptor = chat_completion_http_request(&ai, &request)
            .unwrap()
            .unwrap();
        let body = &descriptor["body"];
        assert_eq!(
            body["messages"][0]["content"],
            DEFAULT_ENGLISH_CANDIDATE_PROMPT
        );
        let user: serde_json::Value =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(user["mode"], "english");
        assert_eq!(user["input"], "accessability");
        assert_eq!(user["context"], request.context);
        ai.prompt_id = "custom_2".into();
        ai.prompt_custom_2 = "合成自定义英文提示".into();
        let descriptor = chat_completion_http_request(&ai, &request)
            .unwrap()
            .unwrap();
        assert_eq!(
            descriptor["body"]["messages"][0]["content"],
            "合成自定义英文提示"
        );
        for input in [
            "",
            "https://synthetic.invalid",
            "word name",
            "word1",
            "拼音",
        ] {
            request.input = input.into();
            assert_eq!(request.validate(), Err(AiError::InvalidEnglishInput));
        }
        request.input = "a".repeat(257);
        assert_eq!(request.validate(), Err(AiError::InvalidEnglishInput));
        request.input = "word".into();
        request.segmented_pinyin = vec!["ni".into()];
        assert_eq!(request.validate(), Err(AiError::InvalidEnglishInput));
    }

    #[test]
    fn default_ai_assistant_requests_carry_the_builtin_associative_prompt() {
        // The stored slot stays empty on purpose: blank means "use the built-in prompt" at request time, here and in the Linux provider's copy of the same text.
        let mut ai = crate::preferences::Preferences::default().ai_assistant;
        assert!(ai.prompt_custom_1.is_empty());
        ai.enabled = true;
        ai.endpoint = "https://synthetic.invalid/chat".into();
        ai.model = "synthetic-model".into();
        ai.tokens.insert(
            "https://synthetic.invalid:443".into(),
            "synthetic-token".into(),
        );
        let request = AiSuggestionRequest {
            mode: AiInputMode::Pinyin,
            input: String::new(),
            segmented_pinyin: vec!["shu".into(), "ru".into()],
            context: String::new(),
            candidate_limit: ai.candidate_limit,
        };
        let descriptor = chat_completion_http_request(&ai, &request)
            .unwrap()
            .unwrap();
        let system = &descriptor["body"]["messages"][0];
        assert_eq!(system["role"], "system");
        assert_eq!(system["content"], DEFAULT_CANDIDATE_PROMPT);
        assert!(DEFAULT_CANDIDATE_PROMPT.contains("JSON"));
        assert!(DEFAULT_CANDIDATE_PROMPT.contains("\"candidates\""));
    }

    #[test]
    fn deepseek_thinking_level_shapes_the_request() {
        // 档位按接口地址的厂商判定（Windows 的 thinking.type/reasoning_effort/8192/30s 契约）：disabled 只写 type；开启写 enabled 加 reasoning_effort，并放大输出上限与请求预算。
        let request = AiSuggestionRequest {
            mode: AiInputMode::Pinyin,
            input: String::new(),
            segmented_pinyin: vec!["ni".into()],
            context: String::new(),
            candidate_limit: 3,
        };
        let config = |provider: &str, endpoint: &str, thinking: &str| {
            let mut ai = crate::preferences::Preferences::default().ai_assistant;
            ai.enabled = true;
            ai.provider = provider.into();
            ai.endpoint = endpoint.into();
            ai.model = "deepseek-v4-flash".into();
            ai.thinking = thinking.into();
            ai.tokens
                .insert("https://api.deepseek.com:443".into(), "t".into());
            ai.tokens
                .insert("https://synthetic.invalid:443".into(), "t".into());
            ai
        };
        // disabled：只写 thinking.type，预算不变。
        let descriptor = chat_completion_http_request(
            &config(
                "deepseek",
                "https://api.deepseek.com/chat/completions",
                "disabled",
            ),
            &request,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            descriptor["body"]["thinking"],
            serde_json::json!({"type":"disabled"})
        );
        assert!(descriptor["body"].get("reasoning_effort").is_none());
        assert_eq!(descriptor["body"]["max_tokens"], 512);
        assert_eq!(descriptor["timeout_ms"], 8000);
        // high：enabled 加 reasoning_effort，输出上限和超时放大。
        let descriptor = chat_completion_http_request(
            &config(
                "deepseek",
                "https://api.deepseek.com/chat/completions",
                "high",
            ),
            &request,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            descriptor["body"]["thinking"],
            serde_json::json!({"type":"enabled"})
        );
        assert_eq!(descriptor["body"]["reasoning_effort"], "high");
        assert_eq!(descriptor["body"]["max_tokens"], 8192);
        assert_eq!(descriptor["timeout_ms"], 30000);
        // 目录外的档位按关闭处理。
        let descriptor = chat_completion_http_request(
            &config(
                "deepseek",
                "https://api.deepseek.com/chat/completions",
                "medium",
            ),
            &request,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            descriptor["body"]["thinking"],
            serde_json::json!({"type":"disabled"})
        );
        // 标签是 deepseek、接口地址在别家：厂商字段不外发。
        let descriptor = chat_completion_http_request(
            &config("deepseek", "https://synthetic.invalid/chat", "high"),
            &request,
        )
        .unwrap()
        .unwrap();
        assert!(descriptor["body"].get("thinking").is_none());
        // 自定义标签指向真实 DeepSeek：按地址判定厂商。
        let descriptor = chat_completion_http_request(
            &config("custom", "https://api.deepseek.com/chat/completions", "low"),
            &request,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            descriptor["body"]["thinking"],
            serde_json::json!({"type":"enabled"})
        );
        assert_eq!(descriptor["body"]["reasoning_effort"], "low");
        // 其他厂商不受影响。
        let descriptor = chat_completion_http_request(
            &config("openai", "https://synthetic.invalid/chat", "high"),
            &request,
        )
        .unwrap()
        .unwrap();
        assert!(descriptor["body"].get("thinking").is_none());
    }

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
            mode: AiInputMode::Pinyin,
            input: String::new(),
            segmented_pinyin: vec!["ni".into(), "hao".into()],
            context: String::new(),
            candidate_limit: 3,
        };
        assert_eq!(suggest(&Stub, &request).unwrap().candidates[0].text, "你好");
    }
    #[test]
    fn rejects_invalid_bounds() {
        let mut request = AiSuggestionRequest {
            mode: AiInputMode::Pinyin,
            input: String::new(),
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
    fn distinguishes_empty_translation_from_failed_reply() {
        let empty = serde_json::json!({
            "choices": [{"message": {"content": "{\"translation\":\"\"}"}}]
        });
        assert!(!chat_translation_response_failed(
            &serde_json::to_vec(&empty).unwrap()
        ));

        let failed = serde_json::json!({"error": {"message": "synthetic failure"}});
        assert!(chat_translation_response_failed(
            &serde_json::to_vec(&failed).unwrap()
        ));
    }

    #[test]
    fn rejects_control_characters_in_model_inputs() {
        let mut request = AiSuggestionRequest {
            mode: AiInputMode::Pinyin,
            input: String::new(),
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
            chat_completion_body(&request, "openai", "", "model", "prompt\u{0}", ""),
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
            mode: AiInputMode::Pinyin,
            input: String::new(),
            segmented_pinyin: vec!["ni".into(), "hao".into()],
            context: "合成上下文\n\"引用\"".into(),
            candidate_limit: 3,
        };
        for provider in AI_PROVIDERS {
            let body = chat_completion_body(
                &request,
                provider,
                "",
                "synthetic-model",
                "synthetic\nprompt",
                "",
            )
            .unwrap();
            assert_eq!(body["stream"], false);
            assert_eq!(body["temperature"], 0.2);
            assert_eq!(body["max_tokens"], 512);
            assert_eq!(body["response_format"]["type"], "json_object");
            assert_eq!(body["messages"][0]["content"], "synthetic\nprompt");
            let user = body["messages"][1]["content"].as_str().unwrap();
            let decoded: AiSuggestionRequest = serde_json::from_str(user).unwrap();
            assert_eq!(decoded, request);
            // 网关只在 input 里找 "json"，system 提示里的不算。
            let fields: serde_json::Value = serde_json::from_str(user).unwrap();
            assert_eq!(fields["output_format"], "json");
            assert_eq!(body.get("thinking").is_some(), provider == "deepseek");
            assert!(body.get("token").is_none());
        }
        for (provider, model, prompt) in [("unknown", "model", "prompt"), ("openai", "", "prompt")]
        {
            assert_eq!(
                chat_completion_body(&request, provider, "", model, prompt, ""),
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
    fn response_accepts_json_wrapped_in_a_code_fence() {
        let fenced = |content: &str| {
            serde_json::to_vec(&serde_json::json!({"choices":[{"message":{"content":content}}]}))
                .unwrap()
        };
        let inner = r#"{"candidates":[{"text":"甲"},{"text":"乙"}]}"#;
        for content in [
            format!("```json\n{inner}\n```"),
            format!("  ```JSON\n{inner}\n```\n"),
            format!("```\n{inner}\n```"),
            format!("```{inner}```"),
        ] {
            let response = parse_chat_completion_response(&fenced(&content), 3).unwrap();
            assert_eq!(
                response
                    .candidates
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<Vec<_>>(),
                vec!["甲", "乙"],
                "{content:?}"
            );
        }
        for content in [
            format!("```json\n{inner}"),
            format!("说明：\n```json\n{inner}\n```"),
            "```json\nnot json\n```".to_string(),
        ] {
            assert!(
                parse_chat_completion_response(&fenced(&content), 3).is_none(),
                "{content:?}"
            );
        }
    }

    #[test]
    fn translation_descriptor_reuses_ai_credentials_and_parses_one_gloss() {
        let mut config = crate::preferences::Preferences::default().ai_assistant;
        config.enabled = true;
        config.provider = "openai".into();
        config.endpoint = "https://synthetic.invalid/v1/chat/completions".into();
        config.model = "synthetic-model".into();
        config.tokens.insert(
            "https://synthetic.invalid:443".into(),
            "synthetic-token".into(),
        );
        let request = AiTranslationRequest {
            text: "这是一段超过旧限制的合成长句，用来验证共享翻译上限已经放宽。".into(),
            source_language: "zh".into(),
            target_language: "en".into(),
        };
        let descriptor = chat_translation_http_request(&config, &request)
            .unwrap()
            .unwrap();
        assert_eq!(descriptor["body"]["model"], "synthetic-model");
        assert_eq!(descriptor["body"]["response_format"]["type"], "json_object");
        assert_eq!(
            descriptor["headers"]["Authorization"],
            "Bearer synthetic-token"
        );
        let body = envelope(serde_json::json!({"translation":"a synthetic translation"}));
        assert_eq!(
            parse_chat_translation_response(&body).as_deref(),
            Some("a synthetic translation")
        );
    }

    #[test]
    fn translation_descriptor_rejects_invalid_provider_and_model() {
        let mut config = crate::preferences::Preferences::default().ai_assistant;
        config.enabled = true;
        config.endpoint = "https://synthetic.invalid/v1/chat/completions".into();
        config.tokens.insert(
            "https://synthetic.invalid:443".into(),
            "synthetic-token".into(),
        );
        let request = AiTranslationRequest {
            text: "合成文本".into(),
            source_language: "zh".into(),
            target_language: "en".into(),
        };
        config.provider = "unknown".into();
        assert_eq!(
            chat_translation_http_request(&config, &request),
            Err(AiError::InvalidConfiguration)
        );
        config.provider = "openai".into();
        config.model.clear();
        assert_eq!(
            chat_translation_http_request(&config, &request),
            Err(AiError::InvalidConfiguration)
        );
    }

    #[test]
    fn translation_rejects_invalid_pairs_and_malformed_replies() {
        assert!(AiTranslationRequest {
            text: "你好".into(),
            source_language: "zh".into(),
            target_language: "zh".into(),
        }
        .validate()
        .is_err());
        assert!(
            parse_chat_translation_response(&envelope(serde_json::json!({"translation":""})))
                .is_none()
        );
        assert!(parse_chat_translation_response(&envelope(
            serde_json::json!({"translation":"x\u{0}"})
        ))
        .is_none());
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
            mode: AiInputMode::Pinyin,
            input: String::new(),
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
    fn ollama_requests_never_read_or_send_a_token() {
        let mut config = crate::preferences::AiAssistantPreferences {
            enabled: true,
            provider: "ollama".into(),
            endpoint: "http://localhost:11434/v1/chat/completions".into(),
            model: "synthetic-model".into(),
            ..Default::default()
        };
        // 以前在这个地址上存过的 Token 也不读。
        config
            .tokens
            .insert("http://localhost:11434".into(), "synthetic-stale".into());
        let request = AiSuggestionRequest {
            mode: AiInputMode::Pinyin,
            input: String::new(),
            segmented_pinyin: vec!["ni".into()],
            context: String::new(),
            candidate_limit: config.candidate_limit,
        };
        let descriptor = chat_completion_http_request(&config, &request)
            .unwrap()
            .unwrap();
        assert_eq!(
            descriptor["headers"],
            serde_json::json!({"Content-Type": "application/json"})
        );
        assert_eq!(
            descriptor["url"],
            "http://localhost:11434/v1/chat/completions"
        );
        let translation = chat_translation_http_request(
            &config,
            &AiTranslationRequest {
                text: "你好".into(),
                source_language: "zh".into(),
                target_language: "en".into(),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            translation["headers"],
            serde_json::json!({"Content-Type": "application/json"})
        );
        config.tokens.clear();
        assert!(has_request_credential(&config));

        // 公网的 http 地址照样拒绝，不鉴权不等于放宽地址规则。
        config.endpoint = "http://8.8.8.8/v1/chat/completions".into();
        assert_eq!(
            chat_completion_http_request(&config, &request),
            Err(AiError::InvalidConfiguration)
        );

        // 其他服务没有 Token 仍然不发请求。
        config.provider = "custom".into();
        config.endpoint = "http://localhost:11434/v1/chat/completions".into();
        assert!(!has_request_credential(&config));
        assert_eq!(
            chat_completion_http_request(&config, &request),
            Err(AiError::InvalidConfiguration)
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
            mode: AiInputMode::Pinyin,
            input: String::new(),
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
