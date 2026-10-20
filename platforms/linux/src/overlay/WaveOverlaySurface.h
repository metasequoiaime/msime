#pragma once
#include "WaveOverlayModel.h"
#include <functional>
namespace msime::linux_host {
class WaveOverlaySurface {
 public:
  using ActionHandler = std::function<void(WaveOverlayModel::Action)>;
  virtual ~WaveOverlaySurface() = default;
  virtual bool show(const WaveOverlayModel&) = 0;
  virtual void update(const WaveOverlayModel&) = 0;
  virtual void hide() = 0;
  // The display connection the shown surface reads its pointer and expose events from, or -1. The host watches it on its own loop while the surface is shown and calls dispatch_events() when it is readable, so a click on an action is handled at once rather than at the next redraw.
  virtual int event_fd() const { return -1; }
  virtual void dispatch_events() {}
  // Whether the shown surface draws the wave itself and so wants a frame every kWaveOverlayFrameMilliseconds while the model animates; a text fallback does not.
  virtual bool animates() const { return false; }
};
}
