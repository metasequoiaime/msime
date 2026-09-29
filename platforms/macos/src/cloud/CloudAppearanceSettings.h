#pragma once
#import <Foundation/Foundation.h>
#include "../candidate/CandidateSkin.h"
#include "../candidate/CandidatePageSize.h"

static inline BOOL MSIMECloudAppearanceIntegerInRange(id value, NSInteger minimum, NSInteger maximum) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
           [value doubleValue] == [value integerValue] && [value integerValue] >= minimum && [value integerValue] <= maximum;
}

// The whole range the shared preferences accept, not the three sizes this platform's window used to
// offer: a cloud snapshot written by any other host carries the size that host allowed, and rejecting
// it here would drop the user's appearance on the way in.
static inline BOOL MSIMECloudAppearanceCandidatePageSize(id value) {
    return MSIMECloudAppearanceIntegerInRange(value, (NSInteger)msime::mac::kMinimumCandidatePageSize,
                                              (NSInteger)msime::mac::kMaximumCandidatePageSize);
}

static inline NSArray<NSString *> *MSIMECloudHelpcodeSchemas() {
    return @[@"lantian", @"ziranma", @"shouyou2_0", @"shouyouplus", @"xiaohe", @"jiajia"];
}

static inline NSInteger MSIMECloudHelpcodeSchemaIndex(NSUserDefaults *defaults, NSString *scheme) {
    NSString *fallback = [scheme isEqual:@"shuangpin"] ? @"lantian" : @"ziranma";
    NSDictionary *all = [defaults dictionaryForKey:@"MSIMEClientHelpcodeOptions"];
    NSDictionary *stored = [all isKindOfClass:NSDictionary.class] && [all[scheme] isKindOfClass:NSDictionary.class] ? all[scheme] : nil;
    NSString *schema = [stored[@"schema"] isKindOfClass:NSString.class] ? stored[@"schema"] : fallback;
    NSUInteger index = [MSIMECloudHelpcodeSchemas() indexOfObject:schema];
    return index == NSNotFound ? (NSInteger)[MSIMECloudHelpcodeSchemas() indexOfObject:fallback] : (NSInteger)index;
}

static inline NSArray<NSString *> *MSIMECloudLocalModeKeys() {
    return @[@"quick_phrase", @"date_time", @"unicode", @"emoji", @"kaomoji", @"super_jianpin", @"temporary_english", @"temporary_japanese"];
}

