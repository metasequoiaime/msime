#pragma once
#import <AppKit/AppKit.h>
#include <cstddef>
#include <functional>
#include <memory>
#include <vector>
#include "../voice/VoiceTextCommit.h"

// Rewrites the character left of the caret in a host that exposes no document text (Terminal.app, iTerm2, the VS Code terminal), where insertText:replacementRange: has nothing to replace. It is the macOS counterpart of the reference's SendInput rewrite (_QueueSmartPunctuationSendInputRewrite / _RunSmartPunctuationSendInputRewrite): one Delete down/up pair takes the character off the screen, then one Unicode keystroke types the replacement. The events carry the same self tag as the voice sendinput route, so handleEvent: lets them through to the application instead of treating them as typing.
//
// Posting needs the Accessibility permission. It is only ever checked here, never requested: a prompt raised from a keystroke would steal focus from the editor being typed into, which is the same rule the screen keyboard follows.

// The reference gives its queued rewrite 500 ms (SMART_PUNCTUATION_SENDINPUT_TIMEOUT_MS) before the spot it was computed against no longer counts as the one under the caret.
static constexpr NSTimeInterval MSIMESmartPunctuationRewriteTimeout = 0.5;
static constexpr CGKeyCode MSIMESmartPunctuationRewriteDeleteKey = 51; // kVK_Delete

struct MSIMESmartPunctuationRewriteIO {
    std::function<bool()> permitted = [] { return CGPreflightPostEventAccess(); };
    std::function<CGEventRef(CGKeyCode, bool)> create = [](CGKeyCode key, bool down) {
        return CGEventCreateKeyboardEvent(nullptr, key, down);
    };
    std::function<void(pid_t, CGEventRef)> post = [](pid_t pid, CGEventRef event) { CGEventPostToPid(pid, event); };
    std::function<NSTimeInterval()> now = [] { return NSProcessInfo.processInfo.systemUptime; };
};

struct MSIMESmartPunctuationRewrite {
    static constexpr std::size_t kEventCount = 4;
    pid_t pid = 0;
    NSTimeInterval deadline = 0;
    // Still the application the client belongs to, frontmost and running.
    std::function<bool()> current;

    // Returns true only when the whole burst was posted; nothing is posted otherwise.
    bool deliver(unichar replacement, const MSIMESmartPunctuationRewriteIO &io = {}) const {
        if (!replacement || pid <= 0 || !current) return false;
        if (!io.permitted() || !current() || io.now() > deadline) return false;
        using Event = std::unique_ptr<__CGEvent, decltype(&CFRelease)>;
        std::vector<Event> events;
        events.reserve(kEventCount);
        for (const bool unicode : {false, true}) {
            for (const bool down : {true, false}) {
                Event event(io.create(unicode ? 0 : MSIMESmartPunctuationRewriteDeleteKey, down), CFRelease);
                if (!event) return false;
                CGEventSetFlags(event.get(), 0);
                CGEventSetIntegerValueField(event.get(), kCGEventSourceUserData, MSIMEVoiceCommitEventTag);
                if (unicode) CGEventKeyboardSetUnicodeString(event.get(), 1, &replacement);
                events.push_back(std::move(event));
            }
        }
        // Focus can move while the events were being built; a burst for the old editor is dropped whole.
        if (!current() || io.now() > deadline) return false;
        for (const Event &event : events) io.post(pid, event.get());
        return true;
    }
};

// Captures the frontmost application when it is the one the client belongs to. Any other frontmost application, or this process itself, gives a route that delivers nothing.
static inline MSIMESmartPunctuationRewrite MSIMECaptureSmartPunctuationRewrite(id client, NSTimeInterval now) {
    MSIMESmartPunctuationRewrite route;
    if (![client respondsToSelector:@selector(bundleIdentifier)]) return route;
    NSString *bundle = [[client bundleIdentifier] copy];
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!bundle.length || ![application.bundleIdentifier isEqual:bundle] ||
        application.processIdentifier == NSProcessInfo.processInfo.processIdentifier) return route;
    route.pid = application.processIdentifier;
    route.deadline = now + MSIMESmartPunctuationRewriteTimeout;
    route.current = [application, bundle] {
        NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
        return !application.terminated && front.processIdentifier == application.processIdentifier &&
            [front.bundleIdentifier isEqual:bundle];
    };
    return route;
}
