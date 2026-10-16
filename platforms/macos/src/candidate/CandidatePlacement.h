#pragma once
#import <AppKit/AppKit.h>
#include <cmath>

inline bool MSIMEValidCaret(NSRect caret) {
    return std::isfinite(caret.origin.x) && std::isfinite(caret.origin.y) &&
           std::isfinite(caret.size.width) && std::isfinite(caret.size.height) && caret.size.height > 0;
}

// The tallest vertical candidate panel shown since the panel last became visible, matching Windows' g_max_vertical_container_height_dip. A panel that was hidden in between starts over, as Windows' ResetCandidatePlacementMemory does when its candidate window hides; a horizontal panel carries the memory without adding to it.
inline CGFloat MSIMETallestCandidateHeight(CGFloat remembered, CGFloat current, bool vertical, bool panelWasVisible) {
    const CGFloat base = panelWasVisible ? MAX(remembered, 0.0) : 0.0;
    return vertical ? MAX(base, current) : base;
}

// Decides the side of the caret with the taller of the current height and flipDecisionHeight, but places the panel with its current height. A vertical list that grows as candidates arrive therefore does not jump from below the caret to above it, and a shorter later page that stays above the caret sits against the line instead of leaving the tallest page's gap.
// cardTopInset 是窗口顶上留给皮肤装饰的透明带（MSIMECandidateChromeView.cardTopInset），size 和 flipDecisionHeight 都含这条带。放在光标下方时贴着光标的是卡片顶边，不是窗口顶边：透明带朝上伸，盖在光标所在的这一行和上面几行上，装饰图不透明的地方会挡住那里的字。放不下、翻到光标上方时窗口底边就是卡片底边，透明带在最上面，和以前一样贴着光标上方。翻不翻只看卡片：卡片在光标下方放得下就放下方，透明带朝上伸不占光标下方的地方。最后整个窗口（连同透明带）仍夹进可见区域，光标贴近屏幕上边、透明带伸出上边时宁可把窗口往下压、卡片和光标之间留出一段，也不让装饰图被屏幕边缘裁掉；留出的这段至多是透明带伸出的部分，比以前整条透明带的空隙小。cardTopInset 为 0 时结果和以前完全一样。
inline NSPoint MSIMECandidateOrigin(NSRect caret, NSSize size, NSRect bounds, CGFloat flipDecisionHeight, CGFloat cardTopInset = 0) {
    const CGFloat x = MIN(MAX(NSMinX(caret), NSMinX(bounds)), MAX(NSMinX(bounds), NSMaxX(bounds) - size.width));
    const CGFloat band = MIN(MAX(cardTopInset, 0.0), size.height);
    const CGFloat decisionHeight = MAX(size.height, flipDecisionHeight) - band;
    CGFloat y = NSMinY(caret) - (size.height - band) - 4;
    if (NSMinY(caret) - decisionHeight - 4 < NSMinY(bounds)) y = NSMaxY(caret) + 4;
    y = MIN(MAX(y, NSMinY(bounds)), MAX(NSMinY(bounds), NSMaxY(bounds) - size.height));
    return NSMakePoint(x, y);
}

inline NSPoint MSIMECandidateOrigin(NSRect caret, NSSize size, NSRect bounds) {
    return MSIMECandidateOrigin(caret, size, bounds, size.height);
}

// 候选窗放在 frames 里的哪一块屏幕上：先取 NSPointInRect 含住锚点的第一块（两块屏幕共用的边归右边、上边那块，和以前一样）；一块都不含时取离锚点最近的一块，距离相同取排在前面的。NSPointInRect 不含矩形的右边和上边，光标正好贴着某块屏幕的右边或上边时以前一块都找不到，退回 NSScreen.mainScreen，候选窗就被摆到了另一块屏幕上。frames 为空时返回 NSNotFound。
inline NSUInteger MSIMECandidateScreenIndex(NSPoint anchor, NSArray<NSValue *> *frames) {
    NSUInteger nearest = NSNotFound;
    CGFloat nearestDistance = INFINITY;
    for (NSUInteger index = 0; index < frames.count; ++index) {
        const NSRect frame = frames[index].rectValue;
        if (NSPointInRect(anchor, frame)) return index;
        const CGFloat dx = MAX(MAX(NSMinX(frame) - anchor.x, 0.0), anchor.x - NSMaxX(frame));
        const CGFloat dy = MAX(MAX(NSMinY(frame) - anchor.y, 0.0), anchor.y - NSMaxY(frame));
        const CGFloat distance = dx * dx + dy * dy;
        if (distance < nearestDistance) {
            nearest = index;
            nearestDistance = distance;
        }
    }
    return nearest;
}

// 锚点所在的屏幕，规则见 MSIMECandidateScreenIndex；没有任何屏幕时才退回 NSScreen.mainScreen。候选窗、模式提示、双拼键位图、打字特效、悬浮工具栏和语音浮层都按它选屏，光标或鼠标贴着屏幕右边、上边时不会跳到另一块屏幕。
inline NSScreen *MSIMEScreenForPoint(NSPoint anchor) {
    NSArray<NSScreen *> *screens = NSScreen.screens;
    NSMutableArray<NSValue *> *frames = [NSMutableArray arrayWithCapacity:screens.count];
    for (NSScreen *screen in screens) [frames addObject:[NSValue valueWithRect:screen.frame]];
    const NSUInteger index = MSIMECandidateScreenIndex(anchor, frames);
    return index == NSNotFound ? NSScreen.mainScreen : screens[index];
}

// 候选窗所在的屏幕，锚点是光标的左边和竖直中线。
inline NSScreen *MSIMECandidateScreen(NSRect caret) {
    return MSIMEScreenForPoint(NSMakePoint(NSMinX(caret), NSMidY(caret)));
}
