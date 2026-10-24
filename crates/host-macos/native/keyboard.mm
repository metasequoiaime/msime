#include "keyboard.h"
#include "../../../platforms/macos/src/input/ToggleHotkeyEvent.h"
#include "../../../platforms/macos/src/core/ScreenKeyboardWindowAppearance.h"

@interface MSIMEDetachedWindowContent : NSObject
@property(strong) NSWindow *window;
@property(strong) NSView *view;
@end
@implementation MSIMEDetachedWindowContent
@end

extern "C" const char *msime_macos_recorded_toggle_key_name(const char *physicalName) {
    @autoreleasepool {
        return msime::mac::RecordedToggleKeyName(physicalName);
    }
}

extern "C" uintptr_t msime_macos_detach_window_content(uintptr_t address) {
    if (!NSThread.isMainThread) return 0;
    @autoreleasepool {
        for (NSWindow *window in NSApp.windows) {
            if ((uintptr_t)(__bridge void *)window != address || !window.contentView) continue;
            MSIMEDetachedWindowContent *content = [MSIMEDetachedWindowContent new];
            content.window = window;
            content.view = window.contentView;
            // WebKit unregisters its KVO observers while the original window
            // class is still installed. The adapter still needs a content view.
            window.contentView = [[NSView alloc] initWithFrame:content.view.frame];
            return (uintptr_t)CFBridgingRetain(content);
        }
    }
    return 0;
}

extern "C" void msime_macos_restore_window_content(uintptr_t token) {
    @autoreleasepool {
        MSIMEDetachedWindowContent *content = CFBridgingRelease((void *)token);
        content.window.contentView = content.view;
    }
}

namespace {
struct SystemKeyboardHost {
    bool mainThread() { return NSThread.isMainThread; }
    bool allowed() { return CGPreflightPostEventAccess(); }
    pid_t ownProcess() { return NSProcessInfo.processInfo.processIdentifier; }
    pid_t foreground() { return NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier; }
    double launchTime(pid_t target) {
        return [NSRunningApplication runningApplicationWithProcessIdentifier:target].launchDate.timeIntervalSince1970;
    }
    bool activate(pid_t target) {
        return [[NSRunningApplication runningApplicationWithProcessIdentifier:target] activateWithOptions:0];
    }
    CGEventRef event(unsigned short code, bool down) {
        return CGEventCreateKeyboardEvent(nullptr, code, down);
    }
    void post(pid_t target, CGEventRef event) { CGEventPostToPid(target, event); }
};
}

extern "C" int msime_macos_send_keyboard_key_result(unsigned short code, uint64_t flags) {
    @autoreleasepool {
        SystemKeyboardHost host;
        return static_cast<int>(msime::SendKeyboardKey(host, code, flags));
    }
}

extern "C" int msime_macos_capture_launch_target(double *launched) {
    @autoreleasepool {
        SystemKeyboardHost host;
        if (!host.mainThread()) return 0;
        const pid_t target = host.foreground();
        if (target <= 0 || target == host.ownProcess()) return 0;
        *launched = host.launchTime(target);
        return target;
    }
}

extern "C" bool msime_macos_restore_launch_target(int pid, double launched) {
    @autoreleasepool {
        SystemKeyboardHost host;
        return msime::RestoreLaunchFocus(host, pid, launched);
    }
}

extern "C" bool msime_macos_activate_panel_target(int pid, double launched) {
    @autoreleasepool {
        SystemKeyboardHost host;
        if (!host.mainThread() || pid <= 0 || pid == host.ownProcess() || host.launchTime(pid) != launched) return false;
        const pid_t foreground = host.foreground();
        if (foreground == pid) return true;
        // Do not steal focus after the user has selected a third application.
        if (foreground != host.ownProcess()) return false;
        NSRunningApplication *target = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
        if (!target || target.terminated) return false;
        if (@available(macOS 14.0, *)) {
            [NSApp yieldActivationToApplication:target];
            return [target activateFromApplication:NSRunningApplication.currentApplication options:0];
        }
        return [target activateWithOptions:0];
    }
}

extern "C" bool msime_macos_prepare_keyboard_window(uintptr_t address) {
    if (!NSThread.isMainThread) return false;
    for (NSWindow *window in NSApp.windows) {
        // 只接受当前应用的活窗口，不解引用调用者传入的地址。
        if ((uintptr_t)(__bridge void *)window != address || !window.contentView) continue;
        MSIMEConfigureScreenKeyboardWindow(window);
        return true;
    }
    return false;
}
