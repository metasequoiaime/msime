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
  // 共享页的十二个服务商，顺序相同，最后是没有默认值的「自定义」；id 不重复，与偏好库的 AI_PROVIDERS 相同。
  auto const &table = providers();
  check(table.size() == 12 && table.front().id == L"everyapi" && table[4].id == L"deepseek" &&
            table.back().id == L"custom" && table.back().endpoint.empty() &&
            table.back().model.empty() && table.back().models.empty() &&
            table.back().documentation.empty(),
        "the provider list");
  std::set<std::wstring_view> ids;
  for (auto const &entry : table)
    ids.insert(entry.id);
  const std::set<std::wstring_view> store{L"everyapi", L"openai",     L"anthropic", L"gemini",
                                          L"deepseek", L"qwen",       L"kimi",      L"zhipu",
                                          L"siliconflow", L"groq",    L"openrouter", L"custom"};
  check(ids == store, "provider ids match the preferences store");
  for (auto const &entry : table)
    if (entry.id != L"custom")
      check(!entry.endpoint.empty() && !entry.model.empty() && !entry.models.empty() &&
                entry.models.front() == entry.model && !entry.documentation.empty(),
            "every catalog provider has defaults, its default model first, and documentation");
  check(find_provider(L"deepseek")->endpoint == L"https://api.deepseek.com/chat/completions",
        "deepseek endpoint");
  check(find_provider(L"qwen")->title == L"通义千问 · 阿里云百炼", "qwen title");
  check(find_provider(L"unknown") == nullptr, "an unknown provider has no option");

  // aiProviderUpdate：默认值跟着换，用户填的保留。
  auto update = provider_update(L"openai", {L"deepseek", L"https://api.deepseek.com/chat/completions",
                                            L"deepseek-v4-flash"});
  check(update.provider == L"openai" &&
            update.endpoint == L"https://api.openai.com/v1/chat/completions" &&
            update.model == L"gpt-4.1-mini",
        "defaults follow the provider");
  update = provider_update(L"openai", {L"deepseek", L"https://llm.internal.example/v1/chat/completions",
                                       L"my-model"});
  check(update.endpoint == L"https://llm.internal.example/v1/chat/completions" &&
            update.model == L"my-model",
        "custom endpoint and model are kept");
  update = provider_update(L"kimi", {L"deepseek", L"  ", L""});
  check(update.endpoint == L"https://api.moonshot.cn/v1/chat/completions" &&
            update.model == L"kimi-k2.6",
        "blank values take the new defaults");
  update = provider_update(L"custom", {L"deepseek", L"https://api.deepseek.com/chat/completions",
                                       L"deepseek-v4-flash"});
  check(update.provider == L"custom" && update.endpoint.empty() && update.model.empty(),
        "switching to custom clears the previous defaults");
  update = provider_update(L"nonexistent", {L"openai", L"https://example.com/v1", L"x"});
  check(update.provider == L"custom" && update.endpoint == L"https://example.com/v1" &&
            update.model == L"x",
        "an unknown provider falls back to custom");
  // 原服务商不认识时，只有空值会被换掉；与默认值相同但不是原服务商的，也算用户自己填的。
  update = provider_update(L"openai", {L"legacy", L"https://api.deepseek.com/chat/completions",
                                       L"deepseek-v4-flash"});
  check(update.endpoint == L"https://api.deepseek.com/chat/completions" &&
            update.model == L"deepseek-v4-flash",
        "only the previous provider's defaults are replaced");
  // 比较不去空白：带空格的默认值算用户填的。
  update = provider_update(L"openai", {L"deepseek", L" https://api.deepseek.com/chat/completions",
                                       L"deepseek-v4-flash "});
  check(update.endpoint == L" https://api.deepseek.com/chat/completions" &&
            update.model == L"deepseek-v4-flash ",
        "padded defaults are kept");

  // aiServiceCredentialTestDisabled。
  check(!credential_test_disabled(true, true, L"m", L"t"), "complete settings can be tested");
  check(credential_test_disabled(false, true, L"m", L"t"), "disabled assistant");
  check(credential_test_disabled(true, false, L"m", L"t"), "unusable endpoint");
  check(credential_test_disabled(true, true, L" \t", L"t"), "blank model");
  check(credential_test_disabled(true, true, L"m", L"　"), "blank token");

  // 提示词槽位：只编辑 prompt_id 选中的那一个。
  check(prompt_slot(L"").id == L"custom_1" && prompt_slot(L"default").id == L"custom_1" &&
            prompt_slot(L"custom_3").name == L"三" && prompt_slot(L"custom_2").name == L"二",
        "prompt slots");
  // 说明讲清楚是整个替换、要求只输出 JSON；不是第一个槽位时说出是哪一个，与共享页相同。
  const std::wstring base =
      L"留空时使用内置的联想提示词。填写后会整个替换内置提示词，不是追加；内置提示词规定了只输出 JSON 格式的候选，自定义时也要这样要求，否则联想可能不出结果。";
  check(prompt_description(L"") == base && prompt_description(L"custom_1") == base &&
            prompt_description(L"default") == base,
        "the first slot names no slot");
  check(prompt_description(L"custom_2") == base + L"当前使用的是槽位二。" &&
            prompt_description(L"custom_3") == base + L"当前使用的是槽位三。",
        "another slot is named");

  // 获取模型后的提示和要换上的模型。
  check(models_status(3) == L"已获取 3 个可用模型。", "models status");
  check(fetched_model({L"a", L"b"}, L"b").empty() && fetched_model({}, L"x").empty() &&
            fetched_model({L"a", L"b"}, L"x") == L"a",
        "the fetched model replaces an unknown one");

  if (failures == 0)
    std::puts("ai settings: ok");
  return failures == 0 ? 0 : 1;
}
