//! AI 设置的公开服务目录和切换规则；不做 I/O，不处理凭据。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct AiProviderPreset {
    pub id: &'static str,
    pub title: &'static str,
    pub endpoint: &'static str,
    pub model: &'static str,
    pub models: &'static [&'static str],
    pub documentation: &'static str,
    pub desktop_only: bool,
}

/// 沿用共享设置页的公开预设快照；模型建议不限制用户的自定义值或服务返回的目录。
pub const PROVIDERS: &[AiProviderPreset] = &[
    AiProviderPreset {
        id: "everyapi",
        title: "EveryAPI",
        endpoint: "https://api.everyapi.ai/v1/chat/completions",
        model: "deepseek-v4-flash",
        documentation: "https://everyapi.ai/models",
        models: &[
            "deepseek-v4-flash",
            "deepseek-v4-pro",
            "claude-sonnet-5",
            "glm-5.3-flash",
        ],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "openai",
        title: "OpenAI",
        endpoint: "https://api.openai.com/v1/chat/completions",
        model: "gpt-4.1-mini",
        documentation: "https://developers.openai.com/api/docs/models/gpt-4.1-mini",
        models: &["gpt-4.1-mini"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "anthropic",
        title: "Anthropic · Claude",
        endpoint: "https://api.anthropic.com/v1/chat/completions",
        model: "claude-sonnet-4-6",
        documentation:
            "https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk",
        models: &["claude-sonnet-4-6", "claude-opus-5"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "gemini",
        title: "Google · Gemini",
        endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        model: "gemini-3.8-flash",
        documentation: "https://ai.google.dev/gemini-api/docs/openai",
        models: &["gemini-3.8-flash", "gemini-2.5-flash"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "deepseek",
        title: "DeepSeek",
        endpoint: "https://api.deepseek.com/chat/completions",
        model: "deepseek-v4-flash",
        documentation: "https://api-docs.deepseek.com/",
        models: &["deepseek-v4-flash", "deepseek-v4-pro"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "qwen",
        title: "通义千问 · 阿里云百炼",
        endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        model: "qwen-plus",
        documentation:
            "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope",
        models: &["qwen-plus"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "kimi",
        title: "Kimi · 月之暗面",
        endpoint: "https://api.moonshot.cn/v1/chat/completions",
        model: "kimi-k2.6",
        documentation: "https://platform.kimi.com/docs/api/chat",
        models: &["kimi-k2.6", "kimi-k2.5"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "zhipu",
        title: "智谱 · GLM",
        endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        model: "glm-4.7",
        documentation: "https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7",
        models: &["glm-4.7", "glm-4.7-flashx"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "minimax",
        title: "MiniMax",
        endpoint: "https://api.minimax.cn/v1/chat/completions",
        model: "MiniMax-M3.1-Flash",
        documentation: "https://platform.minimax.cn/docs/api-reference/text-chat-openai",
        models: &["MiniMax-M3.1-Flash", "MiniMax-M3"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "volcengine",
        title: "火山方舟 · 豆包",
        endpoint: "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
        model: "doubao-seed-1.6-flash",
        documentation: "https://ark.volcengine.com/region:cn-beijing/docs/chat-api",
        models: &["doubao-seed-1.6-flash", "doubao-seed-1.6"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "siliconflow",
        title: "硅基流动",
        endpoint: "https://api.siliconflow.cn/v1/chat/completions",
        model: "Qwen/Qwen3.6-27B",
        documentation: "https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation",
        models: &["Qwen/Qwen3.6-27B"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "groq",
        title: "Groq",
        endpoint: "https://api.groq.com/openai/v1/chat/completions",
        model: "llama-3.3-70b-versatile",
        documentation: "https://console.groq.com/docs/quickstart",
        models: &["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "openrouter",
        title: "OpenRouter",
        endpoint: "https://openrouter.ai/api/v1/chat/completions",
        model: "openrouter/auto",
        documentation: "https://openrouter.ai/docs/quickstart",
        models: &["openrouter/auto"],
        desktop_only: false,
    },
    AiProviderPreset {
        id: "ollama",
        title: "Ollama",
        endpoint: "http://localhost:11434/v1/chat/completions",
        model: "",
        documentation: "https://docs.ollama.com/api/openai-compatibility",
        models: &[],
        desktop_only: true,
    },
    AiProviderPreset {
        id: "custom",
        title: "自定义",
        endpoint: "",
        model: "",
        documentation: "",
        models: &[],
        desktop_only: false,
    },
];

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AiProviderFields {
    pub enabled: bool,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    /// 每个提供方记住的接口地址/模型槽位（与共享文档 `ai_assistant.endpoints`/`models` 同构），键为提供方 id；只存自定义值，缺键或空值表示用目录默认值。
    #[serde(default)]
    pub endpoints: BTreeMap<String, String>,
    #[serde(default)]
    pub models: BTreeMap<String, String>,
}

/// 换提供方：先把原提供方的接口地址/模型写回槽位，再从新提供方的槽位回填，没有槽位用目录默认值。补丁不包含凭据、提示词或其他偏好。
pub fn provider_update(provider: &str, current: &AiProviderFields) -> Option<AiProviderFields> {
    let next = PROVIDERS.iter().find(|entry| entry.id == provider)?;
    if !crate::text::is_bounded_text(&current.provider, 64)
        || !crate::text::is_bounded_text(&current.endpoint, 4096)
        || !crate::text::is_bounded_text(&current.model, 256)
    {
        return None;
    }
    let previous = PROVIDERS.iter().find(|entry| entry.id == current.provider);
    let mut endpoints = current.endpoints.clone();
    let mut models = current.models.clone();
    store_slot(
        &mut endpoints,
        &current.provider,
        &current.endpoint,
        previous.map(|entry| entry.endpoint),
    );
    store_slot(
        &mut models,
        &current.provider,
        &current.model,
        previous.map(|entry| entry.model),
    );
    let slot = |slots: &BTreeMap<String, String>| {
        slots
            .get(next.id)
            .filter(|value| !value.is_empty())
            .cloned()
    };
    let endpoint = slot(&endpoints).unwrap_or_else(|| next.endpoint.to_owned());
    let model = slot(&models).unwrap_or_else(|| next.model.to_owned());
    Some(AiProviderFields {
        enabled: current.enabled && super::endpoint::validate(&endpoint).is_ok(),
        provider: next.id.to_owned(),
        endpoint,
        model,
        endpoints,
        models,
    })
}

/// 槽位只记自定义值：与目录默认相同的值不存，默认值升级后空槽自动跟上（`ai-provider-update.ts` 的 `storeSlot`）。
fn store_slot(
    slots: &mut BTreeMap<String, String>,
    provider: &str,
    value: &str,
    fallback: Option<&str>,
) {
    let value = value.trim();
    if !provider.is_empty() && !value.is_empty() && fallback != Some(value) {
        slots.insert(provider.to_owned(), value.to_owned());
    } else {
        slots.remove(provider);
    }
}

/// 共享 UI 的静态投影；保留可选字段缺省形状，不改变 Android C ABI 的完整字段。
#[derive(Debug, Serialize)]
pub struct UiAiProviderPreset {
    pub id: &'static str,
    pub title: &'static str,
    pub endpoint: &'static str,
    pub model: &'static str,
    #[serde(skip_serializing_if = "<[&str]>::is_empty")]
    pub models: &'static [&'static str],
    #[serde(skip_serializing_if = "str::is_empty")]
    pub documentation: &'static str,
}

/// 元数据只维护在 `PROVIDERS`，平台差异通过投影表达。
pub fn ui_provider_catalog() -> Vec<UiAiProviderPreset> {
    PROVIDERS
        .iter()
        .map(|preset| UiAiProviderPreset {
            id: preset.id,
            title: preset.title,
            endpoint: preset.endpoint,
            model: preset.model,
            models: preset.models,
            documentation: preset.documentation,
        })
        .collect()
}
