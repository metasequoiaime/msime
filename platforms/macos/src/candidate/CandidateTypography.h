#pragma once

#import <AppKit/AppKit.h>
#include "../../../../shared/candidate/CandidateChromeMetrics.h"

// Windows candidate rendering uses a label font at 80% of the candidate font.
// Keep the scale in one place while letting AppKit retain the selected font's
// family and variation axes.
static const CGFloat MSIMECandidateNumberScale = msime::candidate::kCandidateNumberScale;
static const CGFloat MSIMECandidateNumberGap = msime::candidate::kCandidateNumberGap;
// The brand mark at the start of the top row, and the room between it and the reading.
static const CGFloat MSIMECandidateLogoSide = msime::candidate::kCandidateLogoSide;
static const CGFloat MSIMECandidateLogoGap = msime::candidate::kCandidateLogoGap;

static inline NSFont *MSIMECandidateNumberFont(NSFont *font, CGFloat numberScale = MSIMECandidateNumberScale)
{
    if (![font isKindOfClass:NSFont.class])
    {
        return [NSFont systemFontOfSize:14.4];
    }
    // AppKit 把 0 字号解释为默认字号；隐藏序号由绘制方决定，测量只使用正的最小字号。
    const CGFloat pointSize = MAX(0.01, font.pointSize * numberScale);
    return [NSFont fontWithDescriptor:font.fontDescriptor size:pointSize] ?:
        [NSFont systemFontOfSize:pointSize];
}
