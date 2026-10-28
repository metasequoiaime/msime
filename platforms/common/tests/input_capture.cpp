// 输入宿主使用真实偏好、队列和故障处理器；只替代发送入口，绝不访问产品网络。
#include "Telemetry.h"
#include "CrashRecorder.h"
#include "HostApiString.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <chrono>
#include <condition_variable>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <thread>
#ifdef _WIN32
#include <windows.h>
#else
#include <sys/mman.h>
#include <unistd.h>
#endif

namespace {
using Json = nlohmann::json;
std::mutex sending_mutex;
std::condition_variable sending_changed;
bool sending_entered = false;
bool sending_released = false;
Json sent;
std::mutex preparing_mutex;
std::condition_variable preparing_changed;
bool preparing_entered = false;
bool preparing_released = false;
bool preparing_paused = false;
std::filesystem::path denied_preferences;

Json result(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned) throw std::runtime_error("synthetic empty host response");
  const auto value = Json::parse(owned.get());
  if (!value.value("ok", false)) throw std::runtime_error("synthetic rejected host response");
  return value.at("value");
}

void save(const std::filesystem::path &directory, bool enabled) {
  const auto path = directory.u8string();
  auto snapshot = result(msime_client_load_preferences(
      reinterpret_cast<const uint8_t *>(path.data()), path.size()));
  snapshot["preferences"]["usage_reporting"] = enabled;
  const auto document = snapshot.dump();
  result(msime_client_save_preferences(
      reinterpret_cast<const uint8_t *>(path.data()), path.size(), snapshot.at("revision").get<uint64_t>(),
      reinterpret_cast<const uint8_t *>(document.data()), document.size()));
}

Json capture_status(const std::filesystem::path &directory) {
  std::ifstream file(directory / "telemetry-session.json");
  Json marker = file ? Json::parse(file) : Json::object();
  return Json{{"armed", msime::crash::is_armed()}, {"marker", marker}};
}

[[noreturn]] void fault() {
#ifdef _WIN32
  auto *page = VirtualAlloc(nullptr, 4096, MEM_RESERVE | MEM_COMMIT, PAGE_NOACCESS);
  if (!page) std::exit(3);
#else
  auto *page = mmap(nullptr, static_cast<size_t>(sysconf(_SC_PAGESIZE)), PROT_NONE,
                    MAP_PRIVATE | MAP_ANON, -1, 0);
  if (page == MAP_FAILED) std::exit(3);
#endif
  *static_cast<volatile char *>(page) = 7;
  std::exit(4);
}
} // namespace

// 真实 begin 完成并释放偏好锁后才暂停，确定性覆盖回包前关闭或换代。
extern "C" char *msime_test_telemetry_begin(const uint8_t *request, size_t length) {
  char *response = msime_client_telemetry_begin(request, length);
  std::unique_lock lock(preparing_mutex);
  if (preparing_paused) {
    preparing_entered = true;
    preparing_changed.notify_all();
    preparing_changed.wait(lock, [] { return preparing_released; });
  }
  return response;
}

// 返回真实 Rust 所有权的关闭响应，模拟旧请求迟到；只检验 C++ 请求身份与收尾，逐条发送由 Rust 测试覆盖。
extern "C" char *msime_test_telemetry_flush(const uint8_t *request, size_t length) {
  {
    std::unique_lock lock(sending_mutex);
    sent = Json::parse(request, request + length);
    if (!sent.contains("consent_epoch") || sent.contains("enabled") || !sent.contains("preferences_directory"))
      std::exit(6);
    sending_entered = true;
    sending_changed.notify_all();
    sending_changed.wait(lock, [] { return sending_released; });
  }
  const auto body = Json{{"directory", denied_preferences.u8string()}}.dump();
  return msime_client_telemetry_consent(reinterpret_cast<const uint8_t *>(body.data()), body.size());
}

