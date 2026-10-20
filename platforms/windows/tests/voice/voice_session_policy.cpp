#include "VoiceSessionPolicy.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("voice session policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)

constexpr std::size_t kRate = 16000;
} // namespace

int main() {
  try {
    // The batch buffer is the shared upload budget, about 655 s, not a minute. A 60 s cap used to discard every longer recording outright.
    REQUIRE(voice_batch_sample_limit == batch_capture_sample_limit);
    REQUIRE(voice_batch_sample_limit > kRate * 600);
    REQUIRE(voice_batch_sample_limit < batch_upload_sample_limit);
    const auto minute = voice_batch_capture(kRate * 60, 3200, voice_batch_sample_limit);
    REQUIRE(minute.keep == 3200 && !minute.full);
    // 录音按服务的上传上限截取：阿里云百炼（chat_audio）约 218 秒，录到就结束并提交；其余整句服务仍是上面的上限。
    REQUIRE(batch_capture_sample_limit_for("openai") == voice_batch_sample_limit);
    const auto bailian = batch_capture_sample_limit_for("bailian");
    REQUIRE(bailian * 2 + 44 <= chat_audio_max_wav_bytes && bailian > kRate * 210);
    REQUIRE(voice_batch_capture(kRate * 240, 3200, bailian).full);

    // Below the limit everything is kept and the recording goes on.
    const auto early = voice_batch_capture(0, 3200, 10000);
    REQUIRE(early.keep == 3200 && !early.full);
    // The callback that reaches the limit keeps what fits and ends the recording; it is submitted, not thrown away.
    const auto edge = voice_batch_capture(9000, 3200, 10000);
    REQUIRE(edge.keep == 1000 && edge.full);
    const auto exact = voice_batch_capture(6800, 3200, 10000);
    REQUIRE(exact.keep == 3200 && exact.full);
    // Callbacks that arrive before the control thread stops the capture add nothing.
    const auto late = voice_batch_capture(10000, 3200, 10000);
    REQUIRE(late.keep == 0 && late.full);
    const auto empty = voice_batch_capture(0, 0, 10000);
    REQUIRE(empty.keep == 0 && !empty.full);

    // Starting: a disabled voice input stays silent; every other refusal says why, in MSIME-Windows' words.
    REQUIRE(voice_start_verdict({false, false, "", "", "", ""}).check == VoiceStartCheck::Disabled);
    REQUIRE(voice_start_verdict({false, false, "", "", "", ""}).message.empty());
    const auto no_token = voice_start_verdict({true, true, "", "wss://synthetic", "", "synthetic"});
    REQUIRE(no_token.check == VoiceStartCheck::Rejected);
    REQUIRE(no_token.message == voice_missing_token_message);
    REQUIRE(voice_start_verdict({true, false, "", "https://synthetic", "m", ""}).message ==
            voice_missing_token_message);
    REQUIRE(voice_start_verdict({true, true, "t", "wss://synthetic", "", ""}).message ==
            voice_doubao_start_message);
    // 读配置时空资源 ID 已经回退到 seedasr，与设置页的测试按钮一致，所以留空的豆包配置照常开始录音。
    REQUIRE(doubao_resource_id("") == voice_doubao_default_resource_id);
    REQUIRE(doubao_resource_id(" \t ") == "volc.seedasr.sauc.duration");
    REQUIRE(doubao_resource_id(" volc.bigasr.sauc.duration ") == "volc.bigasr.sauc.duration");
    const auto fallback = doubao_resource_id("");
    REQUIRE(voice_start_verdict({true, true, "t", "wss://synthetic", "", fallback}).check ==
            VoiceStartCheck::Ready);
    REQUIRE(voice_start_verdict({true, true, "t", "", "", "r"}).message ==
            voice_doubao_start_message);
    REQUIRE(voice_start_verdict({true, false, "t", "", "m", ""}).message ==
            voice_missing_endpoint_message);
    REQUIRE(voice_start_verdict({true, false, "t", "https://synthetic", "", ""}).message ==
            voice_missing_model_message);
    // Doubao needs no model name, batch providers need no resource id.
    REQUIRE(voice_start_verdict({true, true, "t", "wss://synthetic", "", "r"}).check ==
            VoiceStartCheck::Ready);
    REQUIRE(voice_start_verdict({true, false, "t", "https://synthetic", "m", ""}).check ==
            VoiceStartCheck::Ready);
    REQUIRE(voice_start_verdict({true, false, "t", "http://synthetic", "m", ""}).message ==
            voice_missing_endpoint_message);
    REQUIRE(voice_start_verdict({true, false, "t", "https://user:pass@synthetic", "m", ""}).message ==
            voice_missing_endpoint_message);
    REQUIRE(voice_start_verdict({true, true, "t", "ws://synthetic", "", "r"}).message ==
            voice_missing_endpoint_message);
    // The on-device provider needs an installed model and none of the cloud fields: no token, endpoint, model name or resource id.
    const auto local_ready = voice_start_verdict({true, false, "", "", "", "", true, "C:\\Users\\synthetic\\voice-models\\x-asr-zh-en-streaming"});
    REQUIRE(local_ready.check == VoiceStartCheck::Ready);
    REQUIRE(local_ready.message.empty());
    const auto local_missing = voice_start_verdict({true, false, "", "", "", "", true, ""});
    REQUIRE(local_missing.check == VoiceStartCheck::Rejected);
    REQUIRE(local_missing.message == voice_missing_local_model_message);
    // A token left over from a cloud provider does not stand in for the model.
    REQUIRE(voice_start_verdict({true, false, "t", "https://synthetic", "m", "", true, ""}).message ==
            voice_missing_local_model_message);
    // Disabled still wins, silently.
    REQUIRE(voice_start_verdict({false, false, "", "", "", "", true, ""}).check == VoiceStartCheck::Disabled);
    // 缺 Token、缺本地模型各是一类，标题本身就是完整说明，浮层上只有一行；其余配置问题归到识别服务设置，具体那句作为详情。
    REQUIRE(no_token.failure == VoiceFailure::MissingToken);
    REQUIRE(voice_start_failure(no_token) ==
            (VoiceFailureNotice{std::string(voice_missing_token_message), ""}));
    REQUIRE(local_missing.failure == VoiceFailure::MissingLocalModel);
    REQUIRE(voice_start_failure(local_missing) ==
            (VoiceFailureNotice{std::string(voice_missing_local_model_message), ""}));
    const auto doubao_incomplete = voice_start_verdict({true, true, "t", "", "", "r"});
    REQUIRE(doubao_incomplete.failure == VoiceFailure::Provider);
    REQUIRE(voice_start_failure(doubao_incomplete) ==
            (VoiceFailureNotice{"识别失败，请检查语音服务设置", std::string(voice_doubao_start_message)}));
    REQUIRE(voice_start_failure(voice_start_verdict({true, false, "t", "https://synthetic", "", ""})) ==
            (VoiceFailureNotice{"识别失败，请检查语音服务设置", std::string(voice_missing_model_message)}));
    // Windows 系统识别不需要 Token、接口地址、模型名或本地模型；关掉语音时照样静默。
    const auto system_ready = voice_start_verdict({true, false, "", "", "", "", false, "", true});
    REQUIRE(system_ready.check == VoiceStartCheck::Ready);
    REQUIRE(system_ready.message.empty());
    REQUIRE(voice_start_verdict({false, false, "", "", "", "", false, "", true}).check ==
            VoiceStartCheck::Disabled);
    // A cloud provider is unaffected by a model path.
    REQUIRE(voice_start_verdict({true, false, "", "https://synthetic", "m", "", false, "C:\\model"}).message ==
            voice_missing_token_message);

    // A failed on-device recognition names what the person can fix below the recognition headline: the runtime first, then the model; with neither at fault there is no detail.
    REQUIRE(voice_local_failure(false, true).detail == voice_local_runtime_message);
    REQUIRE(voice_local_failure(false, false).detail == voice_local_runtime_message);
    REQUIRE(voice_local_failure(true, false).detail == voice_local_model_unusable_message);
    REQUIRE(voice_local_failure(true, true) ==
            (VoiceFailureNotice{"识别失败，请检查语音服务设置", ""}));
    REQUIRE(voice_local_failure(false, true).headline == "识别失败，请检查语音服务设置");
    // 缺本地模型的提示与 macOS 同一句。
    REQUIRE(voice_missing_local_model_message == "请先在设置的“语音输入”分区下载或选择本地语音模型。");

    REQUIRE(voice_missing_token_message.find("API Token") != std::string_view::npos);
    REQUIRE(voice_capture_interrupted_message == "录音中断，请重试。");
    // The settings page, not a config file, is where these fields are edited here.
    REQUIRE(voice_doubao_start_message.find("config.toml") == std::string_view::npos);

    // A failed batch recognition shows the macOS recognition headline with the provider's sentence as the detail line, and the headline alone when the provider said nothing.
    const CloudAsrError status("Voice HTTP status 401", "语音识别失败：synthetic key rejected（code 401）");
    REQUIRE(voice_recognition_failure(status) ==
            (VoiceFailureNotice{"识别失败，请检查语音服务设置", "语音识别失败：synthetic key rejected（code 401）"}));
    const CloudAsrError blank("Voice HTTP status 500", "");
    REQUIRE(voice_recognition_failure(blank) == (VoiceFailureNotice{"识别失败，请检查语音服务设置", ""}));
    const std::runtime_error other("synthetic");
    REQUIRE(voice_recognition_failure(other) == (VoiceFailureNotice{"识别失败，请检查语音服务设置", ""}));

    // 各类失败的标题与 macOS VoiceWaveOverlay 的措辞相同；麦克风权限一类写出 Windows 设置的路径。
    REQUIRE(voice_failure_headline(VoiceFailure::Capture) == "录音失败，请检查麦克风设置");
    REQUIRE(voice_failure_headline(VoiceFailure::Provider) == "识别失败，请检查语音服务设置");
    REQUIRE(voice_failure_headline(VoiceFailure::NoSpeech) == "未识别到语音，请重试");
    REQUIRE(voice_failure_headline(VoiceFailure::MissingToken) ==
            "请先在设置的“语音输入”分区填写当前 ASR 提供商的 API Token。");
    REQUIRE(voice_failure_headline(VoiceFailure::MissingLocalModel) == voice_missing_local_model_message);
    REQUIRE(voice_failure_headline(VoiceFailure::MicrophonePermission) == voice_microphone_permission_message);
    // 供应商、豆包或系统识别给的具体原因作为第二行；只是笼统的「语音识别失败」，或与标题相同时，只留标题。
    REQUIRE(voice_failure_notice(VoiceFailure::Provider, "豆包语音识别握手失败。") ==
            (VoiceFailureNotice{"识别失败，请检查语音服务设置", "豆包语音识别握手失败。"}));
    REQUIRE(voice_failure_notice(VoiceFailure::Provider, voice_recognition_failed_message).detail.empty());
    REQUIRE(voice_failure_notice(VoiceFailure::NoSpeech, voice_no_speech_message) ==
            (VoiceFailureNotice{std::string(voice_no_speech_message), ""}));
    REQUIRE(voice_failure_notice(VoiceFailure::Capture, voice_capture_interrupted_message).detail ==
            voice_capture_interrupted_message);

    // 麦克风启动失败按原因给出能照着改的提示：权限被拒时标题就是 Windows 隐私设置的路径，设备不可用时在录音失败的标题下请人重选设备，其余只有录音失败的标题。
    REQUIRE(voice_capture_start_failure(AudioCaptureFailure::AccessDenied) ==
            (VoiceFailureNotice{std::string(voice_microphone_permission_message), ""}));
    REQUIRE(voice_microphone_permission_message.find("隐私和安全性") != std::string_view::npos);
    REQUIRE(voice_capture_start_failure(AudioCaptureFailure::DeviceUnavailable) ==
            (VoiceFailureNotice{"录音失败，请检查麦克风设置", std::string(voice_microphone_device_message)}));
    REQUIRE(voice_capture_start_failure(AudioCaptureFailure::Failed) ==
            (VoiceFailureNotice{"录音失败，请检查麦克风设置", ""}));
    REQUIRE(voice_capture_start_failure(AudioCaptureFailure::None) ==
            (VoiceFailureNotice{"录音失败，请检查麦克风设置", ""}));
    // 所选麦克风不可用的提示与 macOS VoiceCaptureDevice.h 同一句。
    REQUIRE(voice_microphone_device_message == "所选麦克风不可用，请重新选择录音设备");
    // 没识别出文字时的提示与 macOS 同一句。
    REQUIRE(voice_no_speech_message == "未识别到语音，请重试");
    // 润色等 30 秒，与 macOS 相同；共享层默认的 3 秒等不到 chat completion。
    REQUIRE(voice_polish_timeout_ms == 30000);

    // Space lock shows the confirm and cancel buttons on a native recording only.
    REQUIRE(voice_lock_shows_actions(true, false));
    REQUIRE(!voice_lock_shows_actions(true, true));
    REQUIRE(!voice_lock_shows_actions(false, false));
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
