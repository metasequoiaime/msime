#pragma once

#include "../../../common/HostApiString.h"
#include <nlohmann/json.hpp>
#include <string>

namespace msime::linux_host {

// 只交出目录并创建 Rust 工作线程；此处不读取配置或联网。
inline bool start_settings_sync(const std::string &directory) {
  if (directory.empty() || directory.front() != '/') return false;
  try {
    const auto request = nlohmann::json{{"preferences_directory", directory}}.dump();
    auto response = host_api::own_string(msime_client_settings_sync_start(
        reinterpret_cast<const uint8_t *>(request.data()), request.size()));
    if (!response) return false;
    const auto envelope = nlohmann::json::parse(response.get(), nullptr, false);
    return envelope.is_object() && envelope.value("ok", false);
  } catch (...) {
    return false;
  }
}

} // namespace msime::linux_host
