#pragma once
#include <cstddef>

namespace msime::mac
{
enum class MaintenanceShortcutAction
{
    None,
    ClearCache,
    Restart,
    Terminate,
};

// ANSI 字母键的 keyCode（kVK_ANSI_A…Z）在美式布局上打出的小写字母，不是字母键时为 '\0'。
constexpr char PhysicalAnsiLetter(unsigned short keyCode)
{
    switch (keyCode)
    {
    case 0: return 'a';
    case 11: return 'b';
    case 8: return 'c';
    case 2: return 'd';
    case 14: return 'e';
    case 3: return 'f';
    case 5: return 'g';
    case 4: return 'h';
    case 34: return 'i';
    case 38: return 'j';
    case 40: return 'k';
    case 37: return 'l';
    case 46: return 'm';
    case 45: return 'n';
    case 31: return 'o';
    case 35: return 'p';
    case 12: return 'q';
    case 15: return 'r';
    case 1: return 's';
    case 17: return 't';
    case 32: return 'u';
    case 9: return 'v';
    case 13: return 'w';
    case 7: return 'x';
    case 16: return 'y';
    case 6: return 'z';
    default: return '\0';
    }
}

// 字母快捷键（Control+Shift+F、Option+Shift+H、Control+Shift+E、Control+Shift+Command+K 和下面的维护快捷键）认的是当前键盘布局在这个键上打出的字母，而不是美式布局的物理位置：Dvorak、Colemak 用户按标着 F 的键就是 Control+Shift+F，和 Windows 宿主按随布局变化的虚拟键码匹配一致，也和组字时按布局字符取字母一致。`layoutCharacter` 是 `charactersIgnoringModifiers` 的唯一字符（没有时传 0），它只保留 Shift 的作用，Control、Option 不会把它变成控制字符或 ç 这类 Option 字符，所以大小写一律折成小写。布局在这个键上放的是 ASCII 标点或数字（Dvorak 的物理 E 键打出 '.'）时它不是任何字母快捷键，不能再按物理位置认成 E，否则同一个快捷键会有两个键都能触发。只有拿不到 ASCII 字符时才退回物理位置：没有字符的合成事件，或者布局在这个键上给的是非拉丁字母。
constexpr char ShortcutLetter(unsigned short keyCode, unsigned short layoutCharacter)
{
    if (layoutCharacter >= 'A' && layoutCharacter <= 'Z') return static_cast<char>(layoutCharacter - 'A' + 'a');
    if (layoutCharacter >= 'a' && layoutCharacter <= 'z') return static_cast<char>(layoutCharacter);
    if (layoutCharacter > ' ' && layoutCharacter < 0x7f) return '\0';
    return PhysicalAnsiLetter(keyCode);
}

// InputMethodKit 只把当前输入上下文的事件交给输入法。沿用 Windows 的维护快捷键，Alt 换成 macOS 的 Option；字母按当前键盘布局认（见 `ShortcutLetter`）。
constexpr MaintenanceShortcutAction MaintenanceShortcut(char letter, bool control, bool shift, bool option, bool command)
{
    if (!control || !shift || !option || command)
        return MaintenanceShortcutAction::None;
    switch (letter)
    {
    case 'c': return MaintenanceShortcutAction::ClearCache;
    case 'r': return MaintenanceShortcutAction::Restart;
    case 't': return MaintenanceShortcutAction::Terminate;
    default: return MaintenanceShortcutAction::None;
    }
}

// Main-row ANSI digit key codes used for candidate selection.  These helpers
// live in the Engine-facing namespace so InputController.mm can include them
// alongside CandidateSkin.h, which reserves metasequoia::mac as an alias.
constexpr int PhysicalCandidateDigitSlot(unsigned short keyCode)
{
    switch (keyCode)
    {
    case 18: return 0; // 1
    case 19: return 1; // 2
    case 20: return 2; // 3
    case 21: return 3; // 4
    case 23: return 4; // 5
    case 22: return 5; // 6
    case 26: return 6; // 7
    case 28: return 7; // 8
    case 25: return 8; // 9
    case 29: return 9; // 0：第十个候选，只在每页十个时是选词键，见 CandidateDigitSlotOnPage
    // AppKit reports the physical ANSI keypad digits separately from the
    // number row. Windows normalizes VK_NUMPAD1..9 before candidate routing;
    // keep the same selection contract on macOS. 小键盘 0 与数字行 0 一样选第十个。
    case 83: return 0; // kVK_ANSI_Keypad1
    case 84: return 1; // kVK_ANSI_Keypad2
    case 85: return 2; // kVK_ANSI_Keypad3
    case 86: return 3; // kVK_ANSI_Keypad4
    case 87: return 4; // kVK_ANSI_Keypad5
    case 88: return 5; // kVK_ANSI_Keypad6
    case 89: return 6; // kVK_ANSI_Keypad7
    case 91: return 7; // kVK_ANSI_Keypad8
    case 92: return 8; // kVK_ANSI_Keypad9
    case 82: return 9; // kVK_ANSI_Keypad0
    default: return -1;
    }
}

// 0 键对应的第十格只在每页排得下十个候选时才是选词键；每页不到十个时它照旧是普通按键（交给 Engine 或应用），和放宽上限之前一样。1–9 不受影响。
constexpr int CandidateDigitSlotOnPage(int slot, size_t pageSize)
{
    return slot == 9 && pageSize < 10 ? -1 : slot;
}

// Candidate digits are a controller shortcut only for an ordinary candidate panel. A digit the Engine lists in the view's spelling_symbols is input - the code point being typed in Unicode mode, the number in expression mode - while nine-key mode and modified chords belong to the Engine.
constexpr bool ShouldRoutePhysicalCandidateDigit(bool candidatePanelVisible, bool nineKeyMode, bool digitIsSpelling,
                                                   bool modified)
{
    return candidatePanelVisible && !nineKeyMode && !digitIsSpelling && !modified;
}

// A mode that spells with digits still has to let the user reach the second candidate.
//
// Its digits are what is being typed, so the reference moves selection onto Shift+digit, which cannot be part of a code point: `Shift + 数字 选其他候选` in the Unicode mode's own documentation. Without it the only candidate a keyboard can commit there is the first one. Expression mode also spells some shifted digits - ( ) % ^ * on a US layout - and a Shift+digit whose character the mode takes stays input; only the others select.
constexpr bool ShouldRouteSpellingShiftCandidateDigit(bool candidatePanelVisible, bool digitIsSpelling, bool shiftOnly,
                                                        bool shiftedIsSpelling)
{
    return candidatePanelVisible && digitIsSpelling && shiftOnly && !shiftedIsSpelling;
}

// The digit a candidate slot's physical key types, so it can be looked up in spelling_symbols whatever the keyboard layout puts on that key.
constexpr char PhysicalCandidateDigitCharacter(int slot)
{
    return slot >= 0 && slot <= 8 ? static_cast<char>('1' + slot) : slot == 9 ? '0' : '\0';
}

// The key class msime_client_key_sound takes: 1 space, 2 enter (main or keypad), 3 backspace, 0 any other key.
constexpr unsigned PhysicalKeySoundClass(unsigned short keyCode)
{
    switch (keyCode)
    {
    case 49: return 1; // kVK_Space
    case 36: // kVK_Return
    case 76: // kVK_ANSI_KeypadEnter
        return 2;
    case 51: return 3; // kVK_Delete
    default: return 0;
    }
}

constexpr bool IsKeypadDecimal(unsigned short keyCode)
{
    return keyCode == 65; // kVK_ANSI_KeypadDecimal
}

// Physical macOS keypad punctuation.  Keep this independent of the active
// keyboard layout so keypad operators cannot fall into the main-row paging
// shortcuts (notably '-' and '=').
constexpr char KeypadPunctuation(unsigned short keyCode)
{
    switch (keyCode)
    {
    case 65: return '.'; // kVK_ANSI_KeypadDecimal
    case 67: return '*'; // kVK_ANSI_KeypadMultiply
    case 69: return '+'; // kVK_ANSI_KeypadPlus
    case 75: return '/'; // kVK_ANSI_KeypadDivide
    case 78: return '-'; // kVK_ANSI_KeypadMinus
    case 81: return '='; // kVK_ANSI_KeypadEquals
    case 95: return ','; // kVK_JIS_KeypadComma / keypad separator
    default: return '\0';
    }
}

constexpr bool IsJapaneseMinusEqualInput(int scheme, bool temporaryJapanese, char character)
{
    return (scheme == 3 || temporaryJapanese) && (character == '-' || character == '=');
}

// Candidate paging is configured by characters, but Japanese input owns the
// physical ANSI minus/equal keys.  AppKit's charactersIgnoringModifiers can
// change with the active keyboard layout, so retain the character fallback for
// synthetic/older events while preferring the physical key codes in real input.
constexpr bool IsJapaneseMinusEqualKey(int scheme, bool temporaryJapanese, unsigned short keyCode, char character)
{
    if (scheme != 3 && !temporaryJapanese) return false;
    // Real AppKit events must identify the physical ANSI key.  Keep the
    // keyCode==0 character fallback for older synthetic tests/events only;
    // accepting a punctuation character from another physical key would
    // diverge from the Windows virtual-key contract.
    return keyCode == 24 || keyCode == 27 ||
           (keyCode == 0 && (character == '-' || character == '='));
}

// Candidate paging follows Windows' physical virtual-key policy.  AppKit's
// charactersIgnoringModifiers varies with the active keyboard layout, so the
// key code—not the produced glyph—selects the binding.  A zero result means
// that the key is not one of the paging keys; -1 is previous and +1 is next.
constexpr int PhysicalCandidatePageDirection(unsigned short keyCode)
{
    switch (keyCode)
    {
    case 27: // ANSI '-'
    case 43: // ANSI ','
    case 33: // ANSI '['
    case 116: // Page Up
        return -1;
    case 24: // ANSI '='
    case 47: // ANSI '.'
    case 30: // ANSI ']'
    case 121: // Page Down
        return 1;
    default:
        return 0;
    }
}

// Word-to-character is stricter than paging: both the physical key and its
// expected unshifted punctuation must match.  This prevents a keyboard layout
// from making an unrelated physical key with the same glyph select an edge.
constexpr bool IsPhysicalWordCharacterKey(unsigned short keyCode, bool brackets, char character)
{
    if (brackets)
        return (keyCode == 33 && character == '[') || (keyCode == 30 && character == ']');
    return (keyCode == 27 && character == '-') || (keyCode == 24 && character == '=');
}

// The Engine's `SchemeType::Korean`, as it appears in a view's `scheme`.
constexpr int KoreanScheme = 4;

// Dubeolsik binds jamo to letters by case: Shift+Q/W/E/R/T/O/P type ㅃ ㅉ ㄸ ㄲ ㅆ ㅒ ㅖ, and every other letter types the same jamo either way. So the case the Engine receives is Shift's alone. AppKit folds Caps Lock into the typed character, and passing that on would turn r (ㄱ) into R (ㄲ) for a user who only left Caps Lock on. Anything that is not an ASCII letter comes back unchanged.
constexpr char KoreanKeyLetter(char character, bool shift)
{
    if (character >= 'A' && character <= 'Z') character = static_cast<char>(character - 'A' + 'a');
    else if (character < 'a' || character > 'z') return character;
    return shift ? static_cast<char>(character - 'a' + 'A') : character;
}

// The Engine's `CandidateSource::Fallback`, as it appears in a view candidate's `source`.
constexpr int CandidateSourceFallback = 9;

// A lone Fallback row is the raw composition the Engine shows when there is nothing to convert (a bare Shift+R prefix, or romaji it cannot read). Windows commits it on the first Space, so Japanese Space must not arm a conversion on it.
constexpr bool JapaneseSpaceCommitsFallback(unsigned long candidateCount, int firstCandidateSource)
{
    return candidateCount == 1 && firstCandidateSource == CandidateSourceFallback;
}
} // namespace msime::mac
