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
  static_assert(input_mode_code(InputMode::Stroke) == L'7');
  static_assert(input_mode_code(InputMode::Shuangpin) == L'8');
  static_assert(input_mode_code(InputMode::Wubi) == L'9');
  for (const auto mode : {InputMode::Chinese, InputMode::Japanese, InputMode::Korean, InputMode::Cantonese,
                          InputMode::Zhuyin, InputMode::Vietnamese, InputMode::Tibetan, InputMode::Stroke,
                          InputMode::Shuangpin, InputMode::Wubi})
    assert(input_mode_from_code(input_mode_code(mode)) == mode);
  assert(input_mode_from_code(L':') == InputMode::Chinese);
  assert(input_mode_from_code(L'\0') == InputMode::Chinese);

  // Every configured scheme maps to the mode its view reports, and the mode back to a scheme with the same traits.
  const char *names[] = {"quanpin",   "shuangpin", "wubi",       "japanese", "korean",
                         "cantonese", "zhuyin",    "vietnamese", "tibetan",  "stroke"};
  for (int scheme = Quanpin; scheme <= Stroke; ++scheme) {
    assert(scheme_from_name(names[scheme]) == scheme);
    assert(input_mode(names[scheme]) == input_mode(scheme));
    const int representative = mode_scheme(input_mode(scheme));
    assert(LetterComposition(representative) == LetterComposition(scheme));
    assert(OpensCandidateList(representative) == OpensCandidateList(scheme));
    assert(UsesChinesePunctuation(representative) == UsesChinesePunctuation(scheme));
    assert(HostSmartPunctuation(representative) == HostSmartPunctuation(scheme));
    assert(ScriptConversionApplies(representative) == ScriptConversionApplies(scheme));
    assert(CommitsOnBlur(representative) == CommitsOnBlur(scheme));
    assert(OpensTableModes(representative) == OpensTableModes(scheme));
    assert(LetterPassesWhileIdle(representative, false, L'x') == LetterPassesWhileIdle(scheme, false, L'x'));
  }
  for (int scheme = Quanpin; scheme <= Stroke; ++scheme)
    assert(scheme_from_name(scheme_name(scheme)) == scheme);
  assert(scheme_name(10).empty() && scheme_name(-1).empty());
  // 笔画有自己的模式：它的 trait 与全拼不同，TIP 不能把它当中文模式处理。
  assert(input_mode(Stroke) == InputMode::Stroke && mode_scheme(InputMode::Stroke) == Stroke);
  assert(input_mode("stroke") == InputMode::Stroke);
  // 双拼和五笔的码只给语言栏图标用：TIP 仍按全拼键入它们，语言是中文。
  assert(input_mode(Shuangpin) == InputMode::Shuangpin && mode_scheme(InputMode::Shuangpin) == Quanpin);
  assert(input_mode(Wubi) == InputMode::Wubi && mode_scheme(InputMode::Wubi) == Quanpin);
  assert(input_language(InputMode::Shuangpin) == InputLanguage::Chinese &&
         input_language(InputMode::Wubi) == InputLanguage::Chinese);

  // The scheme that runs, as host-api's effective_scheme picks it: Cantonese, Zhuyin and Stroke need their dictionary, and without it the last Chinese scheme that can run takes over, then quanpin.
  constexpr LanguageDictionaryPresence none{};
  constexpr LanguageDictionaryPresence all{true, true, true};
  constexpr LanguageDictionaryPresence cantonese_only{true, false, false};
  constexpr LanguageDictionaryPresence stroke_only{false, false, true};
  constexpr auto full = all_schemes();
  for (int scheme = Quanpin; scheme <= Stroke; ++scheme)
    assert(effective_scheme(names[scheme], "wubi", all, full) == scheme);
  assert(effective_scheme("stroke", "shuangpin", none, full) == Shuangpin);
  assert(effective_scheme("stroke", "shuangpin", cantonese_only, full) == Shuangpin);
  assert(effective_scheme("stroke", "", stroke_only, full) == Stroke);
  assert(effective_scheme("japanese", "stroke", stroke_only, full) == Japanese);
  // 笔画是可以切回的中文方案，但只在它的词库存在时。
  assert(effective_scheme("zhuyin", "stroke", stroke_only, full) == Stroke);
  assert(effective_scheme("zhuyin", "stroke", cantonese_only, full) == Quanpin);
  assert(!scheme_installed(Stroke, cantonese_only) && scheme_installed(Stroke, stroke_only));
  assert(scheme_installed(Tibetan, none) && !scheme_installed(10, all));
  assert(effective_scheme("zhuyin", "shuangpin", none, full) == Shuangpin);
  assert(effective_scheme("zhuyin", "cantonese", cantonese_only, full) == Cantonese);
  assert(effective_scheme("zhuyin", "zhuyin", cantonese_only, full) == Quanpin);
  assert(effective_scheme("cantonese", "", none, full) == Quanpin);
  assert(effective_scheme("vietnamese", "zhuyin", none, full) == Vietnamese);
  assert(effective_scheme("tibetan", "zhuyin", none, full) == Tibetan);
  assert(effective_scheme("pinyin", "wubi", none, full) == Wubi);
  assert(effective_scheme("japanese", "korean", none, full) == Japanese);
  // A last_chinese_scheme that names a language is not a Chinese scheme to return to.
  assert(effective_scheme("zhuyin", "japanese", none, full) == Quanpin);

  // 只提供五笔的版本：配置里写着它不提供的方案（比如从账号同步下来的全拼）时退回五笔，上次的中文方案也要是它提供的才算数。
  OfferedSchemes wubi;
  wubi.offered[Wubi] = true;
  wubi.fallback = Wubi;
  assert(effective_scheme("wubi", "quanpin", all, wubi) == Wubi);
  assert(effective_scheme("quanpin", "quanpin", all, wubi) == Wubi);
  assert(effective_scheme("japanese", "shuangpin", all, wubi) == Wubi);
  // 拼音版：双拼可用，日文不在其中，回到上次的双拼。
  OfferedSchemes pinyin;
  pinyin.offered[Quanpin] = pinyin.offered[Shuangpin] = true;
  assert(effective_scheme("japanese", "shuangpin", all, pinyin) == Shuangpin);
  assert(effective_scheme("wubi", "wubi", all, pinyin) == Quanpin);
  // 本次构建的版本提供它自己的默认方案，full 提供全部方案。
  constexpr auto built = edition_schemes();
  assert(built.offers(built.fallback));
  if constexpr (MSIME_EDITION_IS_FULL != 0)
    for (int scheme = Quanpin; scheme <= Stroke; ++scheme)
      assert(built.offers(scheme) && built.fallback == Quanpin);
  assert(scheme_from_name("pinyin") == -1 && input_mode("pinyin") == InputMode::Chinese);
  assert(input_mode(-1) == InputMode::Chinese && input_mode(10) == InputMode::Chinese);

  // 粤拼、注音和笔画是 `last_chinese_scheme` 会记住的中文方案；日文、韩文、越南文和藏文各是独立的语言。
  for (const char *chinese : {"quanpin", "shuangpin", "wubi", "cantonese", "zhuyin", "stroke"}) {
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
  // Stroke takes Cantonese's traits: a Chinese scheme in the candidate window, its characters written as stored, Chinese punctuation with the smart gestures, no glosses, and nothing committed on blur.
  assert(IsChinese(Stroke) && !ScriptConversionApplies(Stroke) && OutputsTraditionalNatively(Stroke));
  assert(UsesChinesePunctuation(Stroke) && HostSmartPunctuation(Stroke) && WidensFullWidth(Stroke) && !ShowsGlosses(Stroke));
  assert(!CommitsOnBlur(Stroke) && !LocksCaret(Stroke));
  assert(!LetterComposition(Stroke) && !OpensCandidateList(Stroke) && !AlwaysInlinePreedit(Stroke));
  assert(!CancelRestoresRaw(Stroke) && !KeyboardOnlyCandidateList(Stroke) && !FoldsLetterCase(Stroke) &&
         !CapsLockBypassExempt(Stroke));
  // With nothing composing only the five strokes start a Stroke composition; the wildcard, the other letters and a capital belong to the application. No other scheme hands a letter back this way.
  for (const wchar_t stroke : {L'h', L's', L'p', L'n', L'z'})
    assert(!LetterPassesWhileIdle(Stroke, false, stroke));
  for (const wchar_t other : {L'x', L'a', L'q', L'H', L'X'})
    assert(LetterPassesWhileIdle(Stroke, false, other));
  assert(!LetterPassesWhileIdle(Stroke, false, L'1') && !LetterPassesWhileIdle(Stroke, false, L',') &&
         !LetterPassesWhileIdle(Stroke, false, L' '));
  for (int scheme = Quanpin; scheme <= Tibetan; ++scheme)
    assert(!LetterPassesWhileIdle(scheme, false, L'x') && !LetterPassesWhileIdle(scheme, false, L'a'));
  // In the Engine's own English mode under Stroke (Ctrl+Shift+E, the toolbar English) the Engine composes every letter as English before it looks at the scheme, so "apple" must reach it from its first letter: none passes to the application.
  for (const wchar_t letter : {L'a', L'x', L'q', L'h', L'A'})
    assert(!LetterPassesWhileIdle(Stroke, true, letter));
  // Only Stroke takes a composing apostrophe as punctuation; the pinyin schemes (Wubi included, which the TIP keys as quanpin) keep it as the separator.
  for (int scheme = Quanpin; scheme <= Stroke + 1; ++scheme)
    assert(ApostropheIsPunctuationWhileComposing(scheme) == (scheme == Stroke));
  assert(FoldsLetterCase(Korean) && !FoldsLetterCase(Vietnamese));
  // 藏文和越南文一样字母直接组字、组字总是画在行内，没有可打开的列表；威利转写区分大小写，所以不折叠大小写，Caps Lock 打出的大写字母也照样组字。
  assert(LetterComposition(Tibetan) && AlwaysInlinePreedit(Tibetan) && !OpensCandidateList(Tibetan));
  assert(!FoldsLetterCase(Tibetan) && CapsLockBypassExempt(Tibetan) && CapsLockBypassExempt(Vietnamese));
  assert(!UsesChinesePunctuation(Tibetan) && !WidensFullWidth(Tibetan) && !HostSmartPunctuation(Tibetan) &&
         !ScriptConversionApplies(Tibetan) && !IsChinese(Tibetan) && !ShowsGlosses(Tibetan));
  assert(CommitsOnBlur(Tibetan) && LocksCaret(Tibetan));
  // 只有越南文词和藏文音节串在 Esc 之后继续组字；其他组字（包括方案 0-4 和笔画）都被 Esc 丢弃。
  for (int scheme = Quanpin; scheme <= Stroke; ++scheme)
    assert(CancelRestoresRaw(scheme) == (scheme == Vietnamese || scheme == Tibetan));
  // 整句改字只在全拼和双拼里有；未知的方案号也没有。
  for (int scheme = Quanpin; scheme <= Stroke + 1; ++scheme)
    assert(EditsSentence(scheme) == (scheme == Quanpin || scheme == Shuangpin));
  // Only the Zhuyin list refuses the mouse; the Korean Hanja list and every Chinese list stay clickable, and an unknown number keeps the mouse too.
  for (int scheme = Quanpin; scheme <= Stroke + 1; ++scheme)
    assert(KeyboardOnlyCandidateList(scheme) == (scheme == Zhuyin));
  // Every mode code passes the TIP's frame check, the ones after Chinese and Japanese included, and so does a code from a newer Server, which reads as Chinese; an empty payload or one longer than a character does not.
  for (const auto mode : {InputMode::Chinese, InputMode::Japanese, InputMode::Korean, InputMode::Cantonese,
                          InputMode::Zhuyin, InputMode::Vietnamese, InputMode::Tibetan, InputMode::Stroke,
                          InputMode::Shuangpin, InputMode::Wubi}) {
    const wchar_t payload[4] = {input_mode_code(mode), L'\0', L'\0', L'\0'};
    assert(is_input_mode_payload(payload, 4));
    assert(input_mode_from_code(payload[0]) == mode);
  }
  const wchar_t newer[4] = {L':', L'\0', L'\0', L'\0'};
  assert(is_input_mode_payload(newer, 4) && input_mode_from_code(newer[0]) == InputMode::Chinese);
  const wchar_t empty[4] = {L'\0', L'\0', L'\0', L'\0'};
  const wchar_t longer[4] = {L'2', L'2', L'\0', L'\0'};
  assert(!is_input_mode_payload(empty, 4) && !is_input_mode_payload(longer, 4));
  assert(!is_input_mode_payload(newer, 1));
  return 0;
}
