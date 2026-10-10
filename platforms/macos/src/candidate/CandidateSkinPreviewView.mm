// Adapted from MSIME-Apple b637828e15eafcb5e459edd270a962dd14517285.
#import "CandidateSkinPreviewView.h"
#import "../settings/AppearancePreferences.h"
#import "CandidateTextMetrics.h"
#import "CandidateTypography.h"

static NSColor *PreviewColor(msime::mac::Rgba color) {
    return [NSColor colorWithSRGBRed:color.r green:color.g blue:color.b alpha:color.a];
}

namespace
{
NSArray<NSString *> *PreviewSamples();

// The candidate list the preview draws: the words typed into 预览示例文字, split on whitespace, or the built-in samples while that field is empty. A word is cut at 24 composed characters and the list at 32 words, because the field is free text and one pasted paragraph would otherwise set the row height of every preview in the window.
NSArray<NSString *> *PreviewSampleWords(NSString *text)
{
    NSMutableArray<NSString *> *words = [NSMutableArray array];
    for (NSString *token in [text componentsSeparatedByCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet])
    {
        if (token.length == 0) continue;
        NSString *word = token.length > 24
                             ? [token substringWithRange:[token rangeOfComposedCharacterSequencesForRange:NSMakeRange(0, 24)]]
                             : token;
        [words addObject:word];
        if (words.count == 32) break;
    }
    return words.count > 0 ? words : PreviewSamples();
}

// One page of candidates out of the sample list. A page holds what 每页候选 says it holds, so a shorter list is cycled rather than allowed to shorten the page: a user who types one word into 预览示例文字 is asking what that word looks like, not to be shown a page of one.
NSArray<NSString *> *PreviewPageWords(NSArray<NSString *> *words, NSInteger count)
{
    NSMutableArray<NSString *> *page = [NSMutableArray arrayWithCapacity:MAX(count, (NSInteger)0)];
    for (NSInteger index = 0; index < count; ++index)
        [page addObject:words[static_cast<NSUInteger>(index) % words.count]];
    return page;
}

CGFloat PreviewCandidateHeight(NSArray<NSString *> *words, NSFont *font) {
    CGFloat height = MSIMECandidateTextHeight(@"", font);
    for (NSString *sample in words) height = MAX(height, MSIMECandidateTextHeight(sample, font));
    return height + 8.0;
}

// How many rows a vertical list shows, and what the corner says about the rest. The candidate window is a panel the size of its content and the preview is one column of a settings page, so nine candidates do not fit at every font size the window offers; five rows plus a count of what is left over is the compromise, and both the single preview and the showcase's 竖排候选 section ask here so that neither can drift from the other.
constexpr NSInteger kMaximumPreviewRows = 5;
NSInteger PreviewVisibleRows(NSInteger pageSize) { return MIN(MAX(pageSize, (NSInteger)1), kMaximumPreviewRows); }
NSString *PreviewPendingFooter(NSInteger pageSize)
{
    const NSInteger pending = MAX(pageSize, (NSInteger)1) - PreviewVisibleRows(pageSize);
    return pending > 0 ? [NSString stringWithFormat:@"另有 %ld 个", static_cast<long>(pending)] : nil;
}
CGFloat PreviewPreeditHeight(CGFloat preeditFontSize, MSIMEAppearancePreferences *preferences)
{
    if (preeditFontSize <= 0) return 0;
    NSFont *font = [preferences candidateFontOfSize:preeditFontSize englishFirst:YES] ?: [NSFont systemFontOfSize:preeditFontSize];
    return MAX(22.0, MSIMECandidateTextHeight(@"nihao", font) + 6.0);
}

// The three toolbar settings the preview draws from, in the shape the drawing wants them: one bit per component in the order of FloatingToolbarComponentKeys(), the percentage 工具栏缩放 stores, and the point size 工具栏字号 stores.
struct ToolbarPreviewInputs
{
    NSUInteger components;
    CGFloat scalePercent;
    CGFloat fontSize;
    // 偏好 `show_app_logo`：NO 时左端画握把而不是 logo。
    BOOL logo;
};

ToolbarPreviewInputs ToolbarInputs(MSIMEAppearancePreferences *preferences)
{
    // What MetasequoiaFloatingToolbarPanel -applySizingPreferences: makes of an empty dictionary, which is the state a preview with no preferences behind it is in: 100%, 24pt, and every component but the screen keyboard.
    static const BOOL defaults[10] = {YES, YES, YES, YES, YES, YES, YES, NO, YES, YES};
    ToolbarPreviewInputs inputs = {0, 100.0, 24.0, YES};
    NSArray<NSNumber *> *enabled = preferences == nil
        ? nil
        : @[@(preferences.floatingToolbarEnglishMode), @(preferences.floatingToolbarInputScheme), @(preferences.floatingToolbarPunctuation),
            @(preferences.floatingToolbarFullWidth), @(preferences.floatingToolbarCharacterSet),
            @(preferences.floatingToolbarEmoji), @(preferences.floatingToolbarHandwriting),
            @(preferences.floatingToolbarScreenKeyboard), @(preferences.floatingToolbarVoice),
            @(preferences.floatingToolbarSettings)];
    for (NSUInteger index = 0; index < sizeof(defaults) / sizeof(defaults[0]); ++index)
        if (enabled != nil ? enabled[index].boolValue : defaults[index]) inputs.components |= 1u << index;
    if (preferences != nil)
    {
        inputs.scalePercent = preferences.floatingToolbarScalePercent;
        inputs.fontSize = preferences.floatingToolbarFontSize;
        inputs.logo = preferences.showsAppLogo;
    }
    return inputs;
}

// The size the panel gives itself for these settings. It is ToolbarPreferredWidth() and the height beside it in FloatingToolbarPanel.mm, constant for constant: the whole point of drawing the toolbar here is that the size is the thing being previewed, so a preview with metrics of its own would be answering a different question.
NSSize ToolbarPreviewSize(NSUInteger components, CGFloat scalePercent, CGFloat fontSize, BOOL logo)
{
    const CGFloat scale = scalePercent / 100.0;
    CGFloat count = 0.0;
    for (NSUInteger index = 0; index < 10; ++index) count += (components & (1u << index)) != 0 ? 1.0 : 0.0;
    const CGFloat gaps = count > 0.0 ? count - 1.0 : 0.0;
    // 没有 logo 时左端是 10pt 的握把而不是 34pt 的 logo，前导段从 46.2pt 缩到 22.2pt。
    return NSMakeSize(ceil((count * (fontSize + 8.0) + gaps * 2.0 + 6.0 + (logo ? 46.2 : 22.2)) * scale),
                      ceil((fontSize + 20.0) * scale));
}

// The card, as MSIMECandidateChromeView draws it: the surface, the package background clipped to the rounded outline, then the border.
void DrawSkinChrome(NSRect rect, const msime::mac::ResolvedSkin &skin)
{
    const msime::mac::SkinTokens &tokens = skin.tokens;
    NSBezierPath *path = [NSBezierPath bezierPathWithRoundedRect:rect xRadius:tokens.radius yRadius:tokens.radius];
    [PreviewColor(tokens.surface) setFill];
    [path fill];
    NSImage *background = skin.backgroundPath.empty() || !(skin.backgroundOpacity > 0.0)
        ? nil
        : [[NSImage alloc] initWithContentsOfFile:@(skin.backgroundPath.c_str())];
    const auto rects = background != nil
        ? msime::mac::BackgroundRects(skin.backgroundFit, {NSMinX(rect), NSMinY(rect), NSWidth(rect), NSHeight(rect)},
                                      background.size.width, background.size.height)
        : std::nullopt;
    if (rects)
    {
        [NSGraphicsContext saveGraphicsState];
        [path addClip];
        [background drawInRect:NSMakeRect(rects->destination.x, rects->destination.y, rects->destination.width, rects->destination.height)
                      fromRect:NSMakeRect(rects->source.x, rects->source.y, rects->source.width, rects->source.height)
                     operation:NSCompositingOperationSourceOver
                      fraction:MIN(1.0, skin.backgroundOpacity)
                respectFlipped:YES
                         hints:nil];
        [NSGraphicsContext restoreGraphicsState];
    }
    if (tokens.borderWidth > 0.0 && tokens.border.a > 0.01f)
    {
        [PreviewColor(tokens.border) setStroke];
        path.lineWidth = tokens.borderWidth;
        [path stroke];
    }
}

struct SkinPreviewMetrics
{
    CGFloat fontSize;
    CGFloat captionHeight;
    CGFloat captionGap;
    CGFloat sectionGap;
    CGFloat top;
    CGFloat bottom;
    CGFloat preeditHeight;
    CGFloat rowHeight;
    CGFloat decorationHeight;
    CGFloat panelHeight;
    CGFloat horizontalHeight;
    CGFloat verticalHeight;
    CGFloat toolbarHeight;
    CGFloat totalHeight;
};

// The showcase draws the same panel as the single preview, twice over and with the toolbar beside it. It used to draw a panel of its own: the candidate font was clamped to 15pt and the two lists held five and four candidates whatever 每页候选 said, so ticking 同时预览横排、竖排与状态栏 quietly disconnected the preview from the two controls under it that it exists to answer for. Everything here now comes from the settings, and what does not fit is disclosed — the footer counts the candidates below the fifth row, and a row too wide for the column ends in an ellipsis.
// `scale` is 整体大小: the panels are measured at 100% and take that much more room, because DrawScaledPreviewCandidates draws them at that size.
SkinPreviewMetrics MakeShowcaseMetrics(NSInteger pageSize, CGFloat candidateFontSize, CGFloat decorationTop,
                                       CGFloat preeditFontSize, NSArray<NSString *> *words,
                                       MSIMEAppearancePreferences *preferences, CGFloat scale)
{
    SkinPreviewMetrics metrics;
    metrics.fontSize = MAX(12.0, candidateFontSize);
    NSFont *font = [preferences candidateFontOfSize:metrics.fontSize englishFirst:YES] ?: [NSFont systemFontOfSize:metrics.fontSize];
    metrics.captionHeight = 16.0;
    metrics.captionGap = 4.0;
    metrics.sectionGap = 10.0;
    metrics.top = 10.0;
    metrics.bottom = 14.0;
    metrics.preeditHeight = PreviewPreeditHeight(preeditFontSize, preferences);
    metrics.rowHeight = PreviewCandidateHeight(words, font);
    metrics.decorationHeight = MAX(0.0, decorationTop);
    const CGFloat footer = PreviewPendingFooter(pageSize) != nil ? 18.0 : 0.0;
    metrics.horizontalHeight = (6.0 + metrics.decorationHeight + metrics.preeditHeight + metrics.rowHeight + 6.0) * scale;
    metrics.verticalHeight = (6.0 + metrics.decorationHeight + metrics.preeditHeight +
                              PreviewVisibleRows(pageSize) * metrics.rowHeight + footer + 6.0) * scale;
    const ToolbarPreviewInputs toolbar = ToolbarInputs(preferences);
    metrics.toolbarHeight = ToolbarPreviewSize(toolbar.components, toolbar.scalePercent, toolbar.fontSize, toolbar.logo).height;
    metrics.panelHeight = 0.0;
    metrics.totalHeight = metrics.top + metrics.captionHeight + metrics.captionGap + metrics.horizontalHeight +
                          metrics.sectionGap + metrics.captionHeight + metrics.captionGap + metrics.verticalHeight +
                          metrics.sectionGap + metrics.captionHeight + metrics.captionGap + metrics.toolbarHeight +
                          metrics.bottom;
    return metrics;
}

SkinPreviewMetrics MakeAppearanceMetrics(NSInteger panelStyle, NSInteger pageSize, CGFloat candidateFontSize,
                                         CGFloat decorationTop, CGFloat preeditFontSize,
                                         NSArray<NSString *> *words, MSIMEAppearancePreferences *preferences, CGFloat scale)
{
    SkinPreviewMetrics metrics;
    metrics.fontSize = MAX(12.0, candidateFontSize);
    NSFont *font = [preferences candidateFontOfSize:metrics.fontSize englishFirst:YES] ?: [NSFont systemFontOfSize:metrics.fontSize];
    metrics.captionHeight = 16.0;
    metrics.captionGap = 4.0;
    metrics.sectionGap = 0.0;
    metrics.top = 10.0;
    metrics.bottom = 14.0;
    metrics.preeditHeight = PreviewPreeditHeight(preeditFontSize, preferences);
    metrics.rowHeight = PreviewCandidateHeight(words, font);
    metrics.decorationHeight = MAX(0.0, decorationTop);
    const NSInteger visibleRows = panelStyle == 1 ? PreviewVisibleRows(pageSize) : 1;
    const CGFloat footer = (panelStyle == 1 && PreviewPendingFooter(pageSize) != nil) ? 18.0 : 0.0;
    metrics.panelHeight =
        (6.0 + metrics.decorationHeight + metrics.preeditHeight + visibleRows * metrics.rowHeight + footer + 6.0) * scale;
    metrics.horizontalHeight = 0.0;
    metrics.verticalHeight = 0.0;
    metrics.toolbarHeight = 0.0;
    metrics.totalHeight =
        metrics.top + metrics.captionHeight + metrics.captionGap + metrics.panelHeight + metrics.bottom;
    return metrics;
}

void DrawAlignedString(NSString *text, NSRect row, CGFloat x, NSDictionary *attributes)
{
    NSSize size = [text sizeWithAttributes:attributes];
    [text drawAtPoint:NSMakePoint(x, NSMinY(row) + (NSHeight(row) - size.height) / 2.0) withAttributes:attributes];
}

void DrawSelectedBar(NSRect row, const msime::mac::SkinTokens &tokens, CGFloat fontSize)
{
    if (!tokens.showSelectedBar)
    {
        return;
    }
    const CGFloat barHeight = MAX(10.0, fontSize * 0.8);
    NSRect bar = NSMakeRect(NSMinX(row) + 3.0, NSMinY(row) + (NSHeight(row) - barHeight) / 2.0, 3.0, barHeight);
    [PreviewColor(tokens.accent) setFill];
    [[NSBezierPath bezierPathWithRoundedRect:bar xRadius:1.5 yRadius:1.5] fill];
}

// Drawn after the card, over its top edge, placed as the candidate window places it; `card` is in this flipped view, with the transparent band above it.
void DrawDecoration(NSRect card, CGFloat pad, const msime::mac::ResolvedSkin &skin)
{
    if (skin.decorationTopDip <= 0.0 || skin.decorationPath.empty())
    {
        return;
    }
    NSImage *image = [[NSImage alloc] initWithContentsOfFile:@(skin.decorationPath.c_str())];
    const auto placed = image == nil ? std::nullopt
                                     : msime::mac::DecorationPlacement(skin.decorationAlign, NSWidth(card), pad, skin.decorationTopDip,
                                                                       skin.decorationWidthDip, image.size.width, image.size.height);
    if (!placed)
    {
        return;
    }
    const NSRect imageRect = NSMakeRect(NSMinX(card) + placed->x, NSMinY(card) - skin.decorationTopDip + placed->top,
                                        placed->width, placed->height);
    [image drawInRect:imageRect
              fromRect:NSZeroRect
             operation:NSCompositingOperationSourceOver
              fraction:1.0
        respectFlipped:YES
                 hints:nil];
}

// The row height is the caller's rather than this function's own: `words` is the visible page, which a wide candidate or a narrow column can cut short, while the height of a row is measured over the whole sample list. Measuring it here would make the panel shrink as the list it holds is truncated.
void DrawPreviewCandidates(NSRect rect, const msime::mac::ResolvedSkin &skin, BOOL vertical,
                           NSArray<NSString *> *words, CGFloat fontSize, CGFloat rowHeight, NSString *footer, CGFloat preeditFontSize, MSIMEAppearancePreferences *preferences)
{
    const msime::mac::SkinTokens &tokens = skin.tokens;
    const CGFloat decorationTop = MAX(0.0, skin.decorationTopDip);
    NSRect chrome =
        NSMakeRect(NSMinX(rect), NSMinY(rect) + decorationTop, NSWidth(rect), NSHeight(rect) - decorationTop);
    DrawSkinChrome(chrome, skin);
    NSBezierPath *clip = [NSBezierPath bezierPathWithRoundedRect:NSInsetRect(chrome, 1.0, 1.0)
                                                         xRadius:MAX(1.0, tokens.radius - 1.0)
                                                         yRadius:MAX(1.0, tokens.radius - 1.0)];
    [NSGraphicsContext saveGraphicsState];
    [clip addClip];

    NSFont *font = [preferences candidateFontOfSize:fontSize englishFirst:YES] ?: [NSFont systemFontOfSize:fontSize];
    NSFont *preeditFont = [preferences candidateFontOfSize:MAX(11.0, preeditFontSize) englishFirst:YES] ?: [NSFont systemFontOfSize:MAX(11.0, preeditFontSize)];
    NSFont *numberFont = [NSFont monospacedDigitSystemFontOfSize:MAX(10.0, fontSize - 4.0) weight:NSFontWeightRegular];
    NSDictionary *preeditAttributes = @{
        NSFontAttributeName : preeditFont,
        NSForegroundColorAttributeName : PreviewColor(tokens.text),
    };
    const CGFloat pad = 6.0;
    const CGFloat preeditHeight = PreviewPreeditHeight(preeditFontSize, preferences);
    NSRect preeditRow =
        NSMakeRect(NSMinX(chrome) + pad, NSMinY(chrome) + pad, NSWidth(chrome) - pad * 2.0, preeditHeight);
    if (preeditFontSize > 0) {
        // The brand mark leads the reading, as the candidate window's top row draws it.
        static NSImage *logo;
        static dispatch_once_t once;
        dispatch_once(&once, ^{
            NSString *path = [[NSBundle bundleForClass:MSIMECandidatePreviewView.class] pathForResource:@"MSIMEClientInputMethod" ofType:@"icns"];
            logo = path == nil ? nil : [[NSImage alloc] initWithContentsOfFile:path];
        });
        CGFloat readingX = NSMinX(preeditRow);
        if (logo != nil && (preferences == nil || preferences.showsAppLogo)) {
            const CGFloat side = MIN(MSIMECandidateLogoSide, NSHeight(preeditRow));
            [logo drawInRect:NSMakeRect(readingX + 2.0, NSMidY(preeditRow) - side / 2.0, side, side)
                    fromRect:NSZeroRect
                   operation:NSCompositingOperationSourceOver
                    fraction:1.0
              respectFlipped:YES
                       hints:nil];
            readingX += MSIMECandidateLogoSide + MSIMECandidateLogoGap;
        }
        DrawAlignedString(@"nihao", preeditRow, readingX, preeditAttributes);
        const CGFloat caretX = readingX + [@"nihao" sizeWithAttributes:preeditAttributes].width + 2.0;
        NSRect caret = NSMakeRect(caretX, NSMinY(preeditRow) + 3.0, 1.5, NSHeight(preeditRow) - 6.0);
        [PreviewColor(tokens.accent) setFill];
        NSRectFill(caret);
    }

    const CGFloat textInset = 8.0 + (tokens.showSelectedBar ? 6.0 : 0.0);
    const CGFloat contentTop = NSMinY(chrome) + pad + preeditHeight;
    CGFloat x = NSMinX(chrome) + pad;
    const CGFloat maxX = NSMaxX(chrome) - pad;
    for (NSInteger index = 0; index < static_cast<NSInteger>(words.count); ++index)
    {
        const BOOL selected = index == 0;
        NSDictionary *numberAttributes = @{
            NSFontAttributeName : numberFont,
            NSForegroundColorAttributeName :
                PreviewColor(selected && tokens.selected.a >= 0.85f ? tokens.selectedNumber : tokens.number),
        };
        NSDictionary *wordAttributes = @{
            NSFontAttributeName : font,
            NSForegroundColorAttributeName : selected ? PreviewColor(tokens.selectedText) : PreviewColor(tokens.text),
        };
        NSString *number = [NSString stringWithFormat:@"%ld", static_cast<long>(index + 1)];
        NSString *word = words[index];
        const CGFloat numberWidth = [number sizeWithAttributes:numberAttributes].width;
        const CGFloat wordWidth = [word sizeWithAttributes:wordAttributes].width;
        const CGFloat itemWidth = textInset + numberWidth + 6.0 + wordWidth + 8.0;
        if (!vertical && x + itemWidth > maxX)
        {
            NSDictionary *ellipsisAttributes = @{
                NSFontAttributeName : font,
                NSForegroundColorAttributeName : PreviewColor(tokens.text),
            };
            NSSize ellipsisSize = [@"…" sizeWithAttributes:ellipsisAttributes];
            if (x + ellipsisSize.width <= maxX)
            {
                DrawAlignedString(@"…", NSMakeRect(x, contentTop, ellipsisSize.width, rowHeight), x,
                                  ellipsisAttributes);
            }
            break;
        }
        NSRect row = vertical ? NSMakeRect(NSMinX(chrome) + 4.0, contentTop + index * rowHeight, NSWidth(chrome) - 8.0,
                                           rowHeight)
                              : NSMakeRect(x, contentTop, itemWidth, rowHeight);
        if (NSMaxY(row) > NSMaxY(chrome) - 2.0)
        {
            break;
        }
        if (selected)
        {
            [PreviewColor(tokens.selected) setFill];
            [[NSBezierPath bezierPathWithRoundedRect:row
                                             xRadius:tokens.selectedRadius
                                             yRadius:tokens.selectedRadius]
                fill];
            DrawSelectedBar(row, tokens, fontSize);
        }
        const CGFloat textX = NSMinX(row) + textInset;
        DrawAlignedString(number, row, textX, numberAttributes);
        DrawAlignedString(word, row, textX + numberWidth + 6.0, wordAttributes);
        if (!vertical)
        {
            x += itemWidth;
        }
    }
    if (footer.length > 0)
    {
        NSDictionary *footerAttributes = @{
            NSFontAttributeName : [NSFont systemFontOfSize:11.0 weight:NSFontWeightSemibold],
            NSForegroundColorAttributeName : [NSColor secondaryLabelColor],
        };
        NSSize footerSize = [footer sizeWithAttributes:footerAttributes];
        [footer drawAtPoint:NSMakePoint(NSMaxX(chrome) - footerSize.width - 12.0,
                                        NSMaxY(chrome) - footerSize.height - 8.0)
             withAttributes:footerAttributes];
    }
    [NSGraphicsContext restoreGraphicsState];
    DrawDecoration(chrome, pad, skin);
}

// The panel at 整体大小: laid out at 100% in a rect that much smaller and drawn magnified into `rect`, which is what the candidate window does by multiplying its fonts and lengths. A row that no longer fits the column at the larger size ends in an ellipsis as it would at 100% in a narrower column.
void DrawScaledPreviewCandidates(NSRect rect, CGFloat scale, const msime::mac::ResolvedSkin &skin, BOOL vertical,
                                 NSArray<NSString *> *words, CGFloat fontSize, CGFloat rowHeight, NSString *footer,
                                 CGFloat preeditFontSize, MSIMEAppearancePreferences *preferences)
{
    [NSGraphicsContext saveGraphicsState];
    NSAffineTransform *transform = [NSAffineTransform transform];
    [transform translateXBy:NSMinX(rect) yBy:NSMinY(rect)];
    [transform scaleBy:scale];
    [transform concat];
    DrawPreviewCandidates(NSMakeRect(0.0, 0.0, NSWidth(rect) / scale, NSHeight(rect) / scale), skin, vertical, words, fontSize,
                          rowHeight, footer, preeditFontSize, preferences);
    [NSGraphicsContext restoreGraphicsState];
}

// What each component puts on its button, in the order of FloatingToolbarComponentKeys(): a title, or the SF Symbol the panel gives the button instead. The four titled buttons carry the state they toggle, so these are the ones the panel starts in — Chinese input, Chinese punctuation, half width, simplified output.
NSArray<NSArray<NSString *> *> *ToolbarPreviewGlyphs()
{
    return @[ @[@"中", @""], @[@"", @"list.bullet"], @[@"。", @""], @[@"半", @""], @[@"简", @""], @[@"", @"face.smiling"],
              @[@"", @"hand.draw"], @[@"", @"keyboard"], @[@"", @"mic.fill"], @[@"", @"gearshape"] ];
}

// The toolbar as the panel would build it for these settings, drawn into `slot` at the panel's own metrics: the logo, the divider, and one button per ticked component, sized (font + 8) x (font + 8) and spaced 2pt before everything is multiplied by the scale. A toolbar wider than the column is drawn down to fit rather than clipped — losing the trailing buttons would hide exactly the thing 工具栏缩放 changes — and the factor comes back so the caller can say so. It is never drawn up: 75% has to look smaller than 100%.
CGFloat DrawPreviewToolbar(NSRect slot, const msime::mac::SkinTokens &tokens, NSUInteger components,
                           CGFloat scalePercent, CGFloat fontSize, BOOL showsLogo)
{
    const NSSize natural = ToolbarPreviewSize(components, scalePercent, fontSize, showsLogo);
    const CGFloat fit = NSWidth(slot) > 0.0 ? MIN(1.0, NSWidth(slot) / natural.width) : 1.0;
    const CGFloat scale = scalePercent / 100.0;
    const CGFloat buttonHeight = (fontSize + 8.0) * scale;
    const CGFloat buttonWidth = (fontSize + 8.0) * scale;
    const CGFloat buttonTop = (natural.height - buttonHeight) / 2.0;
    [NSGraphicsContext saveGraphicsState];
    NSAffineTransform *transform = [NSAffineTransform transform];
    [transform translateXBy:NSMinX(slot) yBy:NSMinY(slot) + (NSHeight(slot) - natural.height * fit) / 2.0];
    [transform scaleBy:fit];
    [transform concat];
    NSBezierPath *chrome = [NSBezierPath bezierPathWithRoundedRect:NSMakeRect(0.5, 0.5, natural.width - 1.0, natural.height - 1.0)
                                                          xRadius:tokens.radius * scale
                                                          yRadius:tokens.radius * scale];
    [PreviewColor(tokens.surface) setFill];
    [chrome fill];
    if (tokens.border.a > 0.01f)
    {
        [PreviewColor(tokens.border) setStroke];
        chrome.lineWidth = 1.0;
        [chrome stroke];
    }
    // The logo, which is also the panel's drag handle: the application icon centred in the 38pt slot of the 34pt mark and its 4pt gap, inset 2pt as the panel draws it. The panel's vector fallback for a missing icon is not repeated here; without the icon the slot stays empty.
    static NSImage *logo;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSString *path = [[NSBundle bundleForClass:MSIMEToolbarPreviewView.class] pathForResource:@"MSIMEClientInputMethod" ofType:@"icns"];
        logo = path == nil ? nil : [[NSImage alloc] initWithContentsOfFile:path];
    });
    if (showsLogo)
    {
        const CGFloat logoSlot = 38.0 * scale;
        const CGFloat logoSide = MIN(logoSlot, natural.height) - 4.0 * scale;
        [logo drawInRect:NSMakeRect((logoSlot - logoSide) / 2.0, (natural.height - logoSide) / 2.0, logoSide, logoSide)
                fromRect:NSZeroRect
               operation:NSCompositingOperationSourceOver
                fraction:1.0
          respectFlipped:YES
                   hints:nil];
    }
    else
    {
        // 不显示 logo 时的握把：10pt 宽的槽里两列三行小圆点，颜色同分隔线，和工具栏本身画的一样。
        const CGFloat dot = 2.4 * scale;
        const CGFloat pitch = 4.4 * scale;
        [PreviewColor(tokens.divider.value_or(tokens.border)) setFill];
        for (int column = 0; column < 2; ++column)
            for (int row = 0; row < 3; ++row)
            {
                const NSPoint centre = NSMakePoint(5.0 * scale + (column - 0.5) * pitch, natural.height / 2.0 + (row - 1) * pitch);
                [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(centre.x - dot * 0.5, centre.y - dot * 0.5, dot, dot)] fill];
            }
    }
    // 握把比 logo 窄 24pt，分隔线和按钮都跟着左移。
    const CGFloat leadingShift = showsLogo ? 0.0 : 24.0 * scale;
    // The divider takes the candidate outline, as the panel's does (THEME_CONTRACT §3), or a package's own toolbar divider colour.
    if (components != 0)
    {
        [PreviewColor(tokens.divider.value_or(tokens.border)) setFill];
        NSRectFillUsingOperation(NSMakeRect(41.0 * scale - leadingShift, buttonTop, 1.2 * scale, buttonHeight),
                                 NSCompositingOperationSourceOver);
    }
    NSDictionary *attributes = @{
        NSFontAttributeName : [NSFont systemFontOfSize:fontSize * scale * 0.95 weight:NSFontWeightRegular],
        NSForegroundColorAttributeName : PreviewColor(tokens.text),
    };
    NSImageSymbolConfiguration *symbols = [[NSImageSymbolConfiguration configurationWithPointSize:fontSize * scale
                                                                                          weight:NSFontWeightRegular]
        configurationByApplyingConfiguration:[NSImageSymbolConfiguration configurationWithPaletteColors:@[PreviewColor(tokens.text)]]];
    NSArray<NSArray<NSString *> *> *glyphs = ToolbarPreviewGlyphs();
    CGFloat x = 46.2 * scale - leadingShift;
    for (NSUInteger index = 0; index < glyphs.count; ++index)
    {
        if ((components & (1u << index)) == 0) continue;
        NSRect button = NSMakeRect(x, buttonTop, buttonWidth, buttonHeight);
        NSString *title = glyphs[index][0];
        NSString *symbol = glyphs[index][1];
        if (title.length > 0)
        {
            NSSize size = [title sizeWithAttributes:attributes];
            [title drawAtPoint:NSMakePoint(NSMidX(button) - size.width / 2.0, NSMinY(button) + (NSHeight(button) - size.height) / 2.0)
                withAttributes:attributes];
        }
        else
        {
            NSImage *image = [[NSImage imageWithSystemSymbolName:symbol accessibilityDescription:nil]
                imageWithSymbolConfiguration:symbols];
            if (image != nil)
                [image drawInRect:NSMakeRect(NSMidX(button) - image.size.width / 2.0,
                                             NSMinY(button) + (NSHeight(button) - image.size.height) / 2.0,
                                             image.size.width, image.size.height)
                         fromRect:NSZeroRect
                        operation:NSCompositingOperationSourceOver
                         fraction:1.0
                   respectFlipped:YES
                            hints:nil];
        }
        x += buttonWidth + 2.0 * scale;
    }
    [NSGraphicsContext restoreGraphicsState];
    return fit;
}

