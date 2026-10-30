#import "CandidateMotion.h"

#import <ImageIO/ImageIO.h>
#import <QuartzCore/QuartzCore.h>

#include <algorithm>
#include <cmath>
#include <vector>

#include "../../../../shared/candidate/CandidateMotionRules.h"

#if !__has_feature(objc_arc)
#error "CandidateMotion.mm must be compiled with -fobjc-arc"
#endif

using msime::mac::DecorationMotion;
using msime::mac::SkinAnimation;
using msime::mac::SkinAnimationPreset;

namespace
{
// 时长和幅度与 Linux 的 X11 原生画布共用 shared/candidate/CandidateMotionRules.h，含义见那里。
constexpr CFTimeInterval kTypingBurstInterval = msime::candidate::kMotionTypingBurstSeconds;
constexpr CFTimeInterval kStaggerCap = msime::candidate::kMotionStaggerCapSeconds;
constexpr CGFloat kSlideDistance = msime::candidate::kMotionSlideDistance;
constexpr CGFloat kRowRevealDistance = msime::candidate::kMotionRowRevealDistance;

NSString *const kEnterOpacityKey = @"msime-candidate-enter-opacity";
NSString *const kEnterTransformKey = @"msime-candidate-enter-transform";
NSString *const kRowRevealKey = @"msime-candidate-row-reveal";
NSString *const kExitOpacityKey = @"msime-candidate-exit-opacity";
NSString *const kExitTransformKey = @"msime-candidate-exit-transform";
NSString *const kSelectionKey = @"msime-candidate-selection";
NSString *const kDecorationMotionKey = @"msime-decoration-motion";
NSString *const kDecorationFramesKey = @"msime-decoration-frames";

constexpr CGFloat kFloatDistance = msime::candidate::kMotionFloatDistance;
constexpr CGFloat kSwayRadians = msime::candidate::kMotionSwayRadians;
constexpr CGFloat kBreatheScale = msime::candidate::kMotionBreatheScale;
constexpr CGFloat kBounceHeight = msime::candidate::kMotionBounceHeight;
// 与共享加载器的 MAX_ANIMATION_FRAMES 一致；加载器已经拒绝了更多帧的包，这里只是不信任文件。
constexpr size_t kMaxDecorationFrames = 120;

// 与共享契约一致的 40–600ms。
CFTimeInterval MotionSeconds(double milliseconds)
{
    return msime::candidate::MotionSeconds(milliseconds);
}

CAMediaTimingFunction *Curve(NSString *name)
{
    return [CAMediaTimingFunction functionWithName:name];
}

// 进场（卡片和逐行浮现）用标准 ease-out。Web 候选栏的 cubic-bezier(.16, 1, .3, 1) 在 200ms 里第一帧就走完 44%、第三帧走完 83%，候选窗上浮和淡入几乎在三帧内结束，用户在真机上看到的是「没有动画」；ease-out 在第二帧约 26%、五帧约 59%、170ms 到 96%，过程看得见又不拖尾。时长仍是皮肤写的，连续输入照样跳过进场。
CAMediaTimingFunction *SettleCurve()
{
    return [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseOut];
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

// 以图片底边中点为轴缩放 scale、旋转 angle 弧度，再在屏幕方向上抬起 lift 点。轴心和抬起的方向都看父图层是否翻转，与 CenteredTransform 相同。
CATransform3D BottomCenterTransform(CALayer *layer, CGFloat scale, CGFloat angle, CGFloat lift)
{
    const CGRect bounds = layer.bounds;
    const BOOL flipped = layer.superlayer.contentsAreFlipped;
    const CGFloat anchorX = CGRectGetMinX(bounds) + layer.anchorPoint.x * CGRectGetWidth(bounds);
    const CGFloat anchorY = CGRectGetMinY(bounds) + layer.anchorPoint.y * CGRectGetHeight(bounds);
    const CGFloat pivotX = CGRectGetMidX(bounds) - anchorX;
    const CGFloat pivotY = (flipped ? CGRectGetMaxY(bounds) : CGRectGetMinY(bounds)) - anchorY;
    CATransform3D transform = CATransform3DRotate(CATransform3DMakeScale(scale, scale, 1.0), angle, 0.0, 0.0, 1.0);
    transform = CATransform3DConcat(CATransform3DMakeTranslation(-pivotX, -pivotY, 0.0), transform);
    return CATransform3DConcat(transform, CATransform3DMakeTranslation(pivotX, pivotY + (flipped ? -lift : lift), 0.0));
}

// 一个周期的待机动效，从原位开始、回到原位，无限重复。幅度按图片大小收小：图片四周只留了 kDecorationMotionMargin 点，摇摆时顶角的横移约为 高×角度、上抬约为 半宽×角度，呼吸时顶边上抬 高×(倍数−1)、两侧外扩 半宽×(倍数−1)，都不超过它。
CAKeyframeAnimation *DecorationMotionAnimation(CALayer *layer, DecorationMotion motion, CFTimeInterval period)
{
    const CGFloat margin = msime::mac::kDecorationMotionMargin;
    const CGFloat width = MAX(1.0, CGRectGetWidth(layer.bounds));
    const CGFloat height = MAX(1.0, CGRectGetHeight(layer.bounds));
    NSArray<NSValue *> *values = nil;
    NSArray<NSNumber *> *times = @[ @0.0, @0.25, @0.5, @0.75, @1.0 ];
    CAMediaTimingFunction *smooth = Curve(kCAMediaTimingFunctionEaseInEaseOut);
    NSArray<CAMediaTimingFunction *> *curves = @[ smooth, smooth, smooth, smooth ];
    switch (motion)
    {
    case DecorationMotion::none:
        return nil;
    case DecorationMotion::floating:
    {
        const CGFloat distance = MIN(kFloatDistance, margin);
        values = @[
            TransformValue(CATransform3DIdentity), TransformValue(BottomCenterTransform(layer, 1.0, 0.0, distance)),
            TransformValue(CATransform3DIdentity), TransformValue(BottomCenterTransform(layer, 1.0, 0.0, -distance)),
            TransformValue(CATransform3DIdentity)
        ];
        break;
    }
    case DecorationMotion::sway:
    {
        const CGFloat angle = MIN(kSwayRadians, margin / MAX(height, width / 2.0));
        values = @[
            TransformValue(CATransform3DIdentity), TransformValue(BottomCenterTransform(layer, 1.0, angle, 0.0)),
            TransformValue(CATransform3DIdentity), TransformValue(BottomCenterTransform(layer, 1.0, -angle, 0.0)),
            TransformValue(CATransform3DIdentity)
        ];
        break;
    }
    case DecorationMotion::breathe:
    {
        const CGFloat scale = 1.0 + MIN(kBreatheScale - 1.0, MIN(margin / height, 2.0 * margin / width));
        values = @[
            TransformValue(CATransform3DIdentity), TransformValue(BottomCenterTransform(layer, scale, 0.0, 0.0)),
            TransformValue(CATransform3DIdentity)
        ];
        times = @[ @0.0, @0.5, @1.0 ];
        curves = @[ smooth, smooth ];
        break;
    }
    case DecorationMotion::bounce:
    {
        // 大半个周期站着不动，然后起跳、落下：上升减速、下落加速，像受重力。
        const CGFloat hop = MIN(kBounceHeight, margin);
        values = @[
            TransformValue(CATransform3DIdentity), TransformValue(CATransform3DIdentity),
            TransformValue(BottomCenterTransform(layer, 1.0, 0.0, hop)), TransformValue(CATransform3DIdentity),
            TransformValue(CATransform3DIdentity)
        ];
        times = @[ @0.0, @0.6, @0.75, @0.9, @1.0 ];
        curves = @[
            Curve(kCAMediaTimingFunctionLinear), Curve(kCAMediaTimingFunctionEaseOut), Curve(kCAMediaTimingFunctionEaseIn),
            Curve(kCAMediaTimingFunctionLinear)
        ];
        break;
    }
    }
    CAKeyframeAnimation *animation = [CAKeyframeAnimation animationWithKeyPath:@"transform"];
    animation.values = values;
    animation.keyTimes = times;
    animation.timingFunctions = curves;
    animation.duration = period;
    animation.repeatCount = HUGE_VALF;
    return animation;
}

// 只有 fade_scale 和 slide_up 逐行错峰，与 Web 候选栏相同。
bool StaggersRows(SkinAnimationPreset preset)
{
    return preset == SkinAnimationPreset::fade_scale || preset == SkinAnimationPreset::slide_up;
}
} // namespace

@implementation MSIMEDecorationFrames
{
    NSArray *_images;
    NSArray<NSNumber *> *_keyTimes;
    CFTimeInterval _duration;
    NSUInteger _loopCount;
}

+ (instancetype)framesWithContentsOfFile:(NSString *)path
{
    if (path.length == 0) return nil;
    CGImageSourceRef source = CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:path], nullptr);
    if (source == nullptr) return nil;
    MSIMEDecorationFrames *frames = [[MSIMEDecorationFrames alloc] initWithSource:source];
    CFRelease(source);
    return frames;
}

