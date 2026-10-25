#pragma once

#include <string>
#include <string_view>
#include <utility>

namespace msime::windows {
// 规范主键到 Win32 虚拟键的映射；组合键写法只交给 Rust 解析。
inline std::string toggle_key_name(unsigned key) {
  if (key >= 'A' && key <= 'Z') return "Key" + std::string(1, static_cast<char>(key));
  if (key >= '0' && key <= '9') return "Digit" + std::string(1, static_cast<char>(key));
  if (key >= 0x70 && key <= 0x7B) return "F" + std::to_string(key - 0x70 + 1);
  constexpr std::pair<unsigned, std::string_view> names[] = {
      {0x20, "Space"}, {0xBA, "Semicolon"}, {0xBB, "Equal"}, {0xBC, "Comma"},
      {0xBD, "Minus"}, {0xBE, "Period"}, {0xBF, "Slash"}, {0xC0, "Backquote"},
      {0xDB, "BracketLeft"}, {0xDC, "Backslash"}, {0xDD, "BracketRight"}, {0xDE, "Quote"}};
  for (const auto &[value, name] : names)
    if (value == key) return std::string(name);
  return {};
}
inline unsigned toggle_key_code(std::string_view name) {
  for (unsigned key = 1; key <= 0xFE; ++key)
    if (!name.empty() && toggle_key_name(key) == name) return key;
  return 0;
}
} // namespace msime::windows
