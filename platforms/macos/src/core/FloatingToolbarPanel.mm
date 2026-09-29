#import "FloatingToolbarPanel.h"
#import "../candidate/CandidateSkinAppearance.h"
#import "SupportWindowController.h"
#import "DesktopSettingsLauncher.h"
#import "WindowPresentationLog.h"
#import <CoreGraphics/CoreGraphics.h>

#include <algorithm>
#include <cmath>

// The compact water-fir mark keeps the toolbar identifiable when it is detached from the settings window. It mirrors the shared MSIME app mark without loading an image resource, so it remains crisp at every toolbar scale.
//
// It is also the drag handle, the counterpart of the reference's ToolbarDragHandle: pressing it moves the panel through movableByWindowBackground, and the reference's IDC_SIZEALL cursor maps to the open-hand cursor, the macOS cue for a movable surface. The toolbar used to carry the reference's own handle, a #8E8CD8 bar, beside the logo; with the logo already leading the row that bar was a second mark saying the same thing, so the logo took over its job.
@interface MetasequoiaFloatingToolbarLogoView : NSView
@property(nonatomic) CGFloat scale;
@end
@implementation MetasequoiaFloatingToolbarLogoView
{
    NSImage *_image;
    NSTrackingArea *_trackingArea;
}
- (instancetype)initWithFrame:(NSRect)frameRect
{
    self = [super initWithFrame:frameRect];
    if (self != nil)
    {
        _scale = 1.0;
        NSString *path = [[NSBundle bundleForClass:self.class] pathForResource:@"MSIMEClientInputMethod" ofType:@"icns"];
        _image = path == nil ? nil : [[NSImage alloc] initWithContentsOfFile:path];
        self.accessibilityIdentifier = @"MetasequoiaFloatingToolbarLogo";
        self.accessibilityLabel = @"水杉输入法";
    }
    return self;
}
- (void)setScale:(CGFloat)scale
{
    _scale = scale;
    self.needsDisplay = YES;
}
- (BOOL)mouseDownCanMoveWindow
{
    return YES;
}
- (void)resetCursorRects
{
    [self addCursorRect:self.bounds cursor:NSCursor.openHandCursor];
}
// Cursor rects only apply while the window is key, and this panel never becomes key, so the always-active tracking area sets the cursor as well.
- (void)updateTrackingAreas
{
    if (_trackingArea != nil) [self removeTrackingArea:_trackingArea];
    _trackingArea = [[NSTrackingArea alloc]
        initWithRect:NSZeroRect
             options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect
               owner:self
            userInfo:nil];
    [self addTrackingArea:_trackingArea];
    [super updateTrackingAreas];
}
- (void)mouseEntered:(NSEvent *)event
{
    (void)event;
    [NSCursor.openHandCursor set];
}
- (void)mouseExited:(NSEvent *)event
{
    (void)event;
    [NSCursor.arrowCursor set];
}
- (void)drawRect:(NSRect)dirtyRect
{
    (void)dirtyRect;
    const CGFloat side = std::min(NSWidth(self.bounds), NSHeight(self.bounds)) - 4.0 * _scale;
    const NSRect mark = NSMakeRect(NSMidX(self.bounds) - side * 0.5, NSMidY(self.bounds) - side * 0.5, side, side);
    if (_image != nil)
    {
        [_image drawInRect:mark fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1.0 respectFlipped:YES hints:nil];
        return;
    }
    const CGFloat radius = 4.0 * _scale;
    NSBezierPath *background = [NSBezierPath bezierPathWithRoundedRect:mark xRadius:radius yRadius:radius];
    [[NSColor colorWithSRGBRed:0x25 / 255.0 green:0x25 / 255.0 blue:0x25 / 255.0 alpha:1.0] setFill];
    [background fill];
    [[NSColor colorWithSRGBRed:0xA8 / 255.0 green:0xDF / 255.0 blue:0x8E / 255.0 alpha:1.0] setStroke];
    background.lineWidth = 2.0 * _scale;
    [background stroke];

    NSBezierPath *stroke = [NSBezierPath bezierPath];
    [stroke moveToPoint:NSMakePoint(NSMinX(mark) + side * 0.68, NSMinY(mark) + side * 0.88)];
    [stroke lineToPoint:NSMakePoint(NSMinX(mark) + side * 0.28, NSMinY(mark) + side * 0.73)];
    [stroke lineToPoint:NSMakePoint(NSMinX(mark) + side * 0.68, NSMinY(mark) + side * 0.62)];
    [stroke lineToPoint:NSMakePoint(NSMinX(mark) + side * 0.28, NSMinY(mark) + side * 0.44)];
    [stroke curveToPoint:NSMakePoint(NSMinX(mark) + side * 0.25, NSMinY(mark) + side * 0.16)
           controlPoint1:NSMakePoint(NSMinX(mark) + side * 0.68, NSMinY(mark) + side * 0.38)
           controlPoint2:NSMakePoint(NSMinX(mark) + side * 0.42, NSMinY(mark) + side * 0.25)];
    stroke.lineWidth = 2.5 * _scale;
    stroke.lineCapStyle = NSLineCapStyleRound;
    stroke.lineJoinStyle = NSLineJoinStyleRound;
    [[NSColor whiteColor] setStroke];
    [stroke stroke];
}
@end

// Hairline between the logo and the buttons, the counterpart of the reference's ToolbarDivider.
@interface MetasequoiaFloatingToolbarDivider : NSView
@property(nonatomic, copy) NSColor *fillColor;
@end
@implementation MetasequoiaFloatingToolbarDivider
- (void)setFillColor:(NSColor *)fillColor
{
    _fillColor = [fillColor copy];
    self.needsDisplay = YES;
}
- (void)drawRect:(NSRect)dirtyRect
{
    (void)dirtyRect;
    if (_fillColor == nil) return;
    [_fillColor setFill];
    NSRectFillUsingOperation(self.bounds, NSCompositingOperationSourceOver);
}
@end

