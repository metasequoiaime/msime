#pragma once
#include "CandidateAction.h"
#include "CandidatePresentation.h"
#include <atomic>
#include <condition_variable>
#include <functional>
#include <thread>

namespace msime::windows {
struct CandidateClick {
  FocusLease lease;
  uint64_t session, generation;
  size_t index;
  CandidateAction action = CandidateAction::Select;
  uint8_t position = 0;
};
struct CandidatePage {
  FocusLease lease;
  uint64_t session;
  uint64_t generation;
  bool previous;
  unsigned steps;
  // 滚轮发起的翻页为 true，首行箭头为 false；Server 只对滚轮套「鼠标滚轮翻页」开关，见 candidate_page_allowed。
  bool from_wheel = false;
};
// One worker and one outstanding click. No backlog/retries. Dependencies must
// outlive stop(); cancel handler I/O before joining when necessary.
template <class Task> class SingleClickWorker final {
public:
  using Handler = std::function<void(const Task &)>;
  explicit SingleClickWorker(Handler handler)
      : handler_(std::move(handler)) {
    if (!handler_)
      throw std::invalid_argument("Missing click handler");
    worker_ = std::thread([this] { run(); });
  }
  ~SingleClickWorker() { stop(); }
  SingleClickWorker(const SingleClickWorker &) = delete;
  SingleClickWorker &operator=(const SingleClickWorker &) = delete;
  bool submit(const Task &click) {
    // Handler/I/O never holds this short state lock.
    std::lock_guard lock(mutex_);
    if (stopping_ || busy_)
      return false;
    pending_ = click;
    busy_ = true;
    ready_.notify_one();
    return true;
  }
  void request_stop() {
    std::lock_guard lock(mutex_);
    stopping_ = true;
    pending_.reset();
    ready_.notify_all();
  }
  void stop() {
    request_stop();
    if (worker_.joinable())
      worker_.join();
  }
  bool failed() const { return failed_.load(); }

private:
  void run() noexcept {
    for (;;) {
      std::optional<Task> click;
      {
        std::unique_lock lock(mutex_);
        ready_.wait(lock, [&] { return stopping_ || pending_.has_value(); });
        if (stopping_)
          return;
        click = std::move(pending_);
        pending_.reset();
      }
      try {
        handler_(*click);
      } catch (...) {
        failed_ = true;
        request_stop();
        return;
      }
      std::lock_guard lock(mutex_);
      busy_ = false;
    }
  }
  Handler handler_;
  std::mutex mutex_;
  std::condition_variable ready_;
  std::optional<Task> pending_;
  bool stopping_ = false, busy_ = false;
  std::atomic<bool> failed_{false};
  std::thread worker_;
};
using CandidateClickWorker = SingleClickWorker<CandidateClick>;
using CandidatePageWorker = SingleClickWorker<CandidatePage>;
} // namespace msime::windows
