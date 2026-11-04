#import <AppKit/AppKit.h>
#import "../../src/candidate/CandidateChrome.h"
#import "../../src/candidate/CandidateExpandToggle.h"
#import "../../src/candidate/CandidatePager.h"

#include <cassert>
#include <cstdio>

@interface MSIMEButtonFactoryProbeTarget : NSObject
- (void)fire:(id)sender;
@end
@implementation MSIMEButtonFactoryProbeTarget
- (void)fire:(id)sender
{
    (void)sender;
}
@end

// 候选窗按钮改用 MSIMEMakeCandidateButton 建之后，不该在外观和行为上和 +buttonWithTitle:target:action: 建的有任何差别，只少那次 sizeToFit。这里逐项对比两种建法得到的属性；frame 除外，那正是要省掉的那次计算，调用方随后总会赋排版算出的 frame。
static NSArray<NSString *> *ComparedKeyPaths()
{
    return @[
        @"title", @"attributedTitle", @"alternateTitle", @"attributedAlternateTitle",
        @"bezelStyle", @"bordered", @"transparent", @"showsBorderOnlyWhileMouseInside",
        @"alignment", @"font", @"baseWritingDirection", @"usesSingleLineMode",
        @"lineBreakMode", @"cell.lineBreakMode", @"cell.wraps", @"cell.scrollable", @"cell.truncatesLastVisibleLine",
        @"imagePosition", @"imageScaling", @"imageHugsTitle", @"cell.imageDimsWhenDisabled", @"image", @"alternateImage",
        @"cell.highlightsBy", @"cell.showsStateBy", @"cell.type", @"cell.backgroundColor",
        @"controlSize", @"focusRingType", @"refusesFirstResponder", @"keyEquivalent", @"keyEquivalentModifierMask",
        @"enabled", @"state", @"allowsMixedState", @"continuous", @"springLoaded", @"maxAcceleratorLevel",
        @"bezelColor", @"contentTintColor", @"sound", @"tag", @"toolTip",
        @"autoresizingMask", @"translatesAutoresizingMaskIntoConstraints", @"userInterfaceLayoutDirection",
        @"hidden", @"alphaValue", @"wantsLayer", @"intrinsicContentSize",
    ];
}

static void CheckParity(Class cls, NSString *title)
{
    MSIMEButtonFactoryProbeTarget *target = [MSIMEButtonFactoryProbeTarget new];
    NSButton *expected = [cls buttonWithTitle:title target:target action:@selector(fire:)];
    NSButton *actual = MSIMEMakeCandidateButton(cls, title, target, @selector(fire:));
    assert([actual isMemberOfClass:cls]);
    assert(actual.target == target);
    assert(actual.action == @selector(fire:));
    // 不按标题算尺寸：建好时还是零大小，等调用方赋 frame。
    assert(NSEqualRects(actual.frame, NSZeroRect));
    for (NSString *keyPath in ComparedKeyPaths()) {
        id want = [expected valueForKeyPath:keyPath];
        id got = [actual valueForKeyPath:keyPath];
        if (want == got || [want isEqual:got]) continue;
        fprintf(stderr, "%s \"%s\": %s differs, buttonWithTitle %s, factory %s\n", NSStringFromClass(cls).UTF8String,
            title.UTF8String, keyPath.UTF8String, [want description].UTF8String, [got description].UTF8String);
        assert(false);
    }
    // 按下时机（按下还是松开时发动作）也要一致；-sendActionOn: 返回原来的掩码。
    assert([expected sendActionOn:NSEventMaskLeftMouseUp] == [actual sendActionOn:NSEventMaskLeftMouseUp]);
}

int main()
{
    @autoreleasepool {
        // 两个展开开关和列表翻页按钮用空标题建；initWithFrame: 默认标题是 "Button"，工厂必须照样写进空串。
        NSArray<NSString *> *titles = @[@"", @"‹", @"›", @"1  候选", @"這是一個很長很長的在線候選預覽，會被截斷"];
        NSArray<Class> *classes = @[MSIMECandidateButton.class, MSIMECandidatePagerButton.class,
            MSIMECandidateExpandToggleButton.class];
        for (Class cls in classes)
            for (NSString *title in titles) CheckParity(cls, title);
        puts("Candidate button factory parity passed");
    }
    return 0;
}
