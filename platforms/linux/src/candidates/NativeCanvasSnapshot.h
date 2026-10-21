#pragma once

#include "../core/JsonInteger.h"
#include <optional>

namespace msime::linux_host {

// 完整列表与当前页必须是同一代；逐项复核身份，不能用数组位置猜全局下标。
inline bool native_canvas_snapshot_current(const nlohmann::json &snapshot,
    std::uint64_t session, std::uint64_t generation) {
  if (!snapshot.is_object() || !snapshot.contains("session") || !snapshot.contains("generation") ||
      !snapshot.contains("candidates") || !snapshot["candidates"].is_array()) return false;
  try {
    if (strict_json_required_integer<std::uint64_t>(snapshot["session"]) != session ||
        strict_json_required_integer<std::uint64_t>(snapshot["generation"]) != generation) return false;
    std::optional<std::size_t> previous;
    for (const auto &candidate : snapshot["candidates"]) {
      if (!candidate.is_object() || !candidate.contains("id") || !candidate["id"].is_object() ||
          !candidate.contains("text") || !candidate["text"].is_string()) return false;
      const auto &id = candidate["id"];
      if (strict_json_required_integer<std::uint64_t>(id.at("session")) != session ||
          strict_json_required_integer<std::uint64_t>(id.at("generation")) != generation) return false;
      const auto index = strict_json_required_integer<std::size_t>(id.at("index"));
      if (previous && index <= *previous) return false;
      previous = index;
    }
    return true;
  } catch (...) { return false; }
}

} // msime::linux_host 命名空间
