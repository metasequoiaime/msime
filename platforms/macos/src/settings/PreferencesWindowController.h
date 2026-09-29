#pragma once
#import <AppKit/AppKit.h>

FOUNDATION_EXPORT NSNotificationName const MSIMEStandalonePreferencesDidCloseNotification;

/// Native preferences entry point; pages are added incrementally to this controller.
@interface MSIMEPreferencesWindowController : NSWindowController <NSWindowDelegate>
+ (instancetype)sharedController;
+ (NSString *)storedGlobalTheme;
+ (void)setStoredGlobalTheme:(NSString *)themeId;
/// The catalog title of a global theme id (系统, 水杉, …), or the id itself when the catalog does not know it. The account page's settings list shows theme ids through this.
+ (NSString *)themeTitleForIdentifier:(NSString *)themeId;
- (void)showAndActivate;
/// Presents the window on a named page — see -[MSIMEAppearancePreferences showSettingsPageWithIdentifier:]
/// for the names. This is the entry point for a native fallback whose desktop route names a page;
/// an entry point that just says "open settings" uses -showAndActivate, so the window opens where
/// the user left it.
- (void)showAndActivateWithPageIdentifier:(NSString *)identifier;
- (void)showAndActivateForStandaloneLaunch;
- (NSDictionary<NSString *, id> *)cloudSettingsSnapshot;
- (BOOL)validateCloudSettingsSnapshot:(NSDictionary<NSString *, id> *)values;
- (BOOL)applyCloudSettingsSnapshot:(NSDictionary<NSString *, id> *)values;
+ (NSDictionary<NSString *, id> *)cloudSettingsSnapshot;
+ (NSNumber *)validateCloudSettingsSnapshot:(NSDictionary<NSString *, id> *)values;
+ (NSNumber *)applyCloudSettingsSnapshot:(NSDictionary<NSString *, id> *)values;
@end
#define MetasequoiaPreferencesWindowController MSIMEPreferencesWindowController
