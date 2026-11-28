#import <AppKit/AppKit.h>

#import "../../../platforms/macos/src/core/UpdateChannelFeed.h"

// 这些选择器来自随产品分发的 Sparkle 2.9.6 公共接口；动态加载避免开发编译依赖 SDK 的安装路径。
@protocol MSIMESettingsSparkleUpdater <NSObject>
@property(nonatomic, readonly) BOOL canCheckForUpdates;
@property(nonatomic) BOOL automaticallyChecksForUpdates;
@property(nonatomic) BOOL automaticallyDownloadsUpdates;
- (BOOL)startUpdater:(NSError * __autoreleasing *)error;
@end

@protocol MSIMESettingsSparkleController <NSObject>
- (instancetype)initWithStartingUpdater:(BOOL)startUpdater
                       updaterDelegate:(id)updaterDelegate
                    userDriverDelegate:(id)userDriverDelegate;
@property(nonatomic, readonly) id<MSIMESettingsSparkleUpdater> updater;
- (void)checkForUpdates:(id)sender;
@end

static NSString *MSIMESettingsSparklePath(void) {
    return [NSBundle.mainBundle.privateFrameworksPath stringByAppendingPathComponent:@"Sparkle.framework"];
}

extern "C" bool msime_macos_settings_update_supported(void) {
    NSBundle *bundle = NSBundle.mainBundle;
    id feed = [bundle objectForInfoDictionaryKey:@"SUFeedURL"];
    id key = [bundle objectForInfoDictionaryKey:@"SUPublicEDKey"];
    return [bundle.bundlePath.pathExtension isEqualToString:@"app"] &&
        [feed isKindOfClass:NSString.class] && [feed length] > 0 &&
        [key isKindOfClass:NSString.class] && [key length] > 0 &&
        [NSFileManager.defaultManager fileExistsAtPath:MSIMESettingsSparklePath()];
}

// Sparkle 的更新器委托，只实现 SPUUpdaterDelegate 的 feedURLStringForUpdater:：Sparkle 每次检查前取 feed 地址都会问它。Info.plist 的 SUFeedURL 是稳定版的 appcast-settings-<版本 id>.xml，测试版和每日构建的地址由它推出（UpdateChannelFeed.h，与输入法同一条规则）；推不出来时返回 nil，Sparkle 用 SUFeedURL，也就是退回稳定版。
@interface MSIMESettingsUpdateFeed : NSObject
@property(nonatomic, copy) NSString *channel;
@end

@implementation MSIMESettingsUpdateFeed
- (NSString *)feedURLStringForUpdater:(id)updater {
    (void)updater;
    if (self.channel.length == 0 || [self.channel isEqualToString:@"stable"]) return nil;
    return MSIMEUpdateFeedURLForChannel([NSBundle.mainBundle objectForInfoDictionaryKey:@"SUFeedURL"], self.channel);
}
@end

// 本进程是否在后台检查并下载设置应用的更新：只有设置应用的主实例按偏好 update_auto_download 打开它（msime_macos_settings_update_configure_background），面板进程和手动检查从不打开。
static BOOL MSIMESettingsBackgroundUpdates = NO;
static id<MSIMESettingsSparkleController> MSIMESettingsController;
// Sparkle 只弱引用委托，由这里持有。
static MSIMESettingsUpdateFeed *MSIMESettingsFeed;

