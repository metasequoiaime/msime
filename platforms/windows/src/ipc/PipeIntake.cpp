#include "PipeIntake.h"
#include <cstring>
#include <stdexcept>
#include <system_error>

namespace msime::windows {
PipeIntake::PipeIntake(PipeRegistry &registry, size_t workers, size_t capacity,
                       uint32_t capabilities, DWORD timeout,
                       Completion completion,
                       IdentityRejected identity_rejected)
    : registry_(registry), capacity_(capacity), capabilities_(capabilities),
      timeout_(timeout), completion_(std::move(completion)),
      identity_rejected_(std::move(identity_rejected)) {
  if (!workers || workers > 32 || !capacity || capacity > 1024 || !timeout ||
      timeout == INFINITE || !completion_ ||
      (capabilities & FanyImeProtocol::RequiredCapabilities) !=
          FanyImeProtocol::RequiredCapabilities)
    throw std::invalid_argument("Invalid Windows handshake pool configuration");
  cancel_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (!cancel_)
    throw std::system_error(static_cast<int>(GetLastError()),
                            std::system_category(),
                            "Create handshake cancellation event");
  try {
    workers_.reserve(workers);
    for (size_t i = 0; i < workers; ++i)
      workers_.emplace_back([this] { run(); });
  } catch (...) {
    stop();
    CloseHandle(cancel_);
    cancel_ = nullptr;
    throw;
  }
}
PipeIntake::~PipeIntake() {
  stop();
  CloseHandle(cancel_);
}
bool PipeIntake::submit(std::unique_ptr<PipeConnection> connection,
                        uint32_t role) {
  std::lock_guard lock(mutex_);
  if (!connection || role > FanyImePipeRole::ToTsfWorkerThread || stopping_ ||
      queue_.size() >= capacity_) {
    ++stats_.rejected;
    return false;
  }
  queue_.push_back({std::move(connection), role});
  ready_.notify_one();
  return true;
}
IntakeStats PipeIntake::stats() const {
  std::lock_guard lock(mutex_);
  auto result = stats_;
  result.queued = queue_.size();
  return result;
}
PipeRegistration PipeIntake::handshake(Job job) {
  if (job.role != FanyImePipeRole::Main)
    return registry_.register_reverse(std::move(job.connection), job.role,
                                      timeout_, cancel_);
  auto io = read_frame(job.connection->handle(), sizeof(FanyImeNamedpipeData),
                       timeout_, cancel_);
  if (!io.complete()) {
    PipeRegistration result;
    result.status = RegistryStatus::TransportError;
    result.io = std::move(io);
    return result;
  }
  FanyImeNamedpipeData hello{};
  std::memcpy(&hello, io.frame.data(), sizeof(hello));
  return registry_.register_main(std::move(job.connection), hello,
                                 capabilities_, timeout_, cancel_);
}
void PipeIntake::run() {
  for (;;) {
    Job job;
    {
      std::unique_lock lock(mutex_);
      ready_.wait(lock, [this] { return stopping_ || !queue_.empty(); });
      if (stopping_)
        return;
      job = std::move(queue_.front());
      queue_.pop_front();
      ++stats_.active;
    }
    const auto role = job.role;
    PipeRegistration result;
    bool delivered = false;
    // 握手会接管并关闭连接，对端 pid 要在这之前取。
    ULONG pid = 0;
    if (identity_rejected_ &&
        !GetNamedPipeClientProcessId(job.connection->handle(), &pid))
      pid = 0;
    try {
      result = handshake(std::move(job));
      if (result.status == RegistryStatus::IdentityRejected &&
          identity_rejected_)
        identity_rejected_(pid, result.io.system_error);
      bool stopping;
      {
        std::lock_guard lock(mutex_);
        stopping = stopping_;
      }
      if (!stopping && result.status == RegistryStatus::Ready)
        delivered = completion_(role, result);
    } catch (...) {
      // Do not log hello frames, input, caller exceptions or private paths.
    }
    if (!delivered && result.status == RegistryStatus::Ready)
      registry_.remove(result.ticket, role);
    {
      std::lock_guard lock(mutex_);
      --stats_.active;
      if (!delivered)
        ++stats_.failed;
    }
  }
}
void PipeIntake::request_stop() {
  {
    std::lock_guard lock(mutex_);
    stopping_ = true;
    queue_.clear();
  }
  SetEvent(cancel_);
  ready_.notify_all();
}
void PipeIntake::stop() {
  std::lock_guard stop_lock(stop_mutex_);
  request_stop();
  for (auto &worker : workers_)
    if (worker.joinable())
      worker.join();
}
} // namespace msime::windows
