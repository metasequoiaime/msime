#pragma once
#include "HostApiString.h"
#include <chrono>
#include <future>
#include <nlohmann/json.hpp>
#include <string>

namespace msime::cloud_quick_clear {
using Json = nlohmann::json;

// 原生窗口只传小型动作和一次性令牌；快照筛选、登录绑定与 CAS 归共享 Rust。
class Model {
public:
  explicit Model(Json envelope) : envelope_(std::move(envelope)) {}
  bool busy() const { return pending_.valid(); }
  bool poll() {
    if (!busy() || pending_.wait_for(std::chrono::milliseconds(0)) != std::future_status::ready) return false;
    try {
      auto response = pending_.get();
      error = response.value("ok", false) ? std::string{} : response.value("error", std::string("snapshot_unavailable"));
      value = error.empty() ? response.at("value") : Json();
    } catch (...) { error = "snapshot_unavailable"; value = Json(); }
    return true;
  }
  void preview() { perform({{"operation", "quick_clear_preview"}}); }
  void apply(const std::string &token) { perform({{"operation", "quick_clear_apply"}, {"token", token}}); }
  void cancel() { perform({{"operation", "quick_clear_cancel"}}); }
  std::string operation, error;
  Json value;
  static const char *message(const std::string &code) {
    if (code == "account_unauthorized") return "请先在账号设置中登录。";
    if (code == "account_conflict" || code == "snapshot_conflict") return "云端词库已变化，请重新预览。";
    if (code == "account_cancelled" || code == "snapshot_invalid") return "预览已失效，请重新预览。";
    return "云端快捷短语操作失败，请稍后重试。";
  }
private:
  void perform(Json action) {
    if (busy()) return;
    operation = action.at("operation").get<std::string>(); error.clear(); value = Json();
    auto envelope = envelope_; envelope["action"] = std::move(action);
    pending_ = std::async(std::launch::async, [envelope = std::move(envelope)] {
      const auto bytes = envelope.dump();
      auto response = host_api::own_string(msime_client_cloud_dictionary(
          reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
      if (!response) return Json{{"ok", false}, {"error", "snapshot_unavailable"}};
      return Json::parse(response.get());
    });
  }
  Json envelope_;
  std::future<Json> pending_;
};
} // 命名空间 msime::cloud_quick_clear
