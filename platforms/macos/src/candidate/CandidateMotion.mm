#import "CandidateMotion.h"

#import <QuartzCore/QuartzCore.h>

#include <algorithm>
#include <cmath>

#if !__has_feature(objc_arc)
#error "CandidateMotion.mm must be compiled with -fobjc-arc"
#endif

using msime::mac::SkinAnimation;
using msime::mac::SkinAnimationPreset;

namespace
{
// 收起后这么短时间内再次显示，算同一段连续输入，不播进场。连续打字的平均击键间隔约 240ms（Dhakal 等，CHI 2018，1.36 亿次击键），上屏键到下一个声母之间再多一点停顿；超过 400ms 才算停下来看，候选窗那时重新出现可以有进场。
constexpr CFTimeInterval kTypingBurstInterval = 0.4;
// 错峰的总延迟上限：再长的一页，最后一行也只比第一行晚这么久开始。
constexpr CFTimeInterval kStaggerCap = 0.08;
// 卡片的位移只用于从下方升起和向下退出；窗口正好是卡片大小，位移越大，起止几帧被窗口边缘裁掉的越多。
constexpr CGFloat kSlideDistance = 8.0;
constexpr CGFloat kRowRevealDistance = 3.0;

NSString *const kEnterOpacityKey = @"msime-candidate-enter-opacity";
NSString *const kEnterTransformKey = @"msime-candidate-enter-transform";
NSString *const kRowRevealKey = @"msime-candidate-row-reveal";
NSString *const kExitOpacityKey = @"msime-candidate-exit-opacity";
NSString *const kExitTransformKey = @"msime-candidate-exit-transform";
NSString *const kSelectionKey = @"msime-candidate-selection";

// 与共享契约一致的 40–600ms。
CFTimeInterval MotionSeconds(double milliseconds)
{
    return std::clamp(milliseconds / 1000.0, 0.04, 0.6);
}

CAMediaTimingFunction *Curve(NSString *name)
{
    return [CAMediaTimingFunction functionWithName:name];
}

// Web 候选栏进场用的 cubic-bezier(.16, 1, .3, 1)：一开始就走完大半，尾部很长。
CAMediaTimingFunction *SettleCurve()
{
    return [CAMediaTimingFunction functionWithControlPoints:0.16f :1.0f :0.3f :1.0f];
}

// 以图层中心缩放 scale，再在屏幕方向上抬起 lift 点（负数向下）。AppKit 视图图层的锚点在角上，直接缩放会缩向角落，所以把锚点到中心的偏移补进平移。变换里的平移落在父图层的坐标系里，方向要看父图层是否翻转，不能看图层自己：NSButton 是翻转视图，它的图层 contentsAreFlipped，而它所在的卡片图层并不翻转。
CATransform3D CenteredTransform(CALayer *layer, CGFloat scale, CGFloat lift)
{
    const CGRect bounds = layer.bounds;
    const CGFloat anchorX = CGRectGetMinX(bounds) + layer.anchorPoint.x * CGRectGetWidth(bounds);
    const CGFloat anchorY = CGRectGetMinY(bounds) + layer.anchorPoint.y * CGRectGetHeight(bounds);
    CATransform3D transform = CATransform3DMakeScale(scale, scale, 1.0);
    transform.m41 = (1.0 - scale) * (CGRectGetMidX(bounds) - anchorX);
    transform.m42 = (1.0 - scale) * (CGRectGetMidY(bounds) - anchorY) + (layer.superlayer.contentsAreFlipped ? -lift : lift);
    return transform;
}

// AppKit 给图层宿主视图的子视图建图层是惰性的：整页重建后的候选行要到这轮事件结束时的绘制才有自己的图层、才挂到卡片图层下面，在那之前加的错峰和选中装饰都找不到图层。面板已经在屏幕上时提前做这次绘制，它本来就要在这轮事件结束时发生，所以不多花时间；没有缺图层的行就什么都不做。
void AttachRowLayers(NSArray<NSView *> *rows)
{
    for (NSView *row in rows)
        if (row.window.isVisible && (row.layer == nil || row.layer.superlayer != row.superview.layer))
        {
            [row.window displayIfNeeded];
            return;
        }
}

CABasicAnimation *Basic(NSString *keyPath, id from, id to, CFTimeInterval duration, CAMediaTimingFunction *curve)
{
    CABasicAnimation *animation = [CABasicAnimation animationWithKeyPath:keyPath];
    animation.fromValue = from;
    animation.toValue = to;
    animation.duration = duration;
    animation.timingFunction = curve;
    return animation;
}

CAKeyframeAnimation *Keyframes(NSString *keyPath, NSArray *values, NSArray<NSNumber *> *times, CFTimeInterval duration)
{
    CAKeyframeAnimation *animation = [CAKeyframeAnimation animationWithKeyPath:keyPath];
    animation.values = values;
    animation.keyTimes = times;
    animation.duration = duration;
    animation.timingFunction = Curve(kCAMediaTimingFunctionEaseOut);
    return animation;
}

NSValue *TransformValue(CATransform3D transform)
{
    return [NSValue valueWithCATransform3D:transform];
}

// 只有 fade_scale 和 slide_up 逐行错峰，与 Web 候选栏相同。
bool StaggersRows(SkinAnimationPreset preset)
{
    return preset == SkinAnimationPreset::fade_scale || preset == SkinAnimationPreset::slide_up;
}
} // namespace

