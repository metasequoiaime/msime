//! 设置页共用的公开 AI 服务预设；只供展示和填入默认值，不覆盖用户配置。

use serde::Serialize;

/// AI 候选与语音润色使用各自的模型策略，不因字段相同而共用目录。
#[derive(Debug, Serialize)]
pub struct AiProviderPreset {
    pub id: &'static str,
    pub title: &'static str,
    pub endpoint: &'static str,
    pub model: &'static str,
    #[serde(skip_serializing_if = "<[&str]>::is_empty")]
    pub models: &'static [&'static str],
    #[serde(skip_serializing_if = "str::is_empty")]
    pub documentation: &'static str,
}

/// 保持共享 UI 与 Windows 的历史顺序；最后的自定义项也作为未知服务的回退。
pub const AI_PROVIDER_PRESETS: &[AiProviderPreset] = &[
    AiProviderPreset {
        id: "everyapi",
        title: "EveryAPI",
        endpoint: "https://api.everyapi.ai/v1/chat/completions",
        model: "deepseek-v4-flash",
        models: &[
            "deepseek-v4-flash",
            "deepseek-v4-pro",
            "claude-sonnet-5",
            "glm-5.3-flash",
        ],
        documentation: "https://everyapi.ai/models",
    },
    AiProviderPreset {
        id: "openai",
        title: "OpenAI",
        endpoint: "https://api.openai.com/v1/chat/completions",
        model: "gpt-4.1-mini",
        models: &["gpt-4.1-mini"],
        documentation: "https://developers.openai.com/api/docs/models/gpt-4.1-mini",
    },
    AiProviderPreset {
        id: "anthropic",
        title: "Anthropic · Claude",
        endpoint: "https://api.anthropic.com/v1/chat/completions",
        model: "claude-sonnet-4-6",
        models: &["claude-sonnet-4-6", "claude-opus-5"],
        documentation:
            "https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk",
    },
    AiProviderPreset {
        id: "gemini",
        title: "Google · Gemini",
        endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        model: "gemini-3.8-flash",
        models: &["gemini-3.8-flash", "gemini-2.5-flash"],
        documentation: "https://ai.google.dev/gemini-api/docs/openai",
    },
    AiProviderPreset {
        id: "deepseek",
        title: "DeepSeek",
        endpoint: "https://api.deepseek.com/chat/completions",
        model: "deepseek-v4-flash",
        models: &["deepseek-v4-flash", "deepseek-v4-pro"],
        documentation: "https://api-docs.deepseek.com/",
    },
    AiProviderPreset {
        id: "qwen",
        title: "通义千问 · 阿里云百炼",
        endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        model: "qwen-plus",
        models: &["qwen-plus"],
        documentation:
            "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope",
    },
    AiProviderPreset {
        id: "kimi",
        title: "Kimi · 月之暗面",
        endpoint: "https://api.moonshot.cn/v1/chat/completions",
        model: "kimi-k2.6",
        models: &["kimi-k2.6", "kimi-k2.5"],
        documentation: "https://platform.kimi.com/docs/api/chat",
    },
    AiProviderPreset {
        id: "zhipu",
        title: "智谱 · GLM",
        endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        model: "glm-4.7",
        models: &["glm-4.7", "glm-4.7-flashx"],
        documentation: "https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7",
    },
    AiProviderPreset {
        id: "siliconflow",
        title: "硅基流动",
        endpoint: "https://api.siliconflow.cn/v1/chat/completions",
        model: "Qwen/Qwen3.6-27B",
        models: &["Qwen/Qwen3.6-27B"],
        documentation: "https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation",
    },
    AiProviderPreset {
        id: "groq",
        title: "Groq",
        endpoint: "https://api.groq.com/openai/v1/chat/completions",
        model: "llama-3.3-70b-versatile",
        models: &["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
        documentation: "https://console.groq.com/docs/quickstart",
    },
    AiProviderPreset {
        id: "openrouter",
        title: "OpenRouter",
        endpoint: "https://openrouter.ai/api/v1/chat/completions",
        model: "openrouter/auto",
        models: &["openrouter/auto"],
        documentation: "https://openrouter.ai/docs/quickstart",
    },
    AiProviderPreset {
        id: "ollama",
        title: "Ollama",
        endpoint: "http://localhost:11434/v1/chat/completions",
        model: "",
        models: &[],
        documentation: "https://docs.ollama.com/api/openai-compatibility",
    },
    AiProviderPreset {
        id: "custom",
        title: "自定义",
        endpoint: "",
        model: "",
        models: &[],
        documentation: "",
    },
];
