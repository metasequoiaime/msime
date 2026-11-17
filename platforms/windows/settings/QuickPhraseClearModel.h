#pragma once

#include <cmath>
#include <cstdint>
#include <limits>
#include <optional>
#include <string>
#include <string_view>

namespace msime::settings::quick_phrase_clear {

inline constexpr std::string_view operation = "clear_quick_phrases";
inline constexpr std::wstring_view title = L"清空快捷短语";
inline constexpr std::wstring_view confirmation =
    L"所有个人快捷短语将被删除，内置短语会保留。此操作无法撤销，建议先导出备份。";
inline constexpr std::wstring_view confirm_button = L"清空";
inline constexpr std::wstring_view success_prefix = L"已清空 ";
inline constexpr std::wstring_view success_suffix = L" 条快捷短语。";

inline std::optional<uint64_t> count_from_number(double value) {
  if (!std::isfinite(value) || value < 0 || std::floor(value) != value ||
      value > static_cast<double>((std::numeric_limits<uint64_t>::max)()))
    return std::nullopt;
  return static_cast<uint64_t>(value);
}

inline std::wstring success_message(uint64_t count) {
  return std::wstring(success_prefix) + std::to_wstring(count) + std::wstring(success_suffix);
}

} // 命名空间 msime::settings::quick_phrase_clear
