#pragma once

#include "FocusGate.h"

#include <atomic>
#include <condition_variable>
#include <functional>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <unordered_map>
#include <vector>

namespace msime::windows {
// Requests AI 联想 candidates away from the input queue.
//
// Everything below this worker already existed - msime_client_ai_http_request,
// msime_client_parse_ai_response and msime_client_apply_online_candidates are
// exported, and the online query already carries the resolved ai_assistant
// config and an ai_eligible flag - but nothing on Windows ever called them, so
// turning AI 辅助 on in settings changed nothing and no AI candidate appeared.
class AiCandidateWorker final {
public:
  struct Result {
    FocusLease lease;
    // The query this answers, so a stale reply can be recognised.
    std::string query;
    // 空表示这次发问没有拿到可用的回答（失败、被拒或模型没给词），owner 据此告诉 Host API 这次没有回答。
    std::vector<std::string> candidates;
  };
  using Completed = std::function<void(Result)>;
  // Performs one request. `cancelled` answers true once this request has been
  // superseded or the worker is stopping, and is polled during the transfer so
  // a superseded request stops paying for a model call nobody is waiting for.
  // Injected for the same reason as the cloud worker's: what goes wrong above
  // it is queueing, debouncing and cancellation, none of which needs a network
  // or a paid provider to exercise.
  using Fetcher = std::function<std::vector<std::string>(
      const std::string &, const std::string &, const std::function<bool()> &)>;

  explicit AiCandidateWorker(Completed completed);
  AiCandidateWorker(Completed completed, Fetcher fetcher);
  ~AiCandidateWorker();
  AiCandidateWorker(const AiCandidateWorker &) = delete;
  AiCandidateWorker &operator=(const AiCandidateWorker &) = delete;

  // Keeps only the newest request; never waits for a response.
  bool submit(const FocusLease &lease, std::string query, std::string descriptor);
  void request_stop();
  void stop();
  bool failed() const { return failed_.load(std::memory_order_acquire); }

private:
  struct Request {
    FocusLease lease;
    std::string query;
    std::string descriptor;
    uint64_t serial = 0;
  };
  void run();
  bool cancelled(uint64_t serial) const noexcept;
  static std::vector<std::string> fetch(const std::string &query,
                                        const std::string &descriptor,
                                        const std::function<bool()> &cancelled);

  Completed completed_;
  Fetcher fetch_;
  std::mutex mutex_;
  std::mutex join_mutex_;
  std::condition_variable wake_;
  std::optional<Request> pending_;
  uint64_t next_serial_ = 0;
  std::atomic<uint64_t> latest_serial_{0};
  std::atomic<bool> stopping_{false};
  std::atomic<bool> failed_{false};
  // Successful results are reusable across candidate generations. The key is
  // derived from provider identity and pinyin segments, never credentials.
  std::unordered_map<std::string, std::vector<std::string>> candidate_cache_;
  std::thread worker_;
};
} // namespace msime::windows
