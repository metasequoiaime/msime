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

int main()
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        MSIMECandidateButton *button = [MSIMECandidateButton buttonWithTitle:@"1  候选" target:nil action:nil];
        button.frame = NSMakeRect(0, 0, 160, 36);
        button.hoverColor = [NSColor colorWithSRGBRed:0.2 green:0.3 blue:0.4 alpha:0.5];
        [button updateTrackingAreas];
        assert(!button.candidateHovered);
        [button mouseEntered:(NSEvent *)[NSNull null]];
        assert(button.candidateHovered);
        [button mouseExited:(NSEvent *)[NSNull null]];
        assert(!button.candidateHovered);
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
        // 背景材质必须避开装饰带，且不改变按钮归属、鼠标命中与可用的回退背景。
        MSIMECandidateChromeView *chrome = [[MSIMECandidateChromeView alloc] initWithFrame:NSMakeRect(0, 0, 240, 110)];
        chrome.cardTopInset = 30;
        chrome.cornerRadius = 12;
        chrome.fillColor = [NSColor.windowBackgroundColor colorWithAlphaComponent:0.6];
        button.frame = NSMakeRect(8, 8, 160, 36);
        [chrome addSubview:button];
        chrome.liquidGlassEnabled = YES;
        if (@available(macOS 26.0, *)) {
            if (!NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency) {
                NSGlassEffectView *glass = (id)chrome.subviews.firstObject;
                assert([glass isKindOfClass:NSGlassEffectView.class]);
                assert(NSEqualRects(glass.frame, NSMakeRect(0, 0, 240, 80)));
                assert(glass.cornerRadius == 12);
                assert(glass.alphaValue == 0.6);
                assert(button.superview == chrome);
                assert([chrome hitTest:NSMakePoint(20, 20)] == button);
                chrome.frame = NSMakeRect(0, 0, 320, 160);
                [chrome layout];
                assert(NSEqualRects(glass.frame, NSMakeRect(0, 0, 320, 130)));
                [NSWorkspace.sharedWorkspace.notificationCenter postNotificationName:
                    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:nil];
                assert(chrome.subviews.firstObject == glass);
            } else {
                assert(chrome.subviews.count == 1);
            }
        } else {
            assert(chrome.subviews.count == 1);
        }
        chrome.liquidGlassEnabled = NO;
        assert(chrome.subviews.count == 1 && chrome.subviews.firstObject == button);
        assert(chrome.fillColor.alphaComponent == 0.6);
        puts("Candidate hover tracking passed");
    }
    return 0;
}
