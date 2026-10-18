#import <AppKit/AppKit.h>

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

extern "C" bool msime_macos_settings_update_check(void) {
    if (!NSThread.isMainThread || !msime_macos_settings_update_supported()) return false;
    static id<MSIMESettingsSparkleController> controller;
    if (!controller) {
        NSBundle *framework = [NSBundle bundleWithPath:MSIMESettingsSparklePath()];
        NSError *error = nil;
        if (![framework loadAndReturnError:&error]) return false;
        Class controllerClass = NSClassFromString(@"SPUStandardUpdaterController");
        if (!controllerClass) return false;
        id<MSIMESettingsSparkleController> candidate = (id)[controllerClass alloc];
        candidate = [candidate initWithStartingUpdater:NO updaterDelegate:nil userDriverDelegate:nil];
        if (![candidate.updater startUpdater:&error]) return false;
        controller = candidate;
    }
    // 已有检查或安装正在进行时，不并发启动另一轮。
    if (!controller.updater.canCheckForUpdates) return false;
    [controller checkForUpdates:nil];
    return true;
}
