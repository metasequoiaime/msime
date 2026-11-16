#include "system/SettingsSyncWorker.h"
#include <cstdlib>
#include <cstring>
#include <stdexcept>
#include <string>

namespace {
int starts = 0;
int stops = 0;
int releases = 0;
bool exited = true;
bool missing_response = false;
std::string answer = R"({"ok":true,"value":{}})";
nlohmann::json request;

void require(bool condition) {
  if (!condition) throw std::runtime_error("Settings sync worker test failed");
}
}

extern "C" char *msime_client_settings_sync_start(const uint8_t *bytes, size_t size) {
  ++starts;
  request = nlohmann::json::parse(bytes, bytes + size);
  if (missing_response) return nullptr;
  auto *copy = static_cast<char *>(std::malloc(answer.size() + 1));
  if (!copy) throw std::bad_alloc();
  std::memcpy(copy, answer.c_str(), answer.size() + 1);
  return copy;
}

extern "C" void msime_client_string_free(char *value) {
  ++releases;
  std::free(value);
}

extern "C" bool msime_client_settings_sync_stop(uint32_t timeout_ms) {
  require(timeout_ms == 2000);
  ++stops;
  return exited;
}

int main() {
  const nlohmann::json options{{"preferences_directory", R"(C:\synthetic\quoted"folder)"},
                              {"preferences", {{"token", "synthetic-private"}}}};
  {
    msime::windows::SettingsSyncWorker worker;
    require(!worker.start(false, options));
    require(!worker.start(true, nlohmann::json::object()));
    require(!worker.start(true, {{"preferences_directory", 7}}));
    require(!worker.start(true, {{"preferences_directory", ""}}));
    require(starts == 0);
    require(worker.start(true, options));
    require((request == nlohmann::json{{"preferences_directory", options["preferences_directory"]}}));
    require(worker.start(true, options));
    require(starts == 1 && releases == 1);
    exited = false;
    worker.stop();
    require(stops == 1);
    exited = true;
  }
  require(stops == 2);
  {
    msime::windows::SettingsSyncWorker worker;
    require(worker.start(true, options));
    worker.stop();
    worker.stop();
  }
  require(stops == 3);
  for (const auto &bad : {"not json", "[]", R"({"ok":"true"})", R"({"ok":false})"}) {
    answer = bad;
    msime::windows::SettingsSyncWorker worker;
    require(!worker.start(true, options));
  }
  missing_response = true;
  {
    msime::windows::SettingsSyncWorker worker;
    require(!worker.start(true, options));
  }
  require(stops == 3 && starts == 7 && releases == 6);
}
