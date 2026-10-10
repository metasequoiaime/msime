#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

namespace msime::windows {

inline constexpr uint32_t kVirtualKeyBackspace = 0x08;
inline constexpr uint32_t kVirtualKeyShift = 0x10;
inline constexpr uint32_t kVirtualKeyEscape = 0x1B;
inline constexpr uint32_t kVirtualKeyLeft = 0x25;
inline constexpr uint32_t kVirtualKeyRight = 0x27;
inline constexpr uint32_t kVirtualKeyLeftShift = 0xA0;
inline constexpr uint32_t kVirtualKeyRightShift = 0xA1;
inline constexpr uint32_t kVirtualKeyNumpad0 = 0x60;
inline constexpr uint32_t kVirtualKeyNumpad9 = 0x69;
inline constexpr uint32_t kModifierShift = 0b00000001u;
inline constexpr uint32_t kModifierControl = 0b00000010u;
inline constexpr uint32_t kModifierAlt = 0b00000100u;
inline constexpr uint32_t kKeyModifierMask = kModifierShift | kModifierControl | kModifierAlt;

constexpr uint32_t normalize_numpad_digit_key(uint32_t keycode) {
  return keycode >= kVirtualKeyNumpad0 && keycode <= kVirtualKeyNumpad9
             ? static_cast<uint32_t>('0') + keycode - kVirtualKeyNumpad0
             : keycode;
}

constexpr bool is_backend_independent_reset_key(uint32_t keycode) {
  return keycode == kVirtualKeyShift || keycode == kVirtualKeyEscape ||
         keycode == kVirtualKeyLeftShift || keycode == kVirtualKeyRightShift;
}

constexpr bool is_segment_backspace_key(uint32_t keycode, uint32_t modifiers) {
  return keycode == kVirtualKeyBackspace &&
         (modifiers & kKeyModifierMask) == kModifierControl;
}

// 整句改字的方向键（msime_client.h）：`edits_sentence`（全拼、双拼，不在本地模式和专用英文里）时，不带修饰键的左右键换成改字命令，Ctrl+左右换成逐个字母的光标命令；其他命令和其他方案原样返回。数值与 msime_client.h 的 MsimeCommand 相同。
inline constexpr uint32_t kCommandMoveLeft = 4;
inline constexpr uint32_t kCommandMoveRight = 5;
inline constexpr uint32_t kCommandMoveLeftSegment = 13;
inline constexpr uint32_t kCommandMoveRightSegment = 14;
inline constexpr uint32_t kCommandConversionLeft = 17;
inline constexpr uint32_t kCommandConversionRight = 18;
constexpr uint32_t sentence_edit_command(uint32_t command, bool edits_sentence) {
  if (!edits_sentence)
    return command;
  switch (command) {
  case kCommandMoveLeft:
    return kCommandConversionLeft;
  case kCommandMoveRight:
    return kCommandConversionRight;
  case kCommandMoveLeftSegment:
    return kCommandMoveLeft;
  case kCommandMoveRightSegment:
    return kCommandMoveRight;
  default:
    return command;
  }
}

constexpr bool is_segment_caret_key(uint32_t keycode, uint32_t modifiers) {
  return (keycode == kVirtualKeyLeft || keycode == kVirtualKeyRight) &&
         (modifiers & kKeyModifierMask) == kModifierControl;
}

// Ctrl+Shift+E without Alt toggles the dedicated English mode, matching the reference Server's IsEnglishModeToggleKey. The TSF claims the chord while the IME is open and cancels its own composition locally.
constexpr bool is_english_mode_toggle_key(uint32_t keycode, uint32_t modifiers) {
  return keycode == 'E' && (modifiers & kKeyModifierMask) == (kModifierShift | kModifierControl);
}

// A composing key needs a reverse-pipe reply when the TSF side cannot finish
// the edit locally. Japanese long-vowel input follows the same explicit policy
// as letters, separators, and Unicode digits.
constexpr bool should_send_composition_reply(bool is_alpha_key,
                                             bool is_manual_pinyin_separator,
                                             bool is_microsoft_shuangpin_ing_key,
                                             bool is_unicode_hex_digit,
                                             bool is_unicode_plus,
                                             bool is_japanese_long_vowel) {
  return is_alpha_key || is_manual_pinyin_separator ||
         is_microsoft_shuangpin_ing_key || is_unicode_hex_digit ||
         is_unicode_plus || is_japanese_long_vowel;
}

inline std::size_t previous_segment_boundary(const std::vector<std::size_t> &boundaries,
                                             std::size_t caret) {
  std::size_t result = caret;
  for (const auto boundary : boundaries) {
    if (boundary >= caret) break;
    result = boundary;
  }
  return result;
}

inline std::size_t next_segment_boundary(const std::vector<std::size_t> &boundaries,
                                          std::size_t caret) {
  for (const auto boundary : boundaries)
    if (boundary > caret) return boundary;
  return caret;
}

// Enter learns an ASCII word only for the same cases as the Windows server:
// dedicated English, a Shift-letter special mode, or an incomplete Chinese
// composition. Complete pure-pinyin input is deliberately excluded because
// it represents Chinese candidate input rather than an entered English word.
constexpr bool should_learn_entered_english_word(bool dedicated_english_mode,
                                                 bool shift_letter_special_mode,
                                                 bool chinese_scheme,
                                                 bool all_complete_pure_pinyin) {
  return dedicated_english_mode || shift_letter_special_mode ||
         (chinese_scheme && !all_complete_pure_pinyin);
}

} // namespace msime::windows