// 把后台检查与下载的选择交给 Sparkle。两项都存进本应用的用户默认值（app.msime.macos），盖过 Info.plist 里 settings_updater.py 写的 false；Sparkle 自己按 SUScheduledCheckInterval 安排检查，换了选择时重排周期。
//
// 自动下载只在 automaticallyChecksForUpdates 为真时生效（Info.plist 没有 SUAllowsAutomaticUpdates，Sparkle 用检查开关代替它），所以打开时先开检查再开下载，关闭时反过来。
//
// 不会静默安装：/Applications/MSIME.app 归 root，SPUAutomaticUpdateDriver 下载完、要启动安装器时需要授权，它以 requiresSilentInstall 运行、不弹授权，于是返回 SUInstallationAuthorizeLaterError 并保留已下载的更新；Sparkle 随即用带界面的驱动提示「新版本已可安装」，用户点安装后才弹管理员授权、校验归档签名（SUVerifyUpdateBeforeExtraction）并替换应用。在提示里选「以后再说」时已下载的更新留在本进程里，下一次后台检查再提示；取消管理员授权时 Sparkle 删掉这份下载，旧版本照常可用，下一次后台检查重新下载。
static void MSIMESettingsApplyBackgroundUpdates(id<MSIMESettingsSparkleUpdater> updater) {
    if (MSIMESettingsBackgroundUpdates) {
        updater.automaticallyChecksForUpdates = YES;
        updater.automaticallyDownloadsUpdates = YES;
    } else {
        updater.automaticallyDownloadsUpdates = NO;
        updater.automaticallyChecksForUpdates = NO;
    }
}

// 取设置应用的 Sparkle 控制器，没有时创建并启动。`channel` 是已经解析过的通道名。只在主线程调用。
static id<MSIMESettingsSparkleController> MSIMESettingsUpdateController(NSString *channel) {
    if (MSIMESettingsController) return MSIMESettingsController;
    NSBundle *framework = [NSBundle bundleWithPath:MSIMESettingsSparklePath()];
    NSError *error = nil;
    if (![framework loadAndReturnError:&error]) return nil;
    Class controllerClass = NSClassFromString(@"SPUStandardUpdaterController");
    if (!controllerClass) return nil;
    MSIMESettingsUpdateFeed *candidateFeed = [MSIMESettingsUpdateFeed new];
    candidateFeed.channel = channel;
    id<MSIMESettingsSparkleController> candidate = (id)[controllerClass alloc];
    candidate = [candidate initWithStartingUpdater:NO updaterDelegate:candidateFeed userDriverDelegate:nil];
    // 启动之前定下后台选择：以前打开过、后来在别处关掉的用户默认值不会让这次启动先安排一次后台检查。
    MSIMESettingsApplyBackgroundUpdates(candidate.updater);
    if (![candidate.updater startUpdater:&error]) return nil;
    MSIMESettingsController = candidate;
    MSIMESettingsFeed = candidateFeed;
    return candidate;
}

extern "C" bool msime_macos_settings_update_check_channel(const char *channel) {
    if (!NSThread.isMainThread || !channel || !msime_macos_settings_update_supported()) return false;
    // 通道名来自偏好 update_channel；不认识的值是调用方的错误，不检查，免得静默地按稳定版检查。
    NSString *parsed = MSIMEUpdateChannelParse([NSString stringWithUTF8String:channel]);
    if (!parsed) return false;
    id<MSIMESettingsSparkleController> controller = MSIMESettingsUpdateController(parsed);
    if (!controller) return false;
    // 已有检查或安装正在进行时，不并发启动另一轮。
    if (!controller.updater.canCheckForUpdates) return false;
    // 每次点「检查更新」都按当时所选的通道取 feed。
    MSIMESettingsFeed.channel = parsed;
    [controller checkForUpdates:nil];
    return true;
}

extern "C" bool msime_macos_settings_update_configure_background(const char *channel, bool enabled) {
    if (!NSThread.isMainThread || !channel || !msime_macos_settings_update_supported()) return false;
    NSString *parsed = MSIMEUpdateChannelParse([NSString stringWithUTF8String:channel]);
    if (!parsed) return false;
    MSIMESettingsBackgroundUpdates = enabled;
    // 关着且更新器还没创建时不加载 Sparkle：以后手动检查创建控制器时会按关闭处理。
    if (!enabled && !MSIMESettingsController) return true;
    id<MSIMESettingsSparkleController> controller = MSIMESettingsUpdateController(parsed);
    if (!controller) return false;
    // 后台检查按偏好里保存的通道取 feed。
    MSIMESettingsFeed.channel = parsed;
    MSIMESettingsApplyBackgroundUpdates(controller.updater);
    return true;
}

extern "C" bool msime_macos_settings_update_check(void) {
    return msime_macos_settings_update_check_channel("stable");
}