// Toolbar button with the reference's ToolbarIconButton hover and pressed fill: a rounded rect of radius max(2, height * 0.25) behind the glyph while the pointer is over it or it is pressed.
@interface MetasequoiaFloatingToolbarButton : NSButton
@property(nonatomic, copy) NSColor *hoverFillColor;
@property(nonatomic) BOOL hovered;
@end
@implementation MetasequoiaFloatingToolbarButton
{
    NSTrackingArea *_trackingArea;
    BOOL _hovered;
}
- (void)setHoverFillColor:(NSColor *)hoverFillColor
{
    _hoverFillColor = [hoverFillColor copy];
    if (_hovered || self.isHighlighted) self.needsDisplay = YES;
}
- (void)setHovered:(BOOL)hovered
{
    if (_hovered == hovered) return;
    _hovered = hovered;
    self.needsDisplay = YES;
}
- (BOOL)hovered
{
    return _hovered;
}
// A click-drag on a button must press it, never move the panel.
- (BOOL)mouseDownCanMoveWindow
{
    return NO;
}
// The panel is non-activating and never key, so only an always-active tracking area reports the pointer.
- (void)updateTrackingAreas
{
    if (_trackingArea != nil) [self removeTrackingArea:_trackingArea];
    _trackingArea = [[NSTrackingArea alloc]
        initWithRect:NSZeroRect
             options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect
               owner:self
            userInfo:nil];
    [self addTrackingArea:_trackingArea];
    [super updateTrackingAreas];
}
- (void)mouseEntered:(NSEvent *)event
{
    (void)event;
    [self setHovered:YES];
}
- (void)mouseExited:(NSEvent *)event
{
    (void)event;
    [self setHovered:NO];
}
- (void)setHidden:(BOOL)hidden
{
    [super setHidden:hidden];
    if (hidden) [self setHovered:NO];
}
- (void)drawRect:(NSRect)dirtyRect
{
    if ((_hovered || self.isHighlighted) && _hoverFillColor != nil)
    {
        const CGFloat radius = std::max<CGFloat>(2.0, NSHeight(self.bounds) * 0.25);
        [_hoverFillColor setFill];
        [[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:radius yRadius:radius] fill];
    }
    [super drawRect:dirtyRect];
}
@end

namespace
{
// Unscaled leading run: the 34pt logo and its 4pt gap, which together are the drag handle, a further 3pt, the 1.2pt divider, then the 4pt gap before the first button. It replaces the plain 10pt leading inset.
constexpr CGFloat kToolbarLogoWidth = 34.0;
constexpr CGFloat kToolbarLogoGap = 4.0;
constexpr CGFloat kToolbarLogoDividerGap = 3.0;
constexpr CGFloat kToolbarDividerWidth = 1.2;
constexpr CGFloat kToolbarDividerButtonGap = 4.0;
constexpr CGFloat kToolbarLeadingChrome = kToolbarLogoWidth + kToolbarLogoGap + kToolbarLogoDividerGap + kToolbarDividerWidth + kToolbarDividerButtonGap;
// Unscaled trailing run: the 6pt trailing inset. The stack gets no slack to spread, so its gaps stay at kToolbarButtonSpacing.
constexpr CGFloat kToolbarTrailingChrome = 6.0;
constexpr CGFloat kToolbarButtonSpacing = 2.0;
// A button is its glyph plus 4pt a side, and the glyph is 0.95 of the chosen font size, in the regular weight that the SF Symbol buttons beside it use. Wider buttons and 8pt gaps left the characters so far apart that the toolbar covered more of the text it sits over than its five buttons needed.
constexpr CGFloat kToolbarButtonPadding = 8.0;
constexpr CGFloat kToolbarGlyphScale = 0.95;

CGFloat ToolbarPreferredWidth(NSUInteger count, CGFloat fontSize, CGFloat scale)
{
    const CGFloat buttons = static_cast<CGFloat>(count);
    const CGFloat gaps = count > 0 ? static_cast<CGFloat>(count - 1) : 0.0;
    // NSWindow rounds fractional point sizes; round outward so controls are never clipped.
    return std::ceil((buttons * (fontSize + kToolbarButtonPadding) + gaps * kToolbarButtonSpacing + kToolbarTrailingChrome + kToolbarLeadingChrome) * scale);
}

// Default row: the five buttons a profile that has not chosen gets - 中/英, punctuation, full width,
// simplified/traditional and settings - at 24pt and 100%. Emoji, handwriting, voice and the screen
// keyboard are opt-in (see FloatingToolbarPreferences::default() in crates/client-core). This is the
// size the window opens at, before any preferences are applied, so a wider value here would show a
// toolbar that immediately shrinks.
constexpr CGFloat kToolbarWidth = 221.0;
static_assert(kToolbarWidth >= 5 * (24.0 + kToolbarButtonPadding) + 4 * kToolbarButtonSpacing + kToolbarTrailingChrome + kToolbarLeadingChrome &&
                  kToolbarWidth < 5 * (24.0 + kToolbarButtonPadding) + 4 * kToolbarButtonSpacing + kToolbarTrailingChrome + kToolbarLeadingChrome + 1.0,
              "kToolbarWidth must be ToolbarPreferredWidth(5, 24, 1)");
constexpr CGFloat kToolbarHeight = 44.0;
NSString *const kToolbarFrameAutosaveName = @"MetasequoiaFloatingToolbarFrame";

NSButton *ToolbarButton(NSString *title, NSString *identifier, id target, SEL action)
{
    MetasequoiaFloatingToolbarButton *button = [MetasequoiaFloatingToolbarButton buttonWithTitle:title target:target action:action];
    button.translatesAutoresizingMaskIntoConstraints = NO;
    button.bordered = NO;
    button.font = [NSFont systemFontOfSize:15.0 weight:NSFontWeightRegular];
    button.accessibilityIdentifier = identifier;
    NSLayoutConstraint *width = [button.widthAnchor constraintEqualToConstant:24.0 + kToolbarButtonPadding];
    width.identifier = @"ToolbarButtonWidth";
    width.active = YES;
    NSLayoutConstraint *height = [button.heightAnchor constraintEqualToConstant:32.0];
    height.identifier = @"ToolbarButtonHeight";
    height.active = YES;
    return button;
}

NSScreen *ScreenContainingFrame(NSRect frame)
{
    NSScreen *bestScreen = nil;
    CGFloat bestArea = 0.0;
    for (NSScreen *screen in NSScreen.screens)
    {
        NSRect intersection = NSIntersectionRect(frame, screen.frame);
        CGFloat area = intersection.size.width * intersection.size.height;
        if (area > bestArea)
        {
            bestArea = area;
            bestScreen = screen;
        }
    }
    return bestScreen;
}

NSScreen *ScreenContainingMouse()
{
    NSPoint mouseLocation = NSEvent.mouseLocation;
    for (NSScreen *screen in NSScreen.screens)
    {
        if (NSPointInRect(mouseLocation, screen.frame))
        {
            return screen;
        }
    }
    return NSScreen.mainScreen;
}
} // namespace

