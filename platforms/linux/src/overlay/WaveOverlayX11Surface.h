#pragma once

#include "WaveOverlayPainter.h"
#include "WaveOverlaySurface.h"

#include <X11/Xlib.h>
#include <cairo/cairo.h>

#include <chrono>
#include <functional>
#include <utility>

namespace msime::linux_host {

// Lightweight X11 fallback for desktops that do not expose a layer-shell protocol. The surface never takes focus and accepts pointer input only in its explicit action-button regions. It draws with cairo-xlib and Pango through WaveOverlayPainter: on a 32-bit ARGB visual with translucent rounded corners when a compositing manager runs, otherwise opaque and cut to the rounded shape with the Shape extension.
class WaveOverlayX11Surface final : public WaveOverlaySurface {
 public:
  explicit WaveOverlayX11Surface(ActionHandler action_handler = {})
      : action_handler_(std::move(action_handler)) {}
  ~WaveOverlayX11Surface() override;

  bool show(const WaveOverlayModel &model) override;
  void update(const WaveOverlayModel &model) override;
  void hide() override;
  int event_fd() const override;
  void dispatch_events() override;
  bool animates() const override { return true; }

 private:
  bool ensure_display();
  bool ensure_window();
  bool compositor_running() const;
  void destroy_window();
  void close_display();
  void place(bool force);
  void apply_shapes();
  void draw();
  void pump_events();

  Display *display_ = nullptr;
  Window window_ = 0;
  Visual *visual_ = nullptr;
  Colormap colormap_ = 0;
  cairo_surface_t *cairo_surface_ = nullptr;
  // The window has a 32-bit visual with alpha because a compositing manager was running when it was made; without one the panel is opaque and shaped instead.
  bool translucent_ = false;
  bool made_composited_ = false;
  WaveOverlayPainter painter_;
  // The last model shown, for an Expose redraw and for hit testing between updates.
  WaveOverlayModel model_;
  WaveOverlayFrame frame_;
  bool visible_ = false;
  ActionHandler action_handler_;
  WaveOverlayModel::Action pressed_action_ = WaveOverlayModel::Action::Confirm;
  bool action_pressed_ = false;
  // RandR 1.5 monitor enumeration; without it the bar is placed on the EWMH work area or the root window.
  bool randr_monitors_ = false;
  double scale_ = 1.0;
  // The device size and the logical size it was computed from; the shapes are cut for this geometry and the panel's actions.
  unsigned width_ = 0;
  unsigned height_ = 0;
  int logical_width_ = 0;
  int logical_height_ = 0;
  bool shaped_actions_ = false;
  bool shaped_ = false;
  std::chrono::steady_clock::time_point placed_at_{};
};

}  // namespace msime::linux_host
