#import "../../src/settings/AppearancePreferences.h"
#import "../../src/candidate/CandidateSkinPreviewView.h"
#import "../../src/candidate/CandidateTextMetrics.h"
#import "../../src/cloud/CloudAppearanceSettings.h"
#import "../../src/settings/PreferenceSnapshotMerge.h"
#import <CoreText/CoreText.h>
#include <cassert>
#include <cstring>
#include <fstream>
#import "../settings/TestPreferenceSuite.h"
#import "../settings/PreferenceViewLookup.h"

@interface MSIMECandidatePreviewView (ScaledGeometryTest)
- (msime::mac::ResolvedSkin)previewWindowSkin;
@end

static NSView *FindControl(NSView *root, NSString *label) {
    if ([root.accessibilityLabel isEqual:label]) return root;
    for (NSView *child in root.subviews) {
        NSView *found = FindControl(child, label);
        if (found) return found;
    }
    return nil;
}

static NSString *RenderedFamilyForText(NSFont *font, NSString *value) {
    NSAttributedString *text = [[NSAttributedString alloc] initWithString:value attributes:@{NSFontAttributeName:font}];
    CTLineRef line = CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)text);
    CFArrayRef runs = CTLineGetGlyphRuns(line);
    assert(CFArrayGetCount(runs) == 1);
    CTRunRef run = (CTRunRef)CFArrayGetValueAtIndex(runs, 0);
    CTFontRef actual = (CTFontRef)CFDictionaryGetValue(CTRunGetAttributes(run), kCTFontAttributeName);
    NSString *family = CFBridgingRelease(CTFontCopyFamilyName(actual));
    CFRelease(line);
    return family;
}

static NSString *RenderedFamily(NSFont *font) { return RenderedFamilyForText(font, @"合"); }

static NSColor *TestCandidateColor(NSString *value) {
    unsigned int rgb = 0;
    [[NSScanner scannerWithString:[value substringFromIndex:1]] scanHexInt:&rgb];
    return [NSColor colorWithSRGBRed:((rgb >> 16) & 255) / 255.0 green:((rgb >> 8) & 255) / 255.0 blue:(rgb & 255) / 255.0 alpha:1];
}

static NSDictionary *Pickers(NSDictionary *colors) {
    return @{@"candidate_colors": colors};
}

static bool TokenIs(msime::mac::Rgba color, NSString *hex, CGFloat alpha = 1) {
    NSColor *expected = TestCandidateColor(hex);
    return std::abs(color.r - expected.redComponent) < .002 && std::abs(color.g - expected.greenComponent) < .002 &&
           std::abs(color.b - expected.blueComponent) < .002 && std::abs(color.a - alpha) < .002;
}

static void TestFallbackFonts(MSIMEAppearancePreferences *preferences, NSUserDefaults *defaults) {
    // The list has no editor on the page: the 字体预设 popup writes it, and the shared document carries it between hosts.
    assert(FindControl(preferences.window.contentView, @"候选字体卡片"));
    assert(!FindControl(preferences.window.contentView, @"添加补充字体") && !FindControl(preferences.window.contentView, @"补充字体顺序"));
    NSString *sans = [NSFont fontWithName:@"PingFangSC-Regular" size:18].familyName;
    NSString *serif = [NSFont fontWithName:@"STSongti-SC-Regular" size:18].familyName;
    assert(sans && serif);
    __block NSUInteger notifications = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:preferences queue:nil usingBlock:^(NSNotification *note) { (void)note; ++notifications; }];
    [preferences applySharedCandidatePreferences:@{@"candidate_font_family": @"Menlo", @"candidate_english_font": @"Helvetica", @"candidate_fallback_fonts": @[sans, serif]}];
    assert(notifications == 0 && ([preferences.fallbackFonts isEqual:@[sans, serif]]));
    assert([preferences.candidateEnglishFont isEqual:@"Helvetica"]);
    NSDictionary *fontMerge = [preferences sharedPreferencesByMerging:@{}];
    assert([fontMerge[@"candidate_english_font"] isEqual:@"Helvetica"]);
    assert([RenderedFamilyForText([preferences candidateFontOfSize:18 englishFirst:YES], @"Latin") isEqual:@"Helvetica"]);
    [preferences applySharedCandidatePreferences:@{}];
    assert(!preferences.candidateEnglishFont);
    [preferences applySharedCandidatePreferences:@{@"candidate_english_font": @"Helvetica"}];
    assert([RenderedFamily([preferences candidateFontOfSize:18]) isEqual:sans]);
    preferences.fallbackFonts = @[serif, sans];
    assert([RenderedFamily([preferences candidateFontOfSize:18]) isEqual:serif]);
    preferences.fallbackFonts = @[sans, serif];
    MSIMEAppearancePreferences *reloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
    assert([reloaded.fallbackFonts isEqual:preferences.fallbackFonts]);
    NSArray *saved = preferences.fallbackFonts;
    for (id invalid in @[@"bad", NSNull.null, @[@""], @[@YES], @[[ @"字" stringByPaddingToLength:43 withString:@"字" startingAtIndex:0]]]) {
        [preferences applySharedCandidatePreferences:@{@"candidate_fallback_fonts": invalid}];
        assert([preferences.fallbackFonts isEqual:saved]);
    }
    [preferences applySharedCandidatePreferences:@{@"candidate_english_font": @"Helvetica"}];
    assert([preferences.candidateEnglishFont isEqual:@"Helvetica"]);
    [preferences applySharedCandidatePreferences:@{@"candidate_english_font": @"bad\nname"}];
    assert([preferences.candidateEnglishFont isEqual:@"Helvetica"]);
    NSMutableArray *limit = [NSMutableArray array];
    for (NSUInteger i = 0; i < 32; ++i) [limit addObject:sans];
    preferences.fallbackFonts = limit;
    assert(preferences.fallbackFonts.count == 32);
    [limit addObject:serif];
    preferences.fallbackFonts = limit;
    assert(preferences.fallbackFonts.count == 32);
    NSMutableString *mutableFamily = [sans mutableCopy];
    NSMutableArray *mutableFonts = [NSMutableArray arrayWithObject:mutableFamily];
    [preferences applySharedCandidatePreferences:@{@"candidate_fallback_fonts": mutableFonts}];
    [mutableFamily appendString:@" synthetic mutation"];
    [mutableFonts removeAllObjects];
    assert(([preferences.fallbackFonts isEqual:@[sans]]));
    preferences.fontFamily = @"MSIME Synthetic Unavailable Primary";
    preferences.fallbackFonts = @[@"MSIME Synthetic Unavailable Supplement", serif];
    assert([[preferences candidateFontOfSize:18].familyName isEqual:serif]);
    preferences.fallbackFonts = @[];
    assert([[preferences sharedPreferencesByMerging:@{@"candidate_fallback_fonts": @[sans]}][@"candidate_fallback_fonts"] isEqual:@[]]);
    assert([[preferences candidateFontOfSize:18].fontName isEqual:[NSFont systemFontOfSize:18].fontName]);
    preferences.fontFamily = @"Segoe UI";
    [NSNotificationCenter.defaultCenter removeObserver:observer];
}

