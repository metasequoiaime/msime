#include "../../src/core/EditableShiftFocus.h"
#include "../../src/core/ModifierTap.h"
#include <cassert>

@interface EditableShiftEvent : NSEvent
@property(nonatomic) NSEventType eventType;
@property(nonatomic) NSEventModifierFlags flags;
@property(nonatomic) unsigned short code;
@property(nonatomic) NSTimeInterval time;
@end
@implementation EditableShiftEvent
- (NSEventType)type { return self.eventType; }
- (NSEventModifierFlags)modifierFlags { return self.flags; }
- (unsigned short)keyCode { return self.code; }
- (NSTimeInterval)timestamp { return self.time; }
@end

static NSEvent *Event(NSEventType type, unsigned short code, NSEventModifierFlags flags, double time) {
    EditableShiftEvent *event = [EditableShiftEvent new];
    event.eventType = type; event.code = code; event.flags = flags; event.time = time;
    return event;
}

int main() {
    @autoreleasepool {
        using msime::mac::EditableShiftFocus;
        msime::mac::EditableShiftGate gate;
        MSIMEModifierTap tap;
        tap.setKeyHeldProbe([](unsigned short) { return false; });
        EditableShiftFocus focus{1, @"synthetic-field"};
        NSUInteger captures = 0;
        auto capture = [&] { ++captures; return focus; };
        auto down = Event(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.0);
        auto up = Event(NSEventTypeFlagsChanged, 56, 0, 1.1);
        auto observe = [&](NSEvent *event, bool restricted = true) {
            const bool allowed = gate.observe(event, restricted, capture);
            const bool toggled = tap.observe(event, true, true);
            const bool shift = event.keyCode == 56 || event.keyCode == 60;
            return toggled && (!shift || allowed);
        };
        auto reset = [&] { gate.reset(); tap.reset(); captures = 0; focus = {1, @"synthetic-field"}; };
        assert(!observe(up) && captures == 0);
        assert(!observe(down) && gate.armed() && captures == 1);
        assert(observe(up) && !gate.armed() && captures == 2);
        // 权限不足、未知或只读焦点都由捕获失败表达；按下或松开时失败均不得切换。
        for (bool failOnPress : {false, true}) {
            reset();
            if (failOnPress) focus = {};
            assert(!observe(down));
            focus = failOnPress ? EditableShiftFocus{1, @"synthetic-field"} : EditableShiftFocus{};
            assert(!observe(up));
        }
        for (EditableShiftFocus changed : {EditableShiftFocus{1, @"other-field"}, EditableShiftFocus{2, @"synthetic-field"}, EditableShiftFocus{1, @"synthetic-field", 1}}) {
            reset(); assert(!observe(down)); focus = changed; assert(!observe(up));
        }
        reset(); assert(!observe(down));
        gate.cancel(); // 鼠标监视器取消，随后按另一侧 Shift 也不能重新布防。
        assert(!observe(Event(NSEventTypeFlagsChanged, 60, NSEventModifierFlagShift, 1.02)));
        assert(!observe(Event(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.04)));
        assert(!observe(Event(NSEventTypeFlagsChanged, 60, 0, 1.1)));
        assert(captures == 1);
        reset(); assert(!observe(down));
        assert(!observe(Event(NSEventTypeFlagsChanged, 60, NSEventModifierFlagShift, 1.02)));
        assert(!observe(Event(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.04)));
        assert(observe(Event(NSEventTypeFlagsChanged, 60, 0, 1.1)));
        assert(captures == 2);
        for (NSEvent *interruption : @[
            Event(NSEventTypeKeyDown, 0, NSEventModifierFlagShift, 1.02),
            Event(NSEventTypeKeyUp, 0, NSEventModifierFlagShift, 1.02),
            Event(NSEventTypeFlagsChanged, 55, NSEventModifierFlagShift | NSEventModifierFlagCommand, 1.02)]) {
            reset(); assert(!observe(down)); assert(!observe(interruption));
            assert(!observe(up) && captures == 1);
        }
        reset(); assert(!observe(down));
        gate.reset(); tap.reset(); assert(!observe(up));
        reset(); assert(!observe(down));
        assert(!observe(Event(NSEventTypeFlagsChanged, 56, 0, 1.8)));
        reset(); focus = {};
        assert(!observe(down, false)); assert(observe(up, false)); assert(captures == 0);
        reset(); focus = {};
        assert(!observe(Event(NSEventTypeFlagsChanged, 59, NSEventModifierFlagControl, 1.0)));
        assert(observe(Event(NSEventTypeFlagsChanged, 59, 0, 1.1)) && captures == 0);
        // 不依赖系统授权也能验证空客户端必定拒绝，且不弹授权框。
        assert(!msime::mac::CaptureEditableShiftFocus(nil));
    }
}
