#pragma once
#import <AppKit/AppKit.h>
#import <os/log.h>
#import "EditionIdentity.h"

// Window presentation tracing for the windows the input method opens on the user's behalf: which button or menu asked, whether the settings app was found and launched or the in-process fallback taken, how long a window took to build, and where it stood among the other applications' windows afterwards.
//
// The unified log rather than diagnostic.log: that file is only configured once preferences have loaded and is off by default, while a window that opens behind the editor has to be diagnosable on the machine it happened on, after the fact, with `log show --predicate 'subsystem == "app.msime.inputmethod.MetasequoiaIME" && category == "ui"'`. Like diagnostic.log it carries only state: window numbers, classes and titles, policies, flags, timings and error codes - never input text, candidates or paths.

static inline os_log_t MSIMEUILog(void) {
    static os_log_t log;
    static dispatch_once_t once;
    // 子系统是本版本输入法的 bundle id，同时安装的几个版本的日志不会混在一起。
    dispatch_once(&once, ^{ log = os_log_create(MSIMEInputMethodBundleIdentifier().UTF8String, "ui"); });
    return log;
}

static inline const char *MSIMEActivationPolicyName(NSApplicationActivationPolicy policy) {
    switch (policy) {
        case NSApplicationActivationPolicyRegular: return "regular";
        case NSApplicationActivationPolicyAccessory: return "accessory";
        case NSApplicationActivationPolicyProhibited: return "prohibited";
    }
    return "unknown";
}

// The window's place among on-screen windows of its own level, 0 being frontmost; -1 when it is not on screen at all. This is the number that says "behind the editor", which isVisible does not: a window covered by another application's is still visible.
static inline NSInteger MSIMEWindowFrontIndex(NSWindow *window) {
    if (!window || window.windowNumber <= 0) return -1;
    CFArrayRef list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
    if (!list) return -1;
    NSArray<NSDictionary *> *windows = CFBridgingRelease(list);
    NSNumber *layer = nil;
    for (NSDictionary *info in windows)
        if ([info[(id)kCGWindowNumber] integerValue] == window.windowNumber) { layer = info[(id)kCGWindowLayer]; break; }
    if (!layer) return -1;
    NSInteger index = 0;
    for (NSDictionary *info in windows) {
        if (![info[(id)kCGWindowLayer] isEqual:layer]) continue;
        if ([info[(id)kCGWindowNumber] integerValue] == window.windowNumber) return index;
        ++index;
    }
    return -1;
}

static inline void MSIMELogWindowState(const char *stage, NSWindow *window) {
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!window) {
        os_log(MSIMEUILog(), "%{public}s window=nil app_active=%d policy=%{public}s frontmost=%{public}@", stage,
               NSApp.isActive, MSIMEActivationPolicyName(NSApp.activationPolicy), front.bundleIdentifier ?: @"-");
        return;
    }
    os_log(MSIMEUILog(),
           "%{public}s window=%ld class=%{public}@ title=%{public}@ visible=%d key=%d occluded=%d level=%ld front_index=%ld app_active=%d policy=%{public}s frontmost=%{public}@",
           stage, (long)window.windowNumber, NSStringFromClass(window.class), window.title ?: @"",
           window.isVisible, window.isKeyWindow, (window.occlusionState & NSWindowOcclusionStateVisible) == 0,
           (long)window.level, (long)MSIMEWindowFrontIndex(window), NSApp.isActive,
           MSIMEActivationPolicyName(NSApp.activationPolicy), front.bundleIdentifier ?: @"-");
}

// Process-wide activation, key and close events, registered once however many translation units include this header: the flag lives on the main thread's dictionary rather than in a per-file static.
static inline void MSIMEObserveWindowEvents(void) {
    NSMutableDictionary *shared = NSThread.mainThread.threadDictionary;
    if (shared[@"MSIMEWindowEventObservers"]) return;
    NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
    NSMutableArray *observers = [NSMutableArray array];
    [observers addObject:[center addObserverForName:NSApplicationDidBecomeActiveNotification object:nil queue:NSOperationQueue.mainQueue
        usingBlock:^(NSNotification *note) { (void)note; MSIMELogWindowState("app_did_become_active", nil); }]];
    [observers addObject:[center addObserverForName:NSApplicationDidResignActiveNotification object:nil queue:NSOperationQueue.mainQueue
        usingBlock:^(NSNotification *note) { (void)note; MSIMELogWindowState("app_did_resign_active", nil); }]];
    [observers addObject:[center addObserverForName:NSWindowDidBecomeKeyNotification object:nil queue:NSOperationQueue.mainQueue
        usingBlock:^(NSNotification *note) { MSIMELogWindowState("window_did_become_key", note.object); }]];
    [observers addObject:[center addObserverForName:NSWindowDidResignKeyNotification object:nil queue:NSOperationQueue.mainQueue
        usingBlock:^(NSNotification *note) { MSIMELogWindowState("window_did_resign_key", note.object); }]];
    [observers addObject:[center addObserverForName:NSWindowWillCloseNotification object:nil queue:NSOperationQueue.mainQueue
        usingBlock:^(NSNotification *note) { MSIMELogWindowState("window_will_close", note.object); }]];
    shared[@"MSIMEWindowEventObservers"] = observers;
}

// The state right after presenting says what was asked for; the state a moment later says what the window server and the other applications made of it.
static inline void MSIMELogWindowFollowUps(NSWindow *window) {
    __weak NSWindow *weakWindow = window;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.5 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        NSWindow *current = weakWindow;
        if (current) MSIMELogWindowState("present_after_500ms", current);
    });
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(2 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
        NSWindow *current = weakWindow;
        if (current) MSIMELogWindowState("present_after_2s", current);
    });
}