BOOL MetasequoiaFloatingToolbarShouldShow(BOOL configuredEnabled, BOOL imeActive, BOOL fullscreen)
{
    return configuredEnabled && imeActive && !fullscreen;
}

static BOOL __attribute__((unused)) FrontmostApplicationOwnsFullscreenDisplay(void)
{
    NSRunningApplication *frontmost = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (frontmost == nil || frontmost == NSRunningApplication.currentApplication)
    {
        return NO;
    }

    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
    if (![windows isKindOfClass:NSArray.class])
    {
        return NO;
    }

    const pid_t pid = frontmost.processIdentifier;
    for (NSDictionary *window in windows)
    {
        if (![window isKindOfClass:NSDictionary.class] ||
            [window[(id)kCGWindowOwnerPID] intValue] != pid ||
            [window[(id)kCGWindowLayer] integerValue] != 0)
        {
            continue;
        }
        CGRect bounds = CGRectZero;
        if (!CGRectMakeWithDictionaryRepresentation((CFDictionaryRef)window[(id)kCGWindowBounds], &bounds))
        {
            continue;
        }
        for (NSScreen *screen in NSScreen.screens)
        {
            // CGWindow and CGDisplay bounds share Quartz' global display
            // coordinate space. Prefer the display rectangle over NSScreen's
            // AppKit frame so the origin is checked as well as the size; a
            // maximized or partially off-screen window must not hide the
            // toolbar merely because it is large enough.
            NSNumber *number = screen.deviceDescription[@"NSScreenNumber"];
            CGRect display = number != nil ? CGDisplayBounds((CGDirectDisplayID)number.unsignedIntValue)
                                           : NSRectToCGRect(screen.frame);
            if (MetasequoiaWindowCoversDisplay(bounds, display)) return YES;
        }
    }
    return NO;
}

BOOL MetasequoiaWindowCoversDisplay(CGRect windowBounds, CGRect displayBounds)
{
    if (!std::isfinite(windowBounds.origin.x) || !std::isfinite(windowBounds.origin.y) ||
        !std::isfinite(windowBounds.size.width) || !std::isfinite(windowBounds.size.height) ||
        !std::isfinite(displayBounds.origin.x) || !std::isfinite(displayBounds.origin.y) ||
        !std::isfinite(displayBounds.size.width) || !std::isfinite(displayBounds.size.height) ||
        CGRectIsEmpty(windowBounds) || CGRectIsEmpty(displayBounds))
        return NO;
    constexpr CGFloat tolerance = 2.0;
    return CGRectGetMinX(windowBounds) <= CGRectGetMinX(displayBounds) + tolerance &&
           CGRectGetMinY(windowBounds) <= CGRectGetMinY(displayBounds) + tolerance &&
           CGRectGetMaxX(windowBounds) >= CGRectGetMaxX(displayBounds) - tolerance &&
           CGRectGetMaxY(windowBounds) >= CGRectGetMaxY(displayBounds) - tolerance;
}

static NSRect SizedToolbarFrame(NSRect proposedFrame, NSRect visibleFrame, BOOL hasSavedFrame, NSSize size)
{
    constexpr CGFloat kDefaultMargin = 20.0;
    constexpr CGFloat kRestoredMargin = 12.0;
    proposedFrame.size = size;
    if (!hasSavedFrame)
    {
        proposedFrame.origin.x = NSMaxX(visibleFrame) - proposedFrame.size.width - kDefaultMargin;
        proposedFrame.origin.y = NSMinY(visibleFrame) + kDefaultMargin;
        return proposedFrame;
    }

    CGFloat minimumX = NSMinX(visibleFrame) + kRestoredMargin;
    CGFloat maximumX = NSMaxX(visibleFrame) - proposedFrame.size.width - kRestoredMargin;
    CGFloat minimumY = NSMinY(visibleFrame) + kRestoredMargin;
    CGFloat maximumY = NSMaxY(visibleFrame) - proposedFrame.size.height - kRestoredMargin;
    proposedFrame.origin.x = std::clamp(proposedFrame.origin.x, minimumX, std::max(minimumX, maximumX));
    proposedFrame.origin.y = std::clamp(proposedFrame.origin.y, minimumY, std::max(minimumY, maximumY));
    return proposedFrame;
}

NSRect MetasequoiaFloatingToolbarFrame(NSRect proposedFrame, NSRect visibleFrame, BOOL hasSavedFrame)
{
    return SizedToolbarFrame(proposedFrame, visibleFrame, hasSavedFrame, NSMakeSize(kToolbarWidth, kToolbarHeight));
}

NSMenu *CreateMetasequoiaFloatingToolbarUtilityMenu(id target)
{
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"水杉输入法"];
    // Every item targets the panel, an NSWindow subclass, and NSMenu's automatic enabling asks
    // NSWindow's own -validateMenuItem: about each one. NSWindow implements -hideToolbar: for real
    // toolbars and answers NO when the window has none, which greyed out 隐藏悬浮状态栏 and swallowed
    // the click. The input menu already opts out of automatic enabling for the same reason.
    menu.autoenablesItems = NO;
    for (NSMenuItem *item in @[
             [[NSMenuItem alloc] initWithTitle:@"表情与符号…"
                                        action:@selector(openCharacterPalette:)
                                 keyEquivalent:@""],
             [[NSMenuItem alloc] initWithTitle:@"打开设置…" action:@selector(openSettings:) keyEquivalent:@""],
             [[NSMenuItem alloc] initWithTitle:@"检查更新…" action:@selector(checkForUpdates:) keyEquivalent:@""],
         ])
    {
        item.target = target;
        item.enabled = YES;
        [menu addItem:item];
    }
    [menu addItem:[NSMenuItem separatorItem]];
    NSMenuItem *website = [[NSMenuItem alloc] initWithTitle:@"访问 msime.app"
                                                     action:@selector(openWebsite:)
                                              keyEquivalent:@""];
    website.target = target;
    website.enabled = YES;
    [menu addItem:website];
    for (NSMenuItem *item in @[
             [[NSMenuItem alloc] initWithTitle:@"使用帮助…" action:@selector(openHelp:) keyEquivalent:@""],
             [[NSMenuItem alloc] initWithTitle:@"关于水杉输入法…" action:@selector(openAbout:) keyEquivalent:@""],
             [[NSMenuItem alloc] initWithTitle:@"问题反馈…" action:@selector(openFeedback:) keyEquivalent:@""],
         ])
    {
        item.target = target;
        item.enabled = YES;
        [menu addItem:item];
    }
    [menu addItem:[NSMenuItem separatorItem]];
    NSMenuItem *hide = [[NSMenuItem alloc] initWithTitle:@"隐藏悬浮状态栏"
                                                  action:@selector(dismissFloatingToolbar:)
                                           keyEquivalent:@""];
    hide.target = target;
    hide.enabled = YES;
    [menu addItem:hide];
    return menu;
}

