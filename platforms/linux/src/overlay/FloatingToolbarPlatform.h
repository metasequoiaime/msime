#pragma once

namespace msime::linux_host {

// X11 工具栏只在纯 X11 会话中可用。Wayland 的 Xwayland 不保证 override-redirect
// 窗口不进入合成器的顶层窗口列表，因此不能把它当作悬浮工具栏的承载后端。
inline bool floating_toolbar_should_use_x11(const char *wayland_display, const char *x11_display) {
  return (!wayland_display || !*wayland_display) && x11_display && *x11_display;
}

}  // namespace msime::linux_host
