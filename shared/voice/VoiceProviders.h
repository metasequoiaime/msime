#pragma once
// Platform-independent provider adaptation. Preserve historical Windows ABI;
// new hosts use the platform-neutral aliases below.

#include <string>
#include <string_view>
#include <atomic>
#include <cstddef>
#include <memory>
#include <stdexcept>
#include <utility>
#include <vector>

namespace msime::windows {
// MSIME-Windows voice_batch_protocol.cpp caps a batch upload at upload_sample_limit = (maximum_encoded_audio_bytes - 44) / 2: the Engine's 20 MiB encoded-audio budget less the 44-byte WAV header, counted in 16-bit mono samples of the 16 kHz stream (about 655 s). Batch providers are always sent 16-bit WAV, whatever sample format the host captured in, so this is the budget of the bytes actually uploaded. VoiceProviders.cpp checks it against the Engine constants.
inline constexpr std::size_t batch_upload_sample_limit = (20u * 1024u * 1024u - 44u) / 2u;
// Longest recording a batch host keeps: the upload limit less the 0.2 s of silence SiliconFlow gets on each side, so a recording that reached the cap still encodes for every batch provider.
inline constexpr std::size_t batch_capture_sample_limit = batch_upload_sample_limit - 2u * (16000u / 5u);
// The Engine's Whisper worker validates its input with the default 60-second WavWriter budget, so the on-device provider cannot take more than this.
inline constexpr std::size_t local_asr_sample_limit = 16000u * 60u;
std::string transcription_language(std::string_view provider, std::string_view language);
std::string normalize_voice_provider(std::string_view provider);
std::string default_asr_endpoint(std::string_view provider);
std::string default_asr_model(std::string_view provider);
std::string default_polish_endpoint(std::string_view provider);
std::string default_polish_model(std::string_view provider);
// True when the configured provider is Doubao. Deliberately ignores the
// endpoint: see the definition for why trusting it leaked tokens.
bool is_doubao_asr_provider(std::string_view provider);
// ws:// or wss://. Used to spot an endpoint left over from another provider.
bool voice_endpoint_is_websocket(std::string_view endpoint);
// Windows sends bearer credentials directly from the native host. Only the
// provider's encrypted transport may carry them; reject userinfo, fragments,
// whitespace and malformed authorities before libcurl/WinHTTP sees the URL.
bool secure_voice_endpoint(std::string_view endpoint, bool websocket);
std::string resolved_asr_endpoint(std::string_view provider,
                                  std::string_view configured_endpoint);
// What recognize_cloud_asr throws when the request is built, sent or answered badly. what() keeps the terse English diagnostic hosts already log; user_message() is the sentence MSIME-Windows voice_input_service.cpp Recognize() shows the person dictating - the provider's own JSON message or the HTTP status, and for SiliconFlow the trace id its support asks for. It never carries the token or the request body.
class CloudAsrError : public std::runtime_error {
public:
  CloudAsrError(const std::string &diagnostic, std::string user_message)
      : std::runtime_error(diagnostic), user_message_(std::move(user_message)) {}
  const std::string &user_message() const noexcept { return user_message_; }

private:
  std::string user_message_;
};
// MSIME-Windows JsonErrorMessage: the provider's error text from a failed response body - error (string or object.message), else message with its code and data - or, when the body is not such JSON, the body itself cut to 240 bytes. The cut never splits a UTF-8 sequence.
std::string cloud_asr_error_detail(std::string_view body);
// The message for a non-2xx answer: "语音识别失败：" and the detail above, "HTTP <status>" when there is none, plus the SiliconFlow 5xx explanation and trace id.
std::string cloud_asr_status_message(long status, std::string_view body,
                                     std::string_view provider,
                                     std::string_view model,
                                     std::string_view trace_id);
// The message for a request that never got an HTTP answer; `detail` is the transport's own error text.
std::string cloud_asr_transport_message(std::string_view detail);
std::string recognize_cloud_asr(
    const std::vector<float> &samples, std::string_view provider,
    std::string_view endpoint, std::string_view model, std::string_view token,
    std::string_view language,
    const std::shared_ptr<std::atomic_bool> &cancelled);
// timeout_ms is the whole request budget - connection, upload and response - not a connect timeout. The default is the Windows one; a host that needs longer states its own. Getting this wrong is silent: callers keep the ASR text when polish fails, so a budget the service cannot meet means the transcript was uploaded and the answer thrown away with nothing shown.
std::string polish_cloud_text(
    std::string_view text, std::string_view provider, std::string_view endpoint,
    std::string_view model, std::string_view token, std::string_view prompt,
    const std::shared_ptr<std::atomic_bool> &cancelled, long timeout_ms = 3000);
// Whether this host can recognize on-device: the build carries Whisper, or the sherpa-onnx runtime loads (LocalAsr.h). Hosts offer the "local" provider only when it answers true; without it the recognizer below always throws, and a host that advertised the option anyway would fall back to the platform recognizer without saying so.
bool local_asr_available();
// Transcribe on this machine. `model_path` is an installed catalog model directory (LocalAsr.h), or a Whisper ggml file when the build carries Whisper. Nothing leaves the process. `language` is the host's language tag; "auto" asks the model to detect. `hotwords` are the user's words for models that take them natively. Throws VoiceError when neither recognizer can take the model, the model cannot be loaded, or the request was cancelled.
std::string recognize_local_asr(
    const std::vector<float> &samples, std::string_view model_path,
    std::string_view language,
    const std::shared_ptr<std::atomic_bool> &cancelled,
    const std::vector<std::string> &hotwords = {});
}
namespace msime::voice {
using windows::batch_upload_sample_limit;
using windows::batch_capture_sample_limit;
using windows::local_asr_sample_limit;
using windows::transcription_language;
using windows::normalize_voice_provider;
using windows::default_asr_endpoint;
using windows::default_asr_model;
using windows::default_polish_endpoint;
using windows::default_polish_model;
using windows::is_doubao_asr_provider;
using windows::voice_endpoint_is_websocket;
using windows::secure_voice_endpoint;
using windows::resolved_asr_endpoint;
using windows::CloudAsrError;
using windows::cloud_asr_error_detail;
using windows::cloud_asr_status_message;
using windows::cloud_asr_transport_message;
using windows::recognize_cloud_asr;
using windows::polish_cloud_text;
using windows::local_asr_available;
using windows::recognize_local_asr;
}
