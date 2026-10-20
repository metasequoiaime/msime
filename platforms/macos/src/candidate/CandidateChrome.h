#pragma once
#import <AppKit/AppKit.h>
#import "CandidateTypography.h"
#import "CandidateTextMetrics.h"
#include "CandidateItemLayout.h"
#include "CandidateSkin.h"
#import "CandidatePronunciation.h"
#import "CandidateMotion.h"
#import "CandidateCanvas.h"
// Drawing adapted from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
static const CGFloat MSIMECandidateTranslationOpacity = 0.62;
// A reading is quieter than the gloss it follows, relative to the gloss colour.
static const CGFloat MSIMECandidatePronunciationOpacity = 0.72;
// The translation under or beside a candidate is a fixed 12pt run at every candidate size (design-input-surfaces §2.1), so a large candidate font does not grow a gloss the eye only glances at.
static const CGFloat MSIMECandidateTranslationPointSize = 12.0;
// The top row of the card: the reading on the left, then 「1 / 3」 and the two page arrows on the right.
static const CGFloat MSIMECandidateHeaderHeight = 26.0;
// Room kept between a horizontal card that grows past half the screen and each edge of the visible area.
static const CGFloat MSIMECandidateScreenMargin = 16.0;
// 「1 / 3」 is set at the design's 13pt in the secondary colour (dc.html L1324), a point above the translation run.
static const CGFloat MSIMECandidatePageIndicatorPointSize = 13.0;
static const CGFloat MSIMECandidatePageArrowWidth = 28.0;
static const CGFloat MSIMECandidatePageIndicatorGap = 4.0;
// A row keeps 6 points above and below its text, and a gloss under the text 2 points on each side of its lines.
static const CGFloat MSIMECandidateRowPadding = 12.0;
static const CGFloat MSIMECandidateGlossPadding = 4.0;
static const CGFloat MSIMECandidateTextRight = 8.0;

