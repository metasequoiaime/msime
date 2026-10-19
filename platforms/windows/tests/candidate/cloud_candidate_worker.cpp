#include "CloudCandidateWorker.h"
#include "msime_client.h"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string>
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

// The worker coalesces submissions for this long before it fetches, so a test
// that wants two separate fetches has to wait past it. Kept here rather than
// exported: the number is the worker's business, and a test that reads it from
// the worker would pass whatever the worker happened to say.
constexpr auto beyond_debounce = std::chrono::milliseconds(700);

// A fetcher that records what it was asked for and can be held inside the call,
// which is where supersession has to be observed.
struct Recorder {
  std::mutex mutex;
  std::condition_variable changed;
  std::vector<std::string> queries;
  std::vector<bool> cancelled_during;
  bool hold = false;
  bool release = false;

  std::string operator()(const std::string &query,
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
    return "body:" + query;
  }
};
} // namespace

int main() {
  try {
    // A completed request reaches the owner with its own lease and query.
    {
      Recorder recorder;
      std::mutex mutex;
      std::condition_variable done;
      std::vector<CloudCandidateWorker::Result> results;
      CloudCandidateWorker worker(
          [&](CloudCandidateWorker::Result result) {
            std::lock_guard lock(mutex);
            results.push_back(std::move(result));
            done.notify_all();
          },
          [&](const std::string &query, const std::function<bool()> &cancelled) {
            return recorder(query, cancelled);
          });
      require(worker.submit(lease(42, 7, 9), "ni"), "a valid request is accepted");
      std::unique_lock lock(mutex);
      require(done.wait_for(lock, std::chrono::seconds(5), [&] { return !results.empty(); }),
              "the completion runs");
      require(results.size() == 1, "one submission completes once");
      require(results[0].query == "ni", "the query travels with its result");
      require(results[0].body == "body:ni", "the body is the one fetched");
      require(results[0].lease.epoch == 7 && results[0].lease.token == 9,
              "the result carries the lease that asked for it");
    }

    // Two submissions inside the debounce window are one fetch, for the newer
    // query: the older one was never worth paying for.
    {
      Recorder recorder;
      std::mutex mutex;
      std::condition_variable done;
      std::vector<CloudCandidateWorker::Result> results;
      CloudCandidateWorker worker(
          [&](CloudCandidateWorker::Result result) {
            std::lock_guard lock(mutex);
            results.push_back(std::move(result));
            done.notify_all();
          },
          [&](const std::string &query, const std::function<bool()> &cancelled) {
            return recorder(query, cancelled);
          });
      require(worker.submit(lease(42, 7, 9), "ni"), "the first request is accepted");
      require(worker.submit(lease(42, 7, 9), "niha"), "the second request is accepted");
      std::unique_lock lock(mutex);
      require(done.wait_for(lock, std::chrono::seconds(5), [&] { return !results.empty(); }),
              "the coalesced request completes");
      lock.unlock();
      std::this_thread::sleep_for(beyond_debounce);
      std::lock_guard recorded(recorder.mutex);
      require(recorder.queries.size() == 1, "only one request is paid for");
      require(recorder.queries[0] == "niha", "and it is the newer one");
      require(results.size() == 1 && results[0].query == "niha",
              "the stale query never reaches the owner");
    }

    // A request superseded while it is in flight sees the cancellation and its
    // result is dropped, however it finishes.
    {
      Recorder recorder;
      recorder.hold = true;
      std::mutex mutex;
      std::condition_variable done;
      std::vector<CloudCandidateWorker::Result> results;
      CloudCandidateWorker worker(
          [&](CloudCandidateWorker::Result result) {
            std::lock_guard lock(mutex);
            results.push_back(std::move(result));
            done.notify_all();
          },
          [&](const std::string &query, const std::function<bool()> &cancelled) {
            return recorder(query, cancelled);
          });
      require(worker.submit(lease(42, 7, 9), "ni"), "the held request is accepted");
      {
        std::unique_lock lock(recorder.mutex);
        require(recorder.changed.wait_for(lock, std::chrono::seconds(5),
                                          [&] { return !recorder.queries.empty(); }),
                "the first request reaches the network");
      }
      // Supersede it while it is still inside the fetch.
      require(worker.submit(lease(42, 7, 9), "nihao"), "a newer request arrives mid-flight");
      {
        std::lock_guard lock(recorder.mutex);
        recorder.release = true;
      }
      recorder.changed.notify_all();
      {
        std::unique_lock lock(recorder.mutex);
        require(recorder.changed.wait_for(lock, std::chrono::seconds(5),
                                          [&] { return !recorder.cancelled_during.empty(); }),
                "the held request returns");
        require(recorder.cancelled_during[0],
                "an in-flight request that has been superseded knows it");
      }
      std::unique_lock lock(mutex);
      require(done.wait_for(lock, std::chrono::seconds(5),
                            [&] {
                              return !results.empty() && results.back().query == "nihao";
                            }),
              "the newer request is the one delivered");
      for (const auto &result : results)
        require(result.query == "nihao", "the superseded body is never delivered");
    }

    // Rejected envelopes never reach the network: no lease, no query, or a
    // query past the bound this side is willing to send.
    {
      Recorder recorder;
      CloudCandidateWorker worker(
          [](CloudCandidateWorker::Result) {},
          [&](const std::string &query, const std::function<bool()> &cancelled) {
            return recorder(query, cancelled);
          });
      require(!worker.submit(lease(42, 0, 9), "ni"), "an epoch-less lease is refused");
      require(!worker.submit(lease(42, 7, 0), "ni"), "a token-less lease is refused");
      require(!worker.submit(lease(42, 7, 9), ""), "an empty query is refused");
      require(!worker.submit(lease(42, 7, 9), std::string(MSIME_ONLINE_QUERY_MAX_BYTES + 1, 'a')),
              "an oversized query is refused");
      worker.request_stop();
      require(!worker.submit(lease(42, 7, 9), "ni"), "a stopping worker accepts nothing");
      std::lock_guard lock(recorder.mutex);
      require(recorder.queries.empty(), "none of them reached the network");
    }

    std::cout << "Windows cloud candidate worker checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
