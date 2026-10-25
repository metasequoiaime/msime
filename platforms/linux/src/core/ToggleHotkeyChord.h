#pragma once

#include "../../../common/ToggleHotkeyChord.h"
#include <string>
#include <utility>

namespace msime::linux_host {
// Shift 改变的是符号，不是录键时的主键；松开顺序也不能改变一次按键的身份。
inline unsigned toggle_key_symbol(unsigned symbol) {
  if (symbol >= 'A' && symbol <= 'Z')
    return symbol - 'A' + 'a';
  constexpr std::pair<unsigned, unsigned> shifted[] = {
      {'!', '1'}, {'@', '2'}, {'#', '3'}, {'$', '4'}, {'%', '5'},
      {'^', '6'}, {'&', '7'}, {'*', '8'}, {'(', '9'}, {')', '0'},
      {':', ';'}, {'+', '='}, {'<', ','}, {'_', '-'}, {'>', '.'},
      {'?', '/'}, {'~', '`'}, {'{', '['}, {'|', '\\'}, {'}', ']'}, {'"', '\''}};
  for (const auto &[from, to] : shifted)
    if (symbol == from) return to;
  return symbol;
}

// DOM code 与 evdev 物理位置对应；真实事件优先用键码，布局字母和 Shift 符号不改变绑定。
inline std::string toggle_physical_key_name(unsigned evdev) {
  if (evdev >= 2 && evdev <= 11) return "Digit" + std::string(1, "1234567890"[evdev - 2]);
  if (evdev >= 16 && evdev <= 25) return "Key" + std::string(1, "QWERTYUIOP"[evdev - 16]);
  if (evdev >= 30 && evdev <= 38) return "Key" + std::string(1, "ASDFGHJKL"[evdev - 30]);
  if (evdev >= 44 && evdev <= 50) return "Key" + std::string(1, "ZXCVBNM"[evdev - 44]);
  if (evdev >= 59 && evdev <= 68) return "F" + std::to_string(evdev - 58);
  if (evdev == 87 || evdev == 88) return "F" + std::to_string(evdev - 76);
  constexpr std::pair<unsigned, std::string_view> names[] = {
      {57, "Space"}, {39, "Semicolon"}, {13, "Equal"}, {51, "Comma"},
      {12, "Minus"}, {52, "Period"}, {53, "Slash"}, {41, "Backquote"},
      {26, "BracketLeft"}, {43, "Backslash"}, {27, "BracketRight"}, {40, "Quote"}};
  for (const auto &[key, name] : names)
    if (evdev == key) return std::string(name);
  return {};
}

inline std::string toggle_key_name(unsigned symbol, unsigned evdev = 0) {
  if (evdev != 0) return toggle_physical_key_name(evdev);
  symbol = toggle_key_symbol(symbol);
  if (symbol >= 'a' && symbol <= 'z')
    return "Key" + std::string(1, static_cast<char>(symbol - 'a' + 'A'));
  if (symbol >= '0' && symbol <= '9')
    return "Digit" + std::string(1, static_cast<char>(symbol));
  if (symbol >= 0xffbeu && symbol <= 0xffc9u)
    return "F" + std::to_string(symbol - 0xffbeu + 1);
  constexpr std::pair<unsigned, std::string_view> names[] = {
      {' ', "Space"}, {';', "Semicolon"}, {'=', "Equal"}, {',', "Comma"},
      {'-', "Minus"}, {'.', "Period"}, {'/', "Slash"}, {'`', "Backquote"},
      {'[', "BracketLeft"}, {'\\', "Backslash"}, {']', "BracketRight"}, {'\'', "Quote"}};
  for (const auto &[key, name] : names)
    if (symbol == key) return std::string(name);
  return {};
}
} // namespace msime::linux_host
