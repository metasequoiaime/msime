#pragma once
#import <Foundation/Foundation.h>
#include "../candidate/CandidateSkin.h"
#include "../candidate/CandidatePageSize.h"
#import "../core/EditionIdentity.h"

static inline BOOL MSIMECloudAppearanceIntegerInRange(id value, NSInteger minimum, NSInteger maximum) {
    if (![value isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID()) return NO;
    if (CFNumberIsFloatType((__bridge CFNumberRef)value)) return NO;
    NSInteger integer = [value integerValue];
    return integer >= minimum && integer <= maximum;
}

// The whole range the shared preferences accept, not the three sizes this platform's window used to
// offer: a cloud snapshot written by any other host carries the size that host allowed, and rejecting
// it here would drop the user's appearance on the way in.
static inline BOOL MSIMECloudAppearanceCandidatePageSize(id value) {
    return MSIMECloudAppearanceIntegerInRange(value, (NSInteger)msime::mac::kMinimumCandidatePageSize,
                                              (NSInteger)msime::mac::kMaximumCandidatePageSize);
}

// 云端按下标同步辅助码方案，新方案只能追加在末尾，旧下标不能变。
static inline NSArray<NSString *> *MSIMECloudHelpcodeSchemas() {
    return @[@"lantian", @"ziranma", @"shouyou2_0", @"shouyouplus", @"xiaohe", @"jiajia", @"wubi86"];
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
             @"wubi_auto_commit_unique": @[@"MSIMEClientWubiAutoCommitUnique", @YES],
             @"candidate_learning": @[@"MSIMEClientCandidateLearning", @YES],
             @"shuangpin_keymap": @[@"MSIMEClientShuangpinKeymap", @NO],
             @"shuangpin_preedit_uses_raw": @[@"MSIMEClientShuangpinPreeditUsesRaw", @YES]};
}

// 版本与账号同步：几个版本同时登录同一个账号，云端只有一份 macOS 设置。规则与 client-core 的 `filter_uploaded_account_settings` / `filter_downloaded_account_settings` 相同——只有一个方案的版本既不上传也不应用 `input_scheme`；有几个方案的版本只认自己有的方案；只属于某个方案的字段，本版本没有那个方案时不上传也不应用，免得一个版本拿本机缺省值盖掉另一个版本在云端的选择。`offered` 是 MSIMEEditionInputSchemes()，nil 表示 full，什么也不去掉。
static inline NSArray<NSString *> *MSIMECloudInputSchemes() { return @[@"quanpin", @"shuangpin", @"wubi"]; }

static inline NSDictionary<NSString *, NSString *> *MSIMECloudSchemeScopedKeys() {
    return @{@"platform.macos.quanpin_helpcode_schema": @"quanpin",
             @"platform.macos.shuangpin_helpcode_schema": @"shuangpin",
             @"platform.macos.shuangpin_keymap": @"shuangpin",
             @"platform.macos.shuangpin_preedit_uses_raw": @"shuangpin",
             @"platform.macos.wubi_auto_commit_unique": @"wubi"};
}

static inline BOOL MSIMECloudSyncsInputScheme(NSArray<NSString *> *offered) { return !offered || offered.count > 1; }

// 本版本的快照里不带的键。
static inline BOOL MSIMECloudKeyOmitted(NSString *key, NSArray<NSString *> *offered) {
    if (!offered) return NO;
    if ([key isEqualToString:@"platform.macos.input_scheme"]) return !MSIMECloudSyncsInputScheme(offered);
    NSString *scheme = MSIMECloudSchemeScopedKeys()[key];
    return scheme && ![offered containsObject:scheme];
}

// `input_scheme` 可以取的值：云端契约里本版本有的那几个方案的编号。
static inline NSArray<NSNumber *> *MSIMECloudInputSchemeValues(NSArray<NSString *> *offered) {
    NSMutableArray<NSNumber *> *values = [NSMutableArray array];
    NSArray<NSString *> *schemes = MSIMECloudInputSchemes();
    for (NSUInteger index = 0; index < schemes.count; ++index)
        if (!offered || [offered containsObject:schemes[index]]) [values addObject:@(index)];
    return values;
}

// 去掉本版本不带的键。full 原样返回。
static inline NSDictionary *MSIMENarrowCloudAppearance(NSDictionary *values, NSArray<NSString *> *offered) {
    if (!offered) return values;
    NSMutableDictionary *narrowed = [values mutableCopy];
    for (NSString *key in values)
        if (MSIMECloudKeyOmitted(key, offered)) [narrowed removeObjectForKey:key];
    return narrowed;
}

