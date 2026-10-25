#import "../../src/core/UpdateController.h"

#import <AppKit/AppKit.h>

#include <cassert>
#include <cstdio>

// Stands in for Sparkle so the controller's own behaviour can be checked without starting an updater.
@interface UpdateDriverFixture : NSObject <MetasequoiaUpdateDriver>
@property(nonatomic) BOOL canCheckForUpdates;
@property(nonatomic) BOOL automaticallyChecksForUpdates;
@property(nonatomic) NSUInteger checks;
@property(nonatomic) id lastSender;
@end

@implementation UpdateDriverFixture
- (void)checkForUpdates:(id)sender
{
    self.lastSender = sender;
    ++self.checks;
}
@end

// The Sparkle driver's shape: it can also install an update it downloaded in the background.
@interface InstallingUpdateDriverFixture : UpdateDriverFixture
@property(nonatomic) NSUInteger installs;
@end

@implementation InstallingUpdateDriverFixture
- (void)installDownloadedUpdate
{
    ++self.installs;
}
@end

// Sparkle 驱动的通道接口：记下每次收到的通道。
@interface ChannelUpdateDriverFixture : UpdateDriverFixture
@property(nonatomic) NSMutableArray<NSString *> *channels;
@end

@implementation ChannelUpdateDriverFixture
- (instancetype)init
{
    self = [super init];
    if (self != nil) _channels = [NSMutableArray array];
    return self;
}
- (void)setUpdateChannel:(NSString *)channel
{
    [self.channels addObject:channel];
}
@end

