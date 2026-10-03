#pragma once

#include <cstddef>
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
constexpr int Stroke = 8;

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

// Escape first shows the keys typed for the composing word again and keeps composing, and only an Escape with those keys already showing discards it (`restore_vietnamese_raw` in crates/engine/src/ime/mod.rs). The TIP and the Server each send one MSIME_CANCEL for the key, so their sessions take the same step.
constexpr bool CancelRestoresRaw(int scheme) { return scheme == Vietnamese; }


// A choice from the list fixes one reading and keeps the conversion composing in the TIP's own host session, which a row picked or a page turned by the mouse in the Server's candidate window would leave behind, so the list is driven from the keyboard only. A Korean Hanja click ends the syllable on both sides and stays clickable.
constexpr bool KeyboardOnlyCandidateList(int scheme) { return scheme == Zhuyin; }

// Stroke's five stroke keys, the Engine's STROKES (crates/engine/src/stroke/mod.rs): h 横, s 竖, p 撇, n 点, z 折. Only they start a composition; the wildcard x only extends one.
inline constexpr std::string_view kStrokeKeys = "hspnz";
inline constexpr wchar_t kStrokeWildcard = L'x';

// A letter the TIP would otherwise take into a composition that, with nothing composing, belongs to the application: under Stroke every letter but the five lowercase strokes (the wildcard x, the other letters, a capital) is one the Engine answers handled=false for, so the application inserts it. While composing the TIP takes every letter, and the Engine swallows the ones that are not strokes on both sides alike. In the Engine's own English mode (`dedicated_english`, which the Server sends as DedicatedEnglishChanged because the TIP still reports Chinese there) the Engine checks that mode before the scheme and composes every letter as English, so none passes.
constexpr bool LetterPassesWhileIdle(int scheme, bool dedicatedEnglish, wchar_t wch)
{
    if (scheme != Stroke || dedicatedEnglish)
        return false;
    const bool letter = (wch >= L'a' && wch <= L'z') || (wch >= L'A' && wch <= L'Z');
    return letter && (wch > 0x7F || kStrokeKeys.find(static_cast<char>(wch)) == std::string_view::npos);
}

// ---- Engine traits the view does not publish; each mirrors the `SchemeType` predicate of the same name ----

// `is_chinese`: a Chinese scheme, the kind `last_chinese_scheme` remembers and the Chinese mode returns to.
constexpr bool IsChinese(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Zhuyin ||
           scheme == Stroke;
}

// `script_conversion_applies`: commits are Simplified and the Traditional output switch converts them. Cantonese, Zhuyin and Stroke write their characters as stored, so converting them again would be wrong; the view's own `script_conversion` also turns off in the Unicode and temporary Japanese modes.
constexpr bool ScriptConversionApplies(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi; }

// `outputs_traditional_natively`: the scheme's dictionary is Traditional, whatever the Traditional output switch says.
constexpr bool OutputsTraditionalNatively(int scheme) { return scheme == Cantonese || scheme == Zhuyin || scheme == Stroke; }

// `commits_on_blur`: leaving the composition (focus loss, a scheme or mode switch, a navigation key handed to the application) writes it out instead of discarding it.
constexpr bool CommitsOnBlur(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `locks_caret`: the caret stays at the end of the composition, so there are no segments for Ctrl+Backspace and Ctrl+Left/Right to edit.
constexpr bool LocksCaret(int scheme) { return scheme == Korean || scheme == Zhuyin || scheme == Vietnamese; }

// `uses_chinese_punctuation`: punctuation goes through the Chinese table. Korean and Vietnamese write half-width ASCII marks whatever the Chinese punctuation switches say.
constexpr bool UsesChinesePunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin || scheme == Stroke;
}

// `host_smart_punctuation`: the reversible smart punctuation gestures (space-to-ASCII, repeat-to-Chinese) may run.
constexpr bool HostSmartPunctuation(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Cantonese || scheme == Stroke;
}

// `widens_full_width`: commits and direct characters are widened when the full-width switch is on.
constexpr bool WidensFullWidth(int scheme)
{
    return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Japanese || scheme == Cantonese ||
           scheme == Zhuyin || scheme == Stroke;
}

// `shows_glosses`: candidates may carry translation glosses.
constexpr bool ShowsGlosses(int scheme) { return scheme == Quanpin || scheme == Shuangpin || scheme == Wubi || scheme == Korean; }

