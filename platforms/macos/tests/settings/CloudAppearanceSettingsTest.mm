#import "../../src/cloud/CloudAppearanceSettings.h"
#include <cassert>
#import "TestPreferenceSuite.h"
int main() {
  @autoreleasepool {
    NSString *suite = [@"msime.synthetic." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    NSDictionary *initial = MSIMECloudAppearanceSnapshot(defaults);
    assert([initial[@"platform.macos.candidate_font_size"] isEqual:@18]);
    assert([initial[@"platform.macos.candidate_page_size"] isEqual:@9]);
    assert([initial[@"platform.macos.candidate_panel_style"] isEqual:@0]);
    assert(MSIMEValidateCloudAppearance(initial));
    assert(initial.count == 25);
    // A fresh install is on the system theme with no custom base or package.
    assert([initial[@"platform.macos.global_theme"] isEqual:@"system"]);
    assert([initial[@"platform.macos.custom_theme_base"] isEqual:@"system"]);
    assert([initial[@"platform.macos.custom_candidate_skin"] isEqual:@""]);
    assert([initial[@"platform.macos.quanpin_helpcode_schema"] isEqual:@1]);
    assert([initial[@"platform.macos.shuangpin_helpcode_schema"] isEqual:@0]);
    assert([initial[@"platform.macos.local_input_modes"] isEqual:@YES]);
    assert([initial[@"platform.macos.shuangpin_preedit_uses_raw"] isEqual:@YES]);
    // 每套内置辅助码在全拼和双拼里都能导出，再导入回相同选择。
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
      for (NSUInteger index = 0; index < MSIMECloudHelpcodeSchemas().count; ++index) {
        NSString *schema = MSIMECloudHelpcodeSchemas()[index];
        [defaults setObject:@{scheme: @{@"schema": schema}} forKey:@"MSIMEClientHelpcodeOptions"];
        NSDictionary *snapshot = MSIMECloudAppearanceSnapshot(defaults);
        NSString *key = [NSString stringWithFormat:@"platform.macos.%@_helpcode_schema", scheme];
        assert([snapshot[key] isEqual:@(index)]);
        assert(MSIMEValidateCloudAppearance(snapshot));
        [defaults removeObjectForKey:@"MSIMEClientHelpcodeOptions"];
        assert(MSIMEApplyCloudAppearance(snapshot, defaults));
        assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:snapshot]);
        assert([[defaults dictionaryForKey:@"MSIMEClientHelpcodeOptions"][scheme][@"schema"] isEqual:schema]);
        NSDictionary *narrowed = MSIMENarrowCloudAppearance(snapshot, @[@"quanpin", @"shuangpin"]);
        assert(MSIMEApplyCloudAppearanceForSchemes(narrowed, defaults, @[@"quanpin", @"shuangpin"]));
      }
    }
    assert(MSIMEApplyCloudAppearance(initial, defaults));
    for (NSString *key in @[@"autocorrect", @"helpcode", @"chinese_punctuation", @"input_mode_shortcut", @"floating_toolbar", @"candidate_learning"])
      assert([initial[[@"platform.macos." stringByAppendingString:key]] isEqual:@YES]);
    for (NSString *key in @[@"english_input_mode", @"full_width_input", @"smart_punctuation", @"smart_punctuation_repeat", @"traditional_chinese_output", @"wubi_auto_commit_unique", @"shuangpin_keymap"])
      assert([initial[[@"platform.macos." stringByAppendingString:key]] isEqual:@NO]);
    [defaults setObject:@"japanese" forKey:@"MSIMEClientInputScheme"];
    NSDictionary *japaneseNativeSnapshot = MSIMECloudAppearanceSnapshot(defaults);
    assert([japaneseNativeSnapshot[@"platform.macos.input_scheme"] isEqual:@0]);
    assert(MSIMEValidateCloudAppearance(japaneseNativeSnapshot));
    // The fixed Apple cloud contract has no Korean entry either, so it keeps the same Chinese fallback.
    [defaults setObject:@"korean" forKey:@"MSIMEClientInputScheme"];
    NSDictionary *koreanNativeSnapshot = MSIMECloudAppearanceSnapshot(defaults);
    assert([koreanNativeSnapshot[@"platform.macos.input_scheme"] isEqual:@0]);
    assert(MSIMEValidateCloudAppearance(koreanNativeSnapshot));

    // The schemes added after the contract was fixed export the same quanpin fallback and still validate.
    for (NSString *scheme in @[@"cantonese", @"zhuyin", @"vietnamese", @"tibetan", @"stroke"]) {
        [defaults setObject:scheme forKey:@"MSIMEClientInputScheme"];
        NSDictionary *snapshot = MSIMECloudAppearanceSnapshot(defaults);
        assert([snapshot[@"platform.macos.input_scheme"] isEqual:@0]);
        assert(MSIMEValidateCloudAppearance(snapshot));
    }
    NSMutableDictionary *values = [initial mutableCopy];
    values[@"platform.macos.global_theme"] = @"custom";
    values[@"platform.macos.custom_theme_base"] = @"night";
    values[@"platform.macos.custom_candidate_skin"] = @"wide-card";
    values[@"platform.macos.candidate_panel_style"] = @1;
    values[@"platform.macos.candidate_font_size"] = @20;
    values[@"platform.macos.candidate_page_size"] = @7;
    values[@"platform.macos.input_scheme"] = @2;
    values[@"platform.macos.candidate_page_shortcut"] = @1;
    values[@"platform.macos.quanpin_helpcode_schema"] = @4;
    values[@"platform.macos.shuangpin_helpcode_schema"] = @3;
    values[@"platform.macos.local_input_modes"] = @NO;
    for (NSString *key in MSIMECloudBooleanPreferences()) {
      NSString *cloudKey = [@"platform.macos." stringByAppendingString:key];
      values[cloudKey] = @(![initial[cloudKey] boolValue]);
    }
    assert(MSIMEApplyCloudAppearance(values, defaults));
    assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:values]);
    assert([[defaults stringForKey:@"MSIMEClientInputScheme"] isEqual:@"wubi"]);
    assert([defaults integerForKey:@"MSIMEClientCandidatePageShortcut"] == 1);
    NSDictionary *helpcode = [defaults dictionaryForKey:@"MSIMEClientHelpcodeOptions"];
    assert([helpcode[@"quanpin"][@"schema"] isEqual:@"xiaohe"]);
    assert([helpcode[@"shuangpin"][@"schema"] isEqual:@"shouyouplus"]);
    NSDictionary *localModes = [defaults dictionaryForKey:@"MSIMEClientLocalModes"];
    for (NSString *key in MSIMECloudLocalModeKeys()) assert([localModes[key] isEqual:@NO]);
    assert(![defaults boolForKey:@"MSIMEClientCandidateLearning"]);
    assert([defaults boolForKey:@"MSIMEClientFullWidthInput"]);
    assert(![defaults boolForKey:@"MSIMEClientChinesePunctuation"]);
    NSDictionary *saved = [values copy];
    // Every size offered by the active native UI survives cloud export/import.
    for (NSInteger font = 12; font <= 32; ++font) {
      for (NSInteger page : {5, 7, 9}) {
        [defaults setInteger:font forKey:@"MSIMEClientCandidateFontSize"];
        [defaults setInteger:page forKey:@"MSIMEClientCandidatePageSize"];
        NSDictionary *snapshot = MSIMECloudAppearanceSnapshot(defaults);
        assert([snapshot[@"platform.macos.candidate_font_size"] integerValue] == font);
        assert([snapshot[@"platform.macos.candidate_page_size"] integerValue] == page);
        [defaults setInteger:18 forKey:@"MSIMEClientCandidateFontSize"];
        [defaults setInteger:9 forKey:@"MSIMEClientCandidatePageSize"];
        assert(MSIMEApplyCloudAppearance(snapshot, defaults));
        assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:snapshot]);
      }
    }
    assert(MSIMEApplyCloudAppearance(saved, defaults));
    for (NSString *key in MSIMECloudBooleanPreferences()) {
      for (id invalid in @[@1, @"true", NSNull.null]) {
        NSMutableDictionary *bad = [saved mutableCopy];
        bad[[@"platform.macos." stringByAppendingString:key]] = invalid;
        assert(!MSIMEApplyCloudAppearance(bad, defaults));
        assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:saved]);
      }
    }
    for (NSString *key in @[@"platform.macos.input_scheme", @"platform.macos.candidate_page_shortcut"]) {
      for (id invalid in @[@YES, @3, @(-1), @1.5, @"1"]) {
        NSMutableDictionary *bad = [saved mutableCopy]; bad[key] = invalid;
        assert(!MSIMEApplyCloudAppearance(bad, defaults));
        assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:saved]);
      }
    }
    for (NSString *key in @[@"platform.macos.quanpin_helpcode_schema", @"platform.macos.shuangpin_helpcode_schema"]) {
      for (id invalid in @[@YES, @(MSIMECloudHelpcodeSchemas().count), @(-1), @1.5, @"1"]) {
        NSMutableDictionary *bad = [saved mutableCopy]; bad[key] = invalid;
        assert(!MSIMEApplyCloudAppearance(bad, defaults));
        assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:saved]);
      }
    }
    for (id invalid in @[@1, @0, @1.5, @"true", NSNull.null]) {
      NSMutableDictionary *bad = [saved mutableCopy]; bad[@"platform.macos.local_input_modes"] = invalid;
      assert(!MSIMEApplyCloudAppearance(bad, defaults));
      assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:saved]);
    }
    for (id invalid in @[@YES, @11, @33, @18.5, @"18", NSNull.null]) {
      values[@"platform.macos.candidate_font_size"] = invalid;
      assert(!MSIMEApplyCloudAppearance(values, defaults));
      assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:saved]);
    }
    // 1 and 6 are sizes the shared preferences accept, so a snapshot carrying either is applied rather
    // than refused; what stays invalid is a non-number, a non-integer, and anything outside 1..9.
    for (id invalid in @[@YES, @0, @10, @1.5, @"5", NSNull.null]) {
      values = [saved mutableCopy];
      values[@"platform.macos.candidate_page_size"] = invalid;
      assert(!MSIMEApplyCloudAppearance(values, defaults));
      assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:saved]);
    }
    for (NSNumber *accepted in @[@1, @4, @6, @9]) {
      values = [saved mutableCopy];
      values[@"platform.macos.candidate_page_size"] = accepted;
      assert(MSIMEApplyCloudAppearance(values, defaults));
      assert([MSIMECloudAppearanceSnapshot(defaults)[@"platform.macos.candidate_page_size"] isEqual:accepted]);
    }
    MSIMEApplyCloudAppearance(saved, defaults);
    for (id invalid in @[@YES, @0, @99, @12.5, @"12"]) {
      [defaults setObject:invalid forKey:@"MSIMEClientCandidateFontSize"];
      [defaults setObject:invalid forKey:@"MSIMEClientCandidatePageSize"];
      NSDictionary *snapshot = MSIMECloudAppearanceSnapshot(defaults);
      assert([snapshot[@"platform.macos.candidate_font_size"] isEqual:@18]);
      assert([snapshot[@"platform.macos.candidate_page_size"] isEqual:@9]);
      assert(MSIMEValidateCloudAppearance(snapshot));
    }
    [defaults setObject:@{@"quanpin": @{@"schema": @"invalid"}} forKey:@"MSIMEClientHelpcodeOptions"];
    assert([MSIMECloudAppearanceSnapshot(defaults)[@"platform.macos.quanpin_helpcode_schema"] isEqual:@1]);
    assert(MSIMEApplyCloudAppearance(saved, defaults));
    // The retired per-host skin key is an unexpected field now, and the three theme keys accept only their own values.
    values = [saved mutableCopy]; values[@"platform.macos.candidate_skin"] = @"wechat";
    assert(!MSIMEApplyCloudAppearance(values, defaults));
    NSDictionary *invalidThemes = @{@"platform.macos.global_theme": @[@"fluent", @"", @"../unsafe", @1, NSNull.null],
                                    @"platform.macos.custom_theme_base": @[@"custom", @"fluent", @"", @1, NSNull.null],
                                    @"platform.macos.custom_candidate_skin": @[@"../unsafe", @"shuishan", @"custom", @1, NSNull.null]};
    for (NSString *key in invalidThemes) {
      for (id invalid in invalidThemes[key]) {
        NSMutableDictionary *bad = [saved mutableCopy]; bad[key] = invalid;
        assert(!MSIMEApplyCloudAppearance(bad, defaults));
        assert([MSIMECloudAppearanceSnapshot(defaults) isEqual:saved]);
      }
    }
    // A stored value outside the catalog exports as the default rather than poisoning the upload.
    [defaults setObject:@"fluent" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:@"custom" forKey:@"MSIMEClientCustomThemeBase"];
    [defaults setObject:@"../unsafe" forKey:@"MSIMEClientCustomCandidateSkin"];
    NSDictionary *sanitized = MSIMECloudAppearanceSnapshot(defaults);
    assert([sanitized[@"platform.macos.global_theme"] isEqual:@"system"]);
    assert([sanitized[@"platform.macos.custom_theme_base"] isEqual:@"system"]);
    assert([sanitized[@"platform.macos.custom_candidate_skin"] isEqual:@""]);
    assert(MSIMEValidateCloudAppearance(sanitized));
    assert(MSIMEApplyCloudAppearance(saved, defaults));
    values = [saved mutableCopy]; values[@"unexpected"] = @1;
    assert(!MSIMEApplyCloudAppearance(values, defaults));

    // 版本收窄（与 client-core 的账号偏好过滤规则相同）。测试进程是 full：什么也不去掉。
    assert(MSIMENarrowCloudAppearance(saved, nil) == saved);
    assert(MSIMEAdoptCloudAppearance(saved, saved, nil) == saved);
    // 五笔版只有一个方案：不带 input_scheme，也不带只属于全拼、双拼的字段；五笔的字段照常带。
    NSArray *wubi = @[@"wubi"];
    NSDictionary *wubiSnapshot = MSIMENarrowCloudAppearance(saved, wubi);
    assert(wubiSnapshot[@"platform.macos.input_scheme"] == nil);
    for (NSString *key in @[@"platform.macos.quanpin_helpcode_schema", @"platform.macos.shuangpin_helpcode_schema", @"platform.macos.shuangpin_keymap", @"platform.macos.shuangpin_preedit_uses_raw"])
      assert(wubiSnapshot[key] == nil);
    assert(wubiSnapshot[@"platform.macos.wubi_auto_commit_unique"] != nil);
    assert(wubiSnapshot.count == saved.count - 5);
    assert(MSIMEValidateCloudAppearanceForSchemes(wubiSnapshot, wubi));
    // 一份 full 的完整快照不是五笔版的快照，反之亦然。
    assert(!MSIMEValidateCloudAppearanceForSchemes(saved, wubi));
    assert(!MSIMEValidateCloudAppearanceForSchemes(wubiSnapshot, nil));
    // 应用五笔版的快照不碰本机的方案和全拼、双拼的设置。
    [defaults setObject:@"wubi" forKey:@"MSIMEClientInputScheme"];
    [defaults setBool:YES forKey:@"MSIMEClientShuangpinKeymap"];
    NSMutableDictionary *wubiValues = [wubiSnapshot mutableCopy];
    wubiValues[@"platform.macos.candidate_font_size"] = @22;
    assert(MSIMEApplyCloudAppearanceForSchemes(wubiValues, defaults, wubi));
    assert([[defaults stringForKey:@"MSIMEClientInputScheme"] isEqualToString:@"wubi"]);
    assert([defaults boolForKey:@"MSIMEClientShuangpinKeymap"]);
    assert([[defaults objectForKey:@"MSIMEClientCandidateFontSize"] isEqual:@22]);
    // 拼音版有两个方案：同步 input_scheme，但只认全拼和双拼；别处来的五笔当作没有这一项，保留本机的选择。
    NSArray *pinyin = @[@"quanpin", @"shuangpin"];
    NSMutableDictionary *pinyinSnapshot = [MSIMENarrowCloudAppearance(saved, pinyin) mutableCopy];
    assert(pinyinSnapshot[@"platform.macos.input_scheme"] != nil && pinyinSnapshot[@"platform.macos.wubi_auto_commit_unique"] == nil);
    pinyinSnapshot[@"platform.macos.input_scheme"] = @1;
    assert(MSIMEValidateCloudAppearanceForSchemes(pinyinSnapshot, pinyin));
    NSMutableDictionary *fromWubi = [saved mutableCopy];
    fromWubi[@"platform.macos.input_scheme"] = @2;
    assert(!MSIMEValidateCloudAppearanceForSchemes(MSIMENarrowCloudAppearance(fromWubi, pinyin), pinyin));
    NSDictionary *adopted = MSIMEAdoptCloudAppearance(fromWubi, pinyinSnapshot, pinyin);
    assert([adopted[@"platform.macos.input_scheme"] isEqual:@1]);
    assert(MSIMEValidateCloudAppearanceForSchemes(adopted, pinyin));
    MSIMERemoveTestPreferenceSuite(defaults, suite);
  }
}
