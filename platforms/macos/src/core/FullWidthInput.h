#pragma once

// From MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.

#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#include "../input/ToggleHotkeyEvent.h"

namespace msime::mac
{
// `character` 是当前键盘布局的无修饰字符。全半角有两条触发臂：共享偏好 `keybindings.toggle_full_width`
// 绑定的 chord（出厂 Ctrl+Shift+Space，各布局上同一个键，按 keyCode 认），以及
// macOS 既有的 Option+Shift+H（按布局字母认），见 ToggleHotkeys.h。
inline ToggleFullWidthArm FullWidthInputToggleArm(unsigned short keyCode, unsigned short character,
                                                  NSEventModifierFlags modifiers, ToggleChord chord)
{
    return FullWidthToggleArmFor(chord, keyCode, character,
        (modifiers & NSEventModifierFlagControl) != 0,
        (modifiers & NSEventModifierFlagOption) != 0,
        (modifiers & NSEventModifierFlagShift) != 0,
        (modifiers & NSEventModifierFlagCommand) != 0);
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