@interface MetasequoiaFloatingToolbarChromeView : NSView
@property(nonatomic, weak) id appearanceTarget;
@property(nonatomic) SEL appearanceAction;
@property(nonatomic, copy) NSColor *fillColor;
@property(nonatomic, copy) NSColor *strokeColor;
@end
@implementation MetasequoiaFloatingToolbarChromeView
- (void)viewDidChangeEffectiveAppearance
{
    [super viewDidChangeEffectiveAppearance];
    if (self.appearanceTarget != nil && self.appearanceAction != nullptr)
    {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Warc-performSelector-leaks"
        [self.appearanceTarget performSelector:self.appearanceAction];
#pragma clang diagnostic pop
    }
}
- (void)drawRect:(NSRect)dirtyRect
{
    (void)dirtyRect;
    const CGFloat radius = self.layer.cornerRadius;
    NSBezierPath *path = [NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:radius yRadius:radius];
    [(self.fillColor != nil ? self.fillColor : NSColor.windowBackgroundColor) setFill];
    [path fill];
    if (self.strokeColor.alphaComponent > 0.01)
    {
        path.lineWidth = 1.0;
        [self.strokeColor setStroke];
        [path stroke];
    }
}
@end

// Every toolbar action, and whether anything was behind it: a nil delegate sends the message nowhere, which is indistinguishable on screen from a window that opened behind the editor.
static void MSIMELogToolbarAction(const char *action, BOOL hasDelegate, id sender)
{
    os_log(MSIMEUILog(), "toolbar_action action=%{public}s delegate=%d from=%{public}s", action, hasDelegate,
           [sender isKindOfClass:NSMenuItem.class] ? "menu" : "button");
}

@implementation MetasequoiaFloatingToolbarPanel
{
    MetasequoiaFloatingToolbarChromeView *_chrome;
    NSButton *_inputModeButton;
    NSButton *_punctuationButton;
    NSButton *_fullWidthButton;
    NSButton *_traditionalOutputButton;
    NSButton *_emojiButton;
    NSButton *_handwritingButton;
    NSButton *_keyboardButton;
    NSButton *_voiceButton;
    NSButton *_settingsButton;
    NSStackView *_actions;
    MetasequoiaFloatingToolbarLogoView *_logo;
    MetasequoiaFloatingToolbarDivider *_divider;
    NSLayoutConstraint *_logoWidth;
    NSLayoutConstraint *_logoDividerGap;
    NSLayoutConstraint *_dividerWidth;
    NSLayoutConstraint *_dividerHeight;
    NSLayoutConstraint *_dividerButtonGap;
    NSLayoutConstraint *_trailingInset;
    NSSize _preferredSize;
    CGFloat _appliedScale;
    CGFloat _appliedFontSize;
    NSUInteger _appliedComponentMask;
    BOOL _hasHostSkin;
    msime::mac::SkinTokens _lightSkin;
    msime::mac::SkinTokens _darkSkin;
    BOOL _hasHostToolbarSkin;
    msime::mac::SkinTokens _lightToolbarSkin;
    msime::mac::SkinTokens _darkToolbarSkin;
    BOOL _requestedVisible;
    BOOL _imeActive;
    BOOL _idleHidden;
    BOOL _recentInput;
    NSTimer *_idleTimer;
}

+ (instancetype)sharedPanel
{
    static MetasequoiaFloatingToolbarPanel *panel = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
      panel = [[MetasequoiaFloatingToolbarPanel alloc] init];
    });
    return panel;
}

