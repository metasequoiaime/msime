#pragma once

#include "InputSchemeTraits.h"
#include "msime_client.h"
#include <cstdint>

namespace msime::windows {
// The Hanja key of a Korean keyboard. It shares its code with VK_KANJI, so it only means Hanja while the Korean scheme is active.
inline constexpr std::uint32_t kVirtualKeyHanja = 0x19;

enum class KoreanHanjaKeyKind {
  // Not a key of the Hanja list: it keeps the meaning it has with no list open.
  None,
  // Choose the candidate in this slot of the visible page (value is the 0-based slot).
  Select,
  // Send this MSIME command to the session (value is the MsimeCommand).
  Command,
};

struct KoreanHanjaKey {
  KoreanHanjaKeyKind kind = KoreanHanjaKeyKind::None;
  std::uint32_t value = 0;
};

// What an unmodified key does to the Korean Hanja list of the composing syllable. The TIP applies it to its own host session and the Server to its session, from the same key, so the two sessions stay in step; neither consults the Chinese navigation bindings, which the TIP cannot see. The Hanja key converts (and closes an open list). With the list open, digits 1-9 choose from the visible page, Space and Enter choose the highlighted Hanja, the arrows move the highlight, Page Up/Down turn the page, Home/End go to the first and last Hanja, and Escape and Backspace close the list and keep the syllable. Every other key keeps its meaning: a letter closes the list and composes, and punctuation, '0', Tab, Insert and Delete close it and commit the Hangul. `text` is the key's character (0 when it has none), so a digit counts whichever row typed it and Shift+1 stays '!'.
constexpr KoreanHanjaKey korean_hanja_key(std::uint32_t virtual_key, std::uint32_t text) {
  switch (virtual_key) {
  case kVirtualKeyHanja:
    return {KoreanHanjaKeyKind::Command, MSIME_CONVERT_HANJA};
  case 0x08: // Backspace
    return {KoreanHanjaKeyKind::Command, MSIME_BACKSPACE};
  case 0x1B: // Escape
    return {KoreanHanjaKeyKind::Command, MSIME_CANCEL};
  case 0x0D: // Enter
  case 0x20: // Space
    return {KoreanHanjaKeyKind::Command, MSIME_COMMIT_CANDIDATE};
  case 0x21: // Page Up
    return {KoreanHanjaKeyKind::Command, MSIME_PREVIOUS_PAGE};
  case 0x22: // Page Down
    return {KoreanHanjaKeyKind::Command, MSIME_NEXT_PAGE};
  case 0x23: // End
    return {KoreanHanjaKeyKind::Command, MSIME_LAST_CANDIDATE};
  case 0x24: // Home
    return {KoreanHanjaKeyKind::Command, MSIME_FIRST_CANDIDATE};
  case 0x25: // Left
  case 0x26: // Up
    return {KoreanHanjaKeyKind::Command, MSIME_PREVIOUS_CANDIDATE};
  case 0x27: // Right
  case 0x28: // Down
    return {KoreanHanjaKeyKind::Command, MSIME_NEXT_CANDIDATE};
  default:
    break;
  }
  if (text >= '1' && text <= '9')
    return {KoreanHanjaKeyKind::Select, text - '1'};
  return {};
}

// A key the open Hanja list takes, other than the Hanja key itself.
constexpr bool is_korean_hanja_list_key(std::uint32_t virtual_key, std::uint32_t text) {
  return virtual_key != kVirtualKeyHanja && korean_hanja_key(virtual_key, text).kind != KoreanHanjaKeyKind::None;
}

inline constexpr std::uint32_t kVirtualKeyDown = 0x28;

// The unmodified key that sends MSIME_OPEN_CANDIDATE_LIST while a scheme with an openable list (scheme::OpensCandidateList) composes. Korean's is the Hanja key, which opens a closed list and closes an open one. Zhuyin's is Down on a closed list, libchewing's key for it and the one macOS uses; once its list is open Down moves the highlight like any list key. The Zhuyin list opens from Space as well, but that is the first tone the Engine spells, so it goes on as a character. Every other scheme has no such key.
constexpr bool opens_candidate_list(int scheme, std::uint32_t virtual_key, bool list_open) {
  if (scheme == scheme::Korean)
    return virtual_key == kVirtualKeyHanja;
  if (scheme == scheme::Zhuyin)
    return virtual_key == kVirtualKeyDown && !list_open;
  return false;
}
} // namespace msime::windows
