#import "CandidatePanel.h"
#import "CandidateSkinAppearance.h"
#import "CandidateTypography.h"

#include "../core/DiagnosticLog.h"
#include <cmath>

@interface MetasequoiaCandidateWindow : NSPanel
@property(nonatomic, weak) id<MetasequoiaCandidatePanelDelegate> candidateDelegate;
@property(nonatomic) BOOL hasPreviousPage;
@property(nonatomic) BOOL hasNextPage;
@end
@implementation MetasequoiaCandidateWindow
- (BOOL)canBecomeKeyWindow
{
    return NO;
}
- (BOOL)canBecomeMainWindow
{
    return NO;
}
- (void)scrollWheel:(NSEvent *)event
{
    if (event.scrollingDeltaY > 0 && self.hasPreviousPage) [self.candidateDelegate candidatePanelPreviousPage];
    else if (event.scrollingDeltaY < 0 && self.hasNextPage) [self.candidateDelegate candidatePanelNextPage];
    else [super scrollWheel:event];
}
@end

@interface MetasequoiaCandidateButton : NSButton
@property(nonatomic) BOOL candidateHighlighted;
@property(nonatomic) BOOL candidateHovered;
@property(nonatomic, strong) NSFont *numberFont;
@property(nonatomic, copy) NSColor *fillColor;
@property(nonatomic, copy) NSColor *hoverColor;
@property(nonatomic, copy) NSColor *selectedHoverColor;
@property(nonatomic, copy) NSColor *titleColor;
@property(nonatomic, copy) NSColor *numberColor;
@property(nonatomic, copy) NSColor *barColor;
@property(nonatomic) BOOL showSelectedBar;
@property(nonatomic) CGFloat cornerRadius;
@end
@implementation MetasequoiaCandidateButton
- (BOOL)acceptsFirstResponder
{
    return NO;
}
- (BOOL)acceptsFirstMouse:(NSEvent *)event
{
    (void)event;
    return YES;
}
- (void)resetCursorRects
{
    [self addCursorRect:self.bounds cursor:NSCursor.pointingHandCursor];
}
- (void)updateTrackingAreas
{
    [super updateTrackingAreas];
    for (NSTrackingArea *area in [self.trackingAreas copy])
        [self removeTrackingArea:area];
    [self addTrackingArea:[[NSTrackingArea alloc]
                              initWithRect:NSZeroRect
                                   options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways |
                                           NSTrackingInVisibleRect
                                     owner:self
                                  userInfo:nil]];
}
- (void)mouseEntered:(NSEvent *)event
{
    (void)event;
    self.candidateHovered = YES;
    self.needsDisplay = YES;
}
- (void)mouseExited:(NSEvent *)event
{
    (void)event;
    self.candidateHovered = NO;
    self.needsDisplay = YES;
}
- (void)drawRect:(NSRect)dirtyRect
{
    (void)dirtyRect;
    NSRectClip(self.bounds);
    NSColor *rowFill = self.candidateHighlighted
        ? (self.candidateHovered ? self.selectedHoverColor : self.fillColor)
        : (self.candidateHovered ? self.hoverColor : nil);
    if (rowFill != nil && rowFill.alphaComponent > 0.01)
    {
        [rowFill setFill];
        const CGFloat radius = MAX(0.0, self.cornerRadius);
        [[NSBezierPath bezierPathWithRoundedRect:NSInsetRect(self.bounds, 1, 1)
                                         xRadius:radius
                                         yRadius:radius]
            fill];
    }
    if (self.candidateHighlighted && self.showSelectedBar)
    {
        const CGFloat barHeight = MAX(10.0, self.font.pointSize * 0.8);
        [self.barColor setFill];
        [[NSBezierPath
            bezierPathWithRoundedRect:NSMakeRect(3.0, (self.bounds.size.height - barHeight) / 2.0, 3.0, barHeight)
                              xRadius:1.5
                              yRadius:1.5] fill];
    }
    NSMutableParagraphStyle *paragraph = [NSMutableParagraphStyle new];
    paragraph.lineBreakMode = NSLineBreakByTruncatingTail;
    NSDictionary *numberAttributes = @{
        NSFontAttributeName : self.numberFont ?: MSIMECandidateNumberFont(self.font),
        NSForegroundColorAttributeName : self.numberColor != nil ? self.numberColor : NSColor.tertiaryLabelColor,
    };
    NSDictionary *titleAttributes = @{
        NSFontAttributeName : self.font,
        NSForegroundColorAttributeName : self.titleColor != nil ? self.titleColor : NSColor.labelColor,
        NSParagraphStyleAttributeName : paragraph,
    };
    NSString *title = self.title;
    NSRange split = [title rangeOfString:@"  "];
    const CGFloat textLeft = 8.0 + (self.showSelectedBar ? 6.0 : 0.0);
    if (split.location == NSNotFound)
    {
        const NSSize size = [title sizeWithAttributes:titleAttributes];
        const CGFloat maxWidth = MAX(0.0, self.bounds.size.width - textLeft - 8.0);
        [title drawInRect:NSMakeRect(textLeft, (self.bounds.size.height - size.height) / 2, maxWidth, size.height)
            withAttributes:titleAttributes];
        return;
    }
    NSString *number = [title substringToIndex:split.location];
    NSString *word = [title substringFromIndex:NSMaxRange(split)];
    const NSSize numberSize = [number sizeWithAttributes:numberAttributes];
    const NSSize wordSize = [word sizeWithAttributes:titleAttributes];
    const CGFloat numberY = (self.bounds.size.height - numberSize.height) / 2;
    const CGFloat wordY = (self.bounds.size.height - wordSize.height) / 2;
    [number drawAtPoint:NSMakePoint(textLeft, numberY) withAttributes:numberAttributes];
    const CGFloat wordX = textLeft + numberSize.width + MSIMECandidateNumberGap;
    const CGFloat maxWidth = MAX(0.0, self.bounds.size.width - wordX - 8.0);
    [word drawInRect:NSMakeRect(wordX, wordY, maxWidth, wordSize.height) withAttributes:titleAttributes];
}
@end

