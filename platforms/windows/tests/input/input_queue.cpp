#include "InputQueue.h"
#include <vector>

using namespace msime::windows;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Input queue test failed");
}
// Test-only blocking task, always released before queue destruction on failure.
struct Release {
  std::promise<void> promise;
  std::shared_future<void> future = promise.get_future().share();
  bool done = false;
  void release() {
    if (!done) {
      done = true;
      promise.set_value();
    }
  }
  ~Release() { release(); }
};
int main() {
  FocusGate gate;
  {
    bool rejected = false;
    try {
      InputQueue invalid(gate, 0, 2, "{}");
    } catch (const std::runtime_error &) {
      rejected = true;
    }
    require(rejected);
  }
  {
    // 维护期间连入只登记传输，即使没有可创建引擎会话的配置也应成功。
    InputQueue queue(gate, 1, 1, "{}");
    auto maintenance = queue.submit([](InputState &state) {
      require(state.quiesce_dictionaries() == 0);
      PipeTicket ticket{314, {41, 42, 43}};
      require(state.connected(ticket).accepted);
      require(state.quiesce_dictionaries() == 0);
      ticket.generations = {44, 45, 46};
      require(state.connected(ticket).accepted);
      require(state.quiesce_dictionaries() == 0);
      require(state.disconnected(ticket).accepted);
    });
    require(maintenance && maintenance->get() == InputTaskStatus::Completed);
    queue.stop();
  }
  {
    InputQueue queue(gate, 1, 2, "{}");
    std::vector<int> order;
    const auto owner = std::this_thread::get_id();
    std::thread::id worker;
    auto first = queue.submit([&](InputState &) {
      worker = std::this_thread::get_id();
      require(worker != owner);
      order.push_back(1);
    });
    auto second = queue.submit([&](InputState &) {
      require(worker == std::this_thread::get_id());
      order.push_back(2);
    });
    require(first && second && first->get() == InputTaskStatus::Completed &&
            second->get() == InputTaskStatus::Completed);
    queue.stop();
    queue.stop();
    require(order == std::vector<int>({1, 2}));
    require(queue.stats().completed == 2 && !queue.stats().accepting);
    require(!queue.submit([](InputState &) {}));
  }
  {
    InputQueue queue(gate, 1, 1, "{}");
    Release release;
    std::promise<void> entered;
    auto active = queue.submit([&](InputState &) {
      entered.set_value();
      release.future.wait();
    });
    entered.get_future().wait();
    auto cancelled = queue.submit([](InputState &) {
      throw std::runtime_error("Cancelled task executed");
    });
    require(active && cancelled);
    require(!queue.submit([](InputState &) {}));
    require(!queue.submit({}));
    require(queue.stats().queued == 1 && queue.stats().active);
    queue.request_stop();
    require(!queue.submit([](InputState &) {}));
    release.release();
    queue.stop();
    require(active->get() == InputTaskStatus::Completed &&
            cancelled->get() == InputTaskStatus::Cancelled);
    require(queue.stats().cancelled == 1 && queue.stats().failed == 0);
  }
  {
    InputQueue queue(gate, 1, 2, "{}");
    Release release;
    std::promise<void> entered;
    auto failed = queue.submit([&](InputState &) {
      entered.set_value();
      release.future.wait();
      throw std::runtime_error("Synthetic task failure");
    });
    entered.get_future().wait();
    auto cancelled = queue.submit([](InputState &) {});
    require(failed && cancelled);
    release.release();
    require(failed->get() == InputTaskStatus::Failed);
    queue.stop();
    require(cancelled->get() == InputTaskStatus::Cancelled &&
            queue.stats().failed == 1 && queue.stats().cancelled == 1);
  }
  {
    InputQueue queue(gate, 1, 1, "{}");
    auto stopped = queue.submit([&](InputState &) { queue.request_stop(); });
    require(stopped && stopped->get() == InputTaskStatus::Completed);
    queue.stop();
  }
  {
    InputQueue queue(gate, 1, 1, "{}");
    auto invalid_join = queue.submit([&](InputState &) { queue.stop(); });
    require(invalid_join && invalid_join->get() == InputTaskStatus::Failed);
    queue.stop();
  }
  {
    InputQueue queue(gate, 1, 256, "{}");
    std::vector<std::pair<int, int>> order;
    std::vector<std::future<void>> producers;
    for (int producer = 0; producer < 4; ++producer)
      producers.push_back(std::async(std::launch::async, [&, producer] {
        std::vector<std::future<InputTaskStatus>> receipts;
        for (int i = 0; i < 50; ++i) {
          auto result = queue.submit([&, producer, i](InputState &) {
            order.emplace_back(producer, i);
          });
          require(result.has_value());
          receipts.push_back(std::move(*result));
        }
        for (auto &receipt : receipts)
          require(receipt.get() == InputTaskStatus::Completed);
      }));
    for (auto &producer : producers)
      producer.get();
    auto first_stop = std::async(std::launch::async, [&] { queue.stop(); });
    queue.stop();
    first_stop.get();
    int expected[4] = {};
    for (auto [producer, sequence] : order)
      require(sequence == expected[producer]++);
    require(order.size() == 200 && queue.stats().completed == 200);
  }
}
