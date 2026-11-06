#pragma once

#include "HostApiString.h"
#include <atomic>
#include <chrono>
#include <functional>
#include <future>
#include <nlohmann/json.hpp>
#include <string>
#include <vector>

namespace msime::translation {

// 原生窗口的异步适配器。校验、偏好修改与模型安装仍全部调用共享 Rust。
class SettingsModel {
public:
  using Json = nlohmann::json;
  explicit SettingsModel(std::string directory) : directory_(std::move(directory)) { refresh(); }
  ~SettingsModel() { cancel(); }
  SettingsModel(const SettingsModel &) = delete;
  SettingsModel &operator=(const SettingsModel &) = delete;
  bool busy() const { return pending_.valid(); }
  bool installing() const { return busy() && static_cast<bool>(progress_); }
  double progress() const {
    if (!progress_ || !progress_->total) return 0;
    return 100.0 * static_cast<double>(progress_->done.load()) / static_cast<double>(progress_->total.load());
  }
  bool poll() {
    if (!busy() || pending_.wait_for(std::chrono::milliseconds(0)) != std::future_status::ready) return false;
    try {
      auto result = pending_.get();
      view = result.value("view", Json::object());
      notice = result.value("error", std::string{});
    } catch (...) { notice = "unavailable"; }
    progress_.reset();
    return true;
  }
  void refresh() { start([] { return Json{{"ok", true}}; }); }
  void save(bool enabled, const std::string &language) {
    if (!view.contains("settings")) return;
    auto request = Json{{"directory", directory_}, {"update", {
        {"expected_revision", view.at("settings").at("revision")},
        {"enabled", enabled}, {"target_language", language}}}};
    start([request] { return call(msime_client_local_translation_settings, request); });
  }
  void install(const std::vector<std::string> &files = {}) {
    if (busy()) return;
    progress_ = std::make_shared<Progress>();
    auto state = progress_;
    auto request = Json{{"state_root", directory_}, {"pack", "local-translation"}};
    if (!files.empty()) request["files"] = files;
    start([request, state] {
      const auto bytes = request.dump();
      return read(msime_client_resource_pack_install(reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size(), on_progress, state.get()));
    });
  }
  void remove() {
    if (!view.contains("settings")) return;
    auto request = Json{{"state_root", directory_}, {"pack", "local-translation"}};
    auto disable = Json{{"directory", directory_}, {"update", {
        {"expected_revision", view.at("settings").at("revision")}, {"enabled", false},
        {"target_language", view.at("settings").at("local_translation").at("target_language")}}}};
    start([request, disable] {
      auto saved = call(msime_client_local_translation_settings, disable);
      return saved.value("ok", false) ? call(msime_client_resource_pack_remove, request) : saved;
    });
  }
  void cancel() {
    if (!progress_) return;
    progress_->cancelled = true;
    constexpr char pack[] = "local-translation";
    msime_client_resource_pack_cancel(reinterpret_cast<const uint8_t *>(pack), sizeof(pack) - 1);
  }
  Json view = Json::object();
  std::string notice;

private:
  struct Progress {
    std::atomic<bool> cancelled{false};
    std::atomic<uint64_t> done{0}, total{0};
  };
  static void on_progress(void *context, const char *, uint64_t done, uint64_t total) noexcept {
    auto &state = *static_cast<Progress *>(context);
    state.done = done; state.total = total;
    if (state.cancelled) {
      constexpr char pack[] = "local-translation";
      msime_client_resource_pack_cancel(reinterpret_cast<const uint8_t *>(pack), sizeof(pack) - 1);
    }
  }
  static Json read(char *bytes) {
    auto owned = host_api::own_string(bytes);
    return owned ? Json::parse(owned.get()) : Json{{"ok", false}, {"error", "unavailable"}};
  }
  static Json call(char *(*function)(const uint8_t *, size_t), const Json &request) {
    const auto bytes = request.dump();
    return read(function(reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
  }
  static Json load(const std::string &directory) {
    auto settings = call(msime_client_local_translation_settings, Json{{"directory", directory}});
    auto catalog = read(msime_client_local_translation_catalog());
    auto packs = call(msime_client_resource_packs, Json{{"state_root", directory}});
    if (!settings.value("ok", false) || !catalog.value("ok", false) || !packs.value("ok", false)) return Json::object();
    bool installed = false;
    for (const auto &pack : packs.at("value"))
      if (pack.at("id") == "local-translation") installed = pack.at("state") == "installed";
    return Json{{"settings", settings.at("value")}, {"catalog", catalog.at("value")}, {"installed", installed}};
  }
  void start(std::function<Json()> operation) {
    if (busy()) return;
    notice.clear();
    try { pending_ = std::async(std::launch::async, [directory = directory_, operation = std::move(operation)] {
      auto result = operation();
      auto current = load(directory);
      auto error = result.value("ok", false) ? std::string{} : result.value("error", std::string("unavailable"));
      if (current.empty() && error.empty()) error = "unavailable";
      return Json{{"view", current}, {"error", error}};
    }); } catch (...) { progress_.reset(); notice = "unavailable"; }
  }
  std::string directory_;
  std::shared_ptr<Progress> progress_;
  std::future<Json> pending_;
};

} // namespace msime::translation