- (instancetype)init
{
    self = [super initWithContentRect:NSMakeRect(0.0, 0.0, kToolbarWidth, kToolbarHeight)
                            styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                              backing:NSBackingStoreBuffered
                                defer:NO];
    if (self == nil)
    {
        return nil;
    }

    self.level = NSFloatingWindowLevel;
    self.ignoresMouseEvents = NO;
    _preferredSize = NSMakeSize(kToolbarWidth, kToolbarHeight);
    self.opaque = NO;
    self.backgroundColor = [NSColor clearColor];
    self.hasShadow = YES;
    self.hidesOnDeactivate = NO;
    self.becomesKeyOnlyIfNeeded = YES;
    self.movableByWindowBackground = YES;
    // Keep the toolbar on ordinary Spaces without opting it into another
    // app's full-screen Space. Candidate and panel windows remain auxiliary;
    // the toolbar itself should disappear while a full-screen app owns the
    // display, matching the Windows foreground policy.
    self.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
    [self setFrameAutosaveName:kToolbarFrameAutosaveName];
    // The autosave name only writes the frame out; a programmatically created window has to read it back itself, and
    // force: is required because this panel is borderless and therefore not resizable.
    [self setFrameUsingName:kToolbarFrameAutosaveName force:YES];

    _chrome = [[MetasequoiaFloatingToolbarChromeView alloc] initWithFrame:self.contentView.bounds];
    _chrome.appearanceTarget = self;
    _chrome.appearanceAction = @selector(applySkin);
    _chrome.wantsLayer = YES;
    _chrome.layer.cornerRadius = 10.0;
    _chrome.layer.masksToBounds = YES;
    self.contentView = _chrome;

    _inputModeButton = ToolbarButton(@"中", @"MetasequoiaFloatingToolbarInputMode", self, @selector(toggleInputMode:));
    _punctuationButton =
        ToolbarButton(@"。", @"MetasequoiaFloatingToolbarPunctuation", self, @selector(togglePunctuation:));
    _fullWidthButton = ToolbarButton(@"半", @"MetasequoiaFloatingToolbarFullWidth", self, @selector(toggleFullWidth:));
    _traditionalOutputButton =
        ToolbarButton(@"简", @"MetasequoiaFloatingToolbarTraditionalOutput", self, @selector(toggleTraditionalOutput:));
    _emojiButton = ToolbarButton(@"", @"MetasequoiaFloatingToolbarEmoji", self, @selector(openEmoji:));
    _emojiButton.image = [NSImage imageWithSystemSymbolName:@"face.smiling" accessibilityDescription:@"表情"];
    _emojiButton.accessibilityLabel = @"打开水杉表情面板";
    _emojiButton.toolTip = _emojiButton.accessibilityLabel;
    _handwritingButton = ToolbarButton(@"", @"MetasequoiaFloatingToolbarHandwriting", self, @selector(openHandwriting:));
    _handwritingButton.image = [NSImage imageWithSystemSymbolName:@"hand.draw" accessibilityDescription:@"手写"];
    _handwritingButton.accessibilityLabel = @"打开水杉手写识别板";
    _handwritingButton.toolTip = _handwritingButton.accessibilityLabel;
    _keyboardButton = ToolbarButton(@"", @"MetasequoiaFloatingToolbarScreenKeyboard", self, @selector(openScreenKeyboard:));
    _keyboardButton.image = [NSImage imageWithSystemSymbolName:@"keyboard" accessibilityDescription:@"屏幕键盘"];
    _keyboardButton.accessibilityLabel = @"打开水杉屏幕键盘";
    _keyboardButton.toolTip = _keyboardButton.accessibilityLabel;
    _voiceButton = ToolbarButton(@"", @"MetasequoiaFloatingToolbarVoice", self, @selector(toggleVoice:));
    _voiceButton.image = [NSImage imageWithSystemSymbolName:@"mic.fill" accessibilityDescription:@"语音输入"];
    _voiceButton.accessibilityLabel = @"开始或结束语音输入";
    _voiceButton.toolTip = _voiceButton.accessibilityLabel;
    // A click opens settings directly; the utility menu (updates, help, hiding the toolbar) stays reachable on right-click / control-click.
    _settingsButton = ToolbarButton(@"", @"MetasequoiaFloatingToolbarSettings", self, @selector(openSettings:));
    _settingsButton.image = [NSImage imageWithSystemSymbolName:@"gearshape" accessibilityDescription:@"设置"];
    _settingsButton.menu = CreateMetasequoiaFloatingToolbarUtilityMenu(self);
    _settingsButton.accessibilityLabel = @"打开水杉输入法设置";
    _settingsButton.toolTip = _settingsButton.accessibilityLabel;

    NSStackView *actions = [NSStackView stackViewWithViews:@[
        _inputModeButton, _punctuationButton, _fullWidthButton, _traditionalOutputButton, _emojiButton,
        _handwritingButton, _keyboardButton, _voiceButton, _settingsButton
    ]];
    actions.translatesAutoresizingMaskIntoConstraints = NO;
    actions.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    actions.alignment = NSLayoutAttributeCenterY;
    actions.distribution = NSStackViewDistributionEqualSpacing;
    actions.spacing = kToolbarButtonSpacing;
    _actions = actions;
    [_chrome addSubview:actions];

    // The reference lays out handle, divider, then icons from the left edge, and its hit test makes the handle the caption drag region; here the logo is the handle.
    _logo = [[MetasequoiaFloatingToolbarLogoView alloc] initWithFrame:NSZeroRect];
    _logo.translatesAutoresizingMaskIntoConstraints = NO;
    [_chrome addSubview:_logo];
    _divider = [[MetasequoiaFloatingToolbarDivider alloc] initWithFrame:NSZeroRect];
    _divider.translatesAutoresizingMaskIntoConstraints = NO;
    _divider.accessibilityIdentifier = @"MetasequoiaFloatingToolbarDivider";
    [_chrome addSubview:_divider];

    _logoWidth = [_logo.widthAnchor constraintEqualToConstant:kToolbarLogoWidth + kToolbarLogoGap];
    _logoDividerGap = [_divider.leadingAnchor constraintEqualToAnchor:_logo.trailingAnchor constant:kToolbarLogoDividerGap];
    _dividerWidth = [_divider.widthAnchor constraintEqualToConstant:kToolbarDividerWidth];
    _dividerHeight = [_divider.heightAnchor constraintEqualToConstant:32.0];
    _dividerButtonGap = [actions.leadingAnchor constraintEqualToAnchor:_divider.trailingAnchor constant:kToolbarDividerButtonGap];
    _trailingInset = [actions.trailingAnchor constraintEqualToAnchor:_chrome.trailingAnchor constant:-kToolbarTrailingChrome];
    [NSLayoutConstraint activateConstraints:@[
        [_logo.leadingAnchor constraintEqualToAnchor:_chrome.leadingAnchor],
        [_logo.topAnchor constraintEqualToAnchor:_chrome.topAnchor],
        [_logo.bottomAnchor constraintEqualToAnchor:_chrome.bottomAnchor],
        _logoWidth,
        _logoDividerGap,
        _dividerWidth,
        _dividerHeight,
        [_divider.centerYAnchor constraintEqualToAnchor:_chrome.centerYAnchor],
        _dividerButtonGap,
        _trailingInset,
        [actions.centerYAnchor constraintEqualToAnchor:_chrome.centerYAnchor],
    ]];
    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(applySkin)
                                                 name:MetasequoiaCandidateSkinDidChangeNotification
                                               object:nil];
    [[NSWorkspace sharedWorkspace].notificationCenter addObserver:self
                                                          selector:@selector(refreshVisibility)
                                                              name:NSWorkspaceDidActivateApplicationNotification
                                                            object:nil];
    [[NSNotificationCenter defaultCenter] addObserver:self
                                             selector:@selector(refreshVisibility)
                                                 name:NSApplicationDidChangeScreenParametersNotification
                                               object:nil];
    [self applySizingPreferences:@{}];
    [self updateEnglishInputMode:NO
              japaneseInputMode:NO
                       capsLock:NO
              chinesePunctuationEnabled:YES
                       fullWidthEnabled:NO
        traditionalChineseOutputEnabled:NO];
    return self;
}

- (void)dealloc
{
    [_idleTimer invalidate];
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    [[NSWorkspace sharedWorkspace].notificationCenter removeObserver:self];
}

