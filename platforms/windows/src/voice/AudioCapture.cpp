// The one translation unit that compiles miniaudio's implementation; CuePlayer.cpp uses the same library through its declarations.
#define MINIAUDIO_IMPLEMENTATION
#include "miniaudio.h"

#include "AudioCapture.h"
#include "CaptureDeviceId.h"

#include <atomic>
#include <utility>

namespace msime::windows {
struct AudioCapture::Impl {
  ma_device device{};
  ma_context context{};
  ma_device_id selected{};
  bool context_initialized = false;
  bool initialized = false;
  AudioCallback callback;
  std::atomic_bool failed{false};
  AudioCaptureFailure failure = AudioCaptureFailure::None;

  static void receive(ma_device *device, void *, const void *input, ma_uint32 frames) noexcept {
    auto *self = static_cast<Impl *>(device->pUserData);
    if (!input || self->failed.load())
      return;
    try {
      if (self->callback)
        self->callback(static_cast<const float *>(input), frames);
    } catch (...) {
      self->failed = true;
    }
  }
};

AudioCapture::AudioCapture() : impl_(std::make_unique<Impl>()) {}

AudioCapture::~AudioCapture() { stop(); }

bool AudioCapture::start(AudioCallback callback, const std::string &device_id) {
  stop();
  impl_->failure = AudioCaptureFailure::Failed;
  // Windows 隐私设置关掉麦克风时，WASAPI 的 Activate 或 Initialize 返回 E_ACCESSDENIED，miniaudio 把它译成 MA_ACCESS_DENIED。
  const auto fail = [this](ma_result result) {
    stop();
    impl_->failure = result == MA_ACCESS_DENIED ? AudioCaptureFailure::AccessDenied
                                                : AudioCaptureFailure::Failed;
    return false;
  };
  if (!callback)
    return false;
  if (!device_id.empty()) {
    if (!is_wasapi_capture_device_id(device_id)) {
      impl_->failure = AudioCaptureFailure::DeviceUnavailable;
      return false;
    }
    const ma_backend backend = ma_backend_wasapi;
    if (const auto result = ma_context_init(&backend, 1, nullptr, &impl_->context);
        result != MA_SUCCESS)
      return fail(result);
    impl_->context_initialized = true;
    ma_device_info *capture = nullptr;
    ma_uint32 count = 0;
    if (const auto result =
            ma_context_get_devices(&impl_->context, nullptr, nullptr, &capture, &count);
        result != MA_SUCCESS)
      return fail(result);
    const auto *selected = select_capture_device(capture, count, device_id);
    if (!selected) {
      stop();
      impl_->failure = AudioCaptureFailure::DeviceUnavailable;
      return false;
    }
    impl_->selected = selected->id;
  }
  impl_->callback = std::move(callback);
  impl_->failed = false;
  auto config = ma_device_config_init(ma_device_type_capture);
  config.capture.format = ma_format_f32;
  config.capture.channels = 1;
  config.capture.pDeviceID = impl_->context_initialized ? &impl_->selected : nullptr;
  config.sampleRate = 16000;
  config.dataCallback = Impl::receive;
  config.pUserData = impl_.get();
  if (const auto result = ma_device_init(
          impl_->context_initialized ? &impl_->context : nullptr, &config, &impl_->device);
      result != MA_SUCCESS)
    return fail(result);
  impl_->initialized = true;
  if (const auto result = ma_device_start(&impl_->device); result != MA_SUCCESS)
    return fail(result);
  impl_->failure = AudioCaptureFailure::None;
  return true;
}

void AudioCapture::stop() {
  if (impl_->initialized) {
    ma_device_uninit(&impl_->device);
    impl_->initialized = false;
  }
  impl_->callback = {};
  if (impl_->context_initialized) {
    ma_context_uninit(&impl_->context);
    impl_->context_initialized = false;
  }
}

bool AudioCapture::callback_failed() const { return impl_->failed.load(); }

AudioCaptureFailure AudioCapture::last_failure() const { return impl_->failure; }
} // namespace msime::windows
