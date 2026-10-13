#import "InputModeHUDPanel.h"
#import "../candidate/CandidatePlacement.h"
#import "../candidate/CandidateWindowLevel.h"

#import <algorithm>
#import <cmath>

namespace {
// The badge is the floating toolbar's size, read from the same floating_toolbar.font_size and scale_percent (FloatingToolbarPanel.mm -applySizingPreferences:): the toolbar's (font + 20) x scale height, its 0.95 x font glyphs, its 22pt brand mark and its 10pt native corner radius, all scaled.
constexpr CGFloat kDefaultFontSize = 24.0;
constexpr CGFloat kGlyphScale = 0.95;
constexpr CGFloat kHeightPadding = 20.0;
constexpr CGFloat kLogoSide = 22.0;
constexpr CGFloat kCornerRadius = 10.0;
constexpr CGFloat kHorizontalInset = 12.0;
constexpr CGFloat kContentSpacing = 6.0;
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
    NSImageView *_logoView;
    NSTimer *_dismissTimer;
    NSColor *_surfaceColor;
    NSColor *_borderColor;
    NSColor *_textColor;
    NSStackView *_content;
    NSLayoutConstraint *_logoWidth;
    NSLayoutConstraint *_logoHeight;
    CGFloat _fontSize;
    CGFloat _scale;
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
    _content = [NSStackView stackViewWithViews:logo ? @[_logoView, _label] : @[_label]];
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
    self.contentView = background;
    [self applySizingPreferences:@{}];
    [self applyThemeColors];
    return self;
}

- (void)applySizingPreferences:(NSDictionary *)preferences {
    id toolbar = preferences[@"floating_toolbar"];
    if (![toolbar isKindOfClass:NSDictionary.class]) toolbar = @{};
    id scaleValue = toolbar[@"scale_percent"] ?: @100;
    id fontValue = toolbar[@"font_size"] ?: @(kDefaultFontSize);
    _scale = [@[@75, @100, @125, @150] containsObject:scaleValue] ? [scaleValue doubleValue] / 100.0 : 1.0;
    _fontSize = [@[@16, @18, @20, @22, @24, @26, @28] containsObject:fontValue] ? [fontValue doubleValue] : kDefaultFontSize;
    _label.font = [NSFont systemFontOfSize:_fontSize * _scale * kGlyphScale weight:NSFontWeightSemibold];
    _logoWidth.constant = kLogoSide * _scale;
    _logoHeight.constant = kLogoSide * _scale;
    _content.spacing = kContentSpacing * _scale;
    self.contentView.layer.cornerRadius = kCornerRadius * _scale;
    [self setContentSize:self.panelSize];
}

- (NSSize)panelSize {
    // The wider of the two characters, so switching modes never changes the badge's width.
    CGFloat glyph = 0.0;
    for (NSString *text in @[MSIMEInputModeHUDText(NO), MSIMEInputModeHUDText(YES)])
        glyph = std::max(glyph, [text sizeWithAttributes:@{NSFontAttributeName : _label.font}].width);
    const CGFloat logo = _logoView.hidden ? 0.0 : (kLogoSide + kContentSpacing) * _scale;
    // NSWindow rounds fractional point sizes; round outward so nothing is clipped, as the toolbar does.
    return NSMakeSize(std::ceil(2.0 * kHorizontalInset * _scale + logo + glyph), std::ceil((_fontSize + kHeightPadding) * _scale));
}

- (void)setSurfaceColor:(NSColor *)surface borderColor:(NSColor *)border textColor:(NSColor *)text {
    _surfaceColor = [surface copy];
    _borderColor = [border copy];
    _textColor = [text copy];
    [self applyThemeColors];
}

- (void)applyThemePreferences:(NSDictionary *)preferences {
    id surface = preferences[@"toolbar_theme"];
    id global = preferences[@"theme"];
    id resolved = ([surface isEqual:@"dark"] || [surface isEqual:@"light"]) ? surface : global;
    if ([resolved isEqual:@"light"]) self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    else if (resolved == nil || [resolved isEqual:@"system"]) self.appearance = nil;
    else self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    [self applyThemeColors];
}

- (NSColor *)surfaceColor { return _surfaceColor ?: NSColor.windowBackgroundColor; }
- (NSColor *)borderColor { return _borderColor ?: NSColor.separatorColor; }
- (NSColor *)textColor { return _textColor ?: NSColor.labelColor; }

// Layer colours are resolved once, so they are taken again in the panel's current appearance each time it is shown.
- (void)applyThemeColors {
    [self.effectiveAppearance performAsCurrentDrawingAppearance:^{
        self.contentView.layer.backgroundColor = self.surfaceColor.CGColor;
        self.contentView.layer.borderColor = self.borderColor.CGColor;
        self.contentView.layer.borderWidth = kBorderWidth;
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