NSArray<NSString *> *PreviewSamples()
{
    return @[
        @"水杉(Ss)", @"输入法(Sw)", @"你好(Nh)", @"世界(Sj)", @"中国(Zg)", @"水仙(Sx)", @"水山(Ss)", @"水衫(Ss)",
        @"水善(Ss)"
    ];
}

// The desktop a preview draws its panel against: not a colour the skin owns, so that a skin surface close to the settings window's own background still reads as a window floating over something.
NSColor *PreviewCanvasFill(BOOL dark)
{
    return dark ? [NSColor colorWithSRGBRed:0.04 green:0.04 blue:0.05 alpha:1.0]
                : [NSColor colorWithSRGBRed:0.90 green:0.91 blue:0.92 alpha:1.0];
}

NSDictionary<NSAttributedStringKey, id> *PreviewCaptionAttributes()
{
    return @{
        NSFontAttributeName : [NSFont systemFontOfSize:11.0 weight:NSFontWeightSemibold],
        NSForegroundColorAttributeName : [NSColor secondaryLabelColor],
    };
}
} // namespace

@implementation MSIMECandidatePreviewView
{
    NSInteger _panelStyle;
    NSInteger _pageSize;
    CGFloat _candidateFontSize;
    NSString *_previewSkinId;
    NSString *_sampleText;
    NSArray<NSString *> *_sampleWords;
    NSNumber *_forcedDark;
    BOOL _showsLayoutShowcase;
    NSLayoutConstraint *_heightConstraint;
    msime::mac::ResolvedSkin _lightSkin;
    msime::mac::ResolvedSkin _darkSkin;
    // The showcase's 悬浮状态栏 draws the toolbar palette, not the candidate one, so a package's card radius does not reach it.
    msime::mac::SkinTokens _lightToolbar;
    msime::mac::SkinTokens _darkToolbar;
}

