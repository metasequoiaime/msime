#pragma once

#import <AppKit/AppKit.h>

#import "UpdateChannelFeed.h"

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
// 偏好 update_channel 读出的通道（stable、beta、nightly）。Sparkle 驱动每次取 feed 时按它选 appcast（UpdateChannelFeed.h），通道变了就请 Sparkle 重排检查周期；其他驱动没有通道的概念，不实现它。
- (void)setUpdateChannel:(NSString *)channel;
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
// 输入法进程启动时用：带着偏好里的 update_channel 创建共享控制器，Sparkle 启动之前驱动就知道通道，它安排的第一次检查不会先按稳定版取 feed。控制器已经存在时等同于 applyUpdateChannelPreference:。只在主线程调用。
+ (instancetype)sharedControllerWithUpdateChannelPreference:(nullable id)preference;
- (instancetype)initWithDriver:(id<MetasequoiaUpdateDriver>)driver
             activationHandler:(MetasequoiaUpdateActivationHandler)activationHandler;
@property(nonatomic, readonly) BOOL canCheckForUpdates;
@property(nonatomic, readonly) BOOL automaticallyChecksForUpdates;
- (void)checkForUpdates:(nullable id)sender;
// Hands a background-downloaded update to the driver for installation. Never activates the process or
// shows anything: it is called when the user has just switched to another input source.
- (void)installDownloadedUpdateIfReady;
// 偏好文档里 update_channel 的原值（缺省、未知值按稳定版，见 MSIMEUpdateChannelFromPreference）。和上次相同时什么也不做，不同时交给驱动。只在主线程调用。
- (void)applyUpdateChannelPreference:(nullable id)preference;
// 最近一次交给驱动的通道；还没有读到偏好时为 nil。
@property(nonatomic, readonly, copy, nullable) NSString *updateChannel;
@end
#define MSIMEUpdateController MetasequoiaUpdateController

NS_ASSUME_NONNULL_END