// 别处来的快照（另一个版本导出的设置文件）收窄到本版本：去掉本版本不带的键；`input_scheme` 是本版本没有的方案时保留本机的选择 `local`。full 除了补上缺的深色槽位外原样返回。
// 深色槽位 `platform.macos.custom_candidate_skin_dark` 比其他键晚加入：早先导出的文件里没有它，这时保留本机的深色槽位，而不是让整份文件因为少一项被拒。
static inline NSDictionary *MSIMEAdoptCloudAppearance(NSDictionary *values, NSDictionary *local, NSArray<NSString *> *offered) {
    if (![values isKindOfClass:NSDictionary.class]) return values;
    NSString *darkSkin = @"platform.macos.custom_candidate_skin_dark";
    if (!values[darkSkin] && local[darkSkin]) {
        NSMutableDictionary *filled = [values mutableCopy];
        filled[darkSkin] = local[darkSkin];
        values = filled;
    }
    if (!offered) return values;
    NSMutableDictionary *adopted = [MSIMENarrowCloudAppearance(values, offered) mutableCopy];
    id scheme = adopted[@"platform.macos.input_scheme"];
    if (scheme && ![MSIMECloudInputSchemeValues(offered) containsObject:scheme]) adopted[@"platform.macos.input_scheme"] = local[@"platform.macos.input_scheme"];
    return adopted;
}

// 全局主题取代了各宿主自己的候选皮肤：`global_theme` 是主题目录里的 id，自定义主题的底色和两个槽位的皮肤包随它一起同步。浅色槽位 `custom_candidate_skin` 和深色槽位 `custom_candidate_skin_dark` 同样校验，空串表示没有。
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
    NSString *darkPackage = [defaults stringForKey:@"MSIMEClientCustomCandidateSkinDark"];
    NSMutableDictionary *snapshot = [@{@"platform.macos.global_theme": msime::mac::IsGlobalThemeId(theme.UTF8String ?: "") ? theme : @"system",
             @"platform.macos.custom_theme_base": msime::mac::IsThemeBaseId(base.UTF8String ?: "") ? base : @"system",
             @"platform.macos.custom_candidate_skin": MSIMECloudCustomCandidateSkin(package) ? package : @"",
             @"platform.macos.custom_candidate_skin_dark": MSIMECloudCustomCandidateSkin(darkPackage) ? darkPackage : @"",
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
    return MSIMENarrowCloudAppearance(snapshot, MSIMEEditionInputSchemes());
}

static inline BOOL MSIMEValidateCloudAppearanceForSchemes(NSDictionary *values, NSArray<NSString *> *offered) {
    NSUInteger omitted = 0;
    for (NSString *key in [@[@"platform.macos.input_scheme"] arrayByAddingObjectsFromArray:MSIMECloudSchemeScopedKeys().allKeys])
        if (MSIMECloudKeyOmitted(key, offered)) {
            if (values[key]) return NO;
            ++omitted;
        }
    if (![values isKindOfClass:NSDictionary.class] || values.count != 12 + MSIMECloudBooleanPreferences().count - omitted) return NO;
    id theme = values[@"platform.macos.global_theme"];
    if (![theme isKindOfClass:NSString.class] || !msime::mac::IsGlobalThemeId([theme UTF8String])) return NO;
    id base = values[@"platform.macos.custom_theme_base"];
    if (![base isKindOfClass:NSString.class] || !msime::mac::IsThemeBaseId([base UTF8String])) return NO;
    if (!MSIMECloudCustomCandidateSkin(values[@"platform.macos.custom_candidate_skin"]) ||
        !MSIMECloudCustomCandidateSkin(values[@"platform.macos.custom_candidate_skin_dark"])) return NO;
    if (!MSIMECloudAppearanceIntegerInRange(values[@"platform.macos.candidate_font_size"], 12, 32) ||
        !MSIMECloudAppearanceCandidatePageSize(values[@"platform.macos.candidate_page_size"])) return NO;
    // 辅助码方案的下标只要求是非负整数：超出本机方案目录的下标是更新的版本追加的方案，应用时保留本机的选择，而不是让整份快照失效。
    for (NSString *key in @[@"platform.macos.quanpin_helpcode_schema", @"platform.macos.shuangpin_helpcode_schema"]) {
        if (MSIMECloudKeyOmitted(key, offered)) continue;
        if (!MSIMECloudAppearanceIntegerInRange(values[key], 0, NSIntegerMax)) return NO;
    }
    NSDictionary *options = @{@"platform.macos.candidate_panel_style": @[@0,@1],
                              @"platform.macos.input_scheme": MSIMECloudInputSchemeValues(offered),
                              @"platform.macos.candidate_page_shortcut": @[@0,@1,@2]};
    for (NSString *key in options) {
        if (MSIMECloudKeyOmitted(key, offered)) continue;
        id value = values[key];
        if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() ||
            ![options[key] containsObject:value]) return NO;
    }
    for (NSString *key in MSIMECloudBooleanPreferences()) {
        if (MSIMECloudKeyOmitted([@"platform.macos." stringByAppendingString:key], offered)) continue;
        id value = values[[@"platform.macos." stringByAppendingString:key]];
        if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID()) return NO;
    }
    id localModes = values[@"platform.macos.local_input_modes"];
    if (![localModes isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)localModes) != CFBooleanGetTypeID()) return NO;
    return YES;
}