@interface MSIMECandidateExitWindow : NSPanel
@end
@implementation MSIMECandidateExitWindow
- (BOOL)canBecomeKeyWindow
{
    return NO;
}
- (BOOL)canBecomeMainWindow
{
    return NO;
}
@end

@implementation MSIMECandidateMotion
{
    MSIMECandidateExitWindow *_exitWindow;
    BOOL _exiting;
    uint64_t _exitToken;
    // 上一次面板从显示变成隐藏的时间，用于连续输入的判断。
    CFTimeInterval _lastHide;
    // 正在播放的进场：面板、内容视图、时间线和预设。_enterPanel 非 nil 且没到 _enterEnd 就是在进场；整页重建换掉的旧内容视图可能已经释放，新的内容视图接着这条时间线播完。
    __weak NSView *_enterContent;
    __weak NSWindow *_enterPanel;
    CFTimeInterval _enterStart;
    CFTimeInterval _enterEnd;
    SkinAnimation _enterAnimation;
    uint64_t _enterToken;
    // 进场期间关掉的窗口阴影：窗口服务器不会跟着图层透明度重算阴影，淡入时卡片还没出现阴影就满了，进场结束再还原。
    __weak NSWindow *_shadowPanel;
    CALayer *_selectionLayer;
    __weak NSView<MSIMECandidateSelectionFillHost> *_selectionRow;
    uint64_t _selectionToken;
}

- (instancetype)init
{
    self = [super init];
    if (self)
    {
        _lastHide = -INFINITY;
        _reduceMotion = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion;
        [NSWorkspace.sharedWorkspace.notificationCenter addObserver:self
                                                           selector:@selector(accessibilityDisplayOptionsDidChange:)
                                                               name:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification
                                                             object:nil];
    }
    return self;
}

- (void)dealloc
{
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    [_exitWindow orderOut:nil];
}

- (void)accessibilityDisplayOptionsDidChange:(NSNotification *)notification
{
    (void)notification;
    self.reduceMotion = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion;
}

- (void)setReduceMotion:(BOOL)reduceMotion
{
    _reduceMotion = reduceMotion;
    if (reduceMotion) [self stop];
}

- (BOOL)exiting
{
    return _exiting;
}

- (NSWindow *)exitWindow
{
    return _exitWindow;
}

- (CALayer *)selectionLayer
{
    return _selectionLayer;
}

- (BOOL)allowsMotion:(const SkinAnimation &)animation
{
    return !_reduceMotion && animation.durationMs > 0.0;
}

