#include "system/TsfConfigDeliveryTracker.h"

#include <stdexcept>
#include <string>
#include <vector>

using namespace msime::windows;

namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}

PipeTicket ticket(uint64_t client, uint64_t generation) {
  PipeTicket value;
  value.client = client;
  value.generations[0] = generation;
  value.generations[1] = generation + 1;
  value.generations[2] = generation + 2;
  value.capabilities = 7;
  return value;
}
} // namespace

int main() {
  try {
    TsfConfigDeliveryTracker tracker;
    const auto first = ticket(1, 10);
    const auto replacement = ticket(1, 20);
    const auto second = ticket(2, 30);

    require(tracker.needs_send(4, {}), "首次配置必须发送");
    tracker.mark_sent(4, {first});
    require(!tracker.needs_send(4, {first}), "同一管道和修订不应重复发送");
    require(tracker.needs_send(4, {replacement}), "重连后的新代次必须重发");
    tracker.mark_sent(4, {replacement});
    require(tracker.needs_send(5, {replacement}), "配置修订变化必须重发");
    tracker.mark_sent(5, {replacement});
    require(tracker.needs_send(5, {replacement, second}), "新宿主必须重发");
    tracker.mark_sent(5, {replacement, second});
    require(tracker.needs_send(5, {second}), "旧宿主退出后应更新已发送集合");
    tracker.mark_sent(5, {second});
    require(!tracker.needs_send(5, {second}), "发送后不应重复发送");
    return 0;
  } catch (const std::exception &error) {
    return (void)error.what(), 1;
  }
}
