#pragma once
#include "TranslationServiceSettings.h"
#include <algorithm>
#include <array>
#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

// 「AI 辅助」页的规则，平台固定为 Windows：服务商列表和各服务商的默认地址、模型、已知模型与接入文档（`packages/ui/src/settings/ai-provider-options.ts`），服务商下拉的品牌图标（`ai-provider-icon.tsx`），换服务商时改哪些字段（`ai-provider-update.ts`），「测试 AI 辅助配置」何时可用（`ai-credential-test-config.ts` 的 aiServiceCredentialTestDisabled），API Token 说明里的服务地址（`ai-endpoint-policy.ts` 的 aiEndpointServiceAddress），提示词槽位和自定义提示词的说明（`ai-settings-page-section.tsx`），以及获取模型的提示文字（`use-ai-assistant.ts`）。接口地址合不合规和地址的问题说明在 Rust（msime_client_ai_endpoint_check），不在这里。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::ai {

// 一个服务商（AI_PROVIDER_OPTIONS 的一项）。models 和 documentation 只用来显示，请求总是用设置里保存的值。
struct Provider {
  std::wstring_view id;
  std::wstring_view title;
  std::wstring_view endpoint;
  std::wstring_view model;
  std::vector<std::wstring_view> models;
  std::wstring_view documentation;
};

// 按共享页的顺序；最后一项「自定义」没有默认值，也是不认识的服务商换过去时落到的那一项。id 与偏好库的 AI_PROVIDERS 一致。
inline std::vector<Provider> const &providers() {
  static const std::vector<Provider> table{
      {L"everyapi", L"EveryAPI", L"https://api.everyapi.ai/v1/chat/completions",
       L"deepseek-v4-flash",
       {L"deepseek-v4-flash", L"deepseek-v4-pro", L"claude-sonnet-5", L"glm-5.3-flash"},
       L"https://everyapi.ai/models"},
      {L"openai", L"OpenAI", L"https://api.openai.com/v1/chat/completions", L"gpt-4.1-mini",
       {L"gpt-4.1-mini"}, L"https://developers.openai.com/api/docs/models/gpt-4.1-mini"},
      {L"anthropic", L"Anthropic · Claude", L"https://api.anthropic.com/v1/chat/completions",
       L"claude-sonnet-4-6", {L"claude-sonnet-4-6", L"claude-opus-5"},
       L"https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk"},
      {L"gemini", L"Google · Gemini",
       L"https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
       L"gemini-3.8-flash", {L"gemini-3.8-flash", L"gemini-2.5-flash"},
       L"https://ai.google.dev/gemini-api/docs/openai"},
      {L"deepseek", L"DeepSeek", L"https://api.deepseek.com/chat/completions",
       L"deepseek-v4-flash", {L"deepseek-v4-flash", L"deepseek-v4-pro"},
       L"https://api-docs.deepseek.com/"},
      {L"qwen", L"通义千问 · 阿里云百炼",
       L"https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions", L"qwen-plus",
       {L"qwen-plus"},
       L"https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope"},
      {L"kimi", L"Kimi · 月之暗面", L"https://api.moonshot.cn/v1/chat/completions", L"kimi-k2.6",
       {L"kimi-k2.6", L"kimi-k2.5"}, L"https://platform.kimi.com/docs/api/chat"},
      {L"zhipu", L"智谱 · GLM", L"https://open.bigmodel.cn/api/paas/v4/chat/completions",
       L"glm-4.7", {L"glm-4.7", L"glm-4.7-flashx"},
       L"https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7"},
      {L"siliconflow", L"硅基流动", L"https://api.siliconflow.cn/v1/chat/completions",
       L"Qwen/Qwen3.6-27B", {L"Qwen/Qwen3.6-27B"},
       L"https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation"},
      {L"groq", L"Groq", L"https://api.groq.com/openai/v1/chat/completions",
       L"llama-3.3-70b-versatile", {L"llama-3.3-70b-versatile", L"llama-3.1-8b-instant"},
       L"https://console.groq.com/docs/quickstart"},
      {L"openrouter", L"OpenRouter", L"https://openrouter.ai/api/v1/chat/completions",
       L"openrouter/auto", {L"openrouter/auto"}, L"https://openrouter.ai/docs/quickstart"},
      {L"ollama", L"Ollama", L"http://localhost:11434/v1/chat/completions", L"", {},
       L"https://docs.ollama.com/api/openai-compatibility"},
      {L"custom", L"自定义", L"", L"", {}, L""},
  };
  return table;
}

