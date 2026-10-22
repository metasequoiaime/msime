//! AI 设置的公开服务目录和切换规则；不做 I/O，不处理凭据。

use serde::{Deserialize, Serialize};

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
}

/// 只替换空值或原预设默认值；补丁不包含凭据、提示词或其他偏好。
pub fn provider_update(provider: &str, current: &AiProviderFields) -> Option<AiProviderFields> {
    let next = PROVIDERS.iter().find(|entry| entry.id == provider)?;
    if !crate::text::is_bounded_text(&current.provider, 64)
        || !crate::text::is_bounded_text(&current.endpoint, 4096)
        || !crate::text::is_bounded_text(&current.model, 256)
    {
        return None;
    }
    let previous = PROVIDERS.iter().find(|entry| entry.id == current.provider);
    let endpoint = replace_default(
        &current.endpoint,
        previous.map(|entry| entry.endpoint),
        next.endpoint,
    );
    let model = replace_default(
        &current.model,
        previous.map(|entry| entry.model),
        next.model,
    );
    Some(AiProviderFields {
        enabled: current.enabled && super::endpoint::validate(&endpoint).is_ok(),
        provider: next.id.to_owned(),
        endpoint,
        model,
    })
}

fn replace_default(current: &str, previous: Option<&str>, next: &str) -> String {
    if current.trim().is_empty() || previous == Some(current) {
        next.to_owned()
    } else {
        current.to_owned()
    }
}
