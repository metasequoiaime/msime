#pragma once
#include "KeyEvent.h"
#include "NavigationPolicy.h"
#include <string_view>

namespace msime::windows {
// Native routing only, not translation. Resolve input separators, Unicode
// selection and word-to-character priority before calling this predicate.
// The Japanese scheme never pages on minus/equals (the reference's IsJapaneseDisabledPagingKey), and the TSF sends those keys as punctuation there, so '_', '=' and '+' commit the highlighted candidate. A bare '-' stays the long-vowel mark.
// 韩文从不在标点上翻页，所以那里每个 ASCII 标点键都是标点，包括 '-' '=' '[' ']' ',' '.'，不论汉字列表是否打开：引擎关闭列表，把标点以半角写在当前音节之后，和其他宿主一样。列表用 Page Up/Down 和方向键翻页（KoreanHanjaKey.h）。注音、越南文和藏文也传入 `korean`：它们都不在标点上翻页，注音拼写注音符号的键和藏文的威利拼写符号在问到这里之前已经被 `edit_kind` 当作输入。
inline bool candidate_punctuation(const FanyImeNamedpipeData &packet,
                                  const NavigationBindings &bindings,
                                  bool japanese = false, bool korean = false) {
  if (translate_key(packet).kind != KeyKind::Character)
    return false;
  if (korean)
    return packet.wch >= 0x21 && packet.wch <= 0x7E &&
           !((packet.wch >= '0' && packet.wch <= '9') ||
             (packet.wch >= 'A' && packet.wch <= 'Z') ||
             (packet.wch >= 'a' && packet.wch <= 'z'));
  switch (packet.keycode) {
  case 0xBD:
  case 0xBB:
    if (japanese)
      return packet.wch == '_' || packet.wch == '=' || packet.wch == '+';
    return false;
  case 0x6D:
  case 0x6B:
    return false;
  case 0xBC:
  case 0xBE:
    if (bindings.comma_period)
      return false;
    break;
  case 0xDB:
  case 0xDD:
    if (bindings.brackets)
      return false;
    break;
  }
  constexpr std::string_view characters = "`!@#$%^&*()[]\\;:'\",<>.?";
  return packet.wch <= 127 && characters.find(static_cast<char>(packet.wch)) !=
                                  std::string_view::npos;
}

// The ASCII mark that follows the highlighted candidate literally, or 0 when the key's punctuation is translated as usual. The numpad arithmetic keys and '/' stay literal and the numpad decimal is always '.', so an expression or a path typed right after a candidate does not become Chinese punctuation (reference 1d2431ad). The numpad '+' and '-' are arithmetic, not the paging keys candidate_punctuation excludes. Only meaningful while composing: the caller routes these keys here only then.
inline char literal_candidate_punctuation(const FanyImeNamedpipeData &packet) {
  if (translate_key(packet).kind != KeyKind::Character)
    return 0;
  switch (packet.keycode) {
  case 0x6B:
    return '+';
  case 0x6D:
    return '-';
  case 0x6E:
    return '.';
  case 0x6F:
    return '/';
  }
  return packet.wch == '/' ? '/' : 0;
}
} // namespace msime::windows