- (void)showPanel:(NSWindow *)panel animation:(const SkinAnimation &)animation rows:(NSArray<NSView *> *)rows
{
    const BOOL wasVisible = panel.isVisible;
    const CFTimeInterval now = CACurrentMediaTime();
    NSView *content = panel.contentView;
    if (!wasVisible) [self endEnter];
    // 正在退场时重新显示、或者刚收起又出现（同一段连续输入），都直接以完整不透明度显示。
    const BOOL enter = !wasVisible && [self allowsMotion:animation] && animation.enter != SkinAnimationPreset::none && !_exiting &&
                       now - _lastHide >= kTypingBurstInterval && content.layer != nil;
    if (enter && panel.hasShadow)
    {
        panel.hasShadow = NO;
        _shadowPanel = panel;
    }
    panel.animationBehavior = NSWindowAnimationBehaviorNone;
    panel.alphaValue = 1.0;
    [panel orderFrontRegardless];
    // 先摆上面板再撤退场窗，中间不留两边都不在屏幕上的一帧。
    [self finishExit];
    if (wasVisible)
    {
        // 进场没播完就重排（打字时每个键都重排）：接着同一条时间线播完，既不从头再来，也不突然跳到不透明。InputController 每次换一个新内容视图，卡片要重新加；CandidatePanel 在同一张卡片里重建候选行，只给新行补上错峰。
        if (_enterPanel == panel && now < _enterEnd && !_reduceMotion)
        {
            if (content != _enterContent) [self addCardEnter:_enterAnimation content:content start:_enterStart];
            _enterContent = content;
            const CFTimeInterval end = [self addRowReveals:_enterAnimation rows:rows start:_enterStart];
            if (end > _enterEnd)
            {
                _enterEnd = end;
                [self scheduleEndEnter];
            }
        }
        return;
    }
    if (!enter) return;
    _enterAnimation = animation;
    _enterContent = content;
    _enterPanel = panel;
    _enterStart = now;
    [self addCardEnter:animation content:content start:now];
    _enterEnd = std::max(now + MotionSeconds(animation.durationMs), [self addRowReveals:animation rows:rows start:now]);
    [self scheduleEndEnter];
}

// 到 _enterEnd 时结束进场；之前安排的结束因令牌变了而作废。
- (void)scheduleEndEnter
{
    const uint64_t token = ++_enterToken;
    const CFTimeInterval delay = std::max(0.0, _enterEnd - CACurrentMediaTime());
    __weak MSIMECandidateMotion *weakSelf = self;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        MSIMECandidateMotion *motion = weakSelf;
        if (motion != nil && motion->_enterToken == token) [motion endEnter];
    });
}

// 进场加在卡片（内容视图的图层）上，从 start 开始计时。模型值保持不透明、不变形，所以撤掉动画总是回到完整显示。
- (void)addCardEnter:(const SkinAnimation &)animation content:(NSView *)content start:(CFTimeInterval)start
{
    CALayer *layer = content.layer;
    if (layer == nil) return;
    const CFTimeInterval duration = MotionSeconds(animation.durationMs);
    NSMutableArray<CAAnimation *> *card = [NSMutableArray array];
    CAAnimation *transform = nil;
    switch (animation.enter)
    {
    case SkinAnimationPreset::none:
        return;
    case SkinAnimationPreset::fade:
        [card addObject:Basic(@"opacity", @0.0, @1.0, duration, Curve(kCAMediaTimingFunctionEaseOut))];
        break;
    case SkinAnimationPreset::fade_scale:
        [card addObject:Basic(@"opacity", @0.0, @1.0, duration, SettleCurve())];
        transform = Basic(@"transform", TransformValue(CenteredTransform(layer, 0.96, 0.0)), TransformValue(CATransform3DIdentity), duration,
                          SettleCurve());
        break;
    case SkinAnimationPreset::slide_up:
        [card addObject:Basic(@"opacity", @0.0, @1.0, duration, SettleCurve())];
        transform = Basic(@"transform", TransformValue(CenteredTransform(layer, 1.0, -kSlideDistance)), TransformValue(CATransform3DIdentity),
                          duration, SettleCurve());
        break;
    case SkinAnimationPreset::spring:
        // 卡片和窗口一样大，越过 1 的部分会被窗口边缘裁掉，所以卡片的回弹只有 Windows 那样的千分之几；明显的回弹留给选中的那一行。
        [card addObject:Basic(@"opacity", @0.0, @1.0, duration, Curve(kCAMediaTimingFunctionEaseOut))];
        transform = Keyframes(@"transform",
                              @[ TransformValue(CenteredTransform(layer, 0.96, 0.0)), TransformValue(CenteredTransform(layer, 1.004, 0.0)),
                                 TransformValue(CATransform3DIdentity) ],
                              @[ @0.0, @0.7, @1.0 ], duration);
        break;
    case SkinAnimationPreset::glow:
        [card addObject:Basic(@"opacity", @0.82, @1.0, duration, Curve(kCAMediaTimingFunctionEaseOut))];
        break;
    }
    if (transform != nil) [card addObject:transform];
    const CFTimeInterval begin = [layer convertTime:start fromLayer:nil];
    for (CAAnimation *item in card)
    {
        item.beginTime = begin;
        item.fillMode = kCAFillModeBackwards;
        [layer addAnimation:item forKey:item == transform ? kEnterTransformKey : kEnterOpacityKey];
    }
}

