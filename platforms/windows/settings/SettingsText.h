#pragma once
#include <string>
#include <string_view>

namespace msime::settings::text {

// JavaScript `trim` 的空白集；供仍由原生表单处理的登录目标、绑定码和社区字段共用。
constexpr bool js_space(wchar_t c) {
  return c == L' ' || c == L'\t' || c == L'\n' || c == L'\v' || c == L'\f' || c == L'\r' ||
         c == 0x00A0 || c == 0x1680 || (c >= 0x2000 && c <= 0x200A) || c == 0x2028 ||
         c == 0x2029 || c == 0x202F || c == 0x205F || c == 0x3000 || c == 0xFEFF;
}

inline std::wstring trim(std::wstring_view value) {
  size_t begin = 0;
  size_t end = value.size();
  while (begin < end && js_space(value[begin]))
    ++begin;
  while (end > begin && js_space(value[end - 1]))
    --end;
  return std::wstring(value.substr(begin, end - begin));
}

// 社区表单的 UTF-16 码点计数，一个代理对算一个；昵称改由 Rust 校验。
inline size_t code_points(std::wstring_view value) {
  size_t count = 0;
  for (size_t i = 0; i < value.size(); ++i) {
    if (value[i] >= 0xD800 && value[i] <= 0xDBFF && i + 1 < value.size() &&
        value[i + 1] >= 0xDC00 && value[i + 1] <= 0xDFFF)
      ++i;
    ++count;
  }
  return count;
}

} // namespace msime::settings::text
