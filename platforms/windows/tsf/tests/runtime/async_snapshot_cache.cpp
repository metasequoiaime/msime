#include "../../AsyncSnapshotCache.h"
#include <cassert>
#include <condition_variable>
#include <future>
#include <stdexcept>
#include <thread>

namespace {
void invalidationDuringReadIsRetried() {
  using Cache = msime::tsf::AsyncSnapshotCache<int>;
  Cache cache;
  cache.initialize([](const auto &) { return std::make_shared<const int>(1); });
  std::promise<void> entered, resume;
  auto resumed = resume.get_future().share();
  std::atomic<int> backing{2};
  int reads = 0;
  std::thread worker;
  auto load = [&](const auto &) {
    const int value = backing.load();
    if (++reads == 1) {
      entered.set_value();
      resumed.wait();
    }
    return std::make_shared<const int>(value);
  };
  const auto start = Cache::Clock::now();
  auto schedule = [&](auto task) { worker = std::thread(std::move(task)); return true; };
  assert(cache.refresh(load, schedule, start));
  assert(entered.get_future().wait_for(std::chrono::seconds(5)) == std::future_status::ready);
  backing.store(3);
  cache.invalidate();
  assert(!cache.refresh(load, schedule, start));
  resume.set_value();
  worker.join();
  assert(reads == 2 && *cache.snapshot() == 3);
}

void invalidationBypassesThrottleAndSurvivesRejection() {
  using Cache = msime::tsf::AsyncSnapshotCache<int>;
  Cache cache;
  cache.initialize([](const auto &) { return std::make_shared<const int>(1); });
  auto immediate = [](auto task) { task(); return true; };
  int value = 2;
  auto load = [&](const auto &) { return std::make_shared<const int>(value); };
  const auto start = Cache::Clock::now();
  assert(cache.refresh(load, immediate, start));
  cache.invalidate();
  assert(!cache.refresh(load, [](auto) { return false; }, start));
  value = 3;
  assert(cache.refresh(load, immediate, start));
  assert(*cache.snapshot() == 3);
  assert(!cache.refresh(load, immediate, start));
  cache.invalidate();
  assert(!cache.refresh(load, [](auto) -> bool { throw std::bad_alloc(); }, start));
  value = 4;
  assert(cache.refresh(load, immediate, start));
  assert(*cache.snapshot() == 4);
}
}

int main() {
  invalidationDuringReadIsRetried();
  invalidationBypassesThrottleAndSurvivesRejection();
  using Cache = msime::tsf::AsyncSnapshotCache<int>;
  Cache cache;
  int loads = 0;
  cache.initialize([&](const auto &) { ++loads; return std::make_shared<const int>(41); });
  cache.initialize([&](const auto &) { ++loads; return std::make_shared<const int>(0); });
  assert(loads == 1 && *cache.snapshot() == 41);
  const auto retained = cache.snapshot();
  const auto start = Cache::Clock::now();
  std::promise<void> entered, resume;
  auto resumed = resume.get_future().share();
  std::thread worker;
  int scheduled = 0;
  auto schedule = [&](auto task) {
    ++scheduled;
    worker = std::thread(std::move(task));
    return true;
  };
  assert(cache.refresh([&](const auto &previous) {
    assert(*previous == 41);
    entered.set_value();
    resumed.wait();
    return std::make_shared<const int>(42);
  }, schedule, start));
  assert(entered.get_future().wait_for(std::chrono::seconds(5)) == std::future_status::ready);
  // 读盘明确停住时，读取者和重复刷新请求仍能返回；不依赖毫秒级耗时断言。
  auto reader = std::async(std::launch::async, [&] {
    for (int i = 0; i < 10000; ++i) {
      assert(cache.snapshot() == retained);
      assert(!cache.refresh([](const auto &) { return std::make_shared<const int>(0); },
                            schedule, start + std::chrono::seconds(1)));
    }
  });
  assert(reader.wait_for(std::chrono::seconds(5)) == std::future_status::ready);
  reader.get();
  assert(scheduled == 1);
  resume.set_value();
  worker.join();
  assert(*cache.snapshot() == 42 && *retained == 41);

  auto immediate = [](auto task) { task(); return true; };
  auto fail = [](const auto &) -> Cache::Snapshot { throw std::runtime_error("synthetic read failure"); };
  assert(!cache.refresh(fail, immediate, start + std::chrono::milliseconds(249)));
  assert(cache.refresh(fail, immediate, start + std::chrono::milliseconds(250)));
  assert(*cache.snapshot() == 42);
  assert(cache.refresh([](const auto &) -> Cache::Snapshot { return {}; }, immediate,
                       start + std::chrono::milliseconds(500)));
  assert(*cache.snapshot() == 42);
  auto load = [](const auto &) { return std::make_shared<const int>(43); };
  assert(!cache.refresh(load, [](auto) { return false; }, start + std::chrono::milliseconds(750)));
  assert(!cache.refresh(load, [](auto) -> bool { throw std::bad_alloc(); },
                        start + std::chrono::milliseconds(1000)));
  assert(cache.refresh(load, immediate, start + std::chrono::milliseconds(1250)));
  assert(*cache.snapshot() == 43);

  Cache initiallyFailed;
  initiallyFailed.initialize(fail);
  assert(!initiallyFailed.snapshot());
  assert(initiallyFailed.refresh(load, immediate, start));
  assert(*initiallyFailed.snapshot() == 43);
}
