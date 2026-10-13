#import "TypingEffectPanel.h"
#import "../candidate/CandidatePlacement.h"
#import "../candidate/CandidateWindowLevel.h"

#import <QuartzCore/QuartzCore.h>

#import <algorithm>
#import <cmath>

namespace {
constexpr uint32_t kComboMask = 0xFFFF;
constexpr uint32_t kStyleShift = 17;
constexpr uint32_t kStyleMask = 0x7;
constexpr NSUInteger kDefaultIntensity = 50;
// Half the side of the square around the caret the sparks fly in; a spark lives 0.45 s at up to 180 pt/s, so it fades out inside it.
constexpr CGFloat kSparkReach = 80.0;
constexpr CGFloat kScreenMargin = 4.0;
constexpr CGFloat kBadgeGap = 4.0;
constexpr CGFloat kBadgeFontSize = 12.0;
constexpr CGFloat kBadgeHorizontalInset = 7.0;
constexpr CGFloat kBadgeHeight = 20.0;
// One key's burst and one commit's: short enough that a typist's next key starts a fresh burst rather than a stream.
constexpr NSTimeInterval kKeyBurst = 0.06;
constexpr NSTimeInterval kCommitBurst = 0.1;
// When the panel settles after the last effect: the sparks' lifetime with a margin, longer while the badge shows so the count can be read.
constexpr NSTimeInterval kSparkSettle = 0.7;
constexpr NSTimeInterval kBadgeSettle = 1.2;
constexpr NSTimeInterval kKeyFlash = 0.15;
constexpr NSTimeInterval kCommitFlash = 0.25;
constexpr float kSparkLifetime = 0.45f;
NSString *const kFlashIdentifier = @"candidate-typing-flash";
NSString *const kSparkCell = @"spark";

CGFloat Clamp(CGFloat value, CGFloat minimum, CGFloat maximum) {
    if (maximum < minimum) return minimum;
    return value < minimum ? minimum : (value > maximum ? maximum : value);
}

BOOL UsableRect(NSRect rect) {
    return std::isfinite(NSMinX(rect)) && std::isfinite(NSMinY(rect)) && std::isfinite(NSMaxX(rect)) && std::isfinite(NSMaxY(rect)) &&
           NSHeight(rect) > 0.0;
}

// A soft white dot, tinted per cell by its colour. Drawn once for the process.
CGImageRef SparkImage(void) {
    static CGImageRef image;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        const size_t side = 16;
        CGColorSpaceRef space = CGColorSpaceCreateDeviceRGB();
        CGContextRef context = CGBitmapContextCreate(nullptr, side, side, 8, 0, space, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
        const CGFloat components[] = {1, 1, 1, 1, 1, 1, 1, 0};
        const CGFloat locations[] = {0, 1};
        CGGradientRef gradient = CGGradientCreateWithColorComponents(space, components, locations, 2);
        const CGPoint center = CGPointMake(side / 2.0, side / 2.0);
        CGContextDrawRadialGradient(context, gradient, center, 0, center, side / 2.0, 0);
        image = CGBitmapContextCreateImage(context);
        CGGradientRelease(gradient);
        CGContextRelease(context);
        CGColorSpaceRelease(space);
    });
    return image;
}

NSScreen *ScreenContaining(NSPoint point) {
    return MSIMEScreenForPoint(point);
}
}

MSIMETypingEffect MSIMETypingEffectDecode(uint32_t packed) {
    MSIMETypingEffect effect;
    effect.combo = packed & kComboMask;
    effect.tierUp = (packed & MSIMETypingEffectTierUp) != 0;
    const uint32_t style = (packed >> kStyleShift) & kStyleMask;
    effect.style = style <= MSIMETypingEffectStylePowerMode ? (MSIMETypingEffectStyle)style : MSIMETypingEffectStyleOff;
    return effect;
}

MSIMETypingEffectStyle MSIMETypingEffectDrawnStyle(MSIMETypingEffectStyle style, BOOL reduceMotion, BOOL lowPower) {
    if ((style == MSIMETypingEffectStyleSparks || style == MSIMETypingEffectStylePowerMode) && (reduceMotion || lowPower))
        return MSIMETypingEffectStyleFlash;
    return style;
}

