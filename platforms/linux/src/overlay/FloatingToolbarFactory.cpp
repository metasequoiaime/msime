#include "FloatingToolbar.h"
#include "FloatingToolbarX11Surface.h"
#include "FloatingToolbarPlatform.h"

#include <cstdlib>

namespace msime::linux_host {

// 现在只有 X11 后端。Wayland 的 Xwayland 不能保证 override-redirect 不进入合成器的顶层
// 窗口列表；先沿菜单入口回退，避免切换输入模式时反复产生 MSIMEFloatingToolbar 窗口事件。
FloatingToolbar::SurfaceFactory FloatingToolbar::default_surfaces(OverlayLoop loop) {
  if (!floating_toolbar_should_use_x11(std::getenv("WAYLAND_DISPLAY"), std::getenv("DISPLAY"))) return {};
  return [loop](FloatingToolbarSurface::Callbacks callbacks) -> std::unique_ptr<FloatingToolbarSurface> {
    return FloatingToolbarX11Surface::create(loop, std::move(callbacks));
  };
}

}  // namespace msime::linux_host
