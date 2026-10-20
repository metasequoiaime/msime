#pragma once

#import <AppKit/AppKit.h>
#include "../candidate/CandidateSkin.h"

// 图片工具栏保留原生按钮动作，独立于普通条的自动布局。
@interface MSIMEToolbarImageCanvas : NSView
@property(nonatomic, readonly) NSSize preferredSize;
- (void)applySkin:(const msime::mac::SkinToolbarImage &)skin
           shown:(NSArray<NSArray<NSString *> *> *)shown
         buttons:(NSDictionary<NSString *, NSButton *> *)buttons
           scale:(CGFloat)scale
           color:(NSColor *)color;
// 坐标以画布左上角为原点；背景按全部动画帧的 alpha 并集命中。
- (BOOL)containsInteractivePoint:(NSPoint)point;
- (void)advanceAnimation;
@end
