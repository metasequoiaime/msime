#pragma once
#include "CandidateSkin.h"
#import <AppKit/AppKit.h>

// 资源只在偏好或皮肤文件重载时解码；候选逐键重排复用同一份位图。
@interface MSIMECandidateCanvasAssets : NSObject
+ (instancetype)assetsForCanvas:(const msime::mac::SkinCanvas &)canvas dark:(BOOL)dark;
@property(nonatomic, readonly) BOOL panelLoaded;
@end

// 三层分别放在卡片下、候选下和候选上；所有层均不接收鼠标。
@interface MSIMECandidateCanvasView : NSView
- (instancetype)initWithFrame:(NSRect)frame
                       assets:(MSIMECandidateCanvasAssets *)assets
                       canvas:(const msime::mac::SkinCanvas &)canvas
                         card:(NSRect)card
                        level:(NSInteger)level;
- (void)setPlaybackVisible:(BOOL)visible anchor:(CFTimeInterval)anchor;
@end