- (instancetype)initWithSource:(CGImageSourceRef)source
{
    const size_t count = std::min(CGImageSourceGetCount(source), kMaxDecorationFrames);
    if (count < 2) return nil;
    self = [super init];
    if (self == nil) return nil;
    NSDictionary *container = CFBridgingRelease(CGImageSourceCopyProperties(source, nullptr));
    NSNumber *loops = container[(__bridge NSString *)kCGImagePropertyPNGDictionary][(__bridge NSString *)kCGImagePropertyAPNGLoopCount];
    _loopCount = [loops isKindOfClass:NSNumber.class] ? loops.unsignedIntegerValue : 0;
    // 先解码好：播放时 Core Animation 直接用这些位图，不在打字的那一帧里解码。
    NSDictionary *decode = @{(__bridge NSString *)kCGImageSourceShouldCacheImmediately : @YES};
    NSMutableArray *images = [NSMutableArray arrayWithCapacity:count];
    std::vector<CFTimeInterval> delays;
    delays.reserve(count);
    for (size_t index = 0; index < count; ++index)
    {
        CGImageRef image = CGImageSourceCreateImageAtIndex(source, index, (__bridge CFDictionaryRef)decode);
        if (image == nullptr) return nil;
        [images addObject:CFBridgingRelease(image)];
        NSDictionary *properties = CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(source, index, nullptr));
        NSNumber *delay = properties[(__bridge NSString *)kCGImagePropertyPNGDictionary][(__bridge NSString *)kCGImagePropertyAPNGDelayTime];
        // ImageIO 给的是按浏览器规则修正过的帧时长；读不到或为 0 时按 0.1 秒，与浏览器相同。
        const double seconds = [delay isKindOfClass:NSNumber.class] ? delay.doubleValue : 0.0;
        delays.push_back(seconds > 0.0 && std::isfinite(seconds) ? seconds : 0.1);
    }
    CFTimeInterval total = 0.0;
    for (const CFTimeInterval delay : delays) total += delay;
    NSMutableArray<NSNumber *> *keyTimes = [NSMutableArray arrayWithCapacity:count + 1];
    CFTimeInterval elapsed = 0.0;
    for (const CFTimeInterval delay : delays)
    {
        [keyTimes addObject:@(elapsed / total)];
        elapsed += delay;
    }
    [keyTimes addObject:@1.0];
    _images = images;
    _keyTimes = keyTimes;
    _duration = total;
    return self;
}

