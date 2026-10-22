# 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。
DEFAULT_MODELS = {
  "asr": {
    "bailian": "qwen3-asr-flash",
    "doubao": "",
    "everyapi": "volc.seedasr.sauc.duration",
    "groq": "whisper-large-v3-turbo",
    "mistral": "voxtral-mini-latest",
    "openai": "whisper-1",
    "siliconflow": "FunAudioLLM/SenseVoiceSmall"
  },
  "polish": {
    "deepseek": "deepseek-v4-flash",
    "groq": "llama-3.3-70b-versatile",
    "openai": "gpt-4o-mini",
    "siliconflow": "Qwen/Qwen3-8B"
  }
}
DEFAULT_ENDPOINTS = {
  "asr": {
    "bailian": "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
    "doubao": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
    "everyapi": "https://api.everyapi.ai/v1/audio/transcriptions",
    "groq": "https://api.groq.com/openai/v1/audio/transcriptions",
    "mistral": "https://api.mistral.ai/v1/audio/transcriptions",
    "openai": "https://api.openai.com/v1/audio/transcriptions",
    "siliconflow": "https://api.siliconflow.cn/v1/audio/transcriptions"
  },
  "polish": {
    "deepseek": "https://api.deepseek.com/chat/completions",
    "groq": "https://api.groq.com/openai/v1/chat/completions",
    "openai": "https://api.openai.com/v1/chat/completions",
    "siliconflow": "https://api.siliconflow.cn/v1/chat/completions"
  }
}
REQUEST_FORMATS = {
  "bailian": "chat_audio",
  "everyapi": "multipart",
  "groq": "multipart",
  "mistral": "multipart",
  "openai": "multipart",
  "siliconflow": "multipart"
}
ASR_PROVIDERS = set(DEFAULT_MODELS["asr"])
POLISH_PROVIDERS = set(DEFAULT_MODELS["polish"])