static void TestCloudImportCache(MSIMEAppearancePreferences *preferences, NSUserDefaults *defaults) {
    NSDictionary *original = MSIMECloudAppearanceSnapshot(defaults);
    [preferences applySharedCandidatePreferences:@{@"candidate_font_size": @12, @"candidate_page_size": @1,
        @"candidate_layout": @"vertical", @"candidate_font_family": @"Menlo", @"candidate_preedit_font_size": @28}];
    [preferences applySharedCandidatePreferences:@{
        @"candidate_font_size": [NSDecimalNumber decimalNumberWithString:@"13.0000000000000001"],
        @"candidate_page_size": [NSDecimalNumber decimalNumberWithString:@"2.0000000000000001"],
        @"candidate_preedit_font_size": [NSDecimalNumber decimalNumberWithString:@"27.0000000000000001"]}];
    assert(preferences.fontSize == 12 && preferences.pageSize == 1 && preferences.preeditFontSize == 28);
    [preferences applySharedInputPreferences:@{@"scheme": @"wubi", @"shuangpin_profile": @"microsoft", @"shuangpin_preedit_uses_raw": @NO, @"chinese_punctuation": @NO}];
    assert(preferences.inlinePreeditStyle == MSIMEInlinePreeditStyleRaw);
    [preferences applySharedInputPreferences:@{@"tsf_preedit_style": @"raw"}];
    assert(preferences.inlinePreeditStyle == MSIMEInlinePreeditStyleRaw);
    [preferences applySharedInputPreferences:@{@"tsf_preedit_style": @"pinyin"}];
    assert(preferences.inlinePreeditStyle == MSIMEInlinePreeditStylePinyin);
    [preferences applySharedInputPreferences:@{@"tsf_preedit_style": @"empty"}];
    assert(preferences.inlinePreeditStyle == MSIMEInlinePreeditStyleEmpty);
    [preferences applySharedInputPreferences:@{@"tsf_preedit_style": @"invalid"}];
    assert(preferences.inlinePreeditStyle == MSIMEInlinePreeditStyleEmpty);
    [preferences applySharedAssistancePreferences:@{@"quanpin": @{@"autocorrect_neighbor": @NO}}];
    [preferences applySharedToolbarVisibility:NO];
    assert(!preferences.chinesePunctuation && !preferences.shuangpinPreeditUsesRaw && !preferences.floatingToolbarEnabled);
    NSDictionary *effective = [preferences cloudSettingsSnapshot];
    assert(MSIMEValidateCloudAppearance(effective));
    // The theme is exported as the host draws it, which the shared document supplied and defaults never saw.
    [preferences applySharedCandidatePreferences:@{@"global_theme": @"night", @"custom_theme": @{@"base": @"ink", @"candidate_skin": @"wide-card"}}];
    effective = [preferences cloudSettingsSnapshot];
    assert(MSIMEValidateCloudAppearance(effective));
    assert([effective[@"platform.macos.global_theme"] isEqual:@"night"] && [effective[@"platform.macos.custom_theme_base"] isEqual:@"ink"]);
    assert([effective[@"platform.macos.custom_candidate_skin"] isEqual:@"wide-card"]);
    assert(![original[@"platform.macos.global_theme"] isEqual:@"night"] && ![original[@"platform.macos.custom_candidate_skin"] isEqual:@"wide-card"]);
    assert([effective[@"platform.macos.candidate_font_size"] isEqual:@12]);
    // One candidate a page is not a size the window offers, but it is one the shared preferences accept,
    // so the snapshot carries what the document said rather than the nine this used to be rewritten to.
    assert([effective[@"platform.macos.candidate_page_size"] isEqual:@1]);
    assert([effective[@"platform.macos.candidate_panel_style"] isEqual:@1]);
    assert([effective[@"platform.macos.input_scheme"] isEqual:@2]);
    for (NSString *key in @[@"chinese_punctuation", @"shuangpin_preedit_uses_raw", @"floating_toolbar"])
        assert([effective[[@"platform.macos." stringByAppendingString:key]] isEqual:@NO]);
    assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:original]);
    NSMutableDictionary *imported = [original mutableCopy];
    imported[@"platform.macos.global_theme"] = @"shuishan";
    imported[@"platform.macos.candidate_font_size"] = @32;
    imported[@"platform.macos.candidate_page_size"] = @9;
    imported[@"platform.macos.candidate_panel_style"] = @0;
    imported[@"platform.macos.input_scheme"] = @1;
    imported[@"platform.macos.shuangpin_preedit_uses_raw"] = @YES;
    imported[@"platform.macos.autocorrect"] = @YES;
    imported[@"platform.macos.chinese_punctuation"] = @YES;
    imported[@"platform.macos.floating_toolbar"] = @YES;
    __block NSUInteger notifications = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:preferences queue:nil usingBlock:^(NSNotification *note) {
        (void)note; ++notifications;
        assert(preferences.fontSize == 32 && preferences.pageSize == 9 && !preferences.vertical);
        assert([preferences resolvedSkinForDark:NO].id == "shuishan");
    }];
    NSMutableDictionary *invalid = [imported mutableCopy];
    invalid[@"platform.macos.candidate_font_size"] = @33;
    assert(![preferences applyCloudSettingsSnapshot:invalid]);
    assert(notifications == 0 && preferences.fontSize == 12);
    assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:original]);
    assert([preferences applyCloudSettingsSnapshot:imported]);
    assert(notifications == 1);
    assert([[preferences cloudSettingsSnapshot] isEqual:imported]);
    assert(notifications == 1); // Export is read-only and must not schedule a save.
    assert([effective[@"platform.macos.candidate_font_size"] isEqual:@12]); // Earlier snapshot stays immutable.
    assert(preferences.shuangpinPreeditUsesRaw && preferences.chinesePunctuation && preferences.floatingToolbarEnabled);
    assert([preferences.inputScheme isEqual:@"shuangpin"]);
    assert([preferences.fontFamily isEqual:@"Menlo"] && preferences.preeditFontSize == 28);
    assert([preferences.shuangpinProfile isEqual:@"microsoft"] && !preferences.autocorrectNeighbor);
    NSDictionary *merged = [preferences sharedPreferencesByMerging:@{}];
    assert([merged[@"candidate_font_size"] isEqual:@32] && [merged[@"candidate_page_size"] isEqual:@9]);
    assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:imported]);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    assert([preferences applyCloudSettingsSnapshot:original]);
    preferences.fontFamily = @"Segoe UI";
    preferences.preeditFontSize = 16;
}

