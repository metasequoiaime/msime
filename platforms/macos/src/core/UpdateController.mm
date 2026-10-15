#import "UpdateController.h"

#import <Sparkle/Sparkle.h>

// 官网下载页的 macOS 一栏读的就是本仓库的 macos-v 发布，GitHub 按钮旁边有国内镜像（dl.msime.app），国内打开 GitHub 发布页下载常常只有几十 KB/s。
static NSString *const MetasequoiaReleasePageURL = @"https://msime.app/download/?platform=macos";

// Sparkle needs an application bundle: a feed URL, a version, a code signature. Started anywhere else
// it reports the misconfiguration with a modal alert, which in an input method process means the user's
// typing stops behind a dialog they never asked for. Non-application processes therefore stay inert;
// application bundles without a feed use the explicit release-page driver below.
@interface MetasequoiaUnavailableUpdateDriver : NSObject <MetasequoiaUpdateDriver>
@end

@implementation MetasequoiaUnavailableUpdateDriver

- (BOOL)canCheckForUpdates
{
    return NO;
}

- (BOOL)automaticallyChecksForUpdates
{
    return NO;
}

- (void)checkForUpdates:(id)sender
{
    (void)sender;
}

@end

@interface MetasequoiaReleasePageUpdateDriver ()
@property(nonatomic, readonly) NSURL *releaseURL;
@property(nonatomic, copy, readonly) MetasequoiaUpdateReleaseConfirmation confirmation;
@property(nonatomic, copy, readonly) MetasequoiaUpdateReleaseOpener opener;
@property(nonatomic, copy, readonly) MetasequoiaUpdateReleaseFailure failure;
@end

@implementation MetasequoiaReleasePageUpdateDriver

- (instancetype)initWithReleaseURL:(NSURL *)releaseURL
                       confirmation:(MetasequoiaUpdateReleaseConfirmation)confirmation
                             opener:(MetasequoiaUpdateReleaseOpener)opener
                            failure:(MetasequoiaUpdateReleaseFailure)failure
{
    self = [super init];
    if (self != nil)
    {
        _releaseURL = releaseURL;
        _confirmation = [confirmation copy];
        _opener = [opener copy];
        _failure = [failure copy];
        if (!releaseURL || !confirmation || !opener || !failure) return nil;
    }
    return self;
}

- (BOOL)canCheckForUpdates { return YES; }
- (BOOL)automaticallyChecksForUpdates { return NO; }

- (void)checkForUpdates:(id)sender
{
    (void)sender;
    if (self.confirmation(self.releaseURL) != NSAlertFirstButtonReturn) return;
    if (!self.opener(self.releaseURL)) self.failure();
}

@end

// The input method is LSBackgroundOnly and lives as long as the login session, so Sparkle's defaults do
// not fit it. Its relaunch would start the bundle as a standalone launch, which opens the preferences
// window (input_method_main.mm). Its install-on-quit rarely comes, and if the delegate leaves the
// background-downloaded update to Sparkle, after SUScheduledImpatientCheckInterval Sparkle presents it in
// a window that takes focus from whatever the user is typing into. So the driver keeps the install
// handler and runs it when the user switches to another input source (installDownloadedUpdate); Sparkle
// still installs on quit if that comes first.
@interface MetasequoiaSparkleUpdateDriver : NSObject <MetasequoiaUpdateDriver, SPUUpdaterDelegate>
@property(nonatomic, readonly) SPUStandardUpdaterController *updaterController;
@property(nonatomic, copy, nullable) void (^pendingInstall)(void);
@end

@implementation MetasequoiaSparkleUpdateDriver

- (instancetype)init
{
    self = [super init];
    if (self != nil)
    {
        _updaterController = [[SPUStandardUpdaterController alloc] initWithStartingUpdater:YES
                                                                           updaterDelegate:self
                                                                        userDriverDelegate:nil];
    }
    return self;
}

- (BOOL)canCheckForUpdates
{
    return self.updaterController.updater.canCheckForUpdates;
}