// Every length below is the window at 100%; `scale` is candidate_scale_percent as a factor, which the candidate fonts are already set at.
static inline CGFloat MSIMECandidateTextLeft(BOOL showSelectedBar, CGFloat scale = 1.0)
{
    return (8.0 + (showSelectedBar ? 6.0 : 0.0)) * scale;
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
static inline CGFloat MSIMECandidateGlossHeight(NSString *text, NSFont *font, CGFloat scale = 1.0)
{
    if (text.length == 0) return 0.0;
    const NSRect bounds = [text boundingRectWithSize:NSMakeSize(CGFLOAT_MAX, CGFLOAT_MAX)
                                             options:NSStringDrawingUsesLineFragmentOrigin
                                          attributes:@{NSFontAttributeName : font}];
    return ceil(bounds.size.height) + MSIMECandidateGlossPadding * scale;
}

// Metrics of a candidate row. The annotation is drawn at the candidate font, the gloss at glossFont; `chrome` is the width of the number, bar and paddings around the content.
static inline msime::mac::CandidateLayoutMetrics MSIMECandidateLayoutMetrics(NSFont *font, NSFont *glossFont, CGFloat candidateRow, CGFloat chrome, CGFloat scale = 1.0)
{
    msime::mac::CandidateLayoutMetrics metrics;
    metrics.candidateRow = candidateRow;
    metrics.chrome = chrome;
    // 注释包在括号里紧贴候选字（「你好(wqvb)」），括号本身就是分隔，不再留空隙。
    metrics.annotationGap = 0.0;
    metrics.annotationLine = MSIMECandidateTextHeight(@"", font);
    metrics.translationGap = font.pointSize * 0.65;
    metrics.translationLine = MSIMECandidateGlossHeight(@"X", glossFont, scale);
    return metrics;
}

static inline msime::mac::CandidateItemWidths MSIMECandidateItemWidths(NSString *text, NSString *annotation, NSString *translation, NSFont *font, NSFont *glossFont, CGFloat scale = 1.0)
{
    msime::mac::CandidateItemWidths widths;
    widths.text = MSIMECandidateSingleLineWidth(text, font);
    widths.annotation = MSIMECandidateSingleLineWidth(annotation, font);
    widths.translation = MSIMECandidateSingleLineWidth(translation, glossFont);
    widths.translationHeight = MSIMECandidateGlossHeight(translation, glossFont, scale);
    return widths;
}

// Wrapped height of each run, as the layout asks for it: the text keeps the row's padding, a gloss its own.
static inline msime::mac::CandidateRunMeasure MSIMECandidateRunMeasure(NSString *text, NSString *annotation, NSString *translation, NSFont *font, NSFont *glossFont, CGFloat scale = 1.0, CGFloat rowPadding = -1.0)
{
    if (rowPadding < 0.0) rowPadding = MSIMECandidateRowPadding * scale;
    return [text = [text copy], annotation = [annotation copy], translation = [translation copy], font, glossFont, scale, rowPadding](msime::mac::CandidateRun run, double width) -> double {
        switch (run)
        {
        case msime::mac::CandidateRun::text:
            return MSIMECandidateWrappedHeight(text, font, width) + rowPadding;
        case msime::mac::CandidateRun::annotation:
            return MSIMECandidateWrappedHeight(annotation, font, width);
        case msime::mac::CandidateRun::translation:
            return MSIMECandidateWrappedHeight(translation, glossFont, width) + MSIMECandidateGlossPadding * scale;
        }
        return 0.0;
    };
}

// The text of a row's gloss run: the reading lines drawn first (a Korean Hanja's 훈음), then the translation, one line each. The reading is shown but never committed, so it stays out of the candidate's `translation`, and an armed translation column is counted from the first line after it.
static inline NSString *MSIMECandidateGlossRun(NSString *reading, NSString *translation)
{
    if (reading.length == 0) return translation ?: @"";
    return translation.length ? [NSString stringWithFormat:@"%@\n%@", reading, translation] : reading;
}

// 脚本插件给候选加的注释（候选的 `script_annotation`）最多画这么多 UTF-16 码元。runtime 已经按同一个上限拒收过长的注释，这里不依赖它，超出的在完整字符处截断并补「…」。
static const NSUInteger MSIMECandidateScriptAnnotationLimit = 32;
// 插件注释是第三方插件写的文字，画成斜体，和同在释义行里的译文、读音区分开；颜色仍是释义色，跟着皮肤和明暗走。
static const CGFloat MSIMECandidateScriptAnnotationObliqueness = 0.2;

// 把候选的 `script_annotation` 收拾成能安全画成一行的文本：不是字符串时为空；控制字符、格式字符（双向控制符、零宽字符）和各种换行（含 U+2028、U+2029，释义行正是按 "\n" 分行的）连成的一段换成一个空格，再去掉首尾空白；超过上限时在完整字符处截断并补「…」。
static inline NSString *MSIMECandidateScriptAnnotationRun(id value)
{
    if (![value isKindOfClass:NSString.class]) return @"";
    NSMutableCharacterSet *breaking = [NSMutableCharacterSet controlCharacterSet];
    [breaking formUnionWithCharacterSet:NSCharacterSet.newlineCharacterSet];
    NSMutableArray<NSString *> *pieces = [NSMutableArray array];
    for (NSString *piece in [(NSString *)value componentsSeparatedByCharactersInSet:breaking])
        if (piece.length) [pieces addObject:piece];
    NSString *line = [[pieces componentsJoinedByString:@" "] stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    if (line.length <= MSIMECandidateScriptAnnotationLimit) return line;
    NSUInteger end = 0;
    while (end < line.length)
    {
        const NSRange character = [line rangeOfComposedCharacterSequenceAtIndex:end];
        if (NSMaxRange(character) > MSIMECandidateScriptAnnotationLimit - 1) break;
        end = NSMaxRange(character);
    }
    return [[line substringToIndex:end] stringByAppendingString:@"…"];
}

// 释义行加上插件注释：插件注释单独占最后一行，排在输入法自己的读音、译文和逐词英文之后，不挪动它们在释义行里的位置。
static inline NSString *MSIMECandidateGlossWithScriptAnnotation(NSString *gloss, NSString *scriptAnnotation)
{
    if (scriptAnnotation.length == 0) return gloss ?: @"";
    return gloss.length ? [NSString stringWithFormat:@"%@\n%@", gloss, scriptAnnotation] : scriptAnnotation;
}

// 路径同时用于静态底色和选中动效；半径按相邻边长统一收缩，避免小格的曲线相交。
static CGPathRef MSIMECreateCandidateCornerPath(NSRect rect, std::array<double, 4> radii, BOOL flipped) CF_RETURNS_RETAINED
{
    const CGFloat width = MAX(0.0, NSWidth(rect)), height = MAX(0.0, NSHeight(rect));
    for (auto &radius : radii) radius = std::max(0.0, radius);
    double factor = 1.0;
    const auto fit = [&](double edge, double sum) { if (sum > 0) factor = std::min(factor, edge / sum); };
    fit(width, radii[0] + radii[1]); fit(width, radii[3] + radii[2]);
    fit(height, radii[0] + radii[3]); fit(height, radii[1] + radii[2]);
    for (auto &radius : radii) radius *= factor;
    const CGFloat left = NSMinX(rect), right = NSMaxX(rect), bottom = NSMinY(rect), top = NSMaxY(rect);
    const auto [tl, tr, br, bl] = radii;
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathMoveToPoint(path, nullptr, left + bl, bottom);
    CGPathAddLineToPoint(path, nullptr, right - br, bottom);
    CGPathAddArcToPoint(path, nullptr, right, bottom, right, bottom + br, br);
    CGPathAddLineToPoint(path, nullptr, right, top - tr);
    CGPathAddArcToPoint(path, nullptr, right, top, right - tr, top, tr);
    CGPathAddLineToPoint(path, nullptr, left + tl, top);
    CGPathAddArcToPoint(path, nullptr, left, top, left, top - tl, tl);
    CGPathAddLineToPoint(path, nullptr, left, bottom + bl);
    CGPathAddArcToPoint(path, nullptr, left, bottom, left + bl, bottom, bl);
    CGPathCloseSubpath(path);
    if (!flipped) return path;
    const CGAffineTransform flip = CGAffineTransformMake(1, 0, 0, -1, 0, bottom + top);
    CGPathRef result = CGPathCreateCopyByTransformingPath(path, &flip);
    CGPathRelease(path);
    return result;
}

@interface MSIMECandidateButton : NSButton <MSIMECandidateSelectionFillHost>
@property(nonatomic, copy) NSDictionary *candidateID;
@property(nonatomic, strong) NSFont *numberFont;
@property(nonatomic) BOOL candidateHighlighted;
@property(nonatomic) BOOL candidateFixed;
// AI 占位行（Engine 候选来源 12）：等回答时是「…」，回答为空或失败后是「—」，画成次要色，选它什么都不上屏。共享内核已经不再产出这种行（AI 回答到达时直接插在第一页最后一格），这里的处理留着，旧的库或数据里出现时仍按占位行对待。
@property(nonatomic) BOOL candidatePending;
@property(nonatomic, copy) NSString *translation;
// How each gloss line is read, "\n"-joined parallel to translation. Drawn on a line of its own under its gloss line and never part of what a gloss column commits.
@property(nonatomic, copy) NSString *pronunciation;
@property(nonatomic, strong) NSFont *translationFont;
@property(nonatomic, copy) NSColor *translationColor;
@property(nonatomic) BOOL translationBelow;
// The 辅助码 or engine annotation, drawn as its own run after the text and moved under it when it does not fit.
@property(nonatomic, copy) NSString *annotation;
// A reading drawn on the gloss run above the translation, in the gloss style, and never committed: a Korean Hanja's 훈음. Empty for every other row.
@property(nonatomic, copy) NSString *glossReading;
// 脚本插件给这一行加的注释，已经过 MSIMECandidateScriptAnnotationRun 收拾；画在释义行的最后一行，斜体，从不上屏，也不替换引擎的 annotation。没有时为空。
@property(nonatomic, copy) NSString *scriptAnnotation;
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
// 仅共享皮肤的贴角模式使用四角；关闭后保留原生统一圆角路径。
@property(nonatomic) BOOL hasCornerRadii;
@property(nonatomic) std::array<double, 4> cornerRadii;
@property(nonatomic) CGFloat selectionLeftInset;
// candidate_scale_percent as a factor, for the lengths the row draws itself: the text insets and the selection bar. Zero draws at 100%.
@property(nonatomic) CGFloat chromeScale;
// 显式零留白只在皮肤布局中生效；旧按钮继续使用原生默认。
@property(nonatomic) BOOL hasSkinLayout;
@property(nonatomic) BOOL hidesNumber;
@property(nonatomic) CGFloat textLeftInset;
@property(nonatomic) CGFloat textRightInset;
@property(nonatomic) CGFloat selectedBarWidth;
@property(nonatomic) CGFloat selectedBarHeight;
@property(nonatomic) BOOL selectedBarInside;
// 选中动效期间高亮底色由 MSIMECandidateMotion 的图层画，行只画文字和选中条。
@property(nonatomic) BOOL selectionFillDetached;
// 右键才构建操作菜单，普通输入不创建菜单树。
@property(nonatomic, copy) NSMenu *(^candidateMenuProvider)(void);
@end
@implementation MSIMECandidateButton
{
    NSTrackingArea *_candidateTrackingArea;
}
- (NSMenu *)menuForEvent:(NSEvent *)event
{
    if (self.candidateMenuProvider) return self.candidateMenuProvider();
    return [super menuForEvent:event];
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
// 读屏：候选行（标签不小于 0）高亮时报告自己被选中（AXSelected），别的行和首行的翻页、展开按钮不报告。按钮的无障碍元素是它的 cell，AppKit 读 AXSelected 时会转到这里。
- (BOOL)isAccessibilitySelected
{
    return self.tag >= 0 && self.candidateHighlighted;
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
- (CGPathRef)createSelectionFillPath
{
    if (!self.hasCornerRadii) return nullptr;
    return MSIMECreateCandidateCornerPath(NSInsetRect(self.bounds, 1, 1), self.cornerRadii, self.isFlipped);
}
- (void)drawRect:(NSRect)dirtyRect
{
    if (self.tag < 0) { [super drawRect:dirtyRect]; return; }
    (void)dirtyRect;
    NSRectClip(self.bounds);
    const CGFloat scale = self.chromeScale > 0.0 ? self.chromeScale : 1.0;
    NSColor *background = self.candidateHighlighted ? (self.selectionFillDetached ? nil : self.fillColor)
                                                     : (self.candidateHovered ? self.hoverColor : nil);
    if (background != nil && background.alphaComponent > 0.01)
    {
        [background setFill];
        CGPathRef path = [self createSelectionFillPath];
        if (path) {
            CGContextRef context = NSGraphicsContext.currentContext.CGContext;
            CGContextAddPath(context, path);
            CGContextFillPath(context);
            CGPathRelease(path);
        } else {
            [[NSBezierPath bezierPathWithRoundedRect:NSInsetRect(self.bounds, 1, 1) xRadius:self.cornerRadius yRadius:self.cornerRadius] fill];
        }
    }
    if (self.candidateHighlighted && self.showSelectedBar)
    {
        const CGFloat barHeight = self.selectedBarHeight > 0.0 ? self.selectedBarHeight : MAX(10.0 * scale, self.font.pointSize * 0.8);
        [self.barColor setFill];
        [[NSBezierPath
            bezierPathWithRoundedRect:NSMakeRect(self.hasSkinLayout ? (self.selectedBarInside ? MAX(1.0, self.selectionLeftInset) : -self.selectedBarWidth / 2.0) : MAX(1.0, self.selectionLeftInset),
                                                 (self.bounds.size.height - barHeight) / 2.0,
                                                 self.selectedBarWidth > 0.0 ? self.selectedBarWidth : 3.0 * scale, barHeight)
                              xRadius:(self.selectedBarWidth > 0.0 ? self.selectedBarWidth : 3.0 * scale) / 2.0
                              yRadius:barHeight / 2.0] fill];
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
    const CGFloat textLeft = self.hasSkinLayout ? self.textLeftInset : MSIMECandidateTextLeft(self.showSelectedBar, scale);
    const CGFloat textRight = self.hasSkinLayout ? self.textRightInset : MSIMECandidateTextRight * scale;
    const BOOL flipped = self.isFlipped;
    const CGFloat boundsHeight = self.bounds.size.height;
    // Layout boxes run downwards from the row top; convert them for either orientation of the view.
    NSRect (^box)(CGFloat, CGFloat, CGFloat, CGFloat) = ^NSRect(CGFloat x, CGFloat y, CGFloat width, CGFloat height) {
        return NSMakeRect(x, flipped ? y : boundsHeight - y - height, MAX(0.0, width), MAX(0.0, height));
    };
    if (split.location == NSNotFound)
    {
        const NSSize size = [title sizeWithAttributes:titleAttributes];
        const CGFloat maxWidth = MAX(0.0, self.bounds.size.width - textLeft - textRight);
        [title drawInRect:NSMakeRect(textLeft, (boundsHeight - size.height) / 2, maxWidth, size.height)
            withAttributes:titleAttributes];
        return;
    }
    NSString *number = [title substringToIndex:split.location];
    NSString *word = [title substringFromIndex:NSMaxRange(split)];
    NSString *annotation = self.annotation ?: @"";
    NSString *glossReading = self.glossReading ?: @"";
    NSMutableArray<NSValue *> *readingRanges = [NSMutableArray array];
    NSMutableArray<NSNumber *> *glossLineStarts = [NSMutableArray array];
    // What is measured and drawn; self.translation stays the committable gloss.
    NSString *glossDisplay = self.translation.length
        ? MSIMECandidateGlossDisplay(self.translation, self.pronunciation, readingRanges, glossLineStarts)
        : @"";
    NSString *scriptAnnotation = self.scriptAnnotation ?: @"";
    NSString *translation = MSIMECandidateGlossWithScriptAnnotation(MSIMECandidateGlossRun(glossReading, glossDisplay), scriptAnnotation);
    // The 훈음 lines come first in the run, so every range measured in the gloss moves past them.
    const NSUInteger glossOffset = glossReading.length && glossDisplay.length ? glossReading.length + 1 : 0;
    const NSSize numberSize = [number sizeWithAttributes:numberAttributes];
    const NSSize wordSize = [word sizeWithAttributes:titleAttributes];
    NSFont *glossFont = self.translationFont ?: [NSFont systemFontOfSize:MSIMECandidateTranslationPointSize * scale];
    const CGFloat contentLeft = self.contentLeft > 0.0 ? self.contentLeft : textLeft + numberSize.width + MSIMECandidateNumberGap * scale;
    msime::mac::CandidateItemLayout layout = self.itemLayout;
    if (!self.hasItemLayout)
    {
        // A button nobody laid out lays itself out in its own bounds, with the same rule the panel uses.
        const msime::mac::CandidateLayoutMetrics metrics =
            MSIMECandidateLayoutMetrics(self.font, glossFont, MAX(boundsHeight, MSIMECandidateTextHeight(word, self.font) + MSIMECandidateRowPadding * scale),
                                        contentLeft + textRight, scale);
        layout = msime::mac::LayoutCandidateItem(MSIMECandidateItemWidths(word, annotation, translation, self.font, glossFont, scale),
                                                 self.bounds.size.width - contentLeft - textRight, metrics,
                                                 self.translationBelow, MSIMECandidateRunMeasure(word, annotation, translation, self.font, glossFont, scale));
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
    if (!self.hidesNumber) [number drawInRect:box(textLeft, textTop + (firstLine - numberSize.height) / 2, ceil(numberSize.width) + 1.0, numberSize.height)
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
        // 选中底色不透明时读音不再减淡：那时的释义色已经是按底色挑出的次要色，再乘 0.72 在中等亮度的底色上对比度不到 3:1。读音单独一行，靠位置就和释义分得开。
        const BOOL opaqueSelection = self.candidateHighlighted && self.fillColor.alphaComponent >= 0.85;
        NSColor *readingColor = opaqueSelection ? glossColor : [glossColor colorWithAlphaComponent:glossColor.alphaComponent * MSIMECandidatePronunciationOpacity];
        for (NSValue *range in readingRanges)
            [glossText addAttribute:NSForegroundColorAttributeName value:readingColor
                              range:NSMakeRange(glossOffset + range.rangeValue.location, range.rangeValue.length)];
        if (scriptAnnotation.length)
            [glossText addAttribute:NSObliquenessAttributeName value:@(MSIMECandidateScriptAnnotationObliqueness)
                              range:NSMakeRange(translation.length - scriptAnnotation.length, scriptAnnotation.length)];
        if (self.armedGlossColumn > 0) {
            // The underline marks what the column commits: the gloss line, not the reading drawn under it nor the 훈음 lines before it.
            NSArray<NSString *> *parts = [self.translation componentsSeparatedByString:@"\n"];
            NSUInteger selected = (NSUInteger)(self.armedGlossColumn - 1);
            if (selected < parts.count && selected < glossLineStarts.count && parts[selected].length)
                [glossText addAttribute:NSUnderlineStyleAttributeName value:@((NSInteger)NSUnderlineStyleSingle)
                                  range:NSMakeRange(glossOffset + glossLineStarts[selected].unsignedIntegerValue, parts[selected].length)];
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
// The transparent band above the card that a skin's decoration stands in. The fill, background image and stroke cover only the card below it.
@property(nonatomic) CGFloat cardTopInset;
// 候选列表的实际区域；画布平移时与真实按钮一起移动。
@property(nonatomic) NSRect candidateRowsRect;
@property(nonatomic) BOOL candidateRowsTouchTop;
// 系统主题允许原生玻璃；旧系统和降低透明度时仍画原有卡片。
@property(nonatomic) BOOL liquidGlassEnabled;
@property(nonatomic, readonly) BOOL liquidGlassActive;
@property(nonatomic, strong, readonly) NSView *candidateContentView;
@property(nonatomic) msime::mac::SkinInsets canvasOuterInsets;
@property(nonatomic) BOOL canvasPanelLoaded;
@property(nonatomic, strong, readonly) MSIMECandidateCanvasAssets *canvasAssets;
- (void)drawCandidateCard;
- (void)configureCanvas:(const msime::mac::SkinCanvas &)canvas assets:(MSIMECandidateCanvasAssets *)assets;
- (void)setCanvasPlaybackVisible:(BOOL)visible anchor:(CFTimeInterval)anchor;
@end

@interface MSIMECandidateCardBackgroundView : NSView
@property(nonatomic, weak) MSIMECandidateChromeView *owner;
@end
@implementation MSIMECandidateCardBackgroundView
- (BOOL)isOpaque { return NO; }
- (NSView *)hitTest:(NSPoint)point { (void)point; return nil; }
- (void)drawRect:(NSRect)dirtyRect { (void)dirtyRect; [self.owner drawCandidateCard]; }
@end
@implementation MSIMECandidateChromeView
{
    NSView *_glassBackground;
    NSView *_candidateContentView;
    MSIMECandidateCardBackgroundView *_cardBackground;
    NSArray<MSIMECandidateCanvasView *> *_canvasViews;
}
- (instancetype)initWithFrame:(NSRect)frame
{
    self = [super initWithFrame:frame];
    if (self) {
        _cardBackground = [[MSIMECandidateCardBackgroundView alloc] initWithFrame:self.bounds];
        _cardBackground.owner = self;
        _cardBackground.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [self addSubview:_cardBackground];
        _candidateContentView = [[NSView alloc] initWithFrame:self.bounds];
        _candidateContentView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [self addSubview:_candidateContentView];
        [NSWorkspace.sharedWorkspace.notificationCenter addObserver:self
            selector:@selector(accessibilityDisplayOptionsChanged:)
            name:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:nil];
    }
    return self;
}
- (NSView *)candidateContentView { return _candidateContentView; }
- (BOOL)liquidGlassActive { return _glassBackground != nil; }
- (void)dealloc
{
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
}
- (void)accessibilityDisplayOptionsChanged:(NSNotification *)notification
{
    (void)notification;
    [self updateGlassBackground];
}
- (NSRect)cardBounds
{
    const auto outer = self.canvasOuterInsets;
    return NSMakeRect(NSMinX(self.bounds) + outer.left, NSMinY(self.bounds) + outer.bottom,
                     MAX(0.0, NSWidth(self.bounds) - outer.left - outer.right),
                     MAX(0.0, NSHeight(self.bounds) - MAX(0.0, self.cardTopInset) - outer.top - outer.bottom));
}
- (void)setLiquidGlassEnabled:(BOOL)enabled
{
    _liquidGlassEnabled = enabled;
    [self updateGlassBackground];
}
- (void)updateGlassBackground
{
    if (@available(macOS 26.0, *)) {
        if (self.liquidGlassEnabled && !NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency) {
            NSGlassEffectView *glass = (NSGlassEffectView *)_glassBackground;
            if (!glass) {
                glass = [[NSGlassEffectView alloc] initWithFrame:self.cardBounds];
                glass.style = NSGlassEffectViewStyleRegular;
                glass.identifier = @"candidate-liquid-glass";
                // 真实候选进入材质的内容容器，系统才能处理文字与背景的可读性。
                _glassBackground = glass;
                glass.contentView = _candidateContentView;
                [self addSubview:glass positioned:NSWindowBelow relativeTo:nil];
                _glassBackground = glass;
            }
            glass.frame = self.cardBounds;
            glass.cornerRadius = self.cornerRadius;
            _candidateContentView.frame = glass.bounds;
            [glass layoutSubtreeIfNeeded];
            // 透明度由系统材质管理，不能淡化承载候选文字的整棵视图。
            glass.alphaValue = 1.0;
            _cardBackground.needsDisplay = YES;
            self.needsDisplay = YES;
            return;
        }
    }
    if (@available(macOS 26.0, *)) {
        if (_glassBackground) ((NSGlassEffectView *)_glassBackground).contentView = nil;
    }
    [_glassBackground removeFromSuperview];
    _glassBackground = nil;
    if (_candidateContentView.superview != self)
        [self addSubview:_candidateContentView positioned:NSWindowAbove relativeTo:_cardBackground];
    _candidateContentView.translatesAutoresizingMaskIntoConstraints = YES;
    _candidateContentView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _candidateContentView.frame = self.cardBounds;
    _cardBackground.needsDisplay = YES;
    self.needsDisplay = YES;
}
- (void)layout
{
    [super layout];
    [self updateGlassBackground];
}
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
- (void)setNeedsDisplay:(BOOL)value
{
    [super setNeedsDisplay:value];
    if (value) _cardBackground.needsDisplay = YES;
}
- (void)configureCanvas:(const msime::mac::SkinCanvas &)canvas assets:(MSIMECandidateCanvasAssets *)assets
{
    for (MSIMECandidateCanvasView *view in _canvasViews) [view removeFromSuperview];
    _canvasAssets = assets;
    self.canvasPanelLoaded = assets.panelLoaded;
    NSMutableArray *views = [NSMutableArray array];
    for (NSNumber *level in @[@(-1), @0, @1]) {
        MSIMECandidateCanvasView *view = [[MSIMECandidateCanvasView alloc] initWithFrame:self.bounds assets:assets canvas:canvas card:self.cardBounds level:level.integerValue];
        view.identifier = [NSString stringWithFormat:@"candidate-canvas-%ld", (long)level.integerValue];
        if (level.integerValue < 0) [self addSubview:view positioned:NSWindowBelow relativeTo:_cardBackground];
        else if (level.integerValue == 0) [self addSubview:view positioned:NSWindowAbove relativeTo:_cardBackground];
        else [self addSubview:view positioned:NSWindowAbove relativeTo:nil];
        [views addObject:view];
    }
    _canvasViews = views;
    self.needsDisplay = YES;
}
- (void)setCanvasPlaybackVisible:(BOOL)visible anchor:(CFTimeInterval)anchor
{
    for (MSIMECandidateCanvasView *view in _canvasViews) [view setPlaybackVisible:visible anchor:anchor];
}
- (void)drawCandidateCard
{
    // 原生玻璃提供底色和边缘高光，不再叠一层不透明填色把材质盖住。
    if (_glassBackground || self.canvasPanelLoaded) return;
    // 坐标未翻转：装饰带在顶部，卡片只占下面的部分。
    const NSRect bounds = self.cardBounds;
    NSBezierPath *path = [NSBezierPath bezierPathWithRoundedRect:bounds
                                                         xRadius:self.cornerRadius
                                                         yRadius:self.cornerRadius];
    [(self.fillColor != nil ? self.fillColor : NSColor.windowBackgroundColor) setFill];
    [path fill];
    NSImage *background = self.backgroundImage;
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

// 所有候选身份校验与遍历都通过当前内容容器，普通测试面板仍可直接放控件。
static inline NSView *MSIMECandidateControlsView(NSView *view) {
    return [view isKindOfClass:MSIMECandidateChromeView.class]
        ? ((MSIMECandidateChromeView *)view).candidateContentView : view;
}
