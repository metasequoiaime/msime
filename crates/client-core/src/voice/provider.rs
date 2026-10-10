//! Which transcription provider a mobile host should use, and the optional rewrite after it.
//!
//! Both answers come out of the settings document, and both used to be resolved inside the desktop
//! shell — which meant the Android keyboard, whose own voice entry never goes through that shell,
//! could not reach either. It launched the platform recogniser unconditionally while the settings
//! app honoured whatever the user had configured, so the same device transcribed one way from the
//! keyboard and another way from the panel.
//!
//! Resolved here so both callers share one implementation rather than the keyboard growing a
//! second copy in Java.

use crate::preferences::Preferences;
use std::collections::BTreeMap;

/// One HTTP or WebSocket header the provider requires, already filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceRequestHeader {
    pub name: String,
    pub value: String,
}

/// 豆包的流式 WebSocket 协议。
pub const ASR_REQUEST_DOUBAO_WEBSOCKET: &str = "doubao_websocket";
/// OpenAI 兼容的 `/audio/transcriptions`：multipart 上传 `model`、`language` 和 `file`（16 kHz 单声道 16 位 WAV），回答里取 `text`。
pub const ASR_REQUEST_MULTIPART: &str = "multipart";
/// Chat Completions 带音频输入（阿里云百炼的 qwen3-asr-flash）：JSON 请求体，唯一一条 user 消息的 content 是 `{"type":"input_audio","input_audio":{"data":"data:audio/wav;base64,..."}}`，回答里取 `choices[0].message.content`。
pub const ASR_REQUEST_CHAT_AUDIO: &str = "chat_audio";
/// 设备上的本地模型，没有网络请求。
pub const ASR_REQUEST_LOCAL: &str = "local";

/// [`ASR_REQUEST_CHAT_AUDIO`] 一次最多上传的 WAV 字节数。百炼限制请求里的音频（含 Base64 编码）不超过 10 MB，Base64 让体积变成 4/3，再给数据 URL 前缀留出余量；16 kHz 单声道 16 位约 218 秒。
pub const CHAT_AUDIO_MAX_WAV_BYTES: usize = 7_000_000;

/// 各识别服务的请求格式。宿主按这个字段挑请求构造，不再按 provider 名字各自判断；不认识的 provider 为 `None`。
pub fn asr_request_format(provider: &str) -> Option<&'static str> {
    match provider {
        "doubao" => Some(ASR_REQUEST_DOUBAO_WEBSOCKET),
        "openai" | "siliconflow" | "groq" | "everyapi" | "mistral" => Some(ASR_REQUEST_MULTIPART),
        "bailian" => Some(ASR_REQUEST_CHAT_AUDIO),
        "local" => Some(ASR_REQUEST_LOCAL),
        _ => None,
    }
}

/// [`ASR_REQUEST_CHAT_AUDIO`] 的请求体。不带 `asr_options`：语种交给模型自动识别，和其他服务不传语种时一样。
pub fn chat_audio_request_body(model: &str, wav: &[u8]) -> serde_json::Value {
    use base64::Engine as _;
    let data = format!(
        "data:audio/wav;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(wav)
    );
    serde_json::json!({
        "model": model,
        "stream": false,
        "messages": [{
            "role": "user",
            "content": [{ "type": "input_audio", "input_audio": { "data": data } }],
        }],
    })
}

/// The transcription provider a mobile host should use, resolved from the settings document.
#[derive(Debug, PartialEq, Eq)]
pub struct MobileVoiceProviderConfiguration {
    pub provider: String,
    /// 请求格式，[`asr_request_format`] 的取值之一。
    pub request_format: String,
    pub endpoint: String,
    pub model: String,
    pub token: String,
    pub headers: Vec<VoiceRequestHeader>,
    pub enable_itn: bool,
    pub enable_punctuation: bool,
    pub enable_ddc: bool,
    pub boosting_table_id: String,
    /// The installed on-device model directory for provider `local`; empty for every network provider, which carry an endpoint and token instead.
    pub model_path: String,
}