NSString *MSIMETypingEffectComboText(NSUInteger combo) {
    return combo >= 2 ? [NSString stringWithFormat:@"连击 ×%lu", (unsigned long)combo] : nil;
}

NSColor *MSIMETypingEffectColor(id value) {
    if (![value isKindOfClass:NSString.class] || [value length] != 7 || ![value hasPrefix:@"#"]) return nil;
    unsigned rgb = 0;
    for (NSUInteger index = 1; index < 7; ++index) {
        const unichar ch = [value characterAtIndex:index];
        unsigned digit = 0;
        if (ch >= '0' && ch <= '9') digit = ch - '0';
        else if (ch >= 'a' && ch <= 'f') digit = ch - 'a' + 10;
        else if (ch >= 'A' && ch <= 'F') digit = ch - 'A' + 10;
        else return nil;
        rgb = (rgb << 4) | digit;
    }
    return [NSColor colorWithSRGBRed:((rgb >> 16) & 0xFF) / 255.0 green:((rgb >> 8) & 0xFF) / 255.0 blue:(rgb & 0xFF) / 255.0 alpha:1.0];
}

@implementation MSIMETypingEffectFlashView
- (BOOL)isOpaque { return NO; }
- (NSView *)hitTest:(NSPoint)point {
    (void)point;
    return nil;
}
- (BOOL)isAccessibilityElement { return NO; }
@end

@implementation MSIMETypingEffectPanel {
    NSString *_style;
    BOOL _comboCounter;
    BOOL _packSelected;
    NSUInteger _intensity;
    NSArray<NSColor *> *_effectColors;
    NSTimeInterval _effectDuration;
    NSInteger _effectParticles;
    CALayer *_root;
    CAEmitterLayer *_emitter;
    CALayer *_caretFlash;
    CALayer *_badge;
    CATextLayer *_badgeText;
    NSTimer *_burstTimer;
    NSTimer *_settleTimer;
    __weak MSIMETypingEffectFlashView *_cardFlash;
    BOOL _emitting;
    MSIMETypingEffectStyle _drawnStyle;
}

+ (instancetype)sharedPanel {
    static MSIMETypingEffectPanel *panel;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ panel = [[self alloc] init]; });
    return panel;
}

- (instancetype)init {
    self = [super initWithContentRect:NSMakeRect(0, 0, 1, 1)
                              styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                backing:NSBackingStoreBuffered defer:YES];
    if (!self) return nil;
    self.floatingPanel = YES;
    // 紧贴在候选窗之下，火花不会盖住候选；也就和候选窗一起高过截图工具的遮罩（#7326）。
    self.level = MSIMECandidateWindowLevel() - 1;
    self.becomesKeyOnlyIfNeeded = YES;
    self.hidesOnDeactivate = NO;
    self.opaque = NO;
    self.backgroundColor = NSColor.clearColor;
    self.hasShadow = NO;
    self.ignoresMouseEvents = YES;
    self.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary |
                              NSWindowCollectionBehaviorTransient | NSWindowCollectionBehaviorIgnoresCycle;
    self.animationBehavior = NSWindowAnimationBehaviorNone;
    _style = @"off";
    _intensity = kDefaultIntensity;
    _effectColors = @[];
    _effectParticles = -1;

    // Layer hosting: the panel draws nothing through AppKit, only these layers.
    NSView *host = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 1, 1)];
    _root = [CALayer layer];
    host.layer = _root;
    host.wantsLayer = YES;

    _emitter = [CAEmitterLayer layer];
    _emitter.emitterShape = kCAEmitterLayerPoint;
    _emitter.renderMode = kCAEmitterLayerAdditive;
    _emitter.birthRate = 0;
    _emitter.hidden = YES;
    CAEmitterCell *spark = [CAEmitterCell emitterCell];
    spark.name = kSparkCell;
    spark.contents = (__bridge id)SparkImage();
    spark.lifetime = kSparkLifetime;
    spark.lifetimeRange = 0.15f;
    spark.velocity = 120;
    spark.velocityRange = 60;
    spark.emissionLongitude = M_PI_2;
    spark.emissionRange = M_PI * 0.4;
    spark.yAcceleration = -320;
    spark.scale = 0.45;
    spark.scaleRange = 0.2;
    spark.scaleSpeed = -0.6;
    spark.alphaSpeed = -1.8f;
    _emitter.emitterCells = @[spark];
    [_root addSublayer:_emitter];

    _caretFlash = [CALayer layer];
    _caretFlash.opacity = 0;
    _caretFlash.cornerRadius = 3;
    [_root addSublayer:_caretFlash];

    _badge = [CALayer layer];
    _badge.cornerRadius = kBadgeHeight / 2.0;
    _badge.hidden = YES;
    _badgeText = [CATextLayer layer];
    _badgeText.alignmentMode = kCAAlignmentCenter;
    _badgeText.fontSize = kBadgeFontSize;
    _badgeText.font = (__bridge CFTypeRef)[NSFont monospacedDigitSystemFontOfSize:kBadgeFontSize weight:NSFontWeightSemibold];
    _badgeText.foregroundColor = NSColor.whiteColor.CGColor;
    [_badge addSublayer:_badgeText];
    [_root addSublayer:_badge];

    self.contentView = host;
    return self;
}

- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }

- (void)applyPreferences:(NSDictionary *)preferences {
    if (![preferences isKindOfClass:NSDictionary.class]) return;
    id plugins = preferences[@"plugins"];
    if (![plugins isKindOfClass:NSDictionary.class]) return;
    id style = plugins[@"effect_style"];
    _style = [@[@"flash", @"sparks", @"power_mode"] containsObject:style] ? [style copy] : @"off";
    id pack = plugins[@"effect_pack"];
    _packSelected = [pack isKindOfClass:NSString.class] && [pack length] > 0;
    id counter = plugins[@"combo_counter"];
    _comboCounter = [counter isKindOfClass:NSNumber.class] && [counter boolValue];
    id intensity = plugins[@"effect_intensity"];
    _intensity = [intensity isKindOfClass:NSNumber.class] ? (NSUInteger)Clamp([intensity doubleValue], 0, 100) : kDefaultIntensity;
    if (!self.configured) [self settle];
}

- (void)applySettings:(NSDictionary *)settings {
    _effectColors = @[];
    _effectDuration = 0;
    _effectParticles = -1;
    if (![settings isKindOfClass:NSDictionary.class]) return;
    id intensity = settings[@"intensity"];
    if ([intensity isKindOfClass:NSNumber.class]) _intensity = (NSUInteger)Clamp([intensity doubleValue], 0, 100);
    NSMutableArray<NSColor *> *colors = [NSMutableArray array];
    id listed = settings[@"colors"];
    if ([listed isKindOfClass:NSArray.class])
        for (id value in listed)
            if (NSColor *color = MSIMETypingEffectColor(value)) [colors addObject:color];
    _effectColors = [colors copy];
    id duration = settings[@"duration_ms"];
    if ([duration isKindOfClass:NSNumber.class]) _effectDuration = Clamp([duration doubleValue], 60, 1500) / 1000.0;
    id particles = settings[@"particles"];
    if ([particles isKindOfClass:NSNumber.class]) _effectParticles = (NSInteger)Clamp([particles doubleValue], 0, 64);
}

- (BOOL)configured { return ![_style isEqualToString:@"off"] || _packSelected || _comboCounter; }
- (NSUInteger)intensity { return _intensity; }
- (NSArray<NSColor *> *)effectColors { return _effectColors; }
- (NSTimeInterval)effectDuration { return _effectDuration; }
- (NSInteger)effectParticles { return _effectParticles; }
- (BOOL)emitting { return _emitting; }
- (MSIMETypingEffectStyle)drawnStyle { return _drawnStyle; }
- (NSString *)displayedCombo { return self.isVisible && !_badge.hidden ? (NSString *)_badgeText.string : nil; }

- (void)presentEffect:(uint32_t)packed
               commit:(BOOL)commit
            caretRect:(NSRect)caretRect
        candidateView:(NSView *)candidateView
             cardRect:(NSRect)cardRect
         cornerRadius:(CGFloat)cornerRadius {
    [self presentEffect:packed
                 commit:commit
              caretRect:caretRect
          candidateView:candidateView
               cardRect:cardRect
           cornerRadius:cornerRadius
           reduceMotion:NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion
               lowPower:NSProcessInfo.processInfo.lowPowerModeEnabled];
}