- (NSArray *)images
{
    return _images;
}

- (NSArray<NSNumber *> *)keyTimes
{
    return _keyTimes;
}

- (CFTimeInterval)duration
{
    return _duration;
}

- (NSUInteger)loopCount
{
    return _loopCount;
}
@end

@interface MSIMEDecorationWordView : NSView
@property(nonatomic, copy) NSAttributedString *text;
@end
@implementation MSIMEDecorationWordView
- (BOOL)isFlipped { return YES; }
- (BOOL)isOpaque { return NO; }
- (NSView *)hitTest:(NSPoint)point { (void)point; return nil; }
- (void)drawRect:(NSRect)dirtyRect { (void)dirtyRect; [self.text drawAtPoint:NSZeroPoint]; }
@end

@implementation MSIMECandidateDecorationView
{
    MSIMEDecorationWordView *_candidateWord;
}

- (void)setCandidateWord:(NSString *)word style:(const std::optional<msime::candidate::DecorationCandidate> &)style
                boxSize:(NSSize)boxSize alignment:(msime::mac::DecorationAlign)alignment font:(NSFont *)font
{
    if (!style || word.length == 0 || boxSize.width <= 0 || boxSize.height <= 0) {
        [_candidateWord removeFromSuperview];
        _candidateWord = nil;
        return;
    }
    const auto &value = *style;
    NSFont *initial = [NSFont fontWithDescriptor:font.fontDescriptor size:value.fontSizeDip];
    const CGFloat naturalWidth = [word sizeWithAttributes:@{NSFontAttributeName: initial}].width;
    NSFont *fitted = [NSFont fontWithDescriptor:font.fontDescriptor size:value.fittedFont(naturalWidth, boxSize.width)];
    const auto color = msime::mac::ParseCssColor(value.color).value_or(msime::mac::Rgba{1, 1, 1, 1});
    NSAttributedString *text = [[NSAttributedString alloc] initWithString:word attributes:@{
        NSFontAttributeName: fitted,
        NSForegroundColorAttributeName: [NSColor colorWithSRGBRed:color.r green:color.g blue:color.b alpha:color.a]
    }];
    const NSSize measured = text.size;
    if (!_candidateWord) {
        _candidateWord = [[MSIMEDecorationWordView alloc] initWithFrame:NSZeroRect];
        _candidateWord.identifier = @"candidate-decoration-word";
        _candidateWord.accessibilityElement = NO;
        [self addSubview:_candidateWord];
    }
    // 图像可能被等比缩小，文字仍相对声明的装饰框定位；超框的部分不裁切。
    const CGFloat originX = alignment == msime::mac::DecorationAlign::left ? 0
        : (NSWidth(self.bounds) - boxSize.width) * (alignment == msime::mac::DecorationAlign::center ? 0.5 : 1);
    _candidateWord.frame = NSMakeRect(originX + value.left(boxSize.width, measured.width),
        boxSize.height - value.top(boxSize.height, measured.height) - measured.height,
        std::ceil(measured.width), std::ceil(measured.height));
    _candidateWord.text = text;
    _candidateWord.needsDisplay = YES;
}

