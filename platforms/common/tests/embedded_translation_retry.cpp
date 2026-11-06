#include "../EmbeddedTranslationRetry.h"
#include <cassert>
#include <cstdlib>
#include <cstring>

using Json = nlohmann::json;
namespace {
std::atomic<int> calls{0};
std::atomic<bool> cancelled{false};
int busy_count = 0;
bool unavailable = false;
std::vector<int64_t> budgets;
char *reply(const Json &value) {
  const auto text = value.dump();
  auto *raw = static_cast<char *>(std::malloc(text.size() + 1));
  std::memcpy(raw, text.c_str(), text.size() + 1);
  return raw;
}
auto run(int64_t timeout = 2000) {
  return msime::translation::embedded_with_retry(
      {{"request_id", "synthetic"}, {"timeout_ms", timeout}}, "synthetic",
      [] { return cancelled.load(); });
}
void reset() {
  calls = 0; cancelled = false; busy_count = 0; unavailable = false; budgets.clear();
}
}
extern "C" char *msime_client_local_translation_query(const uint8_t *data, size_t size) {
  const auto query = Json::parse(data, data + size);
  assert(query.at("request_id") == "synthetic");
  budgets.push_back(query.at("timeout_ms").get<int64_t>());
  const int call = ++calls;
  if (unavailable) return reply({{"ok", false}, {"error", "local_translation_unavailable"}});
  if (call <= busy_count) return reply({{"ok", false}, {"error", "local_translation_busy"}});
  return reply({{"ok", true}, {"value", {{"translations", Json::array({{{"text", "合成文本"}, {"translation", "synthetic text"}}})}}}});
}
extern "C" void msime_client_local_translation_cancel(const uint8_t *, size_t) {}
extern "C" void msime_client_string_free(char *raw) { std::free(raw); }

int main() {
  // 两个旧请求仍占着登记槽时，当前查询必须等到空位后得到回答。
  busy_count = 2;
  auto result = run();
  assert(result && Json::parse(result.get()).at("ok") == true);
  assert(calls == 3 && budgets.size() == 3);
  assert(budgets[0] > budgets[1] && budgets[1] > budgets[2] && budgets[2] > 0);

  reset(); unavailable = true;
  result = run();
  assert(result && Json::parse(result.get()).at("error") == "local_translation_unavailable");
  assert(calls == 1);

  // 持续忙碌不能延长整批截止时间，也不能把零预算交给 Rust。
  reset(); busy_count = 1000;
  const auto began = std::chrono::steady_clock::now();
  result = run(150);
  assert(result && !Json::parse(result.get()).at("ok").get<bool>());
  assert(calls >= 1 && calls <= 2);
  assert(std::chrono::steady_clock::now() - began < std::chrono::seconds(2));
  for (auto budget : budgets) assert(budget > 0 && budget <= 150);

  reset(); busy_count = 1000;
  auto pending = std::async(std::launch::async, [] { return run(); });
  while (calls == 0) std::this_thread::yield();
  cancelled = true;
  assert(!pending.get());
  assert(calls == 1);

  reset(); cancelled = true;
  assert(!run() && calls == 0);
}
