#pragma once
#include "PipeTicket.h"
#include <atomic>
#include <limits>
#include <mutex>
#include <optional>
#include <utility>

namespace msime::windows {
struct FocusLease {
  PipeTicket transport;
  uint64_t epoch = 0;
  uint64_t token = 0;
};
struct FocusChange {
  std::optional<FocusLease> previous;
  FocusLease pending;
  bool previous_ready = false;
};
// Mechanism only: caller authenticates lifecycle events and chooses when an
// activation changes. Engine composition still belongs to ServerSession.
class FocusGate final {
public:
  std::optional<FocusChange> begin(const PipeTicket &ticket, uint64_t token) {
    std::lock_guard lock(mutex_);
    if (!ticket.client || !token)
      return std::nullopt;
    for (auto generation : ticket.generations)
      if (!generation)
        return std::nullopt;
    // Match the Windows server's monotonically advancing epoch: zero is
    // reserved as invalid, but exhausting uint64_t must not permanently
    // disable every later focus activation.
    if (next_epoch_ == std::numeric_limits<uint64_t>::max())
      next_epoch_ = 0;
    FocusChange change{current_, {ticket, ++next_epoch_, token}, ready_};
    current_ = change.pending;
    ready_ = false;
    publish_locked();
    return change;
  }
  // Prepare the queue-owned Engine session before scheduling its worker fence.
  template <typename Action>
  bool with_pending(const FocusLease &lease, Action &&action) {
    std::lock_guard lock(mutex_);
    if (!matches(lease) || ready_)
      return false;
    std::forward<Action>(action)();
    return true;
  }
  // Writer performs the ordered worker fence and returns true only on complete
  // delivery. Do not re-enter this gate from either callback. The transport
  // registry must revalidate the ticket while writing. Uncertain writes fail.
  template <typename Writer>
  bool acknowledge(const FocusLease &lease, Writer &&writer) {
    std::lock_guard lock(mutex_);
    if (!matches(lease) || ready_)
      return false;
    try {
      if (std::forward<Writer>(writer)()) {
        ready_ = true;
        return true;
      }
    } catch (...) {
      current_.reset();
      ready_ = false;
      publish_locked();
      throw;
    }
    current_.reset();
    ready_ = false;
    publish_locked();
    return false;
  }
  // Check AND execute under the same focus lock so a new activation cannot
  // overtake a send after its check. Actions must be short/bounded. Input-queue
  // tasks must separately obey ServerSession's thread affinity.
  template <typename Action>
  bool with_active(const FocusLease &lease, Action &&action) {
    std::lock_guard lock(mutex_);
    if (!matches(lease) || !ready_)
      return false;
    std::forward<Action>(action)();
    return true;
  }
  // UI reads must not wait behind a pipe writer. False includes a busy gate;
  // callers must fail closed instead of displaying a cached owner's content.
  template <typename Action>
  bool try_with_active(const FocusLease &lease, Action &&action) {
    std::unique_lock lock(mutex_, std::try_to_lock);
    if (!lock || !matches(lease) || !ready_)
      return false;
    std::forward<Action>(action)();
    return true;
  }
  bool deactivate(const FocusLease &lease) {
    std::lock_guard lock(mutex_);
    if (!matches(lease))
      return false;
    current_.reset();
    ready_ = false;
    publish_locked();
    return true;
  }
  // Drop whatever focus is current, whoever owns it. Used when every session
  // is being torn down at once, so no lease outlives the session behind it.
  void invalidate_all() {
    std::lock_guard lock(mutex_);
    current_.reset();
    ready_ = false;
    publish_locked();
  }
  bool invalidate(const PipeTicket &ticket) {
    std::lock_guard lock(mutex_);
    if (!current_ || !same_ticket(current_->transport, ticket))
      return false;
    current_.reset();
    ready_ = false;
    publish_locked();
    return true;
  }
  // Snapshot the currently acknowledged focus lease for host work that must
  // be re-dispatched after a settings publication. Pending activations are
  // deliberately excluded: their Engine session is not yet authoritative.
  std::optional<FocusLease> active() const {
    std::lock_guard lock(mutex_);
    return ready_ ? current_ : std::nullopt;
  }
  // Lock-free: whether `lease` is still the focus this gate holds. try_with_active reports a busy gate and a lost lease alike as false; a UI thread that must tell them apart without waiting behind a pipe writer asks this. Every activation takes a new epoch, so a different focus, a deactivation or an invalidated transport all answer false. The answer can be stale only for the instant of a concurrent change.
  bool still_current(const FocusLease &lease) const {
    return lease.epoch != 0 && current_epoch_.load(std::memory_order_acquire) == lease.epoch;
  }

private:
  void publish_locked() {
    current_epoch_.store(current_ ? current_->epoch : 0, std::memory_order_release);
  }
  bool matches(const FocusLease &lease) const {
    return current_ && current_->epoch == lease.epoch &&
           current_->token == lease.token &&
           same_ticket(current_->transport, lease.transport);
  }
  mutable std::mutex mutex_;
  uint64_t next_epoch_ = 0;
  std::optional<FocusLease> current_;
  bool ready_ = false;
  // current_'s epoch (0 for none), mirrored for still_current().
  std::atomic<uint64_t> current_epoch_{0};
};
} // namespace msime::windows
