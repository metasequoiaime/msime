#import "InputModeHUDPanel.h"
#import "../core/NativeGlassBackground.h"
#import "../candidate/CandidatePlacement.h"
#import "../candidate/CandidateWindowLevel.h"

#import <algorithm>
#import <cmath>

namespace {
// 短暂切换提示使用独立紧凑尺寸；工具栏为了点击放大时不放大这个鼠标穿透的提示。
constexpr CGFloat kFontSize = 18.0;
constexpr CGFloat kHeight = 32.0;
constexpr CGFloat kLogoSide = 16.0;
constexpr CGFloat kCornerRadius = 8.0;
constexpr CGFloat kHorizontalInset = 8.0;
constexpr CGFloat kContentSpacing = 4.0;
constexpr CGFloat kBorderWidth = 1.0;
constexpr CGFloat kScreenMargin = 8.0;
constexpr CGFloat kCaretGap = 10.0;
constexpr NSTimeInterval kVisibleDuration = 0.6;
constexpr NSTimeInterval kFadeDuration = 0.18;

CGFloat Clamp(CGFloat value, CGFloat minimum, CGFloat maximum) {
    if (maximum < minimum) return minimum;
    return value < minimum ? minimum : (value > maximum ? maximum : value);
}
}

NSString *MSIMEInputModeHUDText(BOOL englishInputMode) { return englishInputMode ? @"英" : @"中"; }

BOOL MSIMEInputModeHUDUsableCaretRect(NSRect caretRect) {
    return std::isfinite(NSMinX(caretRect)) && std::isfinite(NSMinY(caretRect)) &&
           std::isfinite(NSMaxX(caretRect)) && std::isfinite(NSMaxY(caretRect)) && NSHeight(caretRect) > 0.0;
}

NSRect MSIMEInputModeHUDFrame(NSRect caretRect, NSSize panelSize, NSRect visibleFrame) {
    const CGFloat minimumX = NSMinX(visibleFrame) + kScreenMargin;
    const CGFloat maximumX = NSMaxX(visibleFrame) - kScreenMargin - panelSize.width;
    const CGFloat minimumY = NSMinY(visibleFrame) + kScreenMargin;
    const CGFloat maximumY = NSMaxY(visibleFrame) - kScreenMargin - panelSize.height;
    if (!MSIMEInputModeHUDUsableCaretRect(caretRect)) {
        const CGFloat centeredX = NSMidX(visibleFrame) - panelSize.width / 2.0;
        const CGFloat lowerThirdY = NSMinY(visibleFrame) + NSHeight(visibleFrame) / 4.0;
        return NSMakeRect(Clamp(centeredX, minimumX, maximumX), Clamp(lowerThirdY, minimumY, maximumY), panelSize.width, panelSize.height);
    }
    const CGFloat x = Clamp(NSMidX(caretRect) - panelSize.width / 2.0, minimumX, maximumX);
    const CGFloat belowY = NSMinY(caretRect) - kCaretGap - panelSize.height;
    const CGFloat preferredY = belowY >= minimumY ? belowY : NSMaxY(caretRect) + kCaretGap;
    return NSMakeRect(x, Clamp(preferredY, minimumY, maximumY), panelSize.width, panelSize.height);
}

@implementation MSIMEInputModeHUDPanel {
    NSTextField *_label;
    NSView *_glassBackground;
    NSView *_chrome;
    NSImageView *_logoView;
    NSTimer *_dismissTimer;
    NSColor *_surfaceColor;
    NSColor *_borderColor;
    NSColor *_textColor;
    NSStackView *_content;
    NSLayoutConstraint *_logoWidth;
    NSLayoutConstraint *_logoHeight;
}

+ (instancetype)sharedPanel {
    static MSIMEInputModeHUDPanel *panel;
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
    self.level = MSIMECandidateWindowLevel();
    self.becomesKeyOnlyIfNeeded = YES;
    self.hidesOnDeactivate = NO;
    self.opaque = NO;
    self.backgroundColor = NSColor.clearColor;
    self.hasShadow = YES;
    self.ignoresMouseEvents = YES;
    self.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary |
                              NSWindowCollectionBehaviorTransient | NSWindowCollectionBehaviorIgnoresCycle;
    self.animationBehavior = NSWindowAnimationBehaviorNone;

    NSView *background = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 1, 1)];
    background.wantsLayer = YES;
    background.layer.masksToBounds = YES;
    background.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    // The full-colour brand mark the floating toolbar leads with, not the monochrome menu bar template.
    NSString *logoPath = [[NSBundle bundleForClass:self.class] pathForResource:@"MSIMEClientInputMethod" ofType:@"icns"];
    NSImage *logo = logoPath ? [[NSImage alloc] initWithContentsOfFile:logoPath] : nil;
    _logoView = [NSImageView imageViewWithImage:logo ?: [[NSImage alloc] initWithSize:NSZeroSize]];
    _logoView.hidden = logo == nil;
    _logoView.imageScaling = NSImageScaleProportionallyUpOrDown;
    _logoView.translatesAutoresizingMaskIntoConstraints = NO;
    _label = [NSTextField labelWithString:@""];
    _label.alignment = NSTextAlignmentCenter;
    _label.translatesAutoresizingMaskIntoConstraints = NO;
    _content = [NSStackView stackViewWithViews:@[_logoView, _label]];
    _content.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    _content.alignment = NSLayoutAttributeCenterY;
    _content.translatesAutoresizingMaskIntoConstraints = NO;
    [background addSubview:_content];
    _logoWidth = [_logoView.widthAnchor constraintEqualToConstant:kLogoSide];
    _logoHeight = [_logoView.heightAnchor constraintEqualToConstant:kLogoSide];
    [NSLayoutConstraint activateConstraints:@[
        [_content.centerXAnchor constraintEqualToAnchor:background.centerXAnchor],
        [_content.centerYAnchor constraintEqualToAnchor:background.centerYAnchor],
        _logoWidth,
        _logoHeight,
    ]];
    _chrome = background;
    NSView *host = [[NSView alloc] initWithFrame:background.bounds];
    host.wantsLayer = YES;
    [host addSubview:background];
    self.contentView = host;
    [NSWorkspace.sharedWorkspace.notificationCenter addObserver:self selector:@selector(applyThemeColors)
        name:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:nil];
    [self applySizingPreferences:@{}];
    [self applyThemeColors];
    return self;
}