// aiProviderOption：不认识的服务商为空。
inline Provider const *find_provider(std::wstring_view id) {
  for (auto const &entry : providers())
    if (entry.id == id)
      return &entry;
  return nullptr;
}

// 换服务商时读写的三项。
struct ProviderFields {
  std::wstring provider;
  std::wstring endpoint;
  std::wstring model;
};

// aiProviderUpdate（updateAiProvider）：换上新服务商；接口地址和模型为空（去掉首尾空白后）或还是原服务商的默认值时换成新服务商的默认值，否则是用户自己填的，保留。不认识的服务商按「自定义」处理。
inline ProviderFields provider_update(std::wstring_view provider, ProviderFields const &current) {
  auto const &table = providers();
  const auto *next = find_provider(provider);
  if (!next)
    next = &table.back();
  const auto *previous = find_provider(current.provider);
  ProviderFields update;
  update.provider = std::wstring(next->id);
  update.endpoint = js_trim(current.endpoint).empty() ||
                            (previous && current.endpoint == previous->endpoint)
                        ? std::wstring(next->endpoint)
                        : current.endpoint;
  update.model = js_trim(current.model).empty() || (previous && current.model == previous->model)
                     ? std::wstring(next->model)
                     : current.model;
  return update;
}

// 服务商下拉的品牌图标（`ai-provider-icon.tsx` 的 icons）：`file` 是 `packages/ui/src/assets/providers` 里那张 SVG 的文件名，Windows 用的是它转成的 80 × 80 PNG（`platforms/windows/settings/assets/providers`），由 Settings.rc 打成 RCDATA，资源号是 `provider_icon_resource_base` 加上这里的序号，所以顺序要和 Settings.rc 一致（`scripts/test-windows-settings-art.py` 核对）。`dark_tile` 照 `data-brand-tile`：Kimi 的白色笔画和 OpenRouter 的亮绿标志放在深色底板上，其余放在白色底板上，深浅主题和选中都不改图标颜色。「自定义」和不认识的服务商没有图标。
struct ProviderIcon {
  std::wstring_view provider;
  std::string_view file;
  bool dark_tile;
};

inline constexpr int provider_icon_resource_base = 150;

inline constexpr std::array<ProviderIcon, 12> provider_icons{{
    {L"everyapi", "everyapi", false},
    {L"openai", "openai", false},
    {L"anthropic", "claude", false},
    {L"gemini", "gemini", false},
    {L"deepseek", "deepseek", false},
    {L"qwen", "qwen", false},
    {L"kimi", "kimi", true},
    {L"zhipu", "zhipu", false},
    {L"siliconflow", "siliconcloud", false},
    {L"groq", "groq", false},
    {L"openrouter", "openrouter", true},
    {L"ollama", "ollama", false},
}};

// 服务商图标的资源号，没有图标时为 0。
inline int provider_icon_resource(std::wstring_view provider) {
  for (std::size_t i = 0; i < provider_icons.size(); ++i)
    if (provider_icons[i].provider == provider)
      return provider_icon_resource_base + static_cast<int>(i);
  return 0;
}

// 服务商的图标放在深色底板上。
inline bool provider_icon_dark_tile(std::wstring_view provider) {
  for (auto const &icon : provider_icons)
    if (icon.provider == provider)
      return icon.dark_tile;
  return false;
}

// aiProviderUsesToken：Ollama 不鉴权，页面不显示 API Token，请求也不带（client-providers 的 `ai::provider_uses_token`）。
inline bool provider_uses_token(std::wstring_view provider) {
  return provider != L"ollama";
}

// 「测试 AI 辅助配置」不可用：AI 辅助关着、接口地址不能用、没填模型，或服务要 Token 却没填（aiServiceCredentialTestDisabled）。
inline bool credential_test_disabled(bool enabled, bool has_origin, std::wstring_view model,
                                     std::wstring_view token, std::wstring_view provider) {
  return !enabled || !has_origin || js_trim(model).empty() ||
         (provider_uses_token(provider) && js_trim(token).empty());
}

