#include "LocalAsrAudioQueue.h"

#include <iostream>
#include <stdexcept>
#include <string>

namespace {
void require(bool condition, int line) {
  if (!condition)
    throw std::runtime_error("local ASR audio queue failed at line " +
                             std::to_string(line));
}
#define REQUIRE(condition) require((condition), __LINE__)
} // namespace

int main() {
  try {
    msime::windows::LocalAsrAudioQueue queue(4);
    const float samples[] = {0.1f, 0.2f, 0.3f, 0.4f};
    REQUIRE(queue.push(samples, 4) ==
            msime::windows::LocalAsrAudioQueue::PushResult::accepted);
    REQUIRE(queue.size() == 4);
    REQUIRE(queue.push(samples, 1) ==
            msime::windows::LocalAsrAudioQueue::PushResult::overflowed);
    REQUIRE(queue.size() == 4);
    REQUIRE(queue.overflowed());
    REQUIRE(queue.closed());

    bool ended = false;
    const auto batch = queue.wait_and_take(ended);
    REQUIRE(ended);
    REQUIRE(batch.size() == 4);
    REQUIRE(batch[0] == samples[0] && batch[3] == samples[3]);

    msime::windows::LocalAsrAudioQueue cancelled(4);
    REQUIRE(cancelled.push(samples, 2) ==
            msime::windows::LocalAsrAudioQueue::PushResult::accepted);
    cancelled.cancel();
    REQUIRE(cancelled.cancelled());
    REQUIRE(cancelled.closed());
    REQUIRE(cancelled.wait_and_take(ended).empty());

    // The bounded queue reserves its full budget before the first capture
    // callback, so a short first batch does not leave later inserts growing it.
    msime::windows::LocalAsrAudioQueue reserved(4);
    REQUIRE(reserved.push(samples, 1) ==
            msime::windows::LocalAsrAudioQueue::PushResult::accepted);
    const auto short_batch = reserved.wait_and_take(ended);
    REQUIRE(short_batch.capacity() >= 4);
    REQUIRE(reserved.push(samples, 1) ==
            msime::windows::LocalAsrAudioQueue::PushResult::accepted);
    const auto second_batch = reserved.wait_and_take(ended);
    REQUIRE(second_batch.capacity() >= 4);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