int main()
{
    @autoreleasepool
    {
        // The activation handler talks to NSApp; without this it would be messaging nil rather than the
        // accessory application the shipped controller runs in.
        [NSApplication sharedApplication];
        __block NSUInteger activations = 0;
        UpdateDriverFixture *driver = [UpdateDriverFixture new];
        driver.canCheckForUpdates = YES;
        driver.automaticallyChecksForUpdates = YES;
        MSIMEUpdateController *controller =
            [[MSIMEUpdateController alloc] initWithDriver:driver
                                        activationHandler:^{ ++activations; }];

        assert(controller.canCheckForUpdates);
        assert(controller.automaticallyChecksForUpdates);

        // The menu item is in an accessory process, so a manual check has to raise the window itself
        // before handing the request on; a check that activates nothing leaves the sheet behind whatever
        // the user was typing into.
        NSMenuItem *sender = [NSMenuItem new];
        [controller checkForUpdates:sender];
        assert(activations == 1 && driver.checks == 1 && driver.lastSender == sender);

        // Both are read through, not cached: Sparkle answers differently once an update is in flight, and
        // a stale "can check" leaves the menu item enabled for a check that cannot start.
        driver.canCheckForUpdates = NO;
        driver.automaticallyChecksForUpdates = NO;
        assert(!controller.canCheckForUpdates);
        assert(!controller.automaticallyChecksForUpdates);

        // A background install runs when the user has just switched to another input source. Raising the
        // process there would pull focus to an input method the user has just left, so it must not
        // activate; a driver that downloads nothing in the background simply has nothing to install.
        [controller installDownloadedUpdateIfReady];
        assert(activations == 1 && driver.checks == 1);
        InstallingUpdateDriverFixture *installing = [InstallingUpdateDriverFixture new];
        MSIMEUpdateController *installingController =
            [[MSIMEUpdateController alloc] initWithDriver:installing
                                        activationHandler:^{ ++activations; }];
        [installingController installDownloadedUpdateIfReady];
        assert(installing.installs == 1 && installing.checks == 0 && activations == 1);

        // 更新通道：偏好原值交给控制器，规范化之后只在变化时转给驱动。缺省（稳定版不写进文档）、不认识的值和类型不对的值都按稳定版。
        ChannelUpdateDriverFixture *channelDriver = [ChannelUpdateDriverFixture new];
        MSIMEUpdateController *channelController =
            [[MSIMEUpdateController alloc] initWithDriver:channelDriver
                                        activationHandler:^{ ++activations; }];
        assert(channelController.updateChannel == nil);
        [channelController applyUpdateChannelPreference:nil];
        assert([channelController.updateChannel isEqualToString:@"stable"]);
        assert([channelDriver.channels isEqualToArray:@[ @"stable" ]]);
        [channelController applyUpdateChannelPreference:@"stable"];
        [channelController applyUpdateChannelPreference:@"canary"];
        [channelController applyUpdateChannelPreference:@1];
        assert([channelDriver.channels isEqualToArray:@[ @"stable" ]]);
        [channelController applyUpdateChannelPreference:@"beta"];
        [channelController applyUpdateChannelPreference:@"beta"];
        [channelController applyUpdateChannelPreference:@"nightly"];
        [channelController applyUpdateChannelPreference:@"Beta"];
        NSArray *expectedChannels = @[ @"stable", @"beta", @"nightly", @"stable" ];
        assert([channelDriver.channels isEqualToArray:expectedChannels]);
        assert([channelController.updateChannel isEqualToString:@"stable"] && activations == 1 && channelDriver.checks == 0);
        // 没有通道概念的驱动（官网下载页、不可用）收不到通道，也不出错。
        [controller applyUpdateChannelPreference:@"nightly"];
        assert([controller.updateChannel isEqualToString:@"nightly"]);

        // 按通道选 feed：稳定版就是 SUFeedURL；测试版和每日构建在同一目录下、版本 id 前插入通道名。输入法和设置应用两种文件名都覆盖，与 sparkle_feed.py 的 feed_name 一致。
        NSString *base = @"https://github.com/metasequoiaime/msime/releases/download/macos-appcast/";
        NSString *inputFeed = [base stringByAppendingString:@"appcast-full.xml"];
        NSString *settingsFeed = [base stringByAppendingString:@"appcast-settings-wubi.xml"];
        assert([MSIMEUpdateFeedURLForChannel(inputFeed, @"stable") isEqualToString:inputFeed]);
        assert([MSIMEUpdateFeedURLForChannel(inputFeed, @"beta")
            isEqualToString:[base stringByAppendingString:@"appcast-beta-full.xml"]]);
        assert([MSIMEUpdateFeedURLForChannel(inputFeed, @"nightly")
            isEqualToString:[base stringByAppendingString:@"appcast-nightly-full.xml"]]);
        assert([MSIMEUpdateFeedURLForChannel(settingsFeed, @"stable") isEqualToString:settingsFeed]);
        assert([MSIMEUpdateFeedURLForChannel(settingsFeed, @"beta")
            isEqualToString:[base stringByAppendingString:@"appcast-settings-beta-wubi.xml"]]);
        assert([MSIMEUpdateFeedURLForChannel(settingsFeed, @"nightly")
            isEqualToString:[base stringByAppendingString:@"appcast-settings-nightly-wubi.xml"]]);
        // 推不出来时返回 nil，调用方退回 SUFeedURL（稳定版），不请求拼错的地址：通道写错、没有 feed、不是 https、带查询或片段、文件名不是 appcast-<版本 id>.xml、已经是某个通道的 feed。
        assert(MSIMEUpdateFeedURLForChannel(inputFeed, @"canary") == nil);
        assert(MSIMEUpdateFeedURLForChannel(inputFeed, @"Beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(nil, @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"", @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"http://example.invalid/appcast-full.xml", @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"https://example.invalid/appcast-full.xml?x=1", @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"https://example.invalid/appcast-full.xml#top", @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"https://example.invalid/appcast.xml", @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"https://example.invalid/feed-full.xml", @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"https://example.invalid/appcast-full.rss", @"beta") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"https://example.invalid/appcast-beta-full.xml", @"nightly") == nil);
        assert(MSIMEUpdateFeedURLForChannel(@"https://example.invalid/appcast-settings-nightly-full.xml", @"stable") == nil);
        assert([MSIMEUpdateChannelFromPreference(nil) isEqualToString:@"stable"]);
        assert([MSIMEUpdateChannelFromPreference(@"nightly") isEqualToString:@"nightly"]);
        assert([MSIMEUpdateChannelFromPreference(@[ @"beta" ]) isEqualToString:@"stable"]);
        assert([MSIMEUpdateChannelParse(@"beta") isEqualToString:@"beta"]);
        assert(MSIMEUpdateChannelParse(@"") == nil && MSIMEUpdateChannelParse(nil) == nil);

        // 更新包的通道从文件名读，与 sparkle_feed.py 的 archive_channel 一致；换通道时据此决定还装不装已经下好的更新。
        assert([MSIMEUpdateChannelOfArchiveName(@"msime-macos-0.55.0-input-method.zip") isEqualToString:@"stable"]);
        assert([MSIMEUpdateChannelOfArchiveName(@"msime-macos-wubi-0.55.0-beta-430000-settings.zip") isEqualToString:@"beta"]);
        assert([MSIMEUpdateChannelOfArchiveName(@"msime-macos-0.55.0-nightly-430001-input-method.zip") isEqualToString:@"nightly"]);
        assert([MSIMEUpdateChannelOfArchiveName(@"msime-macos-0.55.0-nightly-0-input-method.zip") isEqualToString:@"stable"]);
        assert([MSIMEUpdateChannelOfArchiveName(@"msime-macos-0.55.0-nightly-430001-input-method.zip.sig") isEqualToString:@"stable"]);
        assert([MSIMEUpdateChannelOfArchiveName(nil) isEqualToString:@"stable"]);
        assert(MSIMEUpdateChannelAccepts(@"nightly", @"stable") && MSIMEUpdateChannelAccepts(@"nightly", @"beta"));
        assert(MSIMEUpdateChannelAccepts(@"beta", @"stable") && MSIMEUpdateChannelAccepts(@"beta", @"beta"));
        assert(!MSIMEUpdateChannelAccepts(@"beta", @"nightly"));
        assert(!MSIMEUpdateChannelAccepts(@"stable", @"beta") && !MSIMEUpdateChannelAccepts(@"stable", @"nightly"));
        assert(MSIMEUpdateChannelAccepts(@"stable", @"stable"));

        // Sparkle is only startable inside an application bundle. Everywhere else - a test binary, a
        // command-line tool, a bundle whose identifier never made it into the plist - it answers a start
        // with a modal alert, and an input method that stops typing behind a dialog is the worse failure.
        NSString *feed = @"https://example.invalid/appcast.xml";
        assert(MSIMEUpdateHostCanStartSparkle(@"app.msime.inputmethod.MetasequoiaIME",
                                              @"/Users/someone/Library/Input Methods/水杉输入法.app", feed));
        assert(MSIMEUpdateHostCanStartSparkle(@"app.example", @"/Applications/Example.app/", feed));
        assert(!MSIMEUpdateHostCanStartSparkle(nil, @"/Applications/Example.app", feed));
        assert(!MSIMEUpdateHostCanStartSparkle(@"", @"/Applications/Example.app", feed));
        assert(!MSIMEUpdateHostCanStartSparkle(@"app.example", nil, feed));
        assert(!MSIMEUpdateHostCanStartSparkle(@"app.example", @"/usr/local/bin/example", feed));
        // A bundle with no feed configured is the shape this product currently ships: Sparkle started
        // there has nothing to check and answers with the same modal alert, so it must stay unstarted.
        assert(!MSIMEUpdateHostCanStartSparkle(@"app.example", @"/Applications/Example.app", nil));
        assert(!MSIMEUpdateHostCanStartSparkle(@"app.example", @"/Applications/Example.app", @""));
        // A bundle path that merely contains ".app" is not one: this is the shape a test binary built
        // inside a bundle's directory has.
        assert(!MSIMEUpdateHostCanStartSparkle(@"app.example", @"/Applications/Example.app/Contents/MacOS/example", feed));

        assert(MSIMEUpdateRouteForHost(@"app.example", @"/Applications/Example.app", feed) ==
               MetasequoiaUpdateRouteSparkle);
        assert(MSIMEUpdateRouteForHost(@"app.example", @"/Applications/Example.app", nil) ==
               MetasequoiaUpdateRouteReleasePage);
        assert(MSIMEUpdateRouteForHost(@"app.example", @"/usr/local/bin/example", nil) ==
               MetasequoiaUpdateRouteUnavailable);

        __block NSInteger confirmations = 0;
        __block NSInteger opens = 0;
        __block NSInteger failures = 0;
        NSURL *releaseURL = [NSURL URLWithString:@"https://github.com/metasequoiaime/msime/releases"];
        MetasequoiaReleasePageUpdateDriver *releaseDriver =
            [[MetasequoiaReleasePageUpdateDriver alloc]
                initWithReleaseURL:releaseURL
                     confirmation:^NSModalResponse(NSURL *url) {
                       assert([url isEqual:releaseURL]);
                       confirmations += 1;
                       return NSAlertFirstButtonReturn;
                     }
                           opener:^BOOL(NSURL *url) {
                             assert([url isEqual:releaseURL]);
                             opens += 1;
                             return NO;
                           }
                          failure:^{ failures += 1; }];
        assert(releaseDriver.canCheckForUpdates && !releaseDriver.automaticallyChecksForUpdates);
        [releaseDriver checkForUpdates:nil];
        assert(confirmations == 1 && opens == 1 && failures == 1);

        MetasequoiaReleasePageUpdateDriver *cancelledDriver =
            [[MetasequoiaReleasePageUpdateDriver alloc]
                initWithReleaseURL:releaseURL
                     confirmation:^NSModalResponse(NSURL *url) {
                       assert([url isEqual:releaseURL]);
                       return NSAlertSecondButtonReturn;
                     }
                           opener:^BOOL(NSURL *url) {
                             (void)url;
                             opens += 1;
                             return YES;
                           }
                          failure:^{ failures += 1; }];
        [cancelledDriver checkForUpdates:nil];
        assert(opens == 1 && failures == 1);

        // This binary is exactly the case the guard exists for, so the shared controller must not have
        // started Sparkle - and must still answer rather than crash the caller.
        assert(!MSIMEUpdateHostCanStartSparkle(NSBundle.mainBundle.bundleIdentifier,
                                               NSBundle.mainBundle.bundlePath,
                                               [NSBundle.mainBundle objectForInfoDictionaryKey:@"SUFeedURL"]));
        MSIMEUpdateController *shared = MSIMEUpdateController.sharedController;
        assert(shared && !shared.canCheckForUpdates && !shared.automaticallyChecksForUpdates);
        [shared checkForUpdates:nil];
        // 启动时带着偏好取共享控制器：已经存在时就是同一个控制器，只是换上偏好里的通道。
        MSIMEUpdateController *sameShared = [MSIMEUpdateController sharedControllerWithUpdateChannelPreference:@"beta"];
        assert(sameShared == shared && [shared.updateChannel isEqualToString:@"beta"]);
    }
    std::puts("macOS update controller passed.");
    return 0;
}
