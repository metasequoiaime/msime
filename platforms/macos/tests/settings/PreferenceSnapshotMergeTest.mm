#import "../../src/settings/PreferenceSnapshotMerge.h"
#include <cassert>

int main() {
    @autoreleasepool {
        NSDictionary *captured = @{@"candidate_page_size": @7,
            @"quanpin_helpcode": @{@"enabled": @NO}, @"shuangpin_helpcode": @{@"enabled": @NO},
            @"voice_input": @{}, @"floating_toolbar": @{@"enabled": @NO}};
        NSDictionary *base = @{@"candidate_page_size": @9, @"other_platform": @{@"enabled": @YES},
            @"quanpin_helpcode": @{@"enabled": @YES, @"schema": @"synthetic-a"},
            @"shuangpin_helpcode": @{@"schema": @"synthetic-b"},
            @"voice_input": @{@"language": @"en-US"}, @"floating_toolbar": @{@"position": @42}};
        NSDictionary *first = MSIMEMergePreferenceSnapshot(base, captured);
        assert([first[@"candidate_page_size"] isEqual:@7]);
        assert([first[@"quanpin_helpcode"] isEqual:(@{@"enabled": @NO, @"schema": @"synthetic-a"})]);
        assert([first[@"voice_input"] isEqual:base[@"voice_input"]]);
        assert([first[@"other_platform"] isEqual:base[@"other_platform"]]);
        assert([first[@"floating_toolbar"][@"position"] isEqual:@42]);
        NSMutableDictionary *latest = [base mutableCopy];
        latest[@"quanpin_helpcode"] = @{@"schema": @"synthetic-new"};
        latest[@"candidate_page_size"] = @5;
        NSDictionary *retry = MSIMEMergePreferenceSnapshot(latest, captured);
        assert([retry[@"candidate_page_size"] isEqual:@7]);
        assert([retry[@"quanpin_helpcode"][@"schema"] isEqual:@"synthetic-new"]);
        assert([base[@"candidate_page_size"] isEqual:@9]);
        // 应用例外整张替换：本机移除的规则不能被文档里的旧表合并回来，清空时写空表。
        NSDictionary *rules = MSIMEMergePreferenceSnapshot(
            @{@"app_input_mode_rules": @{@"org.example.a": @"english", @"org.example.b": @"chinese"}},
            @{@"app_input_mode_rules": @{@"org.example.b": @"english"}});
        assert([rules[@"app_input_mode_rules"] isEqual:(@{@"org.example.b": @"english"})]);
        NSDictionary *cleared = MSIMEMergePreferenceSnapshot(
            @{@"app_input_mode_rules": @{@"org.example.a": @"english"}}, @{@"app_input_mode_rules": @{}});
        assert([cleared[@"app_input_mode_rules"] isEqual:@{}]);
        assert(!MSIMEMergePreferenceSnapshot(nil, captured));
        assert(!MSIMEMergePreferenceSnapshot(@{@"quanpin_helpcode": @"invalid"}, captured));
    }
}
