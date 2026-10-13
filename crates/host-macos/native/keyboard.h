#pragma once
#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>

namespace msime {
template<class Host> bool RestoreLaunchFocus(Host &host, pid_t target, double launched) {
    if (!host.mainThread() || target <= 0 || target == host.ownProcess() || launched <= 0) return false;
    if (host.foreground() != host.ownProcess() || host.launchTime(target) != launched) return false;
    return host.activate(target);
}

// 结果值经 C ABI 传给 Rust；前端只读取受控错误码，不读取输入或进程信息。
enum class KeyboardSendResult : int { Sent = 0, PermissionDenied = 1, NoTarget = 2, TargetChanged = 3, Unavailable = 4 };

// 可注入的系统边界：测试构造真实 CGEvent，但不读取用户输入、切换焦点、请求权限或投递按键。
template<class Host> KeyboardSendResult SendKeyboardKey(Host &host, unsigned short code, uint64_t flags) {
    if (!host.mainThread()) return KeyboardSendResult::Unavailable;
    if (!host.allowed()) return KeyboardSendResult::PermissionDenied;
    const pid_t target = host.foreground();
    if (target <= 0 || target == host.ownProcess()) return KeyboardSendResult::NoTarget;
    CGEventRef down = host.event(code, true);
    CGEventRef up = host.event(code, false);
    if (!down || !up) {
        if (down) CFRelease(down);
        if (up) CFRelease(up);
        return KeyboardSendResult::Unavailable;
    }
    CGEventSetFlags(down, flags);
    CGEventSetFlags(up, flags);
    // 权限或焦点变化后不投递待发送的按键；失败原因保留到界面。
    const KeyboardSendResult result = !host.allowed() ? KeyboardSendResult::PermissionDenied
        : host.foreground() != target ? KeyboardSendResult::TargetChanged : KeyboardSendResult::Sent;
    if (result == KeyboardSendResult::Sent) { host.post(target, down); host.post(target, up); }
    CFRelease(down); CFRelease(up);
    return result;
}

}
