#pragma once
#import <AppKit/AppKit.h>
#include "../../../../shared/candidate/CandidatePlacement.h"

// 规则本身在 shared/candidate/CandidatePlacement.h，Linux 的 X11 原生画布用同一份；这里只把 NSRect、NSPoint 换成共享的结构。
static inline msime::candidate::CandidateRect MSIMECandidateRect(NSRect rect) {
    return {NSMinX(rect), NSMinY(rect), NSWidth(rect), NSHeight(rect)};
}

inline bool MSIMEValidCaret(NSRect caret) {
    return msime::candidate::CandidateValidCaret(MSIMECandidateRect(caret));
}

// 见 msime::candidate::TallestCandidateHeight。
inline CGFloat MSIMETallestCandidateHeight(CGFloat remembered, CGFloat current, bool vertical, bool panelWasVisible) {
    return msime::candidate::TallestCandidateHeight(remembered, current, vertical, panelWasVisible);
}

// 见 msime::candidate::WidestCandidateWidth。
inline CGFloat MSIMEWidestCandidateWidth(CGFloat remembered, CGFloat current, bool vertical, bool panelWasVisible) {
    return msime::candidate::WidestCandidateWidth(remembered, current, vertical, panelWasVisible);
}

// 见 msime::candidate::CandidateOrigin：光标下方 4pt，放不下按记住的最高一页翻到上方，装饰透明带不占光标间隙，最后夹进可见区域。
inline NSPoint MSIMECandidateOrigin(NSRect caret, NSSize size, NSRect bounds, CGFloat flipDecisionHeight, CGFloat cardTopInset = 0,
                                   CGFloat cardLeftInset = 0, CGFloat cardBottomInset = 0) {
    const auto origin = msime::candidate::CandidateOrigin(MSIMECandidateRect(caret), size.width, size.height, MSIMECandidateRect(bounds),
                                                          flipDecisionHeight, cardTopInset, cardLeftInset, cardBottomInset);
    return NSMakePoint(origin.x, origin.y);
}

inline NSPoint MSIMECandidateOrigin(NSRect caret, NSSize size, NSRect bounds) {
    return MSIMECandidateOrigin(caret, size, bounds, size.height);
}

// 见 msime::candidate::CandidateScreenIndex。NSPointInRect 不含矩形的右边和上边，光标正好贴着某块屏幕的右边或上边时以前一块都找不到，退回 NSScreen.mainScreen，候选窗就被摆到了另一块屏幕上；共享规则在一块都不含时取最近的一块。frames 为空时返回 NSNotFound。
inline NSUInteger MSIMECandidateScreenIndex(NSPoint anchor, NSArray<NSValue *> *frames) {
    std::vector<msime::candidate::CandidateRect> rects;
    rects.reserve(frames.count);
    for (NSValue *frame in frames) rects.push_back(MSIMECandidateRect(frame.rectValue));
    const auto index = msime::candidate::CandidateScreenIndex({anchor.x, anchor.y}, rects);
    return index ? (NSUInteger)*index : NSNotFound;
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