- (void)refreshVisibility
{
    // The toolbar stays resident across client focus-outs, so its weak owner can be freed while it is on screen (the client app quit). Every application activation re-runs this check, which hides the toolbar instead of leaving buttons with nobody behind them.
    if (self.toolbarDelegate == nil)
    {
        if (self.isVisible) os_log(MSIMEUILog(), "toolbar_hidden reason=owner_released");
        [self orderOut:nil];
        return;
    }
    const BOOL show = MetasequoiaFloatingToolbarShouldShow(
        _requestedVisible && !_idleHidden, _imeActive, NO);
    if (!show)
    {
        [self orderOut:nil];
        return;
    }
    if (self.visible)
    {
        [self orderFrontRegardless];
        return;
    }
    BOOL hasSavedFrame = [[NSUserDefaults standardUserDefaults]
        objectForKey:[@"NSWindow Frame " stringByAppendingString:kToolbarFrameAutosaveName]] != nil;
    NSScreen *screen = hasSavedFrame ? ScreenContainingFrame(self.frame) : ScreenContainingMouse();
    if (screen == nil) screen = NSScreen.mainScreen;
    if (screen != nil) [self setFrame:SizedToolbarFrame(self.frame, screen.visibleFrame, hasSavedFrame, _preferredSize) display:NO];
    [self orderFrontRegardless];
}

- (void)applySizingPreferences:(NSDictionary *)preferences
{
    id toolbar = preferences[@"floating_toolbar"];
    if (![toolbar isKindOfClass:NSDictionary.class]) toolbar = @{};
    id scaleValue = toolbar[@"scale_percent"] ?: @100;
    id fontValue = toolbar[@"font_size"] ?: @24;
    const CGFloat scale = [@[@75, @100, @125, @150] containsObject:scaleValue] ? [scaleValue doubleValue] / 100.0 : 1.0;
    const CGFloat fontSize = [@[@16, @18, @20, @22, @24, @26, @28] containsObject:fontValue] ? [fontValue doubleValue] : 24.0;
    NSArray<NSString *> *keys = @[@"english_mode", @"punctuation", @"fullwidth", @"character_set", @"emoji", @"handwriting", @"screen_keyboard", @"voice", @"settings"];
    NSArray<NSButton *> *optionalButtons = @[_inputModeButton, _punctuationButton, _fullWidthButton, _traditionalOutputButton, _emojiButton, _handwritingButton, _keyboardButton, _voiceButton, _settingsButton];
    NSUInteger mask = 0;
    // Every button on this toolbar can now be turned off; the row can be empty.
    NSUInteger count = 0;
    for (NSUInteger index = 0; index < keys.count; ++index) {
        id value = toolbar[keys[index]];
        // Keep optional utility buttons off when an older or partial settings
        // snapshot omits their keys. They can still be enabled explicitly.
        const BOOL defaultEnabled = ![@[@"emoji", @"handwriting", @"voice", @"screen_keyboard"] containsObject:keys[index]];
        const BOOL enabled = [value isKindOfClass:NSNumber.class] ? [value boolValue] : defaultEnabled;
        if (enabled) { mask |= 1u << index; ++count; }
    }
    if (scale == _appliedScale && fontSize == _appliedFontSize && mask == _appliedComponentMask) return;
    _appliedScale = scale;
    _appliedFontSize = fontSize;
    _appliedComponentMask = mask;
    for (NSUInteger index = 0; index < optionalButtons.count; ++index)
        optionalButtons[index].hidden = (mask & (1u << index)) == 0;
    for (NSButton *button in @[_inputModeButton, _punctuationButton, _fullWidthButton, _traditionalOutputButton, _emojiButton, _handwritingButton, _keyboardButton, _voiceButton, _settingsButton]) {
        for (NSLayoutConstraint *constraint in button.constraints) {
            if (constraint.firstItem != button || constraint.secondItem != nil) continue;
            if ([constraint.identifier isEqualToString:@"ToolbarButtonWidth"]) constraint.constant = (fontSize + kToolbarButtonPadding) * scale;
            if ([constraint.identifier isEqualToString:@"ToolbarButtonHeight"]) constraint.constant = (fontSize + 8.0) * scale;
        }
        button.font = [NSFont systemFontOfSize:fontSize * scale * kToolbarGlyphScale weight:NSFontWeightRegular];
    }
    _settingsButton.symbolConfiguration = [NSImageSymbolConfiguration configurationWithPointSize:fontSize * scale weight:NSFontWeightRegular];
    _emojiButton.symbolConfiguration = _settingsButton.symbolConfiguration;
    _handwritingButton.symbolConfiguration = _settingsButton.symbolConfiguration;
    _keyboardButton.symbolConfiguration = _settingsButton.symbolConfiguration;
    _voiceButton.symbolConfiguration = _settingsButton.symbolConfiguration;
    _actions.spacing = kToolbarButtonSpacing * scale;
    _logo.scale = scale;
    _logoWidth.constant = (kToolbarLogoWidth + kToolbarLogoGap) * scale;
    _logoDividerGap.constant = kToolbarLogoDividerGap * scale;
    _dividerWidth.constant = kToolbarDividerWidth * scale;
    _dividerHeight.constant = (fontSize + 8.0) * scale;
    _dividerButtonGap.constant = kToolbarDividerButtonGap * scale;
    // The logo stays even with every button off, as the reference always keeps its handle, so the panel can still be dragged.
    _divider.hidden = count == 0;
    _trailingInset.constant = -kToolbarTrailingChrome * scale;
    _chrome.layer.cornerRadius = 10.0 * scale;
    // NSWindow rounds fractional point sizes; round outward so controls are never clipped.
    _preferredSize = NSMakeSize(ToolbarPreferredWidth(count, fontSize, scale), std::ceil((fontSize + 20.0) * scale));
    // Only touch the frame once the toolbar is actually on screen. This runs on every shared preference
    // change, hidden or not, and the window carries a frame autosave name - so resizing a hidden toolbar
    // wrote a saved frame for a window the user has never placed, pinned to the restored-margin corner.
    // setVisible: then read that as "the user put it there" and restored the corner instead of the default
    // bottom-right placement on the screen holding the pointer. It sizes from _preferredSize itself, so
    // leaving the frame alone here costs nothing.
    if (self.visible) {
        NSRect frame = self.frame;
        frame.size = _preferredSize;
        NSScreen *screen = ScreenContainingFrame(frame) ?: NSScreen.mainScreen;
        if (screen) frame = SizedToolbarFrame(frame, screen.visibleFrame, YES, _preferredSize);
        [self setFrame:frame display:YES];
    }
    [_chrome layoutSubtreeIfNeeded];
    [self applySkin];
}