static inline BOOL MSIMECloudLocalInputModesEnabled(NSUserDefaults *defaults) {
    NSDictionary *stored = [defaults dictionaryForKey:@"MSIMEClientLocalModes"];
    if (![stored isKindOfClass:NSDictionary.class]) return YES;
    for (NSString *key in MSIMECloudLocalModeKeys()) {
        id value = stored[key];
        if ([value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() && ![value boolValue]) return NO;
    }
    return YES;
}

// Cloud names follow MSIME-Apple develop 2b0250f4dd7012520392b310dfcc0288c3208a75.
// Defaults and storage keys follow the active client host, not the retained Apple host.
static inline NSDictionary *MSIMECloudBooleanPreferences() {
    return @{@"autocorrect": @[@"MSIMEClientAutocorrect", @YES],
             @"helpcode": @[@"MSIMEClientHelpcodeEnabled", @YES],
             @"chinese_punctuation": @[@"MSIMEClientChinesePunctuation", @YES],
             @"smart_punctuation": @[@"MSIMEClientSmartPunctuation", @NO],
             @"smart_punctuation_repeat": @[@"MSIMEClientSmartPunctuationRepeatToChinese", @NO],
             @"english_input_mode": @[@"MSIMEClientEnglishInputMode", @NO],
             @"input_mode_shortcut": @[@"MSIMEClientInputModeShortcut", @YES],
             @"full_width_input": @[@"MSIMEClientFullWidthInput", @NO],
             @"floating_toolbar": @[@"MSIMEClientFloatingToolbarEnabled", @YES],
             @"traditional_chinese_output": @[@"MSIMEClientTraditionalOutput", @NO],
             @"wubi_auto_commit_unique": @[@"MSIMEClientWubiAutoCommitUnique", @NO],
             @"candidate_learning": @[@"MSIMEClientCandidateLearning", @YES],
             @"shuangpin_keymap": @[@"MSIMEClientShuangpinKeymap", @NO],
             @"shuangpin_preedit_uses_raw": @[@"MSIMEClientShuangpinPreeditUsesRaw", @YES]};
}

// The global theme replaced the per-host candidate skin: `global_theme` is one of the catalog ids, and the custom theme's base and package travel beside it. The package is "" for none.
static inline BOOL MSIMECloudCustomCandidateSkin(id value) {
    if (![value isKindOfClass:NSString.class] || [value length] > 64) return NO;
    return [value length] == 0 || (msime::mac::IsSafeSkinId([value UTF8String]) && !msime::mac::IsGlobalThemeId([value UTF8String]));
}

static inline NSDictionary *MSIMECloudAppearanceSnapshot(NSUserDefaults *defaults) {
    id font = [defaults objectForKey:@"MSIMEClientCandidateFontSize"];
    id page = [defaults objectForKey:@"MSIMEClientCandidatePageSize"];
    NSString *theme = [defaults stringForKey:@"MSIMEClientGlobalTheme"];
    NSString *base = [defaults stringForKey:@"MSIMEClientCustomThemeBase"];
    NSString *package = [defaults stringForKey:@"MSIMEClientCustomCandidateSkin"];
    NSMutableDictionary *snapshot = [@{@"platform.macos.global_theme": msime::mac::IsGlobalThemeId(theme.UTF8String ?: "") ? theme : @"system",
             @"platform.macos.custom_theme_base": msime::mac::IsThemeBaseId(base.UTF8String ?: "") ? base : @"system",
             @"platform.macos.custom_candidate_skin": MSIMECloudCustomCandidateSkin(package) ? package : @"",
             @"platform.macos.candidate_panel_style": @([defaults integerForKey:@"MSIMEClientCandidatePanelStyle"] == 1 ? 1 : 0),
             @"platform.macos.candidate_font_size": MSIMECloudAppearanceIntegerInRange(font, 12, 32) ? font : @18,
             @"platform.macos.candidate_page_size": MSIMECloudAppearanceCandidatePageSize(page) ? page : @9} mutableCopy];
    NSInteger shortcut = [defaults integerForKey:@"MSIMEClientCandidatePageShortcut"];
    snapshot[@"platform.macos.candidate_page_shortcut"] = @(shortcut == 1 || shortcut == 2 ? shortcut : 0);
    NSArray *schemes = @[@"quanpin", @"shuangpin", @"wubi"];
    NSUInteger scheme = [schemes indexOfObject:[defaults stringForKey:@"MSIMEClientInputScheme"] ?: @""];
    snapshot[@"platform.macos.input_scheme"] = @(scheme == NSNotFound ? 0 : scheme);
    snapshot[@"platform.macos.quanpin_helpcode_schema"] = @(MSIMECloudHelpcodeSchemaIndex(defaults, @"quanpin"));
    snapshot[@"platform.macos.shuangpin_helpcode_schema"] = @(MSIMECloudHelpcodeSchemaIndex(defaults, @"shuangpin"));
    snapshot[@"platform.macos.local_input_modes"] = @(MSIMECloudLocalInputModesEnabled(defaults));
    NSDictionary *booleans = MSIMECloudBooleanPreferences();
    for (NSString *key in booleans) {
        NSArray *field = booleans[key];
        snapshot[[@"platform.macos." stringByAppendingString:key]] =
            [defaults objectForKey:field[0]] == nil ? field[1] : @([defaults boolForKey:field[0]]);
    }
    return snapshot;
}

static inline BOOL MSIMEValidateCloudAppearance(NSDictionary *values) {
    if (![values isKindOfClass:NSDictionary.class] || values.count != 11 + MSIMECloudBooleanPreferences().count) return NO;
    id theme = values[@"platform.macos.global_theme"];
    if (![theme isKindOfClass:NSString.class] || !msime::mac::IsGlobalThemeId([theme UTF8String])) return NO;
    id base = values[@"platform.macos.custom_theme_base"];
    if (![base isKindOfClass:NSString.class] || !msime::mac::IsThemeBaseId([base UTF8String])) return NO;
    if (!MSIMECloudCustomCandidateSkin(values[@"platform.macos.custom_candidate_skin"])) return NO;
    if (!MSIMECloudAppearanceIntegerInRange(values[@"platform.macos.candidate_font_size"], 12, 32) ||
        !MSIMECloudAppearanceCandidatePageSize(values[@"platform.macos.candidate_page_size"])) return NO;
    NSDictionary *options = @{@"platform.macos.quanpin_helpcode_schema": @[@0,@1,@2,@3,@4],
                              @"platform.macos.shuangpin_helpcode_schema": @[@0,@1,@2,@3,@4],
                              @"platform.macos.candidate_panel_style": @[@0,@1],
                              @"platform.macos.input_scheme": @[@0,@1,@2],
                              @"platform.macos.candidate_page_shortcut": @[@0,@1,@2]};
    for (NSString *key in options) {
        id value = values[key];
        if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() ||
            ![options[key] containsObject:value]) return NO;
    }
    for (NSString *key in MSIMECloudBooleanPreferences()) {
        id value = values[[@"platform.macos." stringByAppendingString:key]];
        if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID()) return NO;
    }
    id localModes = values[@"platform.macos.local_input_modes"];
    if (![localModes isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)localModes) != CFBooleanGetTypeID()) return NO;
    return YES;
}

