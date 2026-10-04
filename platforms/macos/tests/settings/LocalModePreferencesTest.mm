#import "../../src/settings/AppearancePreferences.h"
#import "MSIMEClientSession.h"
#include "msime_client.h"
#include <cassert>
#import "TestPreferenceSuite.h"
#import "PreferenceViewLookup.h"

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *suite = [@"msime.local-modes." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
        NSDictionary *base = @{@"local_modes": @{@"unicode": @NO, @"future_field": @42}, @"untouched": @7};
        assert([[prefs sharedPreferencesByMerging:base][@"local_modes"] isEqual:base[@"local_modes"]]);
        __block NSUInteger changes = 0;
        id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++changes; }];
        [prefs applySharedLocalModes:@{@"unicode": @NO}];
        assert(![prefs localModeEnabled:@"unicode"] && changes == 0);
        assert([defaults objectForKey:@"MSIMEClientLocalModes"] == nil);
        [prefs applySharedAssistancePreferences:@{@"fuzzy_pinyin": @{@"enabled": @NO, @"rules": @[@"z-zh", @"an-ang"]}}];
        assert(!prefs.fuzzyPinyinEnabled && [prefs fuzzyPinyinRuleEnabled:@"z-zh"] && ![prefs fuzzyPinyinRuleEnabled:@"c-ch"]);
        [prefs applySharedAssistancePreferences:@{@"learning": @NO, @"frequency": @{@"mode": @"linear", @"trigger_count": @3, @"linear_step": @4}}];
        assert(!prefs.candidateLearningEnabled && [prefs.frequencyAdjustmentMode isEqual:@"linear"] &&
               prefs.frequencyTriggerCount == 3 && prefs.frequencyLinearStep == 4);
        [prefs applySharedLocalModes:@{@"unicode": @1}];
        assert(![prefs localModeEnabled:@"unicode"]);
        NSView *prefsRoot = prefs.window.contentView;
        NSMutableDictionary<NSString *, NSButton *> *buttons = [NSMutableDictionary dictionary];
        for (NSControl *control in MSIMEFindPreferenceControls(prefsRoot, NSSelectorFromString(@"localModeChanged:")))
            buttons[control.identifier] = (id)control;
        assert(buttons.count == 8);
        NSButton *fuzzy = (id)MSIMEFindPreferenceControl(prefsRoot, @selector(fuzzyPinyinChanged:));
        NSView *fuzzyCard = MSIMEFindPreferenceView(prefsRoot, ^BOOL(NSView *view) {
            return [view.accessibilityLabel isEqual:@"模糊音卡片（全拼与双拼）"];
        });
        NSMutableDictionary<NSString *, NSButton *> *fuzzyRules = [NSMutableDictionary dictionary];
        for (NSControl *control in MSIMEFindPreferenceControls(prefsRoot, @selector(fuzzyPinyinRuleChanged:)))
            fuzzyRules[control.identifier] = (id)control;
        assert(fuzzy != nil && fuzzyCard != nil && fuzzyRules.count == 11 && !fuzzyRules[@"z-zh"].enabled);
        fuzzy.state = NSControlStateValueOn;
        [NSApp sendAction:fuzzy.action to:fuzzy.target from:fuzzy];
        assert(prefs.fuzzyPinyinEnabled && fuzzyRules[@"z-zh"].enabled);
        fuzzyRules[@"z-zh"].state = NSControlStateValueOff;
        [NSApp sendAction:fuzzyRules[@"z-zh"].action to:fuzzyRules[@"z-zh"].target from:fuzzyRules[@"z-zh"]];
        assert(![prefs fuzzyPinyinRuleEnabled:@"z-zh"] && [prefs fuzzyPinyinRuleEnabled:@"an-ang"]);
        assert([[prefs sharedPreferencesByMerging:base][@"fuzzy_pinyin"][@"enabled"] isEqual:@YES]);
        assert([[prefs sharedPreferencesByMerging:base][@"fuzzy_pinyin"][@"rules"] isEqual:@[@"an-ang"]]);
        NSButton *learning = (id)MSIMEFindPreferenceControl(prefsRoot, @selector(candidateLearningChanged:));
        NSPopUpButton *frequencyMode = (id)MSIMEFindPreferenceControl(prefsRoot, @selector(frequencyModeChanged:));
        NSPopUpButton *frequencyTrigger = (id)MSIMEFindPreferenceControl(prefsRoot, @selector(frequencyTriggerChanged:));
        NSPopUpButton *frequencyStep = (id)MSIMEFindPreferenceControl(prefsRoot, @selector(frequencyStepChanged:));
        assert(learning != nil && learning.state == NSControlStateValueOff);
        assert(frequencyMode != nil && frequencyTrigger != nil && frequencyStep != nil);
        learning.state = NSControlStateValueOn;
        [NSApp sendAction:learning.action to:learning.target from:learning];
        [frequencyMode selectItemAtIndex:2];
        [NSApp sendAction:frequencyMode.action to:frequencyMode.target from:frequencyMode];
        [frequencyTrigger selectItemAtIndex:2];
        [NSApp sendAction:frequencyTrigger.action to:frequencyTrigger.target from:frequencyTrigger];
        [frequencyStep selectItemAtIndex:3];
        [NSApp sendAction:frequencyStep.action to:frequencyStep.target from:frequencyStep];
        NSDictionary *assistance = [prefs sharedPreferencesByMerging:base];
        assert([assistance[@"learning"] isEqual:@YES]);
        assert(([assistance[@"frequency"] isEqual:@{@"mode": @"halve", @"trigger_count": @3, @"linear_step": @4}]));
        for (NSString *mode in @[@"unicode", @"date_time", @"quick_phrase", @"emoji", @"kaomoji", @"super_jianpin", @"temporary_english", @"temporary_japanese"]) {
            NSButton *button = buttons[mode];
            assert(button && [prefs localModeEnabled:mode] == ![mode isEqual:@"unicode"]);
            NSUInteger before = changes;
            button.state = NSControlStateValueOff;
            [NSApp sendAction:button.action to:button.target from:button];
            assert(changes == before + 1 && ![prefs localModeEnabled:mode]);
            assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:prefs.skinsRoot] localModeEnabled:mode]);
            assert([[prefs sharedPreferencesByMerging:base][@"local_modes"][mode] isEqual:@NO]);
            button.state = NSControlStateValueOn;
            [NSApp sendAction:button.action to:button.target from:button];
            assert([prefs localModeEnabled:mode]);
        }
        assert([[prefs sharedPreferencesByMerging:base][@"local_modes"][@"future_field"] isEqual:@42]);
        [prefs applySharedLocalModes:@{@"unicode": @NO}];
        assert(buttons[@"unicode"].state == NSControlStateValueOff);
        assert([[prefs sharedPreferencesByMerging:base][@"local_modes"][@"unicode"] isEqual:@NO]);
        [prefs setLocalMode:@"unicode" enabled:YES];
        NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5, @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
        for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
            NSString *path = [root stringByAppendingPathComponent:name];
            assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
            options[name] = path;
        }
        // Four of the eight modes are gated on a runtime resource as well as on the preference: emoji and
        // kaomoji read others.db, temporary English reads msime-english.db, temporary Japanese reads
        // msime-japanese.dat. apply_local_mode_resource_gates turns the mode off when the file is absent, so
        // that a missing optional resource makes Shift+E insert a capital E rather than swallow the key.
        //
        // Without these, this test asserted something that could not hold, and it had been read as the
        // Engine refusing to enter the mode. The gate only asks whether the file is there, so empty files
        // are enough to let the preference plumbing this test is actually about run for all eight modes -
        // and keeping them empty keeps the test off the fetched dictionaries.
        NSString *resources = options[@"resources"];
        NSArray<NSString *> *gated = @[@"others.db", @"msime-english.db", @"msime-japanese.dat"];
        for (NSString *resource in gated)
            assert([NSFileManager.defaultManager createFileAtPath:[resources stringByAppendingPathComponent:resource]
                                                         contents:[NSData data] attributes:nil]);
        NSError *error = nil;
        MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
        assert(session && !error && [session setFocused:YES error:&error]);
        NSDictionary *shared = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert(shared && !error);
        NSUInteger revision = 0;
        NSArray<NSArray<NSString *> *> *modes = @[@[@"unicode", @"U"], @[@"date_time", @"T"],
            @[@"quick_phrase", @"K"], @[@"emoji", @"E"], @[@"kaomoji", @"M"],
            @[@"super_jianpin", @"J"], @[@"temporary_english", @"Y"], @[@"temporary_japanese", @"R"]];
        for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
            prefs.inputScheme = scheme;
            NSDictionary *(^snapshot)(NSUInteger) = ^(NSUInteger nextRevision) {
                return @{@"format_version": @1, @"revision": @(nextRevision),
                    @"preferences": [prefs sharedPreferencesByMerging:shared[@"preferences"]]};
            };
            for (NSArray<NSString *> *entry in modes) {
                NSString *mode = entry[0];
                uint8_t trigger = (uint8_t)[entry[1] characterAtIndex:0];
                // Build each revision explicitly so both schemes exercise the same session.
                NSDictionary *enabled = snapshot(++revision);
                assert([[session updatePreferencesSnapshot:enabled error:&error][@"deferred"] isEqual:@NO]);
                NSDictionary *before = [session typeASCII:trigger shift:YES error:&error][@"view"];
                assert([before[@"local_mode"] isEqual:mode]);
                assert([before[@"scheme"] isEqual:[scheme isEqual:@"quanpin"] ? @0 : @1]);
                buttons[mode].state = NSControlStateValueOff;
                [NSApp sendAction:buttons[mode].action to:buttons[mode].target from:buttons[mode]];
                NSDictionary *disabled = snapshot(++revision);
                assert([[session updatePreferencesSnapshot:disabled error:&error][@"deferred"] isEqual:@YES]);
                assert([[session viewWithError:&error][@"editing_text"] isEqual:before[@"editing_text"]]);
                assert([[session viewWithError:&error][@"local_mode"] isEqual:mode]);
                assert([session command:MSIME_CANCEL error:&error]);
                assert([[session updatePreferencesSnapshot:disabled error:&error][@"deferred"] isEqual:@NO]);
                assert(![[session typeASCII:trigger shift:YES error:&error][@"view"][@"local_mode"] isEqual:mode]);
                assert([session command:MSIME_CANCEL error:&error]);
                buttons[mode].state = NSControlStateValueOn;
                [NSApp sendAction:buttons[mode].action to:buttons[mode].target from:buttons[mode]];
                NSDictionary *restored = snapshot(++revision);
                assert([[session updatePreferencesSnapshot:restored error:&error][@"deferred"] isEqual:@NO]);
                assert([[session typeASCII:trigger shift:YES error:&error][@"view"][@"local_mode"] isEqual:mode]);
                assert([session command:MSIME_CANCEL error:&error]);
            }
        }
        assert([session closeWithError:&error] && !error);

        // The other half of the same contract, which nothing here covered: with the resource gone the mode
        // stays off however the preference is set, and the trigger key falls back to inserting its capital
        // rather than being swallowed. A session reads the gate when it applies a snapshot, so this needs a
        // fresh one rather than another revision on the session above.
        for (NSString *resource in gated)
            assert([NSFileManager.defaultManager removeItemAtPath:[resources stringByAppendingPathComponent:resource]
                                                            error:nil]);
        MSIMEClientSession *ungated = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
        assert(ungated && !error && [ungated setFocused:YES error:&error]);
        prefs.inputScheme = @"quanpin";
        NSDictionary *all = @{@"format_version": @1, @"revision": @(++revision),
            @"preferences": [prefs sharedPreferencesByMerging:shared[@"preferences"]]};
        assert([[ungated updatePreferencesSnapshot:all error:&error][@"deferred"] isEqual:@NO]);
        for (NSArray<NSString *> *entry in @[@[@"emoji", @"E"], @[@"kaomoji", @"M"],
                                             @[@"temporary_english", @"Y"], @[@"temporary_japanese", @"R"]]) {
            uint8_t trigger = (uint8_t)[entry[1] characterAtIndex:0];
            NSDictionary *reply = [ungated typeASCII:trigger shift:YES error:&error];
            assert(![reply[@"view"][@"local_mode"] isEqual:entry[0]]);
            // Not swallowed: unhandled is what leaves the application to insert the capital itself.
            assert([reply[@"handled"] isEqual:@NO]);
            assert([reply[@"view"][@"editing_text"] isEqual:@""]);
            assert([ungated command:MSIME_CANCEL error:&error]);
        }
        assert([ungated closeWithError:&error] && !error);
        [NSNotificationCenter.defaultCenter removeObserver:observer];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
    }
}
