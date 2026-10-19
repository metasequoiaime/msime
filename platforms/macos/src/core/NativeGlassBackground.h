#pragma once
#import <AppKit/AppKit.h>

static inline void MSIMEPinGlassContent(NSView *host, NSView *content) {
    if (content.superview != host) [host addSubview:content];
    if (content.translatesAutoresizingMaskIntoConstraints) {
        content.translatesAutoresizingMaskIntoConstraints = NO;
        [NSLayoutConstraint activateConstraints:@[
            [content.leadingAnchor constraintEqualToAnchor:host.leadingAnchor],
            [content.trailingAnchor constraintEqualToAnchor:host.trailingAnchor],
            [content.topAnchor constraintEqualToAnchor:host.topAnchor],
            [content.bottomAnchor constraintEqualToAnchor:host.bottomAnchor],
        ]];
    }
}

// 实际内容进入玻璃容器，避免兄弟背景遮挡控件或绕过系统的可读性处理。
static inline NSView *MSIMEUpdateNativeGlassBackground(NSView *host, NSView *content, NSView *previous, BOOL enabled, CGFloat radius) {
    NSCAssert(host != content, @"玻璃宿主与实际内容必须分开");
    if (@available(macOS 26.0, *)) {
        if (enabled && !NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency) {
            NSGlassEffectView *glass = (NSGlassEffectView *)previous;
            if (!glass) {
                glass = [[NSGlassEffectView alloc] initWithFrame:host.bounds];
                glass.style = NSGlassEffectViewStyleRegular;
                glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
                glass.contentView = content;
                MSIMEPinGlassContent(host, glass);
            }
            glass.frame = host.bounds;
            glass.cornerRadius = radius;
            return glass;
        }
        if (previous) ((NSGlassEffectView *)previous).contentView = nil;
    }
    [previous removeFromSuperview];
    if (content.superview != host) content.translatesAutoresizingMaskIntoConstraints = YES;
    content.frame = host.bounds;
    MSIMEPinGlassContent(host, content);
    return nil;
}