- (void)presentEffect:(uint32_t)packed
               commit:(BOOL)commit
            caretRect:(NSRect)caretRect
        candidateView:(NSView *)candidateView
             cardRect:(NSRect)cardRect
         cornerRadius:(CGFloat)cornerRadius
         reduceMotion:(BOOL)reduceMotion
             lowPower:(BOOL)lowPower {
    const MSIMETypingEffect effect = MSIMETypingEffectDecode(packed);
    const MSIMETypingEffectStyle style = MSIMETypingEffectDrawnStyle(effect.style, reduceMotion, lowPower);
    NSString *combo = MSIMETypingEffectComboText(effect.combo);
    _drawnStyle = style;
    NSWindow *candidateWindow = candidateView.window;
    const BOOL card = candidateWindow != nil && candidateWindow.isVisible && UsableRect(cardRect);
    const BOOL caret = UsableRect(caretRect);
    if (style == MSIMETypingEffectStyleOff && combo == nil) {
        // A backspace or an idle pause ended the combo: take the badge down now rather than leave a stale count.
        if (!_badge.hidden) [self settle];
        return;
    }
    if (!card && !caret) return;

    const CGFloat strength = 0.4 + 0.6 * (CGFloat)_intensity / 100.0;
    NSRect cardScreen = NSZeroRect;
    if (card) cardScreen = [candidateWindow convertRectToScreen:[candidateView convertRect:cardRect toView:nil]];
    // Sparks rise from the caret; without a usable caret, from the card's top left.
    const NSPoint origin = caret ? NSMakePoint(NSMinX(caretRect), NSMidY(caretRect)) : NSMakePoint(NSMinX(cardScreen) + 12.0, NSMaxY(cardScreen));
    NSScreen *screen = ScreenContaining(origin);
    const NSRect visible = screen ? screen.frame : NSMakeRect(0, 0, 1440, 900);

    NSRect frame = NSMakeRect(origin.x - kSparkReach, origin.y - kSparkReach, 2.0 * kSparkReach, 2.0 * kSparkReach);
    NSRect badgeScreen = NSZeroRect;
    if (combo != nil) {
        NSFont *font = [NSFont monospacedDigitSystemFontOfSize:kBadgeFontSize weight:NSFontWeightSemibold];
        const CGFloat width = std::ceil([combo sizeWithAttributes:@{NSFontAttributeName : font}].width + 2.0 * kBadgeHorizontalInset);
        // Above the card's top right corner, outside the candidate window so it never hides a candidate; beside the caret without a card.
        const NSPoint anchor = card ? NSMakePoint(NSMaxX(candidateWindow.frame) - width, NSMaxY(candidateWindow.frame) + kBadgeGap)
                                    : NSMakePoint(NSMaxX(caretRect) + kBadgeGap, NSMaxY(caretRect) + kBadgeGap);
        badgeScreen = NSMakeRect(Clamp(anchor.x, NSMinX(visible) + kScreenMargin, NSMaxX(visible) - kScreenMargin - width),
                                 Clamp(anchor.y, NSMinY(visible) + kScreenMargin, NSMaxY(visible) - kScreenMargin - kBadgeHeight), width,
                                 kBadgeHeight);
        frame = NSUnionRect(frame, badgeScreen);
    }
    frame = NSIntegralRect(frame);

    [_burstTimer invalidate];
    [_settleTimer invalidate];
    [self setFrame:frame display:NO];
    const CGFloat scale = (screen ?: NSScreen.mainScreen).backingScaleFactor ?: 2.0;
    // The accent is a dynamic colour: resolve it in the panel's appearance, and hold the resolved colours for as long as their CGColors are used.
    __block NS_VALID_UNTIL_END_OF_SCOPE NSColor *accentColor = nil;
    // An effect pack's first colour takes the accent's place.
    if (_effectColors.count > 0) accentColor = _effectColors.firstObject;
    else
        [self.effectiveAppearance performAsCurrentDrawingAppearance:^{
            accentColor = [NSColor.controlAccentColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace] ?: NSColor.systemBlueColor;
        }];
    // The sparks of one burst take the pack's colours in turn, one per key.
    NS_VALID_UNTIL_END_OF_SCOPE NSColor *sparkColor = _effectColors.count > 0 ? _effectColors[effect.combo % _effectColors.count] : accentColor;
    NS_VALID_UNTIL_END_OF_SCOPE NSColor *badgeColor = [accentColor colorWithAlphaComponent:0.92];
    CGColorRef accent = accentColor.CGColor;
    CGColorRef badgeFill = badgeColor.CGColor;

    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _root.contentsScale = scale;
    _emitter.contentsScale = scale;
    _badgeText.contentsScale = scale;
    const CGPoint local = CGPointMake(origin.x - NSMinX(frame), origin.y - NSMinY(frame));
    _emitter.emitterPosition = local;
    _emitter.frame = _root.bounds;

    // A pack that asks for no particles draws its sparks style without sparks.
    const BOOL sparks = (style == MSIMETypingEffectStyleSparks || style == MSIMETypingEffectStylePowerMode) && _effectParticles != 0;
    const BOOL power = style == MSIMETypingEffectStylePowerMode;
    if (sparks) {
        const CGFloat burst = (power ? 2.0 : 1.0) * (commit ? 2.0 : 1.0);
        // A pack's particle count is the sparks of one key's burst, so the birth rate spreads them over the burst.
        const CGFloat birthRate = _effectParticles > 0 ? (CGFloat)_effectParticles * (commit ? 2.0 : 1.0) / (commit ? kCommitBurst : kKeyBurst)
                                                       : 220.0 * strength * burst;
        [_emitter setValue:(__bridge id)sparkColor.CGColor forKeyPath:@"emitterCells.spark.color"];
        [_emitter setValue:@(birthRate) forKeyPath:@"emitterCells.spark.birthRate"];
        [_emitter setValue:@(power ? 170.0 : 120.0) forKeyPath:@"emitterCells.spark.velocity"];
        _emitter.hidden = NO;
        // A fresh begin time starts this burst now instead of continuing an emission the layer believes is long running.
        _emitter.beginTime = CACurrentMediaTime();
        _emitter.birthRate = 1;
        _emitting = YES;
    }

    if (combo != nil) {
        _badge.backgroundColor = badgeFill;
        _badge.frame = CGRectMake(NSMinX(badgeScreen) - NSMinX(frame), NSMinY(badgeScreen) - NSMinY(frame), NSWidth(badgeScreen), kBadgeHeight);
        // A text layer draws from its top edge, so the line is centred by giving it the font's own height.
        NSFont *font = (__bridge NSFont *)_badgeText.font;
        const CGFloat line = std::ceil(font.ascender - font.descender);
        _badgeText.string = combo;
        _badgeText.frame = CGRectMake(0, (kBadgeHeight - line) / 2.0, NSWidth(badgeScreen), line);
        _badge.hidden = NO;
    } else {
        _badge.hidden = YES;
    }
    [CATransaction commit];

    if (combo != nil && !reduceMotion && (effect.tierUp || power)) {
        CAKeyframeAnimation *bounce = [CAKeyframeAnimation animationWithKeyPath:@"transform.scale"];
        bounce.values = effect.tierUp ? @[@1.0, @1.35, @0.95, @1.0] : @[@1.0, @1.12, @1.0];
        bounce.duration = effect.tierUp ? 0.35 : 0.15;
        [_badge addAnimation:bounce forKey:@"bounce"];
    }

    const BOOL flash = style == MSIMETypingEffectStyleFlash || power;
    const NSTimeInterval flashDuration = _effectDuration > 0 ? _effectDuration : (commit ? kCommitFlash : kKeyFlash);
    const float peak = (float)((commit ? 0.32 : 0.2) * strength);
    if (flash && card) {
        [self flashCard:candidateView rect:cardRect cornerRadius:cornerRadius color:accent peak:peak duration:flashDuration];
        if (power && !reduceMotion && !lowPower) [self shakeLayer:candidateView.layer];
    } else if (flash && caret) {
        // No card on screen, as after a commit: flash the caret's line instead.
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        _caretFlash.backgroundColor = accent;
        _caretFlash.frame = CGRectMake(NSMinX(caretRect) - NSMinX(frame) - 2.0, NSMinY(caretRect) - NSMinY(frame), 4.0 + std::max<CGFloat>(NSWidth(caretRect), 2.0),
                                       NSHeight(caretRect));
        _caretFlash.opacity = 0;
        [CATransaction commit];
        CABasicAnimation *fade = [CABasicAnimation animationWithKeyPath:@"opacity"];
        fade.fromValue = @(peak * 1.5f);
        fade.toValue = @0.0f;
        fade.duration = flashDuration;
        [_caretFlash addAnimation:fade forKey:@"flash"];
    }

    if (sparks || combo != nil || (flash && !card)) [self orderFrontRegardless];
    __weak MSIMETypingEffectPanel *weakSelf = self;
    if (sparks) {
        _burstTimer = [NSTimer scheduledTimerWithTimeInterval:commit ? kCommitBurst : kKeyBurst repeats:NO block:^(NSTimer *timer) {
            (void)timer;
            [weakSelf stopEmitting];
        }];
    }
    _settleTimer = [NSTimer scheduledTimerWithTimeInterval:combo != nil ? kBadgeSettle : std::max(kSparkSettle, flashDuration)
                                                   repeats:NO
                                                     block:^(NSTimer *timer) {
                                                         (void)timer;
                                                         [weakSelf settle];
                                                     }];
}

