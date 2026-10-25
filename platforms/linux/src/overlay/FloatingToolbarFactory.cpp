#include "FloatingToolbar.h"
#include "FloatingToolbarX11Surface.h"

namespace msime::linux_host {

// 现在只有 X11 后端。Wayland 的 Xwayland 也可使用，但 open_overlay_display 会拒绝尚不支持
// root overlay 的 xwayland-satellite，避免工具栏变成顶层并抢焦点；后端不可用时沿菜单入口回退。
FloatingToolbar::SurfaceFactory FloatingToolbar::default_surfaces(OverlayLoop loop) {
  return [loop](FloatingToolbarSurface::Callbacks callbacks) -> std::unique_ptr<FloatingToolbarSurface> {
    return FloatingToolbarX11Surface::create(loop, std::move(callbacks));
  };
}

}  // namespace msime::linux_host
