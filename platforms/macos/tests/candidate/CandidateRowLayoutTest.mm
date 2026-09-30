#import "../settings/TestPreferenceSuite.h"
// The candidate page layout as the panel applies it. CandidateItemLayoutTest covers the arithmetic; this renders real pages through the controller, because what matters is the frame a candidate button ends up with and the runs it draws: the card is at most half the screen's visible width, text, 辅助码 and glosses wider than their column wrap inside it, rows take their own heights, and a horizontal page breaks onto a new line instead of squeezing its candidates.
//
// It stands apart from shortcut-test, which imports the same controller, so a failure elsewhere in that suite cannot hide the layout assertions.
#import "../../src/input/InputController.mm"

#include <cassert>

@interface RowLayoutClient : NSObject <MSIMETextClient>
@property(nonatomic) NSRect caret;
@end
@implementation RowLayoutClient
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect
{
    (void)index;
    *rect = self.caret;
    return @{};
}
- (void)insertText:(id)text replacementRange:(NSRange)range
{
    (void)text;
    (void)range;
}
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement
{
    (void)text;
    (void)selection;
    (void)replacement;
}
@end

// Keep the panel off the screen while it still lays its content out.
@interface RowLayoutPanel : MSIMECandidatePanel
@end
@implementation RowLayoutPanel
- (void)orderFrontRegardless
{
}
@end

static MSIMECandidateButton *CandidateButton(NSView *content, NSInteger tag)
{
    for (NSView *view in content.subviews)
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag == tag) return (MSIMECandidateButton *)view;
    return nil;
}

