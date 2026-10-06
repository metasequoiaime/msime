#pragma once

#include <cstdint>
#include <mutex>

namespace msime::windows {

// TIP 这一侧对「焦点会话处于 Engine 自己的英文模式」的镜像。权威值归 Server：它读焦点会话的英文模式，每 250 毫秒最多一次，变了才用 DedicatedEnglishChanged 推过来（`server`）。Ctrl+Shift+E 发给 Server 时 TIP 不读回复，如果只等推送，切换之后 250 毫秒内键入的字母仍按旧状态分类（笔画方案空闲时会把字母交给应用而不是 Engine）。所以 TIP 吞下切换键的那一刻先把镜像翻过来（`toggled`），后面的按键在同一线程上按顺序分类，一定看到新值。这个临时值只在 `kConfirmMs` 内有效：推送一到就以推送为准；超时还没有推送，说明 Server 那边没切换（按键在发出前被丢弃，或者输入被禁用），Server 的值没变也就不会再推送，于是退回推送过的值，不会一直错下去。
class DedicatedEnglishMirror {
public:
  // Server 读一次再推一次，加上 IPC 往返，远小于这个时限。
  static constexpr uint64_t kConfirmMs = 1000;

  void server(bool enabled) {
    std::lock_guard lock(mutex_);
    server_ = enabled;
    deadline_ = 0;
  }

  void toggled(uint64_t now) {
    std::lock_guard lock(mutex_);
    pending_ = !current(now);
    deadline_ = now + kConfirmMs;
  }

  bool active(uint64_t now) {
    std::lock_guard lock(mutex_);
    return current(now);
  }

private:
  bool current(uint64_t now) const { return deadline_ != 0 && now < deadline_ ? pending_ : server_; }

  std::mutex mutex_;
  bool server_ = false;
  bool pending_ = false;
  // 0 表示没有待确认的切换。
  uint64_t deadline_ = 0;
};

} // namespace msime::windows
