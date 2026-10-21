#pragma once
#include <algorithm>
#include <cmath>

namespace msime::mac {
// 来源 CandidateExpandToggle 的尺寸；候选字号已经缩放，DIP 留白另外乘 scale。
struct CandidateExpandToggleMetrics {
    double side = 0, glyphWidth = 0, stroke = 0, marginTop = 0, marginLeft = 0, marginRight = 0;
    double column() const { return marginLeft + side + marginRight; }
};

inline CandidateExpandToggleMetrics CandidateExpandToggle(double font, double scale, double itemHeight) {
    const double side = std::round(font * 1.1);
    return {side, std::round(font * 0.6), std::max(1.2 * scale, font / 13.0),
        std::max(0.0, (itemHeight - side) / 2), 2 * scale, scale};
}
} // msime::mac 命名空间

#ifdef __OBJC__
#import "CandidateChrome.h"

@interface MSIMECandidateExpandToggleButton : MSIMECandidateButton
@property(nonatomic) msime::mac::CandidateExpandToggleMetrics toggleMetrics;
@property(nonatomic) BOOL expanded;
@end
@implementation MSIMECandidateExpandToggleButton
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    NSRectClip(self.bounds);
    NSColor *background = self.enabled ? (self.highlighted ? self.fillColor : self.candidateHovered ? self.hoverColor : nil) : nil;
    if (background && background.alphaComponent > 0.001) {
        [background setFill];
        [[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:self.cornerRadius yRadius:self.cornerRadius] fill];
    }
    const auto metrics = self.toggleMetrics;
    const double halfWidth = std::min(metrics.glyphWidth, NSWidth(self.bounds)) / 2;
    const double halfHeight = halfWidth / 2, cx = NSMidX(self.bounds), cy = NSMidY(self.bounds);
    // 按实际视图坐标方向画：收起状态尖朝下，展开状态尖朝上。
    const double down = self.isFlipped ? 1 : -1;
    const double tip = cy + (self.expanded ? -halfHeight : halfHeight) * down;
    const double tail = cy + (self.expanded ? halfHeight : -halfHeight) * down;
    [self.titleColor setStroke];
    NSBezierPath *path = [NSBezierPath bezierPath];
    path.lineWidth = metrics.stroke;
    path.lineCapStyle = NSLineCapStyleButt;
    // 来源分别画两段线，交点同样保留颜色透明度的叠加。
    for (double x : {cx - halfWidth, cx + halfWidth}) {
        [path removeAllPoints];
        [path moveToPoint:NSMakePoint(x, tail)];
        [path lineToPoint:NSMakePoint(cx, tip)];
        [path stroke];
    }
}
@end
#endif
