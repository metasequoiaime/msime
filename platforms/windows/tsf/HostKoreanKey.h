#pragma once
#include "../common/KoreanHanjaKey.h"
#include <cstdint>

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

// The Hanja list as the deferred-key projection carries it. A key behind the deferred-key barrier is classified before the keys ahead of it have run, so the list's state is projected from the host session's forward through the queue, and a list key is classified as the list's own (KoreanKeyAction::HanjaList) only while the projection has the list open. Every other key keeps the action it has with no list, so typing that never presses the Hanja key projects exactly as it did before Hanja conversion existed.
struct KoreanHanjaProjection {
    bool listOpen = false;
    // The key ends the syllable: a Hanja is chosen with the list open, and the Hangul is committed without one.
    bool syllableEnds = false;
};

// How a key queued as the Hanja key, or as a key of a projected open list, changes the projection. The Hanja key opens a closed list and closes an open one; Escape and Backspace close it and keep the syllable; Space, Enter and a digit choose and end the syllable; the arrows, paging and Home/End only move in it. Opening assumes the syllable has Hanja, which a lone jamo does not: the keys classified on that assumption are still decided against the host session when they run, and the Server decides them against its own session the same way, so the two stay in step and only the projection is off until the queue drains.
constexpr KoreanHanjaProjection project_korean_hanja_key(uint32_t vk, wchar_t wch, bool list_open) {
    if (vk == kVirtualKeyHanja)
        return {!list_open, false};
    const auto key = msime::windows::korean_hanja_key(vk, static_cast<uint32_t>(wch));
    if (key.kind == msime::windows::KoreanHanjaKeyKind::Select ||
        (key.kind == msime::windows::KoreanHanjaKeyKind::Command && key.value == MSIME_COMMIT_CANDIDATE))
        return {false, true};
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
} // namespace msime::tsf
