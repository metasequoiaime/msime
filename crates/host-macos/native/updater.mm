#import <AppKit/AppKit.h>

#import "../../../platforms/macos/src/core/UpdateChannelFeed.h"

// 这些选择器来自随产品分发的 Sparkle 2.9.6 公共接口；动态加载避免开发编译依赖 SDK 的安装路径。
@protocol MSIMESettingsSparkleUpdater <NSObject>
@property(nonatomic, readonly) BOOL canCheckForUpdates;
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

extern "C" bool msime_macos_settings_update_check_channel(const char *channel) {
    if (!NSThread.isMainThread || !channel || !msime_macos_settings_update_supported()) return false;
    // 通道名来自偏好 update_channel；不认识的值是调用方的错误，不检查，免得静默地按稳定版检查。
    NSString *parsed = MSIMEUpdateChannelParse([NSString stringWithUTF8String:channel]);
    if (!parsed) return false;
    static id<MSIMESettingsSparkleController> controller;
    // Sparkle 只弱引用委托，由这里持有。
    static MSIMESettingsUpdateFeed *feed;
    if (!controller) {
        NSBundle *framework = [NSBundle bundleWithPath:MSIMESettingsSparklePath()];
        NSError *error = nil;
        if (![framework loadAndReturnError:&error]) return false;
        Class controllerClass = NSClassFromString(@"SPUStandardUpdaterController");
        if (!controllerClass) return false;
        MSIMESettingsUpdateFeed *candidateFeed = [MSIMESettingsUpdateFeed new];
        candidateFeed.channel = parsed;
        id<MSIMESettingsSparkleController> candidate = (id)[controllerClass alloc];
        candidate = [candidate initWithStartingUpdater:NO updaterDelegate:candidateFeed userDriverDelegate:nil];
        if (![candidate.updater startUpdater:&error]) return false;
        controller = candidate;
        feed = candidateFeed;
    }
    // 已有检查或安装正在进行时，不并发启动另一轮。
    if (!controller.updater.canCheckForUpdates) return false;
    // 每次点「检查更新」都按当时所选的通道取 feed。
    feed.channel = parsed;
    [controller checkForUpdates:nil];
    return true;
}

extern "C" bool msime_macos_settings_update_check(void) {
    return msime_macos_settings_update_check_channel("stable");
}
