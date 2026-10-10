#pragma once

// From MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.

#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

namespace msime::mac
{
// `letter` 是按当前键盘布局认出的快捷键字母（见 InputControllerPhysicalKeys.h 的 `ShortcutLetter`），Option+Shift+H 按它匹配；空格在各布局上是同一个键，仍按 keyCode 认。
inline bool IsFullWidthInputToggle(unsigned short keyCode, char letter, NSEventModifierFlags modifiers)
{
    const NSEventModifierFlags allModifiers = NSEventModifierFlagCommand | NSEventModifierFlagControl |
        NSEventModifierFlagOption | NSEventModifierFlagShift;
    if (keyCode == kVK_Space && (modifiers & allModifiers) == (NSEventModifierFlagControl | NSEventModifierFlagShift)) return true;
    const NSEventModifierFlags competingModifiers =
        modifiers & (NSEventModifierFlagCommand | NSEventModifierFlagControl);
    return letter == 'h' && (modifiers & NSEventModifierFlagOption) != 0 &&
           (modifiers & NSEventModifierFlagShift) != 0 && competingModifiers == 0;
}

inline bool IsFullWidthConvertibleCharacter(unichar character)
{
    return character == ' ' || (character >= '!' && character <= '~');
}

inline bool IsFullWidthDirectCharacter(unichar character, NSEventModifierFlags modifiers)
{
    const NSEventModifierFlags competingModifiers =
        modifiers & (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption);
    return competingModifiers == 0 && IsFullWidthConvertibleCharacter(character);
}

inline unichar FullWidthCharacter(unichar character)
{
    return character == ' ' ? 0x3000 : static_cast<unichar>(character + 0xFEE0);
}
} // namespace msime::mac
