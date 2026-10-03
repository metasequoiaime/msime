#pragma once

#include "../core/InputSchemeTraits.h"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

namespace msime::linux_host {

// Decode the entire candidate before exposing a destructive dictionary action.
// This mirrors Windows' UTF-8 distance check and rejects a valid prefix with
// malformed trailing bytes.
inline std::optional<std::size_t> candidate_utf8_codepoint_count(
    std::string_view text) {
  std::size_t count = 0;
  for (std::size_t offset = 0; offset < text.size();) {
    const auto first = static_cast<std::uint8_t>(text[offset]);
    std::size_t width = 0;
    std::uint32_t code_point = 0;
    if (first < 0x80) {
      width = 1;
      code_point = first;
    } else if (first >= 0xc2 && first <= 0xdf) {
      width = 2;
      code_point = first & 0x1f;
    } else if (first >= 0xe0 && first <= 0xef) {
      width = 3;
      code_point = first & 0x0f;
    } else if (first >= 0xf0 && first <= 0xf4) {
      width = 4;
      code_point = first & 0x07;
    } else {
      return std::nullopt;
    }
    if (offset + width > text.size())
      return std::nullopt;
    for (std::size_t index = 1; index < width; ++index) {
      const auto byte = static_cast<std::uint8_t>(text[offset + index]);
      if ((byte & 0xc0) != 0x80)
        return std::nullopt;
      code_point = (code_point << 6) | (byte & 0x3f);
    }
    if ((width == 2 && code_point < 0x80) ||
        (width == 3 && code_point < 0x800) ||
        (width == 4 && code_point < 0x10000) || code_point > 0x10ffff ||
        (code_point >= 0xd800 && code_point <= 0xdfff))
      return std::nullopt;
    offset += width;
    ++count;
  }
  return count;
}

// Windows deliberately omits deletion for a one-code-point Chinese
// candidate. IBus exposes candidate actions through its property menu instead
// of a per-row context menu, but the persistent dictionary operation keeps the
// same rule.
inline bool candidate_removal_available(std::string_view text) {
  return candidate_utf8_codepoint_count(text).value_or(0) > 1;
}

// Only the base Chinese schemes keep their candidates in the user dictionary (`learns_into_main_dictionary`): Japanese, Korean, Cantonese, Zhuyin, Vietnamese and Stroke candidates have no user-dictionary identity to pin, fix or remove.
inline bool candidate_dictionary_actions_available(std::uint64_t scheme, std::uint64_t source) {
  return scheme <= 255 && scheme::LearnsIntoMainDictionary(static_cast<int>(scheme)) &&
         (source == 0 || source == 1 || source == 4);
}

inline bool candidate_dictionary_removal_available(std::uint64_t scheme,
                                                   std::uint64_t source,
                                                   std::string_view text) {
  if (!candidate_dictionary_actions_available(scheme, source))
    return false;
  const auto count = candidate_utf8_codepoint_count(text);
  // Windows permits deleting one-character English dictionary entries, while
  // Chinese dictionary entries keep the one-code-point protection.
  return count.has_value() && (source == 4 ? *count >= 1 : *count > 1);
}

// IBus keysyms change with the active keyboard layout. Its evdev-derived
// keycode still identifies the physical number row (2..9 are 1..8), so use it
// before the ASCII keysym fallback for synthetic events without a keycode.
inline std::optional<std::size_t>
candidate_removal_slot(std::uint32_t key, std::uint32_t keycode) {
  if (keycode >= 2 && keycode <= 9)
    return static_cast<std::size_t>(keycode - 2);
  if (key >= static_cast<std::uint32_t>('1') &&
      key <= static_cast<std::uint32_t>('8'))
    return static_cast<std::size_t>(key - static_cast<std::uint32_t>('1'));
  return std::nullopt;
}

// Candidate action labels shared by the IBus property menu and the Fcitx5 candidate/status actions, worded like the Windows candidate menu (candidate_presenter.cpp: 置顶, 第 N 位 under 固定排位). Linux keeps the list flat, so the slot entries spell out the whole action.
inline constexpr const char *candidate_pin_label = "置顶";

inline std::string candidate_fix_label(int slot) {
  return "固定到第 " + std::to_string(slot) + " 位";
}

} // namespace msime::linux_host
