#import "../../src/settings/AppearancePreferences.h"
#import "PreferenceViewLookup.h"
#import "TestPreferenceSuite.h"
#include <cassert>

@interface MSIMEAppearancePreferences (QuickPhraseTestAPI)
- (NSString *)quickPhraseHotkey;
@end

static NSEvent *RecordKey(NSWindow *window, unsigned short code, NSEventModifierFlags flags) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0
        windowNumber:window.windowNumber context:nil characters:@"k" charactersIgnoringModifiers:@"k" isARepeat:NO keyCode:code];
}
static void CaptureCard(MSIMEAppearancePreferences *prefs, NSString *path) {
    [prefs showSettingsPageWithIdentifier:@"habits"];
    [prefs.window.contentView layoutSubtreeIfNeeded];
    NSView *card = MSIMEFindPreferenceView(prefs.window.contentView, ^BOOL(NSView *view) {
        return [view.accessibilityLabel isEqual:@"快捷模式卡片"];
    });
    assert(card);
    // 只截真实原生卡片，测试配置全部为合成值。
    const NSSize size = card.bounds.size;
    [card removeFromSuperview];
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,size.width,size.height)
        styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
    window.contentView = card;
    [window makeKeyAndOrderFront:nil];
    [card layoutSubtreeIfNeeded];
    [window displayIfNeeded];
    NSBitmapImageRep *bitmap = [card bitmapImageRepForCachingDisplayInRect:card.bounds];
    assert(bitmap);
    [card cacheDisplayInRect:card.bounds toBitmapImageRep:bitmap];
    assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:path atomically:YES]);
    [window orderOut:nil];
}
int main(int argc, const char **argv) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *suite = [@"msime.quick-phrase-settings." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        [prefs showWindow:nil];
        if (argc == 2) {
            CaptureCard(prefs, @(argv[1]));
            MSIMERemoveTestPreferenceSuite(defaults, suite);
            return 0;
        }
        NSButton *button = (id)MSIMEFindPreferenceView(prefs.window.contentView, ^BOOL(NSView *view) {
            return [view isKindOfClass:NSButton.class] && [view.accessibilityLabel isEqual:@"快捷短语附加快捷键"];
        });
        assert(button && [[prefs quickPhraseHotkey] isEqual:@""]);
        [button performClick:nil];
        [button keyDown:RecordKey(prefs.window, 40, NSEventModifierFlagControl)];
        assert([[prefs quickPhraseHotkey] isEqual:@"Ctrl+KeyK"]);
        NSDictionary *merged = [prefs sharedPreferencesByMerging:@{@"keybindings":@{@"toggle_punctuation":@"Ctrl+Period"}}];
        assert([merged[@"keybindings"][@"enter_quick_phrase"] isEqual:@"Ctrl+KeyK"]);
        assert([merged[@"keybindings"][@"toggle_punctuation"] isEqual:@"Ctrl+Period"]);
        MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        assert([[reopened quickPhraseHotkey] isEqual:@"Ctrl+KeyK"]);
        assert(reopened.quickPhraseToggleChord.bound);
        [button performClick:nil];
        [button keyDown:RecordKey(prefs.window, 47, NSEventModifierFlagControl)];
        assert([[prefs quickPhraseHotkey] isEqual:@"Ctrl+KeyK"]);
        [button performClick:nil];
        [button keyDown:RecordKey(prefs.window, 53, 0)];
        assert([[prefs quickPhraseHotkey] isEqual:@"Ctrl+KeyK"]);
        [button performClick:nil];
        [button keyDown:RecordKey(prefs.window, 51, 0)];
        assert([[prefs quickPhraseHotkey] isEqual:@""]);
        [prefs.window orderOut:nil];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
}