- (instancetype)initWithFrame:(NSRect)frameRect
{
    self = [super initWithFrame:frameRect];
    if (self)
    {
        self.wantsLayer = YES;
        self.layerContentsRedrawPolicy = NSViewLayerContentsRedrawOnSetNeedsDisplay;
        self.clipsToBounds = NO;
    }
    return self;
}

- (BOOL)wantsUpdateLayer
{
    return YES;
}

- (BOOL)isOpaque
{
    return NO;
}

// 装饰图压在卡片顶边和首行上，点击要落到下面的候选和卡片。
- (NSView *)hitTest:(NSPoint)point
{
    (void)point;
    return nil;
}

- (void)setImage:(NSImage *)image
{
    _image = image;
    [self applyContents];
}

- (void)setFrames:(MSIMEDecorationFrames *)frames
{
    _frames = frames;
    [self applyContents];
}

- (void)viewDidChangeBackingProperties
{
    [super viewDidChangeBackingProperties];
    [self applyContents];
}

- (void)updateLayer
{
    [self applyContents];
}

// 图层内容的模型值：APNG 是第一帧，静态图按屏幕倍率取最合适的那一份。动画撤掉后显示的就是它。
- (void)applyContents
{
    CALayer *layer = self.layer;
    if (layer == nil) return;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    layer.contentsGravity = kCAGravityResize;
    if (_frames.images.count > 0)
    {
        layer.contents = _frames.images.firstObject;
    }
    else if (_image != nil)
    {
        const CGFloat backing = self.window.backingScaleFactor > 0.0 ? self.window.backingScaleFactor : NSScreen.mainScreen.backingScaleFactor;
        const CGFloat scale = [_image recommendedLayerContentsScale:backing];
        layer.contentsScale = scale;
        layer.contents = [_image layerContentsForContentsScale:scale];
    }
    else
    {
        layer.contents = nil;
    }
    [CATransaction commit];
}
@end

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
    NSString *_lastDecision;
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
    // 正在动的装饰图和它的动效。InputController 每次重排都换一个新的装饰视图，新视图接着 _decorationAnchor 这条时间线。
    __weak MSIMECandidateDecorationView *_decoration;
    DecorationMotion _decorationMotion;
    CFTimeInterval _decorationPeriod;
    CFTimeInterval _decorationAnchor;
    // 屏幕睡眠、锁屏或切走用户时为 YES，装饰图不动。
    BOOL _screensAsleep;
}

