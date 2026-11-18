#pragma once

#include "TypingStatistics.h"
#include <nlohmann/json.hpp>

namespace msime::linux_host {
// 关闭态不读取旧本地模式；会话尚未交出视图时不查询来源，保留未知。
template <class Resolve>
TypingSource typing_key_press_source(bool input_enabled,
                                     const nlohmann::json &view,
                                     Resolve current_source) {
  if (!input_enabled)
    return TypingSource::English;
  if (!view.is_object() || !view.contains("scheme"))
    return TypingSource::Unknown;
  return current_source();
}

// 两个宿主只写出按下时聚合的快照，不在后台重新读取输入模式。
inline nlohmann::json typing_key_press_request(const KeyPressBatch &batch) {
  return {{"directory", batch.directory},
          {"action", {{"operation", "record_keys"},
                      {"day", batch.day},
                      {"keys", batch.keys},
                      {"sources", batch.sources}}}};
}
} // namespace msime::linux_host
