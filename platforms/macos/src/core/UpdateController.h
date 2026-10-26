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

// 系统范围最后一次键盘、鼠标或触控板输入距今的秒数（生产环境是 CGEventSourceSecondsSinceLastEventType），测试里换成固定值。
typedef CFTimeInterval (^MetasequoiaUpdateIdleClock)(void);

// 闲置多久算用户离开了电脑，可以装后台下好的更新。半小时没有任何输入，回来时多等的那一次输入法启动不会落在连续打字中间。
static const CFTimeInterval MSIMEUpdateAwayIdleSeconds = 30 * 60;

// 多久看一次闲置时长。只决定用户离开之后多快装上，不影响是否装。
static const NSTimeInterval MSIMEUpdateAwayPollSeconds = 5 * 60;

// 装更新时输入法进程会退出，用户手上还有没完成的东西时不能装：没上屏的组字、正在录的语音、本进程开着的窗口。返回 YES 时这次不装，等下一次触发。
typedef BOOL (^MetasequoiaUpdateUnfinishedWork)(void);

// 本进程里有没有用户开着的窗口（词库编辑、AI 设置、反馈、Sparkle 自己的更新界面等）：可以成为 key window、可见或最小化在程序坞里的窗口。候选窗、悬浮工具栏、菜单栏图标这些面板不能成为 key window，不算。
static inline BOOL MSIMEUpdateWindowsHoldUserWork(NSArray<NSWindow *> *windows)
{
    for (NSWindow *window in windows)
        if ((window.visible || window.miniaturized) && window.canBecomeKeyWindow) return YES;
    return NO;
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
// 把后台下好的更新交给驱动安装。不激活进程、不显示任何东西：调用它的时候用户刚切到别的输入源，或者离开了电脑。unfinishedWork 返回 YES 时这次不装。
- (void)installDownloadedUpdateIfReady;
// 见 MetasequoiaUpdateUnfinishedWork。为 nil 时不拦。只在主线程设置。
@property(nonatomic, copy, nullable) MetasequoiaUpdateUnfinishedWork unfinishedWork;
// 只用水杉、从不切到别的输入源的用户等不到上面那一刻，更新要拖到注销。这里在用户离开电脑时也装：屏幕睡眠（包括锁屏后熄屏）、登录会话被切走时立即装，另外每 MSIMEUpdateAwayPollSeconds 看一次 idleClock，闲置达到 MSIMEUpdateAwayIdleSeconds 就装。center 是 NSWorkspace 的通知中心。只在主线程调用，重复调用不再注册。
- (void)installDownloadedUpdateWhenAwayWithWorkspaceCenter:(NSNotificationCenter *)center
                                                 idleClock:(MetasequoiaUpdateIdleClock)idleClock;
// 定时器每次触发时做的事：闲置够久就交给 installDownloadedUpdateIfReady。还没调用上面的方法时什么也不做。
- (void)installDownloadedUpdateIfIdle;
// 偏好文档里 update_channel 的原值（缺省、未知值按稳定版，见 MSIMEUpdateChannelFromPreference）。和上次相同时什么也不做，不同时交给驱动。只在主线程调用。
- (void)applyUpdateChannelPreference:(nullable id)preference;
// 最近一次交给驱动的通道；还没有读到偏好时为 nil。
@property(nonatomic, readonly, copy, nullable) NSString *updateChannel;
@end
#define MSIMEUpdateController MetasequoiaUpdateController

NS_ASSUME_NONNULL_END
