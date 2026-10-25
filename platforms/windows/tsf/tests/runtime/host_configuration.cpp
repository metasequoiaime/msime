#include "../../HostConfiguration.h"
#include "../../HostOptionsPaths.h"
#include <msime_client.h>
#include <atomic>
#include <cassert>
#include <chrono>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <string>
#include <mutex>
#include <thread>

namespace {
std::mutex gate;
std::condition_variable changed;
bool blocked = true;
int started = 0;
int initialReads = 0;
std::atomic<int> optionsReads{0};
std::string initialResponse = R"({"ok":true,"value":{"format_version":1,"revision":1,"preferences":{"scheme":"quanpin","game_compatibility":{"candidate_overlay":false}}}})";
std::string response = R"({"ok":true,"value":{"format_version":1,"revision":2,"preferences":{"scheme":"wubi"}}})";
char *Copy(const std::string &value) {
  auto *result = static_cast<char *>(std::malloc(value.size() + 1));
  std::memcpy(result, value.c_str(), value.size() + 1);
  return result;
}
void WaitForRevision(int revision) {
  const auto end = std::chrono::steady_clock::now() + std::chrono::seconds(5);
  while (std::chrono::steady_clock::now() < end) {
    const auto snapshot = msime::tsf::cached_host_configuration();
    if (snapshot && nlohmann::json::parse(snapshot->preferenceSnapshot)["revision"] == revision) return;
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
  }
  assert(false && "background configuration was not published");
}
// The state root, absolute and holding the installer's DataDir\helpcodes, so the TIP names it to its sessions.
const std::filesystem::path &state_root() {
  static const auto root = [] {
    auto path = std::filesystem::temp_directory_path() /
                ("msime-tsf-host-configuration-" +
                 std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    std::filesystem::create_directories(path / "helpcodes");
    return path;
  }();
  return root;
}
}
namespace msime::tsf {
std::string default_state_directory() { return state_root().u8string(); }
std::string read_prepared_host_options(const std::filesystem::path &path) {
  assert(path.filename() == "runtime-options.json");
  ++optionsReads;
  return R"({"format_version":1})";
}
}
extern "C" char *msime_client_load_preferences(const uint8_t *, size_t) {
  ++initialReads;
  return Copy(initialResponse);
}
extern "C" char *msime_client_try_load_preferences(const uint8_t *, size_t) {
  std::unique_lock<std::mutex> lock(gate);
  ++started;
  changed.notify_all();
  changed.wait(lock, [] { return !blocked; });
  return Copy(response);
}
extern "C" void msime_client_string_free(char *value) { std::free(value); }

int main() {
  using namespace msime::tsf;
  initialize_host_configuration();
  initialize_host_configuration();
  const auto first = cached_host_configuration();
  assert(first && first->preferences["scheme"] == "quanpin");
  assert(!first->preferences["game_compatibility"]["candidate_overlay"].get<bool>());
  assert(initialReads == 1 && optionsReads == 1);
  // The session options carry DataDir as the root of the built-in helpcode tables; the published file does not.
  const auto sessionOptions = nlohmann::json::parse(first->hostOptions);
  assert(sessionOptions.at("format_version") == 1);
  assert(sessionOptions.at("helpcode_fallback_resources") == state_root().u8string());
  // 首次激活已经发布了同一份配置，游戏策略读取不得再次打开偏好文件。
  const auto firstActivation = load_activation_preferences();
  assert(firstActivation && (*firstActivation)["scheme"] == "quanpin");
  assert(initialReads == 1);
  refresh_host_configuration();
  {
    std::unique_lock<std::mutex> lock(gate);
    assert(changed.wait_for(lock, std::chrono::seconds(5), [] { return started == 1; }));
  }
  for (int i = 0; i < 10000; ++i) {
    assert(cached_host_configuration() == first);
    refresh_host_configuration();
  }
  {
    std::lock_guard<std::mutex> lock(gate);
    assert(started == 1 && optionsReads == 1);
    blocked = false;
  }
  changed.notify_all();
  WaitForRevision(2);
  assert(cached_host_configuration()->preferences["scheme"] == "wubi");
  assert(first->preferences["scheme"] == "quanpin");

  // 停用期间新增的排除规则必须在激活时读取，不能借用旧缓存。
  initialResponse = R"({"ok":true,"value":{"format_version":1,"revision":3,"preferences":{"game_compatibility":{"excluded_processes":["synthetic-game.exe"]}}}})";
  const auto activation = load_activation_preferences();
  assert(activation && (*activation)["game_compatibility"]["excluded_processes"][0] == "synthetic-game.exe");
  assert(initialReads == 2);
  initialResponse = R"({"ok":false,"error":"synthetic unavailable"})";
  assert(!load_activation_preferences());
  assert(cached_host_configuration()->preferences["scheme"] == "wubi");

  // 忙碌的配置存储不能发布默认值覆盖已知设置。
  std::this_thread::sleep_for(std::chrono::milliseconds(260));
  {
    std::lock_guard<std::mutex> lock(gate);
    response = R"({"ok":true,"value":null})";
  }
  const auto beforeBusyRead = cached_host_configuration();
  const int optionsBefore = optionsReads;
  refresh_host_configuration();
  const auto end = std::chrono::steady_clock::now() + std::chrono::seconds(5);
  while (optionsReads == optionsBefore && std::chrono::steady_clock::now() < end)
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
  assert(optionsReads > optionsBefore);
  while (cached_host_configuration() == beforeBusyRead && std::chrono::steady_clock::now() < end)
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
  assert(cached_host_configuration() != beforeBusyRead);
  WaitForRevision(2);
  assert(cached_host_configuration()->preferences["scheme"] == "wubi");
  std::error_code error;
  std::filesystem::remove_all(state_root(), error);
}
