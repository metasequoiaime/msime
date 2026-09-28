#include "keyboard.h"
#include <cassert>
#include <vector>

struct TestHost {
    bool main = true, permission = true, losePermission = false, failDown = false, failUp = false;
    pid_t target = 123, laterTarget = 123;
    int reads = 0, checks = 0, allocations = 0;
    std::vector<CGEventType> events;
    bool mainThread() { return main; }
    bool allowed() { return permission && !(losePermission && checks++ > 0); }
    pid_t ownProcess() { return 456; }
    pid_t foreground() { return reads++ == 0 ? target : laterTarget; }
    CGEventRef event(unsigned short code, bool down) {
        allocations++;
        if (down ? failDown : failUp) return nullptr;
        return CGEventCreateKeyboardEvent(nullptr, code, down);
    }
    void post(pid_t pid, CGEventRef event) {
        assert(pid == 123);
        assert(CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) == 0);
        assert(CGEventGetFlags(event) == (kCGEventFlagMaskCommand | kCGEventFlagMaskAlternate));
        events.push_back(CGEventGetType(event));
    }
};

int main() {
    @autoreleasepool {
        const uint64_t flags = kCGEventFlagMaskCommand | kCGEventFlagMaskAlternate;
        TestHost host;
        assert(msime::SendKeyboardKey(host, 0, flags));
        assert((host.events == std::vector<CGEventType>{kCGEventKeyDown, kCGEventKeyUp}));
        for (int scenario = 0; scenario < 8; ++scenario) {
            TestHost denied;
            if (scenario == 0) denied.main = false;
            if (scenario == 1) denied.permission = false;
            if (scenario == 2) denied.target = 0;
            if (scenario == 3) denied.target = 456;
            if (scenario == 4) denied.laterTarget = 789;
            if (scenario == 5) denied.failDown = true;
            if (scenario == 6) denied.failUp = true;
            if (scenario == 7) denied.losePermission = true;
            assert(!msime::SendKeyboardKey(denied, 0, flags));
            assert(denied.events.empty());
            if (scenario < 4) assert(denied.allocations == 0);
        }
        struct FocusHost {
            bool main = true, activated = false;
            pid_t front = 456;
            double launched = 42;
            bool mainThread() { return main; }
            pid_t ownProcess() { return 456; }
            pid_t foreground() { return front; }
            double launchTime(pid_t) { return launched; }
            bool activate(pid_t pid) { assert(pid == 123); activated = true; return true; }
        };
        FocusHost focus;
        assert(msime::RestoreLaunchFocus(focus, 123, 42) && focus.activated);
        for (int scenario = 0; scenario < 4; ++scenario) {
            FocusHost denied;
            if (scenario == 0) denied.main = false;
            if (scenario == 1) denied.front = 789; // User selected another app.
            if (scenario == 2) denied.launched = 43; // PID was reused.
            if (scenario == 3) denied.launched = 0; // App terminated.
            assert(!msime::RestoreLaunchFocus(denied, 123, 42));
            assert(!denied.activated);
        }
    }
}
