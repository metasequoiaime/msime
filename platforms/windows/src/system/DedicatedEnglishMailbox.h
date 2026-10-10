#pragma once
#include "FocusGate.h"

namespace msime::windows {
// 托盘「英文候选模式」交给工作线程的请求：把这个焦点租约的 Engine 英文模式设成 `enabled`。
struct DedicatedEnglishSwitch {
  FocusLease lease;
  bool enabled = false;
};
// Only a mode bit and its owning focus lease cross into the UI thread.
class DedicatedEnglishMailbox {
public:
  void publish(const FocusLease &lease, bool enabled) {
    std::lock_guard lock(mutex_);
    latest_ = std::pair{lease, enabled};
  }
  std::optional<bool> snapshot(const FocusLease &lease) {
    std::lock_guard lock(mutex_);
    if (!latest_ || latest_->first.epoch != lease.epoch ||
        latest_->first.token != lease.token ||
        !same_ticket(latest_->first.transport, lease.transport))
      return std::nullopt;
    return latest_->second;
  }
private:
  std::mutex mutex_;
  std::optional<std::pair<FocusLease, bool>> latest_;
};
} // namespace msime::windows
