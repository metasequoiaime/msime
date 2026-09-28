#import "../../src/settings/PreferencesWindowController.h"
#import "../../src/settings/AppearancePreferences.h"
#import "PreferenceViewLookup.h"
#include <cassert>

// Deliver AppKit close notifications to the real window delegate without presenting a window.
@interface HiddenPreferencesController : MSIMEPreferencesWindowController
@end
@implementation HiddenPreferencesController
- (void)showWindow:(id)sender { (void)sender; }
@end

static void DrainMainQueue() {
    __block BOOL drained = NO;
    dispatch_async(dispatch_get_main_queue(), ^{ drained = YES; });
    while (!drained) {
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.001]];
    }
}

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        HiddenPreferencesController *controller = [[HiddenPreferencesController alloc] initWithWindow:nil];
        __block NSUInteger closes = 0;
        id observer = [NSNotificationCenter.defaultCenter
            addObserverForName:MSIMEStandalonePreferencesDidCloseNotification
            object:controller queue:nil usingBlock:^(NSNotification *note) {
                (void)note;
                ++closes;
            }];
        // The input method runs LSBackgroundOnly; a prohibited process cannot bring its window in front of the app being typed in.
        [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
        [controller showAndActivate];
        assert(NSApp.activationPolicy == NSApplicationActivationPolicyAccessory);
        assert(controller.window == MSIMEAppearancePreferences.sharedPreferences.window);
        assert(controller.window.delegate == controller);
        NSControl *uninstall = MSIMEFindPreferenceControl(controller.window.contentView, @selector(uninstallInputSource:));
        assert(uninstall && uninstall.accessibilityLabel.length > 0);
        [NSNotificationCenter.defaultCenter postNotificationName:NSWindowWillCloseNotification object:controller.window];
        DrainMainQueue();
        assert(closes == 0); // In-process settings must not terminate the input method.

        [controller showAndActivateForStandaloneLaunch];
        [NSNotificationCenter.defaultCenter postNotificationName:NSWindowWillCloseNotification object:controller.window];
        assert(closes == 0); // Termination is deferred until AppKit finishes closing.
        DrainMainQueue();
        assert(closes == 1);
        [NSNotificationCenter.defaultCenter postNotificationName:NSWindowWillCloseNotification object:controller.window];
        DrainMainQueue();
        assert(closes == 1);

        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        [controller showAndActivateForStandaloneLaunch];
        [controller showAndActivate];
        assert(NSApp.activationPolicy == NSApplicationActivationPolicyRegular); // The standalone launch keeps its Dock icon.
        [NSNotificationCenter.defaultCenter postNotificationName:NSWindowWillCloseNotification object:controller.window];
        DrainMainQueue();
        assert(closes == 1); // Ordinary presentation clears standalone state.
        [NSNotificationCenter.defaultCenter removeObserver:observer];
    }
    return 0;
}
