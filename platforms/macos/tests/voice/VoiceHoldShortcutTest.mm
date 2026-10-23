#import "../../src/voice/VoiceHoldShortcut.h"
#include <cassert>

static NSEvent *Event(unsigned short key, NSEventModifierFlags flags, NSEventType type = NSEventTypeFlagsChanged) {
    return [NSEvent keyEventWithType:type location:NSZeroPoint modifierFlags:flags timestamp:1 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key];
}
int main() {
    @autoreleasepool {
        using Action = MSIMEVoiceHoldShortcut::Action;
        MSIMEVoiceHoldShortcut shortcut;
        MSIMEVoiceHoldShortcut::Options all{true, true, true, true};
        const auto ro = NSEventModifierFlagOption | NX_DEVICERALTKEYMASK;
        const auto lo = NSEventModifierFlagOption | NX_DEVICELALTKEYMASK;
        const auto rc = NSEventModifierFlagControl | NX_DEVICERCTLKEYMASK;
        const auto lc = NSEventModifierFlagControl | NX_DEVICELCTLKEYMASK;
        const auto cmd = NSEventModifierFlagCommand | NX_DEVICELCMDKEYMASK;
        auto step = [&](unsigned short key, NSEventModifierFlags flags, bool recording, Action action, bool consumed, NSEventType type = NSEventTypeFlagsChanged) {
            const auto result = shortcut.observe(Event(key, flags, type), all, recording);
            assert(result.action == action && result.consumed == consumed);
        };
        step(49, 0, false, Action::None, false, NSEventTypeKeyDown); // Plain space never starts voice.
        step(61, ro, false, Action::Toggle, true);
        step(61, ro, true, Action::None, true); // Duplicate modifier snapshot.
        step(61, 0, true, Action::Toggle, true);
        step(61, 0, true, Action::None, false);
        shortcut.reset();
        step(61, ro, false, Action::Toggle, true);
        assert(!shortcut.locked());
        step(49, ro, true, Action::None, true, NSEventTypeKeyDown);
        assert(shortcut.locked()); // The overlay shows its actions from here on.
        step(61, 0, true, Action::None, true); // Space lock survives modifier release.
        step(49, 0, true, Action::None, true, NSEventTypeKeyDown);
        step(49, 0, true, Action::None, true, NSEventTypeKeyUp);
        step(61, ro, true, Action::Toggle, true); // Next shortcut stops locked recording.
        assert(!shortcut.locked());
        step(61, 0, true, Action::None, true); // Release must not cancel pending final.
        shortcut.reset();
        step(59, lc, false, Action::None, false);
        step(55, lc | cmd, false, Action::Toggle, true);
        step(59, cmd, true, Action::Toggle, false); // First modifier can release first.
        step(55, 0, true, Action::None, true);
        shortcut.reset();
        step(55, cmd, false, Action::None, false);
        step(59, cmd | lc, false, Action::None, false); // Written order is required.
        shortcut.reset();
        step(62, rc, false, Action::None, false);
        step(61, rc | ro, false, Action::Toggle, true);
        step(62, ro, true, Action::Toggle, false);
        step(61, 0, true, Action::None, true);
        shortcut.reset();
        step(58, lo, false, Action::None, false);
        step(61, lo | ro, false, Action::Toggle, true);
        step(61, lo, true, Action::Toggle, true); // Left Option does not hide right release.
        shortcut.reset();
        step(53, 0, true, Action::Cancel, true, NSEventTypeKeyDown);
        step(53, 0, false, Action::None, true, NSEventTypeKeyDown);
        step(53, 0, false, Action::None, true, NSEventTypeKeyUp);
        shortcut.reset();
        all = {false, false, true, true};
        step(59, lc, false, Action::None, false);
        step(61, lc | ro, false, Action::None, false); // Left Control is not right Control.
        shortcut.reset();
        step(61, 0, true, Action::None, false); // Departing focus cannot stop a successor.

        all = {true, false, false, true};
        auto physicalStep = [&](unsigned short key, NSEventModifierFlags flags, NSEventModifierFlags physicalFlags,
                                bool recording, Action action, bool consumed, bool onRelease = false) {
            const auto result = shortcut.observe(Event(key, flags), all, recording, physicalFlags);
            assert(result.action == action && result.consumed == consumed && result.onRelease == onRelease);
        };
        const auto option = NSEventModifierFlagOption;
        // 事件只带通用 Option 位时，从实体快照恢复右侧位；重复通知不能二次切换。
        physicalStep(61, option, ro, false, Action::Toggle, true);
        physicalStep(61, option, ro, true, Action::None, true);
        physicalStep(61, 0, 0, true, Action::Toggle, true, true);
        physicalStep(61, 0, 0, false, Action::None, false);
        shortcut.reset();
        // 左 Option 保持按住时，右 Option 的松开仍要结束录音。
        physicalStep(58, option, lo, false, Action::None, false);
        physicalStep(61, option, lo | ro, false, Action::Toggle, true);
        physicalStep(61, option, lo, true, Action::Toggle, true, true);
        physicalStep(61, option, lo, false, Action::None, false);
        shortcut.reset();
        // 左 Option 先松开不会结束右侧的按住；单独左侧也不能启动语音。
        physicalStep(61, option, ro, false, Action::Toggle, true);
        physicalStep(58, option, ro, true, Action::None, false);
        physicalStep(61, 0, 0, true, Action::Toggle, true, true);
        shortcut.reset();
        physicalStep(58, option, lo, false, Action::None, false);
        physicalStep(61, option, lo, false, Action::None, false);
        shortcut.reset();
        // 处理左侧事件时实体右侧已经按下，也不能提前占用尚未送达的右侧按下沿。
        physicalStep(58, option, lo | ro, false, Action::None, false);
        physicalStep(61, option, lo | ro, false, Action::Toggle, true);
        physicalStep(61, option, lo, true, Action::Toggle, true, true);
        shortcut.reset();
        // 原生事件已有侧别位时，以事件快照为准，不让较晚的实体状态覆盖它。
        physicalStep(61, ro, 0, false, Action::Toggle, true);
        physicalStep(61, lo, ro, true, Action::Toggle, true, true);
    }
}