static inline BOOL MSIMEValidateCloudAppearance(NSDictionary *values) {
    return MSIMEValidateCloudAppearanceForSchemes(values, MSIMEEditionInputSchemes());
}

static inline BOOL MSIMEApplyCloudAppearanceForSchemes(NSDictionary *values, NSUserDefaults *defaults, NSArray<NSString *> *offered) {
    if (!MSIMEValidateCloudAppearanceForSchemes(values, offered)) return NO;
    [defaults setObject:values[@"platform.macos.global_theme"] forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:values[@"platform.macos.custom_theme_base"] forKey:@"MSIMEClientCustomThemeBase"];
    [defaults setObject:values[@"platform.macos.custom_candidate_skin"] forKey:@"MSIMEClientCustomCandidateSkin"];
    [defaults setObject:values[@"platform.macos.custom_candidate_skin_dark"] forKey:@"MSIMEClientCustomCandidateSkinDark"];
    [defaults setObject:values[@"platform.macos.candidate_panel_style"] forKey:@"MSIMEClientCandidatePanelStyle"];
    [defaults setObject:values[@"platform.macos.candidate_font_size"] forKey:@"MSIMEClientCandidateFontSize"];
    [defaults setObject:values[@"platform.macos.candidate_page_size"] forKey:@"MSIMEClientCandidatePageSize"];
    [defaults setObject:values[@"platform.macos.candidate_page_shortcut"] forKey:@"MSIMEClientCandidatePageShortcut"];
    if (values[@"platform.macos.input_scheme"])
        [defaults setObject:MSIMECloudInputSchemes()[[values[@"platform.macos.input_scheme"] unsignedIntegerValue]] forKey:@"MSIMEClientInputScheme"];
    NSMutableDictionary *helpcode = [[defaults dictionaryForKey:@"MSIMEClientHelpcodeOptions"] mutableCopy] ?: [NSMutableDictionary dictionary];
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        NSMutableDictionary *options = [helpcode[scheme] isKindOfClass:NSDictionary.class] ? [helpcode[scheme] mutableCopy] : [NSMutableDictionary dictionary];
        NSString *key = [scheme isEqual:@"quanpin"] ? @"platform.macos.quanpin_helpcode_schema" : @"platform.macos.shuangpin_helpcode_schema";
        // 本机不认识的下标（更新的版本追加的方案）保留本机的选择。
        if (!values[key] || [values[key] unsignedIntegerValue] >= MSIMECloudHelpcodeSchemas().count) continue;
        options[@"schema"] = MSIMECloudHelpcodeSchemas()[[values[key] unsignedIntegerValue]];
        helpcode[scheme] = options;
    }
    [defaults setObject:helpcode forKey:@"MSIMEClientHelpcodeOptions"];
    NSMutableDictionary *localModes = [[defaults dictionaryForKey:@"MSIMEClientLocalModes"] mutableCopy] ?: [NSMutableDictionary dictionary];
    for (NSString *key in MSIMECloudLocalModeKeys()) localModes[key] = values[@"platform.macos.local_input_modes"];
    [defaults setObject:localModes forKey:@"MSIMEClientLocalModes"];
    NSDictionary *booleans = MSIMECloudBooleanPreferences();
    for (NSString *key in booleans) {
        id value = values[[@"platform.macos." stringByAppendingString:key]];
        if (value) [defaults setObject:value forKey:booleans[key][0]];
    }
    return YES;
}

static inline BOOL MSIMEApplyCloudAppearance(NSDictionary *values, NSUserDefaults *defaults) {
    return MSIMEApplyCloudAppearanceForSchemes(values, defaults, MSIMEEditionInputSchemes());
}
