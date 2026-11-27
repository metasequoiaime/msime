#pragma once

#include <memory>

#include "FloatingToolbarSurface.h"
#include "OverlayLoop.h"

namespace msime::linux_host {

std::unique_ptr<FloatingToolbarSurface> create_wayland_floating_toolbar(
    OverlayLoop loop, FloatingToolbarSurface::Callbacks callbacks);

}  // namespace msime::linux_host
