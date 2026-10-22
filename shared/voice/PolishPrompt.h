#pragma once
// 所有宿主保持现有命名空间，内置正文来自共享 Rust 目录的静态导出。
#include "PolishPromptCatalog.h"
#include <string>
#include <string_view>

namespace msime::windows {
// 对应偏好中的方案标识和三个自定义正文。
struct PolishPromptSlots {
  std::string id;
  std::string custom_1;
  std::string custom_2;
  std::string custom_3;
};

inline std::string_view polish_builtin_prompt(std::string_view id) {
  std::string_view cleanup;
  for (const auto &preset : kPolishPromptPresets) {
    if (preset.id == "cleanup") cleanup = preset.prompt;
    if (preset.id == id) return preset.prompt;
  }
  return cleanup;
}

inline std::string polish_prompt_for(const PolishPromptSlots &config) {
  // 空自定义槽位与未知方案都回退到 cleanup；空白正文仍按原样发送。
  const auto cleanup = polish_builtin_prompt("cleanup");
  if (config.id == "custom_1")
    return config.custom_1.empty() ? std::string(cleanup) : config.custom_1;
  if (config.id == "custom_2")
    return config.custom_2.empty() ? std::string(cleanup) : config.custom_2;
  if (config.id == "custom_3")
    return config.custom_3.empty() ? std::string(cleanup) : config.custom_3;
  return std::string(polish_builtin_prompt(config.id));
}

} // 命名空间 msime::windows