- (instancetype)init
{
    self = [super init];
    if (self)
    {
        _lastHide = -INFINITY;
        _reduceMotion = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion;
        NSNotificationCenter *workspace = NSWorkspace.sharedWorkspace.notificationCenter;
        [workspace addObserver:self
                      selector:@selector(accessibilityDisplayOptionsDidChange:)
                          name:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification
                        object:nil];
        // 屏幕睡了、锁屏或切到别的用户时装饰图停下，醒来后候选窗还在就接着动。
        for (NSString *name in @[ NSWorkspaceScreensDidSleepNotification, NSWorkspaceSessionDidResignActiveNotification ])
            [workspace addObserver:self selector:@selector(screensDidSleep:) name:name object:nil];
        for (NSString *name in @[ NSWorkspaceScreensDidWakeNotification, NSWorkspaceSessionDidBecomeActiveNotification ])
            [workspace addObserver:self selector:@selector(screensDidWake:) name:name object:nil];
        [NSDistributedNotificationCenter.defaultCenter addObserver:self
                                                          selector:@selector(screensDidSleep:)
                                                              name:@"com.apple.screenIsLocked"
                                                            object:nil];
        [NSDistributedNotificationCenter.defaultCenter addObserver:self
                                                          selector:@selector(screensDidWake:)
                                                              name:@"com.apple.screenIsUnlocked"
                                                            object:nil];
    }
    return self;
}

- (void)dealloc
{
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    [NSDistributedNotificationCenter.defaultCenter removeObserver:self];
    [_exitWindow orderOut:nil];
}

- (void)screensDidSleep:(NSNotification *)notification
{
    (void)notification;
    _screensAsleep = YES;
    [self stopDecoration];
}

- (void)screensDidWake:(NSNotification *)notification
{
    (void)notification;
    _screensAsleep = NO;
    [self attachDecoration];
}

- (void)accessibilityDisplayOptionsDidChange:(NSNotification *)notification
{
    (void)notification;
    self.reduceMotion = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion;
}

