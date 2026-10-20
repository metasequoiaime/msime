#include "WaveOverlaySurfaceFactory.h"

#include "WaveOverlayIbusSurface.h"

#ifdef MSIME_LINUX_HAS_X11_OVERLAY
#include "WaveOverlayX11Surface.h"
#endif
#ifdef MSIME_LINUX_HAS_WAYLAND_SURFACE
#include "WaveOverlayWaylandSurface.h"
#endif

#include <glib.h>

#include <utility>

namespace msime::linux_host {

namespace {

class FallbackSurface final : public WaveOverlaySurface {
 public:
  FallbackSurface(std::unique_ptr<WaveOverlaySurface> primary,
                  std::unique_ptr<WaveOverlaySurface> fallback)
      : primary_(std::move(primary)), fallback_(std::move(fallback)) {}

  bool show(const WaveOverlayModel &model) override {
    if (primary_ && primary_->show(model)) {
      using_primary_ = true;
      return true;
    }
    using_primary_ = false;
    return fallback_ && fallback_->show(model);
  }

  void update(const WaveOverlayModel &model) override {
    if (using_primary_) {
      if (primary_)
        primary_->update(model);
    } else if (fallback_) {
      fallback_->update(model);
    }
  }

  void hide() override {
    if (primary_)
      primary_->hide();
    if (fallback_)
      fallback_->hide();
    using_primary_ = false;
  }

  int event_fd() const override {
    return using_primary_ && primary_ ? primary_->event_fd() : -1;
  }

  void dispatch_events() override {
    if (using_primary_ && primary_)
      primary_->dispatch_events();
  }

  bool animates() const override {
    return using_primary_ && primary_ && primary_->animates();
  }

 private:
  std::unique_ptr<WaveOverlaySurface> primary_;
  std::unique_ptr<WaveOverlaySurface> fallback_;
  bool using_primary_ = false;
};

}  // namespace

std::unique_ptr<WaveOverlaySurface> create_wave_overlay_surface(
    IBusEngine *engine, WaveOverlaySurface::ActionHandler action_handler) {
  const auto *requested = g_getenv("MSIME_WAVE_OVERLAY_BACKEND");
  const bool force_ibus = requested && g_strcmp0(requested, "ibus") == 0;
  const bool wayland_requested = requested && g_strcmp0(requested, "wayland") == 0;
  const bool x11_requested = requested && g_strcmp0(requested, "x11") == 0;
  (void)force_ibus;
  (void)action_handler;
#ifdef MSIME_LINUX_HAS_WAYLAND_SURFACE
  if (!force_ibus &&
      (wayland_requested || (!x11_requested && g_getenv("WAYLAND_DISPLAY")))) {
    return std::make_unique<FallbackSurface>(
        std::make_unique<WaveOverlayWaylandSurface>(std::move(action_handler)),
        std::make_unique<WaveOverlayIbusSurface>(engine));
  }
#else
  (void)wayland_requested;
#endif
#ifdef MSIME_LINUX_HAS_X11_OVERLAY
  if (!force_ibus &&
      (x11_requested || g_getenv("DISPLAY"))) {
    return std::make_unique<FallbackSurface>(
        std::make_unique<WaveOverlayX11Surface>(std::move(action_handler)),
        std::make_unique<WaveOverlayIbusSurface>(engine));
  }
#else
  (void)x11_requested;
#endif
  return std::make_unique<WaveOverlayIbusSurface>(engine);
}

}  // namespace msime::linux_host
