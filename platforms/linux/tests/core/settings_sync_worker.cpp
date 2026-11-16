#include "core/SettingsSyncWorker.h"
#include <cassert>
#include <cstdlib>
#include <cstring>

namespace {
std::string received;
std::string answer = R"({"ok":true,"value":{}})";
int requests = 0;
int released = 0;
}

// 合成 C 入口只捕获请求；行为测试执行产品的 JSON 编解码和所有权包装。
extern "C" char *msime_client_settings_sync_start(const uint8_t *request, size_t length) {
  ++requests;
  received.assign(reinterpret_cast<const char *>(request), length);
  auto *response = static_cast<char *>(std::malloc(answer.size() + 1));
  assert(response);
  std::memcpy(response, answer.c_str(), answer.size() + 1);
  return response;
}

extern "C" void msime_client_string_free(char *response) {
  if (response) ++released;
  std::free(response);
}

int main() {
  using msime::linux_host::start_settings_sync;
  assert(!start_settings_sync(""));
  assert(!start_settings_sync("relative"));
  assert(requests == 0);
  const std::string directory = "/synthetic/state\"quoted";
  assert(start_settings_sync(directory));
  const auto request = nlohmann::json::parse(received);
  assert(request == nlohmann::json({{"preferences_directory", directory}}));
  assert(requests == 1 && released == 1);
  for (const std::string value : {"not-json", "{}", R"({"ok":false})", R"({"ok":"invalid"})"}) {
    answer = value;
    assert(!start_settings_sync(directory));
  }
  assert(requests == 5 && released == 5);
}