- (void)setReduceMotion:(BOOL)reduceMotion
{
    _reduceMotion = reduceMotion;
    if (reduceMotion)
        [self stop];
    else
        [self attachDecoration];
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

- (NSString *)lastDecision
{
    return _lastDecision;
}

- (BOOL)allowsMotion:(const SkinAnimation &)animation
{
    // 候选显隐与高亮立即呈现；装饰图待机和图片帧走独立的播放通道。
    (void)animation;
    return NO;
}

- (void)showPanel:(NSWindow *)panel animation:(const SkinAnimation &)animation rows:(NSArray<NSView *> *)rows
{
    const BOOL wasVisible = panel.isVisible;
    const CFTimeInterval now = CACurrentMediaTime();
    NSView *content = panel.contentView;
    if (!wasVisible) [self endEnter];
    // 停下来之后再出现，装饰图从原位开始动；同一段连续输入里收起又出现，接着原来的相位，不在每个词开头跳回原位。
    if (!wasVisible && (_decorationAnchor <= 0.0 || now - _lastHide >= kTypingBurstInterval)) _decorationAnchor = now;
    // 正在退场时重新显示、或者刚收起又出现（同一段连续输入），都直接以完整不透明度显示。
    const BOOL enter = !wasVisible && [self allowsMotion:animation] && animation.enter != SkinAnimationPreset::none && !_exiting &&
                       now - _lastHide >= kTypingBurstInterval && content.layer != nil;
    // 按上面的条件逐条给出没播的原因，顺序与判断一致。
    const char *reason = wasVisible                                  ? "visible"
                         : ![self allowsMotion:animation]             ? "instant"
                         : animation.durationMs <= 0.0               ? "no_duration"
                         : animation.enter == SkinAnimationPreset::none ? "enter_none"
                         : _exiting                                  ? "exiting"
                         : now - _lastHide < kTypingBurstInterval    ? "burst"
                         : content.layer == nil                      ? "no_layer"
                                                                     : "enter";
    _lastDecision = [NSString stringWithFormat:@"show=%s since_hide_ms=%.0f", reason,
                                               isfinite(_lastHide) ? (now - _lastHide) * 1000.0 : -1.0];
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
        id content = shadowPanel.contentView;
        shadowPanel.hasShadow = [content conformsToProtocol:@protocol(MSIMECandidateWindowShadowHost)]
            ? [(id<MSIMECandidateWindowShadowHost>)content nativeWindowShadowEnabled] : YES;
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
        _lastDecision = @"hide=hidden";
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
        _lastDecision = ![self allowsMotion:animation]              ? @"hide=instant"
                        : !allowExit                                 ? @"hide=immediate"
                        : _reduceMotion                            ? @"hide=reduce_motion"
                        : animation.durationMs <= 0.0              ? @"hide=no_duration"
                        : animation.exit == SkinAnimationPreset::none ? @"hide=exit_none"
                                                                   : @"hide=no_layer";
        // 候选窗不在屏幕上时装饰图不动。有退场时装饰图跟着卡片在退场窗里淡出，退场结束整张卡片随退场窗的内容一起放掉。
        [self stopDecoration];
        [panel orderOut:nil];
        return;
    }
    _lastDecision = @"hide=exit";
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
    CGPathRef path = [row respondsToSelector:@selector(createSelectionFillPath)] ? [row createSelectionFillPath] : nullptr;
    CAShapeLayer *shape = path ? [CAShapeLayer layer] : nil;
    CALayer *layer = shape ?: [CALayer layer];
    layer.name = kSelectionKey;
    NSColor *ring = accent ?: fill;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    // 静态与动画高亮都铺满候选项，自定义四角沿用行自己的路径与坐标方向。
    if (shape) {
        layer.frame = rowLayer.frame;
        layer.bounds = rowLayer.bounds;
        layer.geometryFlipped = rowLayer.geometryFlipped;
        shape.path = path;
        shape.fillColor = fill.CGColor;
        CGPathRelease(path);
    } else {
        layer.frame = rowLayer.frame;
        layer.cornerRadius = MAX(0.0, cornerRadius);
        layer.backgroundColor = fill.CGColor;
    }
    if (animation.selection == SkinAnimationPreset::glow)
    {
        if (shape) {
            shape.lineWidth = 2.0;
            shape.strokeColor = [ring colorWithAlphaComponent:0.0].CGColor;
        } else {
            layer.borderWidth = 2.0;
            layer.borderColor = [ring colorWithAlphaComponent:0.0].CGColor;
        }
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
        decoration = Basic(shape ? @"strokeColor" : @"borderColor", (__bridge id)[ring colorWithAlphaComponent:ring.alphaComponent * 0.65].CGColor,
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
    [self stopDecoration];
}

- (void)animateDecoration:(MSIMECandidateDecorationView *)decoration
                   motion:(DecorationMotion)motion
                 periodMs:(double)periodMs
{
    MSIMECandidateDecorationView *previous = _decoration;
    if (previous != decoration) [self stopDecoration];
    _decoration = decoration;
    _decorationMotion = motion;
    _decorationPeriod = std::clamp(periodMs, 1200.0, 8000.0) / 1000.0;
    [self attachDecoration];
}

// 按当前状态给装饰图挂上或撤掉动画。两种动画都从 _decorationAnchor 开始计时，重挂时动画对象换了，时间线不变，所以装饰图不会跳。
- (void)attachDecoration
{
    MSIMECandidateDecorationView *decoration = _decoration;
    CALayer *layer = decoration.layer;
    if (layer == nil) return;
    MSIMEDecorationFrames *frames = decoration.frames;
    const BOOL animated = frames.images.count > 1 && frames.duration > 0.0;
    if (!decoration.window.isVisible || _reduceMotion || _screensAsleep || (_decorationMotion == DecorationMotion::none && !animated))
    {
        [self stopDecoration];
        return;
    }
    if (_decorationAnchor <= 0.0) _decorationAnchor = CACurrentMediaTime();
    const CFTimeInterval begin = [layer convertTime:_decorationAnchor fromLayer:nil];
    if (CAKeyframeAnimation *motion = DecorationMotionAnimation(layer, _decorationMotion, _decorationPeriod))
    {
        motion.beginTime = begin;
        [layer addAnimation:motion forKey:kDecorationMotionKey];
    }
    else
    {
        [layer removeAnimationForKey:kDecorationMotionKey];
    }
    if (animated)
    {
        // 逐帧切换图层内容，帧时长和循环次数照文件；有限循环播完后停在最后一帧，与浏览器相同。
        CAKeyframeAnimation *playback = [CAKeyframeAnimation animationWithKeyPath:@"contents"];
        playback.values = frames.images;
        playback.keyTimes = frames.keyTimes;
        playback.calculationMode = kCAAnimationDiscrete;
        playback.duration = frames.duration;
        playback.repeatCount = frames.loopCount == 0 ? HUGE_VALF : static_cast<float>(frames.loopCount);
        playback.beginTime = begin;
        playback.fillMode = kCAFillModeForwards;
        playback.removedOnCompletion = NO;
        [layer addAnimation:playback forKey:kDecorationFramesKey];
    }
    else
    {
        [layer removeAnimationForKey:kDecorationFramesKey];
    }
}

- (void)stopDecoration
{
    CALayer *layer = _decoration.layer;
    [layer removeAnimationForKey:kDecorationMotionKey];
    [layer removeAnimationForKey:kDecorationFramesKey];
}

- (MSIMECandidateDecorationView *)animatedDecoration
{
    MSIMECandidateDecorationView *decoration = _decoration;
    CALayer *layer = decoration.layer;
    return [layer animationForKey:kDecorationMotionKey] != nil || [layer animationForKey:kDecorationFramesKey] != nil ? decoration : nil;
}

- (CFTimeInterval)decorationAnchor
{
    return _decorationAnchor;
}
@end
