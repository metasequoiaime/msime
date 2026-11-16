#import "UpdateController.h"

#import "SystemUpdateStaging.h"

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

// 输入法是 LSBackgroundOnly，进程与登录会话同寿，Sparkle 的默认做法不适合它：它的重启会把 bundle 当成独立启动拉起，打开的是设置窗口（input_method_main.mm）；退出时安装很少等得到；委托若把后台下好的更新交还给 Sparkle，过了 SUScheduledImpatientCheckInterval 它会弹窗，抢走用户正在打字的应用的焦点。所以驱动留着安装闭包，在用户切到别的输入源或离开电脑时运行（installDownloadedUpdate，见控制器的 installDownloadedUpdateWhenAwayWithWorkspaceCenter:idleClock:）；退出先来的话 Sparkle 照常在退出时装。
@interface MetasequoiaSparkleUpdateDriver : NSObject <MetasequoiaUpdateDriver, SPUUpdaterDelegate>
// 装在用户目录时由标准控制器更新本 bundle；装在系统目录时没有它，见下面的 stagedUpdater。
@property(nonatomic, readonly, nullable) SPUStandardUpdaterController *updaterController;
// 装在 /Library/Input Methods 时的更新器：更新对象是用户缓存目录里的暂存副本（SystemUpdateStaging.h），暂存副本备好之后才创建，在那之前为 nil。
@property(nonatomic, nullable) SPUUpdater *stagedUpdater;
@property(nonatomic, nullable) SPUStandardUserDriver *stagedUserDriver;
@property(nonatomic, nullable) NSURL *stagingURL;
// 上一次提交给更新助手的构建号，同一个构建只提交一次。
@property(nonatomic) long long submittedBuild;
@property(nonatomic) BOOL submitting;
@property(nonatomic, readonly, nullable) SPUUpdater *updater;
@property(nonatomic, copy, nullable) void (^pendingInstall)(void);
// pendingInstall 那个更新包所属的通道（MSIMEUpdateChannelOfArchiveName），换通道时据此决定还装不装它。
@property(nonatomic, copy, nullable) NSString *pendingInstallChannel;
@property(nonatomic, copy) NSString *updateChannel;
@end

@implementation MetasequoiaSparkleUpdateDriver

- (instancetype)initWithUpdateChannel:(NSString *)channel
{
    self = [super init];
    if (self != nil)
    {
        // 先记下通道再启动 Sparkle：启动时它就会问 feed 地址，随后安排的检查也按这个通道取 appcast。
        _updateChannel = [MSIMEUpdateChannelFromPreference(channel) copy];
        if (MSIMEUpdateHostIsSystemInstall(NSBundle.mainBundle.bundlePath))
            [self startUpdatingStagedCopy];
        else
            _updaterController = [[SPUStandardUpdaterController alloc] initWithStartingUpdater:YES
                                                                               updaterDelegate:self
                                                                            userDriverDelegate:nil];
    }
    return self;
}

- (nullable SPUUpdater *)updater
{
    return self.updaterController != nil ? self.updaterController.updater : self.stagedUpdater;
}

// 把系统目录里的这份拷到缓存目录当暂存副本（两百多 MB，放到后台队列），再让 Sparkle 以它为更新对象。applicationBundle 也是暂存副本：它不是一个在运行的应用，Sparkle 装更新时不用等谁退出，本进程照常工作。
- (void)startUpdatingStagedCopy
{
    NSBundle *installed = NSBundle.mainBundle;
    NSURL *caches = [NSFileManager.defaultManager URLsForDirectory:NSCachesDirectory inDomains:NSUserDomainMask].firstObject;
    if (caches == nil || installed.bundleIdentifier.length == 0) return;
    NSURL *staging = MSIMEUpdateStagingURL(caches, installed.bundleIdentifier, installed.bundlePath.lastPathComponent);
    NSURL *installedURL = installed.bundleURL;
    __weak MetasequoiaSparkleUpdateDriver *weakSelf = self;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
      BOOL prepared = MSIMEUpdatePrepareStaging([NSFileManager new], installedURL, staging);
      dispatch_async(dispatch_get_main_queue(), ^{
        MetasequoiaSparkleUpdateDriver *driver = weakSelf;
        NSBundle *host = prepared ? [NSBundle bundleWithURL:staging] : nil;
        if (driver == nil || host == nil) return;
        driver.stagingURL = staging;
        driver.stagedUserDriver = [[SPUStandardUserDriver alloc] initWithHostBundle:host delegate:nil];
        SPUUpdater *updater = [[SPUUpdater alloc] initWithHostBundle:host
                                                   applicationBundle:host
                                                          userDriver:driver.stagedUserDriver
                                                            delegate:driver];
        // 启动失败时不留下它：没有更新器，「检查更新…」显示为不可用，而不是点了没反应。
        if ([updater startUpdater:nil]) driver.stagedUpdater = updater;
      });
    });
}

