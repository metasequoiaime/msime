#pragma once
#import <AppKit/AppKit.h>

#include "CandidateSkin.h"

// 候选窗皮肤动效（进场、退场、选中），两条渲染路径共用：IMK 产品用的 InputController 和测试用的 MetasequoiaCandidatePanel。规则见 .agents/notes/implemented/feature/2026-10-16-macos-candidate-declared-motion.md。
//
// 显示、隐藏和选中都是同步的：面板的 isVisible、窗口透明度和高亮状态立即变成新值，动画只加在 Core Animation 图层上，从不决定输入路由。退场时内容视图移到一个不接收鼠标的独立窗口里淡出，真正的候选窗已经收起；完成回调带代次令牌，再次显示会立即撤掉退场窗。

// 选中预设接管高亮底色的候选行：动画期间底色画在动效自己的图层上，行只画文字和选中条。
@protocol MSIMECandidateSelectionFillHost <NSObject>
@property(nonatomic) BOOL selectionFillDetached;
@end

// APNG 装饰图解码好的各帧，供 Core Animation 逐帧播放。帧时长和循环次数取自文件（ImageIO 的 APNG 属性）。
@interface MSIMEDecorationFrames : NSObject
// 文件不是多帧图片或读不出时返回 nil。帧数按共享加载器的上限截断。
+ (instancetype)framesWithContentsOfFile:(NSString *)path;
// CGImageRef，已解码。
@property(nonatomic, readonly) NSArray *images;
// 每帧开始的时刻占整轮的比例，比 images 多一项，首项 0、末项 1，供离散关键帧使用。
@property(nonatomic, readonly) NSArray<NSNumber *> *keyTimes;
// 播完一轮的秒数。
@property(nonatomic, readonly) CFTimeInterval duration;
// 播放几轮，0 表示无限循环。
@property(nonatomic, readonly) NSUInteger loopCount;
@end

// 候选窗上方的装饰图。图片画在自己的图层内容里，所以 APNG 的各帧和待机动效都能直接加在这个图层上；不接收鼠标。
@interface MSIMECandidateDecorationView : NSView
@property(nonatomic, strong) NSImage *image;
// APNG 的各帧；nil 时是静态图。图层内容平时是第一帧，动画只在 MSIMECandidateMotion 挂上时播放。
@property(nonatomic, strong) MSIMEDecorationFrames *frames;
@end

@interface MSIMECandidateMotion : NSObject
// 系统「减弱动态效果」，随通知更新；测试直接设置。为 YES 时不播放任何动画，设为 YES 会立刻结束正在播放的动画。
@property(nonatomic) BOOL reduceMotion;
// 退场窗还在屏幕上。
@property(nonatomic, readonly) BOOL exiting;
@property(nonatomic, readonly) NSWindow *exitWindow;
// 正在接管底色的选中图层，没有时为 nil。
@property(nonatomic, readonly) CALayer *selectionLayer;
// 最近一次显示或收起对动效的处理，写进诊断日志：播了进场或退场，或者没播、被哪一条规则挡下（减弱动态效果、皮肤没写这个预设、连续输入、正在退场等）。
@property(nonatomic, readonly, copy) NSString *lastDecision;
// 把面板摆上屏幕（替代 orderFrontRegardless）。面板原本隐藏时视情况播放进场；正在退场时撤掉退场窗，以完整不透明度显示。rows 是参与错峰的候选行，按显示顺序。
- (void)showPanel:(NSWindow *)panel animation:(const msime::mac::SkinAnimation &)animation rows:(NSArray<NSView *> *)rows;
// 立即收起面板。allowExit 为 YES 且皮肤写了退场时，内容视图移到退场窗淡出，面板换上一个空的内容视图。
- (void)hidePanel:(NSWindow *)panel animation:(const msime::mac::SkinAnimation &)animation allowExit:(BOOL)allowExit;
// 高亮已经同步移到 row 之后调用：按皮肤的选中预设装饰新高亮行的底色。
- (void)decorateSelectionOfRow:(NSView<MSIMECandidateSelectionFillHost> *)row
                          fill:(NSColor *)fill
                  cornerRadius:(CGFloat)cornerRadius
                        accent:(NSColor *)accent
                     animation:(const msime::mac::SkinAnimation &)animation;
// 结束选中装饰，行重新自己画底色。
- (void)endSelection;
// 给面板上的装饰图挂上待机动效和 APNG 帧动画，在 showPanel: 之后调用；每次重排都可以再调，相位接着同一条时间线，不从头开始。面板不可见、减弱动态效果或屏幕睡眠时不挂，已有的撤掉。
- (void)animateDecoration:(MSIMECandidateDecorationView *)decoration
                   motion:(msime::mac::DecorationMotion)motion
                 periodMs:(double)periodMs;
// 撤掉装饰图上的动画，图片停在第一帧、回到原位。
- (void)stopDecoration;
// 正在动的装饰图，没有时为 nil。
@property(nonatomic, readonly) MSIMECandidateDecorationView *animatedDecoration;
// 装饰图动画的相位起点（CACurrentMediaTime 的时刻）：候选窗停下来之后第一次出现时取当时，连续输入期间不变。
@property(nonatomic, readonly) CFTimeInterval decorationAnchor;
// 立即结束所有动画并撤掉退场窗。
- (void)stop;
@end
