#pragma once

#include <atomic>
#include <cstdint>

namespace msime::windows {

// The Server loop may finish sending an older snapshot after the preference
// monitor has published a newer one. A revision lets the sender acknowledge
// only the snapshot it actually sent, leaving newer work pending.
class RevisionFence final {
public:
  uint64_t snapshot() const {
    return revision_.load(std::memory_order_acquire);
  }

  void mark_changed() {
    revision_.fetch_add(1, std::memory_order_release);
  }

  bool is_current(uint64_t snapshot) const {
    return revision_.load(std::memory_order_acquire) == snapshot;
  }

private:
  std::atomic<uint64_t> revision_{1};
};

} // namespace msime::windows