- (instancetype)initWithFrame:(NSRect)frameRect
{
    self = [super initWithFrame:frameRect];
    if (self != nil)
    {
        self.translatesAutoresizingMaskIntoConstraints = NO;
        self.accessibilityLabel = @"候选窗口预览";
        self.accessibilityRole = NSAccessibilityGroupRole;
        _sampleWords = PreviewSamples();
        _heightConstraint = [self.heightAnchor constraintEqualToConstant:190.0];
        _heightConstraint.active = YES;
        [self updatePanelStyle:0 pageSize:9 fontSize:18];
    }
    return self;
}

- (BOOL)isFlipped
{
    return YES;
}

- (void)setPreferences:(MSIMEAppearancePreferences *)preferences {
    _preferences = preferences;
    [self reloadPreview];
}
- (void)viewDidChangeEffectiveAppearance {
    [super viewDidChangeEffectiveAppearance];
    [self reloadPreview];
}
- (void)reloadPreview
{
    if (_preferences && _previewSkinId == nil) {
        _lightSkin = [_preferences resolvedSkinForDark:NO];
        _darkSkin = [_preferences resolvedSkinForDark:YES];
        _lightToolbar = [_preferences toolbarSkinForDark:NO];
        _darkToolbar = [_preferences toolbarSkinForDark:YES];
    } else {
        // A card previews one choice as it would look once chosen: a global theme id as that theme (the custom theme with what the user has made of it), and an external package id as the custom theme drawing that package over its manifest base.
        const std::filesystem::path root = _preferences.skinsRoot.fileSystemRepresentation ?: "";
        const std::string_view layout = _preferences.vertical ? "vertical" : "horizontal";
        const std::string id = _previewSkinId.UTF8String ?: "system";
        std::string theme = id;
        msime::mac::CustomTheme custom;
        // 外部包只在它 base 的明暗下画：固定明暗的包两种预览都按那一种明暗解析，否则卡片在另一种明暗下预览到的是没有包的底色。
        std::optional<bool> packageDark;
        if (id == "custom" && _preferences != nil) {
            custom = [_preferences customTheme];
        } else if (!msime::mac::IsGlobalThemeId(id)) {
            theme = "custom";
            // 两个槽位都放这个包，深色模式不会回落到别的包。
            custom.candidateSkin = id;
            custom.candidateSkinDark = id;
            if (const auto package = msime::mac::LoadSkinPackage(root, id)) {
                custom.base = package->base;
                const msime::mac::SkinSlot slot = msime::mac::SkinSlotOfBase(package->base);
                if (slot == msime::mac::SkinSlot::light || slot == msime::mac::SkinSlot::dark)
                    packageDark = slot == msime::mac::SkinSlot::dark;
            }
        }
        _lightSkin = msime::mac::ResolveSkin(theme, custom, packageDark.value_or(false), layout, root);
        _darkSkin = msime::mac::ResolveSkin(theme, custom, packageDark.value_or(true), layout, root);
        _lightToolbar = msime::mac::ToolbarSkinTokens(_lightSkin, root);
        _darkToolbar = msime::mac::ToolbarSkinTokens(_darkSkin, root);
    }
    self.themeButton.title = [self forcedThemeButtonTitle];
    // A theme with a fixed mode looks the same in both, so there is nothing to preview in the other.
    self.themeButton.hidden = [self previewHasFixedMode];
    _heightConstraint.constant = [self previewContentHeight];
    self.needsDisplay = YES;
}

