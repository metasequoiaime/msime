#pragma once
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#include <initializer_list>

namespace msime::mac {

struct EditableShiftFocus {
    pid_t pid = 0;
    id element = nil;
    uint64_t mouseEvents = 0;
    explicit operator bool() const { return pid > 0 && element != nil; }
    bool operator==(const EditableShiftFocus &other) const {
        return *this && other && pid == other.pid && mouseEvents == other.mouseEvents &&
            CFEqual((__bridge CFTypeRef)element, (__bridge CFTypeRef)other.element);
    }
};

inline bool EditableShiftMouseButtonHeld() {
    for (unsigned int button = 0; button < 32; ++button)
        if (CGEventSourceButtonState(kCGEventSourceStateCombinedSessionState, static_cast<CGMouseButton>(button))) return true;
    return false;
}

// 计数器补上全局鼠标监视回调晚于 IMK 松键通知到达的情况；不读坐标或目标窗口。
inline uint64_t EditableShiftMouseEvents() {
    uint64_t count = 0;
    for (CGEventType type : {kCGEventLeftMouseDown, kCGEventLeftMouseUp, kCGEventRightMouseDown,
                            kCGEventRightMouseUp, kCGEventOtherMouseDown, kCGEventOtherMouseUp,
                            kCGEventLeftMouseDragged, kCGEventRightMouseDragged, kCGEventOtherMouseDragged,
                            kCGEventScrollWheel})
        count += CGEventSourceCounterForEventType(kCGEventSourceStateCombinedSessionState, type);
    return count;
}

// 只查元数据，不读文本或选区，也不在按键路径请求权限。未知实现保守拒绝。
inline EditableShiftFocus CaptureEditableShiftFocus(NSString *clientBundle) {
    if (!clientBundle.length || !AXIsProcessTrusted() || EditableShiftMouseButtonHeld()) return {};
    const uint64_t mouseEvents = EditableShiftMouseEvents();
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (front.processIdentifier <= 0 || ![front.bundleIdentifier isEqual:clientBundle]) return {};
    AXUIElementRef app = AXUIElementCreateApplication(front.processIdentifier);
    if (!app) return {};
    // 每次跨进程消息最多等 20 毫秒；只在 Shift 的两个边沿查询。
    AXError timeout = AXUIElementSetMessagingTimeout(app, 0.02f);
    CFTypeRef raw = nullptr;
    AXError error = timeout == kAXErrorSuccess
        ? AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute, &raw) : timeout;
    CFRelease(app);
    if (error != kAXErrorSuccess || !raw) { if (raw) CFRelease(raw); return {}; }
    id element = CFBridgingRelease(raw);
    if (CFGetTypeID((__bridge CFTypeRef)element) != AXUIElementGetTypeID()) return {};
    AXUIElementRef focus = (__bridge AXUIElementRef)element;
    pid_t focusPID = 0;
    if (AXUIElementGetPid(focus, &focusPID) != kAXErrorSuccess || focusPID != front.processIdentifier ||
        AXUIElementSetMessagingTimeout(focus, 0.02f) != kAXErrorSuccess) return {};
    CFArrayRef values = nullptr;
    NSArray *attributes = @[(__bridge NSString *)kAXRoleAttribute, (__bridge NSString *)kAXEnabledAttribute];
    if (AXUIElementCopyMultipleAttributeValues(focus, (__bridge CFArrayRef)attributes,
                                             kAXCopyMultipleAttributeOptionStopOnError, &values) != kAXErrorSuccess) {
        if (values) CFRelease(values);
        return {};
    }
    NSArray *metadata = CFBridgingRelease(values);
    if (metadata.count != 2 || ![metadata[1] isEqual:@YES]) return {};
    id role = metadata[0];
    if (![role isEqual:(__bridge NSString *)kAXTextFieldRole] &&
        ![role isEqual:(__bridge NSString *)kAXTextAreaRole]) return {};
    Boolean writable = false;
    if (AXUIElementIsAttributeSettable(focus, kAXValueAttribute, &writable) != kAXErrorSuccess || !writable)
        return {};
    if (EditableShiftMouseButtonHeld() || mouseEvents != EditableShiftMouseEvents() || !AXIsProcessTrusted() ||
        NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != front.processIdentifier) return {};
    return {front.processIdentifier, element, mouseEvents};
}

// 和 ModifierTap 并行观察，取消后不在同一轮双侧 Shift 里重新布防。
class EditableShiftGate {
public:
    void reset() { shiftDown_ = false; cancel(); }
    void cancel() { focus_ = {}; }
    bool armed() const { return static_cast<bool>(focus_); }

    template<typename Capture>
    bool observe(NSEvent *event, bool restricted, Capture capture) {
        if (!restricted) { reset(); return true; }
        const auto mask = NSEventModifierFlagShift | NSEventModifierFlagControl |
            NSEventModifierFlagOption | NSEventModifierFlagCommand | NSEventModifierFlagFunction;
        const auto flags = event.modifierFlags & mask;
        const bool previous = shiftDown_;
        shiftDown_ = (flags & NSEventModifierFlagShift) != 0;
        const bool shiftEvent = event.type == NSEventTypeFlagsChanged &&
            (event.keyCode == 56 || event.keyCode == 60);
        if (!shiftDown_) {
            const bool allow = previous && shiftEvent && flags == 0 && armed() && focus_ == capture();
            cancel();
            return allow;
        }
        if (!previous && shiftEvent && flags == NSEventModifierFlagShift) focus_ = capture();
        else if (!shiftEvent || flags != NSEventModifierFlagShift) cancel();
        return false;
    }
private:
    bool shiftDown_ = false;
    EditableShiftFocus focus_;
};

}