// 候选行逐行错峰浮现，第 i 行晚 min(i × stagger, kStaggerCap) 开始；已经在浮现的行不重加。返回最后一行播完的时间，没有错峰时返回 start。
- (CFTimeInterval)addRowReveals:(const SkinAnimation &)animation rows:(NSArray<NSView *> *)rows start:(CFTimeInterval)start
{
    if (!StaggersRows(animation.enter)) return start;
    AttachRowLayers(rows);
    const CFTimeInterval duration = MotionSeconds(animation.durationMs);
    const CFTimeInterval step = animation.staggerMs / 1000.0;
    CFTimeInterval end = start;
    NSUInteger index = 0;
    for (NSView *row in rows)
    {
        CALayer *rowLayer = row.layer;
        const CFTimeInterval delay = std::min(index++ * step, kStaggerCap);
        if (rowLayer == nil || [rowLayer animationForKey:kRowRevealKey] != nil) continue;
        end = std::max(end, start + delay + duration);
        CABasicAnimation *opacity = Basic(@"opacity", @0.0, @1.0, duration, SettleCurve());
        CABasicAnimation *lift = Basic(@"transform", TransformValue(CenteredTransform(rowLayer, 1.0, -kRowRevealDistance)),
                                       TransformValue(CATransform3DIdentity), duration, SettleCurve());
        CAAnimationGroup *reveal = [CAAnimationGroup animation];
        reveal.animations = @[ opacity, lift ];
        reveal.duration = duration;
        reveal.beginTime = [rowLayer convertTime:start + delay fromLayer:nil];
        reveal.fillMode = kCAFillModeBackwards;
        [rowLayer addAnimation:reveal forKey:kRowRevealKey];
    }
    return end;
}

// 撤掉进场动画、还原窗口阴影。内容视图之后可能移进退场窗，所以连同各行一起撤。
- (void)endEnter
{
    ++_enterToken;
    NSView *content = _enterContent;
    [content.layer removeAnimationForKey:kEnterOpacityKey];
    [content.layer removeAnimationForKey:kEnterTransformKey];
    for (NSView *row in content.subviews)
        [row.layer removeAnimationForKey:kRowRevealKey];
    _enterContent = nil;
    _enterPanel = nil;
    _enterEnd = 0.0;
    NSWindow *shadowPanel = _shadowPanel;
    if (shadowPanel != nil)
    {
        shadowPanel.hasShadow = YES;
        [shadowPanel invalidateShadow];
    }
    _shadowPanel = nil;
}

