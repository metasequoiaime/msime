#pragma once
#import <AppKit/AppKit.h>
#import "CandidateTypography.h"
#import "CandidateTextMetrics.h"
#include "CandidateItemLayout.h"
#include "CandidateSkin.h"
// Drawing adapted from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
static const CGFloat MSIMECandidateTranslationOpacity = 0.62;
// The translation under or beside a candidate is a fixed 12pt run at every candidate size (design-input-surfaces §2.1), so a large candidate font does not grow a gloss the eye only glances at.
static const CGFloat MSIMECandidateTranslationPointSize = 12.0;
// The top row of the card: the reading on the left, then 「1 / 3」 and the two page arrows on the right.
static const CGFloat MSIMECandidateHeaderHeight = 26.0;
// 「1 / 3」 is set at the design's 13pt in the secondary colour (dc.html L1324), a point above the translation run.
static const CGFloat MSIMECandidatePageIndicatorPointSize = 13.0;
static const CGFloat MSIMECandidatePageArrowWidth = 28.0;
static const CGFloat MSIMECandidatePageIndicatorGap = 4.0;
// A row keeps 6 points above and below its text, and a gloss under the text 2 points on each side of its lines.
static const CGFloat MSIMECandidateRowPadding = 12.0;
static const CGFloat MSIMECandidateGlossPadding = 4.0;
static const CGFloat MSIMECandidateTextRight = 8.0;

static inline CGFloat MSIMECandidateTextLeft(BOOL showSelectedBar)
{
    return 8.0 + (showSelectedBar ? 6.0 : 0.0);
}

// One set of attributes for measuring and drawing a run, so a wrapped run is drawn in exactly the height the layout measured for it. A run that fits clips instead of wrapping.
static inline NSDictionary *MSIMECandidateRunAttributes(NSFont *font, NSColor *color, BOOL wraps)
{
    NSMutableParagraphStyle *paragraph = [NSMutableParagraphStyle new];
    paragraph.lineBreakMode = wraps ? NSLineBreakByWordWrapping : NSLineBreakByClipping;
    NSMutableDictionary *attributes = [@{NSFontAttributeName : font, NSParagraphStyleAttributeName : paragraph} mutableCopy];
    if (color != nil) attributes[NSForegroundColorAttributeName] = color;
    return attributes;
}

static inline CGFloat MSIMECandidateWrappedHeight(NSString *text, NSFont *font, CGFloat width)
{
    if (text.length == 0 || font == nil || !(width > 0.0)) return 0.0;
    const NSRect bounds = [text boundingRectWithSize:NSMakeSize(width, CGFLOAT_MAX)
                                             options:NSStringDrawingUsesLineFragmentOrigin
                                          attributes:MSIMECandidateRunAttributes(font, nil, YES)];
    return ceil(bounds.size.height);
}

static inline CGFloat MSIMECandidateSingleLineWidth(NSString *text, NSFont *font)
{
    return text.length ? ceil([text sizeWithAttributes:@{NSFontAttributeName : font}].width) : 0.0;
}

// Height of a gloss drawn without wrapping, one line per target language, with its padding.
static inline CGFloat MSIMECandidateGlossHeight(NSString *text, NSFont *font)
{
    if (text.length == 0) return 0.0;
    const NSRect bounds = [text boundingRectWithSize:NSMakeSize(CGFLOAT_MAX, CGFLOAT_MAX)
                                             options:NSStringDrawingUsesLineFragmentOrigin
                                          attributes:@{NSFontAttributeName : font}];
    return ceil(bounds.size.height) + MSIMECandidateGlossPadding;
}

// Metrics of a candidate row. The annotation is drawn at the candidate font, the gloss at glossFont; `chrome` is the width of the number, bar and paddings around the content.
static inline msime::mac::CandidateLayoutMetrics MSIMECandidateLayoutMetrics(NSFont *font, NSFont *glossFont, CGFloat candidateRow, CGFloat chrome)
{
    msime::mac::CandidateLayoutMetrics metrics;
    metrics.candidateRow = candidateRow;
    metrics.chrome = chrome;
    metrics.annotationGap = 4.0;
    metrics.annotationLine = MSIMECandidateTextHeight(@"", font);
    metrics.translationGap = font.pointSize * 0.65;
    metrics.translationLine = MSIMECandidateGlossHeight(@"X", glossFont);
    return metrics;
}

