#pragma once

#include <string_view>

namespace msime::windows::character_set {

// 粤拼词库本身是繁体：这个开关保存的是「简体输出」，其他方案保存的是「繁体输出」。
inline constexpr bool is_cantonese(std::string_view scheme) {
  return scheme == "cantonese";
}

inline constexpr const char *preference_key(std::string_view scheme) {
  return is_cantonese(scheme) ? "cantonese_simplified_output"
                             : "traditional_chinese_output";
}

inline constexpr bool is_traditional(std::string_view scheme,
                                     bool traditional_output,
                                     bool cantonese_simplified_output) {
  return is_cantonese(scheme) ? !cantonese_simplified_output
                              : traditional_output;
}

inline constexpr bool stored_value(std::string_view scheme,
                                   bool traditional_output) {
  return is_cantonese(scheme) ? !traditional_output : traditional_output;
}

}  // namespace msime::windows::character_set
