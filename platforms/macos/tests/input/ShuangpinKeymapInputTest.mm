#import "../../src/settings/ShuangpinKeymapPanel.h"
#import "../../src/candidate/CandidateWindowLevel.h"
#include <cassert>

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        MSIMEShuangpinKeymapPanel *panel = [MSIMEShuangpinKeymapPanel new];
        panel.releasedWhenClosed = NO;
        // 和候选窗同层级，截图工具的遮罩盖不住它（#7326）。
        assert(panel.level == MSIMECandidateWindowLevel() && panel.level > NSScreenSaverWindowLevel);
        [panel setProfileName:@"xiaohe"];
        for (NSString *preedit in @[@"hk", @"hao", @"hao ", @""]) {
            NSDictionary *view = @{@"editing_text": @"hk", @"preedit": preedit};
            assert([MSIMEShuangpinKeymapEditingText(view) isEqual:@"hk"]);
            assert([MSIMEShuangpinKeymapHighlightedKey(view) isEqual:@"k"]);
            [panel updateHighlightedKey:MSIMEShuangpinKeymapHighlightedKey(view)];
            assert([panel.contentView.accessibilityValue containsString:@"当前按键 K"]);
        }
        [panel setProfileName:@"microsoft"];
        NSDictionary *semicolon = @{@"editing_text": @"b;", @"preedit": @"bing"};
        [panel updateHighlightedKey:MSIMEShuangpinKeymapHighlightedKey(semicolon)];
        assert([panel.contentView.accessibilityValue containsString:@"当前按键 ;"]);
        assert([MSIMEShuangpinKeymapHighlightedKey(@{@"editing_text": @"H"}) isEqual:@"H"]);
        for (id raw in @[@"", NSNull.null, @42, @[]]) {
            NSDictionary *view = @{@"editing_text": raw, @"preedit": @"hao"};
            assert(MSIMEShuangpinKeymapEditingText(view).length == 0);
            assert(!MSIMEShouldShowShuangpinKeymap(YES, YES, MSIMEShuangpinKeymapEditingText(view).length > 0));
            assert(MSIMEShuangpinKeymapHighlightedKey(view).length == 0);
        }
        assert(MSIMEShuangpinKeymapEditingText(nil).length == 0);
        assert(MSIMEShuangpinKeymapHighlightedKey(@{@"preedit": @"hao"}).length == 0);
        for (NSString *raw in @[@"hk1", @"hk'", @"hk ", @"🙂"]) {
            [panel updateHighlightedKey:MSIMEShuangpinKeymapHighlightedKey(@{@"editing_text": raw})];
            assert(![panel.contentView.accessibilityValue containsString:@"当前按键"]);
        }
        // The highlighted key takes the theme accent the controller hands over, and keeps it across profile changes.
        assert(panel.accentColor != nil);
        NSColor *accent = [NSColor colorWithSRGBRed:0.36 green:0.61 blue:1.0 alpha:1.0];
        [panel setAccentColor:accent];
        assert([panel.accentColor isEqual:accent]);
        [panel setProfileName:@"ziranma"];
        assert([panel.accentColor isEqual:accent]);
        [panel close];
    }
}
