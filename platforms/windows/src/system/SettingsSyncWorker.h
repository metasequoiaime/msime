#pragma once

#include "../../../common/HostApiString.h"
#include <nlohmann/json.hpp>

namespace msime::windows {

// Server 只管理共享线程的生命周期；预览实例不启动后台同步。
class SettingsSyncWorker {
public:
  SettingsSyncWorker() = default;
  SettingsSyncWorker(const SettingsSyncWorker &) = delete;
  SettingsSyncWorker &operator=(const SettingsSyncWorker &) = delete;
  ~SettingsSyncWorker() { stop(); }

  bool start(bool production, const nlohmann::json &host_options) noexcept {
    if (!production || started_) return started_;
    try {
      const auto directory = host_options.find("preferences_directory");
      if (directory == host_options.end() || !directory->is_string() ||
          directory->get_ref<const std::string &>().empty()) return false;
      const auto request = nlohmann::json{{"preferences_directory", *directory}}.dump();
      auto response = host_api::own_string(msime_client_settings_sync_start(
          reinterpret_cast<const uint8_t *>(request.data()), request.size()));
      if (!response) return false;
      const auto envelope = nlohmann::json::parse(response.get(), nullptr, false);
      const auto ok = envelope.is_object() ? envelope.find("ok") : envelope.end();
      started_ = envelope.is_object() && ok != envelope.end() &&
                 ok->is_boolean() && ok->get<bool>();
    } catch (...) {
      started_ = false;
    }
    return started_;
  }

  void stop() noexcept {
    // 当前网络请求由 Rust 收尾；超时后保留所有权，析构时仍可再等一次。
    if (started_ && msime_client_settings_sync_stop(2000)) started_ = false;
  }

private:
  bool started_ = false;
};

} // 命名空间 msime::windows
