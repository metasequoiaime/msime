#include "InputSchemeTraits.h"
#include <cassert>

using namespace msime::windows::scheme;

// The Engine-predicate mirrors are compared against crates/engine/src/types.rs by scripts/test-scheme-traits-parity.py; this covers what that script cannot read: the InputModeChanged codes, the configured-scheme spellings and the languages the modes write.
int main() {
  // The InputModeChanged wire codes the TIP reads.
  static_assert(input_mode_code(InputMode::Chinese) == L'0');
  static_assert(input_mode_code(InputMode::Japanese) == L'1');
  static_assert(input_mode_code(InputMode::Korean) == L'2');
  static_assert(input_mode_code(InputMode::Cantonese) == L'3');
  static_assert(input_mode_code(InputMode::Zhuyin) == L'4');
  static_assert(input_mode_code(InputMode::Vietnamese) == L'5');
  static_assert(input_mode_code(InputMode::Tibetan) == L'6');
  for (const auto mode : {InputMode::Chinese, InputMode::Japanese, InputMode::Korean, InputMode::Cantonese,
                          InputMode::Zhuyin, InputMode::Vietnamese, InputMode::Tibetan})
    assert(input_mode_from_code(input_mode_code(mode)) == mode);
  assert(input_mode_from_code(L'9') == InputMode::Chinese);
  assert(input_mode_from_code(L'\0') == InputMode::Chinese);

  // Every configured scheme maps to the mode its view reports, and the mode back to a scheme with the same traits.
  const char *names[] = {"quanpin", "shuangpin", "wubi",       "japanese", "korean",
                         "cantonese", "zhuyin",   "vietnamese", "tibetan"};
  for (int scheme = Quanpin; scheme <= Tibetan; ++scheme) {
    assert(scheme_from_name(names[scheme]) == scheme);
    assert(input_mode(names[scheme]) == input_mode(scheme));
    const int representative = mode_scheme(input_mode(scheme));
    assert(LetterComposition(representative) == LetterComposition(scheme));
    assert(OpensCandidateList(representative) == OpensCandidateList(scheme));
    assert(UsesChinesePunctuation(representative) == UsesChinesePunctuation(scheme));
    assert(HostSmartPunctuation(representative) == HostSmartPunctuation(scheme));
    assert(ScriptConversionApplies(representative) == ScriptConversionApplies(scheme));
    assert(CommitsOnBlur(representative) == CommitsOnBlur(scheme));
  }
  for (int scheme = Quanpin; scheme <= Tibetan; ++scheme)
    assert(scheme_from_name(scheme_name(scheme)) == scheme);
  assert(scheme_name(9).empty() && scheme_name(-1).empty());

  // The scheme that runs, as host-api's effective_scheme picks it: Cantonese and Zhuyin need their dictionary, and without it the last Chinese scheme that can run takes over, then quanpin.
  constexpr LanguageDictionaryPresence none{};
  constexpr LanguageDictionaryPresence both{true, true};
  constexpr LanguageDictionaryPresence cantonese_only{true, false};
  for (int scheme = Quanpin; scheme <= Tibetan; ++scheme)
    assert(effective_scheme(names[scheme], "wubi", both) == scheme);
  assert(effective_scheme("zhuyin", "shuangpin", none) == Shuangpin);
  assert(effective_scheme("zhuyin", "cantonese", cantonese_only) == Cantonese);
  assert(effective_scheme("zhuyin", "zhuyin", cantonese_only) == Quanpin);
  assert(effective_scheme("cantonese", "", none) == Quanpin);
  assert(effective_scheme("vietnamese", "zhuyin", none) == Vietnamese);
  assert(effective_scheme("tibetan", "zhuyin", none) == Tibetan);
  assert(effective_scheme("pinyin", "wubi", none) == Wubi);
  assert(effective_scheme("japanese", "korean", none) == Japanese);
  // A last_chinese_scheme that names a language is not a Chinese scheme to return to.
  assert(effective_scheme("zhuyin", "japanese", none) == Quanpin);
  assert(scheme_from_name("pinyin") == -1 && input_mode("pinyin") == InputMode::Chinese);
  assert(input_mode(-1) == InputMode::Chinese && input_mode(9) == InputMode::Chinese);

  // 粤拼和注音是 `last_chinese_scheme` 会记住的中文方案；日文、韩文、越南文和藏文各是独立的语言。
  for (const char *chinese : {"quanpin", "shuangpin", "wubi", "cantonese", "zhuyin"}) {
    assert(is_chinese_scheme_name(chinese));
    assert(input_language(input_mode(chinese)) == InputLanguage::Chinese);
  }
  assert(!is_chinese_scheme_name("japanese") && input_language(InputMode::Japanese) == InputLanguage::Japanese);
  assert(!is_chinese_scheme_name("korean") && input_language(InputMode::Korean) == InputLanguage::Korean);
  assert(!is_chinese_scheme_name("vietnamese") && input_language(InputMode::Vietnamese) == InputLanguage::Vietnamese);
  assert(!is_chinese_scheme_name("tibetan") && input_language(InputMode::Tibetan) == InputLanguage::Tibetan);
  assert(!is_chinese_scheme_name("pinyin"));

  // Schemes 0-4 keep the rules they had before Cantonese, Zhuyin and Vietnamese existed: only the pinyin and shape schemes convert to Traditional, only Korean composes letters into text or opens its list.
  assert(ScriptConversionApplies(Quanpin) && ScriptConversionApplies(Wubi) && !ScriptConversionApplies(Japanese) &&
         !ScriptConversionApplies(Korean));
  assert(!ScriptConversionApplies(Cantonese) && !ScriptConversionApplies(Zhuyin) && !ScriptConversionApplies(Vietnamese));
  assert(LetterComposition(Korean) && LetterComposition(Vietnamese) && !LetterComposition(Zhuyin));
  assert(OpensCandidateList(Korean) && OpensCandidateList(Zhuyin) && !OpensCandidateList(Vietnamese));
  assert(AlwaysInlinePreedit(Zhuyin) && AlwaysInlinePreedit(Vietnamese) && !AlwaysInlinePreedit(Cantonese));
  assert(FoldsLetterCase(Korean) && !FoldsLetterCase(Vietnamese));
  // 藏文和越南文一样字母直接组字、组字总是画在行内，没有可打开的列表；威利转写区分大小写，所以不折叠大小写，Caps Lock 打出的大写字母也照样组字。
  assert(LetterComposition(Tibetan) && AlwaysInlinePreedit(Tibetan) && !OpensCandidateList(Tibetan));
  assert(!FoldsLetterCase(Tibetan) && CapsLockBypassExempt(Tibetan) && CapsLockBypassExempt(Vietnamese));
  assert(!UsesChinesePunctuation(Tibetan) && !WidensFullWidth(Tibetan) && !HostSmartPunctuation(Tibetan) &&
         !ScriptConversionApplies(Tibetan) && !IsChinese(Tibetan) && !ShowsGlosses(Tibetan));
  assert(CommitsOnBlur(Tibetan) && LocksCaret(Tibetan));
  // 只有越南文词和藏文音节串在 Esc 之后继续组字；其他组字（包括方案 0-4）都被 Esc 丢弃。
  for (int scheme = Quanpin; scheme <= Tibetan; ++scheme)
    assert(CancelRestoresRaw(scheme) == (scheme == Vietnamese || scheme == Tibetan));
  // Only the Zhuyin list refuses the mouse; the Korean Hanja list and every Chinese list stay clickable, and an unknown number keeps the mouse too.
  for (int scheme = Quanpin; scheme <= Tibetan + 1; ++scheme)
    assert(KeyboardOnlyCandidateList(scheme) == (scheme == Zhuyin));
  // Every mode code passes the TIP's frame check, the ones after Chinese and Japanese included, and so does a code from a newer Server, which reads as Chinese; an empty payload or one longer than a character does not.
  for (const auto mode : {InputMode::Chinese, InputMode::Japanese, InputMode::Korean, InputMode::Cantonese,
                          InputMode::Zhuyin, InputMode::Vietnamese, InputMode::Tibetan}) {
    const wchar_t payload[4] = {input_mode_code(mode), L'\0', L'\0', L'\0'};
    assert(is_input_mode_payload(payload, 4));
    assert(input_mode_from_code(payload[0]) == mode);
  }
  const wchar_t newer[4] = {L'9', L'\0', L'\0', L'\0'};
  assert(is_input_mode_payload(newer, 4) && input_mode_from_code(newer[0]) == InputMode::Chinese);
  const wchar_t empty[4] = {L'\0', L'\0', L'\0', L'\0'};
  const wchar_t longer[4] = {L'2', L'2', L'\0', L'\0'};
  assert(!is_input_mode_payload(empty, 4) && !is_input_mode_payload(longer, 4));
  assert(!is_input_mode_payload(newer, 1));
  return 0;
}