@interface MetasequoiaCandidateChromeView : NSView
@property(nonatomic, weak) id appearanceTarget;
@property(nonatomic) SEL appearanceAction;
@property(nonatomic, copy) NSColor *fillColor;
@property(nonatomic, copy) NSColor *strokeColor;
@property(nonatomic) CGFloat cornerRadius;
@property(nonatomic) CGFloat lineWidth;
@end
@implementation MetasequoiaCandidateChromeView
- (BOOL)isOpaque
{
    return self.fillColor.alphaComponent >= 0.99;
}
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
    NSBezierPath *path = [NSBezierPath bezierPathWithRoundedRect:self.bounds
                                                         xRadius:self.cornerRadius
                                                         yRadius:self.cornerRadius];
    [(self.fillColor != nil ? self.fillColor : NSColor.windowBackgroundColor) setFill];
    [path fill];
    if (self.lineWidth > 0.0 && self.strokeColor.alphaComponent > 0.01)
    {
        path.lineWidth = self.lineWidth;
        [self.strokeColor setStroke];
        [path stroke];
    }
}
@end

@implementation MetasequoiaCandidatePanel
{
    NSPanel *_window;
    MetasequoiaCandidateChromeView *_chrome;
    NSImageView *_decorationView;
    NSArray<NSAttributedString *> *_data;
    NSFont *_font;
    NSInteger _selected;
    metasequoia::mac::ResolvedSkin _skin;
    NSImage *_decorationImage;
    CGFloat _tallestVerticalHeight;
}