// 系统目录安装的更新分三步，每一步都要等上一步在磁盘上生效，所以每次只做当前该做的那一步：让 Sparkle 把下好的更新装进暂存副本，把变新的暂存副本提交给 root 更新助手，系统目录里的那份变新之后退出本进程。allowRestart 为 NO 时不做最后一步。
- (void)advanceSystemUpdateAllowingRestart:(BOOL)allowRestart
{
    NSURL *staging = self.stagingURL;
    if (staging == nil) return;
    long long running = [[NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleVersion"] longLongValue];
    long long installed = MSIMEUpdateBundleBuildOnDisk(NSBundle.mainBundle.bundleURL);
    long long staged = MSIMEUpdateBundleBuildOnDisk(staging);
    switch (MSIMESystemUpdateNextStep(running, installed, staged, self.submittedBuild))
    {
    case MSIMESystemUpdateStepRestart:
        // 系统在下一次需要输入法时拉起新的那份。
        if (allowRestart) [NSApp terminate:nil];
        return;
    case MSIMESystemUpdateStepSubmit: {
        if (self.submitting) return;
        self.submitting = YES;
        __weak MetasequoiaSparkleUpdateDriver *weakSelf = self;
        NSString *name = [NSString stringWithFormat:@"%d-%lld.app", NSProcessInfo.processInfo.processIdentifier, staged];
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
          // 提交不进去（更新助手没装）时也记下这个构建，免得每次触发都再拷一遍；下次启动会再试。
          MSIMEUpdateSubmitToSystemUpdater([NSFileManager new], staging, [NSURL fileURLWithPath:MSIMESystemUpdaterInboxPath isDirectory:YES], name);
          dispatch_async(dispatch_get_main_queue(), ^{
            MetasequoiaSparkleUpdateDriver *driver = weakSelf;
            if (driver == nil) return;
            driver.submitting = NO;
            driver.submittedBuild = staged;
          });
        });
        return;
    }
    case MSIMESystemUpdateStepInstallIntoStaging: {
        void (^install)(void) = self.pendingInstall;
        self.pendingInstall = nil;
        self.pendingInstallChannel = nil;
        if (install == nil) return;
        install();
        // Sparkle 装进暂存副本只要几秒，不必等到下一次切换输入源才提交。这一步不退出进程，所以不用再问用户手上有没有没做完的事。
        __weak MetasequoiaSparkleUpdateDriver *weakSelf = self;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 60 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
          [weakSelf advanceSystemUpdateAllowingRestart:NO];
        });
        return;
    }
    }
}

- (void)setUpdateChannel:(NSString *)channel
{
    NSString *next = MSIMEUpdateChannelFromPreference(channel);
    if ([next isEqualToString:_updateChannel]) return;
    _updateChannel = [next copy];
    // 已经在后台下好、等着切换输入源时安装的更新，若不属于新通道接受的构建（例如从每日构建换回稳定版时手里那个每日构建），就不再在切换输入源时装它：用户换回更窄的通道就是不想要它，装上之后更窄的通道也不会把它降级回去。换到更宽的通道时它仍被接受，照常安装。Sparkle 2.9.6 没有公开接口撤回已经交出的退出时安装，这个更新在输入法进程正常退出（注销、关机）时仍会被 Sparkle 装上；这次会话里 Sparkle 的检查周期也一直停着（SPUUpdaterDelegate.h：委托接管安装后不再运行新的周期），要到下次启动才按新通道检查。
    if (self.pendingInstall != nil && !MSIMEUpdateChannelAccepts(next, self.pendingInstallChannel))
    {
        self.pendingInstall = nil;
        self.pendingInstallChannel = nil;
    }
    // 换了通道就是换了 feed。按 SPUUpdater.h 的说明，设置变化后调用 resetUpdateCycle 重排检查计时，feed 变了时 Sparkle 会在后台马上检查一次；稍等片刻再重排，用户来回切换时只算最后一次。手动检查不受影响，下一次取 feed 时就读新通道。
    [self.updater resetUpdateCycleAfterShortDelay];
}

// Sparkle 每次检查前取 feed 地址都会问这里（SPUUpdater.feedURL），所以设置页改了通道，下一次检查就读新通道的 appcast。Info.plist 的 SUFeedURL 是稳定版的 feed，测试版和每日构建的地址由它推出；推不出来时返回 nil，Sparkle 用 SUFeedURL，也就是退回稳定版。
- (nullable NSString *)feedURLStringForUpdater:(SPUUpdater *)updater
{
    (void)updater;
    NSString *stable = [NSBundle.mainBundle objectForInfoDictionaryKey:@"SUFeedURL"];
    if ([self.updateChannel isEqualToString:@"stable"]) return nil;
    return MSIMEUpdateFeedURLForChannel(stable, self.updateChannel);
}

- (BOOL)canCheckForUpdates
{
    return self.updater.canCheckForUpdates;
}

- (BOOL)automaticallyChecksForUpdates
{
    return self.updater.automaticallyChecksForUpdates;
}

- (void)checkForUpdates:(id)sender
{
    if (self.updaterController != nil)
        [self.updaterController checkForUpdates:sender];
    else
        [self.stagedUpdater checkForUpdates];
}

