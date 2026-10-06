#import <Foundation/Foundation.h>
#import "../../../../shared/apple/ClipboardPreferences.h"
#include <cassert>

int main() {
    @autoreleasepool {
        for (id invalid in @[@YES, @1.5, @1e30, @(-1)]) {
            __block NSUInteger invalidSaves = 0;
            NSDictionary *result = MSIMEEnableClipboardHistory(^NSDictionary *{
                return @{ @"revision": invalid,
                    @"preferences": @{ @"clipboard_history": @NO } };
            }, ^NSDictionary *(uint64_t, NSDictionary *) {
                ++invalidSaves;
                return nil;
            });
            assert(result[@"error"] && invalidSaves == 0);
        }
        NSDictionary *snapshot = @{ @"format_version": @1, @"revision": @7,
            @"preferences": @{ @"clipboard_history": @NO, @"theme": @"light", @"synthetic_setting": @42 } };
        __block NSUInteger saves = 0;
        NSDictionary *result = MSIMEEnableClipboardHistory(^NSDictionary *{ return snapshot; },
            ^NSDictionary *(uint64_t revision, NSDictionary *next) {
                ++saves;
                assert(revision == 7);
                assert([next[@"preferences"][@"clipboard_history"] isEqual:@YES]);
                assert([next[@"preferences"][@"theme"] isEqual:@"light"]);
                assert([next[@"preferences"][@"synthetic_setting"] isEqual:@42]);
                assert([next[@"revision"] isEqual:@7]);
                return next;
            });
        assert([result[@"enabled"] isEqual:@YES] && saves == 1);
        assert([snapshot[@"preferences"][@"clipboard_history"] isEqual:@NO]);
        result = MSIMEEnableClipboardHistory(^NSDictionary *{ return snapshot; },
            ^NSDictionary *(uint64_t, NSDictionary *) { return nil; });
        assert(result[@"error"]);
        result = MSIMEEnableClipboardHistory(^NSDictionary *{ return nil; },
            ^NSDictionary *(uint64_t, NSDictionary *) { assert(false); return nil; });
        assert(result[@"error"]);
        result = MSIMEEnableClipboardHistory(^NSDictionary *{
            return @{ @"revision": @8, @"preferences": @{ @"clipboard_history": @YES } };
        }, ^NSDictionary *(uint64_t, NSDictionary *) { assert(false); return nil; });
        assert([result[@"enabled"] isEqual:@YES]);
        puts("Clipboard enable preserves settings, passes revision and propagates save failure");
    }
}