int main(int argc, char **argv) {
  if (argc != 4) return 2;
#ifdef _WIN32
  SetErrorMode(SEM_NOGPFAULTERRORBOX | SEM_FAILCRITICALERRORS);
  const std::string platform = "windows";
#elif defined(__APPLE__)
  const std::string platform = "macos";
#else
  const std::string platform = "linux";
#endif
  const std::string mode = argv[1];
  const auto directory = std::filesystem::u8path(argv[2]);
  const auto preferences = std::filesystem::u8path(argv[3]);
  const bool independent = mode == "independent";
  const msime::telemetry::Host host{platform, mode == "collect" ? "9.9.9" : "1.2.3", directory,
      false, preferences, mode == "collect" ? "collector" : independent ? "settings" : "input",
      mode == "collect" || independent};
  if (mode == "collect") {
    msime::telemetry::begin(host);
    msime::telemetry::end();
    return 0;
  }
  if (mode != "manual" && mode != "observe" && !independent) return 2;
  denied_preferences = preferences.parent_path() / "denied-preferences";
  save(denied_preferences, false);
  msime::telemetry::install_crash_handlers();
  msime::telemetry::begin(host);
  std::optional<msime::telemetry::ConsentObserver> observer;
  if (mode == "observe" || independent) observer.emplace();
  std::set_terminate([] { msime::telemetry::record_terminate(); std::abort(); });
  std::thread network;
  std::thread preparation;
  std::cout << std::unitbuf << capture_status(directory).dump() << '\n';
  std::string command;
  while (std::getline(std::cin, command)) {
    if (command == "fault") fault();
    if (command == "terminate") throw std::runtime_error("synthetic-private-input");
    if (command == "enable" || command == "disable") {
      save(preferences, command == "enable");
      std::cout << "saved\n";
    } else if (command == "callback-off") {
      msime::telemetry::set_enabled(false);
      std::cout << capture_status(directory).dump() << '\n';
    } else if (command == "status" || command == "refresh") {
      if (command == "refresh") msime::telemetry::refresh();
      std::cout << capture_status(directory).dump() << '\n';
    } else if (command == "prepare-start") {
      if (preparation.joinable()) return 2;
      {
        std::lock_guard guard(preparing_mutex);
        preparing_paused = true;
      }
      preparation = std::thread([] { msime::telemetry::refresh(); });
      std::unique_lock lock(preparing_mutex);
      if (!preparing_changed.wait_for(lock, std::chrono::seconds(10), [] { return preparing_entered; }))
        std::exit(8);
      std::cout << "prepared\n";
    } else if (command == "prepare-release") {
      {
        std::lock_guard guard(preparing_mutex);
        preparing_released = true;
      }
      preparing_changed.notify_all();
      if (preparation.joinable()) preparation.join();
      std::cout << capture_status(directory).dump() << '\n';
    } else if (command == "flush-start") {
      if (network.joinable()) return 2;
      network = std::thread([] { msime::telemetry::flush(); });
      std::unique_lock lock(sending_mutex);
      if (!sending_changed.wait_for(lock, std::chrono::seconds(10), [] { return sending_entered; }))
        std::exit(7);
      std::cout << sent.dump() << '\n';
    } else if (command == "flush-release") {
      {
        std::lock_guard guard(sending_mutex);
        sending_released = true;
      }
      sending_changed.notify_all();
      if (network.joinable()) network.join();
      std::cout << capture_status(directory).dump() << '\n';
    } else if (command == "observer-stop") {
      observer.reset();
      msime::telemetry::end();
      std::cout << capture_status(directory).dump() << '\n';
    } else if (command == "stop") {
      break;
    } else {
      return 2;
    }
  }
  if (preparation.joinable()) {
    {
      std::lock_guard guard(preparing_mutex);
      preparing_released = true;
    }
    preparing_changed.notify_all();
    preparation.join();
  }
  if (network.joinable()) {
    {
      std::lock_guard guard(sending_mutex);
      sending_released = true;
    }
    sending_changed.notify_all();
    network.join();
  }
  observer.reset();
  msime::telemetry::end();
  std::cout << capture_status(directory).dump() << '\n';
  return 0;
}
