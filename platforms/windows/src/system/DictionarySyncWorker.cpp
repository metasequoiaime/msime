#include "DictionarySyncWorker.h"
#include "DictionarySyncSchedule.h"
#include "../../../common/HostApiString.h"
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <exception>
#include <mutex>
#include <system_error>
#include <thread>
#include <utility>

namespace msime::windows {

namespace {
// 停下时等工作线程多久：一次同步可能有几十个请求，不能让 Server 退出卡在这里。
constexpr std::chrono::seconds stop_wait{2};

std::uint64_t now_ms() {
  return static_cast<std::uint64_t>(
      std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now().time_since_epoch())
          .count());
}
} // namespace

struct DictionarySyncWorker::State {
  explicit State(bool configured) : schedule(configured) {}
  Options options;
  std::mutex mutex;
  std::condition_variable wake;
  std::condition_variable done;
  DictionarySyncSchedule schedule;
  bool started = false;
  bool stopping = false;
  bool finished = false;
  std::thread worker;
};

namespace {
std::string call_dictionary_sync(const std::string &body) {
  if (auto owned = host_api::own_string(
          msime_client_dictionary_sync(reinterpret_cast<const uint8_t *>(body.data()), body.size())))
    return owned.get();
  return {};
}

// 工作线程：按调度一步一步调用共享层。锁只在读写调度时持有，调用期间放开，偏好和改动的通知从不等网络。同一种失败只记一次，换成别的结果再记，免得每半小时写一行同样的诊断。
void run_worker(const std::shared_ptr<DictionarySyncWorker::State> &state) {
  std::string last_failure;
  std::unique_lock<std::mutex> lock(state->mutex);
  while (!state->stopping) {
    const auto decision = state->schedule.take(now_ms());
    if (decision.step == DictionarySyncStep::idle) {
      if (decision.wait)
        state->wake.wait_for(lock, *decision.wait);
      else
        state->wake.wait(lock);
      continue;
    }
    lock.unlock();
    const char *trigger = decision.step == DictionarySyncStep::local_change ? "local_change" : "periodic";
    const auto body = dictionary_sync_request(state->options.host_options, state->options.directory, trigger)
                          .dump(-1, ' ', false, nlohmann::json::error_handler_t::replace);
    const auto answer = dictionary_sync::with_keystore_retry(
        [&] { return parse_dictionary_sync_answer(call_dictionary_sync(body)); }, dictionary_sync_keystore_conflict);
    const auto &failure = answer.ok ? answer.sync_error : answer.error;
    if (failure != last_failure && state->options.log)
      state->options.log(failure.empty() ? std::string("Dictionary sync ") + trigger + " succeeded"
                                         : std::string("Dictionary sync ") + trigger + " failed: " + failure);
    last_failure = failure;
    lock.lock();
    state->schedule.finished(answer, now_ms());
  }
  state->finished = true;
  lock.unlock();
  state->done.notify_all();
}
} // namespace

DictionarySyncWorker::DictionarySyncWorker(bool configured) : state_(std::make_shared<State>(configured)) {}

DictionarySyncWorker::~DictionarySyncWorker() { stop(); }

void DictionarySyncWorker::start(Options options) {
  {
    std::lock_guard<std::mutex> lock(state_->mutex);
    if (state_->started || state_->stopping)
      return;
    state_->options = std::move(options);
    state_->started = true;
    state_->schedule.start(now_ms());
  }
  auto shared = state_;
  try {
    state_->worker = std::thread([shared] {
      try {
        run_worker(shared);
      } catch (const std::exception &) {
        // 线程边界：自动同步出了意外只结束这个线程，不能让 std::terminate 带走输入法。
        std::lock_guard<std::mutex> lock(shared->mutex);
        shared->finished = true;
        shared->done.notify_all();
      }
    });
  } catch (const std::system_error &) {
    std::lock_guard<std::mutex> lock(state_->mutex);
    state_->started = false;
    throw;
  }
}

void DictionarySyncWorker::configure(bool configured) {
  {
    std::lock_guard<std::mutex> lock(state_->mutex);
    if (state_->stopping)
      return;
    state_->schedule.configure(configured, now_ms());
  }
  state_->wake.notify_all();
}

void DictionarySyncWorker::note_local_change() {
  {
    std::lock_guard<std::mutex> lock(state_->mutex);
    if (state_->stopping)
      return;
    state_->schedule.note_local_change();
  }
  state_->wake.notify_all();
}

void DictionarySyncWorker::stop() {
  {
    std::lock_guard<std::mutex> lock(state_->mutex);
    state_->stopping = true;
  }
  state_->wake.notify_all();
  if (state_->worker.joinable()) {
    bool finished = false;
    {
      std::unique_lock<std::mutex> lock(state_->mutex);
      finished = state_->done.wait_for(lock, stop_wait, [&] { return state_->finished; });
    }
    if (finished)
      state_->worker.join();
    else
      state_->worker.detach();
  }
}

} // namespace msime::windows
