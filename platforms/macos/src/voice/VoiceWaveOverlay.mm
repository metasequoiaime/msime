#import "VoiceWaveOverlay.h"
#include <algorithm>
#include <cmath>

namespace {
// Geometry, colors and timing follow MSIME-Windows server/src/voice-input/wave_overlay.cpp; Windows logical pixels map to AppKit points.
constexpr CGFloat kCompactWidth = 78, kCompactHeight = 32;
constexpr CGFloat kProcessingWidth = 112, kProcessingHeight = 40;
constexpr CGFloat kActionWidth = 142, kActionHeight = 40;
constexpr CGFloat kTranscriptWidth = 420, kTranscriptHeight = 112;
constexpr CGFloat kActionCenterInset = 18, kActionRadius = 12, kActionContentInset = 35;
constexpr CGFloat kTranscriptHorizontalPadding = 14, kTranscriptTextTop = 33, kTranscriptTextBottom = 8, kTranscriptLineHeight = 22;
constexpr CGFloat kTranscriptFontSize = 15, kStatusFontSize = 14;
constexpr NSUInteger kMaxTranscriptLines = 3;
constexpr CGFloat kPanelOpacity = 0.90;
constexpr CGFloat kDotRadius = 1.15, kMaxHalfHeight = 14;
constexpr CGFloat kWorkAreaBottomGap = 10;
constexpr NSTimeInterval kFrameInterval = 0.016;
// Three lines of 15-point text in 392 points cannot hold this many UTF-16 units, so the newest-text search never lays out more than this tail of a long transcript.
constexpr NSUInteger kTranscriptSearchWindow = 2048;
}

NSPoint MSIMEVoiceWaveOverlayOriginForFrames(NSRect fullFrame, NSRect visibleFrame, NSSize panelSize) {
    const CGFloat width = MAX(0.0, panelSize.width);
    const CGFloat height = MAX(0.0, panelSize.height);
    // Match the Windows host: horizontal placement is centered in the full monitor, so a left/right Dock does not move the voice bar's center.
    CGFloat x = NSMidX(fullFrame) - width / 2.0;
    // Vertical placement sits 10 points above the bottom of the visible work area, clear of a bottom Dock, as the source does with rcWork.bottom - height - 10.
    CGFloat y = NSMinY(visibleFrame) + kWorkAreaBottomGap;
    if (width <= NSWidth(fullFrame))
        x = MIN(MAX(x, NSMinX(fullFrame)), NSMaxX(fullFrame) - width);
    else
        x = NSMinX(fullFrame);
    if (height <= NSHeight(visibleFrame))
        y = MIN(MAX(y, NSMinY(visibleFrame)), NSMaxY(visibleFrame) - height);
    else
        y = NSMinY(visibleFrame);
    return NSMakePoint(x, y);
}

MSIMEVoiceWaveOverlayLayout MSIMEVoiceWaveOverlayLayoutFor(BOOL actionsVisible, BOOL hasStatusLabel, BOOL hasTranscript) {
    if (actionsVisible) return MSIMEVoiceWaveOverlayLayoutAction;
    if (hasStatusLabel) return MSIMEVoiceWaveOverlayLayoutProcessing;
    return hasTranscript ? MSIMEVoiceWaveOverlayLayoutTranscript : MSIMEVoiceWaveOverlayLayoutCompact;
}

NSSize MSIMEVoiceWaveOverlaySizeForLayout(MSIMEVoiceWaveOverlayLayout layout) {
    switch (layout) {
        case MSIMEVoiceWaveOverlayLayoutProcessing: return NSMakeSize(kProcessingWidth, kProcessingHeight);
        case MSIMEVoiceWaveOverlayLayoutAction: return NSMakeSize(kActionWidth, kActionHeight);
        case MSIMEVoiceWaveOverlayLayoutTranscript: return NSMakeSize(kTranscriptWidth, kTranscriptHeight);
        default: return NSMakeSize(kCompactWidth, kCompactHeight);
    }
}

