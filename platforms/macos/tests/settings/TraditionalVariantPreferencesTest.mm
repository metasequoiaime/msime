#import "../../src/settings/AppearancePreferences.h"
#import "TestPreferenceSuite.h"
#include <cassert>

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *suite = [@"msime.traditional-variant." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        assert(preferences.traditionalChineseVariant == 0);
        for (NSArray *entry in @[@[@"s2t", @0], @[@"s2tw", @1], @[@"s2twp", @2]]) {
            [preferences applySharedInputPreferences:@{@"traditional_chinese_variant":entry[0]}];
            assert(preferences.traditionalChineseVariant == [entry[1] unsignedIntValue]);
            // 旧平台快照没有变体字段，账号同步或导入它不能覆盖共享文档里的选择。
            NSDictionary *cloud = preferences.cloudSettingsSnapshot;
            assert(cloud[@"platform.macos.traditional_chinese_variant"] == nil);
            assert([preferences applyCloudSettingsSnapshot:cloud]);
            assert(preferences.traditionalChineseVariant == [entry[1] unsignedIntValue]);
        }
        for (NSDictionary *snapshot in @[@{}, @{@"traditional_chinese_variant":@"s2hk"},
                                         @{@"traditional_chinese_variant":@1}]) {
            [preferences applySharedInputPreferences:@{@"traditional_chinese_variant":@"s2tw"}];
            assert(preferences.traditionalChineseVariant == 1);
            [preferences applySharedInputPreferences:snapshot];
            assert(preferences.traditionalChineseVariant == 0);
        }
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
}