- (void)applySizingPreferences:(NSDictionary *)preferences {
    id value = preferences[@"show_app_logo"];
    const BOOL requested = [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() && [value boolValue];
    _logoView.hidden = !requested || _logoView.image.size.width <= 0;
    _label.font = [NSFont systemFontOfSize:kFontSize weight:NSFontWeightRegular];
    _logoWidth.constant = kLogoSide;
    _logoHeight.constant = kLogoSide;
    _content.spacing = kContentSpacing;
    self.contentView.layer.cornerRadius = kCornerRadius;
    [self setContentSize:self.panelSize];
}

- (NSSize)panelSize {
    // 两个字取较宽者，切换模式时窗口宽度保持稳定。
    CGFloat glyph = 0.0;
    for (NSString *text in @[MSIMEInputModeHUDText(NO), MSIMEInputModeHUDText(YES)])
        glyph = std::max(glyph, [text sizeWithAttributes:@{NSFontAttributeName : _label.font}].width);
    const CGFloat logo = _logoView.hidden ? 0.0 : kLogoSide + kContentSpacing;
    return NSMakeSize(std::ceil(2.0 * kHorizontalInset + logo + glyph), kHeight);
}

- (void)setSurfaceColor:(NSColor *)surface borderColor:(NSColor *)border textColor:(NSColor *)text {
    _surfaceColor = [surface copy];
    _borderColor = [border copy];
    _textColor = [text copy];
    [self applyThemeColors];
}

- (void)applyThemePreferences:(NSDictionary *)preferences {
    // 与悬浮工具栏同一条规则：只看 `theme`，固定明暗的主题由 InputController 钉住。
    id resolved = preferences[@"theme"];
    if ([resolved isEqual:@"light"]) self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    else if (resolved == nil || [resolved isEqual:@"system"]) self.appearance = nil;
    else self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    [self applyThemeColors];
}

- (NSColor *)surfaceColor { return _surfaceColor ?: NSColor.windowBackgroundColor; }
- (NSColor *)borderColor { return _borderColor ?: NSColor.separatorColor; }
- (NSColor *)textColor { return _textColor ?: NSColor.labelColor; }

- (void)setLiquidGlassEnabled:(BOOL)enabled {
    _liquidGlassEnabled = enabled;
    [self applyThemeColors];
}

- (void)dealloc {
    [_dismissTimer invalidate];
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
}

// 图层颜色每次显示时按面板当前外观重新解析。
- (void)applyThemeColors {
    [self.effectiveAppearance performAsCurrentDrawingAppearance:^{
        self->_glassBackground = MSIMEUpdateNativeGlassBackground(self.contentView, self->_chrome, self->_glassBackground,
            self.liquidGlassEnabled, kCornerRadius);
        self.contentView.layer.backgroundColor = self->_glassBackground ? NSColor.clearColor.CGColor : self.surfaceColor.CGColor;
        self.contentView.layer.borderColor = self.borderColor.CGColor;
        self.contentView.layer.borderWidth = self->_glassBackground ? 0.0 : kBorderWidth;
        self->_label.textColor = self.textColor;
    }];
}

- (BOOL)showsLogo { return !_logoView.hidden; }
- (NSString *)displayedText { return self.isVisible ? _label.stringValue : nil; }

- (void)showEnglishInputMode:(BOOL)englishInputMode nearCaretRect:(NSRect)caretRect {
    _label.stringValue = MSIMEInputModeHUDText(englishInputMode);
    NSAccessibilityPostNotificationWithUserInfo(self, NSAccessibilityAnnouncementRequestedNotification,
                                                 @{NSAccessibilityAnnouncementKey : englishInputMode ? @"英文输入" : @"中文输入"});
    NSScreen *screen = MSIMEScreenForPoint(NSMakePoint(NSMidX(caretRect), NSMidY(caretRect)));
    NSRect visible = screen ? screen.visibleFrame : NSMakeRect(0, 0, 1440, 900);
    [self setFrame:MSIMEInputModeHUDFrame(caretRect, self.panelSize, visible) display:YES];
    [self applyThemeColors];
    [_dismissTimer invalidate];
    self.alphaValue = 1.0;
    [self orderFrontRegardless];
    __weak MSIMEInputModeHUDPanel *weakSelf = self;
    _dismissTimer = [NSTimer scheduledTimerWithTimeInterval:kVisibleDuration repeats:NO block:^(NSTimer *timer) {
        (void)timer;
        [weakSelf fadeOut];
    }];
}

- (void)fadeOut {
    [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
        context.duration = kFadeDuration;
        self.animator.alphaValue = 0.0;
    } completionHandler:^{
        if (self.alphaValue <= 0.01) [self orderOut:nil];
    }];
}
@end
