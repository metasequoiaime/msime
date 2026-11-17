#pragma once
#include "KeyTypingSource.h"
#include <nlohmann/json.hpp>

namespace msime::tsf {
// JSON 依赖只留在响应提取处；公开的适配器和轻量快照不依赖解析器。
inline void observe_key_typing_source(KeyTypingSource &source,
                                      const nlohmann::json &value) {
  const auto field = value.find("view");
  const auto &view =
      field != value.end() && field->is_object() ? *field : value;
  if (!view.is_object() || !view.contains("scheme") ||
      !view.contains("local_mode"))
    return;
  source.observe(view.value("scheme", -1), view.value("nine_key", false),
                 view.value("dedicated_english", false),
                 view.value("local_mode", std::string("none")),
                 view.value("shuangpin_profile", std::string("xiaohe")));
}
} // namespace msime::tsf
