#pragma once

#include "TranslationDisplay.h"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

namespace msime::windows {
// 释义列快捷键，和 macOS InputController 的 commitCandidateGlossColumn / cycleArmedGlossColumnBackwards 一致。第 1 列是第一种目标语言的释义，第 2 列是第二种；列号 0 表示候选本身。
//
// - Alt+数字：上屏第 N 个候选的第 1 列释义（macOS 是 Option+数字）。
// - Ctrl+数字：上屏第 N 个候选的第 2 列释义。
// - Tab / Shift+Tab：在高亮候选有释义的列之间循环预选（0 → 1 → 2 → 0），预选的列在候选窗里带下划线；高亮候选没有释义时 Tab 照常翻页。
// - 预选了列之后，数字和空格上屏对应候选的那一列释义；那一列是空的就照常选候选。
//
// 释义上屏后组字整个取消，不再上屏候选本身：释义不是用户选的候选，不该教会引擎。

// IPC 包里的修饰键位（tsf/IPC/Ipc.h）：Shift 1、Ctrl 2、Alt 4。
inline constexpr std::uint32_t gloss_modifier_shift = 0b001u;
inline constexpr std::uint32_t gloss_modifier_control = 0b010u;
inline constexpr std::uint32_t gloss_modifier_alt = 0b100u;

// 数字键带的修饰键直接选哪一列：只按 Alt 是第 1 列，只按 Ctrl 是第 2 列，其余组合（含 AltGr 读成的 Ctrl+Alt）不是释义快捷键。
inline std::optional<int> gloss_column_for_digit_modifiers(std::uint32_t modifiers) {
  switch (modifiers & (gloss_modifier_shift | gloss_modifier_control | gloss_modifier_alt)) {
  case gloss_modifier_alt:
    return 1;
  case gloss_modifier_control:
    return 2;
  default:
    return std::nullopt;
  }
}

// 一条释义的第 column 列（U+2028 分行，见 TranslationDisplay.h）；没有那一列或列号不合法时为空。
inline std::string gloss_column_text(std::string_view translation, int column) {
  if (column <= 0 || translation.empty())
    return {};
  const auto lines = translation_lines(translation);
  const auto index = static_cast<std::size_t>(column - 1);
  return index < lines.size() ? lines[index] : std::string{};
}

// Tab / Shift+Tab 之后预选哪一列：在 0 和高亮候选有释义的列之间循环，当前列不在其中时从 0 算起。高亮候选两列都没有释义时返回空，Tab 留给翻页。
inline std::optional<int> next_armed_gloss_column(int armed, std::string_view translation,
                                                  bool backwards) {
  const bool primary = !gloss_column_text(translation, 1).empty();
  const bool secondary = !gloss_column_text(translation, 2).empty();
  if (!primary && !secondary)
    return std::nullopt;
  int available[3] = {0, 0, 0};
  std::size_t count = 0;
  available[count++] = 0;
  if (primary)
    available[count++] = 1;
  if (secondary)
    available[count++] = 2;
  std::size_t current = 0;
  for (std::size_t index = 0; index < count; ++index)
    if (available[index] == armed)
      current = index;
  const std::size_t next = backwards ? (current + count - 1) % count : (current + 1) % count;
  return available[next];
}
} // namespace msime::windows
