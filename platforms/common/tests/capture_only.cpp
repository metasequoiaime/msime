#include "Telemetry.h"
#include "CrashRecorder.h"
#include "HostApiString.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <mutex>
#include <cstdlib>
#include <iostream>
#include <stdexcept>
#include <string>
#ifdef _WIN32
#include "../../windows/common/AuxiliaryCrashCapture.h"
#else
#include <sys/mman.h>
#include <unistd.h>
#endif

namespace {
struct Recovered {};
std::mutex arming_mutex;
std::condition_variable arming_changed;
bool arming_paused = false;
bool arming_entered = false;
bool arming_released = false;
bool refresh_completed = false;
bool checkpoint_released = false;

void release_checkpoint() {
  {
    std::lock_guard guard(arming_mutex);
    arming_released = true;
    checkpoint_released = true;
  }
  arming_changed.notify_all();
}
nlohmann::json result(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned)
    throw std::runtime_error("synthetic host call failed");
  const auto response = nlohmann::json::parse(owned.get());
  if (!response.value("ok", false))
    throw std::runtime_error("synthetic host call rejected");
  return response.at("value");
}

void save(const std::string &directory, bool enabled, bool initialize_after_load = false) {
  const auto request = nlohmann::json{{"directory", directory}}.dump();
  const auto initialize = [&] {
    result(msime_client_telemetry_consent(reinterpret_cast<const uint8_t *>(request.data()), request.size()));
  };
  // 先初始化持续授权，再读取 CAS 修订号；后台观察器不能在首次 load 和 save 之间抢先升版。
  initialize();
  auto snapshot = result(msime_client_load_preferences(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size()));
  if (initialize_after_load)
    initialize();
  snapshot["preferences"]["usage_reporting"] = enabled;
  const auto document = snapshot.dump();
  result(msime_client_save_preferences(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size(),
      snapshot.at("revision").get<uint64_t>(),
      reinterpret_cast<const uint8_t *>(document.data()), document.size()));
}

[[noreturn]] void fault() {
#ifdef _WIN32
  auto *page = VirtualAlloc(nullptr, 4096, MEM_RESERVE | MEM_COMMIT, PAGE_NOACCESS);
  if (!page)
    std::exit(3);
#else
  auto *page = mmap(nullptr, static_cast<size_t>(sysconf(_SC_PAGESIZE)), PROT_NONE,
                    MAP_PRIVATE | MAP_ANON, -1, 0);
  if (page == MAP_FAILED)
    std::exit(3);
#endif
  *static_cast<volatile char *>(page) = 7;
  std::exit(4);
}
} // namespace

// 在最终武装之前暂停，再在该轮刷新结束后保持现场；两端都不依赖睡眠。
extern "C" void msime_test_capture_before_arm() {
  std::unique_lock lock(arming_mutex);
  if (!arming_paused)
    return;
  arming_entered = true;
  arming_changed.notify_all();
  arming_changed.wait(lock, [] { return arming_released; });
}

extern "C" void msime_test_capture_after_refresh() {
  std::unique_lock lock(arming_mutex);
  if (!arming_entered || !arming_released)
    return;
  refresh_completed = true;
  arming_changed.notify_all();
  arming_changed.wait(lock, [] { return checkpoint_released; });
  arming_paused = false;
}