// 「API Token」说明里的服务地址，照 aiEndpointServiceAddress：地址的 origin（省略默认端口）加上路径，去掉末尾的 /chat/completions、/responses、/messages 这类请求动作和末尾的 /，不带查询参数，保留 /v1 这类接口前缀。`origin` 是 msime_client_ai_endpoint_check 给的来源键（`scheme://host:port`，主机已按 URL 规范化、端口总是写出），只有地址能用时才有，所以这里不再检查地址，只从原文取路径：去掉两头的空格（URL 解析也这样做；控制字符和反斜杠的地址不合规，到不了这里），`://` 之后到第一个 `/` 是主机，到 `?` 为止是路径。与浏览器的 URL 不同，路径按原文显示，不折叠 `.`、`..` 段，也不把非 ASCII 字符编成百分号，接口地址里几乎用不到这些写法。
inline std::wstring service_address(std::wstring_view endpoint, std::wstring_view origin) {
  // windows-ai-settings 按 C++17 编译，没有 starts_with / ends_with，这里自己比较。
  auto ends_with = [](std::wstring_view value, std::wstring_view suffix) {
    return value.size() >= suffix.size() &&
           value.compare(value.size() - suffix.size(), suffix.size(), suffix) == 0;
  };
  std::wstring address(origin);
  auto drop_default_port = [&address, &ends_with](std::wstring_view scheme, std::wstring_view port) {
    if (address.rfind(scheme, 0) == 0 && ends_with(address, port))
      address.resize(address.size() - port.size());
  };
  drop_default_port(L"https://", L":443");
  drop_default_port(L"http://", L":80");
  const auto first = endpoint.find_first_not_of(L' ');
  if (first == std::wstring_view::npos)
    return address;
  endpoint = endpoint.substr(first, endpoint.find_last_not_of(L' ') - first + 1);
  const auto separator = endpoint.find(L"://");
  if (separator == std::wstring_view::npos)
    return address;
  const auto rest = endpoint.substr(separator + 3);
  const auto start = rest.find_first_of(L"/?");
  std::wstring path;
  if (start != std::wstring_view::npos && rest[start] == L'/')
    path = std::wstring(rest.substr(start, rest.find(L'?', start) - start));
  for (std::wstring_view action : {L"/chat/completions", L"/responses", L"/messages"}) {
    if (ends_with(path, std::wstring(action) + L"/")) {
      path.resize(path.size() - action.size() - 1);
      break;
    }
    if (ends_with(path, action)) {
      path.resize(path.size() - action.size());
      break;
    }
  }
  if (!path.empty() && path.back() == L'/')
    path.pop_back();
  return address + path;
}

struct PromptSlot {
  std::wstring_view id;
  std::wstring_view name;
};

// 三个提示词槽位（slotPrompts 的 name），提示词存在 ai_assistant.prompt_<id>。设置页只编辑 prompt_id 选中的那一个，不再给选槽位的下拉，语音润色另有自己的槽位选项（VoiceSettings.h）。
inline constexpr std::array<PromptSlot, 3> prompt_slots{{
    {L"custom_1", L"一"},
    {L"custom_2", L"二"},
    {L"custom_3", L"三"},
}};

// 所选的槽位：prompt_id 为空或不是三个槽位之一时用第一个（`promptId || "custom_1"`，slotPrompts 找不到时退回 custom_1）。
inline PromptSlot const &prompt_slot(std::wstring_view prompt_id) {
  for (auto const &slot : prompt_slots)
    if (slot.id == prompt_id)
      return slot;
  return prompt_slots[0];
}

// 「提示词内容」的说明，与共享页相同：填了就整个替换内置提示词，内置的那份规定了 JSON 输出格式（ai.rs 的 DEFAULT_CANDIDATE_PROMPT），所以要讲清楚。当前槽位不是第一个时说出是哪一个，用户在别的设备或旧版本里选过别的槽位时才看得到这一句。
inline std::wstring prompt_description(std::wstring_view prompt_id) {
  std::wstring text = L"留空使用内置提示词；填写后完整替换，请要求只输出 JSON 格式的候选。";
  auto const &slot = prompt_slot(prompt_id);
  if (slot.id != L"custom_1")
    text += L"当前使用槽位" + std::wstring(slot.name) + L"。";
  return text;
}

// 获取模型成功后的提示。
inline std::wstring models_status(std::size_t count) {
  return L"已获取 " + std::to_wstring(count) + L" 个可用模型。";
}

// 获取到模型列表后要换上的模型：列表不空且不含当前模型时取第一个，否则为空（不改）。
inline std::wstring fetched_model(std::vector<std::wstring> const &models, std::wstring_view model) {
  if (models.empty() || std::find(models.begin(), models.end(), model) != models.end())
    return {};
  return models.front();
}

// 宿主返回的是错误码而不是 Error，共享页只显示它自己的兜底文字。
inline constexpr std::wstring_view models_failure = L"获取模型失败，请检查地址、密钥和网络。";

} // namespace msime::settings::ai