// ---- The input mode the Server tells the TIP about ----

// The scheme family the TIP has to know before its host session answers a key, carried as one character in FanyImeWorkerReplyType::InputModeChanged (shared/contracts/windows_ipc.h). The three pinyin and shape schemes share Chinese: the TIP keys them alike. Stroke has a code of its own: its traits are not quanpin's (ScriptConversionApplies, ShowsGlosses) and the TIP hands most letters to the application while it is idle (LetterPassesWhileIdle). The values are the wire codes and never change; a DLL that predates a mode compares against '1' and '2' only, so it reads a newer code as Chinese.
enum class InputMode : wchar_t
{
    Chinese = L'0',
    Japanese = L'1',
    Korean = L'2',
    Cantonese = L'3',
    Zhuyin = L'4',
    Vietnamese = L'5',
    Stroke = L'6',
};

// The language a mode writes. Japanese, Korean and Vietnamese are separate input languages; Cantonese, Zhuyin and Stroke are Chinese schemes, so switching to them updates `last_chinese_scheme` like any other Chinese scheme.
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
    case InputMode::Stroke:
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
    case Stroke:
        return InputMode::Stroke;
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
    if (name == "stroke")
        return Stroke;
    return -1;
}

// The configured spelling of a scheme number, the inverse of scheme_from_name; empty for an unknown number.
constexpr std::string_view scheme_name(int scheme)
{
    switch (scheme)
    {
    case Quanpin:
        return "quanpin";
    case Shuangpin:
        return "shuangpin";
    case Wubi:
        return "wubi";
    case Japanese:
        return "japanese";
    case Korean:
        return "korean";
    case Cantonese:
        return "cantonese";
    case Zhuyin:
        return "zhuyin";
    case Vietnamese:
        return "vietnamese";
    case Stroke:
        return "stroke";
    default:
        return {};
    }
}

// Which of the Cantonese, Zhuyin and Stroke dictionaries are installed (language-dictionaries/cantonese.db, zhuyin.db and stroke.db beside the resources).
struct LanguageDictionaryPresence
{
    bool cantonese = false;
    bool zhuyin = false;
    bool stroke = false;
};

// Whether a scheme can run with the installed dictionaries: Cantonese, Zhuyin and Stroke need their own, the others read only the shared resources. An unknown scheme cannot.
constexpr bool scheme_installed(int scheme, LanguageDictionaryPresence installed)
{
    if (scheme == Cantonese)
        return installed.cantonese;
    if (scheme == Zhuyin)
        return installed.zhuyin;
    if (scheme == Stroke)
        return installed.stroke;
    return scheme >= Quanpin && scheme <= Stroke;
}

// The scheme the Engine actually runs for a configured `scheme` and `last_chinese_scheme`, as host-api's `effective_scheme` decides it: a scheme that cannot run falls back to the last Chinese scheme when that one can, and to quanpin otherwise. The TIP has to key the scheme that runs, not the one the user picked before its dictionary was installed.
constexpr int effective_scheme(std::string_view scheme_name, std::string_view last_chinese_scheme,
                               LanguageDictionaryPresence installed)
{
    const int preferred = scheme_from_name(scheme_name);
    if (scheme_installed(preferred, installed))
        return preferred;
    const int last = scheme_from_name(last_chinese_scheme);
    if (IsChinese(last) && scheme_installed(last, installed))
        return last;
    return Quanpin;
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
    case static_cast<wchar_t>(InputMode::Stroke):
        return InputMode::Stroke;
    default:
        return InputMode::Chinese;
    }
}

constexpr wchar_t input_mode_code(InputMode mode) { return static_cast<wchar_t>(mode); }

// Whether a worker frame's payload is an InputModeChanged code: one character and its terminator within `size`. Every code is accepted, a known one or not, so a mode added after this build reads as Chinese (input_mode_from_code) rather than leaving the previous mode in force.
constexpr bool is_input_mode_payload(const wchar_t *data, std::size_t size)
{
    return size >= 2 && data[0] != L'\0' && data[1] == L'\0';
}

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
    case InputMode::Stroke:
        return Stroke;
    case InputMode::Chinese:
        break;
    }
    return Quanpin;
}
} // namespace msime::windows::scheme
