#include "../../src/input/ToggleHotkeyEvent.h"
#include <cassert>

// 注入布局翻译结果，不改变运行测试机器的输入源，也不投递事件。
@interface ToggleLayoutEvent : NSEvent
@property(nonatomic) unsigned short physicalCode;
@property(nonatomic, copy) NSString *unmodified;
@property(nonatomic) NSUInteger translations;
@end
@implementation ToggleLayoutEvent
- (unsigned short)keyCode { return self.physicalCode; }
- (NSString *)charactersByApplyingModifiers:(NSEventModifierFlags)modifiers {
    assert(modifiers == 0);
    ++self.translations;
    return self.unmodified;
}
@end

int main() {
    @autoreleasepool {
        ToggleLayoutEvent *event = [ToggleLayoutEvent new];
        const auto period = msime::mac::FactoryPunctuationToggleChord();
        for (NSString *text in @[@".", @"v", @"", @"\u0430"]) {
            event.physicalCode = 47;
            event.unmodified = text;
            const auto character = msime::mac::ToggleHotkeyCharacter(event);
            assert(msime::mac::ToggleChordMatches(period, event.keyCode, character,
                true, false, false, false) == ![text isEqual:@"v"]);
        }
        event.physicalCode = 14;
        event.unmodified = @".";
        assert(msime::mac::ToggleChordMatches(period, event.keyCode,
            msime::mac::ToggleHotkeyCharacter(event), true, false, false, false));
        assert(event.translations == 5);
        assert(msime::mac::RecordedToggleKeyName("Unknown") == nullptr);
    }
}
