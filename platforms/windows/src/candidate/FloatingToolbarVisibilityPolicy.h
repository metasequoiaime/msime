#pragma once
#include <atomic>
#include <cstdint>

namespace msime::windows {

// Temporary focus suspension must not hide the toolbar. Only a real IME
// deactivation or fullscreen presentation removes it.
constexpr bool ShouldShowFloatingToolbar(bool configured_enabled,
                                          bool fullscreen,
                                          bool ime_active) {
  return configured_enabled && !fullscreen && ime_active;
}

// Defer hide requests while the embedded toolbar page is receiving its first
// paint, then reconcile visibility once the page reports ready.
constexpr bool ShouldDeferFloatingToolbarHide(bool paint_grace_active) {
  return paint_grace_active;
}

// 连续这么久没有键盘输入，工具栏自动隐藏，与 macOS 一致。
inline constexpr uint64_t kFloatingToolbarIdleMilliseconds = 10000;

// Server 经 TSF 管道实际收到的按键计数：焦点会话处理的每个键、以及 TIP 交给应用后经 Aux 管道报来的 KeySound 都加一。维护键盘钩子看不到注入的按键（屏幕键盘、SendInput 类的键鼠共享工具）、看不到发往提权窗口的按键，也可能根本没装上，所以工具栏的空闲计时还要看这个计数：用户在这些场景里打字，按键一样经过 Server。输入线程只加一，界面线程每轮循环读一次，无锁、不分配。
class ServerKeyActivity final {
public:
  static ServerKeyActivity &instance() {
    static ServerKeyActivity activity;
    return activity;
  }
  // 输入线程。
  void note() { count_.fetch_add(1, std::memory_order_relaxed); }
  // 界面线程。
  uint64_t count() const { return count_.load(std::memory_order_relaxed); }

private:
  ServerKeyActivity() = default;
  std::atomic<uint64_t> count_{0};
};

// 工具栏的空闲隐藏，与 macOS 的 noteInputForDelegate 同一套规则：按键、点击或拖动工具栏重新计时并让隐藏的工具栏回来；工具栏从不可显示变成可显示（打开开关、输入法重新激活）也算一次输入；计时到了就隐藏，直到下一次输入。偏好刷新和焦点切换不调用 note_input，所以不会唤醒已经空闲隐藏的工具栏。只在 UI 线程上用，时间是 GetTickCount64 的毫秒。
class FloatingToolbarIdleTimer {
public:
  void note_input(uint64_t now) {
    hidden_ = false;
    deadline_ = now + kFloatingToolbarIdleMilliseconds;
  }
  // 每轮循环调用一次，传入 ServerKeyActivity 的计数：和上一轮不同就说明这期间 Server 收到过按键，算一次输入。
  void note_key_activity(uint64_t count, uint64_t now) {
    if (count == key_count_)
      return;
    key_count_ = count;
    note_input(now);
  }
  // 每轮循环调用一次。`active` 是工具栏开关打开且输入法处于激活状态，不含全屏判断：从全屏应用切走只是一次焦点切换，不该唤醒空闲隐藏的工具栏。返回是否因为空闲而隐藏。
  bool idle_hidden(uint64_t now, bool active) {
    if (!active) {
      active_ = false;
      return hidden_;
    }
    if (!active_) {
      active_ = true;
      note_input(now);
    }
    if (!hidden_ && now >= deadline_)
      hidden_ = true;
    return hidden_;
  }

private:
  bool active_ = false;
  bool hidden_ = false;
  uint64_t deadline_ = 0;
  uint64_t key_count_ = 0;
};

}  // namespace msime::windows
