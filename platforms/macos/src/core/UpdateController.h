#pragma once

#import <AppKit/AppKit.h>

NS_ASSUME_NONNULL_BEGIN

@protocol MetasequoiaUpdateDriver <NSObject>
@property(nonatomic, readonly) BOOL canCheckForUpdates;
@property(nonatomic, readonly) BOOL automaticallyChecksForUpdates;
- (void)checkForUpdates:(nullable id)sender;
@optional
// Installs an update that was downloaded in the background, if one is waiting. Ends this process
// without relaunching it; the system starts the input method again from the new bundle when it is next
// needed.
- (void)installDownloadedUpdate;
@end

typedef void (^MetasequoiaUpdateActivationHandler)(void);

typedef NS_ENUM(NSInteger, MetasequoiaUpdateRoute) {
    MetasequoiaUpdateRouteUnavailable,
    MetasequoiaUpdateRouteSparkle,
    MetasequoiaUpdateRouteReleasePage,
};

// Whether Sparkle can be started in this process at all. It needs an application bundle - a feed URL, a
// version, a code signature - and started anywhere else it reports the misconfiguration with a modal
// alert, which in an input method means the user's typing stops behind a dialog they never asked for.
// A pure function so the decision can be tested without starting an updater.
static inline BOOL MSIMEUpdateHostCanStartSparkle(NSString *_Nullable identifier, NSString *_Nullable path,
                                                  NSString *_Nullable feedURL)
{
    return identifier.length > 0 && [path.pathExtension isEqualToString:@"app"] && feedURL.length > 0;
}

// A shipped application with no Sparkle feed must still give an honest, actionable result when the
// shared settings bundle cannot be launched. Test binaries and command-line helpers have neither
// route: presenting AppKit update UI from them would steal focus for an action they did not initiate.
static inline MetasequoiaUpdateRoute MSIMEUpdateRouteForHost(NSString *_Nullable identifier,
                                                             NSString *_Nullable path,
                                                             NSString *_Nullable feedURL)
{
    if (MSIMEUpdateHostCanStartSparkle(identifier, path, feedURL)) return MetasequoiaUpdateRouteSparkle;
    if (identifier.length > 0 && [path.pathExtension isEqualToString:@"app"])
        return MetasequoiaUpdateRouteReleasePage;
    return MetasequoiaUpdateRouteUnavailable;
}

// Whether this process is a release build with a Sparkle feed, so update checks and background installs
// go through Sparkle rather than the settings app's download page.
static inline BOOL MSIMEUpdateHostUsesSparkle(NSBundle *bundle)
{
    return MSIMEUpdateRouteForHost(bundle.bundleIdentifier, bundle.bundlePath,
                                   [bundle objectForInfoDictionaryKey:@"SUFeedURL"]) == MetasequoiaUpdateRouteSparkle;
}

typedef NSModalResponse (^MetasequoiaUpdateReleaseConfirmation)(NSURL *releaseURL);
typedef BOOL (^MetasequoiaUpdateReleaseOpener)(NSURL *releaseURL);
typedef void (^MetasequoiaUpdateReleaseFailure)(void);

// Public only so the no-feed failure path can be tested without opening a browser or showing a real
// alert. Production constructs it with fixed UI blocks and a fixed official release URL.
@interface MetasequoiaReleasePageUpdateDriver : NSObject <MetasequoiaUpdateDriver>
- (instancetype)initWithReleaseURL:(NSURL *)releaseURL
                       confirmation:(MetasequoiaUpdateReleaseConfirmation)confirmation
                             opener:(MetasequoiaUpdateReleaseOpener)opener
                            failure:(MetasequoiaUpdateReleaseFailure)failure;
@end

@interface MetasequoiaUpdateController : NSObject
+ (instancetype)sharedController;
- (instancetype)initWithDriver:(id<MetasequoiaUpdateDriver>)driver
             activationHandler:(MetasequoiaUpdateActivationHandler)activationHandler;
@property(nonatomic, readonly) BOOL canCheckForUpdates;
@property(nonatomic, readonly) BOOL automaticallyChecksForUpdates;
- (void)checkForUpdates:(nullable id)sender;
// Hands a background-downloaded update to the driver for installation. Never activates the process or
// shows anything: it is called when the user has just switched to another input source.
- (void)installDownloadedUpdateIfReady;
@end
#define MSIMEUpdateController MetasequoiaUpdateController

NS_ASSUME_NONNULL_END
