#import "../../src/input/InputMenu.h"

#import <AppKit/AppKit.h>

#include <stdexcept>

namespace
{
void require(bool condition, const char *message)
{
    if (!condition)
    {
        throw std::runtime_error(message);
    }
}
} // namespace

@interface PreferencesTarget : NSObject
@property(nonatomic) BOOL preferencesShown;
@property(nonatomic) BOOL updateCheckStarted;
@property(nonatomic) BOOL characterPaletteOpened;
@property(nonatomic) BOOL chineseSelected;
@property(nonatomic) BOOL englishSelected;
@property(nonatomic) BOOL simplifiedSelected;
@property(nonatomic) BOOL traditionalSelected;
@property(nonatomic) BOOL voicePanelShown;
@property(nonatomic) BOOL voiceSettingsShown;
@end

@implementation PreferencesTarget
- (void)showVoicePanel
{
    self.voicePanelShown = YES;
}
- (void)showVoiceSettings:(id)sender
{
    (void)sender;
    self.voiceSettingsShown = YES;
}
- (void)showPreferences:(id)sender
{
    (void)sender;
    self.preferencesShown = YES;
}

- (void)checkForUpdates:(id)sender
{
    (void)sender;
    self.updateCheckStarted = YES;
}

- (void)openCharacterPalette:(id)sender
{
    (void)sender;
    self.characterPaletteOpened = YES;
}

- (void)selectChineseMode:(id)sender
{
    (void)sender;
    self.chineseSelected = YES;
}

- (void)selectEnglishMode:(id)sender
{
    (void)sender;
    self.englishSelected = YES;
}

- (void)selectSimplifiedOutput:(id)sender
{
    (void)sender;
    self.simplifiedSelected = YES;
}

- (void)selectTraditionalOutput:(id)sender
{
    (void)sender;
    self.traditionalSelected = YES;
}
@end

int main()
{
    @autoreleasepool
    {
        [NSApplication sharedApplication];
        PreferencesTarget *target = [[PreferencesTarget alloc] init];
        NSMenu *menu = CreateMetasequoiaInputMenu(target, NO, NO);
        // macOS 的菜单只看颜色模式，它单独的明暗设置不读。
        ApplyMetasequoiaMenuTheme(menu, @{@"theme": @"light", @"menu_theme": @"dark"});
        assert(menu.appearance != nil);
        assert([[menu.appearance name] isEqualToString:NSAppearanceNameAqua]);
        ApplyMetasequoiaMenuTheme(menu, @{@"theme": @"dark", @"menu_theme": @"light"});
        assert([[menu.appearance name] isEqualToString:NSAppearanceNameDarkAqua]);
        ApplyMetasequoiaMenuTheme(menu, @{@"theme": @"system", @"menu_theme": @"follow"});
        assert(menu.appearance == nil);
        ApplyMetasequoiaMenuTheme(menu, @{@"theme": @"dark", @"menu_theme": @"invalid"});
        assert([[menu.appearance name] isEqualToString:NSAppearanceNameDarkAqua]);

        require(menu.numberOfItems == 12,
                "The input menu did not contain input modes, output character sets, character palette, update, and "
                "settings actions.");
        NSMenuItem *chineseItem = [menu itemAtIndex:0];
        NSMenuItem *englishItem = [menu itemAtIndex:1];
        require([chineseItem.title isEqualToString:@"中文输入"] &&
                    chineseItem.action == @selector(selectChineseMode:) && chineseItem.target == target &&
                    chineseItem.state == NSControlStateValueOn,
                "The Chinese input mode was not represented as selected.");
        require([englishItem.title isEqualToString:@"英文输入"] &&
                    englishItem.action == @selector(selectEnglishMode:) && englishItem.target == target &&
                    englishItem.state == NSControlStateValueOff,
                "The English input mode was not represented as unselected.");
        require([menu itemAtIndex:2].separatorItem, "The input modes were not separated from settings.");

        NSMenuItem *simplifiedItem = [menu itemAtIndex:3];
        NSMenuItem *traditionalItem = [menu itemAtIndex:4];
        require([simplifiedItem.title isEqualToString:@"简体输出"] &&
                    simplifiedItem.action == @selector(selectSimplifiedOutput:) && simplifiedItem.target == target &&
                    simplifiedItem.state == NSControlStateValueOn,
                "Simplified output was not represented as selected.");
        require([traditionalItem.title isEqualToString:@"繁体输出"] &&
                    traditionalItem.action == @selector(selectTraditionalOutput:) && traditionalItem.target == target &&
                    traditionalItem.state == NSControlStateValueOff,
                "Traditional output was not represented as unselected.");
        require([menu itemAtIndex:5].separatorItem,
                "The output character-set actions were not separated from utilities.");

        NSMenuItem *characterPaletteItem = [menu itemAtIndex:6];
        require([characterPaletteItem.title isEqualToString:@"表情与符号…"],
                "The character palette action title was incorrect.");
        require(characterPaletteItem.action == @selector(openCharacterPalette:),
                "The character palette action used the wrong selector.");
        require(characterPaletteItem.target == target && characterPaletteItem.enabled,
                "The character palette action was not enabled for its target.");

        NSMenuItem *updateItem = [menu itemAtIndex:7];
        require([updateItem.title isEqualToString:@"检查更新…"], "The update action title was incorrect.");
        require(updateItem.action == @selector(checkForUpdates:), "The update action used the wrong selector.");
        require(updateItem.target == target && updateItem.enabled, "The update action was not enabled for its target.");

        NSMenuItem *settingsItem = [menu itemAtIndex:8];
        require([settingsItem.title isEqualToString:@"水杉输入法设置…"], "The settings action title was incorrect.");
        require(settingsItem.action == @selector(showPreferences:), "The settings action used the wrong selector.");
        require(settingsItem.target == target, "The settings action did not target the input controller.");
        require(settingsItem.enabled, "The settings action was unexpectedly disabled.");

        [menu performActionForItemAtIndex:10];
        [menu performActionForItemAtIndex:11];
        require(target.voicePanelShown && target.voiceSettingsShown, "Voice menu actions were not dispatched.");
        [menu performActionForItemAtIndex:1];
        require(target.englishSelected, "The English input mode action was not dispatched.");
        NSMenu *englishMenu = CreateMetasequoiaInputMenu(target, YES, YES);
        require([englishMenu itemAtIndex:0].state == NSControlStateValueOff &&
                    [englishMenu itemAtIndex:1].state == NSControlStateValueOn,
                "The English input mode was not represented as selected.");
        require([englishMenu itemAtIndex:3].state == NSControlStateValueOff &&
                    [englishMenu itemAtIndex:4].state == NSControlStateValueOn,
                "The traditional output state was not represented as selected.");
        [englishMenu performActionForItemAtIndex:0];
        require(target.chineseSelected, "The Chinese input mode action was not dispatched.");

        [menu performActionForItemAtIndex:4];
        require(target.traditionalSelected, "The traditional output action was not dispatched.");
        [englishMenu performActionForItemAtIndex:3];
        require(target.simplifiedSelected, "The simplified output action was not dispatched.");
        [menu performActionForItemAtIndex:6];
        require(target.characterPaletteOpened, "The character palette action did not invoke openCharacterPalette:.");
        [menu performActionForItemAtIndex:7];
        require(target.updateCheckStarted, "The update action did not invoke checkForUpdates:.");
        [menu performActionForItemAtIndex:8];
        require(target.preferencesShown, "The settings action did not invoke showPreferences:.");
    }
    return 0;
}
