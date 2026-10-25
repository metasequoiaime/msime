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
void require_at(bool value, int line) {
  if (!value)
    require_failed(line);
}
#define require(value) require_at((value), __LINE__)
// The text a button draws when it has no glyph, and that it has none.
bool text(const ToolbarIcon &icon, const wchar_t *expected) {
  return icon.codepoint == 0 && std::wcscmp(icon.fallback, expected) == 0;
}
} // namespace
int main() {
  try {
    // The mode buttons are text, the titles the macOS toolbar buttons carry.
    require(text(toolbar_icon(kToolbarLanguage, true), L"中"));
    require(text(toolbar_icon(kToolbarLanguage, false), L"英"));
    require(text(toolbar_icon(kToolbarFullwidth, true), L"全"));
    require(text(toolbar_icon(kToolbarFullwidth, false), L"半"));
    require(text(toolbar_icon(kToolbarPunctuation, true), L"。"));
    // 西文标点是句点，与 macOS 一样。
    require(text(toolbar_icon(kToolbarPunctuation, false), L"."));
    require(text(toolbar_icon(kToolbarCharacterSet, true), L"繁"));
    require(text(toolbar_icon(kToolbarCharacterSet, false), L"简"));
    // The utility buttons keep the icon font's glyphs, the counterparts of macOS's SF Symbols.
    require(toolbar_icon(kToolbarEmoji, std::nullopt).codepoint == 0xE76E);
    require(toolbar_icon(kToolbarScreenKeyboard, std::nullopt).codepoint == 0xE765);
    require(toolbar_icon(kToolbarSettings, std::nullopt).codepoint == 0xE713);
    // 手写和语音与托盘工具条同一个字形，切换输入方案是 BulletedList。
    require(toolbar_icon(kToolbarHandwriting, std::nullopt).codepoint == 0xE70F);
    require(std::wcscmp(toolbar_icon(kToolbarHandwriting, std::nullopt).fallback, L"写") == 0);
    require(toolbar_icon(kToolbarVoice, std::nullopt).codepoint == 0xE720);
    require(std::wcscmp(toolbar_icon(kToolbarVoice, std::nullopt).fallback, L"音") == 0);
    require(toolbar_icon(kToolbarInputScheme, std::nullopt).codepoint == 0xE8FD);
    require(std::wcscmp(toolbar_icon(kToolbarInputScheme, std::nullopt).fallback, L"方") == 0);
    // Text is drawn in the Chinese UI font at macOS's 0.95 of the font size; only the Tibetan badge names a face of its own.
    require(std::wcscmp(toolbar_text_font, L"Microsoft YaHei UI") == 0);
    static_assert(toolbar_glyph_scale == 0.95f);
    for (int button = kToolbarLanguage; button <= kToolbarInputScheme; ++button)
      for (const std::optional<bool> state :
           {std::optional<bool>{}, std::optional<bool>{true}, std::optional<bool>{false}})
        require(toolbar_icon(button, state).family == nullptr);

    // An unreported mode is a question mark, not a guessed state. Showing 中
    // when the Server has not said so would tell the user the wrong mode.
    for (const int button : {kToolbarLanguage, kToolbarFullwidth,
                             kToolbarPunctuation, kToolbarCharacterSet})
      require(text(toolbar_icon(button, std::nullopt), L"?"));

    // Every icon carries non-empty text: the mode buttons draw nothing else, and the glyph fonts are not present on every Windows build, where a missing glyph draws a blank box.
    for (int button = kToolbarLanguage; button <= kToolbarInputScheme; ++button)
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
      require(std::wcscmp(on.fallback, off.fallback) != 0);
    }

    // Distinct buttons must not share a glyph or a text either.
    std::set<wchar_t> glyphs;
    std::set<std::wstring> texts;
    for (const auto &entry :
         {toolbar_icon(kToolbarLanguage, true), toolbar_icon(kToolbarLanguage, false),
          toolbar_icon(kToolbarFullwidth, true), toolbar_icon(kToolbarFullwidth, false),
          toolbar_icon(kToolbarPunctuation, true), toolbar_icon(kToolbarPunctuation, false),
          toolbar_icon(kToolbarCharacterSet, true), toolbar_icon(kToolbarCharacterSet, false),
          toolbar_icon(kToolbarEmoji, std::nullopt),
          toolbar_icon(kToolbarScreenKeyboard, std::nullopt),
          toolbar_icon(kToolbarSettings, std::nullopt),
          toolbar_icon(kToolbarHandwriting, std::nullopt),
          toolbar_icon(kToolbarVoice, std::nullopt),
          toolbar_icon(kToolbarInputScheme, std::nullopt)}) {
      if (entry.codepoint)
        require(glyphs.insert(entry.codepoint).second);
      require(texts.insert(entry.fallback).second);
    }

    // 隐藏按钮在来源里没有对应，也没有字形，所以保持文字。表外的 id（包括已经退役的「关于」id 9）画成问号，而不是空白。
    require(toolbar_icon(kToolbarHide, std::nullopt).codepoint == 0);
    require(text(toolbar_icon(9, std::nullopt), L"?"));
    require(text(toolbar_icon(99, true), L"?"));

    // The language button reflects more than Chinese/English.
    {
      ToolbarLanguageState caps;
      caps.caps_lock = true;
      // Caps Lock wins over every input mode: it changes what each letter key
      // does, so showing 中 there would say the wrong thing.
      require(text(toolbar_icon(kToolbarLanguage, true, caps), L"A"));
      require(text(toolbar_icon(kToolbarLanguage, false, caps), L"A"));
      // Even with an unreported mode, Caps Lock is known and shown.
      require(text(toolbar_icon(kToolbarLanguage, std::nullopt, caps), L"A"));

      ToolbarLanguageState japanese;
      japanese.mode = scheme::InputMode::Japanese;
      require(text(toolbar_icon(kToolbarLanguage, true, japanese), L"日"));
      // But the temporary English toggle beats it, as upstream orders these:
      // the toggle is what the next key actually does, and 日 over a keystroke
      // that produces Latin letters tells the user the wrong thing.
      require(text(toolbar_icon(kToolbarLanguage, false, japanese), L"英"));

      // 韩文、粤拼、注音、越南文、藏文和笔画同样画各自的字，排在 Caps Lock 和英文切换之后。藏文是 ཀ，与 macOS 一样，用能画藏文的字体。
      const std::pair<scheme::InputMode, const wchar_t *> drawn[] = {{scheme::InputMode::Korean, L"한"},
                                                                     {scheme::InputMode::Cantonese, L"粤"},
                                                                     {scheme::InputMode::Zhuyin, L"注"},
                                                                     {scheme::InputMode::Vietnamese, L"越"},
                                                                     {scheme::InputMode::Tibetan, L"ཀ"},
                                                                     {scheme::InputMode::Stroke, L"笔"}};
      for (const auto &[mode, expected] : drawn) {
        ToolbarLanguageState language;
        language.mode = mode;
        const auto icon = toolbar_icon(kToolbarLanguage, true, language);
        require(text(icon, expected));
        require((icon.family != nullptr) == (mode == scheme::InputMode::Tibetan));
        require(text(toolbar_icon(kToolbarLanguage, false, language), L"英"));
        language.caps_lock = true;
        require(text(toolbar_icon(kToolbarLanguage, true, language), L"A"));
      }
      ToolbarLanguageState tibetan;
      tibetan.mode = scheme::InputMode::Tibetan;
      require(std::wcscmp(toolbar_icon(kToolbarLanguage, true, tibetan).family, L"Microsoft Himalaya") == 0);

      // 双拼和五笔属于中文模式，按方案画 双 和 五；临时英文、Caps Lock 和 Engine 的英文模式照样排在前面，全拼仍是 中。
      for (const auto &[number, expected] :
           {std::pair{scheme::Shuangpin, L"双"}, std::pair{scheme::Wubi, L"五"}}) {
        ToolbarLanguageState language;
        language.scheme = number;
        require(text(toolbar_icon(kToolbarLanguage, true, language), expected));
        require(text(toolbar_icon(kToolbarLanguage, false, language), L"英"));
        language.dedicated_english = true;
        require(text(toolbar_icon(kToolbarLanguage, true, language), L"En"));
        language.caps_lock = true;
        require(text(toolbar_icon(kToolbarLanguage, true, language), L"A"));
      }
      // 发给 TIP 的模式本身就是双拼或五笔时（输入模式帧的 '8' 和 '9'），方案编号还不知道也画 双 和 五。
      for (const auto &[mode, expected] : {std::pair{scheme::InputMode::Shuangpin, L"双"},
                                           std::pair{scheme::InputMode::Wubi, L"五"}}) {
        ToolbarLanguageState language;
        language.mode = mode;
        require(text(toolbar_icon(kToolbarLanguage, true, language), expected));
        require(text(toolbar_icon(kToolbarLanguage, false, language), L"英"));
      }
      ToolbarLanguageState quanpin;
      quanpin.scheme = scheme::Quanpin;
      require(text(toolbar_icon(kToolbarLanguage, true, quanpin), L"中"));
      // 方案编号只在中文模式里起作用：日文模式下照样是 日。
      ToolbarLanguageState japanese_wubi;
      japanese_wubi.mode = scheme::InputMode::Japanese;
      japanese_wubi.scheme = scheme::Wubi;
      require(text(toolbar_icon(kToolbarLanguage, true, japanese_wubi), L"日"));

      // Caps Lock beats Japanese too, and the two together are not a fourth
      // state.
      ToolbarLanguageState both;
      both.caps_lock = true;
      both.mode = scheme::InputMode::Japanese;
      require(text(toolbar_icon(kToolbarLanguage, true, both), L"A"));

      // With neither, the button is the ordinary CN/EN pair.
      ToolbarLanguageState plain;
      require(text(toolbar_icon(kToolbarLanguage, true, plain), L"中"));
      require(text(toolbar_icon(kToolbarLanguage, false, plain), L"英"));
      require(text(toolbar_icon(kToolbarLanguage, std::nullopt, plain), L"?"));

      // Dedicated English is the Engine's own mode: a plain "En", as on macOS, with no underline.
      ToolbarLanguageState dedicated;
      dedicated.dedicated_english = true;
      require(text(toolbar_icon(kToolbarLanguage, true, dedicated), L"En"));
      // Unreported Chinese state does not suppress it: the dedicated mode is
      // known independently of the toggle.
      require(text(toolbar_icon(kToolbarLanguage, std::nullopt, dedicated), L"En"));

      // The temporary English toggle wins, because it is what the next key
      // actually does and it is what the user just pressed.
      require(text(toolbar_icon(kToolbarLanguage, false, dedicated), L"英"));

      // Caps Lock still beats everything.
      ToolbarLanguageState capped_english;
      capped_english.caps_lock = true;
      capped_english.dedicated_english = true;
      require(text(toolbar_icon(kToolbarLanguage, true, capped_english), L"A"));

      // Dedicated English beats Japanese; the two together are not a fifth
      // state.
      ToolbarLanguageState english_and_japanese;
      english_and_japanese.dedicated_english = true;
      english_and_japanese.mode = scheme::InputMode::Japanese;
      require(text(toolbar_icon(kToolbarLanguage, true, english_and_japanese), L"En"));

      // No other button is affected by the language state.
      require(toolbar_icon(kToolbarEmoji, std::nullopt, caps).codepoint ==
              toolbar_icon(kToolbarEmoji, std::nullopt, plain).codepoint);
      require(text(toolbar_icon(kToolbarFullwidth, true, caps), L"全"));
      require(text(toolbar_icon(kToolbarFullwidth, true, dedicated), L"全"));
    }

    std::cout << "Toolbar icons: mode buttons are text and utility buttons keep their glyphs\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Toolbar icon table failed with an unknown error\n";
    return 1;
  }
}
