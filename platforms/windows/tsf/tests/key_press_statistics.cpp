#include "KeyPressStatistics.h"

#include <cstdint>
#include <cstdio>
#include <set>
#include <string>

namespace
{
int failures = 0;

void Require(bool value, const char *what)
{
    if (!value)
    {
        std::fprintf(stderr, "key press statistics: %s\n", what);
        ++failures;
    }
}

bool Is(const wchar_t *actual, const wchar_t *expected)
{
    return actual != nullptr && std::wstring(actual) == expected;
}

// 物理键盘能产生的 id，抄自 crates/client-services/src/typing_statistics.rs 中的 KEY_IDS。存储只要遇到一个不在该列表里的 id 就会拒绝整批，所以这张表发出的每个 id 都必须在列表里。
const std::set<std::wstring> &PhysicalKeyIds()
{
    static const std::set<std::wstring> ids = {
        L"KeyA", L"KeyB", L"KeyC", L"KeyD", L"KeyE", L"KeyF", L"KeyG", L"KeyH", L"KeyI", L"KeyJ", L"KeyK", L"KeyL",
        L"KeyM", L"KeyN", L"KeyO", L"KeyP", L"KeyQ", L"KeyR", L"KeyS", L"KeyT", L"KeyU", L"KeyV", L"KeyW", L"KeyX",
        L"KeyY", L"KeyZ", L"Digit0", L"Digit1", L"Digit2", L"Digit3", L"Digit4", L"Digit5", L"Digit6", L"Digit7",
        L"Digit8", L"Digit9", L"Backquote", L"Minus", L"Equal", L"BracketLeft", L"BracketRight", L"Backslash",
        L"Semicolon", L"Quote", L"Comma", L"Period", L"Slash", L"IntlBackslash", L"IntlRo", L"IntlYen", L"Lang1",
        L"Lang2", L"Convert", L"NonConvert", L"KanaMode", L"Space", L"Enter", L"Backspace", L"Tab", L"Escape",
        L"Delete", L"Insert", L"Home", L"End", L"PageUp", L"PageDown", L"ArrowUp", L"ArrowDown", L"ArrowLeft",
        L"ArrowRight", L"CapsLock", L"ShiftLeft", L"ShiftRight", L"ControlLeft", L"ControlRight", L"AltLeft",
        L"AltRight", L"MetaLeft", L"MetaRight", L"Fn", L"ContextMenu", L"F1", L"F2", L"F3", L"F4", L"F5", L"F6",
        L"F7", L"F8", L"F9", L"F10", L"F11", L"F12", L"Numpad0", L"Numpad1", L"Numpad2", L"Numpad3", L"Numpad4",
        L"Numpad5", L"Numpad6", L"Numpad7", L"Numpad8", L"Numpad9", L"NumpadDecimal", L"NumpadEnter", L"NumpadAdd",
        L"NumpadSubtract", L"NumpadMultiply", L"NumpadDivide", L"NumLock",
    };
    return ids;
}

// A WM_KEYDOWN lParam for one fresh press: repeat count 1, the scan code, and the extended bit.
std::uintptr_t KeyDown(unsigned scanCode, bool extended)
{
    return 1u | (static_cast<std::uintptr_t>(scanCode) << 16) | (extended ? 0x01000000u : 0u);
}
} // namespace