- (void)setDelegate:(id<MetasequoiaCandidatePanelDelegate>)delegate
{
    _delegate = delegate;
    ((MetasequoiaCandidateWindow *)_window).candidateDelegate = delegate;
}

- (instancetype)init
{
    self = [super init];
    if (self)
    {
        _data = @[];
        _font = [NSFont systemFontOfSize:18];
        _selected = NSNotFound;
        _window = [[MetasequoiaCandidateWindow alloc]
            initWithContentRect:NSZeroRect
                      styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                        backing:NSBackingStoreBuffered
                          defer:NO];
        _window.releasedWhenClosed = NO;
        _window.level = NSPopUpMenuWindowLevel;
        _window.hidesOnDeactivate = NO;
        _window.opaque = NO;
        _window.backgroundColor = NSColor.clearColor;
        _window.hasShadow = YES;
        _window.collectionBehavior =
            NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
        _chrome = [[MetasequoiaCandidateChromeView alloc] initWithFrame:NSZeroRect];
        _chrome.appearanceTarget = self;
        _chrome.appearanceAction = @selector(reloadSkin);
        _window.contentView = _chrome;
        _decorationView = [[NSImageView alloc] initWithFrame:NSZeroRect];
        _decorationView.imageScaling = NSImageScaleProportionallyUpOrDown;
        _decorationView.imageAlignment = NSImageAlignTopRight;
        _decorationView.wantsLayer = YES;
        [self reloadSkin];
        [[NSNotificationCenter defaultCenter] addObserver:self
                                                 selector:@selector(reloadSkin)
                                                     name:MetasequoiaCandidateSkinDidChangeNotification
                                                   object:nil];
    }
    return self;
}
- (void)dealloc
{
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    [_window orderOut:nil];
}
- (NSPanel *)window
{
    return _window;
}
- (void)reloadSkin
{
    _skin = MetasequoiaResolveStoredTheme(MetasequoiaAppearanceIsDark(_chrome.effectiveAppearance),
                                          _panelType == kIMKSingleColumnScrollingCandidatePanel);
    _decorationImage = nil;
    if (_skin.decorationTopDip > 0.0 && !_skin.decorationPath.empty())
    {
        _decorationImage = [[NSImage alloc] initWithContentsOfFile:@(_skin.decorationPath.c_str())];
    }
    [self layoutCandidates];
}
- (void)setPanelType:(IMKCandidatePanelType)type
{
    const BOOL wasVertical = _panelType == kIMKSingleColumnScrollingCandidatePanel;
    _panelType = type;
    // The layout decides whether a package is drawn, so a change of layout resolves the theme again.
    if (wasVertical != (type == kIMKSingleColumnScrollingCandidatePanel))
    {
        [self reloadSkin];
        return;
    }
    [self layoutCandidates];
}
- (void)setHasPreviousPage:(BOOL)value
{
    _hasPreviousPage = value;
    [self layoutCandidates];
}
- (void)setHasNextPage:(BOOL)value
{
    _hasNextPage = value;
    [self layoutCandidates];
}
- (void)setAttributes:(NSDictionary *)attributes
{
    NSFont *font = attributes[NSFontAttributeName];
    if ([font isKindOfClass:NSFont.class])
        _font = font;
    [self layoutCandidates];
}
- (void)setCandidateData:(NSArray<NSAttributedString *> *)candidates
{
    _data = [candidates copy];
    _selected = _data.count > 0 ? 0 : NSNotFound;
    [self layoutCandidates];
    if (_data.count == 0)
        [self hide];
}
- (NSScreen *)screenForCaret
{
    for (NSScreen *screen in NSScreen.screens)
        if (NSPointInRect(NSMakePoint(NSMinX(self.caretRect), NSMidY(self.caretRect)), screen.frame))
            return screen;
    return NSScreen.mainScreen;
}
- (void)layoutCandidates
{
    MetasequoiaCandidateWindow *window = (MetasequoiaCandidateWindow *)_window;
    window.hasPreviousPage = _hasPreviousPage;
    window.hasNextPage = _hasNextPage;
    for (NSView *view in [_chrome.subviews copy])
        [view removeFromSuperview];
    const CGFloat inset = MAX(2.0, _skin.tokens.pad);
    const CGFloat rowHeight = ceil(_font.ascender - _font.descender + _font.leading) + 12;
    const BOOL vertical = _panelType == kIMKSingleColumnScrollingCandidatePanel;
    NSMutableArray<NSNumber *> *widths = [NSMutableArray array];
    NSMutableArray<NSString *> *titles = [NSMutableArray array];
    CGFloat width = 0;
    const BOOL paging = _hasPreviousPage || _hasNextPage;
    const CGFloat screenWidth = [self screenForCaret].visibleFrame.size.width;
    const CGFloat availableWidth = MAX(80, screenWidth - 20 - 2 * inset - (paging && !vertical ? 56 : 0));
    const CGFloat leftPad = 8.0 + (_skin.tokens.showSelectedBar ? 6.0 : 0.0);
    NSDictionary *measure = @{NSFontAttributeName : _font};
    NSFont *numberFont = MSIMECandidateNumberFont(_font);
    NSDictionary *numberMeasure = @{NSFontAttributeName : numberFont};
    for (NSUInteger index = 0; index < _data.count; ++index)
    {
        NSString *number = [NSString stringWithFormat:@"%lu", (unsigned long)index + 1];
        NSString *word = _data[index].string;
        NSString *title = [NSString stringWithFormat:@"%@  %@", number, word];
        const CGFloat itemWidth = ceil(leftPad + [number sizeWithAttributes:numberMeasure].width + MSIMECandidateNumberGap +
                                       [word sizeWithAttributes:measure].width + 8.0);
        [titles addObject:title];
        [widths addObject:@(itemWidth)];
        width = vertical ? MAX(width, itemWidth) : width + itemWidth;
    }
    if (vertical)
    {
        width = MIN(width, availableWidth);
    }
    else if (width > availableWidth && width > 0)
    {
        const CGFloat scale = availableWidth / width;
        width = 0;
        for (NSUInteger index = 0; index < widths.count; ++index)
        {
            const CGFloat scaled = MAX(24.0, floor(widths[index].doubleValue * scale));
            widths[index] = @(scaled);
            width += scaled;
        }
    }
    const CGFloat navigationHeight = paging && vertical ? 26 : 0;
    if (paging)
        width = vertical ? MAX(width, 64) : width + 56;
    const CGFloat decorationHeight = _skin.decorationTopDip > 0.0 ? _skin.decorationTopDip : 0.0;
    const CGFloat minWidth = MAX(_skin.minWidthDip, MAX(_skin.decorationWidthDip, 20));
    NSSize size = NSMakeSize(MAX(width + 2 * inset, minWidth),
                             MAX((vertical ? _data.count : (_data.count > 0 ? 1 : 0)) * rowHeight + navigationHeight +
                                     2 * inset + decorationHeight,
                                 10));
    [_window setContentSize:size];
    _chrome.fillColor = MetasequoiaColorFromRgba(_skin.tokens.surface);
    _chrome.strokeColor = MetasequoiaColorFromRgba(_skin.tokens.border);
    _chrome.cornerRadius = _skin.tokens.radius;
    _chrome.lineWidth = _skin.tokens.borderWidth;
    _chrome.needsDisplay = YES;
    if (decorationHeight > 0.0 && _decorationImage != nil)
    {
        const CGFloat decorationWidth =
            _skin.decorationWidthDip > 0.0 ? _skin.decorationWidthDip : MIN(size.width, _decorationImage.size.width);
        _decorationView.image = _decorationImage;
        _decorationView.frame =
            NSMakeRect(size.width - decorationWidth, size.height - decorationHeight, decorationWidth, decorationHeight);
        [_chrome addSubview:_decorationView];
    }
    CGFloat x = inset;
    const CGFloat contentTop = size.height - inset - decorationHeight;
    const CGFloat verticalItemWidth = MAX(0.0, size.width - 2.0 * inset);
    NSColor *selectedFill = MetasequoiaColorFromRgba(_skin.tokens.selected);
    NSColor *textColor = MetasequoiaColorFromRgba(_skin.tokens.text);
    NSColor *selectedText = MetasequoiaColorFromRgba(_skin.tokens.selectedText);
    NSColor *numberColor = MetasequoiaColorFromRgba(_skin.tokens.number);
    NSColor *accent = MetasequoiaColorFromRgba(_skin.tokens.accent);
    for (NSUInteger index = 0; index < _data.count; ++index)
    {
        const CGFloat itemWidth = vertical ? verticalItemWidth : widths[index].doubleValue;
        const CGFloat y = vertical ? contentTop - (index + 1) * rowHeight : inset;
        MetasequoiaCandidateButton *button =
            [[MetasequoiaCandidateButton alloc] initWithFrame:NSMakeRect(vertical ? inset : x, y, itemWidth, rowHeight)];
        button.title = titles[index];
        button.font = _font;
        button.numberFont = numberFont;
        button.bordered = NO;
        button.tag = (NSInteger)index;
        button.target = self;
        button.action = @selector(selectFromMouse:);
        button.candidateHighlighted = (NSInteger)index == _selected;
        button.fillColor = selectedFill;
        button.hoverColor = MetasequoiaColorFromRgba(_skin.tokens.hover);
        button.selectedHoverColor = MetasequoiaColorFromRgba(_skin.tokens.selectedHover);
        button.titleColor = button.candidateHighlighted ? selectedText : textColor;
        button.numberColor = button.candidateHighlighted ? selectedText : numberColor;
        button.barColor = accent;
        button.showSelectedBar = _skin.tokens.showSelectedBar;
        button.cornerRadius = button.candidateHighlighted ? _skin.tokens.selectedRadius : _skin.tokens.candidateRadius;
        button.accessibilityLabel = titles[index];
        button.toolTip = _data[index].string;
        [_chrome addSubview:button];
        if (!vertical)
            x += itemWidth;
    }
    if (paging)
    {
        for (NSUInteger index = 0; index < 2; ++index)
        {
            NSButton *button = [NSButton buttonWithTitle:index == 0 ? @"‹" : @"›"
                                                  target:self
                                                  action:@selector(changePage:)];
            button.frame = NSMakeRect(vertical ? inset + index * 28 : x + index * 28, inset, 28,
                                      vertical ? navigationHeight : rowHeight);
            button.bordered = NO;
            button.contentTintColor = textColor;
            button.tag = index == 0 ? -1 : -2;
            button.enabled = index == 0 ? _hasPreviousPage : _hasNextPage;
            button.accessibilityLabel = index == 0 ? @"上一页候选" : @"下一页候选";
            [_chrome addSubview:button];
        }
    }
}
- (void)selectFromMouse:(NSButton *)button
{
    if ([self selectCandidateWithIdentifier:button.tag])
        [self.delegate candidateSelected:_data[button.tag]];
}
- (void)changePage:(NSButton *)button
{
    if (button.tag == -1 && _hasPreviousPage)
        [self.delegate candidatePanelPreviousPage];
    if (button.tag == -2 && _hasNextPage)
        [self.delegate candidatePanelNextPage];
}
- (void)show:(IMKCandidatesLocationHint)hint
{
    (void)hint;
    if (_data.count == 0)
    {
        [self hide];
        return;
    }
    NSRect caret = self.caretRect;
    if (!std::isfinite(caret.origin.x) || !std::isfinite(caret.origin.y) || !std::isfinite(caret.size.width) ||
        !std::isfinite(caret.size.height) || caret.size.height <= 0)
    {
        [self hide];
        return;
    }
    [self layoutCandidates];
    NSRect bounds = [self screenForCaret].visibleFrame;
    NSSize size = _window.frame.size;
    CGFloat x = MIN(MAX(NSMinX(caret), NSMinX(bounds)), MAX(NSMinX(bounds), NSMaxX(bounds) - size.width));
    const BOOL vertical = _panelType == kIMKSingleColumnScrollingCandidatePanel;
    if (vertical)
        _tallestVerticalHeight = MAX(_tallestVerticalHeight, size.height);
    const CGFloat decisionHeight = vertical ? _tallestVerticalHeight : size.height;
    CGFloat y = NSMinY(caret) - size.height - 4;
    // Use the tallest vertical page seen in this composition to choose the
    // side of the caret, but place the current page with its actual height.
    // This prevents a growing page from jumping below-to-above while avoiding
    // an empty gap when a later page is shorter.
    const CGFloat decisionY = NSMinY(caret) - decisionHeight - 4;
    if (decisionY < NSMinY(bounds))
        y = NSMaxY(caret) + 4;
    y = MIN(MAX(y, NSMinY(bounds)), MAX(NSMinY(bounds), NSMaxY(bounds) - size.height));
    [_window setFrameOrigin:NSMakePoint(x, y)];
    [_window orderFrontRegardless];
    msime_macos_diagnostic_writef("candidate-position hint=%ld rows=%lu vertical=%d size=(%.0f,%.0f) origin=(%.0f,%.0f) flipped=%d",
        (long)hint, (unsigned long)_data.count, vertical ? 1 : 0, size.width, size.height, x, y, y >= NSMaxY(caret) ? 1 : 0);
}
- (void)hide
{
    if (msime_macos_diagnostic_enabled() && _window.isVisible) msime_macos_diagnostic_write("candidate hide reason=panel_hide");
    _tallestVerticalHeight = 0;
    [_window orderOut:nil];
}
- (BOOL)isVisible
{
    return _window.isVisible;
}
- (NSRect)candidateFrame
{
    return _window.frame;
}
- (NSInteger)candidateIdentifierAtLineNumber:(NSInteger)line
{
    return line >= 0 && (NSUInteger)line < _data.count ? line : NSNotFound;
}
- (NSInteger)lineNumberForCandidateWithIdentifier:(NSInteger)identifier
{
    return [self candidateIdentifierAtLineNumber:identifier];
}
- (NSInteger)candidateStringIdentifier:(NSAttributedString *)candidate
{
    return (NSInteger)[_data indexOfObjectIdenticalTo:candidate];
}
- (BOOL)selectCandidateWithIdentifier:(NSInteger)identifier
{
    if ([self candidateIdentifierAtLineNumber:identifier] == NSNotFound)
        return NO;
    _selected = identifier;
    for (NSView *view in _chrome.subviews)
        if ([view isKindOfClass:MetasequoiaCandidateButton.class])
        {
            MetasequoiaCandidateButton *button = (MetasequoiaCandidateButton *)view;
            button.candidateHighlighted = view.tag == identifier;
            button.cornerRadius = button.candidateHighlighted ? _skin.tokens.selectedRadius : _skin.tokens.candidateRadius;
            button.titleColor = button.candidateHighlighted ? MetasequoiaColorFromRgba(_skin.tokens.selectedText)
                                                            : MetasequoiaColorFromRgba(_skin.tokens.text);
            button.numberColor = button.candidateHighlighted ? MetasequoiaColorFromRgba(_skin.tokens.selectedText)
                                                             : MetasequoiaColorFromRgba(_skin.tokens.number);
            view.needsDisplay = YES;
        }
    return YES;
}
- (NSInteger)selectedCandidate
{
    return _selected;
}
- (NSAttributedString *)selectedCandidateString
{
    return _selected == NSNotFound ? nil : _data[_selected];
}
@end