- (void)setPreviewSkinId:(NSString *)skinId
{
    _previewSkinId = [skinId copy];
    [self reloadPreview];
}

- (NSString *)previewSkinId
{
    return _previewSkinId;
}

- (void)setSampleText:(NSString *)sampleText
{
    _sampleText = [sampleText copy];
    // Debounced because a text field the user is typing into sends this on every keystroke, and applying one costs a resolved font per word, a remeasured row height, and a redraw of the whole canvas — three panels of it in showcase mode.
    [NSObject cancelPreviousPerformRequestsWithTarget:self selector:@selector(applySampleText) object:nil];
    [self performSelector:@selector(applySampleText) withObject:nil afterDelay:0.2];
}

- (NSString *)sampleText
{
    return _sampleText;
}

- (void)applySampleText
{
    _sampleWords = PreviewSampleWords(_sampleText);
    [self reloadPreview];
}

- (NSArray<NSString *> *)previewWords
{
    return _sampleWords.count > 0 ? _sampleWords : PreviewSamples();
}

/// The candidate size to draw at, which is the setting unless nothing has been handed over yet. Both the measuring and the drawing ask here, so the height the preview reserves cannot disagree with what goes into it.
- (CGFloat)previewFontSize
{
    return _candidateFontSize > 0 ? _candidateFontSize : 18.0;
}

