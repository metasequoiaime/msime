#pragma once
// Decisions VoiceInputSession makes that need no Win32: how much of a capture callback a batch recording keeps, and what the person dictating is shown when a recording cannot start or does not produce text - the macOS category headline (VoiceWaveOverlay.mm showFailure) and, below it, the specific sentence. The specific wording is mostly MSIME-Windows voice_input_service.cpp's, which shows each of these in a message box; this host shows them on the voice overlay instead.

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

inline constexpr std::string_view voice_recognition_failed_message = "语音识别失败";
// The on-device provider needs a model instead of a token; the settings page downloads one and fills in asr_model_path. 文案与 macOS VoiceWaveOverlay 的 MSIMEVoiceFailureMissingLocalModel 相同。
inline constexpr std::string_view voice_missing_local_model_message =
    "请先在设置的“语音输入”分区下载或选择本地语音模型。";
// sherpa-onnx-c-api.dll or onnxruntime.dll is missing beside the Server or will not load. The installer puts them there, so the fix is to reinstall rather than to change a setting.
inline constexpr std::string_view voice_local_runtime_message =
    "本地语音识别组件无法加载，请重新安装输入法。";
// The configured model directory is gone or was never finished: the settings page can download it again.
inline constexpr std::string_view voice_local_model_unusable_message =
    "本地语音模型不可用，请在“语音输入”设置中重新下载。";

// 浮层上失败提示的分类，对应 macOS VoiceWaveOverlay 的 MSIMEVoiceFailure。每一类有一句固定的标题；更具体的原因（供应商原文、豆包的错误、Windows 专有的设置路径和组件说明）作为第二行详情。macOS 的语音服务会话失败（Session）和流式识别超时（Timeout）在 Windows 上没有对应的路径，这里不设。
enum class VoiceFailure { MicrophonePermission, Capture, Provider, NoSpeech, MissingToken, MissingLocalModel };

inline constexpr std::string_view voice_capture_failed_headline = "录音失败，请检查麦克风设置";
inline constexpr std::string_view voice_provider_failed_headline = "识别失败，请检查语音服务设置";

// 各类失败的标题，措辞与 macOS 相同。麦克风权限一类用 Windows 设置里那个开关的路径，代替 macOS 的「请在系统设置允许麦克风访问」。
constexpr std::string_view voice_failure_headline(VoiceFailure failure) {
  switch (failure) {
  case VoiceFailure::MicrophonePermission:
    return voice_microphone_permission_message;
  case VoiceFailure::Capture:
    return voice_capture_failed_headline;
  case VoiceFailure::NoSpeech:
    return voice_no_speech_message;
  case VoiceFailure::MissingToken:
    return voice_missing_token_message;
  case VoiceFailure::MissingLocalModel:
    return voice_missing_local_model_message;
  default:
    return voice_provider_failed_headline;
  }
}

// 浮层显示的一次失败：标题一行，详情为空时浮层缩成药丸，不为空时排在标题下面的转写区。
struct VoiceFailureNotice {
  std::string headline;
  std::string detail;
  friend bool operator==(const VoiceFailureNotice &left, const VoiceFailureNotice &right) {
    return left.headline == right.headline && left.detail == right.detail;
  }
};

// 一类失败加上它的详情。详情和标题相同（缺 Token、缺本地模型、麦克风权限这几类的标题本身就是完整说明），或者只是笼统的「语音识别失败」（标题已经说了同一件事）时不再重复，只显示标题。
inline VoiceFailureNotice voice_failure_notice(VoiceFailure failure, std::string_view detail = {}) {
  const auto headline = voice_failure_headline(failure);
  if (detail == headline || detail == voice_recognition_failed_message)
    detail = {};
  return {std::string(headline), std::string(detail)};
}

// 麦克风没能启动：权限被拒时标题就是 Windows 隐私设置的路径；设备不可用时标题之下说明请人重选设备；其余情况只有录音失败的标题。
inline VoiceFailureNotice voice_capture_start_failure(AudioCaptureFailure failure) {
  switch (failure) {
  case AudioCaptureFailure::AccessDenied:
    return voice_failure_notice(VoiceFailure::MicrophonePermission);
  case AudioCaptureFailure::DeviceUnavailable:
    return voice_failure_notice(VoiceFailure::Capture, voice_microphone_device_message);
  default:
    return voice_failure_notice(VoiceFailure::Capture);
  }
}

