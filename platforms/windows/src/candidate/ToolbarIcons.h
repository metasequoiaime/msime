#pragma once
#include "InputSchemeTraits.h"
#include <optional>

namespace msime::windows {
// A toolbar button's icon.
//
// The four mode buttons - language, punctuation, full width and character set - are text, as on macOS, where they are the button titles 中/英/A/En/双/五/日/한/粤/注/越/ཀ/笔, 。or ., 全/半 and 简/繁 in the regular system font at 0.95 of the toolbar font size. They carry no codepoint and draw `fallback` in `toolbar_text_font` (or `family` when set) at `toolbar_glyph_scale`. The icon font's own 中/英/全/半/简/繁 glyphs are drawings of a different shape, not those characters, and Segoe Fluent Icons is missing on Windows 10 altogether.
//
// The utility buttons (input scheme, emoji, handwriting, keyboard, voice, settings) are glyphs of the installed icon font, the counterparts of macOS's SF Symbols, at the full font size. Each carries a text fallback on purpose: "Segoe Fluent Icons" ships with Windows 11 only, and the Windows 10 build of "Segoe MDL2 Assets" may predate some of these codepoints. DirectWrite substitutes a font silently rather than failing, so a button whose glyph is missing renders as a blank box unless something checks first and draws the text instead.
struct ToolbarIcon {
  wchar_t codepoint = 0;
  const wchar_t *fallback = L"";
  // The face for the text, when it is not `toolbar_text_font`: the Tibetan badge needs a font that has Tibetan.
  const wchar_t *family = nullptr;
};
// The face the text buttons are drawn in: the Chinese UI font, regular weight, the counterpart of the macOS system font that draws them. DirectWrite falls back per character for what it lacks (한).
inline constexpr const wchar_t *toolbar_text_font = L"Microsoft YaHei UI";
// The Tibetan badge's face. Microsoft Himalaya ships with Windows; where it is missing, DirectWrite's own fallback picks whatever covers Tibetan.
inline constexpr const wchar_t *toolbar_tibetan_font = L"Microsoft Himalaya";
// Text buttons are drawn at this fraction of the toolbar font size, as macOS's kToolbarGlyphScale.
inline constexpr float toolbar_glyph_scale = 0.95f;
// Button ids as the toolbar's slot order uses them.
enum ToolbarButton {
  kToolbarLanguage = 0,
  kToolbarFullwidth = 1,
  kToolbarPunctuation = 2,
  kToolbarCharacterSet = 3,
  kToolbarEmoji = 4,
  kToolbarScreenKeyboard = 5,
  kToolbarSettings = 6,
  // 手写和语音回到工具栏时沿用它们原来的 7 和 8，含义没有变。9 曾经是「关于」，现在在设置按钮的右键菜单里，这个 id 不再分配，免得旧布局指到别的按钮。
  kToolbarHandwriting = 7,
  kToolbarVoice = 8,
  kToolbarHide = 10,
  // 切换输入方案，点开列出这个版本提供的方案。
  kToolbarInputScheme = 11,
};
// The icon for one button. `state` is that button's two-way mode - Chinese,
// full width, Chinese punctuation, traditional output - and is absent when the
// Server has not reported it yet. An unreported mode shows a question mark
// rather than a guessed state, which would tell the user the wrong thing.
// Extra state the language button reflects beyond Chinese/English.
//
// 语言按钮显示的字与 macOS 相同：Caps Lock 开着时是 A，临时英文是 英，Engine 自己的英文模式是 En，日文模式是 日，韩文模式是 한，粤拼是 粤，注音是 注，越南文是 越，藏文是 ཀ，笔画是 笔，双拼是 双，五笔是 五，其余是 中。Caps Lock 开着时还显示 中，会让用户误判下一个字母键的作用。任务栏语言栏图标不走这里，藏文在那里仍是「藏」。
struct ToolbarLanguageState {
  bool caps_lock = false;
  // The configured scheme's family, as the TIP is told it.
  scheme::InputMode mode = scheme::InputMode::Chinese;
  // 正在运行的方案编号（scheme::Quanpin 等），-1 表示还不知道。中文模式里用它区分双拼和五笔，按钮显示 双 或 五，与 macOS 的方案徽标一致。
  int scheme = -1;
  // The Engine's own English mode, as opposed to the temporary Chinese/English
  // toggle carried in `state`. It outlives a commit, so it is worth telling
  // apart on the button.
  bool dedicated_english = false;
};
inline ToolbarIcon toolbar_icon(int button, std::optional<bool> state,
                                ToolbarLanguageState language = {}) {
  const ToolbarIcon unknown{0, L"?"};
  switch (button) {
  case kToolbarLanguage:
    // Caps Lock wins over everything: it changes what every letter key does,
    // whatever input mode is selected.
    if (language.caps_lock)
      return {0, L"A"};
    // The temporary toggle wins over the dedicated mode: it is what the next
    // key actually does, and it is the one the user just pressed.
    if (state && !*state)
      return {0, L"英"};
    // The Engine's own English mode is a plain "En", as on macOS; it is told apart from the temporary toggle by the letters, not by an underline.
    if (language.dedicated_english)
      return {0, L"En"};
    switch (language.mode) {
    case scheme::InputMode::Japanese:
      return {0, L"日"};
    case scheme::InputMode::Korean:
      return {0, L"한"};
    case scheme::InputMode::Cantonese:
      return {0, L"粤"};
    case scheme::InputMode::Zhuyin:
      return {0, L"注"};
    case scheme::InputMode::Vietnamese:
      return {0, L"越"};
    case scheme::InputMode::Tibetan:
      return {0, L"ཀ", toolbar_tibetan_font};
    case scheme::InputMode::Stroke:
      return {0, L"笔"};
    case scheme::InputMode::Shuangpin:
      return {0, L"双"};
    case scheme::InputMode::Wubi:
      return {0, L"五"};
    case scheme::InputMode::Chinese:
      // 双拼和五笔同属中文模式，按运行中的方案画 双 或 五。
      if (language.scheme == scheme::Shuangpin)
        return {0, L"双"};
      if (language.scheme == scheme::Wubi)
        return {0, L"五"};
      break;
    }
    if (!state)
      return unknown;
    return {0, L"中"};
  case kToolbarFullwidth:
    if (!state)
      return unknown;
    return *state ? ToolbarIcon{0, L"全"} : ToolbarIcon{0, L"半"};
  case kToolbarPunctuation:
    if (!state)
      return unknown;
    // Western punctuation is a full stop, as on macOS.
    return *state ? ToolbarIcon{0, L"。"} : ToolbarIcon{0, L"."};
  case kToolbarCharacterSet:
    if (!state)
      return unknown;
    // state is "traditional output is on".
    return *state ? ToolbarIcon{0, L"繁"} : ToolbarIcon{0, L"简"};
  case kToolbarEmoji:
    return {0xE76E, L"表"}; // 表
  case kToolbarScreenKeyboard:
    return {0xE765, L"键"}; // 键
  case kToolbarSettings:
    return {0xE713, L"设"}; // 设
  // 手写和语音与托盘工具条用同一个字形和替代字。
  case kToolbarHandwriting:
    return {0xE70F, L"写"};
  case kToolbarVoice:
    return {0xE720, L"音"};
  // BulletedList，对应 macOS 的 list.bullet。
  case kToolbarInputScheme:
    return {0xE8FD, L"方"};
  case kToolbarHide:
    return {0, L"×"};
  default:
    return unknown;
  }
}
} // namespace msime::windows
