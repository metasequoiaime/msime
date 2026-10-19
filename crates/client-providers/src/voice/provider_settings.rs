//! 设置页切换语音服务时的字段补丁；不读写偏好、不联网，也不记录凭据。

use super::provider_catalog::{VoiceProviderPreset, ASR_PROVIDERS, POLISH_PROVIDERS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Deserialize)]
pub enum ProviderService {
    #[serde(rename = "voice.asr")]
    Asr,
    #[serde(rename = "voice.polish")]
    Polish,
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

/// 保存原服务的令牌并恢复新服务的分槽，只替换空白或出厂默认的地址和模型。
pub fn provider_update(
    service: ProviderService,
    provider: &str,
    current: &ProviderFields,
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
    ProviderUpdate {
        provider: provider.to_owned(),
        token,
        tokens,
        endpoint: defaults.and_then(|(endpoint, _)| {
            fill_if_default(&current.endpoint, endpoint, table, |preset| preset.endpoint)
        }),
        model: defaults.and_then(|(_, model)| {
            fill_if_default(&current.model, model, table, |preset| preset.model)
        }),
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
