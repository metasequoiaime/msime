#import "../../src/settings/AppearancePreferences.h"
#import "../../src/candidate/SkinSettingsView.h"
#import "../../src/candidate/CandidateSkinPreviewView.h"
#include <cassert>
#include <fstream>
#import "../settings/TestPreferenceSuite.h"
#import "../settings/PreferenceViewLookup.h"

static void WritePackage(const std::filesystem::path &root) {
    std::filesystem::create_directories(root / "synthetic");
    std::ofstream file(root / "synthetic" / "skin.toml");
    file << R"toml(schema_version = 1
id = "synthetic"
name = "Synthetic Card"
version = "1"
description = "Synthetic package fixture"
base = "system"
[supports]
layouts = ["horizontal"]
themes = ["dark", "light"]
[candidate_window]
min_width_dip = 0
[candidate_window.decoration]
top_inset_dip = 0
width_dip = 0
)toml";
    assert(file.good());
}

// A single-mode horizontal package over `base`, for the compatibility rule.
static void WriteSingleModePackage(const std::filesystem::path &root, const std::string &id, const std::string &base, const std::string &mode) {
    std::filesystem::create_directories(root / id);
    std::ofstream file(root / id / "skin.toml");
    file << "schema_version = 1\nid = \"" << id << "\"\nname = \"" << id << "\"\nversion = \"1\"\nbase = \"" << base
         << "\"\n[supports]\nlayouts = [\"horizontal\"]\nthemes = [\"" << mode << "\"]\n[candidate_window]\nmin_width_dip = 0\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n";
    assert(file.good());
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        char temporary[] = "/tmp/msime-skin-cards-test-XXXXXX";
        assert(mkdtemp(temporary));
        const std::filesystem::path root = std::filesystem::path(temporary) / "skins";
        NSString *suite = [@"app.msime.test.skin-cards." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        MetasequoiaSkinSettingsView *cards = [[MetasequoiaSkinSettingsView alloc] initWithFrame:NSMakeRect(0, 0, 700, 700) preferences:preferences];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 700, 700) styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        [window.contentView addSubview:cards];
        [NSLayoutConstraint activateConstraints:@[
            [cards.leadingAnchor constraintEqualToAnchor:window.contentView.leadingAnchor],
            [cards.trailingAnchor constraintEqualToAnchor:window.contentView.trailingAnchor],
            [cards.topAnchor constraintEqualToAnchor:window.contentView.topAnchor],
            [cards.bottomAnchor constraintEqualToAnchor:window.contentView.bottomAnchor]
        ]];
        NSArray<NSSwitch *> *switches = [cards valueForKey:@"switches"];
        NSArray<MSIMECandidatePreviewView *> *previews = [cards valueForKey:@"previews"];
        NSArray<NSButton *> *themes = [cards valueForKey:@"themeButtons"];
        NSTextField *diagnostics = [cards valueForKey:@"diagnosticsLabel"];
        NSTextField *empty = [cards valueForKey:@"emptyLabel"];
        // One card per global theme, in the shared catalog's order, ahead of any package.
        assert(switches.count == 7 && previews.count == 7 && !empty.hidden && diagnostics.hidden);
        NSArray<NSString *> *catalogIds = @[ @"system", @"shuishan", @"light", @"paper", @"night", @"ink", @"custom" ];
        for (NSUInteger index = 0; index < 7; ++index) assert([switches[index].identifier isEqual:catalogIds[index]]);
        // A fresh install selects 跟随系统, the default the shared preferences give an unset theme.
        for (NSSwitch *card in switches)
            assert(card.state == ([card.identifier isEqual:@"system"] ? NSControlStateValueOn : NSControlStateValueOff));
        [window.contentView layoutSubtreeIfNeeded];
        assert(!cards.hasAmbiguousLayout && ![[cards valueForKey:@"externalCards"] hasAmbiguousLayout]);
        __block NSUInteger changes = 0;
        id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:preferences queue:nil usingBlock:^(NSNotification *note) { (void)note; ++changes; }];
        for (NSUInteger index = 0; index < 7; ++index) {
            [NSApp sendAction:switches[index].action to:switches[index].target from:switches[index]];
            assert([preferences.globalTheme isEqual:switches[index].identifier]);
            assert([[[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:preferences.skinsRoot] globalTheme] isEqual:preferences.globalTheme]);
            assert([preferences resolvedSkinForDark:NO].id == preferences.globalTheme.UTF8String);
            for (NSUInteger other = 0; other < 7; ++other) assert(switches[other].state == (other == index ? NSControlStateValueOn : NSControlStateValueOff));
            [switches[index] performClick:nil];
            assert(switches[index].state == NSControlStateValueOn);
            NSDictionary *before = [defaults persistentDomainForName:suite];
            NSUInteger count = changes;
            BOOL dark = previews[index].previewUsesDark;
            const BOOL fixed = previews[index].previewSkin.fixedDark.has_value();
            // The five built-in themes fix their mode, so only 跟随系统 and 自定义 offer the other mode to preview.
            assert(fixed == (index >= 1 && index <= 5) && themes[index].hidden == fixed);
            [NSApp sendAction:themes[index].action to:themes[index].target from:themes[index]];
            assert(previews[index].previewUsesDark == (fixed ? dark : !dark));
            assert([[defaults persistentDomainForName:suite] isEqual:before] && changes == count);
        }
        // Opening is scoped to the injected directory; Finder is replaced in native tests.
        __block NSUInteger opens = 0;
        cards.directoryOpener = ^BOOL(NSURL *url) {
            assert([url isEqual:preferences.skinsRoot]);
            assert(std::filesystem::is_directory(root));
            ++opens;
            return YES;
        };
        assert(!std::filesystem::exists(root));
        [NSApp sendAction:NSSelectorFromString(@"openDirectory:") to:cards from:nil];
        assert(opens == 1 && std::filesystem::is_directory(root));
        WritePackage(root);
        std::filesystem::create_directory(root / "broken");
        { std::ofstream invalid(root / "broken" / "skin.toml"); invalid << "schema_version = 2\n"; }
        for (NSUInteger iteration = 0; iteration < 3; ++iteration) {
            [cards reload];
            assert(switches.count == 8 && previews.count == 8 && !diagnostics.hidden && empty.hidden);
            assert([diagnostics.stringValue containsString:@"1 个"] && [diagnostics.stringValue containsString:@"broken"]);
            [window.contentView layoutSubtreeIfNeeded];
            assert(![[cards valueForKey:@"externalCards"] hasAmbiguousLayout]);
        }
        // A package card makes the custom theme draw that package over the package's own base.
        [NSApp sendAction:switches.lastObject.action to:switches.lastObject.target from:switches.lastObject];
        assert([preferences.globalTheme isEqual:@"custom"] && [preferences.customCandidateSkin isEqual:@"synthetic"] &&
               [preferences.customThemeBase isEqual:@"system"]);
        assert([preferences resolvedSkinForDark:NO].candidateSkin == "synthetic");
        assert(switches.lastObject.state == NSControlStateValueOn);
        assert(previews.lastObject.previewSkin.candidateSkin == "synthetic");
        // The package is drawn over the custom theme, so the custom card is on beside it, as in the React host.
        for (NSUInteger index = 0; index < 6; ++index) assert(switches[index].state == NSControlStateValueOff);
        assert(switches[6].state == NSControlStateValueOn);
        // 不使用外部皮肤 drops only the package: the custom theme stays, and its card is the one left on.
        NSButton *detach = [cards valueForKey:@"detachSkinButton"];
        assert(detach.enabled && [detach.accessibilityLabel isEqual:@"自定义主题不使用外部皮肤"]);
        [NSApp sendAction:detach.action to:detach.target from:detach];
        assert([preferences.globalTheme isEqual:@"custom"] && preferences.customCandidateSkin == nil && !detach.enabled);
        assert(switches[6].state == NSControlStateValueOn && switches.lastObject.state == NSControlStateValueOff);
        [NSApp sendAction:switches.lastObject.action to:switches.lastObject.target from:switches.lastObject];
        assert([preferences.customCandidateSkin isEqual:@"synthetic"] && switches.lastObject.state == NSControlStateValueOn);
        // Leaving for a built-in theme and coming back through the custom card selects the custom theme as it stands: the package is still drawn (THEME_CONTRACT §5).
        [NSApp sendAction:switches[1].action to:switches[1].target from:switches[1]];
        assert([preferences.globalTheme isEqual:@"shuishan"] && switches.lastObject.state == NSControlStateValueOff);
        [NSApp sendAction:switches[6].action to:switches[6].target from:switches[6]];
        assert([preferences.globalTheme isEqual:@"custom"] && [preferences.customCandidateSkin isEqual:@"synthetic"]);
        assert([preferences resolvedSkinForDark:NO].candidateSkin == "synthetic");
        assert(switches[6].state == NSControlStateValueOn && switches.lastObject.state == NSControlStateValueOn);
        // A package over a built-in base is drawn in that base's mode, so a light-only package over 夜色 is selectable in either host mode; over 跟随系统 the host mode still has to be one it lists. The layout rules out both.
        NSSwitch *(^card)(NSString *) = ^NSSwitch *(NSString *identifier) {
            for (NSSwitch *candidate in switches)
                if ([candidate.identifier isEqual:identifier]) return candidate;
            assert(false);
            return nil;
        };
        WriteSingleModePackage(root, "a-night-light", "night", "light");
        WriteSingleModePackage(root, "b-system-light", "system", "light");
        for (NSString *host in @[NSAppearanceNameDarkAqua, NSAppearanceNameAqua]) {
            window.appearance = [NSAppearance appearanceNamed:host];
            [cards reload];
            assert(switches.count == 10 && [diagnostics.stringValue containsString:@"1 个"]);
            assert(card(@"a-night-light").enabled && card(@"b-system-light").enabled == [host isEqual:NSAppearanceNameAqua]);
            preferences.vertical = YES;
            [cards reload];
            assert(!card(@"a-night-light").enabled && !card(@"b-system-light").enabled);
            preferences.vertical = NO;
        }
        // Over a system base the candidate window is drawn in the mode 候选窗主题 picks, not the one the settings window happens to be in, and not the fixed mode of whatever built-in theme is on screen now.
        WriteSingleModePackage(root, "c-system-dark", "system", "dark");
        window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
        preferences.candidateTheme = @"dark";
        [cards reload];
        assert(switches.count == 11 && card(@"c-system-dark").enabled && !card(@"b-system-light").enabled);
        preferences.candidateTheme = @"light";
        preferences.globalTheme = @"night";
        window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
        [cards reload];
        assert(!card(@"c-system-dark").enabled && card(@"b-system-light").enabled && card(@"a-night-light").enabled);
        preferences.candidateTheme = @"follow";
        preferences.globalTheme = @"custom";
        window.appearance = nil;
        std::filesystem::remove_all(root / "a-night-light");
        std::filesystem::remove_all(root / "b-system-light");
        std::filesystem::remove_all(root / "c-system-dark");
        [cards reload];
        assert(switches.count == 8 && card(@"synthetic").state == NSControlStateValueOn);
        preferences.vertical = YES;
        [cards reload];
        assert(!switches.lastObject.enabled &&
               [switches.lastObject.accessibilityValue isEqual:@"当前布局或明暗模式不受支持"]);
        preferences.vertical = NO;
        [cards reload];
        assert(switches.lastObject.enabled);
        std::filesystem::remove_all(root / "synthetic");
        [cards reload];
        assert(switches.count == 7 && !empty.hidden);
        // A package that is gone is not drawn, but the choice is kept for when it comes back.
        assert([preferences.customCandidateSkin isEqual:@"synthetic"] && [preferences resolvedSkinForDark:NO].id == "custom" &&
               [preferences resolvedSkinForDark:NO].candidateSkin.empty());
        cards.directoryOpener = ^BOOL(NSURL *url) { (void)url; return NO; };
        [NSApp sendAction:NSSelectorFromString(@"openDirectory:") to:cards from:nil];
        assert([diagnostics.stringValue isEqual:@"无法打开皮肤目录。"] && !diagnostics.hidden);
        // Construction of a directory cannot replace a regular file.
        std::filesystem::remove_all(root);
        { std::ofstream file(root); file << "synthetic sentinel"; }
        [NSApp sendAction:NSSelectorFromString(@"openDirectory:") to:cards from:nil];
        assert([diagnostics.stringValue containsString:@"无法创建"] && std::filesystem::is_regular_file(root));
        std::filesystem::remove(root);
        [cards reload];
        preferences.globalTheme = @"system";
        [window.contentView layoutSubtreeIfNeeded];
        if (argc == 2) {
            NSBitmapImageRep *bitmap = [cards bitmapImageRepForCachingDisplayInRect:cards.bounds];
            [cards cacheDisplayInRect:cards.bounds toBitmapImageRep:bitmap];
            assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:@(argv[1]) atomically:YES]);
        }
        NSButton *browse = (id)MSIMEFindPreferenceControl(preferences.window.contentView,
                                                          NSSelectorFromString(@"showSkinCatalog:"));
        // The remaining entry is a trip to the shared settings application, not the control that
        // picks a skin: the browser is the 皮肤 page itself.
        assert(browse && [browse.title isEqual:@"在设置应用中打开…"] && [preferences respondsToSelector:browse.action]);
        MetasequoiaSkinSettingsView *catalogView = (id)[preferences skinSettingsView];
        assert([catalogView isKindOfClass:MetasequoiaSkinSettingsView.class]);
        assert([catalogView valueForKey:@"preferences"] == preferences);
        assert([preferences skinSettingsView] == catalogView && catalogView.window == preferences.window);
        [preferences.window.contentView layoutSubtreeIfNeeded];
        assert(catalogView.frame.size.width > 0 && catalogView.frame.size.height > 0);
        for (NSString *theme in @[NSAppearanceNameDarkAqua, NSAppearanceNameAqua]) {
            preferences.window.appearance = [NSAppearance appearanceNamed:theme];
            [catalogView viewDidChangeEffectiveAppearance];
            NSArray<NSTextField *> *titles = [catalogView valueForKey:@"titles"];
            assert([titles.firstObject.stringValue containsString:[theme isEqual:NSAppearanceNameDarkAqua] ? @"Dark" : @"Light"]);
        }
        [NSNotificationCenter.defaultCenter removeObserver:observer];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        std::filesystem::remove_all(std::filesystem::path(temporary));
    }
}