static inline BOOL MSIMEApplyCloudAppearance(NSDictionary *values, NSUserDefaults *defaults) {
    if (!MSIMEValidateCloudAppearance(values)) return NO;
    [defaults setObject:values[@"platform.macos.global_theme"] forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:values[@"platform.macos.custom_theme_base"] forKey:@"MSIMEClientCustomThemeBase"];
    [defaults setObject:values[@"platform.macos.custom_candidate_skin"] forKey:@"MSIMEClientCustomCandidateSkin"];
    [defaults setObject:values[@"platform.macos.candidate_panel_style"] forKey:@"MSIMEClientCandidatePanelStyle"];
    [defaults setObject:values[@"platform.macos.candidate_font_size"] forKey:@"MSIMEClientCandidateFontSize"];
    [defaults setObject:values[@"platform.macos.candidate_page_size"] forKey:@"MSIMEClientCandidatePageSize"];
    [defaults setObject:values[@"platform.macos.candidate_page_shortcut"] forKey:@"MSIMEClientCandidatePageShortcut"];
    [defaults setObject:@[@"quanpin", @"shuangpin", @"wubi"][[values[@"platform.macos.input_scheme"] unsignedIntegerValue]] forKey:@"MSIMEClientInputScheme"];
    NSMutableDictionary *helpcode = [[defaults dictionaryForKey:@"MSIMEClientHelpcodeOptions"] mutableCopy] ?: [NSMutableDictionary dictionary];
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        NSMutableDictionary *options = [helpcode[scheme] isKindOfClass:NSDictionary.class] ? [helpcode[scheme] mutableCopy] : [NSMutableDictionary dictionary];
        NSString *key = [scheme isEqual:@"quanpin"] ? @"platform.macos.quanpin_helpcode_schema" : @"platform.macos.shuangpin_helpcode_schema";
        options[@"schema"] = MSIMECloudHelpcodeSchemas()[[values[key] unsignedIntegerValue]];
        helpcode[scheme] = options;
    }
    [defaults setObject:helpcode forKey:@"MSIMEClientHelpcodeOptions"];
    NSMutableDictionary *localModes = [[defaults dictionaryForKey:@"MSIMEClientLocalModes"] mutableCopy] ?: [NSMutableDictionary dictionary];
    for (NSString *key in MSIMECloudLocalModeKeys()) localModes[key] = values[@"platform.macos.local_input_modes"];
    [defaults setObject:localModes forKey:@"MSIMEClientLocalModes"];
    NSDictionary *booleans = MSIMECloudBooleanPreferences();
    for (NSString *key in booleans) {
        [defaults setObject:values[[@"platform.macos." stringByAppendingString:key]] forKey:booleans[key][0]];
    }
    return YES;
}
