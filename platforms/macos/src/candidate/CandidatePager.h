#pragma once
#include "CandidateSkin.h"
#include <cmath>

namespace msime::mac {
struct CandidatePagerMetrics {
    bool triangle = false;
    double buttonWidth = 0, buttonHeight = 0, gap = 0, glyph = 0, stroke = 0;
    double marginLeft = 0, marginRight = 0, marginBottom = 0, dividerWidth = 0, dividerGap = 0;
    double column = 0, row = 0;
    double glyphWidth() const { return glyph * (triangle ? 0.84 : 0.275); }
};

// 固定来源 candidate_skin_scene.cpp 的 PagerArrows 配方；字号已缩放，DIP 常数另外乘 scale。
inline CandidatePagerMetrics CandidatePager(double font, double scale, double itemHeight, bool horizontal, bool triangle) {
    CandidatePagerMetrics result;
    result.triangle = triangle;
    result.buttonWidth = std::round(font * (triangle ? (horizontal ? 0.96 : 0.81) : 0.72));
    result.buttonHeight = std::round(triangle ? (horizontal ? itemHeight : font * 1.2) : font * 0.9);
    result.gap = triangle ? 0 : scale;
    result.glyph = font * (triangle ? 0.52 : 0.5);
    result.stroke = triangle ? result.glyph * 0.375 : std::max(1.2 * scale, font / 13.0);
    if (horizontal) {
        if (triangle) {
            result.marginLeft = std::round(font * 0.3);
            result.dividerWidth = scale;
            result.dividerGap = std::round(font * 0.12);
        } else {
            result.marginLeft = 2 * scale;
            result.marginRight = scale;
            result.marginBottom = std::max(0.0, (itemHeight - result.buttonHeight) / 2);
        }
        result.column = result.marginLeft + result.marginRight + result.dividerWidth + result.dividerGap +
            2 * result.buttonWidth + result.gap;
    } else result.row = result.buttonHeight + 2 * scale;
    return result;
}
} // msime::mac 命名空间

#ifdef __OBJC__
#import "CandidateChrome.h"

@interface MSIMECandidatePagerButton : MSIMECandidateButton
@property(nonatomic) msime::mac::CandidatePagerMetrics pagerMetrics;
@end
@implementation MSIMECandidatePagerButton
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    NSRectClip(self.bounds);
    NSColor *background = self.enabled ? (self.highlighted ? self.fillColor : self.candidateHovered ? self.hoverColor : nil) : nil;
    if (background && background.alphaComponent > 0.001) {
        [background setFill];
        [[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:self.cornerRadius yRadius:self.cornerRadius] fill];
    }
    const auto metrics = self.pagerMetrics;
    const double height = std::min(metrics.glyph, NSHeight(self.bounds)), width = metrics.triangle ? height * 0.84 : height * 0.275;
    const double cx = NSMidX(self.bounds), cy = NSMidY(self.bounds);
    const double direction = self.tag == -1 ? -1 : 1;
    const NSPoint tip = NSMakePoint(cx + direction * width / 2, cy);
    const NSPoint upper = NSMakePoint(cx - direction * width / 2, cy + height / 2);
    const NSPoint lower = NSMakePoint(upper.x, cy - height / 2);
    NSBezierPath *path = [NSBezierPath bezierPath];
    [self.titleColor set];
    if (metrics.triangle) {
        const NSPoint corners[] = {tip, upper, lower};
        const double radius = std::min(metrics.stroke, height / 2);
        const auto toward = [radius](NSPoint from, NSPoint to) {
            const double dx = to.x - from.x, dy = to.y - from.y, length = std::hypot(dx, dy);
            const double t = length > 0 ? radius / length : 0;
            return NSMakePoint(from.x + dx * t, from.y + dy * t);
        };
        [path moveToPoint:toward(corners[0], corners[1])];
        for (int index = 1; index <= 3; ++index) {
            const NSPoint corner = corners[index % 3];
            const NSPoint start = toward(corner, corners[(index - 1) % 3]);
            const NSPoint end = toward(corner, corners[(index + 1) % 3]);
            [path lineToPoint:start];
            // AppKit 的三次曲线等价表示来源二次贝塞尔，不叠加多个半透明形状。
            [path curveToPoint:end controlPoint1:NSMakePoint(start.x + (corner.x - start.x) * 2 / 3, start.y + (corner.y - start.y) * 2 / 3)
                controlPoint2:NSMakePoint(end.x + (corner.x - end.x) * 2 / 3, end.y + (corner.y - end.y) * 2 / 3)];
        }
        [path closePath];
        [path fill];
    } else {
        path.lineWidth = metrics.stroke;
        path.lineCapStyle = NSLineCapStyleButt;
        // 来源用两段平头直线；分别绘制也保留禁用颜色在交点的透明度叠加。
        for (NSPoint endpoint : {upper, lower}) {
            [path removeAllPoints];
            [path moveToPoint:endpoint]; [path lineToPoint:tip];
            [path stroke];
        }
    }
}
@end
#endif
