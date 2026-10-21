#pragma once

#include <cmath>

namespace msime::mac {

/// 悬浮工具栏自动隐藏延时的可选范围（秒），与 Windows 旧正式版 v0.11.1 的
/// `floating_toolbar_auto_hide_delay` 一致（`server/src/config/ime_config_internal.h`）。
inline constexpr int kFloatingToolbarAutoHideDelayMinSeconds = 1;
inline constexpr int kFloatingToolbarAutoHideDelayMaxSeconds = 60;
/// 出厂默认延时。Windows 的 `installer/default_config/config.default.toml` 写的是 5 秒。
inline constexpr int kFloatingToolbarAutoHideDelayDefaultSeconds = 5;

/// 悬浮工具栏空闲行为和阴影的解析结果。面板只按这里定下的值执行，不自己判缺省。
struct FloatingToolbarBehavior {
    bool autoHideEnabled = false;
    int autoHideDelaySeconds = kFloatingToolbarAutoHideDelayDefaultSeconds;
    /// 画不画窗口阴影。与自动隐藏无关：它只随偏好开关变化。
    bool drawsShadow = true;
};

/// 把偏好文档里的原始值解析成面板要执行的行为。出厂默认对齐 Windows v0.11.1 的
/// `installer/default_config/config.default.toml`：自动隐藏关、延时 5 秒、阴影开；延时可选 1–60 秒，
/// 越界或非有限值回退到 5 秒，和 `server/src/config/ime_config.cpp` 的载入校验一致。
/// `hasAutoHide` / `hasDelay` / `hasShadow` 表示文档里有没有这个键；macOS 读其他工具栏键时读不到按新装处理。
inline FloatingToolbarBehavior ResolveFloatingToolbarBehavior(bool hasAutoHide, bool autoHide,
                                                              bool hasDelay, double delay,
                                                              bool hasShadow, bool shadow) {
    FloatingToolbarBehavior behavior;
    behavior.autoHideEnabled = hasAutoHide && autoHide;
    if (hasDelay && std::isfinite(delay)) {
        const int seconds = static_cast<int>(std::lround(delay));
        if (seconds >= kFloatingToolbarAutoHideDelayMinSeconds &&
            seconds <= kFloatingToolbarAutoHideDelayMaxSeconds)
            behavior.autoHideDelaySeconds = seconds;
    }
    behavior.drawsShadow = !hasShadow || shadow;
    return behavior;
}

/// 空闲计时到点时要不要真的隐藏。自动隐藏关掉后残留的计时器仍会走到这里，所以要在这里再判一次。
inline bool ShouldHideFloatingToolbarForIdle(const FloatingToolbarBehavior &behavior, bool imeActive,
                                             bool requestedVisible) {
    return behavior.autoHideEnabled && imeActive && requestedVisible;
}

} // namespace msime::mac