static NSBitmapImageRep *Draw(MSIMECandidatePreviewView *preview) {
    [preview.superview layoutSubtreeIfNeeded];
    NSBitmapImageRep *bitmap = [preview bitmapImageRepForCachingDisplayInRect:preview.bounds];
    assert(bitmap && bitmap.pixelsWide > 0 && bitmap.pixelsHigh > 0);
    [preview cacheDisplayInRect:preview.bounds toBitmapImageRep:bitmap];
    NSColor *corner = [[bitmap colorAtX:10 y:10] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    NSColor *expected = [preview.previewCanvasFillColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    assert(corner.alphaComponent > .99);
    assert(std::abs(corner.redComponent - expected.redComponent) < .03);
    return bitmap;
}

static NSRect SyntheticSelectionBounds(NSBitmapImageRep *bitmap) {
    NSInteger minX = bitmap.pixelsWide, maxX = -1, minY = bitmap.pixelsHigh, maxY = -1;
    for (NSInteger y = 0; y < bitmap.pixelsHigh; ++y) {
        for (NSInteger x = 0; x < bitmap.pixelsWide; ++x) {
            NSColor *color = [[bitmap colorAtX:x y:y] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            // 显示器色彩配置可改变洋红色的分量，按色相分离高亮与灰色画布、文字。
            if (color.redComponent < .8 || color.greenComponent > .5 || color.blueComponent < .8) continue;
            minX = MIN(minX, x); maxX = MAX(maxX, x);
            minY = MIN(minY, y); maxY = MAX(maxY, y);
        }
    }
    return maxX < minX ? NSZeroRect : NSMakeRect(minX, minY, maxX - minX + 1, maxY - minY + 1);
}

// 共享文档、设置滑块、持久化与预览使用同一份候选窗样式。
static void TestCandidateWindowStyle(MSIMEAppearancePreferences *preferences, NSUserDefaults *defaults) {
    NSView *root = preferences.window.contentView;
    MSIMECandidatePreviewView *preview = (id)MSIMEFindPreferenceViewOfClass(root, MSIMECandidatePreviewView.class);
    NSSlider *scale = (id)FindControl(root, @"整体大小");
    NSSlider *opacity = (id)FindControl(root, @"不透明度");
    NSSlider *radius = (id)FindControl(root, @"圆角大小");
    NSButton *followSkin = (id)FindControl(root, @"圆角大小跟随皮肤");
    NSPopUpButton *presets = (id)FindControl(root, @"字体预设");
    assert(preview && [scale isKindOfClass:NSSlider.class] && [opacity isKindOfClass:NSSlider.class] && [radius isKindOfClass:NSSlider.class]);
    assert([followSkin isKindOfClass:NSButton.class] && [presets isKindOfClass:NSPopUpButton.class] && presets.numberOfItems == 5);
    assert(scale.minValue == 75 && scale.maxValue == 150 && opacity.minValue == 50 && opacity.maxValue == 100 && radius.minValue == 0 && radius.maxValue == 16);

    // Untouched: 100%, 100% and the skin's own radius, published as such so that a restored section reaches the document too.
    assert(preferences.candidateScalePercent == 100 && preferences.candidateOpacityPercent == 100 && preferences.candidateCornerRadius == nil);
    NSDictionary *merged = [preferences sharedPreferencesByMerging:@{}];
    assert([merged[@"candidate_scale_percent"] isEqual:@100] && [merged[@"candidate_opacity_percent"] isEqual:@100]);
    assert(merged[@"candidate_corner_radius"] == NSNull.null && !followSkin.enabled);
    const CGFloat baseHeight = preview.previewContentHeight;

    // A document's values are honoured across the shared ranges, which are wider than the sliders'; anything outside them, or not a whole number, leaves the setting alone.
    NSDictionary *valid = @{@"candidate_scale_percent": @200, @"candidate_opacity_percent": @50, @"candidate_corner_radius": @32};
    [preferences applySharedCandidatePreferences:valid];
    assert(preferences.candidateScalePercent == 200 && preferences.candidateOpacityPercent == 50 && [preferences.candidateCornerRadius isEqual:@32]);
    for (NSArray *entry in @[ @[@"candidate_scale_percent", @49], @[@"candidate_scale_percent", @201], @[@"candidate_scale_percent", @YES],
                              @[@"candidate_scale_percent", @120.5],
                              @[@"candidate_scale_percent", [NSDecimalNumber decimalNumberWithString:@"199.0000000000000001"]],
                              @[@"candidate_opacity_percent", @49], @[@"candidate_opacity_percent", @101],
                              @[@"candidate_opacity_percent", [NSDecimalNumber decimalNumberWithString:@"51.0000000000000001"]],
                              @[@"candidate_corner_radius", @33], @[@"candidate_corner_radius", @(-1)], @[@"candidate_corner_radius", @"8"],
                              @[@"candidate_corner_radius", [NSDecimalNumber decimalNumberWithString:@"31.0000000000000001"]] ]) {
        NSMutableDictionary *document = [valid mutableCopy];
        document[entry[0]] = entry[1];
        [preferences applySharedCandidatePreferences:document];
        assert(preferences.candidateScalePercent == 200 && preferences.candidateOpacityPercent == 50 && [preferences.candidateCornerRadius isEqual:@32]);
    }
    // The shared serializer leaves a default out, so an omitted field is 100% or following the skin again, and so is an explicit null radius.
    [preferences applySharedCandidatePreferences:@{}];
    assert(preferences.candidateScalePercent == 100 && preferences.candidateOpacityPercent == 100 && preferences.candidateCornerRadius == nil);
    [preferences applySharedCandidatePreferences:@{@"candidate_corner_radius": @8}];
    assert([preferences.candidateCornerRadius isEqual:@8] && followSkin.enabled && radius.integerValue == 8);
    [preferences applySharedCandidatePreferences:@{@"candidate_corner_radius": NSNull.null}];
    assert(preferences.candidateCornerRadius == nil && !followSkin.enabled);

    // The candidate window's skin carries the style; the resolved skin the toolbar and the colour wells read does not.
    [preferences applySharedCandidatePreferences:@{@"candidate_scale_percent": @150, @"candidate_opacity_percent": @60, @"candidate_corner_radius": @4}];
    assert(scale.integerValue == 150 && opacity.integerValue == 60 && radius.integerValue == 4);
    const auto plain = [preferences resolvedSkinForDark:NO];
    const auto styled = [preferences candidateWindowSkinForDark:NO];
    assert(styled.tokens.radius == 6.0f && styled.tokens.pad == plain.tokens.pad * 1.5f);
    assert(std::abs(styled.tokens.surface.a - plain.tokens.surface.a * 0.6f) < 0.0001f);
    assert(styled.tokens.text.a == plain.tokens.text.a && styled.tokens.selected.a == plain.tokens.selected.a);
    // The preview draws the panel at the window's scale, so it grows with it.
    assert(preview.previewContentHeight > baseHeight);
    Draw(preview);

    // The sliders write the setting, stored here so it outlives the window, and 跟随皮肤 hands the radius back to the skin with an explicit null.
    scale.integerValue = 125;
    [NSApp sendAction:scale.action to:scale.target from:scale];
    opacity.integerValue = 80;
    [NSApp sendAction:opacity.action to:opacity.target from:opacity];
    radius.integerValue = 12;
    [NSApp sendAction:radius.action to:radius.target from:radius];
    assert(preferences.candidateScalePercent == 125 && preferences.candidateOpacityPercent == 80 && [preferences.candidateCornerRadius isEqual:@12]);
    MSIMEAppearancePreferences *reloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
    assert(reloaded.candidateScalePercent == 125 && reloaded.candidateOpacityPercent == 80 && [reloaded.candidateCornerRadius isEqual:@12]);
    // A fresh process whose first document has these at their defaults (reset on another surface) draws the defaults rather than the stale stored values, and publishes them as such.
    [reloaded applySharedCandidatePreferences:@{}];
    assert(reloaded.candidateScalePercent == 100 && reloaded.candidateOpacityPercent == 100 && reloaded.candidateCornerRadius == nil);
    NSDictionary *fresh = [reloaded sharedPreferencesByMerging:@{}];
    assert([fresh[@"candidate_scale_percent"] isEqual:@100] && fresh[@"candidate_corner_radius"] == NSNull.null);
    NSDictionary *document = MSIMEMergePreferenceSnapshot(@{@"candidate_corner_radius": @3, @"unrelated": @7}, [preferences sharedPreferencesByMerging:@{}]);
    assert([document[@"candidate_scale_percent"] isEqual:@125] && [document[@"candidate_opacity_percent"] isEqual:@80]);
    assert([document[@"candidate_corner_radius"] isEqual:@12] && [document[@"unrelated"] isEqual:@7]);
    [NSApp sendAction:followSkin.action to:followSkin.target from:followSkin];
    assert(preferences.candidateCornerRadius == nil);
    document = MSIMEMergePreferenceSnapshot(document, [preferences sharedPreferencesByMerging:@{}]);
    assert(document[@"candidate_corner_radius"] == NSNull.null);
    [preferences applySharedCandidatePreferences:document];
    assert(preferences.candidateCornerRadius == nil && preferences.candidateScalePercent == 125);
    // The setters refuse what the shared document would refuse.
    preferences.candidateScalePercent = 49;
    preferences.candidateOpacityPercent = 101;
    preferences.candidateCornerRadius = @33;
    assert(preferences.candidateScalePercent == 125 && preferences.candidateOpacityPercent == 80 && preferences.candidateCornerRadius == nil);

    // 内边距 (#7316) works like the radius: it follows the skin until set, a document's value inside 0-16 is honoured, the slider writes it here and 跟随皮肤 hands it back with an explicit null.
    {
        NSSlider *padding = (id)FindControl(root, @"内边距");
        NSButton *followPadding = (id)FindControl(root, @"内边距跟随皮肤");
        assert([padding isKindOfClass:NSSlider.class] && [followPadding isKindOfClass:NSButton.class]);
        assert(padding.minValue == 0 && padding.maxValue == 16);
        [preferences applySharedCandidatePreferences:@{}];
        assert(preferences.candidatePadding == nil && !followPadding.enabled);
        assert(padding.integerValue == lround([preferences resolvedSkinForDark:NO].tokens.pad));
        assert([preferences sharedPreferencesByMerging:@{}][@"candidate_padding"] == NSNull.null);
        [preferences applySharedCandidatePreferences:@{@"candidate_padding": @3}];
        assert([preferences.candidatePadding isEqual:@3] && followPadding.enabled && padding.integerValue == 3);
        assert([preferences candidateWindowSkinForDark:NO].tokens.pad == 3.0f);
        for (id invalid in @[ @17, @(-1), @"3", @YES, @2.5 ]) {
            [preferences applySharedCandidatePreferences:@{@"candidate_padding": invalid}];
            assert([preferences.candidatePadding isEqual:@3]);
        }
        [preferences applySharedCandidatePreferences:@{@"candidate_padding": @0}];
        assert([preferences.candidatePadding isEqual:@0]);
        // 零内边距不再保留边框下限；1pt 和 16pt 分别增加两侧对应的留白。
        const CGFloat minimumPaddingHeight = preview.previewContentHeight;
        preferences.candidatePadding = @1;
        assert(std::abs(preview.previewContentHeight - minimumPaddingHeight - 2.0 * preferences.candidateScalePercent / 100.0) < .01);
        preferences.candidatePadding = @16;
        assert(std::abs(preview.previewContentHeight - minimumPaddingHeight - 2.0 * 16.0 * preferences.candidateScalePercent / 100.0) < .01);
        // 内边距按整体大小缩放；横排、竖排和双布局预览的高度都按真实窗口计算。
        const NSInteger previousScale = preferences.candidateScalePercent;
        const BOOL previousVertical = preferences.vertical;
        for (NSNumber *percent in @[@50, @100, @150, @200]) {
            preferences.candidateScalePercent = percent.integerValue;
            for (NSNumber *vertical in @[@NO, @YES]) {
                preferences.vertical = vertical.boolValue;
                for (NSNumber *showcase in @[@NO, @YES]) {
                    [preview setShowsLayoutShowcase:showcase.boolValue];
                    preferences.candidatePadding = @0;
                    const CGFloat tightHeight = preview.previewContentHeight;
                    const auto actual = [preferences candidateWindowSkinForDark:[preview previewUsesDark]];
                    const auto drawing = [preview previewWindowSkin];
                    const CGFloat factor = percent.doubleValue / 100.0;
                    assert(std::abs(drawing.tokens.borderWidth * factor - actual.tokens.borderWidth) < .001);
                    assert(std::abs(msime::mac::CandidateInset(drawing.tokens) * factor - msime::mac::CandidateInset(actual.tokens)) < .001);
                    assert(std::abs(drawing.tokens.selectedRadius * factor - actual.tokens.selectedRadius) < .001);
                    preferences.candidatePadding = @16;
                    const CGFloat panels = showcase.boolValue ? 2.0 : 1.0;
                    const CGFloat expected = 2.0 * panels * 16.0 * percent.doubleValue / 100.0;
                    assert(std::abs(preview.previewContentHeight - tightHeight - expected) < .01);
                }
            }
        }
        // 位图使用合成洋红色定位高亮，防止只校验高度却漏掉候选整行未绘制。
        [preview setShowsLayoutShowcase:NO];
        NSDictionary *originalAppearance = [preferences cloudSettingsSnapshot];
        const BOOL previousPreedit = preferences.showsCandidatePreedit;
        NSString *previousSelected = preferences.candidateSelectedColor;
        // 贴边宽度只测矩形高亮，避免圆角遮罩与颜色阈值改变可见像素边界。
        [preferences applySharedCandidatePreferences:@{
            @"candidate_corner_radius": @0, @"candidate_item_corner_radius": @0}];
        preferences.showsCandidatePreedit = NO;
        preferences.pageSize = 1;
        preferences.candidateSelectedColor = @"#FF00FF";
        for (NSNumber *percent in @[@50, @100, @150, @200]) {
            preferences.candidateScalePercent = percent.integerValue;
            for (NSNumber *paddingValue in @[@0, @1]) {
                preferences.candidatePadding = paddingValue;
                for (NSNumber *vertical in @[@NO, @YES]) {
                    preferences.vertical = vertical.boolValue;
                    NSBitmapImageRep *bitmap = Draw(preview);
                    const NSRect selected = SyntheticSelectionBounds(bitmap);
                    assert(NSWidth(selected) > 10 && NSHeight(selected) > 2);
                    if (vertical.boolValue) {
                        const CGFloat pixelsPerPoint = bitmap.pixelsWide / NSWidth(preview.bounds);
                        const auto tokens = [preferences candidateWindowSkinForDark:preview.previewUsesDark].tokens;
                        assert(tokens.radius == 0 && tokens.selectedRadius == 0);
                        const CGFloat inset = MAX(msime::mac::CandidateInset(tokens), MAX(0.0, tokens.borderWidth) / 2.0);
                        const CGFloat expectedWidth = NSWidth(preview.bounds) - 28.0 - 2.0 * inset;
                        const CGFloat actualWidth = NSWidth(selected) / pixelsPerPoint;
                        // 两侧半像素的覆盖率会让颜色阈值少计一个物理像素，不按逻辑点放宽。
                        const BOOL matches = std::abs(actualWidth - expectedWidth) <= 1.0 / pixelsPerPoint + .001;
                        if (!matches) {
                            NSLog(@"Synthetic selection width mismatch: scale=%@ padding=%@ pixelsPerPoint=%.3f actual=%.3f expected=%.3f",
                                  percent, paddingValue, pixelsPerPoint, actualWidth, expectedWidth);
                        }
                        assert(matches);
                    }
                }
            }
        }
        preferences.candidateSelectedColor = previousSelected;
        assert([preferences applyCloudSettingsSnapshot:originalAppearance]);
        preferences.showsCandidatePreedit = previousPreedit;
        [preview setShowsLayoutShowcase:NO];
        preferences.vertical = previousVertical;
        preferences.candidateScalePercent = previousScale;
        preferences.candidatePadding = @0;
        [preferences applySharedCandidatePreferences:@{@"candidate_padding": NSNull.null}];
        assert(preferences.candidatePadding == nil);

        padding.integerValue = 2;
        [NSApp sendAction:padding.action to:padding.target from:padding];
        assert([preferences.candidatePadding isEqual:@2]);
        MSIMEAppearancePreferences *again = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert([again.candidatePadding isEqual:@2]);
        NSDictionary *paddedDocument = MSIMEMergePreferenceSnapshot(@{@"candidate_padding": @9}, [preferences sharedPreferencesByMerging:@{}]);
        assert([paddedDocument[@"candidate_padding"] isEqual:@2]);
        [NSApp sendAction:followPadding.action to:followPadding.target from:followPadding];
        assert(preferences.candidatePadding == nil);
        paddedDocument = MSIMEMergePreferenceSnapshot(paddedDocument, [preferences sharedPreferencesByMerging:@{}]);
        assert(paddedDocument[@"candidate_padding"] == NSNull.null);
        preferences.candidatePadding = @17;
        assert(preferences.candidatePadding == nil);
    }

    // 字体预设: this host's family for the preset, all of the preset's names at the front of the fallback list, then what the user had, without repeats.
    preferences.fallbackFonts = @[@"Menlo", @"SimSun"];
    [presets selectItemAtIndex:1];
    [NSApp sendAction:presets.action to:presets.target from:presets];
    assert([preferences.fontFamily isEqual:@"Songti SC"] && preferences.candidateFontPreset == 1 && presets.indexOfSelectedItem == 1);
    assert(([preferences.fallbackFonts isEqual:@[@"Songti SC", @"SimSun", @"Noto Serif CJK SC", @"Noto Serif SC", @"Menlo"]]));
    assert([[preferences sharedPreferencesByMerging:@{}][@"candidate_font_family"] isEqual:@"Songti SC"]);
    // A full list keeps its length: the preset goes in front and the tail gives way.
    NSMutableArray<NSString *> *full = [NSMutableArray array];
    for (NSUInteger index = 0; index < 32; ++index) [full addObject:[NSString stringWithFormat:@"Synthetic Family %lu", (unsigned long)index]];
    preferences.fallbackFonts = full;
    preferences.candidateFontPreset = 3;
    assert([preferences.fontFamily isEqual:@"Kaiti SC"] && preferences.fallbackFonts.count == 32);
    assert(([[preferences.fallbackFonts subarrayWithRange:NSMakeRange(0, 5)] isEqual:@[@"Kaiti SC", @"KaiTi", @"STKaiti", @"AR PL UKai CN", @"Synthetic Family 0"]]));
    // Another platform's name for a preset reads as that preset; any other family is no preset at all.
    preferences.fontFamily = @"Microsoft YaHei";
    assert(preferences.candidateFontPreset == 2 && presets.indexOfSelectedItem == 2);
    preferences.fontFamily = @"Menlo";
    assert(preferences.candidateFontPreset == -1 && presets.indexOfSelectedItem == -1);
    // 默认 is the shared default pair as it stands, whatever was there before.
    [presets selectItemAtIndex:0];
    [NSApp sendAction:presets.action to:presets.target from:presets];
    assert([preferences.fontFamily isEqual:@"Noto Sans SC"] && preferences.candidateFontPreset == 0);
    assert(([preferences.fallbackFonts isEqual:@[@"Noto Sans SC", @"Microsoft YaHei"]]));

    preferences.candidateScalePercent = 100;
    preferences.candidateOpacityPercent = 100;
    preferences.fallbackFonts = @[];
    preferences.fontFamily = @"Segoe UI";
    assert(std::abs(preview.previewContentHeight - baseHeight) < .01);
}

static void TestCandidateSurfaceTheme(MSIMEAppearancePreferences *preferences) {
    NSArray *names = @[NSAppearanceNameAqua, NSAppearanceNameDarkAqua];
    [preferences applySharedCandidatePreferences:@{@"theme": @"light", @"candidate_theme": @"dark"}];
    NSString *match = [preferences.candidateAppearanceOverride bestMatchFromAppearancesWithNames:names];
    assert([match isEqual:NSAppearanceNameDarkAqua]);
    [preferences applySharedCandidatePreferences:@{@"theme": @"dark", @"candidate_theme": @"light"}];
    match = [preferences.candidateAppearanceOverride bestMatchFromAppearancesWithNames:names];
    assert([match isEqual:NSAppearanceNameAqua]);
    [preferences applySharedCandidatePreferences:@{@"theme": @"dark", @"candidate_theme": @"follow"}];
    match = [preferences.candidateAppearanceOverride bestMatchFromAppearancesWithNames:names];
    assert([match isEqual:NSAppearanceNameDarkAqua]);
    [preferences applySharedCandidatePreferences:@{@"theme": @"light", @"candidate_theme": @"follow"}];
    match = [preferences.candidateAppearanceOverride bestMatchFromAppearancesWithNames:names];
    assert([match isEqual:NSAppearanceNameAqua]);
    [preferences applySharedCandidatePreferences:@{@"theme": @"system", @"candidate_theme": @"follow"}];
    assert(preferences.candidateAppearanceOverride == nil);
    [preferences applySharedCandidatePreferences:@{@"theme": @"invalid", @"candidate_theme": @"invalid"}];
    assert(preferences.candidateAppearanceOverride == nil);
    [preferences applySharedCandidatePreferences:@{@"theme": @"dark", @"candidate_theme": @"dark"}];
    [preferences applySharedCandidatePreferences:@{@"theme": @42, @"candidate_theme": @YES}];
    match = [preferences.candidateAppearanceOverride bestMatchFromAppearancesWithNames:names];
    assert([match isEqual:NSAppearanceNameDarkAqua]);
}

// The toolbar preview paints the divider in the candidate outline, as the panel does, so a theme change reaches it. The logo beside it is the brand mark and keeps its own colours.
static void TestToolbarPreviewChrome(MSIMEAppearancePreferences *preferences) {
    NSString *globalTheme = preferences.globalTheme;
    const BOOL showsAppLogo = preferences.showsAppLogo;
    // 带 logo 和不带 logo（左端换成窄 24pt 的握把）两种布局都要量。
    for (NSNumber *logo in @[@YES, @NO])
    for (NSString *skin in @[@"shuishan", @"paper"]) {
        preferences.showsAppLogo = logo.boolValue;
        preferences.globalTheme = skin;
        MSIMEToolbarPreviewView *toolbar = [[MSIMEToolbarPreviewView alloc] initWithFrame:NSMakeRect(0, 0, 580, 100)];
        toolbar.preferences = preferences;
        toolbar.frame = NSMakeRect(0, 0, 580, toolbar.fittingSize.height);
        assert(NSHeight(toolbar.frame) > 48.0);
        NSBitmapImageRep *bitmap = [toolbar bitmapImageRepForCachingDisplayInRect:toolbar.bounds];
        [toolbar cacheDisplayInRect:toolbar.bounds toBitmapImageRep:bitmap];
        const CGFloat pixels = bitmap.pixelsWide / NSWidth(toolbar.bounds);
        const CGFloat scale = preferences.floatingToolbarScalePercent / 100.0;
        const msime::mac::SkinTokens tokens = [preferences toolbarSkinForDark:toolbar.previewUsesDark];
        assert(tokens.border.a > .01f);
        // The divider is a 1.2pt bar 41pt into the toolbar, which sits at the canvas's 14pt inset and is centred vertically between 34pt down and 14pt above the bottom. Sampled mid-bar, it is the outline laid over the toolbar surface just left of it.
        const CGFloat y = (34.0 + NSHeight(toolbar.bounds) - 14.0) / 2.0;
        const CGFloat shift = logo.boolValue ? 0.0 : 24.0;
        NSColor *divider = [[bitmap colorAtX:(NSInteger)((14.0 + (41.6 - shift) * scale) * pixels) y:(NSInteger)(y * pixels)] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        NSColor *surface = [[bitmap colorAtX:(NSInteger)((14.0 + (39.5 - shift) * scale) * pixels) y:(NSInteger)(y * pixels)] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        const CGFloat alpha = tokens.border.a;
        assert(std::abs(divider.redComponent - (tokens.border.r * alpha + surface.redComponent * (1.0 - alpha))) < .03);
        assert(std::abs(divider.greenComponent - (tokens.border.g * alpha + surface.greenComponent * (1.0 - alpha))) < .03);
        assert(std::abs(divider.blueComponent - (tokens.border.b * alpha + surface.blueComponent * (1.0 - alpha))) < .03);
    }
    preferences.globalTheme = globalTheme;
    preferences.showsAppLogo = showsAppLogo;
}

// InputController saves by merging -sharedPreferencesByMerging:@{} over the document on disk with MSIMEMergePreferenceSnapshot and then reloads that document, so a clear has to survive the merge: a key the overrides leave out keeps the old value, and the reload then brings it back over the native setting.
static void TestCustomThemeClearsSurviveSave() {
    NSString *suite = [@"app.msime.test.preview.clears." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    __block NSDictionary *document = @{@"global_theme": @"custom", @"unrelated": @7,
        @"custom_theme": @{@"base": @"night", @"candidate_skin": @"wide-card", @"keyboard": @{@"background": @1},
                           @"candidate_colors": @{@"text": @"#112233", @"accent": @"#445566"}}};
    void (^save)(void) = ^{
        document = MSIMEMergePreferenceSnapshot(document, [preferences sharedPreferencesByMerging:@{}]);
        assert(document && [NSJSONSerialization isValidJSONObject:document]);
        [preferences applySharedCandidatePreferences:document];
    };
    [preferences applySharedCandidatePreferences:document];
    assert([preferences.customCandidateSkin isEqual:@"wide-card"] && [preferences.candidateTextColor isEqual:@"#112233"]);
    [preferences clearCustomCandidateSkin];
    save();
    assert(preferences.customCandidateSkin == nil && document[@"custom_theme"][@"candidate_skin"] == NSNull.null);
    [NSApp sendAction:NSSelectorFromString(@"resetTextColor:") to:preferences from:nil];
    save();
    assert(preferences.candidateTextColor == nil && document[@"custom_theme"][@"candidate_colors"][@"text"] == NSNull.null);
    // Untouched parts of the document are carried through.
    assert([preferences.candidateAccentColor isEqual:@"#445566"] && [preferences.customThemeBase isEqual:@"night"]);
    assert([document[@"unrelated"] isEqual:@7] && [document[@"custom_theme"][@"keyboard"] isEqual:@{@"background": @1}]);
    // A picker set while the system theme is on screen makes system the custom theme's base, over a stored night base and package.
    document = @{@"global_theme": @"system", @"custom_theme": @{@"base": @"night", @"candidate_skin": @"wide-card"}};
    [preferences applySharedCandidatePreferences:document];
    preferences.candidateSurfaceColor = @"#203040";
    save();
    assert([preferences.globalTheme isEqual:@"custom"] && [preferences.customThemeBase isEqual:@"system"]);
    assert(preferences.customCandidateSkin == nil && [preferences.candidateSurfaceColor isEqual:@"#203040"]);
    // A package whose manifest base is system replaces a stored built-in base the same way.
    document = @{@"global_theme": @"night", @"custom_theme": @{@"base": @"night"}};
    [preferences applySharedCandidatePreferences:document];
    [preferences selectExternalSkin:@"wide-card" base:@"system"];
    save();
    assert([preferences.customThemeBase isEqual:@"system"] && [preferences.customCandidateSkin isEqual:@"wide-card"]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestCandidateSettingsTabs(MSIMEAppearancePreferences *preferences, NSUserDefaults *defaults, NSString *suite) {
    NSView *root = preferences.window.contentView;
    NSSegmentedControl *tabs = (id)FindControl(root, @"候选窗口设置标签");
    assert([tabs isKindOfClass:NSSegmentedControl.class] && tabs.segmentCount == 4 && tabs.selectedSegment == 0);
    NSArray<NSString *> *labels = @[@"布局", @"字体大小", @"窗口样式", @"预编辑"];
    NSArray<NSView *> *cards = @[FindControl(root, @"候选窗口卡片"), FindControl(root, @"候选字体卡片"),
        FindControl(root, @"候选窗口样式卡片"), FindControl(root, @"候选预编辑卡片")];
    MSIMECandidatePreviewView *preview = (id)MSIMEFindPreferenceViewOfClass(root, MSIMECandidatePreviewView.class);
    NSTextField *sample = (id)FindControl(root, @"预览示例文字");
    NSDictionary *before = [defaults persistentDomainForName:suite] ?: @{};
    NSDictionary *sharedBefore = [preferences sharedPreferencesByMerging:@{}];
    BOOL wasDark = preview.previewUsesDark;
    sample.stringValue = @"合成 示例";
    [NSApp sendAction:sample.action to:sample.target from:sample];
    __block NSUInteger notifications = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:preferences queue:nil usingBlock:^(NSNotification *note) { (void)note; ++notifications; }];
    for (NSInteger index = 0; index < 4; ++index) {
        assert([[tabs labelForSegment:index] isEqual:labels[index]]);
        tabs.selectedSegment = index;
        [NSApp sendAction:tabs.action to:tabs.target from:tabs];
        [root layoutSubtreeIfNeeded];
        for (NSInteger other = 0; other < 4; ++other) assert(cards[other].hidden == (other != index));
        assert(preview == MSIMEFindPreferenceViewOfClass(root, MSIMECandidatePreviewView.class));
        assert([sample.stringValue isEqual:@"合成 示例"] && preview.previewUsesDark == wasDark);
    }
    assert([before isEqual:([defaults persistentDomainForName:suite] ?: @{})]);
    // 在预编辑标签搜索另一标签的设置，结果要能打开它，而非滚到隐藏的控件。
    NSSearchField *search = nil;
    for (NSToolbarItem *item in preferences.window.toolbar.items)
        if ([item isKindOfClass:NSSearchToolbarItem.class]) search = ((NSSearchToolbarItem *)item).searchField;
    assert(search);
    search.stringValue = @"候选字号";
    [NSApp sendAction:search.action to:search.target from:search];
    NSButton *result = (id)MSIMEFindPreferenceControl(root, NSSelectorFromString(@"openSearchResult:"));
    assert(result && [result.accessibilityLabel containsString:@"候选字号"]);
    [NSApp sendAction:result.action to:result.target from:result];
    assert(tabs.selectedSegment == 1 && !cards[1].hidden && search.stringValue.length == 0);
    assert(notifications == 0 && [sharedBefore isEqual:[preferences sharedPreferencesByMerging:@{}]]);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    tabs.selectedSegment = 0;
    [NSApp sendAction:tabs.action to:tabs.target from:tabs];
    sample.stringValue = @"";
    [NSApp sendAction:sample.action to:sample.target from:sample];
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *suite = [@"app.msime.test.preview." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        char temporary[] = "/tmp/msime-preview-test-XXXXXX";
        assert(mkdtemp(temporary));
        const std::filesystem::path root(temporary);
        MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        TestCandidateSurfaceTheme(preferences);
        TestToolbarPreviewChrome(preferences);
        TestCustomThemeClearsSurviveSave();
        {
            NSDictionary *geometry = @{
                @"candidate_padding_horizontal": @3, @"candidate_padding_vertical": @11,
                @"candidate_item_padding_horizontal": @0, @"candidate_item_padding_vertical": @4,
                @"candidate_item_corner_radius": @0, @"candidate_item_spacing": @6,
            };
            [preferences applySharedCandidatePreferences:geometry];
            const auto style = [preferences candidateWindowStyle];
            assert(style.paddingHorizontal == 3 && style.paddingVertical == 11);
            assert(style.itemPaddingHorizontal == 0 && style.itemPaddingVertical == 4);
            assert(style.itemCornerRadius == 0 && style.itemSpacing == 6);
            NSDictionary *merged = [preferences sharedPreferencesByMerging:@{@"synthetic_unowned": @42}];
            for (NSString *key in geometry) assert([merged[key] isEqual:geometry[key]]);
            assert([merged[@"synthetic_unowned"] isEqual:@42]);
            [preferences applySharedCandidatePreferences:@{}];
            merged = [preferences sharedPreferencesByMerging:geometry];
            for (NSString *key in geometry) assert(merged[key] == NSNull.null);
        }
        // Match the Windows install default and the shared Tauri settings
        // fallback: a new macOS profile starts with word-to-character on for
        // the bracket key group. The bracket paging shortcut remains off by
        // default, so the two bindings are not ambiguous.
        NSDictionary *defaultWordCharacter = [preferences wordCharacterOptions];
        assert([defaultWordCharacter[@"enabled"] boolValue]);
        assert([defaultWordCharacter[@"keys"] isEqual:@"brackets"]);
        assert(![preferences navigationEnabled:@"brackets"]);
        NSDictionary *input = @{@"shuangpin_preedit_uses_raw": @NO, @"synthetic_unowned": @42};
        assert(preferences.shuangpinPreeditUsesRaw);
        assert([[preferences sharedPreferencesByMerging:input][@"shuangpin_preedit_uses_raw"] isEqual:@YES]);
        preferences.shuangpinPreeditUsesRaw = NO;
        NSDictionary *merged = [preferences sharedPreferencesByMerging:input];
        assert([merged[@"shuangpin_preedit_uses_raw"] isEqual:@NO]);
        assert([merged[@"synthetic_unowned"] isEqual:@42]);
        assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot] shuangpinPreeditUsesRaw]);
        preferences.shuangpinPreeditUsesRaw = YES;
        NSWindow *window = preferences.window;
        TestCandidateSettingsTabs(preferences, defaults, suite);
        MSIMECandidatePreviewView *preview =
            (id)MSIMEFindPreferenceViewOfClass(window.contentView, MSIMECandidatePreviewView.class);
        NSButton *theme = (id)MSIMEFindPreferenceControl(window.contentView, NSSelectorFromString(@"togglePreviewTheme:"));
        NSButton *showcase = (id)MSIMEFindPreferenceControl(window.contentView, NSSelectorFromString(@"togglePreviewShowcase:"));
        assert([preview isKindOfClass:MSIMECandidatePreviewView.class] && theme && showcase);
        // The preview sits on the appearance page, which scrolls because this host carries more
        // settings than fit the window.
        NSScrollView *appearanceScroll = preview.enclosingScrollView;
        NSView *appearancePage = appearanceScroll.documentView;
        [window.contentView layoutSubtreeIfNeeded];
        assert(!appearanceScroll.hasAmbiguousLayout && !preview.hasAmbiguousLayout && !appearancePage.hasAmbiguousLayout);
        assert(preview.frame.size.width > 500);
        assert(appearancePage.frame.size.height > appearanceScroll.contentView.bounds.size.height);
        NSPopUpButton *preedit = (id)MSIMEFindPreferenceControl(window.contentView, NSSelectorFromString(@"preeditChanged:"));
        assert(preedit);
        assert(preedit.indexOfSelectedItem == 1);
        [preedit selectItemAtIndex:0];
        [NSApp sendAction:preedit.action to:preedit.target from:preedit];
        assert(!preferences.shuangpinPreeditUsesRaw);
        assert([[preferences sharedPreferencesByMerging:input][@"shuangpin_preedit_uses_raw"] isEqual:@NO]);
        [preedit selectItemAtIndex:1];
        [NSApp sendAction:preedit.action to:preedit.target from:preedit];
        assert(preferences.shuangpinPreeditUsesRaw);
        assert([preview.accessibilityLabel isEqual:@"候选窗口预览"]);
        __block NSUInteger notifications = 0;
        id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:preferences queue:nil usingBlock:^(NSNotification *note) { (void)note; ++notifications; }];
        // 每个主题各跑完整矩阵；默认入口保留最后的自定义主题及后续交互状态。
        const BOOL themeMatrix = argc == 3 && strcmp(argv[1], "--theme-matrix") == 0;
        NSString *matrixSkin = themeMatrix ? @(argv[2]) : @"custom";
        assert(([@[@"system", @"shuishan", @"light", @"paper", @"night", @"ink", @"custom"] containsObject:matrixSkin]));
        for (NSString *skin in @[matrixSkin]) {
            preferences.globalTheme = skin;
            // The five built-in themes fix their mode; only 跟随系统 and 自定义 have another one to preview.
            const BOOL fixed = [skin isEqual:@"system"] || [skin isEqual:@"custom"] ? NO : YES;
            for (NSNumber *vertical in @[@NO, @YES]) {
                preferences.vertical = vertical.boolValue;
                for (NSNumber *size in @[@5, @7, @9]) {
                    preferences.pageSize = size.unsignedIntegerValue;
                    for (NSNumber *font in @[@12, @13, @16, @18, @20, @32]) {
                        preferences.fontSize = font.unsignedIntegerValue;
                        // 默认字体也参与几何；按实际字体及后备字形测量，避免继续断言旧系统字体的高度。
                        NSFont *candidateFont = [preferences candidateFontOfSize:font.doubleValue englishFirst:YES];
                        NSFont *preeditFont = [preferences candidateFontOfSize:preferences.preeditFontSize englishFirst:YES];
                        CGFloat preeditHeight = MAX(22.0, MSIMECandidateTextHeight(@"nihao", preeditFont) + 6.0);
                        CGFloat rowHeight = MSIMECandidateTextHeight(@"", candidateFont);
                        for (NSString *sample in @[@"水杉(Ss)", @"输入法(Sw)", @"你好(Nh)", @"世界(Sj)", @"中国(Zg)",
                                @"水仙(Sx)", @"水山(Ss)", @"水衫(Ss)", @"水善(Ss)"])
                            rowHeight = MAX(rowHeight, MSIMECandidateTextHeight(sample, candidateFont));
                        rowHeight += 8.0;
                        NSInteger rows = vertical.boolValue ? MIN(size.integerValue, 5) : 1;
                        CGFloat footerHeight = vertical.boolValue && size.integerValue > 5 ? 18.0 : 0.0;
                        // 卡片上下各留原生默认的 4pt 内边距。
                        CGFloat expectedHeight = 10 + 16 + 4 + 4 + preeditHeight + rows * rowHeight + footerHeight + 4 + 14;
                        assert(std::abs(preview.previewContentHeight - expectedHeight) < .01);
                        assert(preview.previewSkin.id == skin.UTF8String);
                        NSString *expected = [NSString stringWithFormat:@"%@，%@ 个候选，%@ pt", vertical.boolValue ? @"纵向列表" : @"横向排列", size, font];
                        assert([preview.accessibilityValue isEqual:expected]);
                        NSDictionary *before = [defaults persistentDomainForName:suite];
                        NSUInteger count = notifications;
                        BOOL originalTheme = preview.previewUsesDark;
                        Draw(preview);
                        assert(preview.previewSkin.fixedDark.has_value() == fixed && theme.hidden == fixed);
                        [NSApp sendAction:theme.action to:theme.target from:theme];
                        assert(preview.previewUsesDark == (fixed ? originalTheme : !originalTheme));
                        assert([theme.title isEqual:preview.forcedThemeButtonTitle]);
                        Draw(preview);
                        assert([[defaults persistentDomainForName:suite] isEqual:before] && notifications == count);
                    }
                }
            }
        }
        if (themeMatrix) {
            [NSNotificationCenter.defaultCenter removeObserver:observer];
            MSIMERemoveTestPreferenceSuite(defaults, suite);
            std::filesystem::remove_all(root);
            return 0;
        }
        // Real layout control updates the preview, without a separate preview-specific setting.
        NSPopUpButton *layout = (id)FindControl(window.contentView, @"候选排列");
        assert(layout);
        NSComboBox *familyControl = (id)FindControl(window.contentView, @"候选字体");
        assert(familyControl && familyControl.numberOfItems > 0);
        NSUInteger familyNotifications = notifications;
        [preferences applySharedCandidatePreferences:@{@"candidate_font_family": @"MSIME Synthetic Unavailable Family"}];
        assert(notifications == familyNotifications);
        assert([familyControl.stringValue isEqual:@"MSIME Synthetic Unavailable Family"]);
        assert([[preferences candidateFontOfSize:18].fontName isEqual:[NSFont systemFontOfSize:18].fontName]);
        assert([[preferences sharedPreferencesByMerging:@{}][@"candidate_font_family"] isEqual:familyControl.stringValue]);
        for (id invalid in @[@"", @YES, NSNull.null, [@"字" stringByPaddingToLength:43 withString:@"字" startingAtIndex:0]]) {
            [preferences applySharedCandidatePreferences:@{@"candidate_font_family": invalid}];
            assert([preferences.fontFamily isEqual:@"MSIME Synthetic Unavailable Family"]);
        }
        NSString *installedFamily = [NSFont fontWithName:@"Menlo" size:18].familyName;
        assert(installedFamily);
        familyControl.stringValue = installedFamily;
        [NSApp sendAction:familyControl.action to:familyControl.target from:familyControl];
        assert([[preferences candidateFontOfSize:18].familyName isEqual:installedFamily]);
        MSIMEAppearancePreferences *familyReloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert([familyReloaded.fontFamily isEqual:installedFamily]);
        NSDictionary *familyMerge = [preferences sharedPreferencesByMerging:@{@"candidate_fallback_fonts": @[@"Synthetic Supplementary"], @"synthetic_unowned": @42}];
        assert([familyMerge[@"candidate_font_family"] isEqual:installedFamily]);
        assert(([familyMerge[@"candidate_fallback_fonts"] isEqual:@[@"Synthetic Supplementary"]]));
        assert([familyMerge[@"synthetic_unowned"] isEqual:@42]);
        NSFont *measuredCandidate = [preferences candidateFontOfSize:preferences.fontSize];
        NSFont *measuredPreedit = [preferences candidateFontOfSize:preferences.preeditFontSize];
        CGFloat measuredRow = ceil(measuredCandidate.ascender - measuredCandidate.descender + measuredCandidate.leading) + 8;
        CGFloat measuredHeader = MAX(22.0, ceil(measuredPreedit.ascender - measuredPreedit.descender + measuredPreedit.leading) + 6);
        CGFloat familyHeight = 10 + 16 + 4 + 4 + measuredHeader + 5 * measuredRow + 18 + 4 + 14;
        assert(std::abs(preview.previewContentHeight - familyHeight) < .01);
        Draw(preview);
        familyControl.stringValue = @"";
        [NSApp sendAction:familyControl.action to:familyControl.target from:familyControl];
        assert([familyControl.stringValue isEqual:installedFamily]);
        preferences.fontFamily = @"Segoe UI";
        TestFallbackFonts(preferences, defaults);
        TestCloudImportCache(preferences, defaults);
        TestCandidateWindowStyle(preferences, defaults);
        [preferences applySharedCandidatePreferences:@{@"navigation": @{@"minus_equal": @NO, @"brackets": @YES, @"arrows": @NO}}];
        assert(![preferences navigationEnabled:@"minus_equal"] && [preferences navigationEnabled:@"brackets"] && ![preferences navigationEnabled:@"arrows"]);
        NSButton *arrowsControl = (id)FindControl(preferences.window.contentView, @"方向键选择候选");
        assert(arrowsControl && arrowsControl.state == NSControlStateValueOff);
        arrowsControl.state = NSControlStateValueOn;
        [NSApp sendAction:arrowsControl.action to:arrowsControl.target from:arrowsControl];
        assert([preferences navigationEnabled:@"arrows"]);
        NSDictionary *navigation = [preferences sharedPreferencesByMerging:@{}][@"navigation"];
        assert([navigation[@"arrows"] isEqual:@YES]);
        assert(navigation[@"comma_period"] == nil && navigation[@"tab"] == nil && navigation[@"page_up_down"] == nil);
        // The persistence worker merges captured edits into each fresh revision,
        // including CAS retries; it must not overwrite unowned navigation flags.
        NSDictionary *capturedNavigation = @{@"navigation": navigation};
        NSDictionary *latestNavigation = @{@"navigation": @{@"minus_equal": @NO, @"brackets": @YES,
            @"page_up_down": @NO, @"comma_period": @NO, @"tab": @NO, @"arrows": @NO}};
        NSDictionary *retryNavigation = MSIMEMergePreferenceSnapshot(latestNavigation, capturedNavigation)[@"navigation"];
        assert(retryNavigation.count == 6 && [retryNavigation[@"arrows"] isEqual:@YES]);
        assert([retryNavigation[@"comma_period"] isEqual:@NO] && [retryNavigation[@"tab"] isEqual:@NO]);
        assert([latestNavigation[@"navigation"][@"arrows"] isEqual:@NO]);
        [preferences setNavigation:@"arrows" enabled:NO];
        MSIMEAppearancePreferences *navigationReloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert(![navigationReloaded navigationEnabled:@"arrows"]);
        [preferences setNavigation:@"arrows" enabled:YES];
        // Tab / Shift-Tab 和 Page Up / Page Down 在 macOS 上固定开启：窗口里没有它们的复选框，共享文档里存的关和这里的设置都不起作用，合并时也不写回文档。
        assert(FindControl(preferences.window.contentView, @"Tab / Shift-Tab 翻页") == nil);
        assert(FindControl(preferences.window.contentView, @"Page Up / Page Down 翻页") == nil);
        [preferences applySharedCandidatePreferences:@{@"navigation": @{@"tab": @NO, @"page_up_down": @NO}}];
        [preferences setNavigation:@"tab" enabled:NO];
        [preferences setNavigation:@"page_up_down" enabled:NO];
        assert([preferences navigationEnabled:@"tab"] && [preferences navigationEnabled:@"page_up_down"]);
        NSDictionary *fixedNavigation = [preferences sharedPreferencesByMerging:@{}][@"navigation"];
        assert(fixedNavigation[@"tab"] == nil && fixedNavigation[@"page_up_down"] == nil);
        [preferences applySharedCandidatePreferences:@{@"word_character": @{@"enabled": @YES, @"keys": @"brackets"}, @"navigation": @{@"brackets": @NO}}];
        [preferences setNavigation:@"brackets" enabled:YES];
        assert(![preferences navigationEnabled:@"brackets"]);
        [preferences applySharedCandidatePreferences:@{@"word_character": @{@"enabled": @NO, @"keys": @"brackets"}}];
        preferences.pageShortcut = 0;
        NSButton *wordControl = (id)FindControl(preferences.window.contentView, @"以词定字");
        assert(wordControl);
        wordControl.state = NSControlStateValueOn;
        [NSApp sendAction:wordControl.action to:wordControl.target from:wordControl];
        assert([[preferences wordCharacterOptions][@"enabled"] boolValue]);
        MSIMEAppearancePreferences *wordReloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert([[wordReloaded wordCharacterOptions][@"enabled"] boolValue]);
        [wordReloaded setNavigation:@"brackets" enabled:YES];
        assert(![wordReloaded navigationEnabled:@"brackets"]);
        assert([preferences sharedPreferencesByMerging:@{}][@"word_character"] != nil);
        [preferences setWordCharacterEnabled:NO keys:@"brackets"];
        NSTextField *colorField = (id)FindControl(preferences.window.contentView, @"候选文字颜色");
        NSColorWell *colorWell = (id)FindControl(preferences.window.contentView, @"选择候选文字颜色");
        assert(colorField && colorWell);
        NSUInteger colorNotifications = notifications;
        // The pickers are the custom theme's candidate_colors now, and colour the window only while it is selected.
        [preferences applySharedCandidatePreferences:@{@"global_theme": @"custom", @"custom_theme": Pickers(@{@"text": @"#1234aB"})}];
        assert(notifications == colorNotifications && [colorField.stringValue isEqual:@"#1234aB"]);
        NSColor *custom = [NSColor colorWithSRGBRed:18/255.0 green:52/255.0 blue:171/255.0 alpha:1];
        assert([preview.previewTextColor isEqual:custom]);
        Draw(preview);
        for (id invalid in @[@"red", @"#123", @"#12345678", @"#GG0000", @YES]) {
            [preferences applySharedCandidatePreferences:@{@"custom_theme": Pickers(@{@"text": invalid})}];
            assert([preferences.candidateTextColor isEqual:@"#1234aB"]);
        }
        colorWell.color = [NSColor colorWithSRGBRed:1 green:0 blue:0 alpha:1];
        [NSApp sendAction:colorWell.action to:colorWell.target from:colorWell];
        assert([preferences.candidateTextColor isEqual:@"#FF0000"]);
        MSIMEAppearancePreferences *colorReloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert([colorReloaded.candidateTextColor isEqual:@"#FF0000"]);
        colorField.stringValue = @"#112233";
        [NSApp sendAction:colorField.action to:colorField.target from:colorField];
        assert([[preferences sharedPreferencesByMerging:@{}][@"custom_theme"][@"candidate_colors"][@"text"] isEqual:@"#112233"]);
        [preferences applySharedCandidatePreferences:@{}];
        assert(preferences.candidateTextColor == nil);
        [preferences applySharedCandidatePreferences:@{@"custom_theme": Pickers(@{@"text": @"#112233"})}];
        [preferences applySharedCandidatePreferences:@{@"custom_theme": Pickers(@{@"text": NSNull.null})}];
        assert(preferences.candidateTextColor == nil);
        NSDictionary *rowColors = @{@"text": @"#102030", @"number": @"#203040", @"accent": @"#304050", @"selected": @"#405060",
            @"hover": @"#506070", @"surface": @"#607080", @"border": @"#708090"};
        [preferences applySharedCandidatePreferences:@{@"global_theme": @"custom", @"custom_theme": Pickers(rowColors)}];
        NSArray *pickedColors = @[preferences.candidateTextColor, preferences.candidateAccentColor,
            preferences.candidateSelectedColor, preferences.candidateHoverColor, preferences.candidateSurfaceColor];
        NSArray *expectedColors = @[@"#102030", @"#304050", @"#405060", @"#506070", @"#607080"];
        assert([pickedColors isEqual:expectedColors]);
        {
            const msime::mac::SkinTokens tokens = [preferences resolvedSkinForDark:NO].tokens;
            const msime::mac::Rgba drawn[] = {tokens.text, tokens.accent, tokens.selected, tokens.hover, tokens.surface};
            for (NSUInteger index = 0; index < expectedColors.count; ++index) assert(TokenIs(drawn[index], expectedColors[index]));
            // 文档里的编号色和边框色在 macOS 上不读：编号色由文字颜色推出（约 62% 不透明），边框跟随主题，都不是文档里的值。
            assert(TokenIs(tokens.number, @"#102030", 0x9d / 255.0));
            assert(!TokenIs(tokens.border, @"#708090"));
        }
        // 合并时编号色和边框色照文档原样带过，本机不再写自己的值进去。
        NSDictionary *mergedColors = [preferences sharedPreferencesByMerging:@{@"custom_theme": Pickers(@{@"number": @"#0A0B0C"})}][@"custom_theme"][@"candidate_colors"];
        assert([mergedColors[@"number"] isEqual:@"#0A0B0C"] && mergedColors[@"border"] == nil);
        for (id invalid in @[@YES, @"red", @"#123", @"#12345678", @"#GG0000"])
            [preferences applySharedCandidatePreferences:@{@"custom_theme": Pickers(@{@"text": @"#102030", @"accent": invalid})}];
        assert([preferences.candidateAccentColor isEqual:@"#304050"]);
        [preferences applySharedCandidatePreferences:@{@"custom_theme": Pickers(@{@"text": @"#102030"})}];
        // 设了文字颜色时编号色也由它推出，约 62% 不透明；没设强调色时用原生的那个。
        assert(preferences.candidateAccentColor == nil);
        assert(TokenIs([preferences resolvedSkinForDark:NO].tokens.number, @"#102030", 0x9d / 255.0));
        assert(TokenIs([preferences resolvedSkinForDark:NO].tokens.accent, @"#2C7A4B"));
        preferences.candidateTextColor = @"#112233";
        [NSApp sendAction:NSSelectorFromString(@"resetTextColor:") to:preferences from:nil];
        assert([preferences sharedPreferencesByMerging:@{}][@"custom_theme"][@"candidate_colors"][@"text"] == NSNull.null);
        colorReloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert(colorReloaded.candidateTextColor == nil);
        preferences.fontFamily = @"Helvetica";
        preferences.fontSize = 32;
        NSFont *fallbackFont = [preferences candidateFontOfSize:32];
        CGFloat actualHeight = ceil([@"水杉(Ss)" sizeWithAttributes:@{NSFontAttributeName:fallbackFont}].height);
        assert(actualHeight > ceil(fallbackFont.ascender - fallbackFont.descender + fallbackFont.leading));
        NSFont *headerFont = [preferences candidateFontOfSize:preferences.preeditFontSize];
        CGFloat headerHeight = MAX(22.0, MAX(ceil([@"nihao" sizeWithAttributes:@{NSFontAttributeName:headerFont}].height), ceil(headerFont.ascender - headerFont.descender + headerFont.leading)) + 6);
        CGFloat fallbackPreviewHeight = 10 + 16 + 4 + 4 + headerHeight + 5 * (actualHeight + 8) + 18 + 4 + 14;
        assert(std::abs(preview.previewContentHeight - fallbackPreviewHeight) < .01);
        Draw(preview);
        preferences.fontFamily = @"Segoe UI";
        // Shared updates do not persist or notify; native controls own explicit edits.
        NSUInteger beforeShared = notifications;
        [preferences applySharedCandidatePreferences:@{@"candidate_preedit_font_size": @32, @"candidate_preedit_style": @"empty"}];
        assert(notifications == beforeShared && preferences.preeditFontSize == 32 && !preferences.showsCandidatePreedit);
        for (id invalid in @[@YES, @11, @33, @12.5,
                             [NSDecimalNumber decimalNumberWithString:@"31.0000000000000001"],
                             @"20", NSNull.null]) {
            [preferences applySharedCandidatePreferences:@{@"candidate_preedit_font_size": invalid, @"candidate_preedit_style": invalid}];
            assert(preferences.preeditFontSize == 32 && !preferences.showsCandidatePreedit);
        }
        NSPopUpButton *preeditSizeControl = (id)FindControl(window.contentView, @"候选窗拼音字号");
        NSPopUpButton *preeditStyleControl = (id)FindControl(window.contentView, @"候选窗预编辑");
        assert(preeditSizeControl && preeditStyleControl && preeditSizeControl.indexOfSelectedItem == 20 && preeditStyleControl.indexOfSelectedItem == 1);
        CGFloat hiddenHeight = preview.previewContentHeight;
        [preeditStyleControl selectItemAtIndex:0];
        [NSApp sendAction:preeditStyleControl.action to:preeditStyleControl.target from:preeditStyleControl];
        assert(preview.previewContentHeight > hiddenHeight);
        Draw(preview);
        CGFloat largeHeight = preview.previewContentHeight;
        [preeditSizeControl selectItemAtIndex:0];
        [NSApp sendAction:preeditSizeControl.action to:preeditSizeControl.target from:preeditSizeControl];
        assert(preview.previewContentHeight < largeHeight && preferences.preeditFontSize == 12);
        Draw(preview);
        NSDictionary *preeditMerged = [preferences sharedPreferencesByMerging:@{@"synthetic_unowned": @42}];
        assert([preeditMerged[@"candidate_preedit_font_size"] isEqual:@12]);
        assert([preeditMerged[@"candidate_preedit_style"] isEqual:@"pinyin"] && [preeditMerged[@"synthetic_unowned"] isEqual:@42]);
        MSIMEAppearancePreferences *reloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert(reloaded.preeditFontSize == 12 && reloaded.showsCandidatePreedit);
        // Showcase uses compact fonts; compare the layouts at the standard size.
        preferences.fontSize = 18;
        [layout selectItemAtIndex:0];
        [NSApp sendAction:layout.action to:layout.target from:layout];
        CGFloat horizontal = preview.previewContentHeight;
        [layout selectItemAtIndex:1];
        [NSApp sendAction:layout.action to:layout.target from:layout];
        assert(preview.previewContentHeight > horizontal);
        CGFloat verticalHeight = preview.previewContentHeight;
        NSDictionary *before = [defaults persistentDomainForName:suite];
        NSUInteger count = notifications;
        showcase.state = NSControlStateValueOn;
        [NSApp sendAction:showcase.action to:showcase.target from:showcase];
        assert(preview.previewContentHeight > verticalHeight);
        Draw(preview);
        assert([[defaults persistentDomainForName:suite] isEqual:before] && notifications == count);
        // A fresh preview follows effective appearance until its local theme is overridden.
        MSIMECandidatePreviewView *automatic = [[MSIMECandidatePreviewView alloc] initWithFrame:NSMakeRect(0, 0, 580, 190)];
        automatic.preferences = preferences;
        automatic.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
        assert(automatic.previewUsesDark);
        automatic.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
        assert(!automatic.previewUsesDark);
        [automatic toggleForcedTheme];
        assert(automatic.previewUsesDark);
        automatic.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
        assert(automatic.previewUsesDark);
        NSString *selectedTheme = preferences.globalTheme;
        [automatic setPreviewSkinId:@"shuishan"];
        assert(automatic.previewSkin.id == "shuishan" && [preferences.globalTheme isEqual:selectedTheme]);
        // A theme with a fixed mode is previewed in it, whatever the local override says.
        assert(automatic.previewUsesDark && automatic.previewSkin.fixedDark == true);
        [automatic setPreviewSkinId:@"missing-package"];
        assert(automatic.previewSkin.id == "custom" && automatic.previewSkin.candidateSkin.empty());
        // 包只画在它 base 的明暗下：深色底的包卡片在浅色宿主里也按深色预览出包本身，浅色底的同理，不落回没有包的底色。
        for (NSArray<NSString *> *fixture in @[ @[ @"night-card", @"night", @"dark" ], @[ @"paper-card", @"paper", @"light" ] ]) {
            std::filesystem::create_directories(root / fixture[0].UTF8String);
            std::ofstream manifest(root / fixture[0].UTF8String / "skin.toml");
            manifest << "schema_version = 1\nid = \"" << fixture[0].UTF8String << "\"\nname = \"" << fixture[0].UTF8String
                     << "\"\nversion = \"1\"\nbase = \"" << fixture[1].UTF8String << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\""
                     << fixture[2].UTF8String << "\"]\n[candidate_window]\nmin_width_dip = 0\n[candidate." << fixture[2].UTF8String << "]\nsurface = \"#345678\"\n";
            assert(manifest.good());
        }
        for (NSAppearanceName host in @[ NSAppearanceNameAqua, NSAppearanceNameDarkAqua ]) {
            automatic.appearance = [NSAppearance appearanceNamed:host];
            [automatic setPreviewSkinId:@"night-card"];
            assert(automatic.previewSkin.candidateSkin == "night-card" && automatic.previewUsesDark && [automatic previewHasFixedMode]);
            [automatic setPreviewSkinId:@"paper-card"];
            assert(automatic.previewSkin.candidateSkin == "paper-card" && !automatic.previewUsesDark && [automatic previewHasFixedMode]);
        }
        std::filesystem::remove_all(root / "night-card");
        std::filesystem::remove_all(root / "paper-card");
        // A large external decoration increases scrollable height, not the fixed settings window.
        std::filesystem::create_directory(root / "synthetic");
        {
            std::ofstream manifest(root / "synthetic" / "skin.toml");
            manifest << R"toml(schema_version = 1
id = "synthetic"
name = "Synthetic"
version = "1"
base = "system"
preview = "image.png"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]
[candidate_window]
min_width_dip = 200
[candidate_window.decoration]
top_inset_dip = 180
width_dip = 120
[candidate.light]
surface = "#fff7fa"
[candidate.dark]
surface = "#123456"
)toml";
            assert(manifest.good());
        }
        NSBitmapImageRep *decoration = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:4 pixelsHigh:4 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSCalibratedRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
        for (NSInteger y = 0; y < 4; ++y) for (NSInteger x = 0; x < 4; ++x) {
            unsigned char *pixel = decoration.bitmapData + y * decoration.bytesPerRow + x * 4;
            pixel[0] = 255; pixel[1] = 0; pixel[2] = 0; pixel[3] = 255;
        }
        decoration = [decoration bitmapImageRepByRetaggingWithColorSpace:NSColorSpace.sRGBColorSpace];
        assert(decoration);
        assert([[decoration representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:@((root / "synthetic" / "image.png").c_str()) atomically:YES]);
        [preferences reloadSkins];
        [preferences selectExternalSkin:@"synthetic" base:@"system"];
        assert(preview.previewSkin.candidateSkin == "synthetic" && preview.previewSkin.decorationTopDip == 180);
        NSBitmapImageRep *withDecoration = Draw(preview);
        const CGFloat scale = withDecoration.pixelsWide / preview.bounds.size.width;
        if (argc == 2) assert([[withDecoration representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:@(argv[1]) atomically:YES]);
        // The square image keeps its aspect at 120pt wide, so it fills the lower part of the 180pt band down to 6pt over the card (y 96-216 below the panel's top at 30).
        NSColor *red = [[withDecoration colorAtX:(preview.bounds.size.width - 60) * scale y:150 * scale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        // ColorSync may convert the fixture through the display profile; test visible red,
        // not byte identity between an image profile and the window's backing color space.
        assert(red.alphaComponent > .99 && red.redComponent > .8 &&
               red.redComponent - red.greenComponent > .5 && red.redComponent - red.blueComponent > .5);
        {
            // The msime-skins keys: decoration.image over the preview, left alignment, the card radius, and a background drawn under the rows and clipped to the card; the toolbar keeps its own radius and colours.
            std::ofstream manifest(root / "synthetic" / "skin.toml");
            manifest << R"toml(schema_version = 1
id = "synthetic"
name = "Synthetic"
version = "1"
base = "system"
preview = "image.png"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]
[candidate_window]
min_width_dip = 200
corner_radius_dip = 24
[candidate_window.decoration]
image = "mascot.png"
top_inset_dip = 180
width_dip = 120
align = "left"
[candidate_window.background]
image = "background.png"
fit = "cover"
[candidate.light]
surface = "#fff7fa"
[candidate.dark]
surface = "#123456"
[toolbar]
corner_radius_dip = 6
[toolbar.light]
background = "#FF00FF"
[toolbar.dark]
background = "#FF00FF"
)toml";
            assert(manifest.good());
        }
        for (const auto &[file, rgb] : {std::pair<const char *, unsigned>{"mascot.png", 0x00FF00}, {"background.png", 0x0000FF}}) {
            NSBitmapImageRep *fixture = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:4 pixelsHigh:4 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSCalibratedRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
            for (NSInteger y = 0; y < 4; ++y) for (NSInteger x = 0; x < 4; ++x) {
                unsigned char *pixel = fixture.bitmapData + y * fixture.bytesPerRow + x * 4;
                pixel[0] = (rgb >> 16) & 255; pixel[1] = (rgb >> 8) & 255; pixel[2] = rgb & 255; pixel[3] = 255;
            }
            fixture = [fixture bitmapImageRepByRetaggingWithColorSpace:NSColorSpace.sRGBColorSpace];
            assert([[fixture representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:@((root / "synthetic" / file).c_str()) atomically:YES]);
        }
        [preferences reloadSkins];
        const auto styledSkin = preview.previewSkin;
        assert(styledSkin.candidateSkin == "synthetic" && styledSkin.tokens.radius == 24.0f &&
               styledSkin.decorationPath.find("mascot.png") != std::string::npos &&
               styledSkin.decorationAlign == msime::mac::DecorationAlign::left && !styledSkin.backgroundPath.empty());
        NSBitmapImageRep *styledBitmap = Draw(preview);
        const CGFloat styledScale = styledBitmap.pixelsWide / preview.bounds.size.width;
        // The 横排候选 panel starts 30pt down at the 14pt inset: the transparent decoration band, then the card at 210. The left-aligned mascot is 6pt in from the card's edge and spans y 96-216, over the card's top edge.
        NSColor *mascot = [[styledBitmap colorAtX:(14 + 60) * styledScale y:150 * styledScale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        // Visible green, allowing for the ColorSync conversion noted above (a pure green fixture lands near 0.5, 0.97, 0.37 on a wide-gamut display).
        assert(mascot.greenComponent > .8 && mascot.greenComponent - mascot.redComponent > .3 && mascot.greenComponent - mascot.blueComponent > .3);
        NSColor *rightOfBand = [[styledBitmap colorAtX:(preview.bounds.size.width - 60) * styledScale y:80 * styledScale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(!(rightOfBand.greenComponent - rightOfBand.redComponent > .3));
        const CGFloat cardTop = 30 + 180;
        NSColor *overCard = [[styledBitmap colorAtX:(14 + 60) * styledScale y:(cardTop + 3) * styledScale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(overCard.greenComponent > .8 && overCard.greenComponent - overCard.blueComponent > .3);
        NSColor *aboveMascot = [[styledBitmap colorAtX:(14 + 60) * styledScale y:60 * styledScale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(!(aboveMascot.greenComponent - aboveMascot.redComponent > .3) && !(aboveMascot.blueComponent - aboveMascot.redComponent > .5));
        NSColor *background = [[styledBitmap colorAtX:(preview.bounds.size.width / 2) * styledScale y:(cardTop + 3) * styledScale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(background.blueComponent > .8 && background.blueComponent - background.redComponent > .5);
        // Clipped to the 24pt corner: the card's own top-left pixel is the canvas, not the image.
        NSColor *clipped = [[styledBitmap colorAtX:(14 + 1) * styledScale y:(cardTop + 1) * styledScale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(!(clipped.blueComponent - clipped.redComponent > .5));
        const auto previewToolbar = [preferences toolbarSkinForDark:preview.previewUsesDark];
        assert(previewToolbar.radius == 6.0f && std::abs(previewToolbar.surface.r - 1.0f) < .002f && std::abs(previewToolbar.surface.g) < .002f);
        MSIMEToolbarPreviewView *styledToolbar = [[MSIMEToolbarPreviewView alloc] initWithFrame:NSMakeRect(0, 0, 580, 100)];
        styledToolbar.preferences = preferences;
        styledToolbar.frame = NSMakeRect(0, 0, 580, styledToolbar.fittingSize.height);
        NSBitmapImageRep *toolbarBitmap = [styledToolbar bitmapImageRepForCachingDisplayInRect:styledToolbar.bounds];
        [styledToolbar cacheDisplayInRect:styledToolbar.bounds toBitmapImageRep:toolbarBitmap];
        const CGFloat toolbarPixels = toolbarBitmap.pixelsWide / NSWidth(styledToolbar.bounds);
        const CGFloat toolbarScale = preferences.floatingToolbarScalePercent / 100.0;
        NSColor *toolbarSurface = [[toolbarBitmap colorAtX:(NSInteger)((14.0 + 39.5 * toolbarScale) * toolbarPixels) y:(NSInteger)((34.0 + NSHeight(styledToolbar.bounds) - 14.0) / 2.0 * toolbarPixels)] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(toolbarSurface.redComponent > .8 && toolbarSurface.blueComponent > .8 && toolbarSurface.greenComponent < .3);
        [window.contentView layoutSubtreeIfNeeded];
        // Showcase mode grows the preview; reaching its bottom scrolls the appearance page that
        // now carries it, rather than a scroll view wrapped around the preview alone.
        assert(appearancePage.frame.size.height > appearanceScroll.contentView.bounds.size.height);
        [preview scrollRectToVisible:NSMakeRect(0, preview.frame.size.height - 20, 100, 20)];
        assert(appearanceScroll.contentView.bounds.origin.y > 0);
        [preview setShowsLayoutShowcase:NO];
        if (argc == 2) {
            if (!preview.previewUsesDark) [preview toggleForcedTheme];
            assert([[Draw(preview) representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:@(argv[1]) atomically:YES]);
        }
        [NSNotificationCenter.defaultCenter removeObserver:observer];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        std::filesystem::remove_all(root);
    }
}
