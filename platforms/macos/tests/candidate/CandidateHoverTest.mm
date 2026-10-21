#import <AppKit/AppKit.h>
#import "../../src/candidate/CandidateChrome.h"

#include <cassert>

@interface CursorProbeButton : MSIMECandidateButton
@property(nonatomic, strong) NSCursor *capturedCursor;
@property(nonatomic) NSRect capturedRect;
@end
@implementation CursorProbeButton
- (void)addCursorRect:(NSRect)rect cursor:(NSCursor *)cursor
{
    self.capturedRect = rect;
    self.capturedCursor = cursor;
}
@end

static NSEvent *PointerEvent(NSEventType type, NSWindow *window, NSPoint location)
{
    if (type == NSEventTypeMouseEntered || type == NSEventTypeMouseExited)
        return [NSEvent enterExitEventWithType:type location:location modifierFlags:0
            timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber
            context:nil eventNumber:0 trackingNumber:0 userData:nullptr];
    return [NSEvent mouseEventWithType:type location:location modifierFlags:0
        timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber
        context:nil eventNumber:0 clickCount:0 pressure:0];
}

// 真实 AppKit 按钮及跟踪区域覆盖事件序列；不把注入消息当作系统输入法的人工验收。
static void CheckPointerTracking(MSIMECandidateButton *button)
{
    NSPanel *panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(100, 100, 360, 120)
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    button.frame = NSMakeRect(0, 0, 160, 36);
    [panel.contentView addSubview:button];
    [button updateTrackingAreas];
    assert(!button.candidateHovered);
    BOOL tracksMovement = NO;
    for (NSTrackingArea *area in button.trackingAreas)
        if (area.owner == button && (area.options & NSTrackingMouseMoved) &&
            (area.options & NSTrackingActiveAlways)) tracksMovement = YES;
    assert(tracksMovement);
    const NSPoint inside = NSMakePoint(40, 20);
    // 新窗口或跟踪区域的进入消息本身不能让静止、隐藏的指针激活悬停。
    [button mouseEntered:PointerEvent(NSEventTypeMouseEntered, panel, inside)];
    assert(!button.candidateHovered);
    [button mouseMoved:PointerEvent(NSEventTypeMouseMoved, panel, inside)];
    assert(button.candidateHovered);
    [button mouseExited:PointerEvent(NSEventTypeMouseExited, panel, NSMakePoint(220, 20))];
    assert(!button.candidateHovered);
    [button mouseMoved:PointerEvent(NSEventTypeMouseMoved, panel, inside)];
    assert(button.candidateHovered);
    [button updateTrackingAreas];
    assert(!button.candidateHovered);
    [button mouseEntered:PointerEvent(NSEventTypeMouseEntered, panel, inside)];
    assert(!button.candidateHovered);
    [button mouseMoved:PointerEvent(NSEventTypeMouseMoved, panel, NSMakePoint(50, 20))];
    assert(button.candidateHovered);
    [button mouseExited:PointerEvent(NSEventTypeMouseExited, panel, inside)];
    assert(!button.candidateHovered);
    [button removeFromSuperview];
}

int main()
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        MSIMECandidateButton *button = [MSIMECandidateButton buttonWithTitle:@"1  候选" target:nil action:nil];
        button.frame = NSMakeRect(0, 0, 160, 36);
        button.hoverColor = [NSColor colorWithSRGBRed:0.2 green:0.3 blue:0.4 alpha:0.5];
        CheckPointerTracking(button);
        CursorProbeButton *cursorButton = [[CursorProbeButton alloc] initWithFrame:NSMakeRect(3, 4, 160, 36)];
        [cursorButton resetCursorRects];
        assert(NSEqualRects(cursorButton.capturedRect, cursorButton.bounds));
        assert(cursorButton.capturedCursor == NSCursor.pointingHandCursor);
        cursorButton.enabled = NO;
        cursorButton.capturedCursor = nil;
        cursorButton.capturedRect = NSZeroRect;
        [cursorButton resetCursorRects];
        assert(cursorButton.capturedCursor == nil);
        assert(NSEqualRects(cursorButton.capturedRect, NSZeroRect));
        // 四角路径保持视觉上的上、下顺序，翻转坐标不交换圆角语义。
        for (BOOL flipped : {NO, YES}) {
            CGPathRef path = MSIMECreateCandidateCornerPath(NSMakeRect(1, 1, 98, 38), {16, 2, 2, 2}, flipped);
            assert(!CGPathContainsPoint(path, nullptr, CGPointMake(2, flipped ? 2 : 38), false));
            assert(CGPathContainsPoint(path, nullptr, CGPointMake(98, flipped ? 2 : 38), false));
            assert(CGPathContainsPoint(path, nullptr, CGPointMake(2, flipped ? 38 : 2), false));
            CGPathRelease(path);
        }
        // 材质承载真实按钮并避开装饰带，切换回退时保留同一按钮及其鼠标命中。
        MSIMECandidateChromeView *chrome = [[MSIMECandidateChromeView alloc] initWithFrame:NSMakeRect(0, 0, 240, 110)];
        chrome.cardTopInset = 30;
        chrome.cornerRadius = 12;
        chrome.fillColor = [NSColor.windowBackgroundColor colorWithAlphaComponent:0.6];
        button.frame = NSMakeRect(8, 8, 160, 36);
        [chrome.candidateContentView addSubview:button];
        chrome.liquidGlassEnabled = YES;
        if (@available(macOS 26.0, *)) {
            if (!NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency) {
                NSGlassEffectView *glass = (id)chrome.subviews.firstObject;
                assert([glass isKindOfClass:NSGlassEffectView.class]);
                assert(NSEqualRects(glass.frame, NSMakeRect(0, 0, 240, 80)));
                assert(glass.cornerRadius == 12);
                assert(glass.alphaValue == 1.0);
                assert(glass.contentView == chrome.candidateContentView);
                assert(button.superview == chrome.candidateContentView);
                assert([chrome hitTest:NSMakePoint(20, 20)] == button);
                chrome.frame = NSMakeRect(0, 0, 320, 160);
                [chrome layout];
                assert(NSEqualRects(glass.frame, NSMakeRect(0, 0, 320, 130)));
                [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:
                    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:nil];
                assert(chrome.subviews.firstObject == glass);
            } else {
                assert(chrome.subviews.count == 2);
            }
        } else {
            assert(chrome.subviews.count == 2);
        }
        chrome.liquidGlassEnabled = NO;
        // 独立卡片背景留在候选容器下方，透明回退仍保留按钮身份与鼠标命中。
        assert(chrome.subviews.count == 2);
        assert([chrome.subviews.firstObject isKindOfClass:MSIMECandidateCardBackgroundView.class]);
        assert(((MSIMECandidateCardBackgroundView *)chrome.subviews.firstObject).owner == chrome);
        assert(chrome.subviews.lastObject == chrome.candidateContentView && button.superview == chrome.candidateContentView);
        assert([chrome.subviews.firstObject hitTest:NSMakePoint(20, 20)] == nil);
        assert([chrome hitTest:NSMakePoint(20, 20)] == button);
        assert(chrome.fillColor.alphaComponent == 0.6);
        puts("Candidate hover tracking passed");
    }
    return 0;
}
