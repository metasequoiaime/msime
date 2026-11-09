#pragma once

#include "InputSchemeTraits.h"

namespace msime::linux_host::character_set {

// 粤拼的字符集开关使用专用的简体输出偏好；其他方案沿用繁体输出偏好。
constexpr bool follows_cantonese(int scheme_number) {
  return scheme_number == scheme::Cantonese;
}

constexpr const char *preference_key(int scheme_number) {
  return follows_cantonese(scheme_number) ? "cantonese_simplified_output"
                                          : "traditional_chinese_output";
}

constexpr bool checked(int scheme_number, bool traditional_output,
                       bool cantonese_simplified_output) {
  return follows_cantonese(scheme_number) ? cantonese_simplified_output
                                          : traditional_output;
}

constexpr const char *label(int scheme_number) {
  return follows_cantonese(scheme_number) ? "简体输出" : "繁体输出";
}

constexpr const char *hint(int scheme_number) {
  return follows_cantonese(scheme_number) ? "将粤拼候选和上屏文本转换为简体"
                                          : "将中文候选和上屏文本转换为繁体";
}

} // 命名空间 msime::linux_host::character_set
