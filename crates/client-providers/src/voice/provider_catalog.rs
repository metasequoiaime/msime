//! 设置页与语音运行时共用的服务目录。平台特有的系统识别和本地模型由宿主追加。

use serde::Serialize;

/// 只含公开预设；不读取偏好、凭据、文件或网络。模型列表仅作建议，用户自定义值仍可保存。
#[derive(Debug, Serialize)]
pub struct VoiceProviderPreset {
    pub id: &'static str,
    pub title: &'static str,
    pub endpoint: &'static str,
    pub model: &'static str,
    pub models: &'static [&'static str],
    pub documentation: &'static str,
}

/// 豆包流式识别的公开接口选项；短名称用于设置行，完整名称用于选择器。
#[derive(Debug, Serialize)]
pub struct DoubaoStreamEndpointPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub title: &'static str,
    pub endpoint: &'static str,
}

pub const DOUBAO_STREAM_ENDPOINTS: &[DoubaoStreamEndpointPreset] = &[
    DoubaoStreamEndpointPreset {
        id: "nostream",
        label: "整句流式",
        title: "整句流式（准确率更高）",
        endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream",
    },
    DoubaoStreamEndpointPreset {
        id: "async",
        label: "双向流式",
        title: "双向流式（增量结果）",
        endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
    },
];

/// 设置页的识别服务，顺序与现有下拉框一致。
pub const ASR_PROVIDERS: &[VoiceProviderPreset] = &[
    VoiceProviderPreset {
        id: "doubao",
        title: "豆包",
        endpoint: DOUBAO_STREAM_ENDPOINTS[1].endpoint,
        model: "",
        models: &[],
        documentation: "https://www.volcengine.com/docs/6561/1354869",
    },
    VoiceProviderPreset {
        id: "siliconflow",
        title: "SiliconFlow",
        endpoint: "https://api.siliconflow.cn/v1/audio/transcriptions",
        model: "FunAudioLLM/SenseVoiceSmall",
        models: &["FunAudioLLM/SenseVoiceSmall"],
        documentation: "https://siliconflow.readme.io/reference/createaudiotranscriptions",
    },
    VoiceProviderPreset {
        id: "openai",
        title: "OpenAI",
        endpoint: "https://api.openai.com/v1/audio/transcriptions",
        model: "whisper-1",
        models: &["gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"],
        documentation: "https://developers.openai.com/api/docs/guides/speech-to-text",
    },
    VoiceProviderPreset {
        id: "groq",
        title: "Groq",
        endpoint: "https://api.groq.com/openai/v1/audio/transcriptions",
        model: "whisper-large-v3-turbo",
        models: &["whisper-large-v3-turbo", "whisper-large-v3"],
        documentation: "https://console.groq.com/docs/speech-to-text",
    },
    VoiceProviderPreset {
        id: "everyapi",
        title: "EveryAPI",
        endpoint: "https://api.everyapi.ai/v1/audio/transcriptions",
        model: "volc.seedasr.sauc.duration",
        models: &["volc.seedasr.sauc.duration"],
        documentation: "https://everyapi.ai/models",
    },
    VoiceProviderPreset {
        id: "mistral",
        title: "Mistral · Voxtral",
        endpoint: "https://api.mistral.ai/v1/audio/transcriptions",
        model: "voxtral-mini-latest",
        models: &["voxtral-mini-latest"],
        documentation: "https://docs.mistral.ai/studio/audio/speech_to_text/offline_transcription",
    },
    VoiceProviderPreset {
        id: "bailian",
        title: "阿里云百炼 · 千问",
        endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        model: "qwen3-asr-flash",
        models: &["qwen3-asr-flash", "qwen3-asr-flash-2026-02-10"],
        documentation: "https://help.aliyun.com/zh/model-studio/qwen-speech-recognition",
    },
];

/// 设置页的润色服务，顺序与现有下拉框一致。
pub const POLISH_PROVIDERS: &[VoiceProviderPreset] = &[
    VoiceProviderPreset {
        id: "siliconflow",
        title: "SiliconFlow",
        endpoint: "https://api.siliconflow.cn/v1/chat/completions",
        model: "Qwen/Qwen3-8B",
        models: &["Qwen/Qwen3-8B", "Qwen/Qwen3.6-27B"],
        documentation: "https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation",
    },
    VoiceProviderPreset {
        id: "openai",
        title: "OpenAI",
        endpoint: "https://api.openai.com/v1/chat/completions",
        model: "gpt-4o-mini",
        models: &["gpt-4o-mini", "gpt-4.1-mini"],
        documentation: "https://developers.openai.com/api/docs/models/gpt-4.1-mini",
    },
    VoiceProviderPreset {
        id: "deepseek",
        title: "DeepSeek",
        endpoint: "https://api.deepseek.com/chat/completions",
        model: "deepseek-v4-flash",
        models: &["deepseek-v4-flash", "deepseek-v4-pro"],
        documentation: "https://api-docs.deepseek.com/",
    },
    VoiceProviderPreset {
        id: "groq",
        title: "Groq",
        endpoint: "https://api.groq.com/openai/v1/chat/completions",
        model: "llama-3.3-70b-versatile",
        models: &["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
        documentation: "https://console.groq.com/docs/quickstart",
    },
];
