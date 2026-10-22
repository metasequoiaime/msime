#include "../../src/core/FloatingToolbarBehavior.h"

#include <cassert>
#include <cmath>
#include <limits>

using msime::mac::ResolveFloatingToolbarBehavior;
using msime::mac::ShouldHideFloatingToolbarForIdle;

int main() {
  // 缺省按 Windows 旧正式版 v0.11.1 的出厂配置：自动隐藏关、延时 5 秒。
  const auto defaults = ResolveFloatingToolbarBehavior(false, false, false, 0.0, false, false);
  assert(!defaults.autoHideEnabled);
  assert(defaults.autoHideDelaySeconds == 5);

  // 自动隐藏开着时用给定的 1–60 秒。
  assert(ResolveFloatingToolbarBehavior(true, true, true, 1.0, true, true).autoHideDelaySeconds == 1);
  assert(ResolveFloatingToolbarBehavior(true, true, true, 12.0, true, true).autoHideDelaySeconds == 12);
  assert(ResolveFloatingToolbarBehavior(true, true, true, 60.0, true, true).autoHideDelaySeconds == 60);
  // 越界和非有限值回退到出厂默认的 5 秒，不是夹到最近边界。
  assert(ResolveFloatingToolbarBehavior(true, true, true, 0.0, true, true).autoHideDelaySeconds == 5);
  assert(ResolveFloatingToolbarBehavior(true, true, true, 61.0, true, true).autoHideDelaySeconds == 5);
  assert(ResolveFloatingToolbarBehavior(true, true, true, -3.0, true, true).autoHideDelaySeconds == 5);
  assert(ResolveFloatingToolbarBehavior(true, true, true, std::nan(""), true, true).autoHideDelaySeconds == 5);
  assert(ResolveFloatingToolbarBehavior(true, true, true, std::numeric_limits<double>::infinity(), true, true)
             .autoHideDelaySeconds == 5);
  // 文档里没有这个键时同样按缺省，延时没有值时不采用调用方传进来的数。
  assert(!ResolveFloatingToolbarBehavior(false, true, true, 10.0, true, true).autoHideEnabled);
  assert(ResolveFloatingToolbarBehavior(true, true, false, 0.0, true, true).autoHideDelaySeconds == 5);

  // 空闲计时到点：三个条件都成立才隐藏；自动隐藏关掉后残留的计时器不隐藏。
  const auto enabled = ResolveFloatingToolbarBehavior(true, true, true, 10.0, true, true);
  assert(ShouldHideFloatingToolbarForIdle(enabled, true, true));
  assert(!ShouldHideFloatingToolbarForIdle(enabled, false, true));
  assert(!ShouldHideFloatingToolbarForIdle(enabled, true, false));
  const auto disabled = ResolveFloatingToolbarBehavior(true, false, true, 10.0, true, true);
  assert(!ShouldHideFloatingToolbarForIdle(disabled, true, true));

  // 阴影：出厂为开（Windows v0.11.1 的 `floating_toolbar_shadow = true`，也是 macOS 一直画的那种柔和阴影）。
  assert(ResolveFloatingToolbarBehavior(false, false, false, 0.0, false, false).drawsShadow);
  assert(ResolveFloatingToolbarBehavior(true, true, true, 10.0, true, true).drawsShadow);
  // 关上就是关上，和自动隐藏无关。
  assert(!ResolveFloatingToolbarBehavior(true, true, true, 10.0, true, false).drawsShadow);
  assert(!ResolveFloatingToolbarBehavior(false, false, false, 0.0, true, false).drawsShadow);

  return 0;
}
