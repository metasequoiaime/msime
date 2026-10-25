#include "HostConfiguration.h"
#include "AsyncSnapshotCache.h"
#include "HostOptionsPaths.h"
#include "../../common/HostApiString.h"
#include "../src/system/HelpcodeRoot.h"
#include <atomic>
#include <filesystem>
#include <functional>
#include <msime_client.h>
#include <windows.h>

namespace msime::tsf {
namespace {
AsyncSnapshotCache<HostConfiguration> configuration;
// 首次初始化已经同步读过的配置只供紧接着的激活决策消费一次；后续激活仍重新读取最新规则。
std::atomic<bool> initialSnapshotPending{false};

std::shared_ptr<const nlohmann::json> LoadPreferences(const std::string &directory, bool initial) {
  const auto *bytes = reinterpret_cast<const uint8_t *>(directory.data());
  auto raw = msime::host_api::own_string(initial
      ? msime_client_load_preferences(bytes, directory.size())
      : msime_client_try_load_preferences(bytes, directory.size()));
  if (!raw) return {};
  const auto envelope = nlohmann::json::parse(raw.get(), nullptr, false);
  if (!envelope.is_object() || !envelope.value("ok", false)) return {};
  const auto value = envelope.find("value");
  if (value == envelope.end() || !value->is_object() ||
      !value->contains("preferences") || !(*value)["preferences"].is_object()) return {};
  return std::make_shared<const nlohmann::json>(*value);
}

std::shared_ptr<const HostConfiguration> LoadConfiguration(
    const std::shared_ptr<const HostConfiguration> &previous, bool initial) {
  auto next = previous ? std::make_shared<HostConfiguration>(*previous)
                       : std::make_shared<HostConfiguration>();
  if (next->directory.empty()) next->directory = default_state_directory();
  if (next->directory.empty()) return previous;
  if (const auto value = LoadPreferences(next->directory, initial)) {
    const auto revision = value->find("revision");
    if (revision != value->end() && revision->is_number_unsigned() &&
        (!previous || revision->get<std::uint64_t>() >= previous->revision)) {
      next->revision = revision->get<std::uint64_t>();
      next->preferences = (*value)["preferences"];
      next->preferenceSnapshot = value->dump();
    }
  }
  const auto options = read_prepared_host_options(
      std::filesystem::u8path(next->directory) / L"runtime-options.json");
  const auto parsed = nlohmann::json::parse(options, nullptr, false);
  if (parsed.is_object()) {
    // The TIP creates its sessions from this document, as the Server does from the one it prepared; both name the built-in helpcode tables in DataDir the same way, in memory only.
    auto session = parsed;
    msime::windows::name_builtin_helpcodes(session, std::filesystem::u8path(next->directory));
    next->hostOptions = session == parsed ? options : session.dump();
    const auto user = parsed.find("user_data");
    next->userData = user != parsed.end() && user->is_string()
                         ? std::filesystem::u8path(user->get<std::string>())
                         : std::filesystem::path{};
    const auto directory = parsed.find("language_dictionaries");
    if (directory != parsed.end() && directory->is_string()) {
      const auto path = std::filesystem::u8path(directory->get<std::string>());
      const auto probe = [&path](const char *name, bool &present) {
        std::error_code error;
        const bool found = std::filesystem::is_regular_file(path / name, error);
        // 文件不存在是确定的不可用；其他 I/O 错误保留上次结果。
        if (!error || error == std::errc::no_such_file_or_directory) present = found;
      };
      probe("msime-cantonese.db", next->dictionaries.cantonese);
      probe("msime-zhuyin.db", next->dictionaries.zhuyin);
      probe("msime-stroke.db", next->dictionaries.stroke);
    } else {
      next->dictionaries = {};
    }
  }
  return next;
}

struct RefreshTask {
  HMODULE module;
  std::function<void()> run;
};

void CALLBACK RefreshConfiguration(PTP_CALLBACK_INSTANCE instance, void *context) {
  std::unique_ptr<RefreshTask> task(static_cast<RefreshTask *>(context));
  // 直到回调返回才释放加载引用，不保留 TIP、COM 或引擎会话指针。
  FreeLibraryWhenCallbackReturns(instance, task->module);
  task->run();
}

bool ScheduleRefresh(std::function<void()> run) {
  auto task = std::make_unique<RefreshTask>();
  task->run = std::move(run);
  if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                          reinterpret_cast<LPCWSTR>(&RefreshConfiguration), &task->module)) return false;
  if (!TrySubmitThreadpoolCallback(RefreshConfiguration, task.get(), nullptr)) {
    FreeLibrary(task->module);
    return false;
  }
  task.release();
  return true;
}
}

void initialize_host_configuration() {
  configuration.initialize([](const auto &previous) {
    auto next = LoadConfiguration(previous, true);
    initialSnapshotPending.store(true, std::memory_order_release);
    return next;
  });
}
std::shared_ptr<const HostConfiguration> cached_host_configuration() {
  return configuration.snapshot();
}
void refresh_host_configuration() {
  configuration.refresh([](const auto &previous) { return LoadConfiguration(previous, false); }, ScheduleRefresh);
}
void invalidate_host_configuration() {
  configuration.invalidate();
  refresh_host_configuration();
}
std::shared_ptr<const nlohmann::json> load_activation_preferences() {
  if (initialSnapshotPending.exchange(false, std::memory_order_acquire)) {
    if (const auto snapshot = cached_host_configuration();
        snapshot && snapshot->preferences.is_object()) {
      return {snapshot, &snapshot->preferences};
    }
  }
  try {
    const auto directory = default_state_directory();
    if (directory.empty()) return {};
    const auto snapshot = LoadPreferences(directory, true);
    if (snapshot) return {snapshot, &(*snapshot)["preferences"]};
  } catch (...) {}
  return {};
}

}
