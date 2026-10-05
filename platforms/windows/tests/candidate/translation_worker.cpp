#include "TranslationWorker.h"
#include "CandidateHttpPolicy.h"

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

// The worker coalesces submissions for this long before it translates, so a
// test that wants two separate calls has to wait past it. Kept here rather than
// read from the worker: a test that asked the worker for the number would pass
// whatever the worker happened to say.
constexpr auto beyond_debounce = std::chrono::milliseconds(700);

// A translator that records what it was asked for and can be held inside the
// call, which is where supersession and cache invalidation have to be observed.
struct Recorder {
  std::mutex mutex;
  std::condition_variable changed;
  std::vector<std::string> queries;
  std::vector<bool> cancelled_during;
  bool hold = false;
  bool release = false;
  bool answer = true;

  std::optional<TranslationWorker::Result>
  operator()(const FocusLease &owner, const std::string &query,
             const std::function<bool()> &cancelled) {
    {
      std::unique_lock lock(mutex);
      queries.push_back(query);
      changed.notify_all();
      if (hold)
        changed.wait(lock, [&] { return release; });
    }
    // Read after the wait: a request superseded while it was in flight must be
    // able to see that, which is what stops it paying for the rest of itself.
    const bool seen = cancelled && cancelled();
    {
      std::lock_guard lock(mutex);
      cancelled_during.push_back(seen);
      changed.notify_all();
    }
    if (!answer)
      return std::nullopt;
    return TranslationWorker::Result{owner, 11, R"([{"text":")" + query +
                                                    R"(","translation":"gloss"}])"};
  }
};

struct Harness {
  Recorder recorder;
  std::mutex mutex;
  std::condition_variable done;
  std::vector<TranslationWorker::Result> results;
  TranslationWorker worker;

  Harness()
      : worker(
            [this](TranslationWorker::Result result) {
              std::lock_guard lock(mutex);
              results.push_back(std::move(result));
              done.notify_all();
            },
            [this](const FocusLease &owner, const std::string &query,
                   const std::function<bool()> &cancelled) {
              return recorder(owner, query, cancelled);
            }) {}

  bool wait_for_results(size_t count) {
    std::unique_lock lock(mutex);
    return done.wait_for(lock, std::chrono::seconds(5),
                         [&] { return results.size() >= count; });
  }

  bool wait_for_fetch(size_t count) {
    std::unique_lock lock(recorder.mutex);
    return recorder.changed.wait_for(
        lock, std::chrono::seconds(5),
        [&] { return recorder.queries.size() >= count; });
  }

  size_t translations() {
    std::lock_guard lock(recorder.mutex);
    return recorder.queries.size();
  }
};
} // namespace