// 豆包资源 ID 留空时用的值，与 client-providers 的录音解析（`voice/provider.rs`）、macOS 的 DoubaoVoiceRequest 以及设置页「测试豆包识别配置」发出的值（原生页 `doubao_test_resource_id`、共享页 `asrServiceCredentialTestConfig`）一致。以前这里不回退：存储里没有这个键时测试按 seedasr 通过，录音却报「无法启动豆包流式语音识别」。
inline constexpr std::string_view voice_doubao_default_resource_id = "volc.seedasr.sauc.duration";

// 存着的豆包资源 ID：去掉首尾空白，为空时用 voice_doubao_default_resource_id。
inline std::string doubao_resource_id(std::string_view stored) {
  constexpr std::string_view blank = " \t\r\n";
  const auto first = stored.find_first_not_of(blank);
  if (first == std::string_view::npos)
    return std::string(voice_doubao_default_resource_id);
  const auto last = stored.find_last_not_of(blank);
  return std::string(stored.substr(first, last - first + 1));
}

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
  // 拒绝时浮层上的分类：缺 Token、缺本地模型各是一类，其余配置问题都算识别服务设置，`message` 作为详情。
  VoiceFailure failure = VoiceFailure::Provider;
};

constexpr VoiceStartVerdict voice_start_verdict(const VoiceStartConfig &config) {
  if (!config.enabled)
    return {VoiceStartCheck::Disabled, {}};
  if (config.system)
    return {};
  if (config.local)
    return config.model_path.empty()
               ? VoiceStartVerdict{VoiceStartCheck::Rejected, voice_missing_local_model_message,
                                   VoiceFailure::MissingLocalModel}
               : VoiceStartVerdict{};
  if (config.token.empty())
    return {VoiceStartCheck::Rejected, voice_missing_token_message, VoiceFailure::MissingToken};
  // MSIME-Windows hands an incomplete Doubao configuration to the streaming client, whose Start() refuses it; a batch provider's gaps surface in Recognize().
  if (config.doubao && (config.endpoint.empty() || config.resource_id.empty()))
    return {VoiceStartCheck::Rejected, voice_doubao_start_message};
  if (!secure_voice_endpoint(config.endpoint, config.doubao))
    return {VoiceStartCheck::Rejected, voice_missing_endpoint_message};
  if (!config.doubao && config.model.empty())
    return {VoiceStartCheck::Rejected, voice_missing_model_message};
  return {};
}

// 拒绝开始录音时浮层上的提示。
inline VoiceFailureNotice voice_start_failure(const VoiceStartVerdict &verdict) {
  return voice_failure_notice(verdict.failure, verdict.message);
}

// A failed batch recognition: the recognition-settings headline, with the provider's own message or HTTP status as the detail when the shared client supplied one, the headline alone otherwise. 和 macOS 一样，供应商原文只作第二行。
inline VoiceFailureNotice voice_recognition_failure(const std::exception &error) {
  if (const auto *cloud = dynamic_cast<const CloudAsrError *>(&error))
    return voice_failure_notice(VoiceFailure::Provider, cloud->user_message());
  return voice_failure_notice(VoiceFailure::Provider);
}

// A failed on-device recognition. The recognizer's own error text is an English diagnostic, so the detail tells the person dictating which of the two things they can act on went wrong - the runtime beside the Server, or the model directory the setting points at - and there is no detail otherwise.
inline VoiceFailureNotice voice_local_failure(bool runtime_available, bool model_installed) {
  if (!runtime_available)
    return voice_failure_notice(VoiceFailure::Provider, voice_local_runtime_message);
  if (!model_installed)
    return voice_failure_notice(VoiceFailure::Provider, voice_local_model_unusable_message);
  return voice_failure_notice(VoiceFailure::Provider);
}

// The ✓ and ✗ buttons appear once a native recording is locked, as in MSIME-Windows ControlLoop: the key that started it is up, so the overlay is the only place left to end it besides the shortcut and Escape. A review capture has no overlay.
constexpr bool voice_lock_shows_actions(bool recording, bool review) {
  return recording && !review;
}
} // namespace msime::windows
