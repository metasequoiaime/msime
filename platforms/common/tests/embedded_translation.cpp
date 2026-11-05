#include "../EmbeddedTranslation.h"
#include <cassert>
#include <cstdlib>
#include <cstring>
#include <thread>

namespace {
std::atomic<bool> registered{false};
std::atomic<bool> cancellation{false};
std::atomic<int> calls{0};
std::atomic<int> cancellations{0};
}
extern "C" char *msime_client_local_translation_query(const uint8_t *, size_t) {
  ++calls;
  // 模拟取消发生在 Rust 工作项登记之前；宿主必须持续发送取消，不能只发一次。
  std::this_thread::sleep_for(std::chrono::milliseconds(80));
  registered = true;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(2);
  while (!cancellation && std::chrono::steady_clock::now() < deadline)
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
  assert(cancellation);
  auto *reply = static_cast<char *>(std::malloc(3));
  std::memcpy(reply, "{}", 3);
  return reply;
}
extern "C" void msime_client_local_translation_cancel(const uint8_t *, size_t) {
  ++cancellations;
  if (registered) cancellation = true;
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
  while (calls == 0) std::this_thread::yield();
  state.observe("third");
  assert(!result.get());
  assert(cancellation && cancellations > 1);
  assert(next_request_id() != next_request_id());
}
