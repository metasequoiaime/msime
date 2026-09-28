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
    // The first presentation builds every settings page, which is long enough to read as a click that did nothing.
    const uint64_t started = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
    const BOOL built = [MSIMEAppearancePreferences sharedPreferences].isWindowLoaded;
    [[MSIMEAppearancePreferences sharedPreferences] showWindow:nil];
    NSWindow *window = [MSIMEAppearancePreferences sharedPreferences].window;
    os_log(MSIMEUILog(), "settings_window_shown standalone=%d already_built=%d elapsed_ms=%llu", _standaloneLaunch, built,
           (unsigned long long)((clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started) / 1000000));
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
    const uint64_t started = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
    [[MSIMEAppearancePreferences sharedPreferences] showSettingsPageWithIdentifier:identifier];
    os_log(MSIMEUILog(), "settings_page_shown page=%{public}@ elapsed_ms=%llu", identifier,
           (unsigned long long)((clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started) / 1000000));
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
