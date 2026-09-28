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

/// The transcription provider a mobile host should use, resolved from the settings document.
#[derive(Debug, PartialEq, Eq)]
pub struct MobileVoiceProviderConfiguration {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub token: String,
    pub headers: Vec<VoiceRequestHeader>,
    pub enable_itn: bool,
    pub enable_punctuation: bool,
    pub enable_ddc: bool,
    pub boosting_table_id: String,
    /// The on-device model for provider `local` (an installed model directory or a Whisper file); empty for every network provider, which carry an endpoint and token instead.
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
    pub prompt_legacy: String,
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
    if !fields.endpoint.starts_with("https://")
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
        prompt_legacy: voice.polish_prompt.clone(),
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
        _ => {
            return None;
        }
    };
    let fields = resolve_voice_fields(
        &voice.asr_provider,
        &voice.asr_endpoint,
        &voice.asr_model,
        &voice.asr_token,
        &voice.asr_tokens,
        (default_endpoint, default_model),
    );
    let boosting_table_id = voice.doubao_boosting_table_id.trim();
    if !bounded_resolved_voice_fields(&fields)
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
        preferences.voice_input.asr_model_path = "/models/ggml.bin".into();
        let configuration = mobile_voice_provider_configuration(&preferences).unwrap();
        assert!(configuration.model_path.is_empty());
    }
}