- (void)flashCard:(NSView *)candidateView rect:(NSRect)cardRect cornerRadius:(CGFloat)cornerRadius color:(CGColorRef)color peak:(float)peak duration:(NSTimeInterval)duration {
    MSIMETypingEffectFlashView *flash = _cardFlash;
    if (flash.superview != candidateView) {
        // The candidate window rebuilds its content on a full render, so the overlay is found again rather than kept.
        flash = nil;
        for (NSView *subview in candidateView.subviews)
            if ([subview isKindOfClass:MSIMETypingEffectFlashView.class]) flash = (MSIMETypingEffectFlashView *)subview;
        if (flash == nil) {
            flash = [[MSIMETypingEffectFlashView alloc] initWithFrame:cardRect];
            flash.identifier = kFlashIdentifier;
            flash.wantsLayer = YES;
        }
        _cardFlash = flash;
    }
    // Last among the subviews, so the tint lies over the candidates.
    if (flash.superview != candidateView || candidateView.subviews.lastObject != flash)
        [candidateView addSubview:flash positioned:NSWindowAbove relativeTo:nil];
    flash.frame = cardRect;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    flash.layer.cornerRadius = cornerRadius;
    flash.layer.backgroundColor = color;
    flash.layer.opacity = 0;
    [CATransaction commit];
    CABasicAnimation *fade = [CABasicAnimation animationWithKeyPath:@"opacity"];
    fade.fromValue = @(peak);
    fade.toValue = @0.0f;
    fade.duration = duration;
    fade.timingFunction = [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseOut];
    [flash.layer addAnimation:fade forKey:@"flash"];
}

- (void)shakeLayer:(CALayer *)layer {
    if (layer == nil) return;
    const CGFloat amplitude = 0.5 + 1.5 * (CGFloat)_intensity / 100.0;
    CAKeyframeAnimation *shake = [CAKeyframeAnimation animationWithKeyPath:@"transform.translation.x"];
    shake.values = @[@0.0, @(amplitude), @(-amplitude), @(amplitude / 2.0), @0.0];
    shake.duration = 0.12;
    shake.additive = YES;
    [layer addAnimation:shake forKey:@"typing-shake"];
}

- (void)stopEmitting {
    [_burstTimer invalidate];
    _burstTimer = nil;
    _emitter.birthRate = 0;
    _emitting = NO;
}

- (void)settle {
    [self stopEmitting];
    [_settleTimer invalidate];
    _settleTimer = nil;
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _emitter.hidden = YES;
    _badge.hidden = YES;
    [_badge removeAllAnimations];
    [_caretFlash removeAllAnimations];
    _caretFlash.opacity = 0;
    [CATransaction commit];
    MSIMETypingEffectFlashView *flash = _cardFlash;
    [flash removeFromSuperview];
    _cardFlash = nil;
    [self orderOut:nil];
}
@end