static inline msime::mac::CandidateItemWidths MSIMECandidateItemWidths(NSString *text, NSString *annotation, NSString *translation, NSFont *font, NSFont *glossFont)
{
    msime::mac::CandidateItemWidths widths;
    widths.text = MSIMECandidateSingleLineWidth(text, font);
    widths.annotation = MSIMECandidateSingleLineWidth(annotation, font);
    widths.translation = MSIMECandidateSingleLineWidth(translation, glossFont);
    widths.translationHeight = MSIMECandidateGlossHeight(translation, glossFont);
    return widths;
}

// Wrapped height of each run, as the layout asks for it: the text keeps the row's padding, a gloss its own.
static inline msime::mac::CandidateRunMeasure MSIMECandidateRunMeasure(NSString *text, NSString *annotation, NSString *translation, NSFont *font, NSFont *glossFont)
{
    return [text = [text copy], annotation = [annotation copy], translation = [translation copy], font, glossFont](msime::mac::CandidateRun run, double width) -> double {
        switch (run)
        {
        case msime::mac::CandidateRun::text:
            return MSIMECandidateWrappedHeight(text, font, width) + MSIMECandidateRowPadding;
        case msime::mac::CandidateRun::annotation:
            return MSIMECandidateWrappedHeight(annotation, font, width);
        case msime::mac::CandidateRun::translation:
            return MSIMECandidateWrappedHeight(translation, glossFont, width) + MSIMECandidateGlossPadding;
        }
        return 0.0;
    };
}