- (void)hidePanel:(NSWindow *)panel animation:(const SkinAnimation &)animation allowExit:(BOOL)allowExit
{
    const BOOL wasVisible = panel.isVisible;
    [self endSelection];
    NSView *content = panel.contentView;
    // 退场从卡片此刻的样子开始，进场播到一半时不会先跳回完全不透明。
    CALayer *shown = content.layer.presentationLayer ?: content.layer;
    const float opacity = shown != nil ? shown.opacity : 1.0f;
    const CATransform3D transform = shown != nil ? shown.transform : CATransform3DIdentity;
    [self endEnter];
    if (!wasVisible)
    {
        // 已经收起时再收一次（比如上屏后又一个没有候选的键）不重新开始退场；隐私原因的收起则连退场也立即撤掉。
        [panel orderOut:nil];
        if (!allowExit) [self finishExit];
        return;
    }
    _lastHide = CACurrentMediaTime();
    [self finishExit];
    if (!allowExit || ![self allowsMotion:animation] || animation.exit == SkinAnimationPreset::none || content == nil ||
        content.layer == nil)
    {
        [panel orderOut:nil];
        return;
    }
    NSWindow *exitWindow = [self exitWindowLike:panel];
    [exitWindow setFrame:panel.frame display:NO];
    panel.contentView = [[NSView alloc] initWithFrame:content.frame];
    exitWindow.contentView = content;
    [exitWindow orderFrontRegardless];
    [panel orderOut:nil];
    _exiting = YES;
    CALayer *layer = content.layer;
    const CFTimeInterval duration = MotionSeconds(animation.durationMs);
    CAMediaTimingFunction *curve = Curve(kCAMediaTimingFunctionEaseIn);
    NSMutableDictionary<NSString *, CABasicAnimation *> *exit = [NSMutableDictionary dictionary];
    switch (animation.exit)
    {
    case SkinAnimationPreset::none:
        break;
    case SkinAnimationPreset::fade:
        exit[kExitOpacityKey] = Basic(@"opacity", @(opacity), @0.0, duration, curve);
        break;
    case SkinAnimationPreset::fade_scale:
    case SkinAnimationPreset::spring:
        exit[kExitOpacityKey] = Basic(@"opacity", @(opacity), @0.0, duration, curve);
        exit[kExitTransformKey] = Basic(@"transform", TransformValue(transform), TransformValue(CenteredTransform(layer, 0.96, 0.0)), duration, curve);
        break;
    case SkinAnimationPreset::slide_up:
        // 进场的倒放：向下沉出，与 Web 候选栏相同。
        exit[kExitOpacityKey] = Basic(@"opacity", @(opacity), @0.0, duration, curve);
        exit[kExitTransformKey] =
            Basic(@"transform", TransformValue(transform), TransformValue(CenteredTransform(layer, 1.0, -kSlideDistance)), duration, curve);
        break;
    case SkinAnimationPreset::glow:
        // 只暗到 0.82，结束时整张收起，与 Windows 和 Web 相同。
        exit[kExitOpacityKey] = Basic(@"opacity", @(opacity), @0.82, duration, curve);
        break;
    }
    for (NSString *key in exit)
    {
        CABasicAnimation *item = exit[key];
        item.fillMode = kCAFillModeForwards;
        item.removedOnCompletion = NO;
        [layer addAnimation:item forKey:key];
    }
    const uint64_t token = _exitToken;
    __weak MSIMECandidateMotion *weakSelf = self;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(duration * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        MSIMECandidateMotion *motion = weakSelf;
        // 令牌对不上说明这之后面板又显示或又收起过，这次完成与当前画面无关。
        if (motion != nil && motion->_exitToken == token) [motion finishExit];
    });
}

- (NSWindow *)exitWindowLike:(NSWindow *)panel
{
    if (_exitWindow == nil)
    {
        _exitWindow = [[MSIMECandidateExitWindow alloc] initWithContentRect:NSZeroRect
                                                                  styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                                    backing:NSBackingStoreBuffered
                                                                      defer:YES];
        _exitWindow.releasedWhenClosed = NO;
        _exitWindow.hidesOnDeactivate = NO;
        _exitWindow.opaque = NO;
        _exitWindow.backgroundColor = NSColor.clearColor;
        // 窗口阴影不跟着图层淡出，留着它会在卡片消失后多挂一个空阴影。
        _exitWindow.hasShadow = NO;
        _exitWindow.ignoresMouseEvents = YES;
        _exitWindow.animationBehavior = NSWindowAnimationBehaviorNone;
        _exitWindow.accessibilityElement = NO;
    }
    _exitWindow.level = panel.level;
    _exitWindow.collectionBehavior = panel.collectionBehavior;
    _exitWindow.appearance = panel.appearance;
    return _exitWindow;
}

