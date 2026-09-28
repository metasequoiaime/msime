#import "../../src/settings/AppearancePreferences.h"
#include <cassert>
#import "TestPreferenceSuite.h"

int main() {
    @autoreleasepool {
        NSString *suite = [@"MSIME.ToolbarVisibilityTest." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        __block NSUInteger notifications = 0;
        id token = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:preferences queue:nil usingBlock:^(NSNotification *note) { (void)note; ++notifications; }];
        assert(preferences.floatingToolbarEnabled);
        [preferences applySharedToolbarVisibility:NO];
        assert(!preferences.floatingToolbarEnabled && notifications == 0);
        assert([defaults objectForKey:@"MSIMEClientFloatingToolbarEnabled"] == nil);
        NSDictionary *merged = [preferences sharedPreferencesByMerging:@{}];
        assert(![merged[@"floating_toolbar"][@"enabled"] boolValue]);
        assert([merged[@"floating_toolbar"][@"english_mode"] boolValue]);
        assert([merged[@"floating_toolbar"][@"punctuation"] boolValue]);
        assert([merged[@"floating_toolbar"][@"fullwidth"] boolValue]);
        assert([merged[@"floating_toolbar"][@"character_set"] boolValue]);
        // Emoji is opt-in with the screen keyboard, so the merge publishes the compact toolbar.
        assert(![merged[@"floating_toolbar"][@"emoji"] boolValue]);
        assert(![merged[@"floating_toolbar"][@"screen_keyboard"] boolValue]);
        assert([merged[@"floating_toolbar"][@"settings"] boolValue]);
        assert([merged[@"floating_toolbar"][@"scale_percent"] integerValue] == 100);
        assert([merged[@"floating_toolbar"][@"font_size"] integerValue] == 24);
        preferences.floatingToolbarPunctuation = NO;
        preferences.floatingToolbarScreenKeyboard = YES;
        preferences.floatingToolbarScalePercent = 125;
        preferences.floatingToolbarFontSize = 28;
        assert(!preferences.floatingToolbarPunctuation && preferences.floatingToolbarScreenKeyboard);
        assert(preferences.floatingToolbarScalePercent == 125 && preferences.floatingToolbarFontSize == 28);
        merged = [preferences sharedPreferencesByMerging:@{}];
        assert(![merged[@"floating_toolbar"][@"punctuation"] boolValue]);
        assert([merged[@"floating_toolbar"][@"screen_keyboard"] boolValue]);
        assert([merged[@"floating_toolbar"][@"scale_percent"] integerValue] == 125);
        assert([merged[@"floating_toolbar"][@"font_size"] integerValue] == 28);
        [preferences applySharedToolbarPreferences:@{@"floating_toolbar": @{
            @"english_mode": @NO, @"punctuation": @YES, @"screen_keyboard": @NO, @"scale_percent": @75, @"font_size": @16}}];
        assert(preferences.floatingToolbarPunctuation && !preferences.floatingToolbarScreenKeyboard);
        assert(preferences.floatingToolbarScalePercent == 75 && preferences.floatingToolbarFontSize == 16);
        merged = [preferences sharedPreferencesByMerging:@{}];
        assert(![merged[@"floating_toolbar"][@"english_mode"] boolValue]);
        assert([defaults objectForKey:@"MSIMEClientFloatingToolbarOptions"] != nil);
        NSUInteger beforeSharedVisibility = notifications;
        [preferences applySharedToolbarVisibility:YES];
        assert(preferences.floatingToolbarEnabled && notifications == beforeSharedVisibility);
        preferences.floatingToolbarEnabled = NO;
        assert(!preferences.floatingToolbarEnabled && notifications == beforeSharedVisibility + 1);
        [preferences applySharedToolbarVisibility:YES];
        assert(preferences.floatingToolbarEnabled && notifications == beforeSharedVisibility + 1);
        [NSNotificationCenter.defaultCenter removeObserver:token];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
}
