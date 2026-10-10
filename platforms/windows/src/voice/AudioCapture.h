#pragma once
#include <cstddef>
#include <functional>
#include <memory>
#include <string>

namespace msime::windows {
// start() 失败的原因，供 VoiceInputSession 选择提示文案。AccessDenied 是 Windows 隐私设置拒绝了麦克风访问；DeviceUnavailable 是设置里选定的设备找不到或无法识别。
enum class AudioCaptureFailure { None, Failed, AccessDenied, DeviceUnavailable };

// Streaming microphone capture: mono 32-bit float at 16 kHz, delivered buffer by buffer on the device thread while a recording lasts. The bounded msime_client_voice_capture call cannot serve a push-to-talk recording of unknown length that feeds a live recognizer, so the Server keeps its own capture over miniaudio (third_party/miniaudio), the library the cue sounds already use.
class AudioCapture {
public:
  using AudioCallback = std::function<void(const float *, std::size_t)>;
  AudioCapture();
  ~AudioCapture();
  AudioCapture(const AudioCapture &) = delete;
  AudioCapture &operator=(const AudioCapture &) = delete;
  // The caller serializes start, stop and destruction. The callback runs on the capture thread and must never call stop or destroy this object. An empty device id records from the default device; a nonempty one must be a "wasapi:" id from the settings device list, and a missing or ambiguous device fails without falling back to the default.
  bool start(AudioCallback callback, const std::string &device_id);
  // Idempotent; waits for the device callback to return before releasing it.
  void stop();
  // A callback that threw stops receiving audio, so the recording it was part of is incomplete.
  bool callback_failed() const;
  // 最近一次 start() 失败的原因；成功后为 None。与 start 一样由调用方串行访问。
  AudioCaptureFailure last_failure() const;

private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};
} // namespace msime::windows
