#pragma once
#include "msime_client.h"
#include "PipeMetadata.h"
#include "InputKeyPolicy.h"
#include "windows_ipc.h"
#include <optional>
#include <string>

namespace msime::windows {
static_assert(kCommandMoveLeft == MSIME_MOVE_LEFT && kCommandMoveRight == MSIME_MOVE_RIGHT &&
                  kCommandMoveLeftSegment == MSIME_MOVE_LEFT_SEGMENT &&
                  kCommandMoveRightSegment == MSIME_MOVE_RIGHT_SEGMENT &&
                  kCommandConversionLeft == MSIME_CONVERSION_LEFT &&
                  kCommandConversionRight == MSIME_CONVERSION_RIGHT,
              "sentence_edit_command mirrors msime_client.h");
enum class KeyKind { Ignore, LocalReset, CancelAndForward, Character, Command };
struct KeyAction {
  KeyKind kind = KeyKind::Ignore;
  uint32_t value = 0;
  bool shift = false;
};

// Match the legacy Server boundary: TSF classifies both digit rows as
// candidate keys. Preserve the original packet for request/transport checks.
inline constexpr uint32_t normalize_digit_key(uint32_t key) {
  return normalize_numpad_digit_key(key);
}

// Win32 VK values are stable wire values, not characters. Text comes from wch,
// already translated by TSF using the active keyboard layout. No ToUnicode call
// belongs on the Server thread. Modifiers are the TSF protocol bitset, not
// MK_*.
inline KeyAction translate_key(const FanyImeNamedpipeData &packet) {
  if (packet.event_type != FanyImePipeEventType::KeyEvent)
    return {};
  const auto key = packet.keycode;
  const auto modifiers = PipeMetadata::key_modifiers(packet.modifiers_down);
  // Existing TSF consumes these locally and tells Server to cancel without a
  // reverse-pipe key reply. This differs from raw OS modifier key-down
  // handling.
  if (is_backend_independent_reset_key(key))
    return {KeyKind::LocalReset, MSIME_CANCEL};
  if (key == 0x11 || key == 0x12 || (key >= 0xA2 && key <= 0xA5) ||
      key == 0x5B || key == 0x5C)
    return {};
  // TSF's wire modifier bits are Shift=1, Control=2, Alt=4. Segment
  // editing is deliberately Ctrl-only; Shift+Backspace/Arrow belongs to the
  // host and must not mutate the Engine composition.
  if (is_segment_backspace_key(key, modifiers) ||
      is_segment_caret_key(key, modifiers)) {
    // The conditional is not a constant expression, so MSVC rejects the enum-to-uint32_t conversion as narrowing inside braces; GCC and Clang accept it.
    return {KeyKind::Command, static_cast<uint32_t>(key == 0x08   ? MSIME_BACKSPACE_SEGMENT
                                                    : key == 0x25 ? MSIME_MOVE_LEFT_SEGMENT
                                                                  : MSIME_MOVE_RIGHT_SEGMENT)};
  }
  if (modifiers & ~1u)
    return {KeyKind::CancelAndForward, MSIME_CANCEL};
  switch (key) {
  case 0x08:
    return {KeyKind::Command, MSIME_BACKSPACE};
  case 0x0D:
    return {KeyKind::Command, MSIME_COMMIT_RAW};
  case 0x20:
    return {KeyKind::Command, MSIME_COMMIT_CANDIDATE};
  case 0x21:
    return {KeyKind::Command, MSIME_PREVIOUS_PAGE};
  case 0x22:
    return {KeyKind::Command, MSIME_NEXT_PAGE};
  case 0x23:
    return {KeyKind::Command, MSIME_MOVE_END};
  case 0x24:
    return {KeyKind::Command, MSIME_MOVE_HOME};
  case 0x25:
    return {KeyKind::Command, MSIME_MOVE_LEFT};
  case 0x26:
    return {KeyKind::Command, MSIME_PREVIOUS_CANDIDATE};
  case 0x27:
    return {KeyKind::Command, MSIME_MOVE_RIGHT};
  case 0x28:
    return {KeyKind::Command, MSIME_NEXT_CANDIDATE};
  case 0x2E:
    return {KeyKind::Command, MSIME_DELETE_FORWARD};
  }
  if (key >= 0x60 && key <= 0x69)
    return {KeyKind::Character, normalize_digit_key(key), false};
  const auto text = static_cast<uint32_t>(packet.wch);
  if (text >= 0x21 && text <= 0x7E)
    return {KeyKind::Character, text, (modifiers & 1u) != 0};
  return {KeyKind::CancelAndForward, MSIME_CANCEL};
}

// Enter is completed in-process by the legacy TSF. Carry its bounded local
// observation through the historical pinyin_string field so the out-of-process
// session can clear the same raw composition without inventing a commit.
inline std::optional<std::string>
local_commit_observation(const FanyImeNamedpipeData &packet) {
  if (packet.keycode != 0x0D || packet.pinyin_length < 0 ||
      packet.pinyin_length >= 128)
    return std::nullopt;
  const auto length = static_cast<size_t>(packet.pinyin_length);
  for (size_t index = 0; index < length; ++index)
    if (packet.pinyin_string[index] == 0)
      return std::nullopt;
  if (packet.pinyin_string[length] != 0)
    return std::nullopt;

  std::string result;
  result.reserve(length);
  for (size_t index = 0; index < length; ++index) {
    const uint32_t first = static_cast<uint16_t>(packet.pinyin_string[index]);
    uint32_t scalar = first;
    if (first >= 0xD800 && first <= 0xDBFF) {
      if (index + 1 >= length)
        return std::nullopt;
      const uint32_t second =
          static_cast<uint16_t>(packet.pinyin_string[++index]);
      if (second < 0xDC00 || second > 0xDFFF)
        return std::nullopt;
      scalar = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
    } else if (first >= 0xDC00 && first <= 0xDFFF) {
      return std::nullopt;
    }
    if (scalar <= 0x7F) {
      result.push_back(static_cast<char>(scalar));
    } else if (scalar <= 0x7FF) {
      result.push_back(static_cast<char>(0xC0 | (scalar >> 6)));
      result.push_back(static_cast<char>(0x80 | (scalar & 0x3F)));
    } else if (scalar <= 0xFFFF) {
      result.push_back(static_cast<char>(0xE0 | (scalar >> 12)));
      result.push_back(static_cast<char>(0x80 | ((scalar >> 6) & 0x3F)));
      result.push_back(static_cast<char>(0x80 | (scalar & 0x3F)));
    } else {
      result.push_back(static_cast<char>(0xF0 | (scalar >> 18)));
      result.push_back(static_cast<char>(0x80 | ((scalar >> 12) & 0x3F)));
      result.push_back(static_cast<char>(0x80 | ((scalar >> 6) & 0x3F)));
      result.push_back(static_cast<char>(0x80 | (scalar & 0x3F)));
    }
  }
  return result;
}
} // namespace msime::windows