int main(int argc, char **argv) {
  if (argc != 5 && argc != 6)
    return 2;
#ifdef _WIN32
  SetErrorMode(SEM_NOGPFAULTERRORBOX | SEM_FAILCRITICALERRORS);
  const std::string platform = "windows";
  if (std::string(argv[1]) == "describe") {
    const auto host = msime::windows::auxiliary_crash_host(argv[4], "1.2.3", std::filesystem::u8path(argv[3]));
    if (!host) {
      std::cout << "null\n";
    } else {
      std::cout << nlohmann::json{{"directory", host->directory.u8string()},
                                  {"preferences", host->preferences_directory.u8string()},
                                  {"component", host->component}, {"capture_only", host->capture_only}}.dump() << '\n';
    }
    return 0;
  }
#else
#ifdef __APPLE__
  const std::string platform = "macos";
#else
  const std::string platform = "linux";
#endif
#endif
  const std::string directory = argv[2];
  const std::string preferences = argv[3];
  const std::string component = argv[4];
  const std::string edition = argc == 6 ? argv[5] : "";
  if (std::string(argv[1]) == "enable" || std::string(argv[1]) == "disable" ||
      std::string(argv[1]) == "enable-after-consent") {
    save(preferences, std::string(argv[1]) != "disable", std::string(argv[1]) == "enable-after-consent");
    return 0;
  }
  if (std::string(argv[1]) == "collect") {
    const auto consent_request = nlohmann::json{{"directory", preferences}}.dump();
    const auto consent = result(msime_client_telemetry_consent(
        reinterpret_cast<const uint8_t *>(consent_request.data()), consent_request.size()));
    if (!consent.at("enabled").get<bool>())
      return 0;
    const auto body = nlohmann::json{{"directory", directory}, {"platform", platform},
                                    {"version", "9.9.9"}, {"component", "collector"},
                                    {"capture_only", true}, {"preferences_directory", preferences},
                                    {"consent_epoch", consent.at("epoch")}}.dump();
    result(msime_client_telemetry_begin(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    return 0;
  }
  if (std::string(argv[1]) != "serve")
    return 2;
  std::atomic<int> configuration{0};
  std::mutex observation_lock;
  std::condition_variable observation_changed;
  bool pause_requested = false;
  bool pause_observed = false;
  std::cout << std::unitbuf;
  const auto previous_terminate = std::get_terminate();
  try {
    msime::telemetry::CaptureOnly capture([&]() -> std::optional<msime::telemetry::Host> {
      {
        std::unique_lock lock(observation_lock);
        if (pause_requested) {
          pause_observed = true;
          observation_changed.notify_all();
          observation_changed.wait(lock, [&] { return !pause_requested; });
          pause_observed = false;
        }
      }
      const int state = configuration.load();
      if (state == 1)
        return std::nullopt;
      if (state == 2)
        throw std::runtime_error("synthetic-private-configuration");
      // 故意提供相反的缓存同意，验证观察器始终使用真实共享偏好。
      return msime::telemetry::Host{platform, "1.2.3", std::filesystem::u8path(directory),
                                   false, std::filesystem::u8path(preferences), component, false,
                                   edition};
    });
    std::cout << (msime::crash::is_armed() ? "armed" : "off") << '\n';
    std::string command;
    while (std::getline(std::cin, command)) {
      if (command == "status") {
        std::cout << (msime::crash::is_armed() ? "armed" : "off") << '\n';
      } else if (command == "arm-block") {
        std::lock_guard guard(arming_mutex);
        arming_paused = true;
        arming_entered = false;
        arming_released = false;
        refresh_completed = false;
        checkpoint_released = false;
        std::cout << "blocked\n";
      } else if (command == "arm-wait") {
        std::unique_lock lock(arming_mutex);
        if (!arming_changed.wait_for(lock, std::chrono::seconds(10), [] { return arming_entered; }))
          std::exit(7);
        std::cout << "preparing\n";
      } else if (command == "arm-release") {
        std::unique_lock lock(arming_mutex);
        arming_released = true;
        arming_changed.notify_all();
        if (!arming_changed.wait_for(lock, std::chrono::seconds(10), [] { return refresh_completed; }))
          std::exit(8);
        std::cout << (msime::crash::is_armed() ? "armed" : "off") << '\n';
      } else if (command == "pause") {
        // 停在读取共享偏好之前，确定两次跨进程保存都发生在两次观察之间。
        std::unique_lock lock(observation_lock);
        pause_requested = true;
        if (!observation_changed.wait_for(lock, std::chrono::seconds(10), [&] { return pause_observed; })) {
          pause_requested = false;
          observation_changed.notify_all();
          return 6;
        }
        std::cout << "paused\n";
      } else if (command == "resume") {
        std::lock_guard lock(observation_lock);
        pause_requested = false;
        observation_changed.notify_all();
        std::cout << "resumed\n";
      } else if (command == "enable" || command == "disable") {
        save(preferences, command == "enable");
        std::cout << "saved\n";
      } else if (command == "unknown" || command == "throw" || command == "valid") {
        configuration = command == "unknown" ? 1 : command == "throw" ? 2 : 0;
        std::cout << "configured\n";
      } else if (command == "fault") {
        fault();
      } else if (command == "terminate") {
        throw std::runtime_error("synthetic-private-exception");
      } else if (command == "recover") {
        throw Recovered{};
      } else if (command == "stop") {
        release_checkpoint();
        {
          std::lock_guard lock(observation_lock);
          pause_requested = false;
        }
        observation_changed.notify_all();
        break;
      } else {
        return 2;
      }
    }
  } catch (const Recovered &) {
  }
  if (std::get_terminate() != previous_terminate)
    return 5;
  std::cout << (msime::crash::is_armed() ? "still-armed" : "stopped") << '\n';
  return 0;
}
