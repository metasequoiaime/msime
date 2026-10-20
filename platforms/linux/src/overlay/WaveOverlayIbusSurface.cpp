#include "WaveOverlayIbusSurface.h"

#include <ibus.h>

#include <string>

namespace msime::linux_host {
bool WaveOverlayIbusSurface::show(const WaveOverlayModel &model) {
  if (!engine_)
    return false;
  visible_ = true;
  update(model);
  return true;
}

void WaveOverlayIbusSurface::update(const WaveOverlayModel &model) {
  if (!engine_ || !visible_)
    return;
  const auto feedback = wave_overlay_feedback_text(model);
  ibus_engine_update_auxiliary_text(
      engine_, ibus_text_new_from_string(feedback.c_str()), TRUE);
}

void WaveOverlayIbusSurface::hide() {
  if (engine_ && visible_)
    ibus_engine_hide_auxiliary_text(engine_);
  visible_ = false;
}

}  // namespace msime::linux_host
