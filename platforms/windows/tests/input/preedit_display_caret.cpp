#include "PreeditDisplayCaret.h"
#include <cstddef>
#include <iostream>
#include <string>

using msime::windows::preedit_display_caret;
namespace {
int failures = 0;
void expect(std::string_view editing, std::string_view preedit, std::size_t caret,
            std::size_t expected, int line) {
  const auto actual = preedit_display_caret(editing, preedit, caret);
  if (actual == expected)
    return;
  ++failures;
  std::cerr << "line " << line << ": " << editing << " -> " << preedit << " caret " << caret
            << " gave " << actual << ", expected " << expected << '\n';
}
#define EXPECT(editing, preedit, caret, expected)                                                \
  expect(editing, preedit, caret, expected, __LINE__)
constexpr auto none = std::string::npos;
} // namespace
int main() {
  // The same string: the offset carries over unchanged, ends included.
  EXPECT("nihao", "nihao", 0, 0);
  EXPECT("nihao", "nihao", 2, 2);
  EXPECT("nihao", "nihao", 5, 5);
  EXPECT("shiY", "shiY", 3, 3);
  // Past the end is refused rather than clamped, segmented or not.
  EXPECT("ni", "ni", 9, none);
  EXPECT("nihao", "ni'hao", 6, none);

  // Quanpin segmented by the Engine (#9448): every raw position, matching macOS MSIMEPreeditCaretPosition. The syllable boundary (raw 2) lands before the inserted apostrophe.
  EXPECT("nihao", "ni'hao", 0, 0);
  EXPECT("nihao", "ni'hao", 1, 1);
  EXPECT("nihao", "ni'hao", 2, 2);
  EXPECT("nihao", "ni'hao", 3, 4);
  EXPECT("nihao", "ni'hao", 4, 5);
  EXPECT("nihao", "ni'hao", 5, 6);
  EXPECT("pinyin", "pin'yin", 3, 3);
  EXPECT("pinyin", "pin'yin", 4, 5);
  EXPECT("pinyin", "pin'yin", 6, 7);
  // Corrections keep the typed letters (`sahnghao` shows as `sahng'hao`), so they map too.
  EXPECT("sahnghao", "sahng'hao", 6, 7);

  // An apostrophe the user typed: before it stays before it, after it stays after it, and the Engine's own apostrophes are still skipped.
  EXPECT("pinyin'shurufa", "pin'yin'shu'ru'fa", 6, 7);
  EXPECT("pinyin'shurufa", "pin'yin'shu'ru'fa", 7, 8);
  EXPECT("pinyin'shurufa", "pin'yin'shu'ru'fa", 10, 11);
  EXPECT("pinyin'shurufa", "pin'yin'shu'ru'fa", 11, 13);
  EXPECT("pinyin'shurufa", "pin'yin'shu'ru'fa", 14, 17);
  // A trailing typed apostrophe is kept by the display.
  EXPECT("nihao'", "ni'hao'", 5, 6);
  EXPECT("nihao'", "ni'hao'", 6, 7);
  // Spaces are separators too.
  EXPECT("nihao", "ni hao", 3, 4);

  // No lossless map: a different spelling (expanded shuangpin), non-ASCII compositions (Hangul, stroke glyphs, kana) and separators only.
  EXPECT("nihc", "ni'hao", 2, none);
  EXPECT("nihao", "ni'hai", 2, none);
  EXPECT("nihao", "ni'ha", 2, none);
  EXPECT("gks", "\xED\x95\x9C", 1, none);
  EXPECT("hs", "\xE4\xB8\x80\xE4\xB8\xA8", 1, none);
  EXPECT("nihon", "\xE3\x81\xAB\xE3\x81\xBB\xE3\x82\x93", 2, none);
  EXPECT("ni1", "ni'1", 1, none);
  EXPECT("''", "'''", 1, none);

  if (failures)
    return 1;
  std::cout << "Preedit display caret: carried across display separators only\n";
  return 0;
}
