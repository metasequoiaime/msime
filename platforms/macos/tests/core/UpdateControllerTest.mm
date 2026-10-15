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
    }
    std::puts("macOS update controller passed.");
    return 0;
}
