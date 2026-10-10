#include "AiCandidateWorker.h"

#include <chrono>
#include <condition_variable>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

using namespace msime::windows;
namespace {
void require(bool value, const char *what) {
  if (!value)
    throw std::runtime_error(what);
}

FocusLease lease(uint64_t client, uint64_t epoch, uint64_t token) {
  PipeTicket ticket;
  ticket.client = client;
  return FocusLease{ticket, epoch, token};
}

// The worker debounces AI requests harder than cloud ones - each is a paid model
// call - so a test that wants two separate fetches has to wait past that window.
// Kept here rather than read from the worker: a test that asked the worker for
// the number would pass whatever the worker happened to say.
constexpr auto beyond_debounce = std::chrono::milliseconds(850);

// A query the worker can derive a cache key from: the key is built from the
// provider identity, prompt settings, context digest and pinyin segments.
std::string query_for(const std::string &segments, int candidate_limit = 3,
                      const std::string &prompt = {},
                      const std::string &context = {}) {
  return R"({"ai_eligible":true,"ai_assistant":{"enabled":true,"provider":"openai",)"
         R"("endpoint":"https://example.invalid/v1","model":"gpt","candidate_limit":)" +
         std::to_string(candidate_limit) + R"(,"prompt_custom_1":")" + prompt +
         R"("},"ai_context":")" + context + R"(","pinyin_segments":[")" +
         segments + R"("]})";
}

// A fetcher that records what it was asked for, can be held inside the call -
// which is where supersession has to be observed - and can be told what to
// answer with.
struct Recorder {
  std::mutex mutex;
  std::condition_variable changed;
  std::vector<std::string> queries;
  std::vector<bool> cancelled_during;
  std::vector<std::string> answer{"设想", "思想"};
  bool hold = false;
  bool release = false;
  bool throwing = false;

  std::vector<std::string> operator()(const std::string &query,
                                      const std::function<bool()> &cancelled) {
    {
      std::unique_lock lock(mutex);
      queries.push_back(query);
      changed.notify_all();
      if (hold)
        changed.wait(lock, [&] { return release; });
    }
    if (throwing)
      throw std::runtime_error("provider blew up");
    // Read after the wait: a request superseded while it was in flight must be
    // able to see that, which is what stops it paying for the rest of itself.
    const bool seen = cancelled && cancelled();
    std::lock_guard lock(mutex);
    cancelled_during.push_back(seen);
    changed.notify_all();
    return answer;
  }
};

// The scaffolding every case wants: a worker wired to a recorder, with the
// results it delivered.
struct Harness {
  Recorder recorder;
  std::mutex mutex;
  std::condition_variable done;
  std::vector<AiCandidateWorker::Result> results;
  AiCandidateWorker worker;

  Harness()
      : worker(
            [this](AiCandidateWorker::Result result) {
              std::lock_guard lock(mutex);
              results.push_back(std::move(result));
              done.notify_all();
            },
            [this](const std::string &query,
                   const std::function<bool()> &cancelled) {
              return recorder(query, cancelled);
            }) {}

  bool wait_for_results(size_t count) {
    std::unique_lock lock(mutex);
    return done.wait_for(lock, std::chrono::seconds(5),
                         [&] { return results.size() >= count; });
  }

  size_t fetches() {
    std::lock_guard lock(recorder.mutex);
    return recorder.queries.size();
  }
};
} // namespace

