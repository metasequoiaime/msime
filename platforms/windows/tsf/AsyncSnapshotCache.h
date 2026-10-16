#pragma once

#include <atomic>
#include <chrono>
#include <memory>
#include <mutex>
#include <utility>

namespace msime::tsf {
// 加载器和调度器可注入；读取快照从不调用加载器，也不等待正在进行的 I/O。
template <class T> class AsyncSnapshotCache final {
public:
  using Snapshot = std::shared_ptr<const T>;
  using Clock = std::chrono::steady_clock;

  Snapshot snapshot() const { return std::atomic_load(&snapshot_); }

  // 仅首次激活调用：在决定游戏 UILess 策略前准备配置，不让默认值覆盖用户规则。
  template <class Loader> void initialize(Loader load) {
    std::call_once(initializedOnce_, [this, &load] {
      publish(load);
      initialized_.store(true, std::memory_order_release);
    });
  }

  template <class Loader, class Schedule>
  bool refresh(Loader load, Schedule schedule, Clock::time_point now = Clock::now()) {
    if (!initialized_.load(std::memory_order_acquire)) return false;
    bool expected = false;
    if (!inFlight_.compare_exchange_strong(expected, true)) return false;
    // 此字段仅由持有 inFlight_ 的线程访问；慢读期间不会排队补跑。
    if (now < nextRefresh_) {
      inFlight_.store(false, std::memory_order_release);
      return false;
    }
    nextRefresh_ = now + std::chrono::milliseconds(250);
    try {
      if (schedule([this, load = std::move(load)] {
            publish(load);
            inFlight_.store(false, std::memory_order_release);
          })) return true;
    } catch (...) {}
    inFlight_.store(false, std::memory_order_release);
    return false;
  }

private:
  template <class Loader> void publish(Loader &load) noexcept {
    try {
      if (auto next = load(snapshot())) std::atomic_store(&snapshot_, Snapshot(std::move(next)));
    } catch (...) {
      // 忙碌、读取失败或解析失败时保留最近一次完整发布的值。
    }
  }

  Snapshot snapshot_;
  std::once_flag initializedOnce_;
  std::atomic<bool> initialized_{false};
  std::atomic<bool> inFlight_{false};
  Clock::time_point nextRefresh_{};
};
}
