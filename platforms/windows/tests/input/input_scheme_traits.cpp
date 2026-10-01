#include "InputSchemeTraits.h"
#include <cassert>

using namespace msime::windows::scheme;

// The Engine-predicate mirrors are compared against crates/engine/src/types.rs by scripts/test-scheme-traits-parity.py; this covers what that script cannot read: the InputModeChanged codes, the configured-scheme spellings and the languages the modes write.
int main() {
  // The wire codes an older DLL reads: it knows '1' and '2' only, so every newer code stays Chinese there.
  static_assert(input_mode_code(InputMode::Chinese) == L'0');
  static_assert(input_mode_code(InputMode::Japanese) == L'1');
  static_assert(input_mode_code(InputMode::Korean) == L'2');
  static_assert(input_mode_code(InputMode::Cantonese) == L'3');
  static_assert(input_mode_code(InputMode::Zhuyin) == L'4');
  static_assert(input_mode_code(InputMode::Vietnamese) == L'5');
  for (const auto mode : {InputMode::Chinese, InputMode::Japanese, InputMode::Korean, InputMode::Cantonese,
                          InputMode::Zhuyin, InputMode::Vietnamese})
    assert(input_mode_from_code(input_mode_code(mode)) == mode);
  assert(input_mode_from_code(L'9') == InputMode::Chinese);
  assert(input_mode_from_code(L'\0') == InputMode::Chinese);

  // Every configured scheme maps to the mode its view reports, and the mode back to a scheme with the same traits.
  const char *names[] = {"quanpin", "shuangpin", "wubi", "japanese", "korean", "cantonese", "zhuyin", "vietnamese"};
  for (int scheme = Quanpin; scheme <= Vietnamese; ++scheme) {
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
  for (int scheme = Quanpin; scheme <= Vietnamese; ++scheme)
    assert(scheme_from_name(scheme_name(scheme)) == scheme);
  assert(scheme_name(8).empty() && scheme_name(-1).empty());

  // The scheme that runs, as host-api's effective_scheme picks it: Cantonese and Zhuyin need their dictionary, and without it the last Chinese scheme that can run takes over, then quanpin.
  constexpr LanguageDictionaryPresence none{};
  constexpr LanguageDictionaryPresence both{true, true};
  constexpr LanguageDictionaryPresence cantonese_only{true, false};
  for (int scheme = Quanpin; scheme <= Vietnamese; ++scheme)
    assert(effective_scheme(names[scheme], "wubi", both) == scheme);
  assert(effective_scheme("zhuyin", "shuangpin", none) == Shuangpin);
  assert(effective_scheme("zhuyin", "cantonese", cantonese_only) == Cantonese);
  assert(effective_scheme("zhuyin", "zhuyin", cantonese_only) == Quanpin);
  assert(effective_scheme("cantonese", "", none) == Quanpin);
  assert(effective_scheme("vietnamese", "zhuyin", none) == Vietnamese);
  assert(effective_scheme("pinyin", "wubi", none) == Wubi);
  assert(effective_scheme("japanese", "korean", none) == Japanese);
  // A last_chinese_scheme that names a language is not a Chinese scheme to return to.
  assert(effective_scheme("zhuyin", "japanese", none) == Quanpin);
  assert(scheme_from_name("pinyin") == -1 && input_mode("pinyin") == InputMode::Chinese);
  assert(input_mode(-1) == InputMode::Chinese && input_mode(8) == InputMode::Chinese);

  // Cantonese and Zhuyin are Chinese schemes `last_chinese_scheme` remembers; Japanese, Korean and Vietnamese are languages of their own.
  for (const char *chinese : {"quanpin", "shuangpin", "wubi", "cantonese", "zhuyin"}) {
    assert(is_chinese_scheme_name(chinese));
    assert(input_language(input_mode(chinese)) == InputLanguage::Chinese);
  }
  assert(!is_chinese_scheme_name("japanese") && input_language(InputMode::Japanese) == InputLanguage::Japanese);
  assert(!is_chinese_scheme_name("korean") && input_language(InputMode::Korean) == InputLanguage::Korean);
  assert(!is_chinese_scheme_name("vietnamese") && input_language(InputMode::Vietnamese) == InputLanguage::Vietnamese);
  assert(!is_chinese_scheme_name("pinyin"));

  // Schemes 0-4 keep the rules they had before Cantonese, Zhuyin and Vietnamese existed: only the pinyin and shape schemes convert to Traditional, only Korean composes letters into text or opens its list.
  assert(ScriptConversionApplies(Quanpin) && ScriptConversionApplies(Wubi) && !ScriptConversionApplies(Japanese) &&
         !ScriptConversionApplies(Korean));
  assert(!ScriptConversionApplies(Cantonese) && !ScriptConversionApplies(Zhuyin) && !ScriptConversionApplies(Vietnamese));
  assert(LetterComposition(Korean) && LetterComposition(Vietnamese) && !LetterComposition(Zhuyin));
  assert(OpensCandidateList(Korean) && OpensCandidateList(Zhuyin) && !OpensCandidateList(Vietnamese));
  assert(AlwaysInlinePreedit(Zhuyin) && AlwaysInlinePreedit(Vietnamese) && !AlwaysInlinePreedit(Cantonese));
  assert(FoldsLetterCase(Korean) && !FoldsLetterCase(Vietnamese));
  // Only a Vietnamese word keeps composing after an Escape; every other composition, schemes 0-4 included, is discarded by it.
  for (int scheme = Quanpin; scheme <= Vietnamese; ++scheme)
    assert(CancelRestoresRaw(scheme) == (scheme == Vietnamese));
  // Only the Zhuyin list refuses the mouse; the Korean Hanja list and every Chinese list stay clickable, and an unknown number keeps the mouse too.
  for (int scheme = Quanpin; scheme <= Vietnamese + 1; ++scheme)
    assert(KeyboardOnlyCandidateList(scheme) == (scheme == Zhuyin));
  return 0;
}