- (BOOL)automaticallyChecksForUpdates
{
    return self.updaterController.updater.automaticallyChecksForUpdates;
}

- (void)checkForUpdates:(id)sender
{
    [self.updaterController checkForUpdates:sender];
}

- (void)installDownloadedUpdate
{
    void (^install)(void) = self.pendingInstall;
    if (install != nil) install();
}

- (BOOL)updaterShouldRelaunchApplication:(SPUUpdater *)updater
{
    (void)updater;
    return NO;
}

- (BOOL)updater:(SPUUpdater *)updater
    willInstallUpdateOnQuit:(SUAppcastItem *)item
    immediateInstallationBlock:(void (^)(void))immediateInstallHandler
{
    (void)updater;
    (void)item;
    self.pendingInstall = immediateInstallHandler;
    return YES;
}

@end

@interface MetasequoiaUpdateController ()
@property(nonatomic) id<MetasequoiaUpdateDriver> driver;
@property(nonatomic, copy) MetasequoiaUpdateActivationHandler activationHandler;
@end

@implementation MetasequoiaUpdateController

+ (instancetype)sharedController
{
    static MetasequoiaUpdateController *controller = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
      NSBundle *host = NSBundle.mainBundle;
      MetasequoiaUpdateRoute route = MSIMEUpdateRouteForHost(
          host.bundleIdentifier, host.bundlePath, [host objectForInfoDictionaryKey:@"SUFeedURL"]);
      id<MetasequoiaUpdateDriver> driver = nil;
      if (route == MetasequoiaUpdateRouteSparkle)
      {
          driver = [[MetasequoiaSparkleUpdateDriver alloc] init];
      }
      else if (route == MetasequoiaUpdateRouteReleasePage)
      {
          NSURL *releaseURL = [NSURL URLWithString:MetasequoiaReleasePageURL];
          driver = [[MetasequoiaReleasePageUpdateDriver alloc]
              initWithReleaseURL:releaseURL
                   confirmation:^NSModalResponse(NSURL *url) {
                     (void)url;
                     NSAlert *alert = [NSAlert new];
                     alert.messageText = @"此构建未配置应用内更新";
                     alert.informativeText = @"无法使用 Sparkle 自动检查。可以前往水杉输入法官网的下载页获取最新版本。";
                     [alert addButtonWithTitle:@"前往下载页"];
                     [alert addButtonWithTitle:@"取消"];
                     return [alert runModal];
                   }
                         opener:^BOOL(NSURL *url) {
                           return [NSWorkspace.sharedWorkspace openURL:url];
                         }
                        failure:^{
                          NSAlert *alert = [NSAlert new];
                          alert.alertStyle = NSAlertStyleCritical;
                          alert.messageText = @"无法打开下载页";
                          alert.informativeText = @"请稍后重试，或在浏览器中访问 msime.app/download。";
                          [alert runModal];
                        }];
      }
      else
      {
          driver = [[MetasequoiaUnavailableUpdateDriver alloc] init];
      }
      controller =
          [[MetasequoiaUpdateController alloc] initWithDriver:driver
                                            activationHandler:^{
                                              [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
                                              [NSApp activateIgnoringOtherApps:YES];
                                            }];
    });
    return controller;
}

- (instancetype)initWithDriver:(id<MetasequoiaUpdateDriver>)driver
             activationHandler:(MetasequoiaUpdateActivationHandler)activationHandler
{
    self = [super init];
    if (self != nil)
    {
        _driver = driver;
        _activationHandler = [activationHandler copy];
    }
    return self;
}

- (BOOL)canCheckForUpdates
{
    return self.driver.canCheckForUpdates;
}

- (BOOL)automaticallyChecksForUpdates
{
    return self.driver.automaticallyChecksForUpdates;
}

- (void)checkForUpdates:(id)sender
{
    self.activationHandler();
    [self.driver checkForUpdates:sender];
}

- (void)installDownloadedUpdateIfReady
{
    if ([self.driver respondsToSelector:@selector(installDownloadedUpdate)]) [self.driver installDownloadedUpdate];
}

@end