- (void)applyThemePreferences:(NSDictionary *)preferences
{
    NSString *surface = preferences[@"toolbar_theme"];
    NSString *mode = preferences[@"theme"];
    NSString *resolved = ([surface isEqual:@"dark"] || [surface isEqual:@"light"]) ? surface : mode;
    if ([resolved isEqual:@"light"])
        self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    else if (resolved == nil || [resolved isEqual:@"system"])
        self.appearance = nil;
    else
        self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    [self applySkin];
}

- (void)applyLightSkin:(const msime::mac::SkinTokens &)light darkSkin:(const msime::mac::SkinTokens &)dark
{
    _lightSkin = light;
    _darkSkin = dark;
    _hasHostSkin = YES;
    [self applySkin];
}

- (void)applyLightToolbarSkin:(const msime::mac::SkinTokens &)light darkSkin:(const msime::mac::SkinTokens &)dark
{
    _lightToolbarSkin = light;
    _darkToolbarSkin = dark;
    _hasHostToolbarSkin = YES;
    [self applySkin];
}

- (void)applySkin
{
    if (_inputModeButton == nil || _settingsButton == nil)
    {
        return;
    }
    const BOOL dark = MetasequoiaAppearanceIsDark(_chrome.effectiveAppearance);
    const auto tokens = _hasHostToolbarSkin ? (dark ? _darkToolbarSkin : _lightToolbarSkin)
        : (_hasHostSkin ? (dark ? _darkSkin : _lightSkin) : MetasequoiaResolveStoredCandidateSkin(dark).tokens);
    _chrome.fillColor = MetasequoiaColorFromRgba(tokens.surface);
    _chrome.strokeColor = MetasequoiaColorFromRgba(tokens.border);
    NSColor *text = MetasequoiaColorFromRgba(tokens.text);
    // The reference's native presenter constants (ApplyTheme). The skin's hover token is a candidate-row colour, opaque in the default dark skin, so it would cover the glyph.
    NSColor *hoverFill = dark ? [NSColor colorWithSRGBRed:1.0 green:1.0 blue:1.0 alpha:0.10]
                              : [NSColor colorWithSRGBRed:0.0 green:0.0 blue:0.0 alpha:0.08];
    _divider.fillColor = dark ? [NSColor colorWithSRGBRed:1.0 green:1.0 blue:1.0 alpha:0.15]
                              : [NSColor colorWithSRGBRed:0.0 green:0.0 blue:0.0 alpha:0.12];
    for (MetasequoiaFloatingToolbarButton *button in
         @[ _inputModeButton, _punctuationButton, _fullWidthButton, _traditionalOutputButton, _emojiButton, _handwritingButton, _keyboardButton, _voiceButton, _settingsButton ])
    {
        button.contentTintColor = text;
        button.hoverFillColor = hoverFill;
        if (button.title.length > 0)
        {
            button.attributedTitle = [[NSAttributedString alloc] initWithString:button.title
                                                                     attributes:@{
                                                                         NSFontAttributeName : button.font,
                                                                         NSForegroundColorAttributeName : text,
                                                                     }];
        }
    }
    _chrome.needsDisplay = YES;
}

- (BOOL)canBecomeKeyWindow
{
    return NO;
}

- (void)orderOut:(id)sender
{
    [super orderOut:sender];
    if (_inputModeButton == nil || _settingsButton == nil) return;
    // A hidden window gets no mouseExited:, so a button hovered at the moment the toolbar hides would come back highlighted.
    for (MetasequoiaFloatingToolbarButton *button in
         @[ _inputModeButton, _punctuationButton, _fullWidthButton, _traditionalOutputButton, _emojiButton, _handwritingButton, _keyboardButton, _voiceButton, _settingsButton ])
        [button setHovered:NO];
}

- (void)updateEnglishInputMode:(BOOL)englishInputMode
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled
{
    [self updateEnglishInputMode:englishInputMode
             englishCandidateMode:NO
              japaneseInputMode:NO
                       capsLock:NO
              chinesePunctuationEnabled:chinesePunctuationEnabled
                       fullWidthEnabled:fullWidthEnabled
        traditionalChineseOutputEnabled:traditionalChineseOutputEnabled];
}

- (void)updateEnglishInputMode:(BOOL)englishInputMode
             japaneseInputMode:(BOOL)japaneseInputMode
                      capsLock:(BOOL)capsLock
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled
{
    [self updateEnglishInputMode:englishInputMode
             englishCandidateMode:NO
              japaneseInputMode:japaneseInputMode
                       capsLock:capsLock
              chinesePunctuationEnabled:chinesePunctuationEnabled
                       fullWidthEnabled:fullWidthEnabled
        traditionalChineseOutputEnabled:traditionalChineseOutputEnabled];
}

- (void)updateEnglishInputMode:(BOOL)englishInputMode
         englishCandidateMode:(BOOL)englishCandidateMode
             japaneseInputMode:(BOOL)japaneseInputMode
                      capsLock:(BOOL)capsLock
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled
{
    NSString *inputModeTitle = capsLock ? @"A" :
        (englishInputMode ? @"英" : (englishCandidateMode ? @"En" : (japaneseInputMode ? @"日" : @"中")));
    _inputModeButton.title = inputModeTitle;
    _inputModeButton.accessibilityLabel = englishInputMode || englishCandidateMode ? @"切换到中文输入" : @"切换到英文输入";
    _punctuationButton.title = chinesePunctuationEnabled ? @"。" : @".";
    _punctuationButton.accessibilityLabel = chinesePunctuationEnabled ? @"切换到西文标点" : @"切换到中文标点";
    _fullWidthButton.title = fullWidthEnabled ? @"全" : @"半";
    _fullWidthButton.accessibilityLabel = fullWidthEnabled ? @"切换到半角输入" : @"切换到全角输入";
    _traditionalOutputButton.title = traditionalChineseOutputEnabled ? @"繁" : @"简";
    _traditionalOutputButton.accessibilityLabel =
        traditionalChineseOutputEnabled ? @"切换到简体输出" : @"切换到繁体输出";
    _inputModeButton.toolTip = _inputModeButton.accessibilityLabel;
    _punctuationButton.toolTip = _punctuationButton.accessibilityLabel;
    _fullWidthButton.toolTip = _fullWidthButton.accessibilityLabel;
    _traditionalOutputButton.toolTip = _traditionalOutputButton.accessibilityLabel;
    [self applySkin];
}

