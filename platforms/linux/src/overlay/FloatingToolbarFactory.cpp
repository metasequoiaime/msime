#include "FloatingToolbar.h"
#include "FloatingToolbarX11Surface.h"

namespace msime::linux_host {

// 现在只有 X11 后端。Wayland 会话里有 DISPLAY（Xwayland）时也用它：工具栏不跟随光标，只要一个屏幕坐标里的位置，Xwayland 的根窗口坐标在 GNOME、KDE 和 wlroots 合成器上都对得上桌面布局；没有 Xwayland 时拿不到后端，宿主只用菜单里的「工具栏」子菜单。
FloatingToolbar::SurfaceFactory FloatingToolbar::default_surfaces(OverlayLoop loop) {
  return [loop](FloatingToolbarSurface::Callbacks callbacks) -> std::unique_ptr<FloatingToolbarSurface> {
    return FloatingToolbarX11Surface::create(loop, std::move(callbacks));
  };
}

}  // namespace msime::linux_host
