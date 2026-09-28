#pragma once
#import <AppKit/AppKit.h>
#import "WindowPresentationLog.h"

// Brings a window this process opened on the user's behalf to the front. The input method is LSBackgroundOnly, and a prohibited application cannot become active, so without this the window opens behind whatever app the user was typing in and the button that opened it looks dead. Accessory, as the update window does, lets it activate without a Dock icon; a standalone launch has already chosen Regular and keeps it. Activation is only a request since macOS 14 and may be declined, so the window is also ordered above the other applications' windows regardless; it then takes keyboard focus on the user's first click in it.
static inline void MSIMEPresentWindow(NSWindow *window) {
    MSIMEObserveWindowEvents();
    MSIMELogWindowState("present_begin", window);
    if (!window) return;
    if (NSApp.activationPolicy == NSApplicationActivationPolicyProhibited)
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
    [NSApp activateIgnoringOtherApps:YES];
    [window makeKeyAndOrderFront:nil];
    [window orderFrontRegardless];
    MSIMELogWindowState("present_end", window);
    MSIMELogWindowFollowUps(window);
}