/// The optional rewrite that runs over a transcript, resolved the same way the provider is.
///
/// `None` means the user did not ask for it, which is the common case and not an error. The prompt
/// itself is not resolved here: the shipped preset bodies live in `shared/voice/PolishPrompt.h`,
/// and the host that sends the request reads them from there so there is one copy of the wording
/// that tells the model the transcript is data rather than instructions.
#[derive(Debug, PartialEq, Eq)]
pub struct MobileVoicePolishConfiguration {
    pub endpoint: String,
    pub model: String,
    pub token: String,
    pub prompt_id: String,
    pub prompt_custom_1: String,
    pub prompt_custom_2: String,
    pub prompt_custom_3: String,
}

struct ResolvedVoiceFields {
    endpoint: String,
    model: String,
    token: String,
}

/// Whether the endpoint, model and token fields fit the shared mobile voice contract.
pub fn bounded_voice_fields(endpoint: &str, model: &str, token: &str) -> bool {
    crate::text::is_bounded_text(endpoint, 2_048)
        && crate::text::is_bounded_text(model, 512)
        && crate::text::is_bounded_text(token, 16 * 1024)
}

/// 在交给原生宿主前统一校验移动语音传输使用的 URL 形状。
/// 凭据不能嵌在 authority 中；缺少 authority 的地址若只检查 scheme 前缀会被错误放行，
/// 随后又被平台传输层拒绝。
pub fn valid_mobile_voice_endpoint(endpoint: &str, websocket: bool) -> bool {
    if !crate::text::is_bounded_text(endpoint, 2_048) {
        return false;
    }
    let expected_scheme = if websocket { "wss" } else { "https" };
    reqwest::Url::parse(endpoint).ok().is_some_and(|url| {
        url.scheme() == expected_scheme
            && endpoint.split_once("://").is_some_and(|(_, authority)| {
                authority
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| *byte != b'/')
            })
            && url.host_str().is_some_and(|host| !host.is_empty())
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}

fn bounded_resolved_voice_fields(fields: &ResolvedVoiceFields) -> bool {
    bounded_voice_fields(&fields.endpoint, &fields.model, &fields.token)
}

fn resolve_voice_fields(
    provider: &str,
    endpoint: &str,
    model: &str,
    token: &str,
    tokens: &BTreeMap<String, String>,
    defaults: (&str, &str),
) -> ResolvedVoiceFields {
    let endpoint = match endpoint.trim() {
        "" => defaults.0,
        value => value,
    };
    let model = match model.trim() {
        "" => defaults.1,
        value => value,
    };
    let token = match token.trim() {
        "" => tokens.get(provider).map(String::as_str).unwrap_or(""),
        value => value,
    };
    ResolvedVoiceFields {
        endpoint: endpoint.to_owned(),
        model: model.to_owned(),
        token: token.trim().to_owned(),
    }
}

pub fn mobile_voice_polish_configuration(
    preferences: &Preferences,
) -> Option<MobileVoicePolishConfiguration> {
    let voice = &preferences.voice_input;
    if !(voice.polish_enabled || voice.polish_text) {
        return None;
    }
    // The same OpenAI-compatible chat endpoints the AI assistant uses; an unrecognised provider
    // has no default to fall back to and simply means "not configured".
    let (default_endpoint, default_model) = match voice.polish_provider.as_str() {
        "openai" => ("https://api.openai.com/v1/chat/completions", "gpt-4o-mini"),
        "siliconflow" => (
            "https://api.siliconflow.cn/v1/chat/completions",
            "Qwen/Qwen2.5-7B-Instruct",
        ),
        "groq" => (
            "https://api.groq.com/openai/v1/chat/completions",
            "llama-3.3-70b-versatile",
        ),
        "everyapi" => (
            "https://api.everyapi.ai/v1/chat/completions",
            "openai/gpt-4o-mini",
        ),
        "mistral" => (
            "https://api.mistral.ai/v1/chat/completions",
            "mistral-small-latest",
        ),
        "deepseek" => (
            "https://api.deepseek.com/v1/chat/completions",
            "deepseek-chat",
        ),
        _ => return None,
    };
    let fields = resolve_voice_fields(
        &voice.polish_provider,
        &voice.polish_endpoint,
        &voice.polish_model,
        &voice.polish_token,
        &voice.polish_tokens,
        (default_endpoint, default_model),
    );
    if !valid_mobile_voice_endpoint(&fields.endpoint, false)
        || !bounded_resolved_voice_fields(&fields)
        || fields.model.is_empty()
        || fields.token.is_empty()
    {
        return None;
    }
    Some(MobileVoicePolishConfiguration {
        endpoint: fields.endpoint,
        model: fields.model,
        token: fields.token,
        prompt_id: voice.polish_prompt_id.clone(),
        prompt_custom_1: voice.polish_prompt_custom_1.clone(),
        prompt_custom_2: voice.polish_prompt_custom_2.clone(),
        prompt_custom_3: voice.polish_prompt_custom_3.clone(),
    })
}

pub fn mobile_voice_provider_configuration(
    preferences: &Preferences,
) -> Option<MobileVoiceProviderConfiguration> {
    let voice = &preferences.voice_input;
    if voice.asr_provider == "local" {
        return local_provider_configuration(preferences);
    }
    let (default_endpoint, default_model) = match voice.asr_provider.as_str() {
        "doubao" => (
            "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
            "",
        ),
        "openai" => (
            "https://api.openai.com/v1/audio/transcriptions",
            "whisper-1",
        ),
        "siliconflow" => (
            "https://api.siliconflow.cn/v1/audio/transcriptions",
            "FunAudioLLM/SenseVoiceSmall",
        ),
        "groq" => (
            "https://api.groq.com/openai/v1/audio/transcriptions",
            "whisper-large-v3-turbo",
        ),
        "everyapi" => (
            "https://api.everyapi.ai/v1/audio/transcriptions",
            "openai/whisper-large-v3-turbo",
        ),
        "mistral" => (
            "https://api.mistral.ai/v1/audio/transcriptions",
            "voxtral-mini-latest",
        ),
        "bailian" => (
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "qwen3-asr-flash",
        ),
        _ => {
            return None;
        }
    };
    let request_format = asr_request_format(&voice.asr_provider)?;
    let fields = resolve_voice_fields(
        &voice.asr_provider,
        &voice.asr_endpoint,
        &voice.asr_model,
        &voice.asr_token,
        &voice.asr_tokens,
        (default_endpoint, default_model),
    );
    let boosting_table_id = voice.doubao_boosting_table_id.trim();
    let websocket = voice.asr_provider == "doubao";
    if !valid_mobile_voice_endpoint(&fields.endpoint, websocket)
        || !bounded_resolved_voice_fields(&fields)
        || !crate::text::is_bounded_text(boosting_table_id, 4_096)
    {
        return None;
    }
    let headers = if voice.asr_provider == "doubao" {
        let resource_id = match voice.asr_resource_id.trim() {
            "" => "volc.seedasr.sauc.duration",
            value => value,
        };
        crate::credential::doubao_auth::headers(
            &voice.doubao_auth_mode,
            &voice.asr_app_key,
            &fields.token,
            resource_id,
        )?
        .into_iter()
        .map(|(name, value)| VoiceRequestHeader {
            name: name.into(),
            value,
        })
        .collect()
    } else {
        Vec::new()
    };
    Some(MobileVoiceProviderConfiguration {
        provider: voice.asr_provider.clone(),
        request_format: request_format.to_owned(),
        endpoint: fields.endpoint,
        model: fields.model,
        token: if voice.asr_provider == "doubao" {
            String::new()
        } else {
            fields.token
        },
        headers,
        enable_itn: voice.doubao_enable_itn,
        enable_punctuation: voice.doubao_enable_punc,
        enable_ddc: voice.doubao_enable_ddc,
        boosting_table_id: boosting_table_id.to_owned(),
        model_path: String::new(),
    })
}

/// On-device recognition: no endpoint, token or header applies, only the model the user installed or picked. `None` until a model is chosen, the same "not configured" answer a network provider without a token gets.
fn local_provider_configuration(
    preferences: &Preferences,
) -> Option<MobileVoiceProviderConfiguration> {
    let voice = &preferences.voice_input;
    let model_path = voice.asr_model_path.trim();
    if model_path.is_empty()
        || !crate::text::is_bounded_text(model_path, 4096)
        || !crate::preferences::is_absolute_model_path(model_path)
    {
        return None;
    }
    Some(MobileVoiceProviderConfiguration {
        provider: voice.asr_provider.clone(),
        request_format: ASR_REQUEST_LOCAL.to_owned(),
        endpoint: String::new(),
        model: String::new(),
        token: String::new(),
        headers: Vec::new(),
        enable_itn: voice.doubao_enable_itn,
        enable_punctuation: voice.doubao_enable_punc,
        enable_ddc: voice.doubao_enable_ddc,
        boosting_table_id: String::new(),
        model_path: model_path.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_provider_resolves_without_endpoint_or_token() {
        let mut preferences = Preferences::default();
        preferences.voice_input.asr_provider = "local".into();
        preferences.voice_input.asr_endpoint = "https://fixture.invalid/ignored".into();
        preferences.voice_input.asr_token = "synthetic-ignored".into();
        assert!(mobile_voice_provider_configuration(&preferences).is_none());

        preferences.voice_input.asr_model_path =
            "/data/user/0/app/files/voice-models/x-asr-zh-en-streaming".into();
        let configuration = mobile_voice_provider_configuration(&preferences).unwrap();
        assert_eq!(configuration.provider, "local");
        assert_eq!(configuration.request_format, ASR_REQUEST_LOCAL);
        assert_eq!(
            configuration.model_path,
            "/data/user/0/app/files/voice-models/x-asr-zh-en-streaming"
        );
        assert!(configuration.endpoint.is_empty());
        assert!(configuration.token.is_empty());
        assert!(configuration.headers.is_empty());

        preferences.voice_input.asr_model_path = "relative/model".into();
        assert!(mobile_voice_provider_configuration(&preferences).is_none());
    }

    #[test]
    fn network_providers_carry_no_model_path() {
        let mut preferences = Preferences::default();
        preferences.voice_input.asr_provider = "openai".into();
        preferences.voice_input.asr_token = "synthetic-token".into();
        preferences.voice_input.asr_endpoint =
            "https://fixture.invalid/v1/audio/transcriptions".into();
        preferences.voice_input.asr_model_path = "/models/x-asr-zh-en-streaming".into();
        let configuration = mobile_voice_provider_configuration(&preferences).unwrap();
        assert!(configuration.model_path.is_empty());
    }

    #[test]
    fn each_provider_names_its_request_format() {
        let mut preferences = Preferences::default();
        preferences.voice_input.asr_token = "synthetic-token".into();
        // 默认偏好里的地址是豆包的 wss；清空后每家用自己的默认地址。
        preferences.voice_input.asr_endpoint = String::new();
        for (provider, format) in [
            ("openai", ASR_REQUEST_MULTIPART),
            ("siliconflow", ASR_REQUEST_MULTIPART),
            ("groq", ASR_REQUEST_MULTIPART),
            ("everyapi", ASR_REQUEST_MULTIPART),
            ("mistral", ASR_REQUEST_MULTIPART),
            ("bailian", ASR_REQUEST_CHAT_AUDIO),
        ] {
            preferences.voice_input.asr_provider = provider.into();
            let configuration = mobile_voice_provider_configuration(&preferences)
                .unwrap_or_else(|| panic!("{provider} resolves"));
            assert_eq!(configuration.request_format, format, "{provider}");
            assert_eq!(asr_request_format(provider), Some(format));
        }
        assert_eq!(
            asr_request_format("doubao"),
            Some(ASR_REQUEST_DOUBAO_WEBSOCKET)
        );
        assert_eq!(asr_request_format("system"), None);
        assert_eq!(asr_request_format("whisper"), None);
    }

    #[test]
    fn bailian_resolves_to_the_dashscope_chat_endpoint() {
        let mut preferences = Preferences::default();
        preferences.voice_input.asr_provider = "bailian".into();
        preferences.voice_input.asr_endpoint = String::new();
        preferences.voice_input.asr_model = String::new();
        preferences
            .voice_input
            .asr_tokens
            .insert("bailian".into(), "synthetic-bailian-key".into());
        let configuration = mobile_voice_provider_configuration(&preferences).unwrap();
        assert_eq!(
            configuration.endpoint,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
        assert_eq!(configuration.model, "qwen3-asr-flash");
        assert_eq!(configuration.token, "synthetic-bailian-key");
        assert!(configuration.headers.is_empty());
    }

    #[test]
    fn chat_audio_body_carries_the_wav_as_a_data_url() {
        let body = chat_audio_request_body("qwen3-asr-flash", b"RIFF");
        assert_eq!(body["model"], "qwen3-asr-flash");
        assert_eq!(body["stream"], false);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        let content = &messages[0]["content"][0];
        assert_eq!(content["type"], "input_audio");
        assert_eq!(
            content["input_audio"]["data"],
            "data:audio/wav;base64,UklGRg=="
        );
    }

    #[test]
    fn network_voice_configuration_rejects_malformed_secure_endpoints() {
        let mut preferences = Preferences::default();
        preferences.voice_input.asr_provider = "openai".into();
        preferences.voice_input.asr_token = "synthetic-token".into();
        for endpoint in [
            "https:///v1/audio/transcriptions",
            "https://user:pass@fixture.invalid/v1/audio/transcriptions",
            "https://fixture.invalid/v1/audio/transcriptions#fragment",
        ] {
            preferences.voice_input.asr_endpoint = endpoint.into();
            assert!(
                mobile_voice_provider_configuration(&preferences).is_none(),
                "endpoint must be rejected: {endpoint}"
            );
        }
    }

    #[test]
    fn voice_polish_configuration_rejects_malformed_secure_endpoints() {
        let mut preferences = Preferences::default();
        preferences.voice_input.polish_enabled = true;
        preferences.voice_input.polish_provider = "openai".into();
        preferences.voice_input.polish_token = "synthetic-token".into();
        for endpoint in [
            "https:///v1/chat/completions",
            "https://user:pass@fixture.invalid/v1/chat/completions",
            "https://fixture.invalid/v1/chat/completions#fragment",
        ] {
            preferences.voice_input.polish_endpoint = endpoint.into();
            assert!(
                mobile_voice_polish_configuration(&preferences).is_none(),
                "endpoint must be rejected: {endpoint}"
            );
        }
    }

    #[test]
    fn mobile_voice_endpoint_validation_matches_transport_requirements() {
        for (endpoint, websocket) in [
            ("https://fixture.invalid/v1/audio/transcriptions", false),
            ("wss://fixture.invalid/asr", true),
        ] {
            assert!(valid_mobile_voice_endpoint(endpoint, websocket));
        }
        for (endpoint, websocket) in [
            ("https:///path", false),
            ("https://user:pass@fixture.invalid/path", false),
            ("https://fixture.invalid/path#fragment", false),
            ("wss:///path", true),
            ("wss://user:pass@fixture.invalid/path", true),
            ("wss://fixture.invalid/path#fragment", true),
        ] {
            assert!(!valid_mobile_voice_endpoint(endpoint, websocket));
        }
    }
}
