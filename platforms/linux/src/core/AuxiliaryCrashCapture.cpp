#include "AuxiliaryCrashCapture.h"
#include "FirstRunGuidance.h"
#include "RuntimeOptionsFile.h"

#include <cstdlib>
#include <nlohmann/json.hpp>
#include <utility>

namespace msime::linux_host {

std::optional<telemetry::Host> auxiliary_crash_host(
    const std::string &component, const std::filesystem::path &preferences,
    const std::filesystem::path &options_path) {
  auto directory = telemetry::default_directory();
  if (directory.empty())
    return std::nullopt;
  auto actual_preferences = preferences;
  if (actual_preferences.empty()) {
    const auto explicit_options = options_path.string();
    const char *override_path = options_path.empty() ? std::getenv("MSIME_CLIENT_HOST_OPTIONS")
                                                    : explicit_options.c_str();
    if (!override_path) {
      const char *ibus = std::getenv("MSIME_IBUS_OPTIONS");
      const char *fcitx = std::getenv("MSIME_FCITX5_OPTIONS");
      // 没有父进程的明确选择、两个框架又指定不同配置时，不猜测哪份同意属于自己。
      if (ibus && fcitx && std::string(ibus) != fcitx)
        return std::nullopt;
      override_path = ibus ? ibus : fcitx;
    }
    const auto located = locate_runtime_options(
        override_path, std::getenv("XDG_CONFIG_HOME"), std::getenv("HOME"),
        MSIME_SYSTEM_OPTIONS);
    if (located.state != RuntimeOptionsState::Found)
      return std::nullopt;
    const auto options = nlohmann::json::parse(read_runtime_options(located.path), nullptr, false);
    if (!options.is_object())
      return std::nullopt;
    const auto path = options.find("preferences_directory");
    if (path == options.end() || !path->is_string())
      return std::nullopt;
    actual_preferences = path->get<std::string>();
  }
  if (!actual_preferences.is_absolute())
    return std::nullopt;
  return telemetry::Host{"linux", MSIME_LINUX_VERSION,
                         directory.parent_path() / MSIME_EDITION_TELEMETRY_DIRECTORY,
                         std::nullopt, actual_preferences, component, true,
                         MSIME_EDITION_ID};
}

AuxiliaryCrashCapture::AuxiliaryCrashCapture(std::string component,
                                           std::filesystem::path preferences)
    : telemetry::CaptureOnly([component = std::move(component), preferences = std::move(preferences)] {
        return auxiliary_crash_host(component, preferences);
      }) {}

} // namespace msime::linux_host
