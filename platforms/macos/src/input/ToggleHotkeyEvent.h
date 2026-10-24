#pragma once
#import <AppKit/AppKit.h>
#include "ToggleHotkeys.h"

namespace msime::mac {

// AppKit 按当前布局重译物理键，去掉 Option、Shift、Control 等全部修饰，不改变死键状态。
inline unsigned short ToggleHotkeyCharacter(NSEvent *event)
{
    NSString *characters = [event charactersByApplyingModifiers:0];
    return characters.length == 1 ? [characters characterAtIndex:0] : 0;
}

// DOM code 已由共享 Rust 解析为规范主键名；只在录入时把物理位置转成布局主键。
inline const char *RecordedToggleKeyName(const char *physicalName)
{
    if (!NSThread.isMainThread || !physicalName) return nullptr;
    for (const auto &entry : detail::kToggleChordKeyCodes) {
        if (!ToggleChordNameEquals(physicalName, entry.name)) continue;
        NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
            modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@""
            charactersIgnoringModifiers:@"" isARepeat:NO keyCode:entry.keyCode];
        return ToggleKeyNameForEvent(entry.keyCode, ToggleHotkeyCharacter(event));
    }
    return nullptr;
}

} // namespace msime::mac
