#import "../../src/core/DesktopSettingsLauncher.h"
#include <cassert>

@interface TestWorkspace : NSWorkspace
@property BOOL installed;
@property NSUInteger launches;
@property(strong) NSWorkspaceOpenConfiguration *configuration;
@property(copy) void (^completion)(NSRunningApplication *, NSError *);
@end

@implementation TestWorkspace
- (NSURL *)URLForApplicationWithBundleIdentifier:(NSString *)identifier {
    assert([identifier isEqualToString:@"app.msime.client"]);
    return self.installed ? [NSURL fileURLWithPath:@"/synthetic/Settings.app"] : nil;
}
- (void)openApplicationAtURL:(NSURL *)url configuration:(NSWorkspaceOpenConfiguration *)configuration
          completionHandler:(void (^)(NSRunningApplication *, NSError *))completion {
    assert([url.path isEqualToString:@"/synthetic/Settings.app"]);
    self.launches++;
    self.configuration = configuration;
    self.completion = completion;
}
@end

int main() {
    @autoreleasepool {
        TestWorkspace *workspace = [TestWorkspace new];
        __block NSUInteger fallbacks = 0;
        dispatch_block_t fallback = ^{ assert(NSThread.isMainThread); ++fallbacks; };
        MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Appearance, workspace, fallback);
        assert(fallbacks == 1 && workspace.launches == 0);
        workspace.installed = YES;
        MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Appearance, workspace, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=settings:appearance"]]);
        assert(workspace.configuration.createsNewApplicationInstance);
        assert(workspace.configuration.activates);
        workspace.completion(NSRunningApplication.currentApplication, nil);
        assert(fallbacks == 1);
        MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Voice, workspace, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=settings:voice"]]);
        assert(workspace.launches == 2);
        auto completion = workspace.completion;
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
            completion(nil, [NSError errorWithDomain:@"SyntheticLaunchFailure" code:1 userInfo:nil]);
        });
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
        while (fallbacks < 2 && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
        assert(fallbacks == 2);
        MSIMEOpenDesktopRoute(@"settings:help", workspace, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=settings:help"]]);
        assert(workspace.launches == 3);
        MSIMEOpenDesktopUpdateSettings(workspace, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=settings:about"]]);
        // The short-lived process is what carries the route to an already open settings window.
        assert(workspace.configuration.createsNewApplicationInstance);
        assert(workspace.configuration.activates);
        assert(workspace.launches == 4);
        workspace.completion(NSRunningApplication.currentApplication, nil);
        assert(fallbacks == 2);
        MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Skin, workspace, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=settings:skin"]]);
        assert(workspace.launches == 5);
        workspace.completion(NSRunningApplication.currentApplication, nil);
        assert(fallbacks == 2);
        for (NSArray *entry in @[@[@((int)MSIMEDesktopSettingsPage::Translation), @"--route=settings:expression"],
                                 @[@((int)MSIMEDesktopSettingsPage::AI), @"--route=settings:ai"]]) {
            MSIMEOpenDesktopSettings((MSIMEDesktopSettingsPage)[entry[0] intValue], workspace, fallback);
            assert([workspace.configuration.arguments isEqual:@[entry[1]]]);
            assert(workspace.configuration.createsNewApplicationInstance);
            workspace.completion(NSRunningApplication.currentApplication, nil);
            assert(fallbacks == 2);
        }
        assert(workspace.launches == 7);
        NSString *options = @"/synthetic/共享 配置/runtime-options.json";
        MSIMEOpenDesktopRouteWithOptions(@"settings:input", options, workspace, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=settings:input"]]);
        assert([workspace.configuration.environment isEqual:@{@"MSIME_CLIENT_HOST_OPTIONS":options}]);
        workspace.completion(NSRunningApplication.currentApplication, nil);
        assert(fallbacks == 2 && workspace.launches == 8);
        MSIMEOpenDesktopRouteWithOptions(@"settings:input", @"relative.json", workspace, fallback);
        assert(fallbacks == 3 && workspace.launches == 8);
        MSIMEOpenDesktopRouteWithOptions(@"settings:input", nil, workspace, fallback);
        assert(!workspace.configuration.environment[@"MSIME_CLIENT_HOST_OPTIONS"]);
        assert(workspace.launches == 9);
        // Existing entry points select exactly the same file as the native host.
        MSIMEOpenDesktopRoute(@"settings:ai", workspace, fallback);
        assert([workspace.configuration.environment[@"MSIME_CLIENT_HOST_OPTIONS"] isEqual:MSIMERuntimeOptionsPath()]);
        MSIMEOpenDesktopRoute(@"keyboard", workspace, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=keyboard"]]);
        assert(!workspace.configuration.activates);
        assert(workspace.configuration.createsNewApplicationInstance);
        workspace.completion(NSRunningApplication.currentApplication, nil);
        assert(fallbacks == 3);
        MSIMEOpenDesktopRoute(@"settings:appearance", workspace, fallback);
        assert(workspace.configuration.activates);
        __block BOOL authorized = NO;
        MSIMEOpenDesktopRouteWithContext(@"emoji", options,
            @{@"MSIME_CLIENT_PANEL_SESSION":@"synthetic-session", @"MSIME_CLIENT_HOST_OPTIONS":@"ignored"},
            workspace, ^(NSRunningApplication *application) {
                (void)application;
                assert(NSThread.isMainThread);
                assert(application.processIdentifier == NSRunningApplication.currentApplication.processIdentifier);
                authorized = YES;
            }, fallback);
        assert([workspace.configuration.arguments isEqual:@[@"--route=emoji"]]);
        assert(workspace.configuration.activates && workspace.configuration.createsNewApplicationInstance);
        assert([workspace.configuration.environment[@"MSIME_CLIENT_PANEL_SESSION"] isEqual:@"synthetic-session"]);
        assert([workspace.configuration.environment[@"MSIME_CLIENT_HOST_OPTIONS"] isEqual:options]);
        workspace.completion(NSRunningApplication.currentApplication, nil);
        deadline = [NSDate dateWithTimeIntervalSinceNow:2];
        while (!authorized && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
        assert(authorized && fallbacks == 3);
    }
}
