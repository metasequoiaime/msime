#pragma once
#include "HostApiString.h"
#include <chrono>
#include <functional>
#include <future>
#include <nlohmann/json.hpp>
#include <string>

namespace msime::dictionary_collections {
using Json = nlohmann::json;

// 三个桌面原生窗口的异步传输适配；集合规则和输入会话协调仍归共享 Rust。
class Model {
public:
  explicit Model(Json options) : options_(std::move(options)) { perform({{"operation", "load"}}); }
  bool busy() const { return pending_.valid(); }
  bool poll() {
    if (!busy() || pending_.wait_for(std::chrono::milliseconds(0)) != std::future_status::ready) return false;
    try {
      const auto reply = pending_.get();
      if (reply.contains("view")) view = reply.at("view");
      error = reply.value("error", std::string{});
      remaining = reply.value("remaining", 0u);
      if (reply.contains("import")) report = reply.at("import");
    } catch (...) { error = "unavailable"; }
    if (remaining > 0 && error.empty()) synchronize();
    return true;
  }
  void perform(Json action) {
    if (busy()) return;
    error.clear(); report = Json();
    start([options = options_, action = std::move(action)] {
      const auto reply = call(msime_client_dictionary_collections, {{"options", options}, {"action", action}});
      if (!reply.value("ok", false)) return Json{{"error", reply.value("error", std::string("unavailable"))}};
      Json result{{"view", reply.at("value")}};
      if (reply.at("value").contains("import")) result["import"] = reply.at("value").at("import");
      const auto drained = drain(options);
      result.update(drained);
      return result;
    });
  }
  Json view = {{"collections", Json::array()}};
  Json report;
  std::string error;
  unsigned remaining = 0;

  static const char *source_title(const std::string &source) {
    return source == "community" ? "社区词库" : source == "import" ? "文件导入" : "自建词库";
  }
  static std::string message(const std::string &code) {
    if (code.empty()) return {};
    if (code == "collections_name_invalid") return "分组名称须为 1–32 个字";
    if (code == "collections_source_conflict") return "此分组不能接收该来源，请新建分组导入";
    if (code == "collections_limit") return "最多保存 32 个分组";
    if (code == "collections_corrupt") return "分组数据损坏，原文件未改动";
    if (code == "collections_not_found") return "分组已不存在，请刷新";
    if (code == "dictionary maintenance busy") return "词条已排队；输入法正忙，请稍后刷新继续写入";
    if (code == "collections_too_large") return "文件或分组超出容量限制";
    return "词库操作失败，请重试";
  }
private:
  using Call = char *(*)(const uint8_t *, size_t);
  static Json call(Call function, const Json &request) {
    const auto bytes = request.dump();
    auto value = host_api::own_string(function(reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
    return value ? Json::parse(value.get()) : Json{{"ok", false}, {"error", "unavailable"}};
  }
  static Json drain(const Json &options) {
    const auto reply = call(msime_client_dictionary_collections,
      {{"options", options}, {"action", {{"operation", "synchronize_settings"}, {"budget_millis", 100}}}});
    if (!reply.value("ok", false)) return {{"error", reply.value("error", std::string("unavailable"))}};
    const auto current = call(msime_client_dictionary_collections, {{"options", options}, {"action", {{"operation", "load"}}}});
    Json result{{"remaining", reply.at("value").value("pending_count", 0u)}};
    if (current.value("ok", false)) result["view"] = current.at("value");
    return result;
  }
  void synchronize() { start([options = options_] { return drain(options); }); }
  void start(std::function<Json()> work) {
    try { pending_ = std::async(std::launch::async, std::move(work)); }
    catch (...) { error = "unavailable"; }
  }
  Json options_;
  std::future<Json> pending_;
};
} // namespace msime::dictionary_collections