int main(void)
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        NSString *suite = [@"app.msime.test.rowfit." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        appearance.vertical = NO;
        MSIMEInputController *controller = [[MSIMEInputController alloc] init];
        RowLayoutClient *client = [RowLayoutClient new];
        NSRect screen = NSScreen.mainScreen.visibleFrame;
        client.caret = NSMakeRect(NSMidX(screen), NSMidY(screen), 1, 20);
        RowLayoutPanel *panel = [[RowLayoutPanel alloc] initWithContentRect:NSZeroRect
                                                                  styleMask:NSWindowStyleMaskBorderless |
                                                                            NSWindowStyleMaskNonactivatingPanel
                                                                    backing:NSBackingStoreBuffered
                                                                      defer:NO];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:panel forKey:@"panel"];

        NSDictionary *singleView = @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @1,
                                     @"candidates": @[@{@"text": @"测试", @"highlighted": @YES}]};
        [controller setValue:singleView forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *measured = CandidateButton(panel.contentView, 0);
        assert(measured);
        NSFont *rowFont = measured.font;
        const CGFloat glyphWidth = [@"水" sizeWithAttributes:@{NSFontAttributeName: rowFont}].width;
        assert(glyphWidth > 0);

        const CGFloat halfScreen = MAX(80, floor(screen.size.width * 0.5));

        // A one-character page still gets a card at least seven times the candidate font wide, as the Windows card and the source skins' `min-width: 7em` do.
        NSMutableDictionary *oneCharacterView = [singleView mutableCopy];
        oneCharacterView[@"candidates"] = @[@{@"text": @"的", @"highlighted": @YES}];
        [controller setValue:[oneCharacterView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(panel.contentView.frame.size.width >= MIN(halfScreen, 7 * rowFont.pointSize) - 0.5);
        appearance.vertical = YES;
        [controller renderCandidates];
        assert(panel.contentView.frame.size.width >= MIN(halfScreen, 7 * rowFont.pointSize) - 0.5);
        // A vertical row stretches across the whole card.
        MSIMECandidateButton *oneCharacter = CandidateButton(panel.contentView, 0);
        assert(oneCharacter && fabs(NSWidth(oneCharacter.frame) - (panel.contentView.frame.size.width - 2 * NSMinX(oneCharacter.frame))) <= 1.0);
        appearance.vertical = NO;
        NSString *(^glyphs)(CGFloat) = ^NSString *(CGFloat points) {
            return [@"" stringByPaddingToLength:(NSUInteger)MAX(2.0, floor(points / glyphWidth))
                                     withString:@"水杉输入法" startingAtIndex:0];
        };

        // A sentence worth four fifths of half the screen, then eight candidates that together take far more than a whole screen line: the card grows past half the screen towards the screen less its margins, and the page still breaks onto new lines.
        NSString *sentence = glyphs(halfScreen * 0.8 - 60);
        NSString *shortCandidate = glyphs(halfScreen * 0.24);
        NSMutableArray *page = [NSMutableArray arrayWithObject:@{@"text": sentence, @"highlighted": @YES}];
        while (page.count < 9) [page addObject:@{@"text": shortCandidate}];
        NSMutableDictionary *pageView = [singleView mutableCopy];
        pageView[@"candidates"] = [page copy];
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller renderCandidates];

        MSIMECandidateButton *sentenceButton = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *tailButton = CandidateButton(panel.contentView, 8);
        assert(sentenceButton && tailButton);
        const CGFloat sentenceWidth = ceil([sentence sizeWithAttributes:@{NSFontAttributeName: rowFont}].width);
        // The head keeps its full width and every candidate keeps its natural width; the ones that do not fit start new lines below.
        assert(sentenceButton.frame.size.width >= sentenceWidth);
        assert(!sentenceButton.itemLayout.textWrapped);
        assert(NSMaxY(tailButton.frame) <= NSMinY(sentenceButton.frame) + 0.5);
        assert(!tailButton.itemLayout.textWrapped);
        assert(fabs(tailButton.frame.size.width - CandidateButton(panel.contentView, 1).frame.size.width) <= 1.0);
        for (NSInteger tag = 0; tag < 9; ++tag) {
            NSRect frame = CandidateButton(panel.contentView, tag).frame;
            assert(NSMinX(frame) >= 0 && NSMaxX(frame) <= panel.frame.size.width + 0.5 && NSMinY(frame) >= 0);
        }
        assert(panel.frame.size.width <= MAX(halfScreen, floor(screen.size.width - 2 * MSIMECandidateScreenMargin)) + 0.5);

        // A page that fits keeps every candidate at its natural width on one line.
        NSMutableDictionary *narrowView = [singleView mutableCopy];
        narrowView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES}, @{@"text": @"测试测试"}];
        [controller setValue:[narrowView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *shorter = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *longer = CandidateButton(panel.contentView, 1);
        assert(shorter && longer);
        assert(longer.frame.size.width > shorter.frame.size.width);
        assert(longer.frame.size.width - shorter.frame.size.width >= 2 * glyphWidth - 1.0);
        assert(shorter.frame.origin.y == longer.frame.origin.y && NSMaxX(shorter.frame) == NSMinX(longer.frame));
        const CGFloat oneLine = shorter.frame.size.height;

        // One candidate wider than a whole line is narrowed to the line and wraps inside it rather than being cut off.
        NSString *paragraph = glyphs(screen.size.width * 3);
        NSMutableDictionary *wideView = [singleView mutableCopy];
        wideView[@"candidates"] = @[@{@"text": paragraph, @"highlighted": @YES}, @{@"text": @"测试"}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *wrapped = CandidateButton(panel.contentView, 0);
        assert(wrapped.itemLayout.textWrapped && wrapped.frame.size.height > oneLine * 1.5);
        assert(panel.frame.size.width <= MAX(halfScreen, floor(screen.size.width - 2 * MSIMECandidateScreenMargin)) + 0.5 && NSMaxX(wrapped.frame) <= panel.frame.size.width + 0.5);
        assert(NSMaxY(CandidateButton(panel.contentView, 1).frame) <= NSMinY(wrapped.frame) + 0.5);

        // Vertical: the card is capped at half the screen, a long sentence wraps into a taller row than its neighbours, and rows stack at their own heights.
        appearance.vertical = YES;
        wideView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES}, @{@"text": paragraph}, @{@"text": @"测试测试"}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *first = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *tall = CandidateButton(panel.contentView, 1);
        MSIMECandidateButton *last = CandidateButton(panel.contentView, 2);
        assert(first && tall && last);
        assert(panel.frame.size.width <= halfScreen + 0.5);
        assert(tall.itemLayout.textWrapped && tall.frame.size.height > first.frame.size.height * 2);
        assert(fabs(first.frame.size.height - last.frame.size.height) < 0.5);
        assert(fabs(NSMinY(first.frame) - NSMaxY(tall.frame)) < 0.5 && fabs(NSMinY(tall.frame) - NSMaxY(last.frame)) < 0.5);
        assert(first.frame.size.width == tall.frame.size.width && tall.frame.size.width == last.frame.size.width);

        // The 辅助码 is a run of its own: the button's text is the candidate alone, the annotation is drawn after it, and the tooltip and accessibility label keep the combined form.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"annotation": @"(aB)", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *annotated = CandidateButton(panel.contentView, 0);
        assert(![annotated.title containsString:@"(aB)"] && [annotated.title containsString:@"汉语"]);
        assert([annotated.annotation isEqual:@"(aB)"] && !annotated.itemLayout.annotation.below);
        assert([annotated.toolTip isEqual:@"汉语(aB)"] && [annotated.accessibilityLabel containsString:@"汉语(aB)"]);
        // A text that fills the column pushes the annotation under it, where it takes a line of its own.
        wideView[@"candidates"] = @[@{@"text": paragraph, @"annotation": @"(aB)", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        annotated = CandidateButton(panel.contentView, 0);
        assert(annotated.itemLayout.annotation.below && annotated.itemLayout.annotation.y >= annotated.itemLayout.textHeight);
        assert(annotated.frame.size.height >= annotated.itemLayout.height - 0.5);

        // A vertical gloss stays on the candidate's line when it fits and wraps under it when it does not.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": @"Chinese", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(!CandidateButton(panel.contentView, 0).itemLayout.translation.below);
        NSString *longGloss = [@"" stringByPaddingToLength:(NSUInteger)(screen.size.width / 4) withString:@"gloss " startingAtIndex:0];
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": longGloss, @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *glossed = CandidateButton(panel.contentView, 0);
        assert(glossed.itemLayout.translation.below && glossed.translationBelow);
        assert(glossed.itemLayout.translation.height > glossed.itemLayout.textHeight);
        assert(panel.frame.size.width <= halfScreen + 0.5);
        NSBitmapImageRep *bitmap = [glossed bitmapImageRepForCachingDisplayInRect:glossed.bounds];
        assert(bitmap);
        [glossed cacheDisplayInRect:glossed.bounds toBitmapImageRep:bitmap];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return 0;
}