@interface MSIMECandidateButton : NSButton
@property(nonatomic, copy) NSDictionary *candidateID;
@property(nonatomic, strong) NSFont *numberFont;
@property(nonatomic) BOOL candidateHighlighted;
@property(nonatomic) BOOL candidateFixed;
@property(nonatomic, copy) NSString *translation;
@property(nonatomic, strong) NSFont *translationFont;
@property(nonatomic, copy) NSColor *translationColor;
@property(nonatomic) BOOL translationBelow;
// The 辅助码 or engine annotation, drawn as its own run after the text and moved under it when it does not fit.
@property(nonatomic, copy) NSString *annotation;
// Geometry from the panel's page layout; frames, drawing and hit testing all come from it. Without one the button lays itself out in its bounds.
@property(nonatomic) msime::mac::CandidateItemLayout itemLayout;
@property(nonatomic) BOOL hasItemLayout;
// Where the candidate text starts, the same for every row of a page so their text lines up. Zero derives it from the number drawn.
@property(nonatomic) CGFloat contentLeft;
@property(nonatomic) NSInteger armedGlossColumn;
@property(nonatomic, copy) NSColor *fillColor;
@property(nonatomic, copy) NSColor *hoverColor;
@property(nonatomic, copy) NSColor *titleColor;
@property(nonatomic, copy) NSColor *numberColor;
@property(nonatomic, copy) NSColor *barColor;
@property(nonatomic) BOOL showSelectedBar;
@property(nonatomic) BOOL candidateHovered;
@property(nonatomic) CGFloat cornerRadius;
@property(nonatomic) CGFloat selectionLeftInset;
@end
@implementation MSIMECandidateButton
{
    NSTrackingArea *_candidateTrackingArea;
}
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
    // Candidate rows are actionable surfaces. Keep the native macOS pointer
    // affordance used by the retained Apple panel while leaving keyboard focus
    // disabled for IMK input routing.
    if (!self.enabled)
        return;
    [self addCursorRect:self.bounds cursor:NSCursor.pointingHandCursor];
}
- (void)updateTrackingAreas
{
    if (_candidateTrackingArea != nil)
        [self removeTrackingArea:_candidateTrackingArea];
    _candidateTrackingArea = [[NSTrackingArea alloc]
        initWithRect:NSZeroRect
             options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect
               owner:self
            userInfo:nil];
    [self addTrackingArea:_candidateTrackingArea];
    [super updateTrackingAreas];
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
    if (self.tag < 0) { [super drawRect:dirtyRect]; return; }
    (void)dirtyRect;
    NSRectClip(self.bounds);
    NSColor *background = self.candidateHighlighted ? self.fillColor
                                                     : (self.candidateHovered ? self.hoverColor : nil);
    if (background != nil && background.alphaComponent > 0.01)
    {
        [background setFill];
        [[NSBezierPath bezierPathWithRoundedRect:NSInsetRect(self.bounds, 1, 1) xRadius:self.cornerRadius yRadius:self.cornerRadius] fill];
    }
    if (self.candidateHighlighted && self.showSelectedBar)
    {
        const CGFloat barHeight = MAX(10.0, self.font.pointSize * 0.8);
        [self.barColor setFill];
        [[NSBezierPath
            bezierPathWithRoundedRect:NSMakeRect(MAX(1.0, self.selectionLeftInset),
                                                 (self.bounds.size.height - barHeight) / 2.0,
                                                 3.0, barHeight)
                              xRadius:1.5
                              yRadius:1.5] fill];
    }
    NSDictionary *numberAttributes = @{
        NSFontAttributeName : self.numberFont ?: MSIMECandidateNumberFont(self.font),
        NSForegroundColorAttributeName : self.numberColor != nil ? self.numberColor : NSColor.tertiaryLabelColor,
    };
    NSColor *titleColor = self.titleColor != nil ? self.titleColor : NSColor.labelColor;
    // Runs that fit are drawn on one line with clipping rather than wrapping, so a rounding difference between measuring and drawing can never fold text that fits; only runs the layout wrapped are drawn wrapped.
    NSDictionary *titleAttributes = MSIMECandidateRunAttributes(self.font, titleColor, NO);
    NSString *title = self.title;
    NSRange split = [title rangeOfString:@"  "];
    const CGFloat textLeft = MSIMECandidateTextLeft(self.showSelectedBar);
    const BOOL flipped = self.isFlipped;
    const CGFloat boundsHeight = self.bounds.size.height;
    // Layout boxes run downwards from the row top; convert them for either orientation of the view.
    NSRect (^box)(CGFloat, CGFloat, CGFloat, CGFloat) = ^NSRect(CGFloat x, CGFloat y, CGFloat width, CGFloat height) {
        return NSMakeRect(x, flipped ? y : boundsHeight - y - height, MAX(0.0, width), MAX(0.0, height));
    };
    if (split.location == NSNotFound)
    {
        const NSSize size = [title sizeWithAttributes:titleAttributes];
        const CGFloat maxWidth = MAX(0.0, self.bounds.size.width - textLeft - MSIMECandidateTextRight);
        [title drawInRect:NSMakeRect(textLeft, (boundsHeight - size.height) / 2, maxWidth, size.height)
            withAttributes:titleAttributes];
        return;
    }
    NSString *number = [title substringToIndex:split.location];
    NSString *word = [title substringFromIndex:NSMaxRange(split)];
    NSString *annotation = self.annotation ?: @"";
    NSString *translation = self.translation ?: @"";
    const NSSize numberSize = [number sizeWithAttributes:numberAttributes];
    const NSSize wordSize = [word sizeWithAttributes:titleAttributes];
    NSFont *glossFont = self.translationFont ?: [NSFont systemFontOfSize:MSIMECandidateTranslationPointSize];
    const CGFloat contentLeft = self.contentLeft > 0.0 ? self.contentLeft : textLeft + numberSize.width + MSIMECandidateNumberGap;
    msime::mac::CandidateItemLayout layout = self.itemLayout;
    if (!self.hasItemLayout)
    {
        // A button nobody laid out lays itself out in its own bounds, with the same rule the panel uses.
        const msime::mac::CandidateLayoutMetrics metrics =
            MSIMECandidateLayoutMetrics(self.font, glossFont, MAX(boundsHeight, MSIMECandidateTextHeight(word, self.font) + MSIMECandidateRowPadding),
                                        contentLeft + MSIMECandidateTextRight);
        layout = msime::mac::LayoutCandidateItem(MSIMECandidateItemWidths(word, annotation, translation, self.font, glossFont),
                                                 self.bounds.size.width - contentLeft - MSIMECandidateTextRight, metrics,
                                                 self.translationBelow, MSIMECandidateRunMeasure(word, annotation, translation, self.font, glossFont));
    }
    // The number sits on the text's first line.
    CGFloat textTop = (layout.textHeight - wordSize.height) / 2;
    CGFloat firstLine = wordSize.height;
    if (layout.textWrapped)
    {
        const CGFloat drawn = MSIMECandidateWrappedHeight(word, self.font, layout.textWidth);
        textTop = MAX(0.0, (layout.textHeight - drawn) / 2);
        firstLine = MSIMECandidateTextHeight(@"", self.font);
        [word drawInRect:box(contentLeft, textTop, layout.textWidth, drawn) withAttributes:MSIMECandidateRunAttributes(self.font, titleColor, YES)];
    }
    else
    {
        [word drawInRect:box(contentLeft, textTop, MAX(layout.textWidth, ceil(wordSize.width)) + 1.0, wordSize.height) withAttributes:titleAttributes];
    }
    [number drawInRect:box(textLeft, textTop + (firstLine - numberSize.height) / 2, ceil(numberSize.width) + 1.0, numberSize.height)
        withAttributes:numberAttributes];
    if (annotation.length && layout.annotation.width > 0.0)
    {
        const msime::mac::CandidateRunBox &run = layout.annotation;
        if (run.below)
            [annotation drawInRect:box(contentLeft + run.x, run.y, run.width, run.height) withAttributes:MSIMECandidateRunAttributes(self.font, titleColor, YES)];
        else
        {
            const NSSize size = [annotation sizeWithAttributes:titleAttributes];
            [annotation drawInRect:box(contentLeft + run.x, run.y + (run.height - size.height) / 2, run.width + 1.0, size.height) withAttributes:titleAttributes];
        }
    }
    if (translation.length && layout.translation.width > 0.0)
    {
        const msime::mac::CandidateRunBox &run = layout.translation;
        NSColor *glossColor = self.translationColor ?: [titleColor colorWithAlphaComponent:MSIMECandidateTranslationOpacity];
        NSMutableAttributedString *glossText = [[NSMutableAttributedString alloc] initWithString:translation
            attributes:MSIMECandidateRunAttributes(glossFont, glossColor, run.below)];
        if (self.armedGlossColumn > 0) {
            NSArray<NSString *> *parts = [translation componentsSeparatedByString:@"\n"];
            NSUInteger selected = (NSUInteger)(self.armedGlossColumn - 1);
            if (selected < parts.count && parts[selected].length) {
                NSUInteger location = 0;
                for (NSUInteger index = 0; index < selected; ++index)
                    location += parts[index].length + 1;
                [glossText addAttribute:NSUnderlineStyleAttributeName value:@((NSInteger)NSUnderlineStyleSingle)
                                  range:NSMakeRange(location, parts[selected].length)];
            }
        }
        const CGFloat drawnWidth = run.below ? run.width : run.width + 1.0;
        const CGFloat drawn = MIN(run.height, MSIMECandidateWrappedHeight(translation, glossFont, run.width));
        [glossText drawInRect:box(contentLeft + run.x, run.y + MAX(0.0, (run.height - drawn) / 2), drawnWidth, drawn)];
    }
}
@end

