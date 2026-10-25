#pragma once

#include "../../../shared/contracts/grouped_panel_key.h"
#include <array>
#include <cstdint>

namespace msime::panel_key {
struct Owner {
  std::uint64_t generation = 0;
  std::uint64_t worker_token = 0;
  std::uintptr_t context = 0;
  std::uintptr_t foreground = 0;
  std::uintptr_t focus = 0;
  bool operator==(const Owner &other) const {
    return generation == other.generation && worker_token == other.worker_token &&
           context == other.context && foreground == other.foreground && focus == other.focus;
  }
  bool valid() const {
    return generation && worker_token && context && foreground && focus;
  }
};

// 所有方法只在 TIP 所在线程调用。只返回布尔值，不经 LRESULT 截断 64 位焦点代次。
class Leases final {
public:
  static constexpr std::uint64_t LifetimeMs = 500;
  bool query(std::uint64_t nonce, const Owner &owner, std::uint64_t now) {
    if (!nonce || !owner.valid()) return false;
    Slot *available = nullptr;
    for (auto &slot : slots_) {
      if (expired(slot, now)) slot = {};
      // 同 nonce 不能刷新时限、换主或再次执行。
      if (slot.nonce == nonce) return false;
      if (!slot.nonce && !available) available = &slot;
    }
    if (!available) return false;
    *available = {nonce, now, owner, State::Pending};
    return true;
  }

  template <class Enqueue>
  bool press(std::uint64_t nonce, const Owner &owner, std::uint64_t now, Enqueue enqueue) {
    if (!nonce || !owner.valid()) return false;
    for (auto &slot : slots_) {
      if (slot.nonce != nonce) continue;
      if (expired(slot, now) || !(slot.owner == owner)) return false;
      if (slot.state != State::Pending) return slot.state == State::Accepted;
      // 入队可能触发同步回调，先消费，保证重入不会重复入队。
      slot.state = State::Rejected;
      const bool accepted = enqueue();
      if (slot.nonce == nonce && slot.owner == owner)
        slot.state = accepted ? State::Accepted : State::Rejected;
      return accepted;
    }
    return false;
  }
  void clear() { slots_ = {}; }

private:
  enum class State { Pending, Accepted, Rejected };
  struct Slot {
    std::uint64_t nonce = 0;
    std::uint64_t created = 0;
    Owner owner;
    State state = State::Pending;
  };
  static bool expired(const Slot &slot, std::uint64_t now) {
    return slot.nonce && (now < slot.created || now - slot.created >= LifetimeMs);
  }
  std::array<Slot, 8> slots_{};
};
} // namespace msime::panel_key
