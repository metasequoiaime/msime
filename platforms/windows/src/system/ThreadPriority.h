#pragma once

namespace msime::windows {
// 调用线程的临时提权。禁用、读不到进入时的优先级或系统拒绝新优先级时都不做任何
// 改动，绝不打断调用方的工作；析构恢复的是进入时捕获的值，而不是硬编码的 Normal。
class ScopedThreadPriorityBoost final {
public:
  explicit ScopedThreadPriorityBoost(int priority, bool enabled = true);
  ~ScopedThreadPriorityBoost();
  ScopedThreadPriorityBoost(const ScopedThreadPriorityBoost &) = delete;
  ScopedThreadPriorityBoost &operator=(const ScopedThreadPriorityBoost &) = delete;

  bool active() const { return active_; }

private:
  int previous_ = 0;
  bool active_ = false;
};
} // namespace msime::windows
