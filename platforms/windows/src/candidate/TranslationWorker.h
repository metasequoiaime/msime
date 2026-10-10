#pragma once

#include "CandidateGlossReadings.h"
#include "FocusGate.h"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <functional>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <unordered_map>

namespace msime::windows {
class TranslationWorker final {
public:
  struct Result {
    FocusLease lease;
    uint64_t generation = 0;
    std::string translations;
    // 这一页候选的读音（打开「显示读音」时）和整句逐词拆解（打开离线英文释义时），只用于显示，不交给会话；见 CandidateMailbox::translations。
    CandidateReadings readings{};
  };
  using Completed = std::function<void(Result)>;
  // Produces one request's translations, or nothing when the query is not
  // translatable. `cancelled` answers true once the request has been superseded
  // or the worker is stopping. Injected so the queueing above it - debouncing,
  // supersession, the cache invalidation that preferences trigger - can be
  // exercised without a provider, credentials or a network; the providers
  // themselves are covered by the shared credential tests.
  using Translator = std::function<std::optional<Result>(
      const FocusLease &, const std::string &, const std::function<bool()> &)>;

  explicit TranslationWorker(Completed completed);
  TranslationWorker(Completed completed, Translator translator);
  // 「水杉账号」候选释义用的账号目录（UTF-8 绝对路径）：设置应用登录的 account-session.json 和本机匿名账号都在这里，见 server_main.cpp 的 anonymous_account_directory。整个进程一份，Server 启动时设一次；没有设时选了水杉账号也不联网。
  static void set_account_directory(std::string directory);
  ~TranslationWorker();
  TranslationWorker(const TranslationWorker &) = delete;
  TranslationWorker &operator=(const TranslationWorker &) = delete;

  bool submit(const FocusLease &lease, std::string query);
  // Invalidate reusable and negative results when preferences make the
  // provider inapplicable. The request serial is advanced too, so an
  // in-flight response cannot repopulate a cache after the invalidation.
  void clear_cache();
  void request_stop();
  void stop();

private:
  struct Request {
    FocusLease lease;
    std::string query;
    uint64_t serial = 0;
  };

  void run() noexcept;
  bool cancelled(uint64_t serial) const noexcept;
  // Takes the lease and query rather than the queued Request: the serial is the
  // queue's bookkeeping and means nothing to a translation.
  std::optional<Result> translate(const FocusLease &lease,
                                  const std::string &query,
                                  const std::function<bool()> &cancelled);

  Completed completed_;
  Translator translate_;
  mutable std::mutex mutex_;
  std::condition_variable wake_;
  std::optional<Request> pending_;
  uint64_t next_serial_ = 0;
  std::atomic<uint64_t> latest_serial_{0};
  std::atomic<bool> clear_cache_requested_{false};
  std::atomic<bool> stopping_{false};
  std::mutex join_mutex_;
  // Results are reusable across generations. Keys include provider scope,
  // target language, direction, and source/target items, but no credentials.
  std::unordered_map<std::string, std::string> translation_cache_;
  std::unordered_map<std::string, std::chrono::steady_clock::time_point>
      translation_negative_cache_;
  std::thread worker_;
};
} // namespace msime::windows
