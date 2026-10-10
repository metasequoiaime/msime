#include "FloatingToolbarVisibilityPolicy.h"

#include <cassert>
#include <cstdio>

namespace {
// 空闲隐藏的检查不靠 assert：Release 构建定义了 NDEBUG，assert 会整个消失。
int failures = 0;
void check(bool value, int line) {
  if (!value) {
    std::fprintf(stderr, "Floating toolbar visibility check failed at line %d\n", line);
    ++failures;
  }
}
#define CHECK(value) check((value), __LINE__)
} // namespace

int main() {
  using msime::windows::FloatingToolbarIdleTimer;
  using msime::windows::kFloatingToolbarIdleMilliseconds;
  using msime::windows::ShouldDeferFloatingToolbarHide;
  using msime::windows::ShouldShowFloatingToolbar;

  assert(ShouldShowFloatingToolbar(true, false, true));
  assert(!ShouldShowFloatingToolbar(false, false, true));
  assert(!ShouldShowFloatingToolbar(true, true, true));
  assert(!ShouldShowFloatingToolbar(true, false, false));
  assert(ShouldDeferFloatingToolbarHide(true));
  assert(!ShouldDeferFloatingToolbarHide(false));

  // 与 macOS 一样连续 10 秒没有输入就隐藏。
  CHECK(kFloatingToolbarIdleMilliseconds == 10000);
  {
    FloatingToolbarIdleTimer timer;
    // 第一次变成可显示（Server 启动、输入法激活）算一次输入，从这一刻计时。
    CHECK(!timer.idle_hidden(1000, true));
    CHECK(!timer.idle_hidden(10999, true));
    CHECK(timer.idle_hidden(11000, true));
    // 隐藏后一直隐藏，直到下一次输入。
    CHECK(timer.idle_hidden(60000, true));
    timer.note_input(60000);
    CHECK(!timer.idle_hidden(60001, true));
    // 输入重新计时，而不是从第一次输入算起。
    timer.note_input(65000);
    CHECK(!timer.idle_hidden(74999, true));
    CHECK(timer.idle_hidden(75000, true));
  }
  {
    // 关掉开关或输入法失活再回来，算一次唤醒，与 macOS 重新激活时一样。
    FloatingToolbarIdleTimer timer;
    CHECK(!timer.idle_hidden(0, true));
    CHECK(timer.idle_hidden(20000, true));
    (void)timer.idle_hidden(20050, false);
    CHECK(!timer.idle_hidden(20100, true));
    CHECK(!timer.idle_hidden(30099, true));
    CHECK(timer.idle_hidden(30100, true));
  }
  {
    // 一直可显示时，轮询本身（偏好刷新、焦点切换都只是又一轮循环）不会唤醒已经空闲隐藏的工具栏。
    FloatingToolbarIdleTimer timer;
    CHECK(!timer.idle_hidden(0, true));
    for (uint64_t now = 10000; now < 40000; now += 50)
      CHECK(timer.idle_hidden(now, true));
  }
  {
    // 维护钩子看不到的按键（屏幕键盘注入的按键、发往提权窗口的按键）经过了 Server 时，计数变了就让空闲隐藏的工具栏回来并重新计时。
    using msime::windows::ServerKeyActivity;
    FloatingToolbarIdleTimer timer;
    CHECK(!timer.idle_hidden(0, true));
    CHECK(timer.idle_hidden(10000, true));
    const uint64_t before = ServerKeyActivity::instance().count();
    timer.note_key_activity(before, 10050);
    CHECK(timer.idle_hidden(10050, true));
    ServerKeyActivity::instance().note();
    timer.note_key_activity(ServerKeyActivity::instance().count(), 20000);
    CHECK(!timer.idle_hidden(20000, true));
    // 计数没再变，轮询本身不续期。
    timer.note_key_activity(ServerKeyActivity::instance().count(), 29999);
    CHECK(!timer.idle_hidden(29999, true));
    timer.note_key_activity(ServerKeyActivity::instance().count(), 30000);
    CHECK(timer.idle_hidden(30000, true));
  }
  return failures == 0 ? 0 : 1;
}
