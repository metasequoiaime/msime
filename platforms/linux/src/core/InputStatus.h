#pragma once

#include <filesystem>
#include <optional>
#include <string>
#include <string_view>

#include <nlohmann/json.hpp>

#include "../candidates/CandidatePanelStatus.h"

namespace msime::linux_host {

// A desktop without a tray has nowhere to show the input mode: Omarchy starts fcitx5 with --disable notificationitem, so the status area and its 中/英 label never appear. The Fcitx5 addon therefore also writes the focused context's mode to a per-session file the MSIME bar widget for Omarchy reads (data/omarchy/plugin): {"active":bool,"label":"中"|"英"|"粤"|"注"|"笔"|"日"|"한"|"越"|"⇪","scheme":"<preferences scheme id>"}. "active" is false once MSIME no longer holds the focused context, so the widget can step aside for whatever input method does.
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
