#include "ToolbarIcons.h"
#include <cstring>
#include <iostream>
#include <set>
#include <stdexcept>
#include <string>
#include <utility>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Toolbar icon table failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace
int main() {
  try {
    // Codepoints match the shipped presenter's table exactly.
    require(toolbar_icon(kToolbarLanguage, true).codepoint == 0xE982);
    require(toolbar_icon(kToolbarLanguage, false).codepoint == 0xE983);
    require(toolbar_icon(kToolbarFullwidth, true).codepoint == 0xF138);
    require(toolbar_icon(kToolbarFullwidth, false).codepoint == 0xEC46);
    require(toolbar_icon(kToolbarPunctuation, true).codepoint == 0xF111);
    require(toolbar_icon(kToolbarPunctuation, false).codepoint == 0xF110);
    require(toolbar_icon(kToolbarCharacterSet, true).codepoint == 0xE88C);
    require(toolbar_icon(kToolbarCharacterSet, false).codepoint == 0xE88D);
    require(toolbar_icon(kToolbarEmoji, std::nullopt).codepoint == 0xE76E);
    require(toolbar_icon(kToolbarScreenKeyboard, std::nullopt).codepoint == 0xE765);
    require(toolbar_icon(kToolbarSettings, std::nullopt).codepoint == 0xE713);

    // An unreported mode is a question mark, not a guessed state. Showing 中
    // when the Server has not said so would tell the user the wrong mode.
    for (const int button : {kToolbarLanguage, kToolbarFullwidth,
                             kToolbarPunctuation, kToolbarCharacterSet}) {
      const auto icon = toolbar_icon(button, std::nullopt);
      require(icon.codepoint == 0);
      require(std::wcscmp(icon.fallback, L"?") == 0);
    }

    // Every icon carries a non-empty text fallback: the glyph fonts are not
    // present on every Windows build, and a missing glyph draws a blank box.
    for (int button = kToolbarLanguage; button <= kToolbarHide; ++button)
      for (const std::optional<bool> state :
           {std::optional<bool>{}, std::optional<bool>{true},
            std::optional<bool>{false}}) {
        const auto icon = toolbar_icon(button, state);
        require(icon.fallback != nullptr && icon.fallback[0] != L'\0');
      }

    // The two states of one button must differ, or the toolbar cannot show
    // which mode is active - the whole point of the button.
    for (const int button : {kToolbarLanguage, kToolbarFullwidth,
                             kToolbarPunctuation, kToolbarCharacterSet}) {
      const auto on = toolbar_icon(button, true);
      const auto off = toolbar_icon(button, false);
      require(on.codepoint != off.codepoint);
      require(std::wcscmp(on.fallback, off.fallback) != 0);
    }

    // Distinct buttons must not share a glyph either.
    std::set<wchar_t> seen;
    for (const auto &entry :
         {toolbar_icon(kToolbarLanguage, true), toolbar_icon(kToolbarLanguage, false),
          toolbar_icon(kToolbarFullwidth, true), toolbar_icon(kToolbarFullwidth, false),
          toolbar_icon(kToolbarPunctuation, true), toolbar_icon(kToolbarPunctuation, false),
          toolbar_icon(kToolbarCharacterSet, true), toolbar_icon(kToolbarCharacterSet, false),
          toolbar_icon(kToolbarEmoji, std::nullopt),
          toolbar_icon(kToolbarScreenKeyboard, std::nullopt),
          toolbar_icon(kToolbarSettings, std::nullopt)}) {
      require(seen.insert(entry.codepoint).second);
    }

    // Hide has no upstream counterpart and no glyph, so it stays text. An id
    // outside the table - including the retired handwriting, voice and about
    // ids - is a question mark rather than a blank.
    require(toolbar_icon(kToolbarHide, std::nullopt).codepoint == 0);
    require(toolbar_icon(7, std::nullopt).codepoint == 0);
    require(std::wcscmp(toolbar_icon(7, std::nullopt).fallback, L"?") == 0);
    require(toolbar_icon(8, std::nullopt).codepoint == 0);
    require(toolbar_icon(9, std::nullopt).codepoint == 0);
    require(toolbar_icon(99, std::nullopt).codepoint == 0);
    require(std::wcscmp(toolbar_icon(99, true).fallback, L"?") == 0);

    // The language button reflects more than Chinese/English.
    {
      ToolbarLanguageState caps;
      caps.caps_lock = true;
      // Caps Lock wins over every input mode: it changes what each letter key
      // does, so showing 中 there would say the wrong thing.
      require(toolbar_icon(kToolbarLanguage, true, caps).codepoint == 0xE7B5);
      require(toolbar_icon(kToolbarLanguage, false, caps).codepoint == 0xE7B5);
      require(std::wcscmp(toolbar_icon(kToolbarLanguage, true, caps).fallback,
                          L"A") == 0);
      // Even with an unreported mode, Caps Lock is known and shown.
      require(toolbar_icon(kToolbarLanguage, std::nullopt, caps).codepoint ==
              0xE7B5);

      ToolbarLanguageState japanese;
      japanese.mode = scheme::InputMode::Japanese;
      require(toolbar_icon(kToolbarLanguage, true, japanese).codepoint == 0xE7DE);
      // But the temporary English toggle beats it, as upstream orders these:
      // the toggle is what the next key actually does, and 日 over a keystroke
      // that produces Latin letters tells the user the wrong thing.
      require(toolbar_icon(kToolbarLanguage, false, japanese).codepoint == 0xE983);

      // Korean draws its syllable as text: the icon font has no Hangul glyph, and a codepoint it lacks would render as a blank box. The English toggle and Caps Lock beat it exactly as they beat Japanese.
      ToolbarLanguageState korean;
      korean.mode = scheme::InputMode::Korean;
      const auto hangul = toolbar_icon(kToolbarLanguage, true, korean);
      require(!hangul.codepoint && std::wcscmp(hangul.fallback, L"한") == 0);
      require(!hangul.underline);
      require(toolbar_icon(kToolbarLanguage, false, korean).codepoint == 0xE983);
      ToolbarLanguageState capped_korean;
      capped_korean.caps_lock = true;
      capped_korean.mode = scheme::InputMode::Korean;
      require(toolbar_icon(kToolbarLanguage, true, capped_korean).codepoint == 0xE7B5);

      // 粤拼、注音、越南文和藏文同样把各自的字当文字画出来，排在 Caps Lock 和英文切换之后。
      const std::pair<scheme::InputMode, const wchar_t *> drawn[] = {{scheme::InputMode::Cantonese, L"粤"},
                                                                     {scheme::InputMode::Zhuyin, L"注"},
                                                                     {scheme::InputMode::Vietnamese, L"越"},
                                                                     {scheme::InputMode::Tibetan, L"藏"}};
      for (const auto &[mode, text] : drawn) {
        ToolbarLanguageState language;
        language.mode = mode;
        const auto icon = toolbar_icon(kToolbarLanguage, true, language);
        require(!icon.codepoint && std::wcscmp(icon.fallback, text) == 0 && !icon.underline);
        require(toolbar_icon(kToolbarLanguage, false, language).codepoint == 0xE983);
        language.caps_lock = true;
        require(toolbar_icon(kToolbarLanguage, true, language).codepoint == 0xE7B5);
      }

      // Caps Lock beats Japanese too, and the two together are not a fourth
      // state.
      ToolbarLanguageState both;
      both.caps_lock = true;
      both.mode = scheme::InputMode::Japanese;
      require(toolbar_icon(kToolbarLanguage, true, both).codepoint == 0xE7B5);

      // With neither, the button is the ordinary CN/EN pair.
      ToolbarLanguageState plain;
      require(toolbar_icon(kToolbarLanguage, true, plain).codepoint == 0xE982);
      require(toolbar_icon(kToolbarLanguage, false, plain).codepoint == 0xE983);
      require(!toolbar_icon(kToolbarLanguage, std::nullopt, plain).codepoint);

      // Dedicated English is the Engine's own mode, and is the only button
      // state drawn as underlined text rather than a glyph.
      ToolbarLanguageState dedicated;
      dedicated.dedicated_english = true;
      const auto english = toolbar_icon(kToolbarLanguage, true, dedicated);
      require(english.underline);
      require(!english.codepoint);
      require(std::wstring(english.fallback) == L"En");
      // Unreported Chinese state does not suppress it: the dedicated mode is
      // known independently of the toggle.
      require(toolbar_icon(kToolbarLanguage, std::nullopt, dedicated).underline);

      // The temporary English toggle wins, because it is what the next key
      // actually does and it is what the user just pressed.
      const auto toggled = toolbar_icon(kToolbarLanguage, false, dedicated);
      require(toggled.codepoint == 0xE983);
      require(!toggled.underline);

      // Caps Lock still beats everything.
      ToolbarLanguageState capped_english;
      capped_english.caps_lock = true;
      capped_english.dedicated_english = true;
      require(toolbar_icon(kToolbarLanguage, true, capped_english).codepoint ==
              0xE7B5);
      require(!toolbar_icon(kToolbarLanguage, true, capped_english).underline);

      // Dedicated English beats Japanese; the two together are not a fifth
      // state.
      ToolbarLanguageState english_and_japanese;
      english_and_japanese.dedicated_english = true;
      english_and_japanese.mode = scheme::InputMode::Japanese;
      require(
          toolbar_icon(kToolbarLanguage, true, english_and_japanese).underline);

      // Nothing else is ever underlined; the line means one specific mode.
      require(!toolbar_icon(kToolbarLanguage, true, plain).underline);
      require(!toolbar_icon(kToolbarLanguage, false, plain).underline);
      for (int button = kToolbarLanguage; button <= kToolbarHide; ++button)
        if (button != kToolbarLanguage)
          require(!toolbar_icon(button, true, dedicated).underline);

      // No other button is affected by the language state.
      require(toolbar_icon(kToolbarEmoji, std::nullopt, caps).codepoint ==
              toolbar_icon(kToolbarEmoji, std::nullopt, plain).codepoint);
      require(toolbar_icon(kToolbarFullwidth, true, caps).codepoint ==
              toolbar_icon(kToolbarFullwidth, true, plain).codepoint);
    }

    std::cout << "Toolbar icons: glyphs and fallbacks match the shipped set\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Toolbar icon table failed with an unknown error\n";
    return 1;
  }
}
