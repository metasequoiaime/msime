#include "AudioCapture.h"

#include <cassert>
#include <cstddef>
#include <type_traits>

// The lifetime rules VoiceInputSession relies on, checked without a microphone: stop is idempotent on a capture that never started, and a request that cannot be honoured (no callback, a malformed or foreign device id) fails before any device is opened.
int main() {
  using msime::windows::AudioCapture;
  static_assert(!std::is_copy_constructible_v<AudioCapture>);
  AudioCapture first, second;
  first.stop();
  first.stop();
  second.stop();
  assert(first.last_failure() == msime::windows::AudioCaptureFailure::None);
  assert(!first.start({}, {}));
  assert(first.last_failure() == msime::windows::AudioCaptureFailure::Failed);
  // 格式不对或别的平台写下的设备 id 是「所选麦克风不可用」，不是通用的启动失败，提示才能让人去重选设备。
  for (const auto *invalid : {"0", "wasapi:", "wasapi:001", "wasapi:gggg", "coreaudio:4275696c74496e"}) {
    assert(!first.start([](const float *, std::size_t) {}, invalid));
    assert(first.last_failure() == msime::windows::AudioCaptureFailure::DeviceUnavailable);
  }
  assert(!first.callback_failed() && !second.callback_failed());
  return 0;
}
