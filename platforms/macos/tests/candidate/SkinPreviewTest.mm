#import "../../src/settings/AppearancePreferences.h"
#import "../../src/candidate/CandidateSkinPreviewView.h"
#import "../../src/cloud/CloudAppearanceSettings.h"
#import "../../src/settings/PreferenceSnapshotMerge.h"
#import <CoreText/CoreText.h>
#include <cassert>
#include <fstream>
#import "../settings/TestPreferenceSuite.h"
#import "../settings/PreferenceViewLookup.h"

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
    NSComboBox *entry = (id)FindControl(preferences.window.contentView, @"添加补充字体");
    NSTableView *list = (id)FindControl(preferences.window.contentView, @"补充字体顺序");
    assert(entry && list);
    NSString *sans = [NSFont fontWithName:@"PingFangSC-Regular" size:18].familyName;
    NSString *serif = [NSFont fontWithName:@"STSongti-SC-Regular" size:18].familyName;
    assert(sans && serif);
    __block NSUInteger notifications = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:preferences queue:nil usingBlock:^(NSNotification *note) { (void)note; ++notifications; }];
    [preferences applySharedCandidatePreferences:@{@"candidate_font_family": @"Menlo", @"candidate_english_font": @"Helvetica", @"candidate_fallback_fonts": @[sans, serif]}];
    assert(notifications == 0 && list.numberOfRows == 2);
    assert([preferences.candidateEnglishFont isEqual:@"Helvetica"]);
    NSDictionary *fontMerge = [preferences sharedPreferencesByMerging:@{}];
    assert([fontMerge[@"candidate_english_font"] isEqual:@"Helvetica"]);
    assert([RenderedFamilyForText([preferences candidateFontOfSize:18 englishFirst:YES], @"Latin") isEqual:@"Helvetica"]);
    [preferences applySharedCandidatePreferences:@{}];
    assert(!preferences.candidateEnglishFont);
    [preferences applySharedCandidatePreferences:@{@"candidate_english_font": @"Helvetica"}];
    assert([RenderedFamily([preferences candidateFontOfSize:18]) isEqual:sans]);
    [list selectRowIndexes:[NSIndexSet indexSetWithIndex:1] byExtendingSelection:NO];
    [NSApp sendAction:NSSelectorFromString(@"moveFallbackFontUp:") to:preferences from:nil];
    assert([preferences.fallbackFonts.firstObject isEqual:serif]);
    assert([RenderedFamily([preferences candidateFontOfSize:18]) isEqual:serif]);
    [NSApp sendAction:NSSelectorFromString(@"moveFallbackFontDown:") to:preferences from:nil];
    assert([preferences.fallbackFonts.firstObject isEqual:sans]);
    entry.stringValue = @"MSIME Synthetic Unavailable Supplement";
    [NSApp sendAction:NSSelectorFromString(@"addFallbackFont:") to:preferences from:entry];
    assert(preferences.fallbackFonts.count == 3 && list.selectedRow == 2);
    [NSApp sendAction:NSSelectorFromString(@"removeFallbackFont:") to:preferences from:nil];
    assert(preferences.fallbackFonts.count == 2);
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
    assert(preferences.fallbackFonts.count == 32 && list.numberOfRows == 32);
    [limit addObject:serif];
    preferences.fallbackFonts = limit;
    assert(preferences.fallbackFonts.count == 32);
    NSUInteger countAtLimit = notifications;
    entry.stringValue = serif;
    [NSApp sendAction:NSSelectorFromString(@"addFallbackFont:") to:preferences from:entry];
    assert(preferences.fallbackFonts.count == 32 && notifications == countAtLimit);
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
    [preferences applySharedAssistancePreferences:@{@"autocorrect": @NO, @"quanpin": @{@"autocorrect_neighbor": @NO}}];
    [preferences applySharedToolbarVisibility:NO];
    assert(!preferences.chinesePunctuation && !preferences.autocorrect && !preferences.shuangpinPreeditUsesRaw && !preferences.floatingToolbarEnabled);
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
    for (NSString *key in @[@"autocorrect", @"chinese_punctuation", @"shuangpin_preedit_uses_raw", @"floating_toolbar"])
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
    assert(preferences.shuangpinPreeditUsesRaw && preferences.autocorrect && preferences.chinesePunctuation && preferences.floatingToolbarEnabled);
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
    for (NSString *skin in @[@"shuishan", @"paper"]) {
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
        NSColor *divider = [[bitmap colorAtX:(NSInteger)((14.0 + 41.6 * scale) * pixels) y:(NSInteger)(y * pixels)] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        NSColor *surface = [[bitmap colorAtX:(NSInteger)((14.0 + 39.5 * scale) * pixels) y:(NSInteger)(y * pixels)] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        const CGFloat alpha = tokens.border.a;
        assert(std::abs(divider.redComponent - (tokens.border.r * alpha + surface.redComponent * (1.0 - alpha))) < .03);
        assert(std::abs(divider.greenComponent - (tokens.border.g * alpha + surface.greenComponent * (1.0 - alpha))) < .03);
        assert(std::abs(divider.blueComponent - (tokens.border.b * alpha + surface.blueComponent * (1.0 - alpha))) < .03);
    }
    preferences.globalTheme = globalTheme;
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
    preferences.candidateNumberColor = @"#203040";
    save();
    assert([preferences.globalTheme isEqual:@"custom"] && [preferences.customThemeBase isEqual:@"system"]);
    assert(preferences.customCandidateSkin == nil && [preferences.candidateNumberColor isEqual:@"#203040"]);
    // A package whose manifest base is system replaces a stored built-in base the same way.
    document = @{@"global_theme": @"night", @"custom_theme": @{@"base": @"night"}};
    [preferences applySharedCandidatePreferences:document];
    [preferences selectExternalSkin:@"wide-card" base:@"system"];
    save();
    assert([preferences.customThemeBase isEqual:@"system"] && [preferences.customCandidateSkin isEqual:@"wide-card"]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
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
        for (NSString *skin in @[@"system", @"shuishan", @"light", @"paper", @"night", @"ink", @"custom"]) {
            preferences.globalTheme = skin;
            // The five built-in themes fix their mode; only 跟随系统 and 自定义 have another one to preview.
            const BOOL fixed = [skin isEqual:@"system"] || [skin isEqual:@"custom"] ? NO : YES;
            for (NSNumber *vertical in @[@NO, @YES]) {
                preferences.vertical = vertical.boolValue;
                for (NSNumber *size in @[@5, @7, @9]) {
                    preferences.pageSize = size.unsignedIntegerValue;
                    for (NSNumber *font in @[@12, @13, @16, @18, @20, @32]) {
                        preferences.fontSize = font.unsignedIntegerValue;
                        NSFont *candidateFont = [NSFont systemFontOfSize:font.doubleValue];
                        NSFont *preeditFont = [NSFont systemFontOfSize:preferences.preeditFontSize];
                        CGFloat preeditHeight = MAX(22.0, ceil(preeditFont.ascender - preeditFont.descender + preeditFont.leading) + 6.0);
                        CGFloat rowHeight = ceil(candidateFont.ascender - candidateFont.descender + candidateFont.leading) + 8.0;
                        NSInteger rows = vertical.boolValue ? MIN(size.integerValue, 5) : 1;
                        CGFloat footerHeight = vertical.boolValue && size.integerValue > 5 ? 18.0 : 0.0;
                        CGFloat expectedHeight = 10 + 16 + 4 + 6 + preeditHeight + rows * rowHeight + footerHeight + 6 + 14;
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
        CGFloat familyHeight = 10 + 16 + 4 + 6 + measuredHeader + 5 * measuredRow + 18 + 6 + 14;
        assert(std::abs(preview.previewContentHeight - familyHeight) < .01);
        Draw(preview);
        familyControl.stringValue = @"";
        [NSApp sendAction:familyControl.action to:familyControl.target from:familyControl];
        assert([familyControl.stringValue isEqual:installedFamily]);
        preferences.fontFamily = @"Segoe UI";
        TestFallbackFonts(preferences, defaults);
        TestCloudImportCache(preferences, defaults);
        [preferences applySharedCandidatePreferences:@{@"navigation": @{@"minus_equal": @NO, @"brackets": @YES, @"tab": @NO}}];
        assert(![preferences navigationEnabled:@"minus_equal"] && [preferences navigationEnabled:@"brackets"] && ![preferences navigationEnabled:@"tab"]);
        NSButton *tabControl = (id)FindControl(preferences.window.contentView, @"Tab / Shift-Tab 翻页");
        assert(tabControl && tabControl.state == NSControlStateValueOff);
        tabControl.state = NSControlStateValueOn;
        [NSApp sendAction:tabControl.action to:tabControl.target from:tabControl];
        assert([preferences navigationEnabled:@"tab"]);
        NSDictionary *navigation = [preferences sharedPreferencesByMerging:@{}][@"navigation"];
        assert([navigation[@"tab"] isEqual:@YES]);
        assert(navigation[@"comma_period"] == nil && navigation[@"arrows"] == nil);
        // The persistence worker merges captured edits into each fresh revision,
        // including CAS retries; it must not overwrite unowned navigation flags.
        NSDictionary *capturedNavigation = @{@"navigation": navigation};
        NSDictionary *latestNavigation = @{@"navigation": @{@"minus_equal": @NO, @"brackets": @YES,
            @"page_up_down": @NO, @"comma_period": @NO, @"tab": @NO, @"arrows": @NO}};
        NSDictionary *retryNavigation = MSIMEMergePreferenceSnapshot(latestNavigation, capturedNavigation)[@"navigation"];
        assert(retryNavigation.count == 6 && [retryNavigation[@"tab"] isEqual:@YES]);
        assert([retryNavigation[@"comma_period"] isEqual:@NO] && [retryNavigation[@"arrows"] isEqual:@NO]);
        assert([latestNavigation[@"navigation"][@"tab"] isEqual:@NO]);
        [preferences setNavigation:@"tab" enabled:NO];
        MSIMEAppearancePreferences *navigationReloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot];
        assert(![navigationReloaded navigationEnabled:@"tab"]);
        [preferences setNavigation:@"tab" enabled:YES];
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
        NSArray *pickedColors = @[preferences.candidateTextColor, preferences.candidateNumberColor, preferences.candidateAccentColor,
            preferences.candidateSelectedColor, preferences.candidateHoverColor, preferences.candidateSurfaceColor,
            preferences.candidateBorderColor];
        NSArray *expectedColors = @[@"#102030", @"#203040", @"#304050", @"#405060", @"#506070", @"#607080", @"#708090"];
        assert([pickedColors isEqual:expectedColors]);
        {
            const msime::mac::SkinTokens tokens = [preferences resolvedSkinForDark:NO].tokens;
            const msime::mac::Rgba drawn[] = {tokens.text, tokens.number, tokens.accent, tokens.selected, tokens.hover, tokens.surface, tokens.border};
            for (NSUInteger index = 0; index < expectedColors.count; ++index) assert(TokenIs(drawn[index], expectedColors[index]));
        }
        for (id invalid in @[@YES, @"red", @"#123", @"#12345678", @"#GG0000"])
            [preferences applySharedCandidatePreferences:@{@"custom_theme": Pickers(@{@"text": @"#102030", @"number": invalid})}];
        assert([preferences.candidateNumberColor isEqual:@"#203040"]);
        [preferences applySharedCandidatePreferences:@{@"custom_theme": Pickers(@{@"text": @"#102030"})}];
        // An explicit text colour also sets the numbers when the number picker is unset, about 62% opaque; an unset accent is the native one.
        assert(preferences.candidateNumberColor == nil && preferences.candidateAccentColor == nil);
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
        CGFloat fallbackPreviewHeight = 10 + 16 + 4 + 6 + headerHeight + 5 * (actualHeight + 8) + 18 + 6 + 14;
        assert(std::abs(preview.previewContentHeight - fallbackPreviewHeight) < .01);
        Draw(preview);
        preferences.fontFamily = @"Segoe UI";
        // Shared updates do not persist or notify; native controls own explicit edits.
        NSUInteger beforeShared = notifications;
        [preferences applySharedCandidatePreferences:@{@"candidate_preedit_font_size": @32, @"candidate_preedit_style": @"empty"}];
        assert(notifications == beforeShared && preferences.preeditFontSize == 32 && !preferences.showsCandidatePreedit);
        for (id invalid in @[@YES, @11, @33, @12.5, @"20", NSNull.null]) {
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
        NSColor *red = [[withDecoration colorAtX:(preview.bounds.size.width - 60) * scale y:80 * scale] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        // ColorSync may convert the fixture through the display profile; test visible red,
        // not byte identity between an image profile and the window's backing color space.
        assert(red.alphaComponent > .99 && red.redComponent > .8 &&
               red.redComponent - red.greenComponent > .5 && red.redComponent - red.blueComponent > .5);
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