- (void)activateForDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate visible:(BOOL)visible
{
    const BOOL newlyActive = !_imeActive;
    const BOOL ownerChanged = self.toolbarDelegate != delegate;
    self.toolbarDelegate = delegate;
    _imeActive = YES;
    _requestedVisible = visible;
    // Focus activation is also used when the current owner moves between
    // documents. Preserve an idle timeout across that routine callback; only
    // a genuinely new activation or owner is allowed to wake the toolbar.
    if ((newlyActive || ownerChanged) && visible) [self noteInputForDelegate:delegate];
    [self refreshVisibility];
}

- (void)setVisible:(BOOL)visible forDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate
{
    if (self.toolbarDelegate != delegate)
    {
        return;
    }
    const BOOL newlyEnabled = visible && !_requestedVisible;
    _requestedVisible = visible;
    if (newlyEnabled) [self noteInputForDelegate:delegate];
    if (!visible) { [_idleTimer invalidate]; _idleTimer = nil; }
    [self refreshVisibility];
}

- (void)wakeForInputDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate
{
    if (!delegate) return;
    self.toolbarDelegate = delegate;
    _imeActive = YES;
    _requestedVisible = YES;
    _idleHidden = NO;
    _recentInput = YES;
    [self noteInputForDelegate:delegate];
    [self setIsVisible:YES];
    [self orderFrontRegardless];
}

- (void)noteInputForDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate
{
    if (!delegate || self.toolbarDelegate != delegate || !_imeActive || !_requestedVisible) return;
    _idleHidden = NO;
    _recentInput = YES;
    if (_idleTimer) {
        _idleTimer.fireDate = [NSDate dateWithTimeIntervalSinceNow:10.0];
    } else {
        __weak MetasequoiaFloatingToolbarPanel *weakSelf = self;
        _idleTimer = [NSTimer timerWithTimeInterval:10.0 repeats:NO block:^(NSTimer *timer) {
            MetasequoiaFloatingToolbarPanel *panel = weakSelf;
            if (!panel || panel->_idleTimer != timer) return;
            panel->_idleTimer = nil;
            panel->_idleHidden = YES;
            panel->_recentInput = NO;
            [panel refreshVisibility];
        }];
        [NSRunLoop.mainRunLoop addTimer:_idleTimer forMode:NSRunLoopCommonModes];
    }
    if (!self.visible) [self refreshVisibility];
}

- (void)sendEvent:(NSEvent *)event
{
    // Keep controls available while clicking or dragging the toolbar itself.
    if (event.type == NSEventTypeLeftMouseDown || event.type == NSEventTypeLeftMouseDragged ||
        event.type == NSEventTypeLeftMouseUp || event.type == NSEventTypeRightMouseDown)
        [self noteInputForDelegate:self.toolbarDelegate];
    [super sendEvent:event];
}

- (void)deactivateForDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate
{
    // A deallocating owner reads back as nil through the weak property, so a nil owner is treated as released by the
    // caller rather than as a mismatch.
    id<MetasequoiaFloatingToolbarDelegate> owner = self.toolbarDelegate;
    if (owner != nil && owner != delegate)
    {
        return;
    }
    [self orderOut:nil];
    _imeActive = NO;
    _requestedVisible = NO;
    [_idleTimer invalidate];
    _idleTimer = nil;
    _idleHidden = NO;
    _recentInput = NO;
    self.toolbarDelegate = nil;
}

- (void)deactivateForInputSourceSwitch
{
    [self orderOut:nil];
    _imeActive = NO;
    _requestedVisible = NO;
    [_idleTimer invalidate];
    _idleTimer = nil;
    _idleHidden = NO;
    _recentInput = NO;
    self.toolbarDelegate = nil;
}

- (void)toggleInputMode:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("toggleInputMode", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestToggleInputMode:self];
}

- (void)togglePunctuation:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("togglePunctuation", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestTogglePunctuation:self];
}

- (void)toggleFullWidth:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("toggleFullWidth", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestToggleFullWidth:self];
}

- (void)toggleTraditionalOutput:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("toggleTraditionalOutput", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestToggleTraditionalOutput:self];
}

- (void)openSettings:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openSettings", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestOpenSettings:self];
}

- (void)openEmoji:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openEmoji", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestOpenEmoji:self];
}

- (void)openHandwriting:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openHandwriting", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestOpenHandwriting:self];
}

- (void)openScreenKeyboard:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openScreenKeyboard", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestOpenScreenKeyboard:self];
}

- (void)toggleVoice:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("toggleVoice", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestToggleVoice:self];
}

- (void)openCharacterPalette:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openCharacterPalette", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestOpenCharacterPalette:self];
}

- (void)checkForUpdates:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("checkForUpdates", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestCheckForUpdates:self];
}

- (void)openWebsite:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openWebsite", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestOpenWebsite:self];
}

- (void)openHelp:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openHelp", self.toolbarDelegate != nil, sender);
    MSIMEOpenDesktopRoute(@"settings:help", NSWorkspace.sharedWorkspace, ^{
        [[MSIMESupportWindowController sharedController] showPage:MSIMESupportPageHelp];
    });
}

- (void)openAbout:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openAbout", self.toolbarDelegate != nil, sender);
    MSIMEOpenDesktopRoute(@"settings:about", NSWorkspace.sharedWorkspace, ^{
        [[MSIMESupportWindowController sharedController] showPage:MSIMESupportPageAbout];
    });
}

- (void)openFeedback:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("openFeedback", self.toolbarDelegate != nil, sender);
    MSIMEOpenDesktopRoute(@"settings:feedback", NSWorkspace.sharedWorkspace, ^{
        [[MSIMESupportWindowController sharedController] showPage:MSIMESupportPageFeedback];
    });
}

- (void)dismissFloatingToolbar:(id)sender
{
    (void)sender;
    MSIMELogToolbarAction("dismissFloatingToolbar", self.toolbarDelegate != nil, sender);
    [self.toolbarDelegate floatingToolbarDidRequestHide:self];
}
@end
