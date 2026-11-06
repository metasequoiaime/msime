#include "../LocalTranslationSettings.h"
#include <cassert>
#include <cstring>
#include <thread>

using Json = nlohmann::json;
namespace {
std::atomic<bool> cancelled{false}, started{false};
Json selection{{"enabled", false}, {"target_language", "en"}};
int revision = 1;
bool installed = true;
char *reply(Json value) {
  const auto bytes = value.dump();
  auto *result = static_cast<char *>(std::malloc(bytes.size() + 1));
  std::memcpy(result, bytes.c_str(), bytes.size() + 1);
  return result;
}
void finish(msime::translation::SettingsModel &model) {
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
  while (model.busy() && std::chrono::steady_clock::now() < deadline) {
    model.poll(); std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  assert(!model.busy());
}
}
extern "C" {
void msime_client_string_free(char *value) { std::free(value); }
char *msime_client_local_translation_catalog() {
  return reply({{"ok", true}, {"value", {{"available", true}}}});
}
char *msime_client_local_translation_settings(const uint8_t *data, size_t length) {
  auto request = Json::parse(data, data + length);
  if (request.contains("update")) {
    const auto &update = request.at("update");
    if (update.at("expected_revision") != revision)
      return reply({{"ok", false}, {"error", "preferences changed"}});
    selection = {{"enabled", update.at("enabled")}, {"target_language", update.at("target_language")}};
    ++revision;
  }
  return reply({{"ok", true}, {"value", {{"revision", revision}, {"local_translation", selection}}}});
}
char *msime_client_resource_packs(const uint8_t *, size_t) {
  return reply({{"ok", true}, {"value", Json::array({{{"id", "local-translation"}, {"state", installed ? "installed" : "missing"}}})}});
}
char *msime_client_resource_pack_remove(const uint8_t *, size_t) {
  assert(!selection.at("enabled").get<bool>());
  installed = false;
  return reply({{"ok", true}});
}
void msime_client_resource_pack_cancel(const uint8_t *, size_t) {
  cancelled = true;
}
char *msime_client_resource_pack_install(const uint8_t *, size_t, msime_client_resource_pack_progress_callback callback, void *context) {
  // 模拟安装器晚于窗口关闭才登记取消标记；第一次进度回调必须补发取消。
  std::this_thread::sleep_for(std::chrono::milliseconds(30));
  cancelled = false; started = true;
  callback(context, "local-translation", 0, 100);
  assert(cancelled);
  return reply({{"ok", false}, {"error", "cancelled"}});
}
}
int main() {
  msime::translation::SettingsModel model("/synthetic/local-translation");
  finish(model);
  assert(model.view.at("installed"));
  model.save(true, "ja"); finish(model);
  assert(selection.at("enabled") && selection.at("target_language") == "ja");
  ++revision;
  model.save(false, "fr"); finish(model);
  assert(model.notice == "preferences changed");
  assert(model.view.at("settings").at("revision") == revision);
  assert(selection.at("target_language") == "ja");
  model.remove(); finish(model);
  assert(!installed && !selection.at("enabled").get<bool>());
  model.install(); model.cancel(); finish(model);
  assert(started && cancelled && model.notice == "cancelled");
  model.save(false, "fr"); finish(model);
  assert(model.notice.empty() && selection.at("target_language") == "fr");
}