void MSIMEVoiceWaveAdvanceLevels(float levels[MSIMEVoiceWaveBarCount], float inputLevel, BOOL listening, double seconds) {
    const float level = listening && inputLevel > 0.0f ? std::min(1.0f, inputLevel) : 0.0f;
    const float center = 0.5f * static_cast<float>(MSIMEVoiceWaveBarCount - 1);
    for (int i = 0; i < MSIMEVoiceWaveBarCount; ++i) {
        // Deterministic-but-irregular profile per bar: each bar has its own amplitude, frequency and phase so the movement looks natural.
        const float fi = static_cast<float>(i);
        const float amplitude = 0.55f + 0.45f * std::fabs(std::sin(0.73f * fi + 0.19f));
        const float p = 0.41f * fi + 0.37f * std::sin(0.29f * fi + 1.11f);
        const float f = 4.2f + std::fmod(1.7f * fi, 3.6f);
        const float dist = std::fabs(fi - center) / std::max(1.0f, center);
        const float centerBoost = 1.0f + 0.75f * (1.0f - dist);
        // The clock stays in double: a float of the system uptime loses the 16 ms step after a few days.
        const float n1 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * f + p));
        const float n2 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * (0.57f * f) + 1.7f * p + 0.9f));
        const float n3 = 0.5f + 0.5f * static_cast<float>(std::sin(seconds * (1.23f * f) + 0.6f * p + 2.1f));
        const float irregular = std::min(1.0f, 0.60f * n1 + 0.30f * n2 + 0.10f * n3);
        // When the signal is weak the floor drops too, so bars settle back to dots smoothly.
        const float floor = level * (0.06f + 0.12f * n3);
        float target = level * amplitude * centerBoost * std::max(floor, irregular);
        // A transient punch on the rising edge makes the start feel more energetic.
        if (target > levels[i]) target = std::min(1.0f, target + 0.22f * level * (1.0f - levels[i]));
        const float smooth = target > levels[i] ? 0.52f : (listening ? 0.07f : 0.045f);
        levels[i] = levels[i] * (1.0f - smooth) + target * smooth;
    }
}

NSUInteger MSIMEVoiceTranscriptLineCount(NSString *text, NSFont *font, CGFloat width) {
    if (!text.length || !font || width <= 0) return 0;
    NSTextStorage *storage = [[NSTextStorage alloc] initWithString:text attributes:@{NSFontAttributeName: font}];
    NSLayoutManager *layoutManager = [NSLayoutManager new];
    NSTextContainer *container = [[NSTextContainer alloc] initWithSize:NSMakeSize(width, CGFLOAT_MAX)];
    container.lineFragmentPadding = 0;
    [layoutManager addTextContainer:container];
    [storage addLayoutManager:layoutManager];
    __block NSUInteger lines = 0;
    [layoutManager enumerateLineFragmentsForGlyphRange:[layoutManager glyphRangeForTextContainer:container]
                                            usingBlock:^(NSRect rect, NSRect usedRect, NSTextContainer *textContainer, NSRange glyphRange, BOOL *stop) {
        (void)rect; (void)usedRect; (void)textContainer; (void)glyphRange; (void)stop;
        ++lines;
    }];
    // A trailing newline starts a line of its own.
    if (layoutManager.extraLineFragmentTextContainer) ++lines;
    return lines;
}

NSString *MSIMEVoiceTranscriptVisibleText(NSString *transcript, NSUInteger maxLines, NSUInteger (^lineCount)(NSString *candidate)) {
    if (![transcript isKindOfClass:NSString.class] || !transcript.length) return @"";
    if (!lineCount || lineCount(transcript) <= maxLines) return transcript;
    const NSUInteger length = transcript.length;
    // Cut at the start of a composed character sequence so an emoji or a base letter with its marks is kept whole or dropped whole.
    NSUInteger (^cut)(NSUInteger) = ^NSUInteger(NSUInteger index) {
        if (index >= length) return length;
        const NSRange sequence = [transcript rangeOfComposedCharacterSequenceAtIndex:index];
        return sequence.location == index ? index : NSMaxRange(sequence);
    };
    NSUInteger low = length > kTranscriptSearchWindow ? length - kTranscriptSearchWindow : 0;
    NSUInteger high = length;
    while (low < high) {
        const NSUInteger middle = low + (high - low) / 2;
        if (lineCount([@"…" stringByAppendingString:[transcript substringFromIndex:cut(middle)]]) <= maxLines)
            high = middle;
        else
            low = middle + 1;
    }
    return [@"…" stringByAppendingString:[transcript substringFromIndex:cut(low)]];
}

