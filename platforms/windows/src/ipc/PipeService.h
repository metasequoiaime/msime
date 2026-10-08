#pragma once
#include "PipeIntake.h"
#include <atomic>

namespace msime::windows {
struct PipeServiceOptions {
  // Explicit names indexed by the upstream Main/ToTsf/Worker roles. Tests must
  // use isolated names. No implicit production namespace is published.
  std::array<std::wstring, 3> names;
  size_t max_clients = 64;
  size_t handshake_workers = 2;
  size_t pending_handshakes = 64;
  uint32_t capabilities = 0; // Dispatcher must explicitly opt in.
  DWORD handshake_timeout = 250;
  // 可选，见 PipeIntake::IdentityRejected。
  PipeIntake::IdentityRejected identity_rejected;
};
// Transport assembly, not a TSF installer or Engine input dispatcher.
class PipeService final {
public:
  PipeService(PipeServiceOptions options, PipeIntake::Completion completion);
  ~PipeService();
  PipeService(const PipeService &) = delete;
  PipeService &operator=(const PipeService &) = delete;
  void request_stop(); // May be called from a completion; does not join.
  void stop(); // Control thread: joins owned listeners/handshake workers.
  DWORD failure() const { return failure_.load(); }
  PipeRegistry &registry() { return registry_; }
  // If external input workers use registry(), call stop(), then join those
  // workers BEFORE destroying this service. Registry shutdown cancels reads.

private:
  void listen(uint32_t role);
  PipeRegistry registry_;
  HANDLE cancel_ = nullptr;
  std::array<std::unique_ptr<PipeListener>, 3> listeners_;
  std::unique_ptr<PipeIntake> intake_;
  std::vector<std::thread> threads_;
  std::mutex stop_mutex_;
  std::atomic<DWORD> failure_{ERROR_SUCCESS};
};
} // namespace msime::windows
