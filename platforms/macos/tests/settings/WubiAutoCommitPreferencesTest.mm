#import "TestPreferenceSuite.h"
#import "../../src/settings/AppearancePreferences.h"
#import "../../src/cloud/CloudAppearanceSettings.h"
#include <cassert>

// 四码唯一自动上屏换缺省值（云契约的 @NO -> @YES）时的迁移：接入共享偏好之前 macOS 从不兑现这个键，
// 所以 defaults 里和旧云快照里的 false 都是历史缺省，不是用户选择。只有本机留下过新语义下的真实选择
// 之后，存下来的值才作数——否则升级后（或下一次套用旧云快照后）第四键会莫名其妙不再上屏。
//
// 真实选择有两个入口：原生外观面板的 setter，和共享设置页写进共享文档的 false（macOS 上的主要入口）。
// 文档里的 true 不是选择：升级时旧的原生 NO 合并进文档之前 getter 已经读作开，只有显式的 false 才说明
// 用户关过它。
static NSString *const WubiAutoCommitKey = @"MSIMEClientWubiAutoCommitUnique";
static NSString *const WubiAutoCommitAdoptedKey = @"MSIMEClientWubiAutoCommitUniqueAdopted";

static MSIMEAppearancePreferences *FreshPreferences(NSUserDefaults **defaults, NSString **suite)
{
    *suite = [@"MSIME.WubiAutoCommit." stringByAppendingString:NSUUID.UUID.UUIDString];
    *defaults = [[NSUserDefaults alloc] initWithSuiteName:*suite];
    return [[MSIMEAppearancePreferences alloc] initWithDefaults:*defaults];
}

int main(void)
{
    @autoreleasepool {
        // 升级场景：defaults 里留着旧缺省写下的 NO，用户从没选过 -> 读作开。
        NSUserDefaults *defaults = nil;
        NSString *suite = nil;
        MSIMEAppearancePreferences *preferences = FreshPreferences(&defaults, &suite);
        [defaults setBool:NO forKey:WubiAutoCommitKey];
        assert(preferences.wubiAutoCommitUnique);
        // 导出取生效值，所以共享文档和云快照都拿到 true，而不是 defaults 里的 false。
        assert([[preferences sharedPreferencesByMerging:@{}][@"wubi_auto_commit_unique"] isEqual:@YES]);
        assert([[preferences cloudSettingsSnapshot][@"platform.macos.wubi_auto_commit_unique"] isEqual:@YES]);

        // 旧账号同步下来的 false 也是历史缺省：套用后仍读作开，且没有把用户记成已经选过。
        NSMutableDictionary *cloud = [[preferences cloudSettingsSnapshot] mutableCopy];
        cloud[@"platform.macos.wubi_auto_commit_unique"] = @NO;
        assert([preferences applyCloudSettingsSnapshot:cloud]);
        assert(preferences.wubiAutoCommitUnique);
        assert([defaults objectForKey:WubiAutoCommitAdoptedKey] == nil);

        // 用户在原生面板里第一次选关：这才是新语义下的真实选择，此后存下来的值作数。
        preferences.wubiAutoCommitUnique = NO;
        assert(!preferences.wubiAutoCommitUnique);
        assert([[defaults objectForKey:WubiAutoCommitAdoptedKey] boolValue]);
        assert([[preferences sharedPreferencesByMerging:@{}][@"wubi_auto_commit_unique"] isEqual:@NO]);

        // 选择持久化到新建的对象上；此后云端来的 false 与已选择的值一致，自然继续生效。
        MSIMEAppearancePreferences *fresh = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        assert(!fresh.wubiAutoCommitUnique);
        assert([fresh applyCloudSettingsSnapshot:cloud]);
        assert(!fresh.wubiAutoCommitUnique);
        MSIMERemoveTestPreferenceSuite(defaults, suite);

        // 文档里的 true 不是选择：不写标记，旧云快照的历史 false 仍被当作未设置。
        preferences = FreshPreferences(&defaults, &suite);
        [preferences applySharedInputPreferences:@{@"wubi_auto_commit_unique": @YES}];
        assert(preferences.wubiAutoCommitUnique);
        assert([defaults objectForKey:WubiAutoCommitAdoptedKey] == nil);
        cloud = [[preferences cloudSettingsSnapshot] mutableCopy];
        cloud[@"platform.macos.wubi_auto_commit_unique"] = @NO;
        assert([preferences applyCloudSettingsSnapshot:cloud]);
        assert(preferences.wubiAutoCommitUnique);
        MSIMERemoveTestPreferenceSuite(defaults, suite);

        // 共享设置页关掉：文档里的 false 是真实选择，也记标记，之后套用云快照不会再被恢复成开。
        preferences = FreshPreferences(&defaults, &suite);
        [preferences applySharedInputPreferences:@{@"wubi_auto_commit_unique": @NO}];
        assert(!preferences.wubiAutoCommitUnique);
        assert([[defaults objectForKey:WubiAutoCommitAdoptedKey] boolValue]);
        cloud = [[preferences cloudSettingsSnapshot] mutableCopy];
        cloud[@"platform.macos.wubi_auto_commit_unique"] = @NO;
        assert([preferences applyCloudSettingsSnapshot:cloud]);
        assert(!preferences.wubiAutoCommitUnique);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return 0;
}
