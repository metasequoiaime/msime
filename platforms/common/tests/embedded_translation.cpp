#include "../EmbeddedTranslation.h"
#include <cassert>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <mutex>

namespace {
std::mutex mutex;
std::condition_variable changed;
bool registered = false;
bool early_cancel = false;
std::atomic<bool> cancellation{false};
std::atomic<int> calls{0};
std::atomic<int> cancellations{0};
}
extern "C" char *msime_client_local_translation_query(const uint8_t *, size_t) {
  std::unique_lock<std::mutex> lock(mutex);
  ++calls;
  changed.notify_all();
  // 确认收到一次无效的早取消后才登记，不能用固定休眠猜测 CI 线程调度。
  assert(changed.wait_for(lock, std::chrono::seconds(5), [] { return early_cancel; }));
  registered = true;
  assert(changed.wait_for(lock, std::chrono::seconds(5), [] { return cancellation.load(); }));
  auto *reply = static_cast<char *>(std::malloc(3));
  std::memcpy(reply, "{}", 3);
  return reply;
}
extern "C" void msime_client_local_translation_cancel(const uint8_t *, size_t) {
  std::lock_guard<std::mutex> lock(mutex);
  ++cancellations;
  if (registered) cancellation = true;
  else early_cancel = true;
  changed.notify_all();
}
extern "C" void msime_client_string_free(char *text) { std::free(text); }

int main() {
  using namespace msime::translation;
  EmbeddedState state;
  auto first = state.begin("first");
  state.observe("first");
  assert(!first->load());
  auto second = state.begin("second");
  assert(first->load());
  assert(!second->load());
  assert(!embedded("{}", "already-cancelled", [first] { return first->load(); }));
  assert(calls == 0);
  auto result = std::async(std::launch::async, [second] {
    return embedded("{}", "cancel-during-registration", [second] { return second->load(); });
  });
  {
    std::unique_lock<std::mutex> lock(mutex);
    assert(changed.wait_for(lock, std::chrono::seconds(5), [] { return calls > 0; }));
  }
  state.observe("third");
  assert(!result.get());
  assert(cancellation && cancellations > 1);
  assert(next_request_id() != next_request_id());
}
