#include "FloatingToolbar.h"
#include "FloatingToolbarX11Surface.h"
#include "FloatingToolbarPlatform.h"
#ifdef MSIME_LINUX_WAYLAND_TOOLBAR
#include "FloatingToolbarWaylandSurface.h"
#endif

#include <cstdlib>

namespace msime::linux_host {

FloatingToolbar::SurfaceFactory FloatingToolbar::default_surfaces(OverlayLoop loop) {
  const char *wayland = std::getenv("WAYLAND_DISPLAY");
#ifdef MSIME_LINUX_WAYLAND_TOOLBAR
  if (wayland && *wayland) {
    return [loop](FloatingToolbarSurface::Callbacks callbacks) {
      return create_wayland_floating_toolbar(loop, std::move(callbacks));
    };
  }
#endif
  if (!floating_toolbar_should_use_x11(wayland, std::getenv("DISPLAY"))) return {};
  return [loop](FloatingToolbarSurface::Callbacks callbacks) -> std::unique_ptr<FloatingToolbarSurface> {
    return FloatingToolbarX11Surface::create(loop, std::move(callbacks));
  };
}

}  // namespace msime::linux_host
