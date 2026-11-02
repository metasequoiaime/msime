#pragma once

#include "StateDirectory.h"
#include "../src/system/StateRootLease.h"
#include "../../common/Telemetry.h"
#include <nlohmann/json.hpp>

namespace msime::windows {
// 与 Server/设置使用相同的版本遥测目录和运行时偏好重定向；损坏配置不以默认同意替代。
inline std::optional<telemetry::Host> auxiliary_crash_host(
    const std::string &component, const std::string &version,
    const std::filesystem::path &state) {
  const auto base = telemetry::default_directory();
  if (base.empty() || !state.is_absolute())
    return std::nullopt;
  auto preferences = state;
  const auto options = state / "runtime-options.json";
  std::error_code error;
  if (std::filesystem::exists(options, error)) {
    const auto raw = read_private_file(options, 1024 * 1024);
    if (!raw)
      return std::nullopt;
    const auto document = nlohmann::json::parse(*raw, nullptr, false);
    if (!document.is_object())
      return std::nullopt;
    const auto path = document.find("preferences_directory");
    if (path != document.end()) {
      if (!path->is_string())
        return std::nullopt;
      auto configured = std::filesystem::u8path(path->get<std::string>());
      if (!configured.is_absolute())
        return std::nullopt;
      preferences = std::move(configured);
    }
  } else if (error) {
    return std::nullopt;
  }
  return telemetry::Host{"windows", version,
                         base.parent_path() / MSIME_EDITION_USER_DATA_DIRECTORY,
                         std::nullopt, preferences, component, true,
                         MSIME_EDITION_ID};
}
} // namespace msime::windows