/// The preedit size to draw at, or 0 where 候选窗预编辑 is set to hide it. The showcase reads the same pair of settings as the single preview; it used to derive a size of its own from the candidate font, so the two ways of looking at one panel disagreed about a row that either is there or is not.
- (CGFloat)previewPreeditFontSize
{
    return self.preferences.showsCandidatePreedit ? self.preferences.preeditFontSize : 0.0;
}

/// 整体大小 as a factor, 100% until there are preferences to read it from.
- (CGFloat)previewScale
{
    return self.preferences != nil ? self.preferences.candidateScalePercent / 100.0 : 1.0;
}

/// The skin as the candidate window draws it, with the user's radius and opacity over it. The scale is left out of it here because the preview draws the whole panel magnified instead, the one place a uniform transform is the same thing as multiplying every length.
- (msime::mac::ResolvedSkin)previewWindowSkin
{
    msime::mac::CandidateWindowStyle style = self.preferences != nil ? [self.preferences candidateWindowStyle] : msime::mac::CandidateWindowStyle{};
    style.scale = 1.0;
    return msime::mac::StyledCandidateSkin([self previewSkin], style);
}

- (void)setShowsLayoutShowcase:(BOOL)showsLayoutShowcase
{
    _showsLayoutShowcase = showsLayoutShowcase;
    [self reloadPreview];
}

