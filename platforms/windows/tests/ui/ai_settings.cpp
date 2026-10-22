#include "../../settings/AiSettings.h"
#include <cstdio>
#include <set>
#include <string>

using namespace msime::settings::ai;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
} // namespace

int main() {
  // 共享页的十五个服务商，顺序相同，最后是没有默认值的「自定义」；id 不重复，与偏好库的 AI_PROVIDERS 相同。
  auto const &table = providers();
  check(table.size() == 15 && table.front().id == L"everyapi" && table[4].id == L"deepseek" &&
            table.back().id == L"custom" && table.back().endpoint.empty() &&
            table.back().model.empty() && table.back().models.empty() &&
            table.back().documentation.empty(),
        "the provider list");
  std::set<std::wstring_view> ids;
  for (auto const &entry : table)
    ids.insert(entry.id);
  const std::set<std::wstring_view> store{
      L"everyapi", L"openai", L"anthropic", L"gemini", L"deepseek", L"qwen", L"kimi", L"zhipu",
      L"minimax", L"volcengine", L"siliconflow", L"groq", L"openrouter", L"ollama", L"custom"};
  check(ids == store, "provider ids match the preferences store");
  for (auto const &entry : table)
    if (entry.id != L"custom" && entry.id != L"ollama")
      check(!entry.endpoint.empty() && !entry.model.empty() && !entry.models.empty() &&
                entry.models.front() == entry.model && !entry.documentation.empty(),
            "every catalog provider has defaults, its default model first, and documentation");
  check(find_provider(L"deepseek")->endpoint == L"https://api.deepseek.com/chat/completions",
        "deepseek endpoint");
  check(find_provider(L"qwen")->title == L"通义千问 · 阿里云百炼", "qwen title");
  check(find_provider(L"unknown") == nullptr, "an unknown provider has no option");
  // Ollama 的模型是用户自己拉下来的，没有默认模型，靠「获取模型列表」；也不要 Token。
  check(find_provider(L"ollama")->endpoint == L"http://localhost:11434/v1/chat/completions" &&
            find_provider(L"ollama")->model.empty() && find_provider(L"ollama")->models.empty() &&
            !find_provider(L"ollama")->documentation.empty(),
        "ollama has a local endpoint and no default model");
  check(!provider_uses_token(L"ollama") && provider_uses_token(L"custom") &&
            provider_uses_token(L"deepseek"),
        "only ollama goes without a token");

  // aiProviderUpdate：默认值跟着换；自定义值存进原服务商的槽位，切走再切回来还在。
  std::map<std::wstring, std::wstring> endpoints, models;
  auto update = provider_update(L"openai", {L"deepseek", L"https://api.deepseek.com/chat/completions",
                                            L"deepseek-v4-flash"},
                                endpoints, models);
  check(update.provider == L"openai" &&
            update.endpoint == L"https://api.openai.com/v1/chat/completions" &&
            update.model == L"gpt-4.1-mini" && endpoints.empty() && models.empty(),
        "defaults follow the provider and stay out of the slots");
  update = provider_update(L"openai", {L"deepseek", L"https://llm.internal.example/v1/chat/completions",
                                       L"my-model"},
                           endpoints, models);
  check(update.endpoint == L"https://api.openai.com/v1/chat/completions" &&
            update.model == L"gpt-4.1-mini" &&
            endpoints[L"deepseek"] == L"https://llm.internal.example/v1/chat/completions" &&
            models[L"deepseek"] == L"my-model",
        "custom endpoint and model move into the previous provider's slot");
  // 切回 deepseek：它的槽位回来，openai 的默认值没有槽位可写。
  update = provider_update(L"deepseek", update, endpoints, models);
  check(update.provider == L"deepseek" &&
            update.endpoint == L"https://llm.internal.example/v1/chat/completions" &&
            update.model == L"my-model",
        "switching back restores the remembered values");
  // openai 上改成自定义值再切走：槽位按编辑后的值更新。
  update = provider_update(L"kimi", {L"openai", L"https://proxy.internal/v1", L"gpt-custom"},
                           endpoints, models);
  check(endpoints[L"openai"] == L"https://proxy.internal/v1" &&
            models[L"openai"] == L"gpt-custom",
        "a second provider keeps its own slot");
  update = provider_update(L"kimi", {L"deepseek", L"  ", L""}, endpoints, models);
  check(update.endpoint == L"https://api.moonshot.cn/v1/chat/completions" &&
            update.model == L"kimi-k2.6",
        "blank values take the new defaults");
  update = provider_update(L"custom", {L"deepseek", L"https://api.deepseek.com/chat/completions",
                                       L"deepseek-v4-flash"},
                           endpoints, models);
  check(update.provider == L"custom" && update.endpoint.empty() && update.model.empty(),
        "switching to custom takes its empty defaults");
  update = provider_update(L"nonexistent", {L"openai", L"https://example.com/v1", L"x"}, endpoints,
                           models);
  check(update.provider == L"custom" && update.endpoint.empty() && update.model.empty(),
        "an unknown provider falls back to custom");
  // 原服务商不认识时，现值照存进它名下的槽位。
  update = provider_update(L"openai", {L"legacy", L"https://api.deepseek.com/chat/completions",
                                       L"deepseek-v4-flash"},
                           endpoints, models);
  check(update.endpoint == L"https://example.com/v1" && update.model == L"x" &&
            endpoints[L"legacy"] == L"https://api.deepseek.com/chat/completions" &&
            models[L"legacy"] == L"deepseek-v4-flash",
        "an unknown previous provider still gets a slot");
  // 与默认值相同的值不进槽位（比较前去掉首尾空白）。
  endpoints.clear();
  models.clear();
  update = provider_update(L"openai", {L"deepseek", L" https://api.deepseek.com/chat/completions ",
                                       L"deepseek-v4-flash "},
                           endpoints, models);
  check(update.endpoint == L"https://api.openai.com/v1/chat/completions" && endpoints.empty(),
        "padded defaults are not custom values");

  // aiServiceCredentialTestDisabled。
  check(!credential_test_disabled(true, true, L"m", L"t", L"openai"), "complete settings can be tested");
  check(credential_test_disabled(false, true, L"m", L"t", L"openai"), "disabled assistant");
  check(credential_test_disabled(true, false, L"m", L"t", L"openai"), "unusable endpoint");
  check(credential_test_disabled(true, true, L" \t", L"t", L"openai"), "blank model");
  check(credential_test_disabled(true, true, L"m", L"　", L"openai"), "blank token");
  check(!credential_test_disabled(true, true, L"m", L"", L"ollama"), "ollama needs no token");
  check(credential_test_disabled(true, true, L"", L"", L"ollama"), "ollama still needs a model");

  // 提示词槽位：只编辑 prompt_id 选中的那一个。
  check(prompt_slot(L"").id == L"custom_1" && prompt_slot(L"default").id == L"custom_1" &&
            prompt_slot(L"custom_3").name == L"三" && prompt_slot(L"custom_2").name == L"二",
        "prompt slots");
  // 说明讲清楚是整个替换、要求只输出 JSON；不是第一个槽位时说出是哪一个，与共享页相同。
  const std::wstring base = L"留空使用内置提示词；填写后完整替换，请要求只输出 JSON 格式的候选。";
  check(prompt_description(L"") == base && prompt_description(L"custom_1") == base &&
            prompt_description(L"default") == base,
        "the first slot names no slot");
  check(prompt_description(L"custom_2") == base + L"当前使用槽位二。" &&
            prompt_description(L"custom_3") == base + L"当前使用槽位三。",
        "another slot is named");

  // API Token 说明里的服务地址（aiEndpointServiceAddress）：省略默认端口和请求动作，保留接口前缀，不带查询参数。
  check(service_address(L"https://api.deepseek.com/chat/completions", L"https://api.deepseek.com:443") ==
            L"https://api.deepseek.com",
        "the action and the default port go");
  check(service_address(L"https://api.openai.com/v1/chat/completions", L"https://api.openai.com:443") ==
            L"https://api.openai.com/v1",
        "the api prefix stays");
  check(service_address(L"http://localhost:11434/v1/chat/completions", L"http://localhost:11434") ==
            L"http://localhost:11434/v1",
        "a non-default port stays");
  check(service_address(L"http://192.168.1.2:80/api/messages/", L"http://192.168.1.2:80") ==
            L"http://192.168.1.2/api",
        "a trailing slash after the action goes with it");
  check(service_address(L"  https://Example.COM/v1/responses?key=1  ", L"https://example.com:443") ==
            L"https://example.com/v1",
        "the query and surrounding spaces go, the host comes from the origin");
  check(service_address(L"https://example.com", L"https://example.com:443") == L"https://example.com" &&
            service_address(L"https://example.com/?a=1", L"https://example.com:443") ==
                L"https://example.com" &&
            service_address(L"https://example.com?a=1", L"https://example.com:443") ==
                L"https://example.com",
        "no path");
  check(service_address(L"https://example.com/proxy/", L"https://example.com:8443") ==
            L"https://example.com:8443/proxy",
        "one trailing slash goes");
  check(service_address(L"https://example.com/v1/completions", L"https://example.com:443") ==
            L"https://example.com/v1/completions",
        "other endpoints keep their path");

  // 服务商下拉的图标：已有品牌资源从 150 起按表的顺序，目录新增服务商可以没有图标；
  // Kimi 和 OpenRouter 使用深色底板。
  for (auto const &icon : provider_icons)
    check(find_provider(icon.provider) != nullptr, "every provider icon has a catalog provider");
  check(provider_icon_resource(L"custom") == 0 && provider_icon_resource(L"minimax") == 0 &&
            provider_icon_resource(L"volcengine") == 0,
        "providers without shipped icons have no resource");
  check(provider_icon_resource(L"everyapi") == 150 && provider_icon_resource(L"ollama") == 161 &&
            provider_icon_resource(L"unknown") == 0,
        "icon resource ids");
  check(provider_icon_dark_tile(L"kimi") && provider_icon_dark_tile(L"openrouter") &&
            !provider_icon_dark_tile(L"openai") && !provider_icon_dark_tile(L"custom"),
        "dark tiles");

  // 获取模型后的提示和要换上的模型。
  check(models_status(3) == L"已获取 3 个可用模型。", "models status");
  check(fetched_model({L"a", L"b"}, L"b").empty() && fetched_model({}, L"x").empty() &&
            fetched_model({L"a", L"b"}, L"x") == L"a",
        "the fetched model replaces an unknown one");

  if (failures == 0)
    std::puts("ai settings: ok");
  return failures == 0 ? 0 : 1;
}
