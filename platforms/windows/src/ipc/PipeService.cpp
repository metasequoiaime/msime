#include "PipeService.h"
#include <stdexcept>
#include <system_error>

namespace msime::windows {
PipeService::PipeService(PipeServiceOptions options,
                         PipeIntake::Completion completion)
    : registry_(options.max_clients) {
  static_assert(FanyImePipeRole::Main == 0 && FanyImePipeRole::ToTsf == 1 &&
                FanyImePipeRole::ToTsfWorkerThread == 2);
  if (!options.max_clients || options.max_clients > 1024)
    throw std::invalid_argument("Invalid Windows pipe client capacity");
  cancel_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (!cancel_)
    throw std::system_error(static_cast<int>(GetLastError()),
                            std::system_category(),
                            "Create listener cancellation event");
  try {
    // Validate/start the bounded pool before publishing any pipe name. It has
    // no jobs and cannot invoke completion until a listener submits work.
    intake_ = std::make_unique<PipeIntake>(
        registry_, options.handshake_workers, options.pending_handshakes,
        options.capabilities, options.handshake_timeout, std::move(completion),
        std::move(options.identity_rejected));
    for (size_t role = 0; role < listeners_.size(); ++role) {
      DWORD error = ERROR_SUCCESS;
      listeners_[role] = PipeListener::create(options.names[role], error);
      if (!listeners_[role])
        throw std::system_error(static_cast<int>(error), std::system_category(),
                                "Create Windows pipe listener");
    }
    threads_.reserve(3);
    for (uint32_t role = 0; role < 3; ++role)
      threads_.emplace_back([this, role] { listen(role); });
  } catch (...) {
    stop();
    CloseHandle(cancel_);
    cancel_ = nullptr;
    throw;
  }
}
PipeService::~PipeService() {
  stop();
  CloseHandle(cancel_);
}
void PipeService::request_stop() {
  SetEvent(cancel_);
  if (intake_)
    intake_->request_stop();
}
void PipeService::stop() {
  std::lock_guard lock(stop_mutex_);
  request_stop();
  for (auto &thread : threads_)
    if (thread.joinable())
      thread.join();
  if (intake_)
    intake_->stop();
  registry_.shutdown();
  for (auto &listener : listeners_)
    listener.reset();
}
void PipeService::listen(uint32_t role) {
  DWORD failure = ERROR_SUCCESS;
  try {
    while (WaitForSingleObject(cancel_, 0) == WAIT_TIMEOUT) {
      auto accepted = listeners_[role]->accept(1000, cancel_);
      if (accepted.io.complete()) {
        // submit owns/ closes rejected connections and never grows its queue
        // beyond the configured capacity. No Engine work runs here.
        intake_->submit(std::move(accepted.connection), role);
        continue;
      }
      if (accepted.io.status == IoStatus::Timeout)
        continue;
      if (accepted.io.status == IoStatus::Cancelled)
        return;
      if (accepted.io.system_error == ERROR_NO_DATA ||
          accepted.io.system_error == ERROR_PIPE_NOT_CONNECTED) {
        // A client can disappear before accept finishes. Avoid a busy loop.
        if (WaitForSingleObject(cancel_, 20) != WAIT_TIMEOUT)
          return;
        continue;
      }
      failure = accepted.io.system_error ? accepted.io.system_error
                                         : ERROR_GEN_FAILURE;
      break;
    }
  } catch (...) {
    failure = ERROR_UNHANDLED_EXCEPTION; // Never expose exception/input text.
  }
  if (failure != ERROR_SUCCESS) {
    DWORD expected = ERROR_SUCCESS;
    failure_.compare_exchange_strong(expected, failure);
    request_stop(); // Joining belongs to the control thread, not this listener.
  }
}
} // namespace msime::windows