int main() {
  try {
    // A completed request reaches the owner with its own lease, query and
    // candidates.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9), query_for("ni")),
              "a valid request is accepted");
      require(harness.wait_for_results(1), "the completion runs");
      require(harness.results.size() == 1, "one submission completes once");
      require(harness.results[0].query == query_for("ni"),
              "the query travels with its result");
      require(harness.results[0].candidates ==
                  std::vector<std::string>{"设想", "思想"},
              "the candidates are the ones fetched");
      require(harness.results[0].lease.epoch == 7 &&
                  harness.results[0].lease.token == 9,
              "the result carries the lease that asked for it");
      require(!harness.worker.failed(), "nothing went wrong");
    }

    // Two submissions inside the debounce window are one model call, for the
    // newer query: the older prefix is one the user has already left.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9), query_for("ni")),
              "the first request is accepted");
      require(harness.worker.submit(lease(42, 7, 9), query_for("niha")),
              "the second request is accepted");
      require(harness.wait_for_results(1), "the coalesced request completes");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.fetches() == 1, "only one model call is paid for");
      {
        std::lock_guard lock(harness.recorder.mutex);
        require(harness.recorder.queries[0] == query_for("niha"),
                "and it is the newer one");
      }
      require(harness.results.size() == 1 &&
                  harness.results[0].query == query_for("niha"),
              "the stale query never reaches the owner");
    }

    // The same prefix asked for twice is paid for once: a successful answer is
    // reusable across candidate generations, which is what the cache is for.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9), query_for("nihao")),
              "the first request is accepted");
      require(harness.wait_for_results(1), "the first request completes");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.worker.submit(lease(42, 7, 10), query_for("nihao")),
              "the same prefix is asked for again");
      require(harness.wait_for_results(2), "the repeat completes too");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.fetches() == 1, "the repeat is served from the cache");
      require(harness.results[1].candidates == harness.results[0].candidates,
              "and it delivers the same candidates");
      require(harness.results[1].lease.token == 10,
              "with the lease that asked the second time");
    }

    // An empty answer is not cached. Caching it would turn one bad minute at
    // the provider into a prefix that never gets AI candidates again.
    {
      Harness harness;
      harness.recorder.answer = {};
      require(harness.worker.submit(lease(42, 7, 9), query_for("nihao")),
              "the first request is accepted");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.fetches() == 1, "the first request reached the provider");
      require(harness.results.empty(), "an empty answer delivers nothing");
      {
        std::lock_guard lock(harness.recorder.mutex);
        harness.recorder.answer = {"你好"};
      }
      require(harness.worker.submit(lease(42, 7, 9), query_for("nihao")),
              "the same prefix is asked for again");
      require(harness.wait_for_results(1), "the retry completes");
      require(harness.fetches() == 2, "the retry was not suppressed by a cache");
      require(harness.results[0].candidates == std::vector<std::string>{"你好"},
              "and the answer that arrived is delivered");
    }

    // The candidate limit is part of the provider request. A cached answer
    // for one limit must not suppress a later request for a different number
    // of rows.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9), query_for("nihao", 1)),
              "the one-candidate request is accepted");
      require(harness.wait_for_results(1), "the limited request completes");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.worker.submit(lease(42, 7, 10), query_for("nihao", 2)),
              "the larger-candidate request is accepted");
      require(harness.wait_for_results(2), "the larger request completes");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.fetches() == 2,
              "a changed candidate limit bypasses the old cache entry");
    }

    // Prompt settings are part of the provider request. A cached answer from
    // one custom prompt must not be shown after the user changes that prompt.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9),
                                    query_for("nihao", 3, "first")),
              "the first prompt request is accepted");
      require(harness.wait_for_results(1), "the first prompt completes");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.worker.submit(lease(42, 7, 10),
                                    query_for("nihao", 3, "second")),
              "the changed prompt request is accepted");
      require(harness.wait_for_results(2), "the changed prompt completes");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.fetches() == 2,
              "a changed prompt bypasses the old cache entry");
    }

    // Recent committed text is sent to the model. The same pinyin after a
    // different context needs a fresh answer.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9),
                                    query_for("nihao", 3, {}, "first context")),
              "the first context request is accepted");
      require(harness.wait_for_results(1), "the first context completes");
      require(harness.worker.submit(lease(42, 7, 10),
                                    query_for("nihao", 3, {}, "second context")),
              "the changed context request is accepted");
      require(harness.wait_for_results(2), "the changed context completes");
      require(harness.fetches() == 2,
              "a changed context bypasses the old cache entry");
    }

    // A request superseded while it is in flight sees the cancellation, and its
    // candidates are dropped however it finishes.
    {
      Harness harness;
      harness.recorder.hold = true;
      require(harness.worker.submit(lease(42, 7, 9), query_for("ni")),
              "the held request is accepted");
      {
        std::unique_lock lock(harness.recorder.mutex);
        require(harness.recorder.changed.wait_for(
                    lock, std::chrono::seconds(5),
                    [&] { return !harness.recorder.queries.empty(); }),
                "the first request reaches the provider");
      }
      require(harness.worker.submit(lease(42, 7, 9), query_for("nihao")),
              "a newer request arrives mid-flight");
      {
        std::lock_guard lock(harness.recorder.mutex);
        harness.recorder.release = true;
      }
      harness.recorder.changed.notify_all();
      {
        std::unique_lock lock(harness.recorder.mutex);
        require(harness.recorder.changed.wait_for(
                    lock, std::chrono::seconds(5),
                    [&] { return !harness.recorder.cancelled_during.empty(); }),
                "the held request returns");
        require(harness.recorder.cancelled_during[0],
                "an in-flight request that has been superseded knows it");
      }
      {
        std::unique_lock lock(harness.mutex);
        require(harness.done.wait_for(lock, std::chrono::seconds(5),
                                      [&] {
                                        return !harness.results.empty() &&
                                               harness.results.back().query ==
                                                   query_for("nihao");
                                      }),
                "the newer request is the one delivered");
      }
      for (const auto &result : harness.results)
        require(result.query == query_for("nihao"),
                "the superseded candidates are never delivered");
    }

    // A provider that throws is recorded rather than allowed to take the worker
    // thread down: AI candidates are optional and must never stop input.
    {
      Harness harness;
      harness.recorder.throwing = true;
      require(harness.worker.submit(lease(42, 7, 9), query_for("ni")),
              "the failing request is accepted");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.worker.failed(), "the failure is recorded");
      require(harness.results.empty(), "and nothing is delivered");
      {
        std::lock_guard lock(harness.recorder.mutex);
        harness.recorder.throwing = false;
      }
      require(harness.worker.submit(lease(42, 7, 9), query_for("hao")),
              "the worker still takes requests");
      require(harness.wait_for_results(1), "and still answers them");
    }

    // Rejected envelopes never reach the provider: no lease, no query, or a
    // query past the bound this side is willing to send.
    {
      Harness harness;
      require(!harness.worker.submit(lease(42, 0, 9), query_for("ni")),
              "an epoch-less lease is refused");
      require(!harness.worker.submit(lease(42, 7, 0), query_for("ni")),
              "a token-less lease is refused");
      require(!harness.worker.submit(lease(42, 7, 9), ""),
              "an empty query is refused");
      require(!harness.worker.submit(lease(42, 7, 9), std::string(20000, 'a')),
              "an oversized query is refused");
      harness.worker.request_stop();
      require(!harness.worker.submit(lease(42, 7, 9), query_for("ni")),
              "a stopping worker accepts nothing");
      require(harness.fetches() == 0, "none of them reached the provider");
    }

    std::cout << "Windows AI candidate worker checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