- (BOOL)previewHasFixedMode
{
    return msime::mac::FixedThemeMode(_lightSkin, _darkSkin).has_value();
}

- (BOOL)previewUsesDark
{
    // A theme that fixes its mode is drawn in it whatever the system or the preview toggle says.
    if (const auto fixed = msime::mac::FixedThemeMode(_lightSkin, _darkSkin))
    {
        return *fixed;
    }
    if (_forcedDark != nil)
    {
        return _forcedDark.boolValue;
    }
    NSString *match = [self.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    return [match isEqual:NSAppearanceNameDarkAqua];
}

- (msime::mac::ResolvedSkin)previewSkin
{
    return [self previewUsesDark] ? _darkSkin : _lightSkin;
}

- (NSColor *)previewCanvasFillColor
{
    return PreviewCanvasFill([self previewUsesDark]);
}

- (NSColor *)previewPanelFillColor
{
    return PreviewColor([self previewSkin].tokens.surface);
}

- (NSColor *)previewTextColor
{
    return PreviewColor([self previewSkin].tokens.text);
}

- (NSColor *)previewAccentColor
{
    return PreviewColor([self previewSkin].tokens.accent);
}

- (CGFloat)previewContentHeight
{
    const msime::mac::ResolvedSkin skin = [self previewSkin];
    if (_showsLayoutShowcase)
    {
        return MakeShowcaseMetrics(_pageSize, [self previewFontSize], skin.decorationTopDip,
                                   [self previewPreeditFontSize], [self previewWords], self.preferences, [self previewScale])
            .totalHeight;
    }
    return MakeAppearanceMetrics(_panelStyle, _pageSize, [self previewFontSize], skin.decorationTopDip,
                                 [self previewPreeditFontSize], [self previewWords], self.preferences, [self previewScale])
        .totalHeight;
}

- (void)toggleForcedTheme
{
    _forcedDark = @(![self previewUsesDark]);
    [self reloadPreview];
}

- (NSString *)forcedThemeButtonTitle
{
    return [self previewUsesDark] ? @"预览浅色" : @"预览深色";
}

- (void)updatePanelStyle:(NSInteger)panelStyle pageSize:(NSInteger)pageSize fontSize:(NSInteger)fontSize
{
    _panelStyle = panelStyle;
    _pageSize = pageSize;
    _candidateFontSize = fontSize;
    NSString *layout = panelStyle == 1 ? @"纵向列表" : @"横向排列";
    self.accessibilityValue = [NSString
        stringWithFormat:@"%@，%ld 个候选，%ld pt", layout, static_cast<long>(pageSize), static_cast<long>(fontSize)];
    self.accessibilityHelp = @"预览会随候选排列、每页候选、候选字号与预览示例文字实时变化";
    [self reloadPreview];
}

- (void)drawRect:(NSRect)dirtyRect
{
    [super drawRect:dirtyRect];
    NSAppearance *drawingAppearance = [NSAppearance appearanceNamed:[self previewUsesDark] ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
    [drawingAppearance performAsCurrentDrawingAppearance:^{ [self drawPreviewContents:dirtyRect]; }];
}

- (void)drawPreviewContents:(NSRect)dirtyRect
{
    (void)dirtyRect;
    NSRect canvas = NSInsetRect(self.bounds, 1.0, 1.0);
    NSBezierPath *canvasPath = [NSBezierPath bezierPathWithRoundedRect:canvas xRadius:12.0 yRadius:12.0];
    [[self previewCanvasFillColor] setFill];
    [canvasPath fill];
    [[NSColor separatorColor] setStroke];
    canvasPath.lineWidth = 1.0;
    [canvasPath stroke];
    [NSGraphicsContext saveGraphicsState];
    [canvasPath addClip];

    NSDictionary<NSAttributedStringKey, id> *captionAttributes = PreviewCaptionAttributes();
    const msime::mac::ResolvedSkin skin = [self previewWindowSkin];
    const CGFloat scale = [self previewScale];
    NSArray<NSString *> *samples = [self previewWords];
    const CGFloat preeditFontSize = [self previewPreeditFontSize];
    const NSInteger count = MAX(_pageSize, (NSInteger)1);
    if (_showsLayoutShowcase)
    {
        const SkinPreviewMetrics metrics = MakeShowcaseMetrics(_pageSize, [self previewFontSize], skin.decorationTopDip,
                                                              preeditFontSize, samples, self.preferences, scale);
        NSArray<NSString *> *horizontal = PreviewPageWords(samples, count);
        NSArray<NSString *> *vertical = PreviewPageWords(samples, PreviewVisibleRows(count));
        CGFloat y = metrics.top;
        [@"横排候选" drawAtPoint:NSMakePoint(14.0, y) withAttributes:captionAttributes];
        y += metrics.captionHeight + metrics.captionGap;
        DrawScaledPreviewCandidates(NSMakeRect(14.0, y, NSWidth(self.bounds) - 28.0, metrics.horizontalHeight), scale, skin, NO,
                                    horizontal, metrics.fontSize, metrics.rowHeight, nil, preeditFontSize, self.preferences);
        y += metrics.horizontalHeight + metrics.sectionGap;
        [@"竖排候选" drawAtPoint:NSMakePoint(14.0, y) withAttributes:captionAttributes];
        y += metrics.captionHeight + metrics.captionGap;
        DrawScaledPreviewCandidates(NSMakeRect(14.0, y, NSWidth(self.bounds) - 28.0, metrics.verticalHeight), scale, skin, YES,
                                    vertical, metrics.fontSize, metrics.rowHeight, PreviewPendingFooter(count),
                                    preeditFontSize, self.preferences);
        y += metrics.verticalHeight + metrics.sectionGap;
        [@"悬浮状态栏" drawAtPoint:NSMakePoint(14.0, y) withAttributes:captionAttributes];
        y += metrics.captionHeight + metrics.captionGap;
        const ToolbarPreviewInputs toolbar = ToolbarInputs(self.preferences);
        DrawPreviewToolbar(NSMakeRect(14.0, y, NSWidth(self.bounds) - 28.0, metrics.toolbarHeight),
                           [self previewUsesDark] ? _darkToolbar : _lightToolbar,
                           toolbar.components, toolbar.scalePercent, toolbar.fontSize, toolbar.logo);
        [NSGraphicsContext restoreGraphicsState];
        return;
    }

    const SkinPreviewMetrics metrics =
        MakeAppearanceMetrics(_panelStyle, _pageSize, [self previewFontSize], skin.decorationTopDip, preeditFontSize,
                              samples, self.preferences, scale);
    const BOOL vertical = _panelStyle == 1;
    NSArray<NSString *> *words = PreviewPageWords(samples, vertical ? PreviewVisibleRows(count) : count);
    NSString *footer = vertical ? PreviewPendingFooter(count) : nil;
    CGFloat y = metrics.top;
    [@"输入效果" drawAtPoint:NSMakePoint(14.0, y) withAttributes:captionAttributes];
    NSString *pageSummary = [NSString stringWithFormat:@"每页 %ld 个", static_cast<long>(_pageSize)];
    NSSize pageSummarySize = [pageSummary sizeWithAttributes:captionAttributes];
    [pageSummary drawAtPoint:NSMakePoint(NSMaxX(canvas) - pageSummarySize.width - 14.0, y)
              withAttributes:captionAttributes];
    y += metrics.captionHeight + metrics.captionGap;
    DrawScaledPreviewCandidates(NSMakeRect(14.0, y, NSWidth(self.bounds) - 28.0, metrics.panelHeight), scale, skin, vertical,
                                words, metrics.fontSize, metrics.rowHeight, footer, preeditFontSize, self.preferences);
    [NSGraphicsContext restoreGraphicsState];
}

@end

@implementation MSIMEToolbarPreviewView
{
    NSLayoutConstraint *_heightConstraint;
}

- (instancetype)initWithFrame:(NSRect)frameRect
{
    self = [super initWithFrame:frameRect];
    if (self != nil)
    {
        self.translatesAutoresizingMaskIntoConstraints = NO;
        self.accessibilityLabel = @"悬浮工具栏预览";
        self.accessibilityRole = NSAccessibilityGroupRole;
        _heightConstraint = [self.heightAnchor constraintEqualToConstant:[self previewContentHeight]];
        _heightConstraint.active = YES;
        [self reloadPreview];
    }
    return self;
}

- (BOOL)isFlipped
{
    return YES;
}

- (void)setPreferences:(MSIMEAppearancePreferences *)preferences
{
    _preferences = preferences;
    [self reloadPreview];
}

- (void)viewDidChangeEffectiveAppearance
{
    [super viewDidChangeEffectiveAppearance];
    [self reloadPreview];
}

/// Dark where the toolbar itself would be dark: a theme with a fixed mode decides, then 悬浮工具栏主题, 颜色模式 decides where that is 跟随颜色模式, and where neither names an appearance the panel follows the system — as this view does, being in a window that follows the system too. It is MetasequoiaFloatingToolbarPanel -applyThemePreferences: read back.
- (BOOL)previewUsesDark
{
    // A theme with a mode of its own draws the toolbar in that mode, as InputController tells the panel.
    if (self.preferences != nil)
        if (const auto fixed = [self.preferences fixedThemeMode]) return *fixed;
    NSString *surface = self.preferences.toolbarTheme;
    NSString *resolved = [surface isEqual:@"dark"] || [surface isEqual:@"light"] ? surface : self.preferences.themeMode;
    if ([resolved isEqual:@"dark"]) return YES;
    if ([resolved isEqual:@"light"]) return NO;
    NSString *match = [self.effectiveAppearance
        bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    return [match isEqual:NSAppearanceNameDarkAqua];
}

- (CGFloat)previewContentHeight
{
    const ToolbarPreviewInputs toolbar = ToolbarInputs(self.preferences);
    return 14.0 + 16.0 + 4.0 +
           ToolbarPreviewSize(toolbar.components, toolbar.scalePercent, toolbar.fontSize, toolbar.logo).height + 14.0;
}

- (void)reloadPreview
{
    const ToolbarPreviewInputs toolbar = ToolbarInputs(self.preferences);
    const NSSize size = ToolbarPreviewSize(toolbar.components, toolbar.scalePercent, toolbar.fontSize, toolbar.logo);
    NSUInteger count = 0;
    for (NSUInteger index = 0; index < 10; ++index) count += (toolbar.components & (1u << index)) != 0 ? 1 : 0;
    self.accessibilityValue = [NSString stringWithFormat:@"%lu 个按钮，%ld × %ld pt", (unsigned long)count,
                                                         static_cast<long>(size.width), static_cast<long>(size.height)];
    self.accessibilityHelp = @"预览会随工具栏按钮、工具栏缩放和工具栏字号实时变化";
    _heightConstraint.constant = [self previewContentHeight];
    self.needsDisplay = YES;
}

- (void)drawRect:(NSRect)dirtyRect
{
    [super drawRect:dirtyRect];
    NSAppearance *drawingAppearance =
        [NSAppearance appearanceNamed:[self previewUsesDark] ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
    [drawingAppearance performAsCurrentDrawingAppearance:^{ [self drawPreviewContents]; }];
}

- (void)drawPreviewContents
{
    NSRect canvas = NSInsetRect(self.bounds, 1.0, 1.0);
    NSBezierPath *canvasPath = [NSBezierPath bezierPathWithRoundedRect:canvas xRadius:12.0 yRadius:12.0];
    const BOOL dark = [self previewUsesDark];
    [PreviewCanvasFill(dark) setFill];
    [canvasPath fill];
    [[NSColor separatorColor] setStroke];
    canvasPath.lineWidth = 1.0;
    [canvasPath stroke];
    [NSGraphicsContext saveGraphicsState];
    [canvasPath addClip];
    const ToolbarPreviewInputs toolbar = ToolbarInputs(self.preferences);
    const NSSize size = ToolbarPreviewSize(toolbar.components, toolbar.scalePercent, toolbar.fontSize, toolbar.logo);
    const msime::mac::SkinTokens tokens = self.preferences != nil ? [self.preferences toolbarSkinForDark:dark]
                                                                  : msime::mac::NativeCandidateTokens(dark);
    const CGFloat top = 14.0 + 16.0 + 4.0;
    const CGFloat fit = DrawPreviewToolbar(NSMakeRect(14.0, top, NSWidth(self.bounds) - 28.0, size.height), tokens,
                                           toolbar.components, toolbar.scalePercent, toolbar.fontSize, toolbar.logo);
    // The numbers, because they are the answer to the question this preview exists for: four scale steps and seven font sizes are 28 sizes, and several of the pairs differ by a point or two. A preview that is drawn down to fit the column says so rather than letting the user read the shrunken row as the size they picked.
    NSMutableString *caption = [NSMutableString
        stringWithFormat:@"实际尺寸 %ld × %ld pt", static_cast<long>(size.width), static_cast<long>(size.height)];
    if (fit < 0.999) [caption appendFormat:@"，预览按 %ld%% 缩小显示", static_cast<long>(round(fit * 100.0))];
    if (self.preferences != nil && !self.preferences.floatingToolbarEnabled) [caption appendString:@"；当前未显示"];
    [caption drawAtPoint:NSMakePoint(14.0, 14.0) withAttributes:PreviewCaptionAttributes()];
    [NSGraphicsContext restoreGraphicsState];
}

@end
