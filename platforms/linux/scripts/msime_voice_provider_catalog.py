# 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。
DEFAULT_MODELS = {
  "asr": {
    "bailian": "qwen3-asr-flash",
    "custom": "",
    "doubao": "",
    "everyapi": "volc.seedasr.sauc.duration",
    "groq": "whisper-large-v3-turbo",
    "minimax": "asr-1.0",
    "mistral": "voxtral-mini-latest",
    "openai": "whisper-1",
    "siliconflow": "FunAudioLLM/SenseVoiceSmall",
    "zhipu": "glm-asr"
  },
  "polish": {
    "custom": "",
    "deepseek": "deepseek-v4-flash",
    "groq": "llama-3.3-70b-versatile",
    "kimi": "kimi-k2-0905-preview",
    "minimax": "MiniMax-M3.1-Flash",
    "openai": "gpt-4o-mini",
    "openrouter": "openrouter/auto",
    "qwen": "qwen-plus",
    "siliconflow": "Qwen/Qwen3-8B",
    "volcengine": "doubao-seed-1.6-flash",
    "zhipu": "glm-4-flash"
  }
}
DEFAULT_ENDPOINTS = {
  "asr": {
    "bailian": "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
    "custom": "",
    "doubao": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
    "everyapi": "https://api.everyapi.ai/v1/audio/transcriptions",
    "groq": "https://api.groq.com/openai/v1/audio/transcriptions",
    "minimax": "https://api.minimax.cn/v1/speech_to_text",
    "mistral": "https://api.mistral.ai/v1/audio/transcriptions",
    "openai": "https://api.openai.com/v1/audio/transcriptions",
    "siliconflow": "https://api.siliconflow.cn/v1/audio/transcriptions",
    "zhipu": "https://open.bigmodel.cn/api/paas/v4/audio/transcriptions"
  },
  "polish": {
    "custom": "",
    "deepseek": "https://api.deepseek.com/chat/completions",
    "groq": "https://api.groq.com/openai/v1/chat/completions",
    "kimi": "https://api.moonshot.cn/v1/chat/completions",
    "minimax": "https://api.minimax.cn/v1/chat/completions",
    "openai": "https://api.openai.com/v1/chat/completions",
    "openrouter": "https://openrouter.ai/api/v1/chat/completions",
    "qwen": "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
    "siliconflow": "https://api.siliconflow.cn/v1/chat/completions",
    "volcengine": "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
    "zhipu": "https://open.bigmodel.cn/api/paas/v4/chat/completions"
  }
}
REQUEST_FORMATS = {
  "bailian": "chat_audio",
  "custom": "multipart",
  "everyapi": "multipart",
  "groq": "multipart",
  "minimax": "multipart",
  "mistral": "multipart",
  "openai": "multipart",
  "siliconflow": "multipart",
  "zhipu": "multipart"
}
ASR_PROVIDERS = set(DEFAULT_MODELS["asr"])
POLISH_PROVIDERS = set(DEFAULT_MODELS["polish"])
