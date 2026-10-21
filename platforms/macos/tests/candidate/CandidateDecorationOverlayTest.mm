#import "../settings/TestPreferenceSuite.h"
#import "../../src/input/InputController.mm"

#include <cassert>
#include <cstring>
#include <filesystem>
#include <fstream>

@interface DecorationOverlayClient : NSObject <MSIMETextClient>
@end
@implementation DecorationOverlayClient
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect
{
    (void)index;
    *rect = NSMakeRect(NSMidX(NSScreen.mainScreen.visibleFrame), NSMidY(NSScreen.mainScreen.visibleFrame), 1, 18);
    return @{};
}
- (void)insertText:(id)text replacementRange:(NSRange)range { (void)text; (void)range; }
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement
{ (void)text; (void)selection; (void)replacement; }
@end

static NSView *WordView(NSView *root)
{
    if ([root.identifier isEqual:@"candidate-decoration-word"]) return root;
    for (NSView *child in root.subviews) if (NSView *found = WordView(child)) return found;
    return nil;
}

static NSString *Word(NSView *root)
{
    return [(NSAttributedString *)[WordView(root) valueForKey:@"text"] string] ?: @"";
}

static NSDictionary *Page(NSInteger highlight)
{
    return @{@"focused": @YES, @"editing_text": @"ce", @"page": @0, @"page_count": @2,
        @"candidates": @[@{@"text": @"示例(原文)", @"id": @{@"generation": @1, @"index": @0}, @"source": @2, @"highlighted": @(highlight == 0)},
            @{@"text": @"合成很长很长很长很长很长的候选词", @"id": @{@"generation": @1, @"index": @1}, @"source": @3, @"highlighted": @(highlight == 1)}]};
}

static NSDictionary *TranslatedPage(NSString *translation)
{
    NSMutableDictionary *page = [Page(0) mutableCopy];
    NSMutableArray *candidates = [page[@"candidates"] mutableCopy];
    NSMutableDictionary *candidate = [candidates[0] mutableCopy];
    candidate[@"translation"] = translation;
    candidates[0] = candidate;
    page[@"candidates"] = candidates;
    return page;
}

static void Geometry()
{
    msime::candidate::DecorationCandidate style;
    assert(style.valid());
    assert(style.seat(2, 1) == 0);
    style.index = 0;
    assert(style.seat(2, 1) == 1 && !style.seat(2, -1) && !style.seat(2, 2));
    style.index = 100;
    assert(!style.seat(2, 0) && style.seat(100, 0) == 99);
    style.fontSizeDip = 30;
    style.minFontSizeDip = 10;
    assert(style.fittedFont(300, 120) == 12);
    assert(style.fittedFont(1000, 120) == 10);
    style.minFontSizeDip = 80;
    assert(style.fittedFont(1000, 120) == 30);
    style.minFontSizeDip = 10;
    style.maxWidthDip = 60;
    assert(style.fittedFont(120, 120) == 15);
    const auto scaled = style.scaled(2);
    assert(scaled.fontSizeDip == 60 && scaled.minFontSizeDip == 20 && scaled.maxWidthDip == 120);
    style.alignH = "right"; style.alignV = "bottom";
    style.offsetXDip = -3; style.offsetYDip = 4;
    assert(style.left(120, 150) == -33 && style.top(40, 20) == 24);
}

