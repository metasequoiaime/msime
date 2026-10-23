#pragma once
// 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。
#include <array>
#include <string_view>

namespace msime::voice {
struct VoiceProviderCatalogEntry {
  std::string_view id;
  std::string_view title;
  std::string_view endpoint;
  std::string_view model;
};
inline constexpr std::array<VoiceProviderCatalogEntry, 10> kAsrProviders = {{
  {"doubao", "豆包", "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async", ""},
  {"siliconflow", "SiliconFlow", "https://api.siliconflow.cn/v1/audio/transcriptions", "FunAudioLLM/SenseVoiceSmall"},
  {"openai", "OpenAI", "https://api.openai.com/v1/audio/transcriptions", "whisper-1"},
  {"groq", "Groq", "https://api.groq.com/openai/v1/audio/transcriptions", "whisper-large-v3-turbo"},
  {"everyapi", "EveryAPI", "https://api.everyapi.ai/v1/audio/transcriptions", "volc.seedasr.sauc.duration"},
  {"mistral", "Mistral · Voxtral", "https://api.mistral.ai/v1/audio/transcriptions", "voxtral-mini-latest"},
  {"bailian", "阿里云百炼 · 千问", "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions", "qwen3-asr-flash"},
  {"zhipu", "智谱 · GLM", "https://open.bigmodel.cn/api/paas/v4/audio/transcriptions", "glm-asr"},
  {"minimax", "MiniMax", "https://api.minimax.cn/v1/speech_to_text", "asr-1.0"},
  {"custom", "自定义", "", ""},
}};
} // 命名空间 msime::voice
