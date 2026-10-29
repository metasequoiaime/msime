#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include "../../../../../crates/host-api/include/msime_client.h"
#include <nlohmann/json.hpp>
#include <atomic>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <string>

namespace {
std::mutex lock;
bool enabled = true, readable = true;
unsigned epoch = 1;
std::atomic<unsigned> preparations{0}, invalid{0}, consent_reads{0};
std::atomic<int> block{0};
HANDLE entered = nullptr, resumed = nullptr;
std::string root;
std::filesystem::path preferences;
unsigned marker = 0;
void notify() {
  const auto path = preferences / "consent-marker";
  std::ofstream stream(path, std::ios::binary | std::ios::trunc);
  stream << ++marker;
  stream.close();
  if (stream.fail()) ++invalid;
}
char *response(nlohmann::json value) {
  const auto body = nlohmann::json{{"ok", true}, {"value", std::move(value)}}.dump();
  auto *out = static_cast<char *>(std::malloc(body.size() + 1));
  if (out) std::memcpy(out, body.c_str(), body.size() + 1);
  return out;
}
void barrier(int phase) {
  int expected = phase;
  if (block.compare_exchange_strong(expected, 0)) {
    SetEvent(entered);
    if (WaitForSingleObject(resumed, 10000) != WAIT_OBJECT_0) ++invalid;
  }
}
}
extern "C" {
__declspec(noinline) void msime_test_host_crash() {
  volatile std::uintptr_t address = 1;
  *reinterpret_cast<volatile unsigned *>(address) = 42;
}
void msime_test_host_initialize(const char *directory) {
  root = directory;
  wchar_t value[32768]{};
  const DWORD length = GetEnvironmentVariableW(L"MSIME_TSF_TEST_PREFERENCES", value, 32768);
  preferences = length && length < 32768 ? std::filesystem::path(value) : std::filesystem::u8path(root);
  notify();
  entered = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  resumed = CreateEventW(nullptr, TRUE, FALSE, nullptr);
}
void msime_test_host_finish() { CloseHandle(entered); CloseHandle(resumed); }
void msime_test_host_mode(int mode) {
  std::lock_guard<std::mutex> guard(lock);
  if (mode == 0) enabled = false;
  else if (mode == 1) { if (!enabled) ++epoch; enabled = true; readable = true; }
  else if (mode == 2) readable = false;
  else if (mode == 3) { ++epoch; enabled = true; readable = true; }
  // 不通知的恢复用来验证读取失败后的快速重试，不能依赖新的文件事件。
  else if (mode == 4) { readable = true; return; }
  else if (mode == 5) { enabled = false; return; }
  notify();
}
void msime_test_host_block(int phase) {
  ResetEvent(entered); ResetEvent(resumed); block = phase;
  std::lock_guard<std::mutex> guard(lock);
  notify();
}
bool msime_test_host_wait(DWORD timeout) { return WaitForSingleObject(entered, timeout) == WAIT_OBJECT_0; }
void msime_test_host_resume() { SetEvent(resumed); }
unsigned msime_test_host_preparations() { return preparations.load(); }
unsigned msime_test_host_invalid() { return invalid.load(); }
unsigned msime_test_host_consent_reads() { return consent_reads.load(); }
char *msime_client_telemetry_consent(const uint8_t *, size_t) {
  barrier(1);
  ++consent_reads;
  std::lock_guard<std::mutex> guard(lock);
  return readable ? response({{"enabled", enabled}, {"epoch", std::to_string(epoch)}}) : nullptr;
}
char *msime_client_telemetry_begin(const uint8_t *request, size_t length) {
  const auto value = nlohmann::json::parse(request, request + length);
  if (!value.value("capture_only", false) || value.value("component", "") != "tsf" ||
      value.value("platform", "") != "windows" || value.contains("enabled") || !value.contains("preferences_directory")) ++invalid;
  nlohmann::json result;
  {
    std::lock_guard<std::mutex> guard(lock);
    if (!enabled || value.value("consent_epoch", "") != std::to_string(epoch)) return response({{"enabled", false}});
    const auto id = ++preparations;
    char name[64]{};
    std::snprintf(name, sizeof(name), "%08x-aaaa-4aaa-8aaa-aaaaaaaaaaaa.crash", id);
    const auto path = (std::filesystem::u8path(root) / name).u8string();
    result = {{"enabled", true}, {"crash_record_path", path},
              {"crash_record_header", "MSIME-CRASH/1 {\"component\":\"tsf\",\"epoch\":\"" + std::to_string(epoch) + "\"}\n"}};
  }
  // 在生成旧授权响应后阻塞，以便真实测试晚到结果与最后租约释放的竞争。
  barrier(2);
  return response(std::move(result));
}
char *msime_client_telemetry_clear(const uint8_t *, size_t) {
  std::lock_guard<std::mutex> guard(lock);
  if (enabled) ++invalid;
  return response(!enabled);
}
void msime_client_string_free(char *value) { std::free(value); }
}
