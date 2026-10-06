#pragma once
#include "../common/KoreanHanjaKey.h"
#include <cstdint>
#include <string_view>

namespace msime::tsf {
// What a key does while the Korean scheme is active and the keyboard is open.
//
// Korean is a syllable automaton (see the MsimeCommand comment in msime_client.h): letters compose, and every other key ends the open syllable and then does its ordinary work. The TIP inserts that syllable itself, so no key here depends on the Chinese punctuation table or on full-width conversion. Its only candidates are the Hanja of the composing syllable, listed after the Hanja key; while that list is open the keys in KoreanHanjaKey.h choose from it, and every other key keeps the meaning it has without a list.
enum class KoreanKeyAction {
    // Not Korean-specific: the ordinary classification decides. Backspace and Escape while a syllable is open take this path, as do keys that produce no character.
    Default,
    // A letter is a jamo. The TIP eats it and sends the letter in the case Shift gives it.
    Compose,
    // A printable non-letter (space, digit, punctuation) while a syllable is open. The TIP eats it, commits the syllable and inserts the key's ASCII character right after it, so the character can never land ahead of the syllable it ended.
    CommitWithText,
    // A caret or editing key while a syllable is open. The syllable is committed and the key goes on to the application, which inserts the newline or tab, moves the caret or deletes.
    CommitAndPass,
    // The application gets the key untouched: a printable non-letter, or the Hanja key, with nothing composing, so punctuation stays half-width ASCII.
    Pass,
    // The Hanja key while a syllable is open: the TIP eats it and sends MSIME_CONVERT_HANJA, which lists the syllable's Hanja or closes an open list. A syllable with no Hanja (a lone jamo) keeps composing and the key is still eaten, so it never reaches the application in the middle of a syllable.
    ConvertHanja,
    // A key the open Hanja list takes (KoreanHanjaKey.h): a digit, Space, Enter, an arrow, Page Up/Down, Home/End, Escape or Backspace. The TIP eats it and applies it to the list.
    HanjaList,
};

inline constexpr uint32_t kVirtualKeyHangul = 0x15;
inline constexpr uint32_t kVirtualKeyHanja = msime::windows::kVirtualKeyHanja;

constexpr bool is_korean_letter_key(uint32_t vk, wchar_t wch) {
    return vk >= 'A' && vk <= 'Z' && ((wch >= L'a' && wch <= L'z') || (wch >= L'A' && wch <= L'Z'));
}

// Printable ASCII other than a letter: the characters a Korean syllable is followed by in the document.
constexpr bool is_korean_text_key(wchar_t wch) {
    return wch >= 0x20 && wch <= 0x7E && !((wch >= L'a' && wch <= L'z') || (wch >= L'A' && wch <= L'Z'));
}

constexpr bool is_korean_caret_or_edit_key(uint32_t vk) {
    switch (vk) {
    case 0x09: // Tab
    case 0x0D: // Enter
    case 0x21: // Page Up
    case 0x22: // Page Down
    case 0x23: // End
    case 0x24: // Home
    case 0x25: // Left
    case 0x26: // Up
    case 0x27: // Right
    case 0x28: // Down
    case 0x2D: // Insert
    case 0x2E: // Delete
        return true;
    default:
        return false;
    }
}

// A key the open Hanja list takes, other than the Hanja key itself.
constexpr bool is_korean_hanja_list_key(uint32_t vk, wchar_t wch) {
    return msime::windows::is_korean_hanja_list_key(vk, static_cast<uint32_t>(wch));
}

// Modifier chords are resolved before this: Ctrl, Alt and Windows combinations belong to the application. `hanja_list_open` is whether the composing syllable's Hanja list is open; with it false every key behaves as it did before Hanja conversion existed.
constexpr KoreanKeyAction korean_key_action(uint32_t vk, wchar_t wch, bool composing, bool hanja_list_open = false) {
    if (is_korean_letter_key(vk, wch))
        return KoreanKeyAction::Compose;
    if (vk == kVirtualKeyHanja)
        return composing ? KoreanKeyAction::ConvertHanja : KoreanKeyAction::Pass;
    if (!composing)
        return is_korean_text_key(wch) ? KoreanKeyAction::Pass : KoreanKeyAction::Default;
    if (hanja_list_open && is_korean_hanja_list_key(vk, wch))
        return KoreanKeyAction::HanjaList;
    if (vk == 0x08 || vk == 0x1B)
        return KoreanKeyAction::Default;
    // Numpad keys carry their character too, so they are text; Enter and Tab carry control characters and are not.
    if (is_korean_text_key(wch))
        return KoreanKeyAction::CommitWithText;
    if (is_korean_caret_or_edit_key(vk))
        return KoreanKeyAction::CommitAndPass;
    return KoreanKeyAction::Default;
}

// The Hanja list as the deferred-key projection carries it. A key behind the deferred-key barrier is classified before the keys ahead of it have run, so the list's state is projected from the host session's forward through the queue, and a list key is classified as the list's own (KoreanKeyAction::HanjaList) only while the projection has the list open. Every other key keeps the action it has with no list, so typing that never presses the Hanja key projects exactly as it did before Hanja conversion existed. The Zhuyin list is projected the same way.
struct KoreanHanjaProjection {
    bool listOpen = false;
    // The key ends the syllable: a Hanja is chosen with the list open, and the Hangul is committed without one.
    bool syllableEnds = false;
};

// How a key queued as the list's opening key (the Hanja key, Zhuyin's Down on a closed list), or as a key of a projected open list, changes the projection. The Hanja key opens a closed list and closes an open one; Escape and Backspace close it and keep the composition; Space, Enter and a digit choose, which ends a Korean syllable and only fixes that reading of a Zhuyin conversion, which keeps composing; the arrows, paging and Home/End only move in it. Opening assumes the composition has candidates, which a lone jamo or a Zhuyin initial does not: the keys classified on that assumption are still decided against the host session when they run, and the Server decides them against its own session the same way, so the two stay in step and only the projection is off until the queue drains.
constexpr KoreanHanjaProjection project_korean_hanja_key(int scheme, uint32_t vk, wchar_t wch, bool list_open) {
    if (vk == kVirtualKeyHanja && scheme == msime::windows::scheme::Korean)
        return {!list_open, false};
    if (msime::windows::opens_candidate_list(scheme, vk, list_open))
        return {true, false};
    const auto key = msime::windows::korean_hanja_key(vk, static_cast<uint32_t>(wch));
    if (key.kind == msime::windows::KoreanHanjaKeyKind::Select ||
        (key.kind == msime::windows::KoreanHanjaKeyKind::Command && key.value == MSIME_COMMIT_CANDIDATE))
        return {false, scheme == msime::windows::scheme::Korean};
    if (key.kind == msime::windows::KoreanHanjaKeyKind::Command &&
        (key.value == MSIME_CANCEL || key.value == MSIME_BACKSPACE))
        return {false, false};
    return {list_open, false};
}

// The letter the Engine receives: Shift decides the case, and with it the tense consonants and ㅒ ㅖ, whatever Caps Lock says.
constexpr wchar_t korean_letter(wchar_t wch, bool shift) {
    const wchar_t lower = (wch >= L'A' && wch <= L'Z') ? static_cast<wchar_t>(wch - L'A' + L'a') : wch;
    return shift && lower >= L'a' && lower <= L'z' ? static_cast<wchar_t>(lower - L'a' + L'A') : lower;
}

// Whether a key's character is one the composition spells with rather than one that ends it: `spelling_symbols` is the view's (Zhuyin's digit and punctuation keys, VNI's tone digits).
constexpr bool spelled_key(std::string_view spelling_symbols, wchar_t wch) {
    return wch > 0 && wch <= 0x7E && spelling_symbols.find(static_cast<char>(wch)) != std::string_view::npos;
}

// The Zhuyin keys that start a syllable from idle, the Engine's IDLE_SYMBOLS (crates/engine/src/zhuyin/layout.rs): every Dachen phonetic key that is not a letter. Tone keys (3, 4, 6, 7) and Space need a syllable to act on. The deferred-key classifier reads this because a queued key cannot see the host view; scripts/test-scheme-traits-parity.py checks it against the Engine.
inline constexpr std::string_view kZhuyinIdleSymbols = "125890,./;-";
// The keys a composing Zhuyin conversion spells with while its list is closed, the Engine's DACHEN_SYMBOLS, and while it is open, its LIST_OPEN_SYMBOLS (crates/engine/src/zhuyin/scheme.rs): the digits 1-9 and Space are then the list's.
inline constexpr std::string_view kZhuyinComposingSymbols = "1234567890,./;- ";
inline constexpr std::string_view kZhuyinListOpenSymbols = "0,./;-";
// VNI's tone and mark digits, the Engine's VNI_DIGITS (crates/engine/src/vietnamese/scheme.rs). The TIP cannot tell VNI from Telex before a key runs, so a queued composing digit is taken as spelled and the live view decides when it runs.
inline constexpr std::string_view kVietnameseVniDigits = "0123456789";
// 藏文（威利转写）在空闲和组字时拼写用的非字母键，即引擎的 SPELLING_SYMBOLS_IDLE 和 SPELLING_SYMBOLS_COMPOSING（crates/engine/src/tibetan/scheme.rs）：`'` 随时可以开始 achung 音节，`+` `-` `.` 只在组字时属于拼写；`/` 从不进入原文，引擎把它变成垂符 U+0F0D 上屏，空闲时单独输出垂符。排队的按键看不到视图，所以延迟分类读这两组常量，按键执行时再由实时视图决定。
inline constexpr std::string_view kTibetanIdleSymbols = "'/";
inline constexpr std::string_view kTibetanComposingSymbols = "'+-./";

// 组字是否接收这个键，而不是被它结束：视图的 `spelling_symbols` 里列出的键（spelled_key），以及藏文组字时的空格。藏文的空格把转换出的藏文连同音节点 U+0F0B 一起上屏，所以它要送进宿主会话，而不是像越南文那样结束组字后原样插入空格。
constexpr bool host_composition_takes_key(int scheme, std::string_view spelling_symbols, wchar_t wch, bool composing) {
    if (scheme == msime::windows::scheme::Tibetan && composing && wch == L' ')
        return true;
    return spelled_key(spelling_symbols, wch);
}

// What a key does while a scheme that composes in the TIP's own host session (scheme::AlwaysInlinePreedit) is active and the keyboard is open, in KoreanKeyAction's terms. Korean keeps korean_key_action. `spelling_symbols` is what the view publishes for the current state (spelled_key).
//
// Vietnamese is a word automaton like Korean's syllable: letters compose with their case, a VNI tone digit composes, and every other printable key ends the word and follows it half-width; with nothing composing such a key is the application's.
//
// 藏文按同样的方式组字，区别在于：字母连同大小写组字（威利转写区分大小写）；视图列出的拼写符号随时组字，所以空闲的 `'` 开始一个音节，空闲的 `/` 也送进宿主会话由引擎输出垂符；组字时的空格送进宿主会话，由引擎带音节点上屏；回车只上屏藏文并吃掉按键，和注音一样（CommitWithText，回车没有可跟在后面的字符）。其他可打印键结束组字并以半角跟在后面，光标和编辑键上屏后交给应用。
//
// Zhuyin (Dachen) spells with lowercase letters and the keys `spelling_symbols` names, from idle too. A letter typed with Shift or Caps Lock is not phonetic: it ends the conversion and follows it, as libchewing writes it. Down on a closed list opens the list (opens_candidate_list), whose keys then follow KoreanHanjaKey.h; Enter commits the conversion and is spent; any other printable key ends the conversion through the Chinese punctuation table (CommitWithText). Nothing composing, every key that is not spelled keeps the Chinese rules (Default), so Shift punctuation is the Chinese mark as in the other Chinese schemes.
constexpr KoreanKeyAction host_composed_key_action(int scheme, uint32_t vk, wchar_t wch, bool composing, bool list_open,
                                                   std::string_view spelling_symbols) {
    if (scheme == msime::windows::scheme::Korean)
        return korean_key_action(vk, wch, composing, list_open);
    const bool letter = is_korean_letter_key(vk, wch);
    const bool spelled = !letter && spelled_key(spelling_symbols, wch);
    if (scheme == msime::windows::scheme::Tibetan) {
        if (letter || host_composition_takes_key(scheme, spelling_symbols, wch, composing))
            return KoreanKeyAction::Compose;
        if (!composing)
            return is_korean_text_key(wch) ? KoreanKeyAction::Pass : KoreanKeyAction::Default;
        if (vk == 0x0D)
            return KoreanKeyAction::CommitWithText;
        if (vk == 0x08 || vk == 0x1B)
            return KoreanKeyAction::Default;
        if (is_korean_text_key(wch))
            return KoreanKeyAction::CommitWithText;
        if (is_korean_caret_or_edit_key(vk))
            return KoreanKeyAction::CommitAndPass;
        return KoreanKeyAction::Default;
    }
    if (scheme == msime::windows::scheme::Vietnamese) {
        if (letter || (composing && spelled))
            return KoreanKeyAction::Compose;
        if (!composing)
            return is_korean_text_key(wch) ? KoreanKeyAction::Pass : KoreanKeyAction::Default;
        if (vk == 0x08 || vk == 0x1B)
            return KoreanKeyAction::Default;
        if (is_korean_text_key(wch))
            return KoreanKeyAction::CommitWithText;
        if (is_korean_caret_or_edit_key(vk))
            return KoreanKeyAction::CommitAndPass;
        return KoreanKeyAction::Default;
    }
    if (scheme != msime::windows::scheme::Zhuyin)
        return KoreanKeyAction::Default;
    if (composing && list_open && is_korean_hanja_list_key(vk, wch))
        return KoreanKeyAction::HanjaList;
    if (letter) {
        if (wch >= L'a' && wch <= L'z')
            return KoreanKeyAction::Compose;
        return composing ? KoreanKeyAction::CommitWithText : KoreanKeyAction::Pass;
    }
    if (spelled)
        return KoreanKeyAction::Compose;
    if (!composing)
        return KoreanKeyAction::Default;
    if (msime::windows::opens_candidate_list(scheme, vk, list_open))
        return KoreanKeyAction::ConvertHanja;
    if (vk == 0x0D)
        return KoreanKeyAction::CommitWithText;
    if (vk == 0x08 || vk == 0x1B)
        return KoreanKeyAction::Default;
    if (is_korean_text_key(wch))
        return KoreanKeyAction::CommitWithText;
    if (is_korean_caret_or_edit_key(vk))
        return KoreanKeyAction::CommitAndPass;
    return KoreanKeyAction::Default;
}
} // namespace msime::tsf