int main()
{
    // Physical positions, not layout characters: the ANSI letter rows in scan-code order.
    Require(Is(KeyIdFromScanCode(0x10, false), L"KeyQ"), "0x10 is the Q position");
    Require(Is(KeyIdFromScanCode(0x1E, false), L"KeyA"), "0x1E is the A position");
    Require(Is(KeyIdFromScanCode(0x2C, false), L"KeyZ"), "0x2C is the Z position");
    Require(Is(KeyIdFromScanCode(0x32, false), L"KeyM"), "0x32 is the M position");
    Require(Is(KeyIdFromScanCode(0x02, false), L"Digit1"), "0x02 is Digit1");
    Require(Is(KeyIdFromScanCode(0x0B, false), L"Digit0"), "0x0B is Digit0");
    Require(Is(KeyIdFromScanCode(0x29, false), L"Backquote"), "0x29 is Backquote");
    Require(Is(KeyIdFromScanCode(0x2B, false), L"Backslash"), "0x2B is Backslash");
    Require(Is(KeyIdFromScanCode(0x56, false), L"IntlBackslash"), "0x56 is the ISO key beside left Shift");
    Require(Is(KeyIdFromScanCode(0x73, false), L"IntlRo"), "0x73 is the JIS Ro key");
    Require(Is(KeyIdFromScanCode(0x7D, false), L"IntlYen"), "0x7D is the JIS Yen key");
    Require(Is(KeyIdFromScanCode(0x39, false), L"Space"), "0x39 is Space");
    Require(Is(KeyIdFromScanCode(0x0E, false), L"Backspace"), "0x0E is Backspace");

    // The extended bit separates the left and right modifiers and the keypad from the editing block.
    Require(Is(KeyIdFromScanCode(0x1D, false), L"ControlLeft"), "plain 0x1D is left Control");
    Require(Is(KeyIdFromScanCode(0x1D, true), L"ControlRight"), "extended 0x1D is right Control");
    Require(Is(KeyIdFromScanCode(0x38, false), L"AltLeft"), "plain 0x38 is left Alt");
    Require(Is(KeyIdFromScanCode(0x38, true), L"AltRight"), "extended 0x38 is right Alt");
    Require(Is(KeyIdFromScanCode(0x2A, false), L"ShiftLeft"), "0x2A is left Shift");
    Require(Is(KeyIdFromScanCode(0x36, false), L"ShiftRight"), "0x36 is right Shift");
    Require(Is(KeyIdFromScanCode(0x1C, false), L"Enter"), "plain 0x1C is Enter");
    Require(Is(KeyIdFromScanCode(0x1C, true), L"NumpadEnter"), "extended 0x1C is the keypad Enter");
    Require(Is(KeyIdFromScanCode(0x35, false), L"Slash"), "plain 0x35 is Slash");
    Require(Is(KeyIdFromScanCode(0x35, true), L"NumpadDivide"), "extended 0x35 is the keypad divide");
    Require(Is(KeyIdFromScanCode(0x48, false), L"Numpad8"), "plain 0x48 is keypad 8 even with NumLock off");
    Require(Is(KeyIdFromScanCode(0x48, true), L"ArrowUp"), "extended 0x48 is the arrow key");
    Require(Is(KeyIdFromScanCode(0x53, true), L"Delete"), "extended 0x53 is Delete");
    Require(Is(KeyIdFromScanCode(0x5B, true), L"MetaLeft"), "extended 0x5B is the left Windows key");
    Require(Is(KeyIdFromScanCode(0x45, true), L"NumLock"), "extended 0x45 is NumLock");
    Require(KeyIdFromScanCode(0x45, false) == nullptr, "plain 0x45 is Pause, which has no canonical id");
    Require(KeyIdFromScanCode(0x46, false) == nullptr, "ScrollLock has no canonical id");
    Require(KeyIdFromScanCode(0x37, true) == nullptr, "PrintScreen has no canonical id");
    Require(KeyIdFromScanCode(0x2A, true) == nullptr, "the fake Shift around NumLock-off arrows is not a press");
    Require(KeyIdFromScanCode(0x00, false) == nullptr, "a synthetic key without a scan code is not counted");

    // Every id the table can produce is canonical, and no two positions share one except the Korean keys' two encodings.
    std::set<std::wstring> seen;
    unsigned mapped = 0;
    for (unsigned extended = 0; extended < 2; ++extended)
    {
        for (unsigned scanCode = 0; scanCode < 0x100; ++scanCode)
        {
            const wchar_t *id = KeyIdFromScanCode(scanCode, extended != 0);
            if (id == nullptr)
            {
                continue;
            }
            ++mapped;
            Require(PhysicalKeyIds().count(id) == 1, "every mapped id is in the shared whitelist");
            seen.insert(id);
        }
    }
    Require(mapped == seen.size() + 6, "only Lang1 and Lang2 have more than one scan code");
    // Fn never reaches Windows as a key; every other physical id has a scan code.
    Require(seen.size() + 1 == PhysicalKeyIds().size(), "every physical id but Fn is reachable");
    Require(seen.count(L"Fn") == 0, "Fn is handled by the keyboard firmware");

    // Fresh presses count; auto-repeat and injected characters do not.
    Require(Is(KeyPressIdFromKeyDown(0x41, KeyDown(0x1E, false)), L"KeyA"), "a fresh press counts by scan code");
    Require(Is(KeyPressIdFromKeyDown(0x51, KeyDown(0x1E, false)), L"KeyA"),
            "the VK does not matter: AZERTY's Q on the A position is still KeyA");
    Require(KeyPressIdFromKeyDown(0x08, KeyDown(0x0E, false) | 0x40000000u) == nullptr,
            "a held Backspace counts once, not per repeat");
    Require(KeyPressIdFromKeyDown(0x10, KeyDown(0x2A, false) | 0x40000000u) == nullptr,
            "a held Shift counts once, not per repeat");
    Require(KeyPressIdFromKeyDown(0xE7, KeyDown(0x1E, false)) == nullptr, "VK_PACKET carries a character, not a key");
    Require(Is(KeyPressIdFromKeyDown(0x25, KeyDown(0x4B, true)), L"ArrowLeft"), "the extended bit reaches the table");
    Require(KeyPressPhysicalKey(KeyDown(0x1D, true)) != KeyPressPhysicalKey(KeyDown(0x1D, false)),
            "the de-duplication tells left and right Control apart");
    Require(KeyPressPhysicalKey(KeyDown(0x1E, false) | 0x40000000u) == KeyPressPhysicalKey(KeyDown(0x1E, false)),
            "the de-duplication ignores the repeat flags");

    // Private contexts and the secure desktop are never counted.
    Require(ShouldCountKeyPress(L"KeyA", false, false), "an ordinary press counts");
    Require(!ShouldCountKeyPress(L"KeyA", true, false), "a disabled or empty context does not count");
    Require(!ShouldCountKeyPress(L"KeyA", false, true), "the secure desktop does not count");
    Require(!ShouldCountKeyPress(nullptr, false, false), "an unmapped key does not count");

    if (failures == 0)
    {
        std::puts("key press statistics: scan-code table passed");
    }
    return failures == 0 ? 0 : 1;
}