@interface MSIMECandidateChromeView : NSView
@property(nonatomic, weak) id appearanceTarget;
@property(nonatomic) SEL appearanceAction;
@property(nonatomic, copy) NSColor *fillColor;
@property(nonatomic, copy) NSColor *strokeColor;
@property(nonatomic) CGFloat cornerRadius;
@property(nonatomic) CGFloat lineWidth;
// A package's background image, drawn over the fill and under the stroke and the candidates, clipped to the rounded outline. Nil draws none.
@property(nonatomic, strong) NSImage *backgroundImage;
@property(nonatomic) msime::mac::BackgroundFit backgroundFit;
@property(nonatomic) CGFloat backgroundOpacity;
@end
@implementation MSIMECandidateChromeView
- (BOOL)isOpaque { return NO; }
- (void)viewDidMoveToWindow
{
    [super viewDidMoveToWindow];
    self.wantsLayer = YES;
    self.layer.masksToBounds = YES;
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
    NSImage *background = self.backgroundImage;
    const NSRect bounds = self.bounds;
    const auto rects = background != nil && self.backgroundOpacity > 0.0
        ? msime::mac::BackgroundRects(self.backgroundFit, {NSMinX(bounds), NSMinY(bounds), NSWidth(bounds), NSHeight(bounds)},
                                      background.size.width, background.size.height)
        : std::nullopt;
    if (rects)
    {
        [NSGraphicsContext saveGraphicsState];
        [path addClip];
        [background drawInRect:NSMakeRect(rects->destination.x, rects->destination.y, rects->destination.width, rects->destination.height)
                      fromRect:NSMakeRect(rects->source.x, rects->source.y, rects->source.width, rects->source.height)
                     operation:NSCompositingOperationSourceOver
                      fraction:MIN(1.0, self.backgroundOpacity)
                respectFlipped:YES
                         hints:nil];
        [NSGraphicsContext restoreGraphicsState];
    }
    if (self.lineWidth > 0.0 && self.strokeColor.alphaComponent > 0.01)
    {
        path.lineWidth = self.lineWidth;
        [self.strokeColor setStroke];
        [path stroke];
    }
}
@end
