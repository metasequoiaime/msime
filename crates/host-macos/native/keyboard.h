#pragma once
#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>

namespace msime {
template<class Host> bool RestoreLaunchFocus(Host &host, pid_t target, double launched) {
    if (!host.mainThread() || target <= 0 || target == host.ownProcess() || launched <= 0) return false;
    if (host.foreground() != host.ownProcess() || host.launchTime(target) != launched) return false;
    return host.activate(target);
}

// Injectable OS boundary: tests exercise real CGEvent construction without
// reading user input, changing focus, requesting permissions or posting keys.
template<class Host> bool SendKeyboardKey(Host &host, unsigned short code, uint64_t flags) {
    if (!host.mainThread() || !host.allowed()) return false;
    const pid_t target = host.foreground();
    if (target <= 0 || target == host.ownProcess()) return false;
    CGEventRef down = host.event(code, true);
    CGEventRef up = host.event(code, false);
    if (!down || !up) {
        if (down) CFRelease(down);
        if (up) CFRelease(up);
        return false;
    }
    CGEventSetFlags(down, flags);
    CGEventSetFlags(up, flags);
    // Do not deliver a pending click after a focus/permission change.
    const bool valid = host.allowed() && host.foreground() == target;
    if (valid) { host.post(target, down); host.post(target, up); }
    CFRelease(down); CFRelease(up);
    return valid;
}

}
