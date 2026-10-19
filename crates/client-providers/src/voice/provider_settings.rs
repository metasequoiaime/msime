//! 语音设置页的接口预检与换服务字段补丁；不读写偏好、不联网，也不记录凭据。

use super::provider_catalog::{VoiceProviderPreset, ASR_PROVIDERS, POLISH_PROVIDERS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 设置页允许留空使用默认地址；复用运行时 URL 解析，并保留移动宿主的协议和字符限制。
pub fn endpoint_issue(endpoint: &str, websocket: bool) -> Option<&'static str> {
    let endpoint = endpoint.trim();
    let prefix = if websocket { "wss://" } else { "https://" };
    if endpoint.is_empty()
        || (endpoint.starts_with(prefix)
            && !endpoint.contains('@')
            && super::provider::valid_mobile_voice_endpoint(endpoint, websocket))
    {
        return None;
    }
    Some(if websocket {
        "豆包的接口地址要以 wss:// 开头"
    } else {
        "接口地址要以 https:// 开头，且不能带用户名或密码"
    })
}

#[derive(Clone, Copy, Deserialize)]
pub enum ProviderService {
    #[serde(rename = "voice.asr")]
    Asr,
    #[serde(rename = "voice.polish")]
    Polish,
}

/// 默认替换出厂值并保留自定义字段；清空策略用于不沿用旧服务地址的宿主。
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFieldPolicy {
    #[default]
    FillDefaults,
    Clear,
}

/// 切换前的编辑值；字段名不带 asr/polish 前缀，由宿主映射对应设置。
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct ProviderFields {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub token: String,
    pub tokens: BTreeMap<String, String>,
}

/// 只包含需要写回的字段；省略 endpoint/model 表示保留用户原值，空串表示清空。
#[derive(Serialize)]
pub struct ProviderUpdate {
    pub provider: String,
    pub token: String,
    pub tokens: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'static str>,
}

/// 保存原服务的令牌并恢复新服务的分槽，按显式策略生成地址和模型补丁。
pub fn provider_update(
    service: ProviderService,
    provider: &str,
    current: &ProviderFields,
    field_policy: ProviderFieldPolicy,
) -> ProviderUpdate {
    let mut tokens = current.tokens.clone();
    if !current.provider.is_empty() {
        if current.token.is_empty() {
            tokens.remove(&current.provider);
        } else {
            tokens.insert(current.provider.clone(), current.token.clone());
        }
    }
    let offline = matches!(service, ProviderService::Asr) && matches!(provider, "system" | "local");
    if offline {
        tokens.remove(provider);
    }
    let token = if offline {
        String::new()
    } else {
        tokens.get(provider).cloned().unwrap_or_default()
    };
    let table = match service {
        ProviderService::Asr => ASR_PROVIDERS,
        ProviderService::Polish => POLISH_PROVIDERS,
    };
    let defaults = if offline {
        Some(("", ""))
    } else {
        table
            .iter()
            .find(|preset| preset.id == provider)
            .map(|preset| (preset.endpoint, preset.model))
    };
    let (endpoint, model) = match field_policy {
        ProviderFieldPolicy::Clear if provider != current.provider => (Some(""), Some("")),
        ProviderFieldPolicy::Clear => (None, None),
        ProviderFieldPolicy::FillDefaults => (
            defaults.and_then(|(endpoint, _)| {
                fill_if_default(&current.endpoint, endpoint, table, |preset| preset.endpoint)
            }),
            defaults.and_then(|(_, model)| {
                fill_if_default(&current.model, model, table, |preset| preset.model)
            }),
        ),
    };
    ProviderUpdate {
        provider: provider.to_owned(),
        token,
        tokens,
        endpoint,
        model,
    }
}

fn fill_if_default(
    current: &str,
    next: &'static str,
    table: &[VoiceProviderPreset],
    field: impl Fn(&VoiceProviderPreset) -> &'static str,
) -> Option<&'static str> {
    let value = msime_client_base::text::trim_ui_whitespace(current);
    (value.is_empty()
        || table
            .iter()
            .any(|preset| !field(preset).is_empty() && field(preset) == value))
    .then_some(next)
}