static BOOL VoiceAppearanceIsDark(NSAppearance *appearance)
{
    NSAppearance *resolved = appearance ?: NSApp.effectiveAppearance ?: NSAppearance.currentDrawingAppearance;
    NSString *match = [resolved bestMatchFromAppearancesWithNames:@[ NSAppearanceNameAqua, NSAppearanceNameDarkAqua ]];
    return [match isEqualToString:NSAppearanceNameDarkAqua];
}
static NSFont *VoiceTranscriptFont() { return [NSFont systemFontOfSize:kTranscriptFontSize]; }
static NSFont *VoiceStatusFont() { return [NSFont systemFontOfSize:kStatusFontSize]; }
@interface MSIMEVoiceWaveView : NSView
@property(nonatomic) float level;
@property(nonatomic) BOOL listening;
@property(nonatomic) BOOL lightTheme;
@property(nonatomic) BOOL followsSystemAppearance;
/// The spoken description of the presentation; drawn only when `showsLabel` is set.
@property(nonatomic, copy) NSString *status;
@property(nonatomic) BOOL showsLabel;
@property(nonatomic) BOOL showsActions;
/// Already cut to the newest three lines; nil when the layout has no transcript area.
@property(nonatomic, copy) NSString *visibleTranscript;
- (void)advanceWave;
- (void)resetWave;
@end
@implementation MSIMEVoiceWaveView {
    float _levels[MSIMEVoiceWaveBarCount];
}
- (BOOL)isFlipped { return YES; }
- (void)advanceWave {
    MSIMEVoiceWaveAdvanceLevels(_levels, self.level, self.listening, NSProcessInfo.processInfo.systemUptime);
    [self setNeedsDisplay:YES];
}
- (void)resetWave { std::fill(_levels, _levels + MSIMEVoiceWaveBarCount, 0.0f); }
- (void)drawRect:(NSRect)r {
    (void)r;
    const BOOL light = self.followsSystemAppearance ? !VoiceAppearanceIsDark(self.effectiveAppearance) : self.lightTheme;
    NSColor *background = light
        ? [NSColor colorWithSRGBRed:0.98 green:0.98 blue:0.99 alpha:kPanelOpacity]
        : [NSColor colorWithSRGBRed:0.07 green:0.08 blue:0.10 alpha:kPanelOpacity];
    NSColor *foreground = light ? [NSColor colorWithSRGBRed:0.35 green:0.18 blue:0.42 alpha:1] : NSColor.whiteColor;
    NSColor *border = light
        ? [NSColor colorWithSRGBRed:0.20 green:0.20 blue:0.24 alpha:0.22]
        : [NSColor colorWithSRGBRed:0.90 green:0.93 blue:1.0 alpha:0.20];
    NSColor *actionBackground = light
        ? [NSColor colorWithSRGBRed:0.86 green:0.87 blue:0.89 alpha:1]
        : [NSColor colorWithSRGBRed:0.18 green:0.19 blue:0.21 alpha:1];
    const CGFloat w = NSWidth(self.bounds), h = NSHeight(self.bounds);
    const BOOL actions = self.showsActions;
    const BOOL label = self.showsLabel;
    const BOOL transcript = self.visibleTranscript.length > 0;

    const CGFloat corner = actions || (label && !transcript) ? h * 0.5 : (transcript ? 16.0 : 10.0);
    NSBezierPath *panel = [NSBezierPath bezierPathWithRoundedRect:NSMakeRect(0.5, 0.5, w - 1.0, h - 1.0) xRadius:corner yRadius:corner];
    [background setFill]; [panel fill];
    [border setStroke]; panel.lineWidth = 1.0; [panel stroke];

    const CGFloat contentLeft = actions ? kActionContentInset : 0.0;
    const CGFloat contentRight = actions ? w - kActionContentInset : w;
    const CGFloat contentWidth = contentRight - contentLeft;
    const CGFloat centerY = transcript ? 18.0 : h * 0.5;
    if (label) {
        NSMutableParagraphStyle *style = [NSMutableParagraphStyle new];
        style.alignment = NSTextAlignmentCenter;
        style.lineBreakMode = NSLineBreakByTruncatingTail;
        NSDictionary *attributes = @{NSFontAttributeName: VoiceStatusFont(), NSForegroundColorAttributeName: foreground, NSParagraphStyleAttributeName: style};
        const CGFloat lineHeight = ceil(VoiceStatusFont().ascender - VoiceStatusFont().descender + VoiceStatusFont().leading);
        [self.status drawInRect:NSMakeRect(contentLeft, centerY - lineHeight * 0.5, contentWidth, lineHeight) withAttributes:attributes];
    } else {
        [foreground setFill];
        const CGFloat waveWidth = transcript ? MIN(78.0, w) : (actions ? MIN(78.0, contentWidth) : w);
        const CGFloat waveLeft = actions ? contentLeft + (contentWidth - waveWidth) * 0.5 : (transcript ? (w - waveWidth) * 0.5 : 0.0);
        const CGFloat sideMargin = kDotRadius + 0.25;
        const CGFloat trackWidth = MAX(1.0, waveWidth - 2.0 * sideMargin);
        const CGFloat baseStep = trackWidth / static_cast<CGFloat>(MSIMEVoiceWaveBarCount - 1);
        // A slightly tighter spacing keeps the wave compact.
        const CGFloat step = baseStep * 0.75;
        const CGFloat startX = waveLeft + (waveWidth - step * static_cast<CGFloat>(MSIMEVoiceWaveBarCount - 1)) * 0.5;
        const CGFloat barWidth = MAX(kDotRadius * 2.0, baseStep * 0.26);
        for (int i = 0; i < MSIMEVoiceWaveBarCount; ++i) {
            const CGFloat x = startX + i * step;
            const CGFloat displayLevel = transcript ? MIN(1.0, _levels[i] * 1.35) : _levels[i];
            if (displayLevel < 0.06) {
                [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(x - kDotRadius, centerY - kDotRadius, kDotRadius * 2.0, kDotRadius * 2.0)] fill];
            } else {
                const CGFloat half = kDotRadius + displayLevel * (kMaxHalfHeight - kDotRadius);
                [[NSBezierPath bezierPathWithRoundedRect:NSMakeRect(x - barWidth * 0.5, centerY - half, barWidth, half * 2.0)
                                                 xRadius:barWidth * 0.5 yRadius:barWidth * 0.5] fill];
            }
        }
    }

    if (transcript) {
        NSMutableParagraphStyle *style = [NSMutableParagraphStyle new];
        style.minimumLineHeight = kTranscriptLineHeight;
        style.maximumLineHeight = kTranscriptLineHeight;
        const NSRect textRect = NSMakeRect(kTranscriptHorizontalPadding, kTranscriptTextTop, w - 2.0 * kTranscriptHorizontalPadding,
                                           h - kTranscriptTextTop - kTranscriptTextBottom);
        [NSGraphicsContext saveGraphicsState];
        NSRectClip(textRect);
        [self.visibleTranscript drawWithRect:textRect options:NSStringDrawingUsesLineFragmentOrigin
                              attributes:@{NSFontAttributeName: VoiceTranscriptFont(), NSForegroundColorAttributeName: foreground, NSParagraphStyleAttributeName: style}
                                 context:nil];
        [NSGraphicsContext restoreGraphicsState];
    }

    if (actions) {
        const CGFloat buttonY = h * 0.5;
        const CGFloat leftX = kActionCenterInset, rightX = w - kActionCenterInset;
        [actionBackground setFill];
        for (CGFloat x : {leftX, rightX})
            [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(x - kActionRadius, buttonY - kActionRadius, kActionRadius * 2.0, kActionRadius * 2.0)] fill];
        [foreground setStroke];
        NSBezierPath *glyphs = [NSBezierPath bezierPath];
        glyphs.lineWidth = 1.7;
        constexpr CGFloat xHalf = 3.0;
        [glyphs moveToPoint:NSMakePoint(leftX - xHalf, buttonY - xHalf)]; [glyphs lineToPoint:NSMakePoint(leftX + xHalf, buttonY + xHalf)];
        [glyphs moveToPoint:NSMakePoint(leftX + xHalf, buttonY - xHalf)]; [glyphs lineToPoint:NSMakePoint(leftX - xHalf, buttonY + xHalf)];
        [glyphs moveToPoint:NSMakePoint(rightX - 3.8, buttonY)]; [glyphs lineToPoint:NSMakePoint(rightX - 0.8, buttonY + 3.0)];
        [glyphs lineToPoint:NSMakePoint(rightX + 4.2, buttonY - 3.5)];
        [glyphs stroke];
    }
}
- (void)viewDidChangeEffectiveAppearance {
    [super viewDidChangeEffectiveAppearance];
    if (self.followsSystemAppearance) [self setNeedsDisplay:YES];
}
@end
@implementation MSIMEVoiceWaveOverlay {
    MSIMEVoiceWaveView *_view;
    NSString *_transcript;
    NSString *_visibleTranscriptSource;
    CGFloat _visibleTranscriptWidth;
    NSString *_visibleTranscript;
    NSUInteger _presentationGeneration;
    BOOL _showingFailure;
    BOOL _dismissed;
    BOOL _processing;
    BOOL _recordingLocked;
    NSButton *_cancelButton;
    NSButton *_confirmButton;
    NSTimer *_waveTimer;
    BOOL _followsSystemAppearance;
    id _screenObserver;
}
- (instancetype)init {
    self = [super initWithContentRect:NSMakeRect(0, 0, kCompactWidth, kCompactHeight)
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    if (self) {
        self.opaque = NO; self.backgroundColor = NSColor.clearColor;
        self.level = NSFloatingWindowLevel; self.ignoresMouseEvents = YES;
        self.floatingPanel = YES; self.hidesOnDeactivate = NO;
        _view = [MSIMEVoiceWaveView new]; _view.status = @""; self.contentView = _view;
        _transcript = @"";
        _cancelButton = [NSButton buttonWithTitle:@"取消" target:self action:@selector(cancelVoice:)];
        _confirmButton = [NSButton buttonWithTitle:@"确认" target:self action:@selector(confirmVoice:)];
        for (NSButton *button in @[_cancelButton, _confirmButton]) {
            // The wave view draws the round X and check; the buttons only take the click and carry the accessibility label.
            button.hidden = YES; button.refusesFirstResponder = YES;
            button.bordered = NO; button.transparent = YES;
            button.accessibilityLabel = button.title;
            [_view addSubview:button];
        }
        __weak MSIMEVoiceWaveOverlay *weakSelf = self;
        _screenObserver = [NSNotificationCenter.defaultCenter
            addObserverForName:NSApplicationDidChangeScreenParametersNotification
                        object:nil
                         queue:NSOperationQueue.mainQueue
                    usingBlock:^(NSNotification *notification) {
                        (void)notification;
                        MSIMEVoiceWaveOverlay *overlay = weakSelf;
                        if (overlay && overlay.visible) [overlay repositionOnPreferredScreen];
                    }];
    }
    return self;
}
- (void)dealloc {
    [_waveTimer invalidate];
    if (_screenObserver) [NSNotificationCenter.defaultCenter removeObserver:_screenObserver];
}
- (void)setPreferredScreen:(NSScreen *)screen {
    _preferredScreen = screen;
    if (self.visible) [self repositionOnPreferredScreen];
}
- (NSScreen *)resolvedPreferredScreen {
    NSScreen *screen = _preferredScreen;
    if (screen && [NSScreen.screens containsObject:screen]) return screen;
    return NSScreen.mainScreen ?: NSScreen.screens.firstObject;
}
- (void)repositionOnPreferredScreen {
    NSScreen *screen = [self resolvedPreferredScreen];
    if (!screen) return;
    [self setFrameOrigin:MSIMEVoiceWaveOverlayOriginForFrames(screen.frame, screen.visibleFrame, self.frame.size)];
}
- (BOOL)isLightTheme { return _view.lightTheme; }
- (void)applyThemePreferences:(NSDictionary *)preferences {
    // macOS 的语音输入弹出条一律跟随颜色模式，不读它单独的明暗设置。
    id resolved = preferences[@"theme"];
    _followsSystemAppearance = resolved == nil || [resolved isEqual:@"system"];
    if ([resolved isEqual:@"light"])
        self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    else if (_followsSystemAppearance)
        self.appearance = nil;
    else
        self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    _view.followsSystemAppearance = _followsSystemAppearance;
    _view.lightTheme = !VoiceAppearanceIsDark(self.effectiveAppearance);
    [_view setNeedsDisplay:YES];
}
- (NSString *)statusText { return _view.status; }
- (NSString *)transcriptText { return _transcript; }
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
- (void)setActionHandler:(void (^)(BOOL))handler {
    _actionHandler = [handler copy];
    [self layoutPresentation];
}
- (void)cancelVoice:(id)sender {
    (void)sender;
    if (self.visible && [self actionsVisible]) self.actionHandler(YES);
}
- (void)confirmVoice:(id)sender {
    (void)sender;
    if (self.visible && [self actionsVisible]) self.actionHandler(NO);
}
- (void)dismissProcessing {
    _dismissed = YES;
    ++_presentationGeneration;
    [self orderOut:nil];
}
- (void)orderOut:(id)sender {
    [super orderOut:sender];
    [self updateWaveAnimation];
}
// The source shows the round actions in lock mode or while recognition or polishing is pending (`set_actions_visible`), never during a plain hold.
- (BOOL)actionsVisible {
    return self.actionHandler && _view.status.length && !_showingFailure && !_dismissed &&
           (_processing || (_recordingLocked && _view.listening));
}
- (NSString *)visibleTranscriptForWidth:(CGFloat)width {
    if (!_transcript.length) return nil;
    if (_visibleTranscript && _visibleTranscriptWidth == width && [_visibleTranscriptSource isEqualToString:_transcript]) return _visibleTranscript;
    NSFont *font = VoiceTranscriptFont();
    _visibleTranscript = MSIMEVoiceTranscriptVisibleText(_transcript, kMaxTranscriptLines, ^NSUInteger(NSString *candidate) {
        return MSIMEVoiceTranscriptLineCount(candidate, font, width);
    });
    _visibleTranscriptSource = _transcript;
    _visibleTranscriptWidth = width;
    return _visibleTranscript;
}
- (void)layoutPresentation {
    const BOOL actions = [self actionsVisible];
    NSSize size;
    NSString *visible = nil;
    if (_showingFailure) {
        // Windows reports failures in a message box; this host keeps them in the overlay, so the category message takes the label line and the provider's detail the transcript area.
        const CGFloat labelWidth = ceil([_view.status sizeWithAttributes:@{NSFontAttributeName: VoiceStatusFont()}].width) + 2.0 * kTranscriptHorizontalPadding;
        size = _transcript.length ? NSMakeSize(MAX(kTranscriptWidth, labelWidth), kTranscriptHeight)
                                  : NSMakeSize(MAX(kProcessingWidth, labelWidth), kProcessingHeight);
        if (_transcript.length) visible = [self visibleTranscriptForWidth:size.width - 2.0 * kTranscriptHorizontalPadding];
    } else {
        const MSIMEVoiceWaveOverlayLayout layout = MSIMEVoiceWaveOverlayLayoutFor(actions, _processing, _transcript.length > 0);
        size = MSIMEVoiceWaveOverlaySizeForLayout(layout);
        if (layout == MSIMEVoiceWaveOverlayLayoutTranscript) visible = [self visibleTranscriptForWidth:size.width - 2.0 * kTranscriptHorizontalPadding];
    }
    _view.showsLabel = _showingFailure || _processing;
    _view.showsActions = actions;
    _view.visibleTranscript = visible;
    NSScreen *screen = [self resolvedPreferredScreen];
    const NSPoint origin = screen ? MSIMEVoiceWaveOverlayOriginForFrames(screen.frame, screen.visibleFrame, size) : self.frame.origin;
    [self setFrame:NSMakeRect(origin.x, origin.y, size.width, size.height) display:NO];
    _cancelButton.hidden = _confirmButton.hidden = !actions;
    // The view is flipped, so these centers match the source's top-left coordinates.
    _cancelButton.frame = NSMakeRect(kActionCenterInset - kActionRadius, size.height * 0.5 - kActionRadius, kActionRadius * 2.0, kActionRadius * 2.0);
    _confirmButton.frame = NSMakeRect(size.width - kActionCenterInset - kActionRadius, size.height * 0.5 - kActionRadius, kActionRadius * 2.0, kActionRadius * 2.0);
    // Only the round actions take the pointer; this panel still cannot become key/main or redirect typing away from the IMK client.
    self.ignoresMouseEvents = !actions;
    [self updateWaveAnimation];
    [_view setNeedsDisplay:YES];
}
// The bars animate at the source's 16 ms cadence only while the wave is on screen.
- (void)updateWaveAnimation {
    const BOOL animate = self.visible && !_view.showsLabel;
    if (animate && !_waveTimer) {
        __weak MSIMEVoiceWaveOverlay *weakSelf = self;
        _waveTimer = [NSTimer timerWithTimeInterval:kFrameInterval repeats:YES block:^(NSTimer *timer) {
            MSIMEVoiceWaveOverlay *overlay = weakSelf;
            if (!overlay) { [timer invalidate]; return; }
            [overlay->_view advanceWave];
        }];
        [NSRunLoop.mainRunLoop addTimer:_waveTimer forMode:NSRunLoopCommonModes];
    } else if (!animate && _waveTimer) {
        [_waveTimer invalidate];
        _waveTimer = nil;
        [_view resetWave];
    }
}
- (void)setTranscript:(NSString *)text {
    if (!self.visible || _showingFailure || ![text isKindOfClass:NSString.class] || text.length > 65536) return;
    _transcript = [text copy];
    [self layoutPresentation];
}
- (void)showStatus:(NSString *)status listening:(BOOL)listening failure:(BOOL)failure {
    ++_presentationGeneration;
    _showingFailure = failure;
    _view.listening = listening; _view.level = 0; _view.status = status;
    _view.accessibilityLabel = status;
    [self layoutPresentation];
    if (!status.length || _dismissed) { [self orderOut:nil]; return; }
    [self orderFront:nil];
    [self updateWaveAnimation];
}
- (void)setListening:(BOOL)listening {
    if (listening) _dismissed = NO;
    else self.actionHandler = nil;
    _processing = NO; _recordingLocked = NO;
    _transcript = @"";
    [self showStatus:listening ? @"正在录音…" : @"" listening:listening failure:NO];
}
- (void)setRecordingLocked:(BOOL)locked {
    if (locked && (!_view.listening || _showingFailure)) return;
    if (_recordingLocked == locked) return;
    _recordingLocked = locked;
    [self layoutPresentation];
}
- (void)setProcessing:(BOOL)polishing {
    _processing = YES; _recordingLocked = NO;
    [self showStatus:polishing ? @"处理中..." : @"识别中..." listening:NO failure:NO];
}
- (NSTimeInterval)failureDisplayDuration { return 6; }
- (void)showFailure:(MSIMEVoiceFailure)failure { [self showFailure:failure detail:nil]; }
- (void)showFailure:(MSIMEVoiceFailure)failure detail:(NSString *)detail {
    self.actionHandler = nil;
    _dismissed = NO;
    _processing = NO; _recordingLocked = NO;
    // A pending transcript is discarded. The detail is the sentence the voice request built for the user from the provider's answer - its message, HTTP status or trace id - never a transcript, credential or request body, and it is shown in the transcript area.
    _transcript = [detail isKindOfClass:NSString.class] && detail.length <= 65536 ? [detail copy] : @"";
    NSString *message;
    switch (failure) {
        case MSIMEVoiceFailureMicrophonePermission: message = @"请在系统设置允许麦克风访问"; break;
        case MSIMEVoiceFailureSpeechPermission: message = @"请在系统设置允许语音识别"; break;
        case MSIMEVoiceFailureCapture: message = @"录音失败，请检查麦克风设置"; break;
        case MSIMEVoiceFailureProvider: message = @"识别失败，请检查语音服务设置"; break;
        case MSIMEVoiceFailureNoSpeech: message = @"未识别到语音，请重试"; break;
        case MSIMEVoiceFailureTimeout: message = @"语音处理超时，请重试"; break;
        case MSIMEVoiceFailureMissingToken: message = @"请先在设置的“语音输入”分区填写当前 ASR 提供商的 API Token。"; break;
        case MSIMEVoiceFailureMissingLocalModel: message = @"请先在设置的“语音输入”分区下载或选择本地语音模型。"; break;
        default: message = @"语音输入未能启动，请重试"; break;
    }
    [self showStatus:message listening:NO failure:YES];
    const NSUInteger generation = _presentationGeneration;
    __weak MSIMEVoiceWaveOverlay *weakSelf = self;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)([self failureDisplayDuration] * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        MSIMEVoiceWaveOverlay *panel = weakSelf;
        if (panel && panel->_presentationGeneration == generation) [panel dismissFailure];
    });
}
- (void)dismissFailure { if (_showingFailure) [self setListening:NO]; }
- (void)setInputLevel:(float)level {
    const NSUInteger generation = _presentationGeneration;
    dispatch_async(dispatch_get_main_queue(), ^{
        if (generation != self->_presentationGeneration || !self->_view.listening) return;
        // The animation timer reads the level on its next frame, as the source's WM_TIMER does.
        self->_view.level = MAX(0, MIN(1, level));
    });
}
@end
