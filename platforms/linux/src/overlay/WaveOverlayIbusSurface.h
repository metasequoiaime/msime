#pragma once

#include "WaveOverlayFeedback.h"
#include "WaveOverlaySurface.h"
#include <ibus.h>
#include <string>

namespace msime::linux_host {

// IBus fallback surface. A native Wayland/X11 surface can implement the same
// WaveOverlaySurface contract without changing ClientEngine's voice lifecycle.
class WaveOverlayIbusSurface final : public WaveOverlaySurface {
 public:
  explicit WaveOverlayIbusSurface(IBusEngine *engine) : engine_(engine) {}

  bool show(const WaveOverlayModel &model) override;
  void update(const WaveOverlayModel &model) override;
  void hide() override;

 private:
  IBusEngine *engine_ = nullptr;
  bool visible_ = false;
};

}  // namespace msime::linux_host
