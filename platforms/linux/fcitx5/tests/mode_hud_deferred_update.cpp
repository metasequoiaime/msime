#include "../../src/overlay/ModeHudDeferredUpdate.h"

#include <fcitx-utils/event.h>

#include <stdexcept>
#include <string>

namespace {
void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}
}

int main() {
  fcitx::EventLoop loop;
  msime::linux_host::ModeHudDeferredUpdate update(loop);
  bool called = false;
  std::string label;
  update.schedule("旧", [&](const std::string &value) {
    called = true;
    label = value;
  });
  update.schedule("英", [&](const std::string &value) {
    called = true;
    label = value;
  });
  require(!called, "模式提示不能在 schedule 返回前同步执行");
  auto stop = loop.addTimeEvent(
      CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC) + 1000, 0,
      [&](fcitx::EventSourceTime *timer, uint64_t) {
        timer->setEnabled(false);
        loop.exit();
        return true;
      });
  loop.exec();
  require(called && label == "英", "延后模式提示没有丢弃旧代次");

  fcitx::EventLoop cancel_loop;
  msime::linux_host::ModeHudDeferredUpdate cancelled(cancel_loop);
  called = false;
  cancelled.schedule("中", [&](const std::string &) { called = true; });
  cancelled.cancel();
  auto stop_after_cancel = cancel_loop.addTimeEvent(
      CLOCK_MONOTONIC, fcitx::now(CLOCK_MONOTONIC), 0,
      [&](fcitx::EventSourceTime *timer, uint64_t) {
        timer->setEnabled(false);
        cancel_loop.exit();
        return true;
      });
  cancel_loop.exec();
  require(!called, "取消模式提示后仍执行旧回调");
  return 0;
}