int main() {
  try {
    require(valid_candidate_url("https://remote.example/api"),
            "remote HTTPS is accepted");
    require(valid_candidate_url("http://127.0.0.1:8080/api", true),
            "loopback HTTP is accepted");
    require(valid_candidate_url("http://[::1]:8080/api", true),
            "IPv6 loopback HTTP is accepted");
    require(!valid_candidate_url("http://remote.example/api", true),
            "remote HTTP is refused");
    require(!valid_candidate_url("http://localhost.example/api", true),
            "lookalike loopback host is refused");
    require(valid_candidate_header_name("Authorization"),
            "a token header name is accepted");
    require(valid_candidate_header_value("Bearer synthetic"),
            "a visible header value is accepted");
    require(!valid_candidate_header_name("X-Bad\r\nInjected"),
            "header names cannot contain line breaks");
    require(!valid_candidate_header_value("secret\r\nX-Injected: yes"),
            "header values cannot contain line breaks");
    require(!valid_candidate_header_value("secret\x7f"),
            "header values cannot contain DEL");
    // A completed request reaches the owner with its own lease, generation and
    // rows.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9), "ni"),
              "a valid request is accepted");
      require(harness.wait_for_results(1), "the completion runs");
      require(harness.results.size() == 1, "one submission completes once");
      require(harness.results[0].generation == 11,
              "the generation travels with its result");
      require(harness.results[0].translations.find("\"ni\"") != std::string::npos,
              "the rows are the ones translated");
      require(harness.results[0].lease.epoch == 7 &&
                  harness.results[0].lease.token == 9,
              "the result carries the lease that asked for it");
    }

    // Two submissions inside the debounce window are one translation, for the
    // newer query: the older one describes candidates that are already gone.
    {
      Harness harness;
      require(harness.worker.submit(lease(42, 7, 9), "ni"),
              "the first request is accepted");
      require(harness.worker.submit(lease(42, 7, 9), "niha"),
              "the second request is accepted");
      require(harness.wait_for_results(1), "the coalesced request completes");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.translations() == 1, "only one translation is paid for");
      {
        std::lock_guard lock(harness.recorder.mutex);
        require(harness.recorder.queries[0] == "niha", "and it is the newer one");
      }
      require(harness.results.size() == 1 &&
                  harness.results[0].translations.find("\"niha\"") !=
                      std::string::npos,
              "the stale query never reaches the owner");
    }

    // A request superseded while it is in flight sees the cancellation, and its
    // rows are dropped however it finishes.
    {
      Harness harness;
      harness.recorder.hold = true;
      require(harness.worker.submit(lease(42, 7, 9), "ni"),
              "the held request is accepted");
      require(harness.wait_for_fetch(1), "the first request starts translating");
      require(harness.worker.submit(lease(42, 7, 9), "nihao"),
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
        require(harness.done.wait_for(
                    lock, std::chrono::seconds(5),
                    [&] {
                      return !harness.results.empty() &&
                             harness.results.back().translations.find(
                                 "\"nihao\"") != std::string::npos;
                    }),
                "the newer request is the one delivered");
      }
      for (const auto &result : harness.results)
        require(result.translations.find("\"nihao\"") != std::string::npos,
                "the superseded rows are never delivered");
    }

    // Preferences that make the provider inapplicable clear the cache, and a
    // response already in flight must not survive that: it was produced under
    // the settings the user has just left, and delivering it would show a
    // translation from a provider that is no longer allowed to answer.
    {
      Harness harness;
      harness.recorder.hold = true;
      require(harness.worker.submit(lease(42, 7, 9), "ni"),
              "the held request is accepted");
      require(harness.wait_for_fetch(1), "it starts translating");
      harness.worker.clear_cache();
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
                "the invalidated request knows it was cancelled");
      }
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.results.empty(), "and nothing from it is delivered");
      // The worker is still usable afterwards.
      {
        std::lock_guard lock(harness.recorder.mutex);
        harness.recorder.hold = false;
        harness.recorder.release = false;
      }
      require(harness.worker.submit(lease(42, 7, 9), "hao"),
              "the worker still takes requests");
      require(harness.wait_for_results(1), "and still answers them");
    }

    // A query that is not translatable delivers nothing rather than an empty
    // row: the candidate window would otherwise clear the glosses it has.
    {
      Harness harness;
      harness.recorder.answer = false;
      require(harness.worker.submit(lease(42, 7, 9), "ni"),
              "the request is accepted");
      require(harness.wait_for_fetch(1), "it reaches the translator");
      std::this_thread::sleep_for(beyond_debounce);
      require(harness.results.empty(), "nothing is delivered");
    }

    // Rejected envelopes never reach the translator: no lease, no query, or a
    // query past the bound this side is willing to carry.
    {
      Harness harness;
      require(!harness.worker.submit(lease(42, 0, 9), "ni"),
              "an epoch-less lease is refused");
      require(!harness.worker.submit(lease(42, 7, 0), "ni"),
              "a token-less lease is refused");
      require(!harness.worker.submit(lease(42, 7, 9), ""),
              "an empty query is refused");
      require(!harness.worker.submit(lease(42, 7, 9), std::string(70000, 'a')),
              "an oversized query is refused");
      harness.worker.request_stop();
      require(!harness.worker.submit(lease(42, 7, 9), "ni"),
              "a stopping worker accepts nothing");
      require(harness.translations() == 0, "none of them reached the translator");
    }

    std::cout << "Windows translation worker checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
