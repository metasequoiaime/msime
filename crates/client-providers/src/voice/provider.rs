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
        "openai" | "siliconflow" | "groq" | "everyapi" | "mistral" | "zhipu" | "minimax"
        | "custom" => Some(ASR_REQUEST_MULTIPART),
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

/// 按服务配置解析的可选转写润色；未启用时返回 `None`，不视为错误。
///
/// 这里只传递方案与自定义槽位，不解析正文。内置正文归 `polish_catalog` 所有，
/// 宿主通过静态导出的 `shared/voice/PolishPromptCatalog.h` 读取同一份内容。
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

/// 设置页保存凭据前的共享预检。空白值表示删除凭据，因此允许；非空值必须是单行、有界文本。
/// 豆包凭据会进入 WebSocket 握手头，必须保持 ASCII，其他服务允许 Unicode。
pub fn valid_voice_credential(provider: &str, value: &str) -> bool {
    let value = value.trim();
    value.is_empty()
        || (crate::text::is_bounded_text(value, 16 * 1024)
            && (provider != "doubao" || value.is_ascii()))
}

/// Android 语音设置需要的服务分类和鉴权模式；不读取偏好、凭据、文件或网络。
#[derive(Debug, PartialEq, Eq)]
pub struct MobileVoiceProviderPolicy {
    pub network: bool,
    pub legacy_auth: bool,
}

/// 统一计算 Android 设置页仍需展示的两个服务策略。
pub fn mobile_voice_provider_policy(provider: &str, auth_mode: &str) -> MobileVoiceProviderPolicy {
    MobileVoiceProviderPolicy {
        network: !provider.is_empty() && provider != "system" && provider != "local",
        legacy_auth: auth_mode.trim() == "legacy",
    }
}

/// 设置页识别服务行的状态文案；`resolved` 表示共享解析已经得到可用云端配置。
pub fn mobile_voice_provider_status(
    provider: &str,
    resolved: bool,
    platform_available: bool,
) -> Option<&'static str> {
    let network = mobile_voice_provider_policy(provider, "").network;
    if !network {
        return if provider.is_empty() || provider == "system" {
            Some(if platform_available {
                "使用设备自带的语音识别服务"
            } else {
                "没有可用的系统识别服务，可改用本地模型或豆包"
            })
        } else {
            None
        };
    }
    if resolved {
        return None;
    }
    Some(if provider == "doubao" {
        if platform_available {
            "密钥未填写完整，暂用系统识别"
        } else {
            "密钥未填写完整，系统识别不可用"
        }
    } else if platform_available {
        "未填写 API Key，暂用系统识别"
    } else {
        "未填写 API Key，系统识别不可用"
    })
}

/// 设置页本地模型行的状态文案。
pub fn mobile_voice_local_status(model_ready: bool, runtime_ready: bool) -> Option<&'static str> {
    if !model_ready {
        Some("还没有选用可用模型，请在下方「本地模型」选用、下载或导入")
    } else if runtime_ready {
        None
    } else {
        Some("还需准备本地语音运行库，请在下方「本地识别准备」下载或导入")
    }
}

/// 设置页润色行的状态文案；configured 表示共享解析已经得到可用润色配置。
pub fn mobile_voice_polish_status(
    enabled: bool,
    configured: bool,
    has_token: bool,
) -> Option<&'static str> {
    if !enabled || configured {
        return None;
    }
    Some(if has_token {
        "配置不完整，识别结果不会润色"
    } else {
        "未填写 API Key，识别结果不会润色"
    })
}

/// 在交给原生宿主前统一校验移动语音传输使用的 URL 形状。
/// 凭据不能嵌在 authority 中；缺少 authority 的地址若只检查 scheme 前缀会被错误放行，
/// 随后又被平台传输层拒绝。
pub fn valid_mobile_voice_endpoint(endpoint: &str, websocket: bool) -> bool {
    if !crate::text::is_bounded_text(endpoint, 2_048) {
        return false;
    }
    let expected_scheme = if websocket { "wss" } else { "https" };
    reqwest::Url::parse(endpoint)
        .ok()
        .is_some_and(|url| crate::endpoint::valid_endpoint_shape(endpoint, &url, expected_scheme))
}

/// Android 流式 WebSocket 传输消费的唯一 URL 解释结果。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct VoiceWebSocketEndpoint {
    /// 传给普通 socket 和 TLS 连接器的主机文本，不带 IPv6 方括号。
    pub socket_host: String,
    /// 地址中的端口；未写端口时为安全 WebSocket 默认端口 443。
    pub port: u16,
    /// Host 头的值，包含 IPv6 方括号和非默认端口。
    pub host_header: String,
    /// 已百分号编码的路径和查询串，保留显式空查询。
    pub request_target: String,
}

