#pragma once
#include <cstddef>
#include <string>
#include <string_view>

namespace msime::windows {
// Separators the Engine adds to a preedit for display, or the user typed into editing_text: the syllable apostrophe and a space.
inline bool preedit_display_separator(char value) { return value == '\'' || value == ' '; }

// Whether `text` is ASCII letters and separators only, and if so whether its letters are exactly `letters` (separators skipped).
inline bool preedit_display_letters_match(std::string_view text, std::string_view letters) {
  std::size_t matched = 0;
  for (const char value : text) {
    if (preedit_display_separator(value))
      continue;
    if (!((value >= 'a' && value <= 'z') || (value >= 'A' && value <= 'Z')))
      return false;
    if (matched == letters.size() || letters[matched] != value)
      return false;
    ++matched;
  }
  return matched == letters.size();
}

// Byte offset in `preedit` for the Engine's caret, which is a byte offset into the ASCII `editing` (View.editing_text / caret_position), or npos when it has no place there.
//
// Quanpin preedit is the Engine's segmented display (`ni'hao`) while editing_text stays the raw keys (`nihao`), so the caret is carried across the separators the Engine inserted. This is the same mapping as macOS MSIMEPreeditCaretPosition (shared/apple/TextClient.mm): a raw caret at a syllable boundary lands before the inserted apostrophe, and a raw caret right after an apostrophe the user typed lands after the separators at that point. Only that lossless formatting is mapped. Expanded shuangpin, Hangul, kana, stroke glyphs and converted words spell something other than the raw letters, and a caret at the wrong character is worse than none, so those get npos; so does a caret past the end of `editing`.
inline std::size_t preedit_display_caret(std::string_view editing, std::string_view preedit,
                                         std::size_t caret) {
  if (caret > editing.size())
    return std::string::npos;
  if (editing == preedit)
    return caret;
  std::string letters;
  letters.reserve(editing.size());
  for (const char value : editing)
    if (!preedit_display_separator(value))
      letters.push_back(value);
  if (letters.empty() || !preedit_display_letters_match(editing, letters) ||
      !preedit_display_letters_match(preedit, letters))
    return std::string::npos;
  if (caret == editing.size())
    return preedit.size();
  std::size_t remaining = 0;
  for (std::size_t index = 0; index < caret; ++index)
    if (!preedit_display_separator(editing[index]))
      ++remaining;
  std::size_t position = 0;
  while (position < preedit.size() && remaining) {
    if (!preedit_display_separator(preedit[position]))
      --remaining;
    ++position;
  }
  if (caret && preedit_display_separator(editing[caret - 1]))
    while (position < preedit.size() && preedit_display_separator(preedit[position]))
      ++position;
  return position;
}
} // namespace msime::windows
