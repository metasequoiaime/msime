#import "../settings/TestPreferenceSuite.h"
// The candidate page layout as the panel applies it. CandidateItemLayoutTest covers the arithmetic; this renders real pages through the controller, because what matters is the frame a candidate button ends up with and the runs it draws: the card is at most half the screen's visible width (a horizontal page may grow past it to keep its glosses on one line), text, 辅助码 and glosses wider than their column wrap inside it, rows take their own heights, and a horizontal page breaks onto a new line instead of squeezing its candidates.
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

        // A horizontal page whose glosses are wider than half the screen grows past it instead of wrapping them, as long as the screen has room: every gloss keeps the single line a short one gets.
        wideView[@"candidates"] = @[@{@"text": @"汉语", @"translation": @"Chinese", @"highlighted": @YES}];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *shortGloss = CandidateButton(panel.contentView, 0);
        const CGFloat glossLine = shortGloss.itemLayout.translation.height;
        assert(glossLine > 0 && shortGloss.translationFont);
        // Six glosses of a quarter of half the screen each: half again as wide as a half-screen card, still well inside the screen.
        const CGFloat glossUnit = [@"gloss " sizeWithAttributes:@{NSFontAttributeName: shortGloss.translationFont}].width;
        NSString *sentenceGloss = [@"" stringByPaddingToLength:(NSUInteger)MAX(8.0, ceil(halfScreen / 4 / glossUnit * 6)) withString:@"gloss " startingAtIndex:0];
        NSMutableArray *glossPage = [NSMutableArray array];
        while (glossPage.count < 6) [glossPage addObject:@{@"text": @"测试", @"translation": sentenceGloss}];
        glossPage[0] = @{@"text": @"测试", @"translation": sentenceGloss, @"highlighted": @YES};
        wideView[@"candidates"] = [glossPage copy];
        [controller setValue:[wideView copy] forKey:@"view"];
        [controller renderCandidates];
        CGFloat glossTotal = 0;
        for (NSInteger tag = 0; tag < 6; ++tag) {
            MSIMECandidateButton *button = CandidateButton(panel.contentView, tag);
            assert(button && fabs(button.itemLayout.translation.height - glossLine) < 0.5);
            assert(fabs(NSMinY(button.frame) - NSMinY(CandidateButton(panel.contentView, 0).frame)) < 0.5);
            glossTotal += button.itemLayout.translation.width;
        }
        assert(glossTotal > halfScreen && panel.frame.size.width > halfScreen + 0.5);
        assert(panel.frame.size.width <= screen.size.width + 0.5);

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

        // A Korean Hanja row shows only the Hanja on its line. Its 훈음 is drawn on the gloss line under it whatever the translation switches say, never as `translation`, which is what the gloss chords commit; the tooltip and accessibility label keep it apart from the Hanja.
        appearance.vertical = NO;
        appearance.candidateTranslations = NO;
        appearance.candidateEnglishGloss = NO;
        NSString *reading = @"나라 이름 한, 한나라 한";
        NSDictionary *hanjaBase = @{@"focused": @YES, @"scheme": @(msime::mac::KoreanScheme), @"local_mode": @"none", @"editing_text": @"한",
                                    @"candidate_list_open": @YES, @"page": @0, @"page_count": @1};
        NSMutableDictionary *hanjaView = [hanjaBase mutableCopy];
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"highlighted": @YES}, @{@"text": @"漢", @"annotation": @"한수 한"}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *hanja = CandidateButton(panel.contentView, 0);
        assert(hanja && [hanja.title isEqual:@"1  韓"] && hanja.annotation.length == 0 && hanja.itemLayout.annotation.width == 0);
        assert([hanja.glossReading isEqual:reading] && hanja.translation.length == 0);
        assert(hanja.itemLayout.translation.width > 0 && hanja.itemLayout.translation.below && hanja.translationBelow);
        assert(hanja.itemLayout.translation.y >= hanja.itemLayout.textHeight);
        assert([hanja.toolTip isEqual:[@"韓\n" stringByAppendingString:reading]]);
        assert([hanja.accessibilityLabel isEqual:[@"1  韓 " stringByAppendingString:reading]]);
        // The Hanja's own column is no wider than the reading line under it needs: an inline 훈음 at the candidate size made it several times wider.
        const CGFloat readingWidth = ceil([reading sizeWithAttributes:@{NSFontAttributeName: hanja.translationFont}].width);
        const CGFloat inlineWidth = ceil([reading sizeWithAttributes:@{NSFontAttributeName: rowFont}].width);
        assert(hanja.itemLayout.translation.width <= readingWidth + 0.5 && readingWidth < inlineWidth);
        bitmap = [hanja bitmapImageRepForCachingDisplayInRect:hanja.bounds];
        [hanja cacheDisplayInRect:hanja.bounds toBitmapImageRep:bitmap];

        // The 훈음 line is reserved with both translation switches off, so a Hanja with no 훈음 is as tall as one with it and the card does not jump; a Chinese page reserves nothing then.
        const CGFloat hanjaHeight = NSHeight(hanja.frame);
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *bare = CandidateButton(panel.contentView, 0);
        assert(bare.glossReading.length == 0 && fabs(NSHeight(bare.frame) - hanjaHeight) < 0.5);
        [controller setValue:@{@"focused": @YES, @"editing_text": @"han", @"page": @0, @"page_count": @1,
                               @"candidates": @[@{@"text": @"韩", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        assert(NSHeight(CandidateButton(panel.contentView, 0).frame) + glossLine * 0.5 < hanjaHeight);

        // A real translation goes on the line after the 훈음, and only it is the row's `translation`.
        appearance.candidateTranslations = YES;
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"translation": @"Korea", @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *translated = CandidateButton(panel.contentView, 0);
        assert([translated.translation isEqual:@"Korea"] && [translated.glossReading isEqual:reading]);
        assert(translated.itemLayout.translation.height > glossLine * 1.5);
        NSString *translatedTip = [NSString stringWithFormat:@"韓\n%@\nKorea", reading];
        assert([translated.toolTip isEqual:translatedTip]);
        // An armed translation column underlines the translation, not the reading above it.
        translated.armedGlossColumn = 1;
        bitmap = [translated bitmapImageRepForCachingDisplayInRect:translated.bounds];
        [translated cacheDisplayInRect:translated.bounds toBitmapImageRep:bitmap];

        // Vertical follows the vertical gloss rule: the 훈음 stays beside the Hanja when it fits, and nothing is reserved.
        appearance.vertical = YES;
        appearance.candidateTranslations = NO;
        hanjaView[@"candidates"] = @[@{@"text": @"韓", @"annotation": reading, @"highlighted": @YES}];
        [controller setValue:[hanjaView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *verticalHanja = CandidateButton(panel.contentView, 0);
        assert(verticalHanja.itemLayout.translation.width > 0 && !verticalHanja.itemLayout.translation.below && !verticalHanja.translationBelow);
        assert(verticalHanja.annotation.length == 0 && [verticalHanja.glossReading isEqual:reading]);

        // 脚本插件的注释收拾成一行再画：换行、U+2028、双向控制符和零宽字符连成的一段变成一个空格，首尾空白去掉，不是字符串时为空，超过 32 个 UTF-16 码元时在完整字符处截断补「…」，不拆开代理对。
        assert([MSIMECandidateScriptAnnotationRun(@"甲\n乙\u2028丙\u202E丁\u200B戊\u2029") isEqual:@"甲 乙 丙 丁 戊"]);
        assert([MSIMECandidateScriptAnnotationRun(@"\r\n 常用 \t") isEqual:@"常用"]);
        assert(MSIMECandidateScriptAnnotationRun(@42).length == 0 && MSIMECandidateScriptAnnotationRun(nil).length == 0);
        assert(MSIMECandidateScriptAnnotationRun(@"\u202E \u200B").length == 0);
        NSString *thirtyTwo = [@"" stringByPaddingToLength:32 withString:@"水" startingAtIndex:0];
        assert([MSIMECandidateScriptAnnotationRun(thirtyTwo) isEqual:thirtyTwo]);
        NSString *cutCJK = MSIMECandidateScriptAnnotationRun([thirtyTwo stringByAppendingString:@"杉杉"]);
        assert(cutCJK.length == 32 && [cutCJK hasSuffix:@"…"] && [cutCJK hasPrefix:[thirtyTwo substringToIndex:31]]);
        NSString *emoji = [@"" stringByPaddingToLength:40 withString:@"😀" startingAtIndex:0];
        NSString *cutEmoji = MSIMECandidateScriptAnnotationRun(emoji);
        assert(cutEmoji.length == 31 && [cutEmoji isEqual:[[emoji substringToIndex:30] stringByAppendingString:@"…"]]);

        // 横排：插件注释是释义行的最后一行，排在译文之后；它不进引擎辅助码那一段，也不是按列上屏的 `translation`。悬停提示带着它，读屏标签说明它是插件注释；没有注释的行什么都不多。
        appearance.vertical = NO;
        appearance.candidateTranslations = YES;
        NSDictionary *scriptBase = @{@"focused": @YES, @"editing_text": @"hanyu", @"page": @0, @"page_count": @1};
        NSMutableDictionary *scriptView = [scriptBase mutableCopy];
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"annotation": @"(aB)", @"translation": @"Chinese", @"script_annotation": @"常用词", @"highlighted": @YES},
                                      @{@"text": @"韩语", @"annotation": @"(aB)", @"translation": @"Korean"}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *scripted = CandidateButton(panel.contentView, 0);
        MSIMECandidateButton *unscripted = CandidateButton(panel.contentView, 1);
        assert(scripted && unscripted);
        assert([scripted.title isEqual:@"1  汉语"] && [scripted.annotation isEqual:@"(aB)"] && [scripted.translation isEqual:@"Chinese"]);
        assert([scripted.scriptAnnotation isEqual:@"常用词"] && unscripted.scriptAnnotation.length == 0);
        assert(scripted.itemLayout.translation.below && scripted.translationBelow);
        assert(scripted.itemLayout.translation.height > unscripted.itemLayout.translation.height + glossLine * 0.5);
        assert([scripted.toolTip isEqual:@"汉语(aB)\nChinese\n常用词"] && [unscripted.toolTip isEqual:@"韩语(aB)\nKorean"]);
        assert([scripted.accessibilityLabel isEqual:@"1  汉语(aB)，插件注释：常用词"]);
        assert([unscripted.accessibilityLabel isEqual:@"2  韩语(aB)"]);
        bitmap = [scripted bitmapImageRepForCachingDisplayInRect:scripted.bounds];
        [scripted cacheDisplayInRect:scripted.bounds toBitmapImageRep:bitmap];

        // 没有译文时插件注释自己占释义行；插件写进去的换行和零宽字符画不出第二行。
        appearance.candidateTranslations = NO;
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": @"第一行\n\u200B第二行", @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *alone = CandidateButton(panel.contentView, 0);
        assert([alone.scriptAnnotation isEqual:@"第一行 第二行"] && alone.translation.length == 0);
        assert(alone.itemLayout.translation.width > 0 && alone.itemLayout.translation.below);
        assert(alone.itemLayout.translation.height < glossLine * 1.5);
        assert([alone.toolTip isEqual:@"汉语\n第一行 第二行"]);

        // 只有注释变了的视图不走只重画译文的捷径，按钮换成新的注释。
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": @"别的注释", @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        assert([CandidateButton(panel.contentView, 0).scriptAnnotation isEqual:@"别的注释"]);

        // 竖排：注释放得下时和释义一样留在候选那一行；比整列还长时在列内折行，不撑出卡片。
        appearance.vertical = YES;
        [controller renderCandidates];
        MSIMECandidateButton *verticalScripted = CandidateButton(panel.contentView, 0);
        assert(!verticalScripted.itemLayout.translation.below && !verticalScripted.translationBelow);
        NSString *wideAnnotation = [@"" stringByPaddingToLength:32 withString:@"﷽" startingAtIndex:0];
        scriptView[@"candidates"] = @[@{@"text": @"汉语", @"script_annotation": wideAnnotation, @"highlighted": @YES}];
        [controller setValue:[scriptView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *wideScripted = CandidateButton(panel.contentView, 0);
        assert([wideScripted.scriptAnnotation isEqual:wideAnnotation]);
        assert(panel.frame.size.width <= halfScreen + 0.5 && NSMaxX(wideScripted.frame) <= panel.frame.size.width + 0.5);
        assert(wideScripted.itemLayout.translation.x + wideScripted.itemLayout.translation.width <= NSWidth(wideScripted.frame) + 0.5);
        assert(NSHeight(wideScripted.frame) >= wideScripted.itemLayout.height - 0.5);
        bitmap = [wideScripted bitmapImageRepForCachingDisplayInRect:wideScripted.bounds];
        [wideScripted cacheDisplayInRect:wideScripted.bounds toBitmapImageRep:bitmap];

        // 读屏：高亮的那一行报告自己被选中，别的行和翻页按钮都不报告；每画一页就请求朗读高亮的那一行。AI 座位上的占位行读成它的意思，不读省略号或破折号。
        appearance.vertical = NO;
        NSDictionary *accessibleBase = @{@"focused": @YES, @"editing_text": @"ceshi", @"page": @0, @"page_count": @2};
        NSArray<NSDictionary *> *(^accessibleRows)(NSUInteger, NSString *) = ^NSArray<NSDictionary *> *(NSUInteger highlighted, NSString *placeholder) {
            NSArray<NSDictionary *> *rows = @[@{@"text": @"测试"}, @{@"text": @"侧室"}, @{@"text": placeholder, @"source": @12}];
            NSMutableArray<NSDictionary *> *marked = [NSMutableArray array];
            for (NSUInteger index = 0; index < rows.count; ++index) {
                NSMutableDictionary *row = [rows[index] mutableCopy];
                row[@"highlighted"] = @(index == highlighted);
                [marked addObject:row];
            }
            return marked;
        };
        void (^renderAccessible)(NSUInteger, NSString *) = ^(NSUInteger highlighted, NSString *placeholder) {
            NSMutableDictionary *view = [accessibleBase mutableCopy];
            view[@"candidates"] = accessibleRows(highlighted, placeholder);
            [controller setValue:[view copy] forKey:@"view"];
            [controller renderCandidates];
            for (NSInteger tag = 0; tag < 3; ++tag)
                assert(CandidateButton(panel.contentView, tag).isAccessibilitySelected == (tag == (NSInteger)highlighted));
            for (NSInteger tag : {-1, -2})
                if (MSIMECandidateButton *arrow = CandidateButton(panel.contentView, tag)) assert(!arrow.isAccessibilitySelected);
        };
        renderAccessible(0, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"1  测试"]);
        assert([CandidateButton(panel.contentView, 2).accessibilityLabel isEqual:@"3  AI 候选等待中"]);
        renderAccessible(1, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"2  侧室"]);
        renderAccessible(2, @"…");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"3  AI 候选等待中"]);
        renderAccessible(2, @"—");
        assert([[controller valueForKey:@"announcedCandidate"] isEqual:@"3  AI 候选没有结果"]);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return 0;
}
