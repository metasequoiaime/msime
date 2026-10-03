#pragma once
#include "../../../../shared/contracts/msime_edition.h"
#include "VoiceControllerConnection.h"
#include "VoiceControllerMailbox.h"
#include <thread>

namespace msime::windows {
// Owns all pipe I/O; never calls microphone/UI/dispatcher methods. One
// authenticated controller at a time, with finite handshake/idle/dispatch
// waits.
class VoiceControllerListener final {
public:
  static std::unique_ptr<VoiceControllerListener>
  create(VoiceControllerMailbox &mailbox, DWORD &error,
         const std::wstring &name =
             L"\\\\.\\pipe\\FanyImeVoiceControlNamedPipe" MSIME_EDITION_NAME_SUFFIX) {
    auto pipe = PipeListener::create(name, error);
    if (!pipe)
      return {};
    auto listener = std::unique_ptr<VoiceControllerListener>(
        new VoiceControllerListener(mailbox));
    listener->cancel_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (!listener->cancel_) {
      error = GetLastError();
      return {};
    }
    listener->pipe_ = std::move(pipe);
    try {
      listener->worker_ = std::thread([self = listener.get()] { self->run(); });
    } catch (...) {
      error = ERROR_NOT_ENOUGH_MEMORY;
      return {};
    }
    error = ERROR_SUCCESS;
    return listener;
  }
  ~VoiceControllerListener() {
    stop();
    if (cancel_)
      CloseHandle(cancel_);
  }
  void stop() {
    std::lock_guard lock(stop_mutex_);
    if (cancel_)
      SetEvent(cancel_);
    if (worker_.joinable())
      worker_.join();
  }
  DWORD failure() const { return failure_.load(); }

private:
  explicit VoiceControllerListener(VoiceControllerMailbox &mailbox)
      : mailbox_(mailbox) {}
  bool stopping() const {
    return WaitForSingleObject(cancel_, 0) != WAIT_TIMEOUT;
  }
  void serve(std::unique_ptr<VoiceControllerConnection> connection) {
    auto channel = std::make_shared<VoiceControllerChannel>();
    struct Disconnect {
      std::shared_ptr<VoiceControllerChannel> channel;
      ~Disconnect() { channel->alive.store(false); }
    } disconnect{channel};
    while (!stopping()) {
      // Clients must poll more often than five seconds while holding a session.
      auto request = connection->receive(5000, cancel_);
      if (!request)
        return;
      auto job = std::make_shared<VoiceControllerJob>();
      job->channel = channel;
      job->request = std::move(*request);
      if (!mailbox_.publish(job))
        return;
      const auto deadline =
          std::chrono::steady_clock::now() + std::chrono::seconds(10);
      std::optional<VoiceControllerResponse> response;
      while (!stopping() && std::chrono::steady_clock::now() < deadline) {
        response = job->wait(std::chrono::milliseconds(20));
        if (response)
          break;
      }
      if (stopping() || !response ||
          !connection->reply(response->header, response->text, 2000, cancel_))
        return;
    }
  }
  void run() noexcept {
    try {
      while (!stopping()) {
        auto accepted = pipe_->accept(1000, cancel_);
        if (accepted.io.status == IoStatus::Timeout)
          continue;
        if (accepted.io.status == IoStatus::Cancelled)
          return;
        if (!accepted.io.complete()) {
          if (accepted.io.system_error == ERROR_NO_DATA ||
              accepted.io.system_error == ERROR_PIPE_NOT_CONNECTED) {
            if (WaitForSingleObject(cancel_, 20) != WAIT_TIMEOUT)
              return;
            continue;
          }
          failure_.store(accepted.io.system_error ? accepted.io.system_error
                                                  : ERROR_GEN_FAILURE);
          return;
        }
        auto connection = VoiceControllerConnection::accept(
            std::move(accepted.connection), 2000, cancel_);
        if (connection)
          serve(std::move(connection));
      }
    } catch (...) {
      failure_.store(ERROR_GEN_FAILURE);
    }
  }
  VoiceControllerMailbox &mailbox_; // outlives worker
  std::unique_ptr<PipeListener> pipe_;
  HANDLE cancel_ = nullptr;
  std::thread worker_;
  std::mutex stop_mutex_;
  std::atomic<DWORD> failure_{ERROR_SUCCESS};
};
} // namespace msime::windows