int main()
{
    @autoreleasepool {
        [NSApplication sharedApplication];
        Geometry();
        NSString *name = [@"msime-decoration-overlay-" stringByAppendingString:NSUUID.UUID.UUIDString];
        const std::filesystem::path root([NSTemporaryDirectory() stringByAppendingPathComponent:name].fileSystemRepresentation);
        std::filesystem::create_directories(root / "sample");
        NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:60 pixelsHigh:40
            bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSCalibratedRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
        memset(bitmap.bitmapData, 180, bitmap.bytesPerRow * bitmap.pixelsHigh);
        assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
            writeToFile:@((root / "sample/image.png").c_str()) atomically:YES]);
        auto manifest = [&](bool badge, int index, const std::string &alignH, const std::string &alignV, bool enabled = true) {
            std::ofstream output(root / "sample/skin.toml");
            output << "schema_version = 2\nid = 'sample'\nname = '合成叠加皮肤'\nversion = '1'\nbase = 'system'\n"
                "[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['light', 'dark']\n"
                "[candidate_window.decoration]\nimage = 'image.png'\ntop_inset_dip = 40\nwidth_dip = 120\n"
                "motion = 'sway'\ncandidate_overlay = " << (enabled ? "true" : "false") << "\ncandidate_overlay_font_size_dip = 30\n"
                "candidate_overlay_min_font_size_dip = 10\ncandidate_overlay_color = 'abc'\n"
                "candidate_overlay_index = " << index << "\ncandidate_overlay_strip_badge = " << (badge ? "false" : "true")
                << "\ncandidate_overlay_align_h = '" << alignH << "'\ncandidate_overlay_align_v = '" << alignV << "'\n";
            assert(output.good());
        };
        NSString *suite = [@"app.msime.test.decoration-overlay." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
        [defaults setObject:@"sample" forKey:@"MSIMEClientCustomCandidateSkin"];
        [defaults setObject:@"sample" forKey:@"MSIMEClientCustomCandidateSkinDark"];
        DecorationOverlayClient *client = [DecorationOverlayClient new];
        for (BOOL vertical : {NO, YES}) for (BOOL badge : {NO, YES})
            for (const std::string horizontal : {"left", "center", "right"})
                for (const std::string alignment : {"top", "center", "bottom"}) {
                    manifest(badge, 0, horizontal, alignment);
                    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
                        skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
                    appearance.vertical = vertical;
                    const auto skin = [appearance candidateWindowSkinForDark:NO];
                    assert(skin.decorationCandidate && skin.decorationCandidate->color == "#AABBCCFF");
                    MSIMEInputController *controller = [MSIMEInputController new];
                    [controller setValue:appearance forKey:@"appearance"];
                    [controller setValue:client forKey:@"activeClient"];
                    [controller setValue:Page(0) forKey:@"view"];
                    [controller renderCandidates];
                    NSPanel *panel = [controller valueForKey:@"panel"];
                    assert([Word(panel.contentView) isEqual:(badge ? @"示例(原文) ☁️" : @"示例(原文)")]);
                    NSView *word = WordView(panel.contentView);
                    NSAttributedString *text = [word valueForKey:@"text"];
                    NSColor *color = [[text attribute:NSForegroundColorAttributeName atIndex:0 effectiveRange:nullptr]
                        colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
                    assert(std::abs(color.redComponent - 170.0 / 255) < .001);
                    assert([word.superview hitTest:NSMakePoint(1, 1)] == nil);
                    assert(!word.superview.clipsToBounds && !word.superview.layer.masksToBounds);
                    const auto &style = *skin.decorationCandidate;
                    const CGFloat originX = skin.decorationAlign == msime::mac::DecorationAlign::left ? 0
                        : (NSWidth(word.superview.bounds) - 120) * (skin.decorationAlign == msime::mac::DecorationAlign::center ? .5 : 1);
                    assert(std::abs(NSMinX(word.frame) - (originX + style.left(120, text.size.width))) < .01);
                    assert(std::abs(NSMinY(word.frame) - (40 - style.top(40, text.size.height) - text.size.height)) < .01);
                    [controller setValue:Page(1) forKey:@"view"];
                    [controller renderCandidates];
                    word = WordView(panel.contentView);
                    text = [word valueForKey:@"text"];
                    assert([text.string hasPrefix:@"合成很长"]);
                    NSFont *font = [text attribute:NSFontAttributeName atIndex:0 effectiveRange:nullptr];
                    assert(font.pointSize == 10 && text.size.width > 120);
                    // 装饰来源标注遵守 AI 样式，皮肤去标注开关仍优先；不修改原候选字典。
                    NSDictionary *aiPage = Page(1);
                    for (NSString *badgeStyle in @[@"robot", @"whale"]) {
                        [controller applySharedToolbarPreferences:@{
                            @"candidate_ai_badge_style": badgeStyle,
                            @"global_theme": @"custom",
                            @"custom_theme": @{@"base": @"system", @"candidate_skin": @"sample", @"candidate_skin_dark": @"sample"}
                        }];
                        [controller setValue:aiPage forKey:@"view"];
                        [controller renderCandidates];
                        NSString *suffix = badge ? ([badgeStyle isEqual:@"whale"] ? @" 🐳" : @" 🤖") : @"";
                        NSString *expected = [aiPage[@"candidates"][1][@"text"] stringByAppendingString:suffix];
                        assert([Word(panel.contentView) isEqual:expected]);
                        assert([aiPage[@"candidates"][1][@"source"] isEqual:@3]);
                        assert(([aiPage[@"candidates"][1][@"id"] isEqual:@{@"generation": @1, @"index": @1}]));
                    }
                    [controller setValue:Page(-1) forKey:@"view"];
                    [controller renderCandidates];
                    assert(WordView(panel.contentView) == nil);
                    [panel orderOut:nil];
                }
        // 同一控制器上的换肤、缩放和展开路径，不能只验证新建候选窗。
        auto appearance = [&] {
            return [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
                skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        };
        MSIMEInputController *controller = [MSIMEInputController new];
        [controller setValue:client forKey:@"activeClient"];
        manifest(false, 1, "center", "center");
        MSIMEAppearancePreferences *preferences = appearance();
        [controller setValue:preferences forKey:@"appearance"];
        [controller setValue:TranslatedPage(@"旧释义") forKey:@"view"];
        [controller renderCandidates];
        NSPanel *panel = [controller valueForKey:@"panel"];
        NSView *original = WordView(panel.contentView);
        assert(original && [Word(panel.contentView) isEqual:@"示例(原文)"]);
        [controller setValue:TranslatedPage(@"新释义") forKey:@"view"];
        [controller renderCandidates];
        assert(WordView(panel.contentView) == original);
        assert([Word(panel.contentView) isEqual:@"示例(原文)"]);
        preferences.candidateScalePercent = 150;
        [controller renderCandidates];
        NSAttributedString *scaledText = [WordView(panel.contentView) valueForKey:@"text"];
        NSFont *scaledFont = [scaledText attribute:NSFontAttributeName atIndex:0 effectiveRange:nullptr];
        assert(scaledFont.pointSize > 30);
        [controller setValue:@[@{@"text": @"网格合成词"}, @{@"text": @"网格第二词"}] forKey:@"expandedCandidates"];
        [controller setValue:@1 forKey:@"expandedCursor"];
        assert([controller renderExpandedCandidates]);
        // 来源的展开网格仍取普通当前页，不能把网格光标当成当前页高亮。
        assert([Word(panel.contentView) isEqual:@"示例(原文)"]);
        [controller setValue:nil forKey:@"expandedCandidates"];
        for (int index : {100, 0}) {
            manifest(false, index, "center", "center", index == 100);
            [controller setValue:appearance() forKey:@"appearance"];
            [controller renderCandidates];
            assert(WordView(panel.contentView) == nil);
        }
        [panel orderOut:nil];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        std::filesystem::remove_all(root);
    }
    return 0;
}
