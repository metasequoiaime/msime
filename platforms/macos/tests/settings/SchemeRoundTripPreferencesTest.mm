#import "TestPreferenceSuite.h"
// Switching to Japanese or Korean has to leave a way back to the Chinese scheme the user was on.
//
// The scheme the user returns to is carried in `last_chinese_scheme`, which every other host writes
// when the scheme changes. This window sets the scheme itself, Japanese included, so it has to write
// it too - otherwise the way back points at whatever a different surface last recorded, and a 五笔
// user comes back to 全拼.
#import "../../src/settings/AppearancePreferences.h"
#import <Foundation/Foundation.h>
#include <cassert>

int main(void)
{
    @autoreleasepool {
        NSString *suite = [@"MSIME.SchemeRoundTripTest." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];

        preferences.inputScheme = @"wubi";
        NSDictionary *merged = [preferences sharedPreferencesByMerging:@{}];
        assert([merged[@"scheme"] isEqual:@"wubi"]);
        assert([merged[@"last_chinese_scheme"] isEqual:@"wubi"]);

        // Japanese is not a Chinese scheme, so it must not overwrite the way back. The value in the
        // document being merged into is the one that survives.
        preferences.inputScheme = @"japanese";
        merged = [preferences sharedPreferencesByMerging:@{@"last_chinese_scheme": @"wubi"}];
        assert([merged[@"scheme"] isEqual:@"japanese"]);
        assert([merged[@"last_chinese_scheme"] isEqual:@"wubi"]);

        // Korean is not one either, and moving between the two keeps the Chinese scheme both were entered from.
        preferences.inputScheme = @"korean";
        merged = [preferences sharedPreferencesByMerging:@{@"last_chinese_scheme": @"wubi"}];
        assert([merged[@"scheme"] isEqual:@"korean"]);
        assert([merged[@"last_chinese_scheme"] isEqual:@"wubi"]);
        assert([preferences.lastChineseScheme isEqual:@"wubi"]);

        // Coming back records the scheme that was returned to, so the next excursion goes back there.
        preferences.inputScheme = @"shuangpin";
        merged = [preferences sharedPreferencesByMerging:@{@"last_chinese_scheme": @"wubi"}];
        assert([merged[@"scheme"] isEqual:@"shuangpin"]);
        assert([merged[@"last_chinese_scheme"] isEqual:@"shuangpin"]);

        preferences.inputScheme = @"quanpin";
        merged = [preferences sharedPreferencesByMerging:@{}];
        assert([merged[@"last_chinese_scheme"] isEqual:@"quanpin"]);

        // Vietnamese is not a Chinese scheme either; Cantonese and Zhuyin are, so they become the way back.
        preferences.inputScheme = @"cantonese";
        merged = [preferences sharedPreferencesByMerging:@{}];
        assert([merged[@"scheme"] isEqual:@"cantonese"]);
        assert([merged[@"last_chinese_scheme"] isEqual:@"cantonese"]);
        preferences.inputScheme = @"vietnamese";
        merged = [preferences sharedPreferencesByMerging:@{@"last_chinese_scheme": @"quanpin"}];
        assert([merged[@"scheme"] isEqual:@"vietnamese"]);
        assert([merged[@"last_chinese_scheme"] isEqual:@"cantonese"]);
        assert([preferences.lastChineseScheme isEqual:@"cantonese"]);
        preferences.inputScheme = @"zhuyin";
        assert([preferences.inputScheme isEqual:@"zhuyin"]);
        assert([preferences.lastChineseScheme isEqual:@"zhuyin"]);
        // Stroke is a Chinese scheme as well: picking it makes it the way back, and a shared last_chinese_scheme naming it is kept.
        preferences.inputScheme = @"stroke";
        assert([preferences.inputScheme isEqual:@"stroke"]);
        merged = [preferences sharedPreferencesByMerging:@{}];
        assert([merged[@"scheme"] isEqual:@"stroke"] && [merged[@"last_chinese_scheme"] isEqual:@"stroke"]);
        preferences.inputScheme = @"japanese";
        merged = [preferences sharedPreferencesByMerging:@{@"last_chinese_scheme": @"quanpin"}];
        assert([merged[@"last_chinese_scheme"] isEqual:@"stroke"] && [preferences.lastChineseScheme isEqual:@"stroke"]);
        preferences.inputScheme = @"quanpin";

        // removePersistentDomainForName: empties the domain and leaves the plist on disk, so every
        // run left one behind: 185 of them had piled up on the machine this was found on.
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    return 0;
}
