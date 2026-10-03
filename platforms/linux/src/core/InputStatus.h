#pragma once

#include <filesystem>
#include <optional>
#include <string>
#include <string_view>

#include <nlohmann/json.hpp>

#include "../candidates/CandidatePanelStatus.h"

namespace msime::linux_host {

// 没有托盘的桌面无处显示输入模式：Omarchy 用 --disable notificationitem 启动 fcitx5，所以状态区和它的 中/英 标签都不会出现。因此 Fcitx5 插件还把当前聚焦上下文的模式写进一个按会话区分的文件，供 Omarchy 的水杉输入法状态栏组件读取（data/omarchy/plugin）：{"active":bool,"label":"中"|"英"|"粤"|"注"|"日"|"한"|"越"|"藏"|"⇪","scheme":"<偏好方案 id>"}。水杉输入法不再持有聚焦上下文时 "active" 为 false，组件就可以让位给接手的输入法。
inline std::optional<std::filesystem::path> input_status_file(const char *runtime) {
  if (!runtime || runtime[0] != '/') return std::nullopt;
  return std::filesystem::path(runtime) / "msime-client" / "input-status.json";
}

inline std::string input_status_document(bool active, std::string_view label, std::string_view scheme) {
  return nlohmann::json{{"active", active}, {"label", label}, {"scheme", scheme}}.dump() + "\n";
}

// Called on every key event through the mode indicator, so a document already written by this process is not written, or even compared on disk, again.
inline void publish_input_status(const char *runtime, const std::string &document) {
  static std::string published;
  if (document == published) return;
  const auto file = input_status_file(runtime);
  if (!file || !write_candidate_panel_status(*file, document)) return;
  published = document;
}

} // namespace msime::linux_host
