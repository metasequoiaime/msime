#pragma once

#include <cstdint>

// 按键热力图统计的是物理键，所以一次按键用它的扫描码识别（`WM_KEYDOWN` 的 `lParam` 第 16-23 位，第 24 位标记带 E0 前缀的扩展键），从不用 VK：VK 跟随当前布局，会把 AZERTY 的 "A" 记到 Q 键上。键名采用共享存储的规范 id（W3C `KeyboardEvent.code`，即 `crates/client-services/src/typing_statistics.rs` 中的 `KEY_IDS`）；只要有一个未知 id，存储就会拒绝整批数据，所以这里没有条目的扫描码直接不计数。本文件不引用 Windows 头文件，以便单独测试。
inline const wchar_t *KeyIdFromScanCode(unsigned scanCode, bool extended)
{
    if (extended)
    {
        switch (scanCode)
        {
        case 0x1C: return L"NumpadEnter";
        case 0x1D: return L"ControlRight";
        case 0x35: return L"NumpadDivide";
        case 0x38: return L"AltRight";
        // NumLock is the extended 0x45; the plain 0x45 is Pause, which has no canonical id.
        case 0x45: return L"NumLock";
        case 0x47: return L"Home";
        case 0x48: return L"ArrowUp";
        case 0x49: return L"PageUp";
        case 0x4B: return L"ArrowLeft";
        case 0x4D: return L"ArrowRight";
        case 0x4F: return L"End";
        case 0x50: return L"ArrowDown";
        case 0x51: return L"PageDown";
        case 0x52: return L"Insert";
        case 0x53: return L"Delete";
        case 0x5B: return L"MetaLeft";
        case 0x5C: return L"MetaRight";
        case 0x5D: return L"ContextMenu";
        // Korean keyboards report Hangul and Hanja with or without the prefix depending on the driver.
        case 0x71: case 0xF1: return L"Lang2";
        case 0x72: case 0xF2: return L"Lang1";
        default: return nullptr;
        }
    }
    switch (scanCode)
    {
    case 0x01: return L"Escape";
    case 0x02: return L"Digit1";
    case 0x03: return L"Digit2";
    case 0x04: return L"Digit3";
    case 0x05: return L"Digit4";
    case 0x06: return L"Digit5";
    case 0x07: return L"Digit6";
    case 0x08: return L"Digit7";
    case 0x09: return L"Digit8";
    case 0x0A: return L"Digit9";
    case 0x0B: return L"Digit0";
    case 0x0C: return L"Minus";
    case 0x0D: return L"Equal";
    case 0x0E: return L"Backspace";
    case 0x0F: return L"Tab";
    case 0x10: return L"KeyQ";
    case 0x11: return L"KeyW";
    case 0x12: return L"KeyE";
    case 0x13: return L"KeyR";
    case 0x14: return L"KeyT";
    case 0x15: return L"KeyY";
    case 0x16: return L"KeyU";
    case 0x17: return L"KeyI";
    case 0x18: return L"KeyO";
    case 0x19: return L"KeyP";
    case 0x1A: return L"BracketLeft";
    case 0x1B: return L"BracketRight";
    case 0x1C: return L"Enter";
    case 0x1D: return L"ControlLeft";
    case 0x1E: return L"KeyA";
    case 0x1F: return L"KeyS";
    case 0x20: return L"KeyD";
    case 0x21: return L"KeyF";
    case 0x22: return L"KeyG";
    case 0x23: return L"KeyH";
    case 0x24: return L"KeyJ";
    case 0x25: return L"KeyK";
    case 0x26: return L"KeyL";
    case 0x27: return L"Semicolon";
    case 0x28: return L"Quote";
    case 0x29: return L"Backquote";
    case 0x2A: return L"ShiftLeft";
    case 0x2B: return L"Backslash";
    case 0x2C: return L"KeyZ";
    case 0x2D: return L"KeyX";
    case 0x2E: return L"KeyC";
    case 0x2F: return L"KeyV";
    case 0x30: return L"KeyB";
    case 0x31: return L"KeyN";
    case 0x32: return L"KeyM";
    case 0x33: return L"Comma";
    case 0x34: return L"Period";
    case 0x35: return L"Slash";
    case 0x36: return L"ShiftRight";
    case 0x37: return L"NumpadMultiply";
    case 0x38: return L"AltLeft";
    case 0x39: return L"Space";
    case 0x3A: return L"CapsLock";
    case 0x3B: return L"F1";
    case 0x3C: return L"F2";
    case 0x3D: return L"F3";
    case 0x3E: return L"F4";
    case 0x3F: return L"F5";
    case 0x40: return L"F6";
    case 0x41: return L"F7";
    case 0x42: return L"F8";
    case 0x43: return L"F9";
    case 0x44: return L"F10";
    // With NumLock off these keys arrive as Home, ArrowUp and so on, but the scan code still names the keypad key the user pressed.
    case 0x47: return L"Numpad7";
    case 0x48: return L"Numpad8";
    case 0x49: return L"Numpad9";
    case 0x4A: return L"NumpadSubtract";
    case 0x4B: return L"Numpad4";
    case 0x4C: return L"Numpad5";
    case 0x4D: return L"Numpad6";
    case 0x4E: return L"NumpadAdd";
    case 0x4F: return L"Numpad1";
    case 0x50: return L"Numpad2";
    case 0x51: return L"Numpad3";
    case 0x52: return L"Numpad0";
    case 0x53: return L"NumpadDecimal";
    case 0x56: return L"IntlBackslash";
    case 0x57: return L"F11";
    case 0x58: return L"F12";
    case 0x70: return L"KanaMode";
    case 0x71: case 0xF1: return L"Lang2";
    case 0x72: case 0xF2: return L"Lang1";
    case 0x73: return L"IntlRo";
    case 0x79: return L"Convert";
    case 0x7B: return L"NonConvert";
    case 0x7D: return L"IntlYen";
    default: return nullptr;
    }
}

// The key id one WM_KEYDOWN counts as, or nullptr when it is not a fresh physical press. Auto-repeat (lParam bit 30, the key was already down) counts once per press, so a held Backspace or Shift is one press. VK_PACKET carries an injected character in place of a scan code, so it names no physical key.
inline const wchar_t *KeyPressIdFromKeyDown(std::uintptr_t virtualKey, std::uintptr_t lParam)
{
    constexpr std::uintptr_t VirtualKeyPacket = 0xE7;
    if ((lParam & 0x40000000u) != 0 || (virtualKey & 0xFFFFu) == VirtualKeyPacket)
    {
        return nullptr;
    }
    return KeyIdFromScanCode(static_cast<unsigned>((lParam >> 16) & 0xFFu), (lParam & 0x01000000u) != 0);
}

// The physical identity the per-event de-duplication compares: scan code plus the extended bit, zero for nothing.
inline unsigned KeyPressPhysicalKey(std::uintptr_t lParam)
{
    return static_cast<unsigned>((lParam >> 16) & 0x1FFu);
}

// The secure desktop (sign-in, UAC) and any context with the keyboard disabled or empty - which is how password edits reach a TIP - are never counted. It is the same keyboard-disabled exclusion the passthrough statistics use.
inline bool ShouldCountKeyPress(const wchar_t *keyId, bool keyboardDisabled, bool secureMode)
{
    return keyId != nullptr && !keyboardDisabled && !secureMode;
}