/// 为所有原生传输层统一解析并序列化安全流式地址。
pub fn websocket_endpoint_descriptor(endpoint: &str) -> Option<VoiceWebSocketEndpoint> {
    if !valid_mobile_voice_endpoint(endpoint, true) {
        return None;
    }
    let url = reqwest::Url::parse(endpoint).ok()?;
    let host = url.host_str()?.to_owned();
    let socket_host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(&host)
        .to_owned();
    if socket_host.is_empty() {
        return None;
    }
    let port = url.port_or_known_default()?;
    let host_header = if socket_host.contains(':') {
        if port == 443 {
            format!("[{socket_host}]")
        } else {
            format!("[{socket_host}]:{port}")
        }
    } else if port == 443 {
        socket_host.clone()
    } else {
        format!("{socket_host}:{port}")
    };
    let mut request_target = url.path().to_owned();
    if let Some(query) = url.query() {
        request_target.push('?');
        request_target.push_str(query);
    }
    Some(VoiceWebSocketEndpoint {
        socket_host,
        port,
        host_header,
        request_target,
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

/// 各润色服务的默认接口与模型；设置目录和运行时共用预设，旧配置中的 EveryAPI、Mistral 仍可解析。
pub fn polish_provider_defaults(provider: &str) -> Option<(&'static str, &'static str)> {
    if let Some(preset) = super::provider_catalog::POLISH_PROVIDERS
        .iter()
        .find(|preset| preset.id == provider)
    {
        return Some((preset.endpoint, preset.model));
    }
    // dashscope/moonshot 是 Windows 目录里的旧 id，对应目录中的 qwen/kimi；
    // 与 everyapi/mistral 一样只解析不进目录。
    Some(match provider {
        "everyapi" => (
            "https://api.everyapi.ai/v1/chat/completions",
            "openai/gpt-4o-mini",
        ),
        "mistral" => (
            "https://api.mistral.ai/v1/chat/completions",
            "mistral-small-latest",
        ),
        "dashscope" => (
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "qwen-plus",
        ),
        "moonshot" => (
            "https://api.moonshot.cn/v1/chat/completions",
            "kimi-k2-0905-preview",
        ),
        _ => return None,
    })
}

/// 各联网识别服务的默认接口与模型；系统识别、本地识别和不认识的服务为 `None`。
pub fn asr_provider_defaults(provider: &str) -> Option<(&'static str, &'static str)> {
    super::provider_catalog::ASR_PROVIDERS
        .iter()
        .find(|preset| preset.id == provider)
        .map(|preset| (preset.endpoint, preset.model))
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
    let (default_endpoint, default_model) = polish_provider_defaults(&voice.polish_provider)?;
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
    let (default_endpoint, default_model) = asr_provider_defaults(&voice.asr_provider)?;
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

/// 设置页「测试」的配置，交给 `credential::test_service`：服务、接口、模型和密钥按键盘同样的规则从偏好解析（空的接口和模型补该服务的默认值，空的密钥取该服务的分槽），所以测的就是键盘会用的那一份。`service` 是 `voice.asr` 或 `voice.polish`；系统识别、本地识别、不认识的服务或其他 `service` 为 `None`。不检查完整性：缺密钥、地址不对由测试本身离线回答。
pub fn credential_test_config(
    preferences: &Preferences,
    service: &str,
) -> Option<serde_json::Value> {
    let voice = &preferences.voice_input;
    match service {
        "voice.asr" => {
            let defaults = asr_provider_defaults(&voice.asr_provider)?;
            let fields = resolve_voice_fields(
                &voice.asr_provider,
                &voice.asr_endpoint,
                &voice.asr_model,
                &voice.asr_token,
                &voice.asr_tokens,
                defaults,
            );
            let mut config = serde_json::json!({
                "provider": voice.asr_provider,
                "endpoint": fields.endpoint,
                "model": fields.model,
                "token": fields.token,
            });
            if voice.asr_provider == "doubao" {
                let legacy = voice.doubao_auth_mode.trim() == "legacy";
                let resource_id = match voice.asr_resource_id.trim() {
                    "" => "volc.seedasr.sauc.duration",
                    value => value,
                };
                let extra = serde_json::json!({
                    "auth_mode": if legacy { "legacy" } else { "api_key" },
                    "app_id": if legacy { voice.asr_app_key.trim() } else { "" },
                    "resource_id": resource_id,
                    "doubao_enable_itn": voice.doubao_enable_itn,
                    "doubao_enable_punc": voice.doubao_enable_punc,
                    "doubao_enable_ddc": voice.doubao_enable_ddc,
                    "doubao_boosting_table_id": voice.doubao_boosting_table_id.trim(),
                });
                if let (Some(config), Some(extra)) = (config.as_object_mut(), extra.as_object()) {
                    config.extend(extra.clone());
                }
            }
            Some(config)
        }
        "voice.polish" => {
            let defaults = polish_provider_defaults(&voice.polish_provider)?;
            let fields = resolve_voice_fields(
                &voice.polish_provider,
                &voice.polish_endpoint,
                &voice.polish_model,
                &voice.polish_token,
                &voice.polish_tokens,
                defaults,
            );
            Some(serde_json::json!({
                "provider": voice.polish_provider,
                "endpoint": fields.endpoint,
                "model": fields.model,
                "token": fields.token,
            }))
        }
        _ => None,
    }
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
    fn the_retired_everyapi_asr_model_falls_back_to_the_current_default_on_load() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::preferences::PreferencesStore::new(directory.path());
        let load_with = |provider: &str, model: &str| {
            let snapshot = store.load().unwrap();
            let mut preferences = snapshot.preferences;
            preferences.voice_input.asr_provider = provider.into();
            preferences.voice_input.asr_model = model.into();
            store.save(snapshot.revision, preferences).unwrap();
            store.load().unwrap().preferences.voice_input.asr_model
        };
        // EveryAPI never served this model; an empty field lets the resolver use the current default.
        assert_eq!(load_with("everyapi", "openai/whisper-large-v3-turbo"), "");
        assert_eq!(load_with("everyapi", " openai/whisper-large-v3-turbo "), "");
        let resolved = mobile_voice_provider_configuration(&{
            let mut preferences = store.load().unwrap().preferences;
            preferences.voice_input.asr_token = "fixture-token".into();
            preferences.voice_input.asr_endpoint.clear();
            preferences
        })
        .unwrap();
        assert_eq!(resolved.model, "volc.seedasr.sauc.duration");
        // A model the user chose, or the same name under another provider, is left alone.
        assert_eq!(
            load_with("everyapi", "volc.seedasr.sauc.duration"),
            "volc.seedasr.sauc.duration"
        );
        assert_eq!(load_with("everyapi", "custom-model"), "custom-model");
        assert_eq!(
            load_with("groq", "openai/whisper-large-v3-turbo"),
            "openai/whisper-large-v3-turbo"
        );
    }

    #[test]
    fn credential_test_config_resolves_what_the_keyboard_would_send() {
        let mut preferences = Preferences::default();
        preferences.voice_input.asr_provider = "doubao".into();
        preferences.voice_input.asr_endpoint.clear();
        preferences.voice_input.asr_resource_id.clear();
        preferences.voice_input.asr_token.clear();
        preferences
            .voice_input
            .asr_tokens
            .insert("doubao".into(), "slot-token".into());
        preferences.voice_input.doubao_auth_mode = "legacy".into();
        preferences.voice_input.asr_app_key = " app ".into();
        let asr = credential_test_config(&preferences, "voice.asr").unwrap();
        assert_eq!(
            asr["endpoint"],
            "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async"
        );
        assert_eq!(asr["token"], "slot-token");
        assert_eq!(asr["auth_mode"], "legacy");
        assert_eq!(asr["app_id"], "app");
        assert_eq!(asr["resource_id"], "volc.seedasr.sauc.duration");

        preferences.voice_input.doubao_auth_mode = "api_key".into();
        let api_key = credential_test_config(&preferences, "voice.asr").unwrap();
        assert_eq!(
            api_key["app_id"], "",
            "a stale App ID is not sent in API-key mode"
        );

        preferences.voice_input.polish_provider = "groq".into();
        // The shipped default document carries another service's endpoint on some hosts; an empty one is what a provider switch leaves.
        preferences.voice_input.polish_endpoint.clear();
        preferences.voice_input.polish_model = " ".into();
        let polish = credential_test_config(&preferences, "voice.polish").unwrap();
        assert_eq!(
            polish["endpoint"],
            "https://api.groq.com/openai/v1/chat/completions"
        );
        assert_eq!(polish["model"], "llama-3.3-70b-versatile");

        for provider in ["system", "local", "unknown"] {
            preferences.voice_input.asr_provider = provider.into();
            assert!(credential_test_config(&preferences, "voice.asr").is_none());
        }
        assert!(credential_test_config(&preferences, "ai.assistant").is_none());
    }

    #[test]
    fn voice_credential_preflight_matches_the_mobile_contract() {
        assert!(valid_voice_credential("openai", ""));
        assert!(valid_voice_credential("openai", "  "));
        assert!(valid_voice_credential("openai", "密钥"));
        assert!(!valid_voice_credential("openai", "key\nvalue"));
        assert!(!valid_voice_credential(
            "openai",
            &"x".repeat(16 * 1024 + 1)
        ));
        assert!(valid_voice_credential("doubao", "ascii-key"));
        assert!(!valid_voice_credential("doubao", "密钥"));
    }

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

    #[test]
    fn websocket_endpoint_descriptor_uses_one_serialized_url_representation() {
        let descriptor =
            websocket_endpoint_descriptor("wss://例子.invalid:8443/合成🍃/%2F?lang=zh+cn&empty=")
                .expect("valid websocket endpoint");
        assert_eq!(descriptor.socket_host, "xn--fsqu00a.invalid");
        assert_eq!(descriptor.port, 8443);
        assert_eq!(descriptor.host_header, "xn--fsqu00a.invalid:8443");
        assert_eq!(
            descriptor.request_target,
            "/%E5%90%88%E6%88%90%F0%9F%8D%83/%2F?lang=zh+cn&empty="
        );
    }

    #[test]
    fn websocket_endpoint_descriptor_formats_ipv6_and_default_port() {
        let descriptor =
            websocket_endpoint_descriptor("wss://[::1]/stream?").expect("valid websocket endpoint");
        assert_eq!(descriptor.socket_host, "::1");
        assert_eq!(descriptor.port, 443);
        assert_eq!(descriptor.host_header, "[::1]");
        assert_eq!(descriptor.request_target, "/stream?");
    }

    #[test]
    fn websocket_endpoint_descriptor_matches_shared_validation_and_url_serialization() {
        for endpoint in [
            "wss://fixture.invalid:0/stream",
            "wss://fixture.invalid/%zz?value=%",
        ] {
            assert!(valid_mobile_voice_endpoint(endpoint, true));
            let url = reqwest::Url::parse(endpoint).unwrap();
            let descriptor = websocket_endpoint_descriptor(endpoint).unwrap();
            assert_eq!(descriptor.port, url.port_or_known_default().unwrap());
            let expected = if let Some(query) = url.query() {
                format!("{}?{query}", url.path())
            } else {
                url.path().to_owned()
            };
            assert_eq!(descriptor.request_target, expected);
        }
    }

    #[test]
    fn websocket_endpoint_descriptor_rejects_invalid_ports_and_controls() {
        for endpoint in [
            "wss://fixture.invalid:65536/stream",
            "wss://fixture.invalid/stream\nX: y",
        ] {
            assert!(
                websocket_endpoint_descriptor(endpoint).is_none(),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn mobile_voice_status_explains_fallback_and_platform_availability() {
        assert_eq!(
            mobile_voice_provider_status("doubao", false, true),
            Some("密钥未填写完整，暂用系统识别")
        );
        assert_eq!(
            mobile_voice_provider_status("openai", false, false),
            Some("未填写 API Key，系统识别不可用")
        );
        assert_eq!(mobile_voice_provider_status("openai", true, true), None);
        assert_eq!(
            mobile_voice_provider_status("system", false, true),
            Some("使用设备自带的语音识别服务")
        );
        assert_eq!(
            mobile_voice_provider_status("system", false, false),
            Some("没有可用的系统识别服务，可改用本地模型或豆包")
        );
        assert_eq!(mobile_voice_provider_status("local", true, true), None);
    }

    #[test]
    fn mobile_voice_policy_matches_android_provider_rules() {
        let cloud = mobile_voice_provider_policy("openai", " legacy ");
        assert!(cloud.network);
        assert!(cloud.legacy_auth);

        let offline = mobile_voice_provider_policy("local", "api_key");
        assert!(!offline.network);
        assert!(!offline.legacy_auth);

        let system = mobile_voice_provider_policy("system", "");
        assert!(!system.network);
        assert!(!system.legacy_auth);
    }

    #[test]
    fn mobile_voice_polish_status_explains_missing_configuration() {
        assert_eq!(mobile_voice_polish_status(false, false, false), None);
        assert_eq!(mobile_voice_polish_status(true, true, true), None);
        assert_eq!(
            mobile_voice_polish_status(true, false, false),
            Some("未填写 API Key，识别结果不会润色")
        );
        assert_eq!(
            mobile_voice_polish_status(true, false, true),
            Some("配置不完整，识别结果不会润色")
        );
    }

    #[test]
    fn mobile_voice_local_status_prioritizes_model_then_runtime() {
        assert_eq!(
            mobile_voice_local_status(false, true),
            Some("还没有选用可用模型，请在下方「本地模型」选用、下载或导入")
        );
        assert_eq!(
            mobile_voice_local_status(true, false),
            Some("还需准备本地语音运行库，请在下方「本地识别准备」下载或导入")
        );
        assert_eq!(mobile_voice_local_status(true, true), None);
    }
}
