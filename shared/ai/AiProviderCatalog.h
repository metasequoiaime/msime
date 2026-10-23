#pragma once
// 由共享 Rust AI 服务目录导出；通过 export_ai_provider_catalog 更新。
#include <string_view>
#include <vector>

namespace msime::ai {
struct ProviderPreset {
  std::wstring_view id;
  std::wstring_view title;
  std::wstring_view endpoint;
  std::wstring_view model;
  std::vector<std::wstring_view> models;
  std::wstring_view documentation;
};
inline const std::vector<ProviderPreset>& provider_catalog() {
  static const std::vector<ProviderPreset> table{
    {L"everyapi", L"EveryAPI", L"https://api.everyapi.ai/v1/chat/completions", L"deepseek-v4-flash", {L"deepseek-v4-flash", L"deepseek-v4-pro", L"claude-sonnet-5", L"glm-5.3-flash"}, L"https://everyapi.ai/models"},
    {L"openai", L"OpenAI", L"https://api.openai.com/v1/chat/completions", L"gpt-4.1-mini", {L"gpt-4.1-mini"}, L"https://developers.openai.com/api/docs/models/gpt-4.1-mini"},
    {L"anthropic", L"Anthropic · Claude", L"https://api.anthropic.com/v1/chat/completions", L"claude-sonnet-4-6", {L"claude-sonnet-4-6", L"claude-opus-5"}, L"https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk"},
    {L"gemini", L"Google · Gemini", L"https://generativelanguage.googleapis.com/v1beta/openai/chat/completions", L"gemini-3.8-flash", {L"gemini-3.8-flash", L"gemini-2.5-flash"}, L"https://ai.google.dev/gemini-api/docs/openai"},
    {L"deepseek", L"DeepSeek", L"https://api.deepseek.com/chat/completions", L"deepseek-v4-flash", {L"deepseek-v4-flash", L"deepseek-v4-pro"}, L"https://api-docs.deepseek.com/"},
    {L"qwen", L"通义千问 · 阿里云百炼", L"https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions", L"qwen-plus", {L"qwen-plus"}, L"https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope"},
    {L"kimi", L"Kimi · 月之暗面", L"https://api.moonshot.cn/v1/chat/completions", L"kimi-k2.6", {L"kimi-k2.6", L"kimi-k2.5"}, L"https://platform.kimi.com/docs/api/chat"},
    {L"zhipu", L"智谱 · GLM", L"https://open.bigmodel.cn/api/paas/v4/chat/completions", L"glm-4.7", {L"glm-4.7", L"glm-4.7-flashx"}, L"https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7"},
    {L"minimax", L"MiniMax", L"https://api.minimax.cn/v1/chat/completions", L"MiniMax-M3.1-Flash", {L"MiniMax-M3.1-Flash", L"MiniMax-M3"}, L"https://platform.minimax.cn/docs/api-reference/text-chat-openai"},
    {L"volcengine", L"火山方舟 · 豆包", L"https://ark.cn-beijing.volces.com/api/v3/chat/completions", L"doubao-seed-1.6-flash", {L"doubao-seed-1.6-flash", L"doubao-seed-1.6"}, L"https://ark.volcengine.com/region:cn-beijing/docs/chat-api"},
    {L"siliconflow", L"硅基流动", L"https://api.siliconflow.cn/v1/chat/completions", L"Qwen/Qwen3.6-27B", {L"Qwen/Qwen3.6-27B"}, L"https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation"},
    {L"groq", L"Groq", L"https://api.groq.com/openai/v1/chat/completions", L"llama-3.3-70b-versatile", {L"llama-3.3-70b-versatile", L"llama-3.1-8b-instant"}, L"https://console.groq.com/docs/quickstart"},
    {L"openrouter", L"OpenRouter", L"https://openrouter.ai/api/v1/chat/completions", L"openrouter/auto", {L"openrouter/auto"}, L"https://openrouter.ai/docs/quickstart"},
    {L"ollama", L"Ollama", L"http://localhost:11434/v1/chat/completions", L"", {}, L"https://docs.ollama.com/api/openai-compatibility"},
    {L"custom", L"自定义", L"", L"", {}, L""},
  };
  return table;
}
} // 命名空间 msime::ai
