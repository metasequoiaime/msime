#include "ClipboardHistory.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Clipboard text normalisation failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
// UTF-16 units a UTF-8 string occupies, which is what the cap counts.
size_t utf16_length(const std::string &text) {
  size_t units = 0;
  for (size_t i = 0; i < text.size();) {
    const auto lead = static_cast<unsigned char>(text[i]);
    size_t length = 1;
    if ((lead & 0xE0) == 0xC0) length = 2;
    else if ((lead & 0xF0) == 0xE0) length = 3;
    else if ((lead & 0xF8) == 0xF0) length = 4;
    units += lead >= 0xF0 ? 2 : 1;
    i += length;
  }
  return units;
}
// Does every byte belong to a well-formed sequence?
bool valid_utf8(const std::string &text) {
  for (size_t i = 0; i < text.size();) {
    const auto lead = static_cast<unsigned char>(text[i]);
    size_t length = 0;
    if (lead < 0x80) length = 1;
    else if ((lead & 0xE0) == 0xC0) length = 2;
    else if ((lead & 0xF0) == 0xE0) length = 3;
    else if ((lead & 0xF8) == 0xF0) length = 4;
    else return false;
    if (i + length > text.size()) return false;
    for (size_t k = 1; k < length; ++k)
      if ((static_cast<unsigned char>(text[i + k]) & 0xC0) != 0x80) return false;
    i += length;
  }
  return true;
}
} // namespace
int main() {
  try {
    const size_t cap = ClipboardHistory::max_chars;

    // Only the clipboard terminators are removed; line endings and whitespace
    // are user content and remain byte-for-byte intact.
    require(normalize_clipboard_text("a\r\nb\r\n") == "a\r\nb\r\n");
    require(normalize_clipboard_text("a\rb") == "a\rb");
    require(normalize_clipboard_text("a\tb  \t\n") == "a\tb  \t\n");
    require(normalize_clipboard_text(std::string("a\0b", 3)) == "a");
    require(normalize_clipboard_text("a\r\0") == "a");

    // The cap counts UTF-16 units, not bytes. Chinese is three bytes per
    // character, so a byte cap cut this IME's primary case at about a third of
    // the documented limit.
    std::string chinese;
    for (size_t i = 0; i < cap + 200; ++i) chinese += "\xe4\xb8\xad"; // 中
    const auto capped = normalize_clipboard_text(chinese);
    require(utf16_length(capped) == cap);
    require(capped.size() == cap * 3); // Would have been ~cap bytes before.
    require(valid_utf8(capped));

    // A cut must never land inside a character.
    for (size_t extra = 0; extra < 4; ++extra) {
      std::string mixed(extra, 'a');
      for (size_t i = 0; i < cap; ++i) mixed += "\xe4\xb8\xad";
      const auto result = normalize_clipboard_text(mixed);
      require(valid_utf8(result));
      require(utf16_length(result) <= cap);
    }

    // Astral characters cost two UTF-16 units each, as a surrogate pair does.
    std::string emoji;
    for (size_t i = 0; i < cap; ++i) emoji += "\xf0\x9f\x98\x80"; // U+1F600
    const auto emoji_capped = normalize_clipboard_text(emoji);
    require(valid_utf8(emoji_capped));
    require(utf16_length(emoji_capped) == cap);
    // Two units each, so only half the characters fit.
    require(emoji_capped.size() == (cap / 2) * 4);

    // Text under the cap is untouched.
    const std::string short_text = "hello 世界";
    require(normalize_clipboard_text(short_text) == short_text);
    require(normalize_clipboard_text("").empty());

#ifdef _WIN32
    // ClipboardMonitor 在应用共享 UTF-16 上限前不能复制无界的 GlobalLock 缓冲区。
    const std::wstring oversized(cap + 100, L'合');
    const auto bounded = bounded_clipboard_text(
        oversized.data(), oversized.size() * sizeof(wchar_t));
    require(bounded.size() == cap + 1);
    require(bounded.front() == L'合' && bounded.back() == L'合');
#endif

    std::cout << "Clipboard text: capped in UTF-16 units on character boundaries\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Clipboard text normalisation failed with an unknown error\n";
    return 1;
  }
}