// 撤掉退场窗并放掉旧的候选视图。之后任何迟到的完成回调都因令牌变了而不起作用。
- (void)finishExit
{
    ++_exitToken;
    if (!_exiting) return;
    _exiting = NO;
    [_exitWindow orderOut:nil];
    _exitWindow.contentView = [[NSView alloc] initWithFrame:NSZeroRect];
}

- (void)decorateSelectionOfRow:(NSView<MSIMECandidateSelectionFillHost> *)row
                          fill:(NSColor *)fill
                  cornerRadius:(CGFloat)cornerRadius
                        accent:(NSColor *)accent
                     animation:(const SkinAnimation &)animation
{
    [self endSelection];
    if (![self allowsMotion:animation] || animation.selection == SkinAnimationPreset::none || fill == nil || fill.alphaComponent <= 0.01)
        return;
    // InputController 换高亮时整页重建，新行这时还没有挂上图层。
    AttachRowLayers(@[ row ]);
    CALayer *rowLayer = row.layer;
    CALayer *parent = rowLayer.superlayer;
    if (rowLayer == nil || parent == nil || parent != row.superview.layer) return;
    const CFTimeInterval duration = MotionSeconds(animation.durationMs);
    CALayer *layer = [CALayer layer];
    layer.name = kSelectionKey;
    NSColor *ring = accent ?: fill;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    // 与行自己画底色的位置一致：行边界内缩 1 点的圆角矩形。
    layer.frame = CGRectInset(rowLayer.frame, 1.0, 1.0);
    layer.cornerRadius = MAX(0.0, cornerRadius);
    layer.backgroundColor = fill.CGColor;
    if (animation.selection == SkinAnimationPreset::glow)
    {
        layer.borderWidth = 2.0;
        layer.borderColor = [ring colorWithAlphaComponent:0.0].CGColor;
    }
    [parent insertSublayer:layer below:rowLayer];
    [CATransaction commit];
    CAAnimation *decoration = nil;
    switch (animation.selection)
    {
    case SkinAnimationPreset::none:
        break;
    case SkinAnimationPreset::fade:
    case SkinAnimationPreset::fade_scale:
    case SkinAnimationPreset::slide_up:
        // 只有底色从 0.45 淡入，文字一开始就是完整的，与 Windows 只动高亮底色相同。
        decoration = Basic(@"opacity", @0.45, @1.0, duration, Curve(kCAMediaTimingFunctionEaseOut));
        break;
    case SkinAnimationPreset::spring:
        // 这是自己建的图层，锚点就在中心，直接缩放。
        decoration = Keyframes(@"transform.scale", @[ @0.94, @1.025, @1.0 ], @[ @0.0, @0.65, @1.0 ], duration);
        break;
    case SkinAnimationPreset::glow:
        // 底色照常，外加一圈强调色描边从 0.65 淡到 0，与 Windows 相同。
        decoration = Basic(@"borderColor", (__bridge id)[ring colorWithAlphaComponent:ring.alphaComponent * 0.65].CGColor,
                           (__bridge id)[ring colorWithAlphaComponent:0.0].CGColor, duration, Curve(kCAMediaTimingFunctionEaseOut));
        break;
    }
    if (decoration != nil) [layer addAnimation:decoration forKey:kSelectionKey];
    row.selectionFillDetached = YES;
    row.needsDisplay = YES;
    _selectionLayer = layer;
    _selectionRow = row;
    const uint64_t token = ++_selectionToken;
    __weak MSIMECandidateMotion *weakSelf = self;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(duration * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        MSIMECandidateMotion *motion = weakSelf;
        if (motion != nil && motion->_selectionToken == token) [motion endSelection];
    });
}

- (void)endSelection
{
    ++_selectionToken;
    NSView<MSIMECandidateSelectionFillHost> *row = _selectionRow;
    if (row != nil)
    {
        row.selectionFillDetached = NO;
        row.needsDisplay = YES;
    }
    if (_selectionLayer != nil)
    {
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        [_selectionLayer removeFromSuperlayer];
        [CATransaction commit];
    }
    _selectionLayer = nil;
    _selectionRow = nil;
}

- (void)stop
{
    [self endSelection];
    [self endEnter];
    [self finishExit];
}
@end
