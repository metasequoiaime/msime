#pragma once
#import <AppKit/AppKit.h>

// 只替换背景，原有按钮、布局、拖动与鼠标穿透策略仍由宿主负责。
static inline NSView *MSIMEUpdateNativeGlassBackground(NSView *host, NSView *previous, BOOL enabled, CGFloat radius) {
    if (@available(macOS 26.0, *)) {
        if (enabled && !NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency) {
            NSGlassEffectView *glass = (NSGlassEffectView *)previous;
            if (!glass) {
                glass = [[NSGlassEffectView alloc] initWithFrame:host.bounds];
                glass.style = NSGlassEffectViewStyleRegular;
                glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
                [host addSubview:glass positioned:NSWindowBelow relativeTo:nil];
            }
            glass.frame = host.bounds;
            glass.cornerRadius = radius;
            return glass;
        }
    }
    [previous removeFromSuperview];
    return nil;
}
