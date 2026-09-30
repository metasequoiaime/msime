#pragma once

#include "WaveOverlaySurface.h"

#include <X11/Xlib.h>

#include <chrono>
#include <cstdint>
#include <functional>
#include <utility>
#include <vector>

namespace msime::linux_host {

// Lightweight X11 fallback for desktops that do not expose a layer-shell
// protocol. The surface never takes focus and accepts pointer input only in
// its explicit action-button regions.
class WaveOverlayX11Surface final : public WaveOverlaySurface {
 public:
  explicit WaveOverlayX11Surface(ActionHandler action_handler = {})
      : action_handler_(std::move(action_handler)) {}
  ~WaveOverlayX11Surface() override;

  bool show(const WaveOverlayModel &model) override;
  void update(const WaveOverlayModel &model) override;
  void hide() override;

 private:
  bool ensure_window();
  void place(bool force);
  void load_font_set();
  int scaled(int logical) const;
  void draw(const WaveOverlayModel &model);
  void pump_events();
  void set_input_region(bool actions_visible);
  bool hit_test_action(int x, int y, WaveOverlayModel::Action &action) const;
  void destroy_window();
  unsigned long palette_pixel(std::uint32_t rgb, unsigned long fallback);

  Display *display_ = nullptr;
  Window window_ = 0;
  GC gc_ = nullptr;
  XFontSet font_set_ = nullptr;
  unsigned long background_ = 0;
  unsigned long foreground_ = 0;
  unsigned long accent_ = 0;
  unsigned long light_background_ = 0;
  unsigned long light_foreground_ = 0;
  unsigned long light_accent_ = 0;
  unsigned long border_ = 0;
  // The window border currently set, so a palette change re-borders the window once rather than on every level update.
  unsigned long current_border_ = 0;
  // Pixels allocated for the resolved theme's colours, keyed by RGB: a theme has a handful, and draw() runs for every level change.
  std::vector<std::pair<std::uint32_t, unsigned long>> palette_pixels_;
  bool visible_ = false;
  ActionHandler action_handler_;
  WaveOverlayModel::Action pressed_action_ = WaveOverlayModel::Action::Confirm;
  bool action_pressed_ = false;
  bool actions_visible_ = false;
  // RandR 1.5 monitor enumeration; without it the bar is placed on the EWMH work area or the root window.
  bool randr_monitors_ = false;
  double scale_ = 1.0;
  unsigned width_ = 0;
  unsigned height_ = 0;
  std::chrono::steady_clock::time_point placed_at_{};
};

}  // namespace msime::linux_host
