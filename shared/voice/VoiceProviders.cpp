#include "VoiceProviders.h"
#include "LocalAsr.h"
// Shared implementation; the historical namespace is retained for ABI compatibility.

#include <curl/curl.h>
#include <nlohmann/json.hpp>

#include <algorithm>
#include <cctype>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <limits>
#include <mutex>
#include <thread>
#include <utility>

namespace msime::windows {
namespace {
using msime::voice::VoiceError;
// Every batch provider is sent 16 kHz mono 16-bit WAV, and none takes more than 20 MiB of it.
constexpr std::uint32_t sample_rate = 16000;
constexpr std::size_t maximum_encoded_audio_bytes = 20u * 1024u * 1024u;
constexpr std::size_t wav_header_bytes = 44;
} // namespace
static_assert(batch_upload_sample_limit == (maximum_encoded_audio_bytes - wav_header_bytes) / 2);
static_assert(batch_capture_sample_limit == batch_upload_sample_limit - 2 * (sample_rate / 5));
namespace {
std::string lower(std::string_view value) {
  std::string result(value);
  for (char &ch : result)
    if (ch >= 'A' && ch <= 'Z')
      ch = static_cast<char>(ch - 'A' + 'a');
  return result;
}
} // namespace

std::string normalize_voice_provider(std::string_view provider) {
  const auto result = lower(provider);
  return result.empty() ? "doubao" : result;
}

std::string transcription_language(std::string_view provider, std::string_view language) {
  if (normalize_voice_provider(provider) == "siliconflow")
    return {};
  auto normalized = lower(language);
  if (normalized == "auto")
    return {};
  // Shared panels use locale tags (zh-CN, en-US); transcription requests
  // use their language component, not the regional or script subtag.
  const auto separator = normalized.find_first_of("-_");
  if (separator != std::string::npos)
    normalized.resize(separator);
  return normalized;
}

std::string default_asr_endpoint(std::string_view provider) {
  const auto id = normalize_voice_provider(provider);
  if (id == "openai")
    return "https://api.openai.com/v1/audio/transcriptions";
  if (id == "groq")
    return "https://api.groq.com/openai/v1/audio/transcriptions";
  if (id == "siliconflow")
    return "https://api.siliconflow.cn/v1/audio/transcriptions";
  if (id == "everyapi")
    return "https://api.everyapi.ai/v1/audio/transcriptions";
  if (id == "mistral")
    return "https://api.mistral.ai/v1/audio/transcriptions";
  if (id == "doubao")
    return "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";
  // Keep unknown legacy values on the HTTP provider fallback used by the
  // Windows reference instead of silently selecting the Doubao websocket.
  return "https://api.siliconflow.cn/v1/audio/transcriptions";
}

std::string default_asr_model(std::string_view provider) {
  const auto id = normalize_voice_provider(provider);
  if (id == "openai")
    return "whisper-1";
  if (id == "groq")
    return "whisper-large-v3-turbo";
  if (id == "siliconflow")
    return "FunAudioLLM/SenseVoiceSmall";
  if (id == "everyapi")
    return "openai/whisper-large-v3-turbo";
  if (id == "mistral")
    return "voxtral-mini-latest";
  if (id == "doubao")
    return {};
  return "FunAudioLLM/SenseVoiceSmall";
}

std::string default_polish_endpoint(std::string_view provider) {
  const auto id = normalize_voice_provider(provider);
  if (id == "openai")
    return "https://api.openai.com/v1/chat/completions";
  if (id == "deepseek")
    return "https://api.deepseek.com/chat/completions";
  if (id == "groq")
    return "https://api.groq.com/openai/v1/chat/completions";
  return "https://api.siliconflow.cn/v1/chat/completions";
}

std::string default_polish_model(std::string_view provider) {
  const auto id = normalize_voice_provider(provider);
  if (id == "openai")
    return "gpt-4o-mini";
  if (id == "deepseek")
    return "deepseek-v4-flash";
  if (id == "groq")
    return "llama-3.3-70b-versatile";
  return "Qwen/Qwen3-8B";
}

bool voice_endpoint_is_websocket(std::string_view endpoint) {
  return endpoint.rfind("wss://", 0) == 0 || endpoint.rfind("ws://", 0) == 0;
}
bool secure_voice_endpoint(std::string_view endpoint, bool websocket) {
  constexpr std::size_t maximum_bytes = 2048;
  const std::string_view scheme = websocket ? "wss://" : "https://";
  if (endpoint.empty() || endpoint.size() > maximum_bytes ||
      endpoint.rfind(scheme, 0) != 0)
    return false;
  for (const unsigned char byte : endpoint) {
    if (byte < 0x20 || byte == 0x7f || byte == '\\' ||
        (byte < 0x80 && std::isspace(byte)))
      return false;
  }
  const auto authority_start = scheme.size();
  const auto authority_end = endpoint.find_first_of("/?#", authority_start);
  const auto authority = endpoint.substr(
      authority_start, authority_end == std::string_view::npos
                           ? std::string_view::npos
                           : authority_end - authority_start);
  return !authority.empty() && authority.front() != ':' &&
         authority.find('@') == std::string_view::npos &&
         endpoint.find('#', authority_start) == std::string_view::npos;
}
std::string resolved_asr_endpoint(std::string_view provider,
                                  std::string_view configured_endpoint) {
  const bool doubao = is_doubao_asr_provider(provider);
  if (configured_endpoint.empty() ||
      (voice_endpoint_is_websocket(configured_endpoint) != doubao))
    return default_asr_endpoint(provider);
  return std::string(configured_endpoint);
}
bool is_doubao_asr_provider(std::string_view provider) {
  // The provider id decides, and only the provider id. This used to also treat
  // any wss:// endpoint as Doubao, which inverted the relationship: the shipped
  // default endpoint is the Doubao websocket URL, so a user who chose OpenAI
  // but whose stored endpoint had never been rewritten was routed to
  // DoubaoAsrClient - sending their OpenAI token to ByteDance. An endpoint that
  // disagrees with the provider is stale configuration, not a provider choice.
  // normalize_voice_provider already maps an unset provider to doubao, so
  // nothing is lost by ignoring the endpoint here.
  return normalize_voice_provider(provider) == "doubao";
}

namespace {
struct Response {
  std::string body;
};

// SiliconFlow answers every request with this header; its support needs the value to find a failed one.
size_t write_trace_header(char *data, size_t size, size_t count, void *context) {
  if (size && count > (std::numeric_limits<size_t>::max)() / size)
    return 0;
  const auto length = size * count;
  constexpr std::string_view name = "x-siliconcloud-trace-id:";
  std::string_view line(data, length);
  if (line.size() > name.size() && lower(line.substr(0, name.size())) == name) {
    line.remove_prefix(name.size());
    while (!line.empty() && (line.front() == ' ' || line.front() == '\t'))
      line.remove_prefix(1);
    while (!line.empty() && (line.back() == '\r' || line.back() == '\n' || line.back() == ' '))
      line.remove_suffix(1);
    // A trace id is a short token; anything else is not worth showing anyone.
    if (line.size() <= 128)
      *static_cast<std::string *>(context) = std::string(line);
  }
  return length;
}

size_t write_response(char *data, size_t size, size_t count, void *context) {
  if (size && count > (std::numeric_limits<size_t>::max)() / size)
    return 0;
  const auto length = size * count;
  auto &response = *static_cast<Response *>(context);
  if (length > 1024 * 1024 - response.body.size())
    return 0;
  response.body.append(data, length);
  return length;
}

int progress(void *context, curl_off_t, curl_off_t, curl_off_t, curl_off_t) {
  const auto *cancelled = static_cast<const std::atomic_bool *>(context);
  return cancelled && cancelled->load() ? 1 : 0;
}

void initialize_curl() {
  static std::once_flag once;
  static CURLcode result = CURLE_OK;
  std::call_once(once, [] { result = curl_global_init(CURL_GLOBAL_DEFAULT); });
  if (result != CURLE_OK)
    throw VoiceError("Cannot initialize HTTP runtime");
}

// The recording as a 16 kHz mono 16-bit PCM WAV file, the one upload format every batch provider accepts. Samples are clipped to [-1, 1]; a non-finite sample means the capture is broken, so the recording is refused rather than sent.
std::string encode_wav(const std::vector<float> &samples) {
  if (samples.size() > batch_upload_sample_limit)
    throw VoiceError("Audio exceeds the upload limit");
  const auto data_bytes = static_cast<std::uint32_t>(samples.size() * 2);
  std::string wav;
  wav.reserve(wav_header_bytes + data_bytes);
  const auto tag = [&](const char (&value)[5]) { wav.append(value, 4); };
  const auto u16 = [&](std::uint16_t value) {
    wav.push_back(static_cast<char>(value & 0xff));
    wav.push_back(static_cast<char>(value >> 8));
  };
  const auto u32 = [&](std::uint32_t value) {
    u16(static_cast<std::uint16_t>(value & 0xffff));
    u16(static_cast<std::uint16_t>(value >> 16));
  };
  tag("RIFF");
  u32(36 + data_bytes);
  tag("WAVE");
  tag("fmt ");
  u32(16);
  u16(1); // PCM
  u16(1); // mono
  u32(sample_rate);
  u32(sample_rate * 2);
  u16(2);
  u16(16);
  tag("data");
  u32(data_bytes);
  for (float sample : samples) {
    if (!std::isfinite(sample))
      throw VoiceError("Audio contains a non-finite sample");
    sample = std::clamp(sample, -1.0f, 1.0f);
    u16(static_cast<std::uint16_t>(static_cast<std::int16_t>(sample * 32767.0f)));
  }
  return wav;
}

// The transcript in an OpenAI-style answer: `text`, then `transcription`, then `result.text`, whichever is a non-empty string first. write_response has already bounded the body to 1 MiB.
std::string parse_transcription(const std::string &response) {
  nlohmann::json json;
  try {
    json = nlohmann::json::parse(response);
  } catch (const nlohmann::json::exception &) {
    throw VoiceError("Invalid voice response");
  }
  const auto member = [](const nlohmann::json &object, const char *key) {
    if (!object.is_object())
      return std::string();
    const auto found = object.find(key);
    return found != object.end() && found->is_string() ? found->get<std::string>() : std::string();
  };
  for (const char *key : {"text", "transcription"})
    if (auto text = member(json, key); !text.empty())
      return text;
  if (json.is_object() && json.contains("result"))
    if (auto text = member(json["result"], "text"); !text.empty())
      return text;
  throw VoiceError("Missing transcription text");
}
} // namespace

std::string cloud_asr_error_detail(std::string_view body) {
  try {
    const auto json = nlohmann::json::parse(body);
    if (json.is_object()) {
      if (json.contains("error")) {
        const auto &error = json["error"];
        if (error.is_string())
          return error.get<std::string>();
        if (error.is_object() && error.contains("message") && error["message"].is_string())
          return error["message"].get<std::string>();
      }
      if (json.contains("message") && json["message"].is_string()) {
        std::string message = json["message"].get<std::string>();
        if (json.contains("code") && !json["code"].is_null())
          message += "（code " + json["code"].dump() + "）";
        if (json.contains("data") && json["data"].is_string() && !json["data"].get<std::string>().empty())
          message += " " + json["data"].get<std::string>();
        return message;
      }
    }
  } catch (const nlohmann::json::exception &) {
  }
  if (body.size() <= 240)
    return std::string(body);
  size_t cut = 240;
  while (cut && (static_cast<unsigned char>(body[cut]) & 0xC0) == 0x80)
    --cut;
  return std::string(body.substr(0, cut)) + "...";
}

std::string cloud_asr_status_message(long status, std::string_view body,
                                     std::string_view provider,
                                     std::string_view model,
                                     std::string_view trace_id) {
  std::string detail = cloud_asr_error_detail(body);
  if (detail.empty())
    detail = "HTTP " + std::to_string(status);
  if (status >= 500 && normalize_voice_provider(provider) == "siliconflow") {
    detail += "。这是硅基流动服务端内部错误，模型名 " + std::string(model) + " 本身是官方支持的。";
    if (!trace_id.empty())
      detail += " 追踪 ID：" + std::string(trace_id) + "。";
  }
  return "语音识别失败：" + detail;
}

std::string cloud_asr_transport_message(std::string_view detail) {
  return "语音识别请求失败：" + std::string(detail);
}

std::string recognize_cloud_asr(
    const std::vector<float> &samples, std::string_view provider,
    std::string_view endpoint, std::string_view model, std::string_view token,
    std::string_view language,
    const std::shared_ptr<std::atomic_bool> &cancelled) {
  if (samples.empty())
    return {};
  const auto id = normalize_voice_provider(provider);
  if (endpoint.empty() || model.empty() || token.empty())
    throw VoiceError("Voice endpoint, model and token are required");
  if (cancelled && cancelled->load())
    throw VoiceError("Voice request cancelled");

  std::vector<float> padded;
  const std::vector<float> *audio = &samples;
  if (id == "siliconflow") {
    constexpr size_t padding = sample_rate / 5;
    padded.reserve(samples.size() + padding * 2);
    padded.insert(padded.end(), padding, 0.0f);
    padded.insert(padded.end(), samples.begin(), samples.end());
    padded.insert(padded.end(), padding, 0.0f);
    if (padded.size() < static_cast<size_t>(sample_rate))
      padded.resize(sample_rate, 0.0f);
    audio = &padded;
  }
  std::string wav;
  try {
    wav = encode_wav(*audio);
  } catch (const VoiceError &error) {
    throw CloudAsrError(error.what(), "录音数据无效或超过 20 MiB 上传限制。");
  }
  const std::string model_value(model);
  const std::string language_value = transcription_language(id, language);
  initialize_curl();
  const std::string endpoint_value(endpoint);

  const int attempts = id == "siliconflow" ? 2 : 1;
  std::string response;
  long status = 0;
  CURLcode result = CURLE_OK;
  char error[CURL_ERROR_SIZE] = {};
  std::string trace_id;
  for (int attempt = 0; attempt < attempts; ++attempt) {
    if (attempt)
      std::this_thread::sleep_for(std::chrono::milliseconds(400));
    if (cancelled && cancelled->load())
      throw VoiceError("Voice request cancelled");
    response.clear();
    trace_id.clear();
    Response response_data;
    error[0] = '\0';
    // Declared before the handle so it is freed after it: the handle points at the form until curl_easy_cleanup.
    std::unique_ptr<curl_mime, decltype(&curl_mime_free)> form(nullptr, curl_mime_free);
    std::unique_ptr<CURL, decltype(&curl_easy_cleanup)> curl(
        curl_easy_init(), curl_easy_cleanup);
    if (!curl)
      throw VoiceError("Cannot create HTTP request");
    // The multipart body is libcurl's: it picks a boundary that occurs in no part and sets the Content-Type header that names it.
    form.reset(curl_mime_init(curl.get()));
    const auto add_part = [&](const char *name, std::string_view value, const char *filename,
                              const char *type) {
      curl_mimepart *part = form ? curl_mime_addpart(form.get()) : nullptr;
      return part && curl_mime_name(part, name) == CURLE_OK &&
             curl_mime_data(part, value.data(), value.size()) == CURLE_OK &&
             (!filename || curl_mime_filename(part, filename) == CURLE_OK) &&
             (!type || curl_mime_type(part, type) == CURLE_OK);
    };
    if (!add_part("model", model_value, nullptr, nullptr) ||
        (!language_value.empty() && !add_part("language", language_value, nullptr, nullptr)) ||
        !add_part("file", wav, "audio.wav", "audio/wav"))
      throw VoiceError("Cannot create HTTP request");
    std::unique_ptr<curl_slist, decltype(&curl_slist_free_all)> headers(
        curl_slist_append(nullptr, ("Authorization: Bearer " +
                                    std::string(token))
                               .c_str()),
        curl_slist_free_all);
    if (!headers)
      throw VoiceError("Cannot create HTTP headers");
    auto *next = curl_slist_append(headers.get(), "Expect:");
    if (!next)
      throw VoiceError("Cannot create HTTP headers");
    headers.release();
    headers.reset(next);
    curl_easy_setopt(curl.get(), CURLOPT_URL, endpoint_value.c_str());
    curl_easy_setopt(curl.get(), CURLOPT_HTTPHEADER, headers.get());
    curl_easy_setopt(curl.get(), CURLOPT_MIMEPOST, form.get());
    curl_easy_setopt(curl.get(), CURLOPT_WRITEFUNCTION, write_response);
    curl_easy_setopt(curl.get(), CURLOPT_WRITEDATA, &response_data);
    curl_easy_setopt(curl.get(), CURLOPT_HEADERFUNCTION, write_trace_header);
    curl_easy_setopt(curl.get(), CURLOPT_HEADERDATA, &trace_id);
    curl_easy_setopt(curl.get(), CURLOPT_ERRORBUFFER, error);
    curl_easy_setopt(curl.get(), CURLOPT_NOPROGRESS, 0L);
    curl_easy_setopt(curl.get(), CURLOPT_XFERINFOFUNCTION, progress);
    curl_easy_setopt(curl.get(), CURLOPT_XFERINFODATA, cancelled.get());
    curl_easy_setopt(curl.get(), CURLOPT_HTTP_VERSION, CURL_HTTP_VERSION_1_1);
    curl_easy_setopt(curl.get(), CURLOPT_CONNECTTIMEOUT_MS, 15000L);
    curl_easy_setopt(curl.get(), CURLOPT_TIMEOUT_MS, 60000L);
    curl_easy_setopt(curl.get(), CURLOPT_NOSIGNAL, 1L);
    result = curl_easy_perform(curl.get());
    response = std::move(response_data.body);
    curl_easy_getinfo(curl.get(), CURLINFO_RESPONSE_CODE, &status);
    if (result == CURLE_OK && status < 500)
      break;
  }
  if (result != CURLE_OK) {
    const std::string detail = error[0] ? error : curl_easy_strerror(result);
    throw CloudAsrError("Voice HTTP request failed: " + detail,
                        cloud_asr_transport_message(detail));
  }
  if (status < 200 || status >= 300)
    throw CloudAsrError("Voice HTTP status " + std::to_string(status),
                        cloud_asr_status_message(status, response, id, model, trace_id));
  try {
    return parse_transcription(response);
  } catch (const VoiceError &error) {
    throw CloudAsrError(error.what(), "语音识别返回了无法解析的结果。");
  }
}

std::string polish_cloud_text(
    std::string_view text, std::string_view provider, std::string_view endpoint,
    std::string_view model, std::string_view token, std::string_view prompt,
    const std::shared_ptr<std::atomic_bool> &cancelled, long timeout_ms) {
  if (text.empty())
    return {};
  if (endpoint.empty() || model.empty() || token.empty() || prompt.empty())
    throw VoiceError(
        "Voice polish endpoint, model, token and prompt are required");
  if (cancelled && cancelled->load())
    throw VoiceError("Voice request cancelled");
  const auto id = normalize_voice_provider(provider);
  nlohmann::json body = {
      {"model", std::string(model)},
      {"stream", false},
      {"messages", {{{"role", "system"}, {"content", std::string(prompt)}},
                     {{"role", "user"},
                      {"content", "<asr_text>\n" + std::string(text) +
                                      "\n</asr_text>"}}}}};
  if (id == "siliconflow")
    body["enable_thinking"] = false;
  else if (id == "deepseek")
    body["thinking"] = {{"type", "disabled"}};
  const std::string payload = body.dump();
  initialize_curl();
  const std::string endpoint_value(endpoint);
  std::unique_ptr<CURL, decltype(&curl_easy_cleanup)> curl(
      curl_easy_init(), curl_easy_cleanup);
  if (!curl)
    throw VoiceError("Cannot create HTTP request");
  const std::string authorization = "Authorization: Bearer " + std::string(token);
  std::unique_ptr<curl_slist, decltype(&curl_slist_free_all)> headers(
      curl_slist_append(nullptr, authorization.c_str()), curl_slist_free_all);
  if (!headers)
    throw VoiceError("Cannot create HTTP headers");
  auto *next = curl_slist_append(headers.get(), "Content-Type: application/json");
  if (!next)
    throw VoiceError("Cannot create HTTP headers");
  headers.release();
  headers.reset(next);
  Response response;
  char error[CURL_ERROR_SIZE] = {};
  curl_easy_setopt(curl.get(), CURLOPT_URL, endpoint_value.c_str());
  curl_easy_setopt(curl.get(), CURLOPT_HTTPHEADER, headers.get());
  curl_easy_setopt(curl.get(), CURLOPT_POST, 1L);
  curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDS, payload.data());
  curl_easy_setopt(curl.get(), CURLOPT_POSTFIELDSIZE_LARGE,
                   static_cast<curl_off_t>(payload.size()));
  curl_easy_setopt(curl.get(), CURLOPT_WRITEFUNCTION, write_response);
  curl_easy_setopt(curl.get(), CURLOPT_WRITEDATA, &response);
  curl_easy_setopt(curl.get(), CURLOPT_ERRORBUFFER, error);
  curl_easy_setopt(curl.get(), CURLOPT_NOPROGRESS, 0L);
  curl_easy_setopt(curl.get(), CURLOPT_XFERINFOFUNCTION, progress);
  curl_easy_setopt(curl.get(), CURLOPT_XFERINFODATA, cancelled.get());
  // The connect budget cannot usefully exceed the whole-request budget: with a 3s total, a 15s connect
  // timeout can never be reached. Keep it within whatever the caller allows.
  curl_easy_setopt(curl.get(), CURLOPT_CONNECTTIMEOUT_MS,
                   static_cast<long>(std::min<long long>(15000, timeout_ms)));
  // Whole request, not connection. MSIME-Windows develop 30a22e6f chose 3s so a slow polish service cannot
  // hold the ASR text, and that remains the default. Hosts whose reference measured otherwise pass their
  // own - see the macOS caller.
  curl_easy_setopt(curl.get(), CURLOPT_TIMEOUT_MS, timeout_ms);
  curl_easy_setopt(curl.get(), CURLOPT_NOSIGNAL, 1L);
  const auto result = curl_easy_perform(curl.get());
  if (result != CURLE_OK)
    throw VoiceError(
        std::string("Voice polish request failed: ") +
        (error[0] ? error : curl_easy_strerror(result)));
  long status = 0;
  curl_easy_getinfo(curl.get(), CURLINFO_RESPONSE_CODE, &status);
  if (status < 200 || status >= 300)
    throw VoiceError("Voice polish HTTP status " +
                                         std::to_string(status));
  try {
    const auto result_json = nlohmann::json::parse(response.body);
    const auto polished = result_json.at("choices")
                              .at(0)
                              .at("message")
                              .at("content")
                              .get<std::string>();
    if (!polished.empty())
      return polished;
  } catch (const nlohmann::json::exception &) {
  }
  throw VoiceError("Missing polished text");
}

bool local_asr_available() { return msime::voice::sherpa_runtime_available(); }

std::string recognize_local_asr(
    const std::vector<float> &samples, std::string_view model_path,
    std::string_view language,
    const std::shared_ptr<std::atomic_bool> &cancelled,
    const std::vector<std::string> &hotwords) {
  if (samples.empty())
    return {};
  if (model_path.empty())
    throw VoiceError("Local speech model path is required");
  if (cancelled && cancelled->load())
    throw VoiceError("Voice request cancelled");
  if (!msime::voice::is_local_model_dir(model_path))
    throw VoiceError("Not an installed local speech model; choose a model from the local model list");
  msime::voice::LocalAsrOptions options;
  options.model_dir = std::string(model_path);
  options.language = std::string(language);
  options.hotwords = hotwords;
  return msime::voice::recognize_local_model(samples, options, cancelled);
}
} // namespace msime::windows
