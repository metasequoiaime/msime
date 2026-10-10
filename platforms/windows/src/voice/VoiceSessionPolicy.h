#pragma once
// Decisions VoiceInputSession makes that need no Win32: how much of a capture callback a batch recording keeps, and which sentence the person dictating is shown when a recording cannot start or does not produce text. The wording is MSIME-Windows voice_input_service.cpp's, which shows each of these in a message box; this host shows them on the voice overlay instead.

#include "../../../../shared/voice/VoiceProviders.h"
#include "AudioCapture.h"

#include <cstddef>
#include <exception>
#include <string>
#include <string_view>

namespace msime::windows {
// What a batch recording keeps of one capture callback. `keep` frames are appended; `full` means the buffer has reached `limit` and the recording should finish now and submit what it holds, the way the macOS host does, rather than keep listening to audio it would drop.
struct VoiceBatchCapture {
  std::size_t keep = 0;
  bool full = false;
};

constexpr VoiceBatchCapture voice_batch_capture(std::size_t captured,
                                                std::size_t frames,
                                                std::size_t limit) {
  if (captured >= limit)
    return {0, true};
  const std::size_t room = limit - captured;
  if (frames >= room)
    return {room, true};
  return {frames, false};
}

// The batch buffer is bounded by what one upload can carry, not by a fixed minute count: MSIME-Windows only rejects a recording at its 20 MiB upload check.
inline constexpr std::size_t voice_batch_sample_limit = batch_capture_sample_limit;

inline constexpr std::string_view voice_missing_token_message =
    "请先在设置的“语音输入”分区填写当前 ASR 提供商的 API Token。";
inline constexpr std::string_view voice_missing_endpoint_message =
    "ASR Token 或接口地址为空。";
inline constexpr std::string_view voice_missing_model_message = "ASR 模型名为空。";
// MSIME-Windows ends this sentence with "请检查 config.toml。"; here the same fields are edited in the settings page.
inline constexpr std::string_view voice_doubao_start_message =
    "无法启动豆包流式语音识别。请检查“语音输入”设置中的接口地址、凭据和资源 ID。";
inline constexpr std::string_view voice_microphone_start_message = "无法启动麦克风。";
// Windows 隐私设置拒绝了桌面应用的麦克风访问（WASAPI 返回 E_ACCESSDENIED）。对应 macOS 的「请在系统设置允许麦克风访问」，这里写出 Windows 设置里那个开关的位置。
inline constexpr std::string_view voice_microphone_permission_message =
    "请在 Windows 设置 › 隐私和安全性 › 麦克风 中允许桌面应用访问麦克风。";
// 设置里选定的录音设备不在了，或者存的是别的平台的设备，文案沿用 macOS VoiceCaptureDevice.h。
inline constexpr std::string_view voice_microphone_device_message =
    "所选麦克风不可用，请重新选择录音设备";
inline constexpr std::string_view voice_capture_interrupted_message = "录音中断，请重试。";
// 识别正常结束却没有文字，文案与 macOS VoiceWaveOverlay 的 MSIMEVoiceFailureNoSpeech 相同。不足 0.25 秒的短录音不走这里，和 macOS 一样静默结束。
inline constexpr std::string_view voice_no_speech_message = "未识别到语音，请重试";
// 润色请求的整体超时，与 macOS HTTPVoiceRequest.mm 相同。共享层默认的 3 秒等不到 chat completion 返回，润色结果会被丢掉、只剩原始识别文本，界面上也没有任何提示。
inline constexpr long voice_polish_timeout_ms = 30000;

// 麦克风没能启动时给人看的那句话：权限被拒和设备不可用各有能照着去改的说明，其余情况用通用的一句。
constexpr std::string_view voice_capture_start_message(AudioCaptureFailure failure) {
  switch (failure) {
  case AudioCaptureFailure::AccessDenied:
    return voice_microphone_permission_message;
  case AudioCaptureFailure::DeviceUnavailable:
    return voice_microphone_device_message;
  default:
    return voice_microphone_start_message;
  }
}
inline constexpr std::string_view voice_recognition_failed_message = "语音识别失败";
// The on-device provider needs a model instead of a token; the settings page downloads one and fills in asr_model_path.
inline constexpr std::string_view voice_missing_local_model_message =
    "请先在“语音输入”设置中下载并选用一个本地模型。";
// sherpa-onnx-c-api.dll or onnxruntime.dll is missing beside the Server or will not load. The installer puts them there, so the fix is to reinstall rather than to change a setting.
inline constexpr std::string_view voice_local_runtime_message =
    "本地语音识别组件无法加载，请重新安装输入法。";
// The configured model directory is gone or was never finished: the settings page can download it again.
inline constexpr std::string_view voice_local_model_unusable_message =
    "本地语音模型不可用，请在“语音输入”设置中重新下载。";

// The configuration a recording needs before anything is opened.
struct VoiceStartConfig {
  bool enabled = true;
  bool doubao = false;
  std::string_view token;
  std::string_view endpoint;
  std::string_view model;
  std::string_view resource_id;
  // The on-device provider: recognition needs `model_path` (an installed model directory) and nothing the cloud providers need.
  bool local = false;
  std::string_view model_path{};
  // Windows 系统识别（SAPI）：不需要 Token、接口地址、模型名，也不需要下载模型；识别器和语言是否可用在开始录音前另行检查（system_asr_start_problem）。
  bool system = false;
};

enum class VoiceStartCheck { Ready, Disabled, Rejected };

struct VoiceStartVerdict {
  VoiceStartCheck check = VoiceStartCheck::Ready;
  // Empty unless `check` is Rejected. A disabled voice input stays silent, as it does in MSIME-Windows.
  std::string_view message;
};

constexpr VoiceStartVerdict voice_start_verdict(const VoiceStartConfig &config) {
  if (!config.enabled)
    return {VoiceStartCheck::Disabled, {}};
  if (config.system)
    return {};
  if (config.local)
    return config.model_path.empty()
               ? VoiceStartVerdict{VoiceStartCheck::Rejected, voice_missing_local_model_message}
               : VoiceStartVerdict{};
  if (config.token.empty())
    return {VoiceStartCheck::Rejected, voice_missing_token_message};
  // MSIME-Windows hands an incomplete Doubao configuration to the streaming client, whose Start() refuses it; a batch provider's gaps surface in Recognize().
  if (config.doubao && (config.endpoint.empty() || config.resource_id.empty()))
    return {VoiceStartCheck::Rejected, voice_doubao_start_message};
  if (!secure_voice_endpoint(config.endpoint, config.doubao))
    return {VoiceStartCheck::Rejected, voice_missing_endpoint_message};
  if (!config.doubao && config.model.empty())
    return {VoiceStartCheck::Rejected, voice_missing_model_message};
  return {};
}

// The sentence for a failed batch recognition: the provider's own message or HTTP status when the shared client supplied one, the generic line otherwise.
inline std::string voice_recognition_failure(const std::exception &error) {
  if (const auto *cloud = dynamic_cast<const CloudAsrError *>(&error))
    if (!cloud->user_message().empty())
      return cloud->user_message();
  return std::string(voice_recognition_failed_message);
}

// The sentence for a failed on-device recognition. The recognizer's own error text is an English diagnostic, so the person dictating is told which of the two things they can act on went wrong - the runtime beside the Server, or the model directory the setting points at - and the generic line otherwise.
constexpr std::string_view voice_local_failure(bool runtime_available,
                                               bool model_installed) {
  if (!runtime_available)
    return voice_local_runtime_message;
  if (!model_installed)
    return voice_local_model_unusable_message;
  return voice_recognition_failed_message;
}

// The ✓ and ✗ buttons appear once a native recording is locked, as in MSIME-Windows ControlLoop: the key that started it is up, so the overlay is the only place left to end it besides the shortcut and Escape. A review capture has no overlay.
constexpr bool voice_lock_shows_actions(bool recording, bool review) {
  return recording && !review;
}
} // namespace msime::windows
