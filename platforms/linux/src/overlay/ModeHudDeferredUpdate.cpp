#include "ModeHudDeferredUpdate.h"

#include <fcitx-utils/event.h>

namespace msime::linux_host {

ModeHudDeferredUpdate::ModeHudDeferredUpdate(fcitx::EventLoop &loop) : loop_(loop) {}

ModeHudDeferredUpdate::~ModeHudDeferredUpdate() { cancel(); }

void ModeHudDeferredUpdate::schedule(std::string label, Callback callback) {
  cancel();
  const auto generation = generation_;
  event_ = loop_.addDeferEvent(
      [this, generation, label = std::move(label), callback = std::move(callback)](fcitx::EventSource *) mutable {
        if (generation != generation_) return false;
        // 回调仍在 EventLoop 调度这个 source；由下一次 schedule/cancel 或析构统一销毁，不能在这里 reset 自己。
        callback(label);
        return false;
      });
}

void ModeHudDeferredUpdate::cancel() {
  ++generation_;
  event_.reset();
}

} // namespace msime::linux_host
