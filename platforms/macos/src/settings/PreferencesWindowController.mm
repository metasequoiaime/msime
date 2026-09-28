#import "PreferencesWindowController.h"
#import "AppearancePreferences.h"
#import "SettingsLayout.h"
#import "../candidate/CandidateSkinAppearance.h"
#import "../cloud/CloudAppearanceSettings.h"
#import "../core/WindowPresentation.h"

static NSString *const MSIMESchemeKey = @"MetasequoiaImeScheme";
static NSString *const MSIMEShuangpinSchemaKey = @"MetasequoiaImeShuangpinSchema";
NSNotificationName const MSIMEStandalonePreferencesDidCloseNotification =
    @"MSIMEStandalonePreferencesDidCloseNotification";

@implementation MSIMEPreferencesWindowController {
    BOOL _standaloneLaunch;
}
+ (NSDictionary *)cloudSettingsSnapshot { return [[MSIMEAppearancePreferences sharedPreferences] cloudSettingsSnapshot]; }
+ (NSNumber *)validateCloudSettingsSnapshot:(NSDictionary *)values { return @(MSIMEValidateCloudAppearance(values)); }
+ (NSNumber *)applyCloudSettingsSnapshot:(NSDictionary *)values {
    return @([[MSIMEAppearancePreferences sharedPreferences] applyCloudSettingsSnapshot:values]);
}
+ (NSString *)storedCandidateSkin { return MetasequoiaStoredCandidateSkin(); }
+ (void)setStoredCandidateSkin:(NSString *)skinId { MetasequoiaSetStoredCandidateSkin(skinId); }
+ (instancetype)sharedController {
    static MSIMEPreferencesWindowController *controller;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ controller = [[self alloc] initWithWindow:nil]; });
    return controller;
}
- (void)presentAndActivate {
    [[MSIMEAppearancePreferences sharedPreferences] showWindow:nil];
    NSWindow *window = [MSIMEAppearancePreferences sharedPreferences].window;
    // This controller is the one that presents the settings window and the one that decides what
    // closing it means, so it is the one that has to hold it: -window answered nil until now, and
    // every caller reaching through it — starting with the standalone launch that has to know which
    // window closing terminates the process — was reaching through nothing.
    self.window = window;
    window.delegate = self;
    // Only when the user has never placed this window. Centring unconditionally is what made the
    // saved frame pointless: the window came back the size it was left at, in the middle of the
    // screen, on every single presentation.
    if (!MSIMESettingsWindowHasSavedFrame()) [window center];
    MSIMEPresentWindow(window);
}
- (void)showAndActivate {
    _standaloneLaunch = NO;
    [self presentAndActivate];
}
- (void)showAndActivateWithPageIdentifier:(NSString *)identifier {
    [self showAndActivate];
    [[MSIMEAppearancePreferences sharedPreferences] showSettingsPageWithIdentifier:identifier];
}
- (void)showAndActivateForStandaloneLaunch {
    _standaloneLaunch = YES;
    [self presentAndActivate];
}
- (void)windowWillClose:(NSNotification *)notification {
    (void)notification;
    if (!_standaloneLaunch) return;
    _standaloneLaunch = NO;
    dispatch_async(dispatch_get_main_queue(), ^{
        [[NSNotificationCenter defaultCenter]
            postNotificationName:MSIMEStandalonePreferencesDidCloseNotification object:self];
    });
}
- (NSDictionary *)cloudSettingsSnapshot { return [self.class cloudSettingsSnapshot]; }
- (BOOL)validateCloudSettingsSnapshot:(NSDictionary *)values { return [[self.class validateCloudSettingsSnapshot:values] boolValue]; }
- (BOOL)applyCloudSettingsSnapshot:(NSDictionary *)values { return [[self.class applyCloudSettingsSnapshot:values] boolValue]; }
@end
