#pragma once

#include <string_view>

// Scheme behaviour the Windows Server and TIP decide from a view's `scheme` number or from the configured scheme. The view publishes `chinese_text`, `script_conversion`, `spelling_symbols` and `candidate_list_open` itself, and those are read from the view where it is at hand; everything here is either a host-only trait or an Engine trait the view does not carry. An unknown scheme number answers false everywhere, the way host-api reads `SchemeType::from_u8`. scripts/test-scheme-traits-parity.py checks every Engine mirror below against crates/engine/src/types.rs.
namespace msime::windows::scheme
{
// The Engine's `SchemeType` ordinals (crates/engine/src/types.rs), as they appear in a view's `scheme`.
constexpr int Quanpin = 0;
constexpr int Shuangpin = 1;
constexpr int Wubi = 2;
constexpr int Japanese = 3;
constexpr int Korean = 4;
constexpr int Cantonese = 5;
constexpr int Zhuyin = 6;
constexpr int Vietnamese = 7;

// ---- Host-only traits ----

// The letter the Engine receives takes its case from Shift alone, so Caps Lock does not change it (Dubeolsik binds jamo by case; see korean_letter in tsf/HostKoreanKey.h).
constexpr bool FoldsLetterCase(int scheme) { return scheme == Korean; }

// A Caps Lock uppercase letter that would start a composition is not handed back to the application: the scheme composes it (Korean folds it, Vietnamese keeps it uppercase).
constexpr bool CapsLockBypassExempt(int scheme) { return scheme == Korean || scheme == Vietnamese; }

// Letters build the written text directly (a Hangul syllable, a Vietnamese word) rather than a reading converted through candidates. The TIP writes that text from its own host session (ReplyPath::SyllableCommit), there is no word to take a character from, and every key outside the composition ends it.
constexpr bool LetterComposition(int scheme) { return scheme == Korean || scheme == Vietnamese; }

// Candidates appear only in a list the user opens (MSIME_OPEN_CANDIDATE_LIST: the Korean Hanja list, the Zhuyin list), and that list's keys follow common/KoreanHanjaKey.h rather than the Chinese navigation bindings.
constexpr bool OpensCandidateList(int scheme) { return scheme == Korean || scheme == Zhuyin; }

// The composition is always drawn inline whatever the preedit display preference says: until a list is opened there is no candidate window to show it in, and hidden it would be text the user cannot see being written.
constexpr bool AlwaysInlinePreedit(int scheme) { return LetterComposition(scheme) || OpensCandidateList(scheme); }

// ---- Engine traits the view does not publish; each mirrors the `SchemeType` predicate of the same name ----

// `is_chinese`: a Chinese scheme, the kind `last_chinese_scheme` remembers and the Chinese mode returns to.
constexpr bool IsChinese(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Zhuyin;
}

// `script_conversion_applies`: commits are Simplified and the Traditional output switch converts them. Cantonese and Zhuyin write Traditional as stored, so converting them again would be wrong; the view's own `script_conversion` also turns off in the Unicode and temporary Japanese modes.
constexpr bool ScriptConversionApplies(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi; }

// `outputs_traditional_natively`: the scheme's dictionary is Traditional, whatever the Traditional output switch says.
constexpr bool OutputsTraditionalNatively(int scheme) { return scheme == Cantonese || scheme == Zhuyin; }

// `commits_on_blur`: leaving the composition (focus loss, a scheme or mode switch, a navigation key handed to the application) writes it out instead of discarding it.
constexpr bool CommitsOnBlur(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `locks_caret`: the caret stays at the end of the composition, so there are no segments for Ctrl+Backspace and Ctrl+Left/Right to edit.
constexpr bool LocksCaret(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `uses_chinese_punctuation`: punctuation goes through the Chinese table. Korean and Vietnamese write half-width ASCII marks whatever the Chinese punctuation switches say.
constexpr bool UsesChinesePunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin;
}

// `host_smart_punctuation`: the reversible smart punctuation gestures (space-to-ASCII, repeat-to-Chinese) may run.
constexpr bool HostSmartPunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese;
}

// `widens_full_width`: commits and direct characters are widened when the full-width switch is on.
constexpr bool WidensFullWidth(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin;
}

// `shows_glosses`: candidates may carry translation glosses.
constexpr bool ShowsGlosses(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Korean; }

// ---- The input mode the Server tells the TIP about ----

// The scheme family the TIP has to know before its host session answers a key, carried as one character in FanyImeWorkerReplyType::InputModeChanged (shared/contracts/windows_ipc.h). The three pinyin and shape schemes share Chinese: the TIP keys them alike. The values are the wire codes and never change; a DLL that predates a mode compares against '1' and '2' only, so it reads a newer code as Chinese.
enum class InputMode : wchar_t
{
    Chinese = L'0',
    Japanese = L'1',
    Korean = L'2',
    Cantonese = L'3',
    Zhuyin = L'4',
    Vietnamese = L'5',
};

// The language a mode writes. Japanese, Korean and Vietnamese are separate input languages; Cantonese and Zhuyin are Chinese schemes, so switching to them updates `last_chinese_scheme` like any other Chinese scheme.
enum class InputLanguage
{
    Chinese,
    Japanese,
    Korean,
    Vietnamese,
};

constexpr InputLanguage input_language(InputMode mode)
{
    switch (mode)
    {
    case InputMode::Japanese:
        return InputLanguage::Japanese;
    case InputMode::Korean:
        return InputLanguage::Korean;
    case InputMode::Vietnamese:
        return InputLanguage::Vietnamese;
    case InputMode::Chinese:
    case InputMode::Cantonese:
    case InputMode::Zhuyin:
        break;
    }
    return InputLanguage::Chinese;
}

// The mode a view's scheme number runs in; an unknown number is Chinese, the mode an old DLL assumes.
constexpr InputMode input_mode(int scheme)
{
    switch (scheme)
    {
    case Japanese:
        return InputMode::Japanese;
    case Korean:
        return InputMode::Korean;
    case Cantonese:
        return InputMode::Cantonese;
    case Zhuyin:
        return InputMode::Zhuyin;
    case Vietnamese:
        return InputMode::Vietnamese;
    default:
        return InputMode::Chinese;
    }
}

// The scheme number a configured `scheme` preference names (client-core's InputScheme spelling), or -1 for a name no build knows.
constexpr int scheme_from_name(std::string_view name)
{
    if (name == "quanpin")
        return Quanpin;
    if (name == "shuangpin")
        return Shuangpin;
    if (name == "wubi")
        return Wubi;
    if (name == "japanese")
        return Japanese;
    if (name == "korean")
        return Korean;
    if (name == "cantonese")
        return Cantonese;
    if (name == "zhuyin")
        return Zhuyin;
    if (name == "vietnamese")
        return Vietnamese;
    return -1;
}

// The mode of a configured `scheme` preference. An unknown name is Chinese: host-api falls back to quanpin for it.
constexpr InputMode input_mode(std::string_view scheme_name) { return input_mode(scheme_from_name(scheme_name)); }

// Whether a configured `scheme` preference is one `last_chinese_scheme` may hold (client-core's ChineseScheme).
constexpr bool is_chinese_scheme_name(std::string_view name) { return IsChinese(scheme_from_name(name)); }

// The mode an InputModeChanged payload names. A code this build does not know is Chinese, as an older DLL reads it.
constexpr InputMode input_mode_from_code(wchar_t code)
{
    switch (code)
    {
    case static_cast<wchar_t>(InputMode::Japanese):
        return InputMode::Japanese;
    case static_cast<wchar_t>(InputMode::Korean):
        return InputMode::Korean;
    case static_cast<wchar_t>(InputMode::Cantonese):
        return InputMode::Cantonese;
    case static_cast<wchar_t>(InputMode::Zhuyin):
        return InputMode::Zhuyin;
    case static_cast<wchar_t>(InputMode::Vietnamese):
        return InputMode::Vietnamese;
    default:
        return InputMode::Chinese;
    }
}

constexpr wchar_t input_mode_code(InputMode mode) { return static_cast<wchar_t>(mode); }

// The representative scheme number of a mode, for the scheme traits above. Chinese stands for quanpin: the three schemes it covers answer every trait alike.
constexpr int mode_scheme(InputMode mode)
{
    switch (mode)
    {
    case InputMode::Japanese:
        return Japanese;
    case InputMode::Korean:
        return Korean;
    case InputMode::Cantonese:
        return Cantonese;
    case InputMode::Zhuyin:
        return Zhuyin;
    case InputMode::Vietnamese:
        return Vietnamese;
    case InputMode::Chinese:
        break;
    }
    return Quanpin;
}
} // namespace msime::windows::scheme