- (void)installDownloadedUpdate
{
    if (self.updaterController == nil)
    {
        [self advanceSystemUpdateAllowingRestart:YES];
        return;
    }
    // 安装闭包只交出一次：切换输入源、熄屏、切走会话和闲置定时器可能在进程退出之前接连触发，不让 Sparkle 重复开始同一次安装。
    void (^install)(void) = self.pendingInstall;
    self.pendingInstall = nil;
    self.pendingInstallChannel = nil;
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
    self.pendingInstall = immediateInstallHandler;
    self.pendingInstallChannel = MSIMEUpdateChannelOfArchiveName(item.fileURL.lastPathComponent);
    return YES;
}

@end

@interface MetasequoiaUpdateController ()
@property(nonatomic) id<MetasequoiaUpdateDriver> driver;
@property(nonatomic, copy) MetasequoiaUpdateActivationHandler activationHandler;
@property(nonatomic, readwrite, copy, nullable) NSString *updateChannel;
@property(nonatomic, copy, nullable) MetasequoiaUpdateIdleClock idleClock;
@property(nonatomic, nullable) NSNotificationCenter *awayCenter;
@property(nonatomic, copy, nullable) NSArray<id<NSObject>> *awayObservers;
@property(nonatomic, nullable) NSTimer *awayTimer;
@end

// sharedControllerWithUpdateChannelPreference: 在创建共享控制器之前写下的通道，只在主线程读写。
static NSString *MetasequoiaLaunchUpdateChannel = nil;

@implementation MetasequoiaUpdateController

+ (instancetype)sharedControllerWithUpdateChannelPreference:(id)preference
{
    MetasequoiaLaunchUpdateChannel = MSIMEUpdateChannelFromPreference(preference);
    MetasequoiaUpdateController *controller = [self sharedController];
    [controller applyUpdateChannelPreference:preference];
    return controller;
}

+ (instancetype)sharedController
{
    static MetasequoiaUpdateController *controller = nil;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
      NSBundle *host = NSBundle.mainBundle;
      MetasequoiaUpdateRoute route = MSIMEUpdateRouteForHost(
          host.bundleIdentifier, host.bundlePath, [host objectForInfoDictionaryKey:@"SUFeedURL"]);
      id<MetasequoiaUpdateDriver> driver = nil;
      NSString *launchChannel = MetasequoiaLaunchUpdateChannel;
      if (route == MetasequoiaUpdateRouteSparkle)
      {
          driver = [[MetasequoiaSparkleUpdateDriver alloc] initWithUpdateChannel:launchChannel ?: @"stable"];
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
      // 驱动已经按这个通道创建，之后同样的偏好不必再交给它。
      controller.updateChannel = launchChannel;
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
    if (![self.driver respondsToSelector:@selector(installDownloadedUpdate)]) return;
    if (self.unfinishedWork != nil && self.unfinishedWork()) return;
    [self.driver installDownloadedUpdate];
}

- (void)installDownloadedUpdateWhenAwayWithWorkspaceCenter:(NSNotificationCenter *)center
                                                 idleClock:(MetasequoiaUpdateIdleClock)idleClock
{
    if (self.idleClock != nil) return;
    self.idleClock = idleClock;
    self.awayCenter = center;
    __weak MetasequoiaUpdateController *weakSelf = self;
    void (^away)(NSNotification *) = ^(NSNotification *notification) {
      (void)notification;
      [weakSelf installDownloadedUpdateIfReady];
    };
    // 熄屏时用户显然不在打字；锁屏之后显示器很快也会睡，同样走到这里。快速切换用户时本会话的输入法用不着了。
    self.awayObservers = @[
        [center addObserverForName:NSWorkspaceScreensDidSleepNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:away],
        [center addObserverForName:NSWorkspaceSessionDidResignActiveNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:away],
    ];
    // 显示器设成永不睡眠的机器只能靠闲置时长判断。没有待装的更新时驱动什么也不做，定时器允许一分钟误差，系统可以把唤醒和别的事合并。
    self.awayTimer = [NSTimer scheduledTimerWithTimeInterval:MSIMEUpdateAwayPollSeconds
                                                     repeats:YES
                                                       block:^(NSTimer *timer) {
                                                         (void)timer;
                                                         [weakSelf installDownloadedUpdateIfIdle];
                                                       }];
    self.awayTimer.tolerance = 60;
}

- (void)installDownloadedUpdateIfIdle
{
    if (self.idleClock != nil && self.idleClock() >= MSIMEUpdateAwayIdleSeconds) [self installDownloadedUpdateIfReady];
}

- (void)dealloc
{
    [_awayTimer invalidate];
    for (id<NSObject> observer in _awayObservers) [_awayCenter removeObserver:observer];
}

- (void)applyUpdateChannelPreference:(id)preference
{
    NSString *channel = MSIMEUpdateChannelFromPreference(preference);
    if ([channel isEqualToString:self.updateChannel]) return;
    self.updateChannel = channel;
    if ([self.driver respondsToSelector:@selector(setUpdateChannel:)]) [self.driver setUpdateChannel:channel];
}

@end
