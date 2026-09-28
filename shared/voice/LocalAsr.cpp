#include "LocalAsr.h"

#include <msime/voice/stt_service.h>
#include <nlohmann/json.hpp>
#include <sherpa-onnx/c-api.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <cctype>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <map>
#include <mutex>
#include <set>
#include <sstream>
#include <thread>
#include <tuple>
#include <utility>

#if defined(_WIN32)
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#else
#include <dlfcn.h>
#include <unistd.h>
#endif
#if defined(__APPLE__)
#include <mach-o/dyld.h>
#endif

namespace msime::voice {
namespace {

namespace fs = std::filesystem;
using metasequoia::voice::VoiceError;

constexpr int kSampleRate = 16000;
constexpr int32_t kVadWindow = 512;
constexpr size_t kMaxManifestBytes = 256 * 1024;

// ---- runtime loading ----

#define MSIME_SHERPA_FUNCTIONS(X)                                                                   \
  X(SherpaOnnxCreateOnlineRecognizer)                                                             \
  X(SherpaOnnxDestroyOnlineRecognizer)                                                            \
  X(SherpaOnnxCreateOnlineStream)                                                                 \
  X(SherpaOnnxCreateOnlineStreamWithHotwords)                                                     \
  X(SherpaOnnxDestroyOnlineStream)                                                                \
  X(SherpaOnnxOnlineStreamAcceptWaveform)                                                         \
  X(SherpaOnnxIsOnlineStreamReady)                                                                \
  X(SherpaOnnxDecodeOnlineStream)                                                                 \
  X(SherpaOnnxGetOnlineStreamResult)                                                              \
  X(SherpaOnnxDestroyOnlineRecognizerResult)                                                      \
  X(SherpaOnnxOnlineStreamReset)                                                                  \
  X(SherpaOnnxOnlineStreamInputFinished)                                                          \
  X(SherpaOnnxOnlineStreamIsEndpoint)                                                             \
  X(SherpaOnnxCreateOfflineRecognizer)                                                            \
  X(SherpaOnnxDestroyOfflineRecognizer)                                                           \
  X(SherpaOnnxCreateOfflineStream)                                                                \
  X(SherpaOnnxDestroyOfflineStream)                                                               \
  X(SherpaOnnxAcceptWaveformOffline)                                                              \
  X(SherpaOnnxDecodeOfflineStream)                                                                \
  X(SherpaOnnxGetOfflineStreamResult)                                                             \
  X(SherpaOnnxDestroyOfflineRecognizerResult)                                                     \
  X(SherpaOnnxCreateVoiceActivityDetector)                                                        \
  X(SherpaOnnxDestroyVoiceActivityDetector)                                                       \
  X(SherpaOnnxVoiceActivityDetectorAcceptWaveform)                                                \
  X(SherpaOnnxVoiceActivityDetectorEmpty)                                                         \
  X(SherpaOnnxVoiceActivityDetectorPop)                                                           \
  X(SherpaOnnxVoiceActivityDetectorFront)                                                         \
  X(SherpaOnnxDestroySpeechSegment)                                                               \
  X(SherpaOnnxVoiceActivityDetectorFlush)

struct SherpaApi {
#define MSIME_SHERPA_POINTER(name) decltype(&::name) name = nullptr;
  MSIME_SHERPA_FUNCTIONS(MSIME_SHERPA_POINTER)
#undef MSIME_SHERPA_POINTER
};

#if defined(_WIN32)
constexpr const char *kLibraryName = "sherpa-onnx-c-api.dll";
#elif defined(__APPLE__)
constexpr const char *kLibraryName = "libsherpa-onnx-c-api.dylib";
#else
constexpr const char *kLibraryName = "libsherpa-onnx-c-api.so";
#endif

std::mutex runtime_mutex;
std::string configured_library;
std::string runtime_error;
bool runtime_tried = false;
SherpaApi runtime_api;
bool runtime_loaded = false;

fs::path executable_directory() {
#if defined(_WIN32)
  std::wstring buffer(MAX_PATH, L'\0');
  for (;;) {
    const DWORD length = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
    if (length == 0)
      return {};
    if (length < buffer.size()) {
      buffer.resize(length);
      return fs::path(buffer).parent_path();
    }
    buffer.resize(buffer.size() * 2);
  }
#elif defined(__APPLE__)
  uint32_t size = 0;
  _NSGetExecutablePath(nullptr, &size);
  std::string buffer(size, '\0');
  if (_NSGetExecutablePath(buffer.data(), &size) != 0)
    return {};
  std::error_code error;
  const auto resolved = fs::canonical(buffer.c_str(), error);
  return error ? fs::path(buffer.c_str()).parent_path() : resolved.parent_path();
#else
  std::error_code error;
  const auto resolved = fs::read_symlink("/proc/self/exe", error);
  return error ? fs::path{} : resolved.parent_path();
#endif
}

std::vector<fs::path> library_candidates() {
  std::vector<fs::path> candidates;
  if (!configured_library.empty())
    candidates.emplace_back(fs::u8path(configured_library));
  if (const char *overridden = std::getenv("MSIME_SHERPA_ONNX_LIBRARY"); overridden && *overridden)
    candidates.emplace_back(fs::u8path(overridden));
  if (const auto directory = executable_directory(); !directory.empty()) {
    candidates.push_back(directory / kLibraryName);
#if defined(__APPLE__)
    candidates.push_back(directory / ".." / "Frameworks" / kLibraryName);
#elif !defined(_WIN32)
    candidates.push_back(directory / ".." / "lib" / "msime" / kLibraryName);
    candidates.push_back(directory / ".." / "lib" / kLibraryName);
#endif
  }
  return candidates;
}

void *open_library(const fs::path &path, std::string &error) {
#if defined(_WIN32)
  // Altered search path: onnxruntime.dll, which the C API imports, is found beside the DLL instead of in the process directory.
  HMODULE module = path.has_parent_path()
                       ? LoadLibraryExW(path.wstring().c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH)
                       : LoadLibraryW(path.wstring().c_str());
  if (!module)
    error = path.u8string() + ": LoadLibrary error " + std::to_string(GetLastError());
  return reinterpret_cast<void *>(module);
#else
  void *handle = dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
  if (!handle) {
    const char *detail = dlerror();
    error = detail ? detail : path.u8string() + ": dlopen failed";
  }
  return handle;
#endif
}

void *find_symbol(void *library, const char *name) {
#if defined(_WIN32)
  return reinterpret_cast<void *>(GetProcAddress(reinterpret_cast<HMODULE>(library), name));
#else
  return dlsym(library, name);
#endif
}

bool bind(void *library, SherpaApi &api, std::string &error) {
#define MSIME_SHERPA_BIND(name)                                                                     \
  api.name = reinterpret_cast<decltype(api.name)>(find_symbol(library, #name));                   \
  if (!api.name) {                                                                                \
    error = std::string("sherpa-onnx runtime lacks ") + #name;                                    \
    return false;                                                                                 \
  }
  MSIME_SHERPA_FUNCTIONS(MSIME_SHERPA_BIND)
#undef MSIME_SHERPA_BIND
  return true;
}

// The library is never unloaded: recognizers cached below point into it, and an ONNX Runtime unload is not something upstream supports.
const SherpaApi *load_runtime() {
  std::lock_guard<std::mutex> lock(runtime_mutex);
  if (runtime_loaded)
    return &runtime_api;
  if (runtime_tried)
    return nullptr;
  runtime_tried = true;
  auto candidates = library_candidates();
  candidates.emplace_back(kLibraryName);
  std::string errors;
  for (const auto &candidate : candidates) {
    if (candidate.has_parent_path()) {
      std::error_code missing;
      if (!fs::exists(candidate, missing))
        continue;
    }
    std::string error;
    void *library = open_library(candidate, error);
    if (!library) {
      errors += error + "; ";
      continue;
    }
    SherpaApi api;
    if (!bind(library, api, error)) {
      errors += error + "; ";
      continue;
    }
    runtime_api = api;
    runtime_loaded = true;
    runtime_error.clear();
    return &runtime_api;
  }
  runtime_error = errors.empty() ? std::string("sherpa-onnx runtime not found") : errors;
  return nullptr;
}

const SherpaApi &require_runtime() {
  if (const auto *api = load_runtime())
    return *api;
  throw VoiceError("Local speech recognition runtime is not installed: " + sherpa_runtime_error());
}

// ---- model manifest ----

enum class ModelKind { OnlineTransducer, OfflineSenseVoice, OfflineFunAsrNano };

bool model_path_inside(const fs::path &directory, const fs::path &candidate) {
  std::error_code error;
  const auto root = fs::canonical(directory, error);
  if (error)
    return false;
  const auto resolved = fs::canonical(candidate, error);
  if (error)
    return false;
  const auto relative = resolved.lexically_relative(root);
  if (relative.empty() || relative == ".")
    return false;
  for (const auto &part : relative) {
    if (part == ".." || part == ".")
      return false;
  }
  return true;
}

struct ModelDescription {
  ModelKind kind{};
  fs::path directory;
  std::map<std::string, std::string> files;
  std::string modeling_unit;
  std::string hotwords;

  std::string file(const std::string &key) const {
    const auto found = files.find(key);
    if (found == files.end())
      throw VoiceError("Local model manifest names no " + key + " file");
    const auto path = directory / fs::u8path(found->second);
    std::error_code error;
    if (!fs::exists(path, error) || !model_path_inside(directory, path))
      throw VoiceError("Local model is missing " + found->second);
    return fs::canonical(path, error).u8string();
  }
  std::string optional_file(const std::string &key) const {
    return files.count(key) ? file(key) : std::string();
  }
};

ModelDescription read_model(const std::string &directory) {
  ModelDescription model;
  model.directory = fs::u8path(directory);
  std::ifstream input(model.directory / fs::u8path(std::string(local_model_manifest)), std::ios::binary);
  if (!input)
    throw VoiceError("Not an installed local speech model");
  std::array<char, 8192> buffer{};
  std::string payload;
  while (input) {
    input.read(buffer.data(), static_cast<std::streamsize>(buffer.size()));
    const auto count = input.gcount();
    if (count <= 0) continue;
    const auto bytes = static_cast<size_t>(count);
    if (payload.size() > kMaxManifestBytes - bytes)
      throw VoiceError("Local speech model manifest is too large");
    payload.append(buffer.data(), bytes);
  }
  if (!input.eof())
    throw VoiceError("Local speech model manifest could not be read");
  try {
    const auto manifest = nlohmann::json::parse(payload);
    const auto kind = manifest.at("kind").get<std::string>();
    if (kind == "online_transducer")
      model.kind = ModelKind::OnlineTransducer;
    else if (kind == "offline_sense_voice")
      model.kind = ModelKind::OfflineSenseVoice;
    else if (kind == "offline_funasr_nano")
      model.kind = ModelKind::OfflineFunAsrNano;
    else
      throw VoiceError("Unsupported local speech model kind " + kind);
    for (const auto &[key, value] : manifest.at("files").items())
      model.files[key] = value.get<std::string>();
    model.modeling_unit = manifest.value("modeling_unit", std::string());
    model.hotwords = manifest.value("hotwords", std::string());
  } catch (const nlohmann::json::exception &) {
    throw VoiceError("Local speech model manifest is malformed");
  }
  return model;
}

int thread_count(int requested) {
  if (requested > 0)
    return requested;
  const unsigned hardware = std::max(1u, std::thread::hardware_concurrency());
  return static_cast<int>(std::clamp(hardware / 2u, 1u, 4u));
}

std::string lower(std::string_view text) {
  std::string out(text);
  std::transform(out.begin(), out.end(), out.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
  return out;
}

// SenseVoice detects the language itself and handles Mandarin with English words best that way, so only the languages it would not otherwise guess reliably are pinned.
std::string sense_voice_language(std::string_view tag) {
  const auto value = lower(tag);
  if (value.rfind("yue", 0) == 0 || value == "zh-hk" || value == "zh-mo")
    return "yue";
  if (value.rfind("ja", 0) == 0)
    return "ja";
  if (value.rfind("ko", 0) == 0)
    return "ko";
  return "auto";
}

// ---- hotwords ----

std::set<std::string> read_token_set(const std::string &tokens_path) {
  std::set<std::string> tokens;
  std::ifstream input(fs::u8path(tokens_path), std::ios::binary);
  std::string line;
  while (std::getline(input, line)) {
    const auto space = line.find(' ');
    tokens.insert(line.substr(0, space));
  }
  return tokens;
}

std::vector<std::string> utf8_characters(std::string_view text) {
  std::vector<std::string> characters;
  for (std::size_t i = 0; i < text.size();) {
    const auto lead = static_cast<unsigned char>(text[i]);
    const std::size_t width = lead < 0x80 ? 1 : (lead >> 5) == 0x6 ? 2 : (lead >> 4) == 0xE ? 3 : (lead >> 3) == 0x1E ? 4 : 1;
    characters.emplace_back(text.substr(i, width));
    i += width;
  }
  return characters;
}

// sherpa-onnx drops every hotword of a stream when a single one fails to encode, so a word with a character the model has no token for is left out here rather than taking the others with it. ASCII goes through the BPE model, which can spell anything.
std::string transducer_hotwords(const std::vector<std::string> &words, const std::set<std::string> &tokens) {
  std::string joined;
  std::size_t kept = 0;
  for (const auto &word : words) {
    if (kept == 200)
      break;
    std::string clean;
    bool usable = true;
    bool previous_space = true;
    for (const auto &character : utf8_characters(word)) {
      const auto byte = static_cast<unsigned char>(character[0]);
      if (character.size() == 1) {
        if (std::isspace(byte) || character == "/") {
          if (!previous_space)
            clean += ' ';
          previous_space = true;
          continue;
        }
        if (!std::isalnum(byte) && character != "'" && character != "-") {
          usable = false;
          break;
        }
      } else if (!tokens.count(character)) {
        usable = false;
        break;
      }
      clean += character;
      previous_space = false;
    }
    while (!clean.empty() && clean.back() == ' ')
      clean.pop_back();
    if (!usable || clean.empty())
      continue;
    joined += clean;
    joined += '\n';
    ++kept;
  }
  return joined;
}

std::string funasr_hotwords(const std::vector<std::string> &words) {
  std::string joined;
  std::size_t kept = 0;
  for (const auto &word : words) {
    // The prompt shares the decoder's 512-token context with the audio; a long list crowds out the speech.
    if (kept == 30)
      break;
    if (word.empty() || word.find(',') != std::string::npos)
      continue;
    if (!joined.empty())
      joined += ',';
    joined += word;
    ++kept;
  }
  return joined;
}

// ---- recognizer cache ----

struct LoadedRecognizer {
  ModelKind kind{};
  const SherpaApi *api = nullptr;
  const SherpaOnnxOnlineRecognizer *online = nullptr;
  const SherpaOnnxOfflineRecognizer *offline = nullptr;
  std::string vad_model;
  std::set<std::string> tokens;
  bool native_hotwords = false;
  // Atomic: a session's destructor stamps it without cache_mutex, which acquire() may hold through a model load.
  std::atomic<std::chrono::steady_clock::rep> last_used_ticks{std::chrono::steady_clock::now().time_since_epoch().count()};

  ~LoadedRecognizer() {
    if (online)
      api->SherpaOnnxDestroyOnlineRecognizer(online);
    if (offline)
      api->SherpaOnnxDestroyOfflineRecognizer(offline);
  }
};

std::mutex cache_mutex;
// Keyed by everything baked into the recognizer at creation. FunASR-nano takes its hotwords there, so a dictionary edit costs it one reload; the transducer takes them per stream and never reloads for them.
using RecognizerCache = std::map<std::tuple<std::string, std::string, std::string, int>, std::shared_ptr<LoadedRecognizer>>;
// Deliberately never destroyed. At exit the runtime library's own statics (ONNX Runtime's thread pools and mutexes) may already be gone, and destroying a recognizer after that aborts the process on a dead mutex; the OS reclaims the memory either way.
RecognizerCache &cache = *new RecognizerCache();

std::shared_ptr<LoadedRecognizer> create_recognizer(const SherpaApi &api, const ModelDescription &model,
                                                    const LocalAsrOptions &options, const std::string &nano_hotwords) {
  auto loaded = std::make_shared<LoadedRecognizer>();
  loaded->api = &api;
  loaded->kind = model.kind;
  const int threads = thread_count(options.threads);
  if (model.kind == ModelKind::OnlineTransducer) {
    const auto encoder = model.file("encoder");
    const auto decoder = model.file("decoder");
    const auto joiner = model.file("joiner");
    const auto tokens = model.file("tokens");
    const auto bpe_vocab = model.optional_file("bpe_vocab");
    loaded->native_hotwords = model.hotwords == "native" && !bpe_vocab.empty() && !model.modeling_unit.empty();
    SherpaOnnxOnlineRecognizerConfig config{};
    config.feat_config.sample_rate = kSampleRate;
    config.feat_config.feature_dim = 80;
    config.model_config.transducer.encoder = encoder.c_str();
    config.model_config.transducer.decoder = decoder.c_str();
    config.model_config.transducer.joiner = joiner.c_str();
    config.model_config.tokens = tokens.c_str();
    config.model_config.num_threads = threads;
    config.model_config.provider = "cpu";
    if (loaded->native_hotwords) {
      config.model_config.modeling_unit = model.modeling_unit.c_str();
      config.model_config.bpe_vocab = bpe_vocab.c_str();
    }
    // Per-stream hotwords are only honoured by modified beam search.
    config.decoding_method = loaded->native_hotwords ? "modified_beam_search" : "greedy_search";
    config.max_active_paths = 4;
    config.hotwords_score = 2.0f;
    config.enable_endpoint = 1;
    config.rule1_min_trailing_silence = 2.4f;
    config.rule2_min_trailing_silence = 1.0f;
    config.rule3_min_utterance_length = 20.0f;
    loaded->online = api.SherpaOnnxCreateOnlineRecognizer(&config);
    if (!loaded->online)
      throw VoiceError("Local speech model could not be loaded");
    if (loaded->native_hotwords)
      loaded->tokens = read_token_set(tokens);
    return loaded;
  }
  loaded->vad_model = model.file("vad");
  SherpaOnnxOfflineRecognizerConfig config{};
  config.feat_config.sample_rate = kSampleRate;
  config.feat_config.feature_dim = 80;
  config.model_config.num_threads = threads;
  config.model_config.provider = "cpu";
  config.decoding_method = "greedy_search";
  std::string tokens, sense_voice, language, adaptor, llm, embedding, tokenizer;
  if (model.kind == ModelKind::OfflineSenseVoice) {
    tokens = model.file("tokens");
    sense_voice = model.file("model");
    language = sense_voice_language(options.language);
    config.model_config.tokens = tokens.c_str();
    config.model_config.sense_voice.model = sense_voice.c_str();
    config.model_config.sense_voice.language = language.c_str();
    config.model_config.sense_voice.use_itn = 1;
  } else {
    adaptor = model.file("encoder_adaptor");
    llm = model.file("llm");
    embedding = model.file("embedding");
    tokenizer = model.file("tokenizer");
    config.model_config.funasr_nano.encoder_adaptor = adaptor.c_str();
    config.model_config.funasr_nano.llm = llm.c_str();
    config.model_config.funasr_nano.embedding = embedding.c_str();
    config.model_config.funasr_nano.tokenizer = tokenizer.c_str();
    config.model_config.funasr_nano.itn = 1;
    config.model_config.funasr_nano.hotwords = nano_hotwords.c_str();
    loaded->native_hotwords = true;
  }
  loaded->offline = api.SherpaOnnxCreateOfflineRecognizer(&config);
  if (!loaded->offline)
    throw VoiceError("Local speech model could not be loaded");
  return loaded;
}

std::shared_ptr<LoadedRecognizer> acquire(const SherpaApi &api, const ModelDescription &model,
                                          const LocalAsrOptions &options) {
  const auto nano_hotwords = model.kind == ModelKind::OfflineFunAsrNano ? funasr_hotwords(options.hotwords) : std::string();
  const auto language = model.kind == ModelKind::OfflineSenseVoice ? sense_voice_language(options.language) : std::string();
  const auto key = std::make_tuple(model.directory.u8string(), language, nano_hotwords, thread_count(options.threads));
  std::lock_guard<std::mutex> lock(cache_mutex);
  auto &slot = cache[key];
  if (!slot) {
    // Only one model at a time: two resident models would double a footprint that is already the largest thing the host holds.
    for (auto it = cache.begin(); it != cache.end();) {
      if (it->second && it->second.use_count() == 1 && it->first != key)
        it = cache.erase(it);
      else
        ++it;
    }
    try {
      cache[key] = create_recognizer(api, model, options, nano_hotwords);
    } catch (...) {
      cache.erase(key);
      throw;
    }
  }
  auto &entry = cache[key];
  entry->last_used_ticks.store(std::chrono::steady_clock::now().time_since_epoch().count(), std::memory_order_relaxed);
  return entry;
}

std::string result_text(const LoadedRecognizer &recognizer, const SherpaOnnxOnlineStream *stream) {
  const auto *result = recognizer.api->SherpaOnnxGetOnlineStreamResult(recognizer.online, stream);
  std::string text = result && result->text ? result->text : "";
  if (result)
    recognizer.api->SherpaOnnxDestroyOnlineRecognizerResult(result);
  return text;
}

bool is_cjk_punctuation(std::string_view character) {
  static const std::set<std::string_view> marks = {"，", "。", "？", "！", "、", "：", "；", "“", "”", "‘", "’", "（", "）", "《", "》", "…"};
  return marks.count(character) > 0;
}

bool is_single_capital(const std::vector<std::string> &characters, std::size_t index) {
  if (index >= characters.size() || characters[index].size() != 1)
    return false;
  const auto c = static_cast<unsigned char>(characters[index][0]);
  if (!std::isupper(c))
    return false;
  const bool left_ok = index == 0 || characters[index - 1] == " " || characters[index - 1].size() > 1;
  const bool right_ok = index + 1 >= characters.size() || characters[index + 1] == " " || characters[index + 1].size() > 1;
  return left_ok && right_ok;
}

std::string join_segments(const std::vector<std::string> &segments) {
  std::string joined;
  for (const auto &segment : segments) {
    if (segment.empty())
      continue;
    if (!joined.empty() && static_cast<unsigned char>(joined.back()) < 0x80 && std::isalnum(static_cast<unsigned char>(joined.back())) &&
        static_cast<unsigned char>(segment.front()) < 0x80 && std::isalnum(static_cast<unsigned char>(segment.front())))
      joined += ' ';
    joined += segment;
  }
  return joined;
}

} // namespace

// ---- session ----

struct LocalAsrSession::Impl {
  std::shared_ptr<LoadedRecognizer> recognizer;
  PartialCallback on_partial;
  std::shared_ptr<std::atomic_bool> cancelled;
  const SherpaOnnxOnlineStream *online_stream = nullptr;
  const SherpaOnnxVoiceActivityDetector *vad = nullptr;
  std::vector<std::string> segments;
  std::string current;
  std::string last_partial;
  bool finished = false;

  ~Impl() {
    if (online_stream)
      recognizer->api->SherpaOnnxDestroyOnlineStream(online_stream);
    if (vad)
      recognizer->api->SherpaOnnxDestroyVoiceActivityDetector(vad);
    if (recognizer)
      recognizer->last_used_ticks.store(std::chrono::steady_clock::now().time_since_epoch().count(), std::memory_order_relaxed);
  }

  void check_cancelled() const {
    if (cancelled && cancelled->load())
      throw VoiceError("Voice request cancelled");
  }

  std::string transcript() const {
    auto parts = segments;
    if (!current.empty())
      parts.push_back(current);
    return tidy_local_transcript(join_segments(parts));
  }

  void report() {
    if (!on_partial)
      return;
    auto text = transcript();
    if (text == last_partial)
      return;
    last_partial = text;
    on_partial(text);
  }

  void decode_online() {
    const auto &api = *recognizer->api;
    while (api.SherpaOnnxIsOnlineStreamReady(recognizer->online, online_stream)) {
      check_cancelled();
      api.SherpaOnnxDecodeOnlineStream(recognizer->online, online_stream);
    }
    current = result_text(*recognizer, online_stream);
    if (api.SherpaOnnxOnlineStreamIsEndpoint(recognizer->online, online_stream)) {
      if (!current.empty())
        segments.push_back(current);
      current.clear();
      api.SherpaOnnxOnlineStreamReset(recognizer->online, online_stream);
    }
  }

  void decode_segment(const float *samples, int32_t count) {
    check_cancelled();
    const auto &api = *recognizer->api;
    const auto *stream = api.SherpaOnnxCreateOfflineStream(recognizer->offline);
    if (!stream)
      throw VoiceError("Local speech recognition failed");
    api.SherpaOnnxAcceptWaveformOffline(stream, kSampleRate, samples, count);
    api.SherpaOnnxDecodeOfflineStream(recognizer->offline, stream);
    const auto *result = api.SherpaOnnxGetOfflineStreamResult(stream);
    if (result && result->text && *result->text)
      segments.emplace_back(result->text);
    if (result)
      api.SherpaOnnxDestroyOfflineRecognizerResult(result);
    api.SherpaOnnxDestroyOfflineStream(stream);
  }

  void drain_vad() {
    const auto &api = *recognizer->api;
    while (!api.SherpaOnnxVoiceActivityDetectorEmpty(vad)) {
      const auto *segment = api.SherpaOnnxVoiceActivityDetectorFront(vad);
      if (segment && segment->n > 0) {
        try {
          decode_segment(segment->samples, segment->n);
        } catch (...) {
          api.SherpaOnnxDestroySpeechSegment(segment);
          throw;
        }
      }
      if (segment)
        api.SherpaOnnxDestroySpeechSegment(segment);
      api.SherpaOnnxVoiceActivityDetectorPop(vad);
    }
  }
};

LocalAsrSession::LocalAsrSession(const LocalAsrOptions &options, PartialCallback on_partial,
                                 std::shared_ptr<std::atomic_bool> cancelled)
    : impl_(std::make_unique<Impl>()) {
  const auto &api = require_runtime();
  const auto model = read_model(options.model_dir);
  impl_->on_partial = std::move(on_partial);
  impl_->cancelled = std::move(cancelled);
  impl_->check_cancelled();
  impl_->recognizer = acquire(api, model, options);
  auto &recognizer = *impl_->recognizer;
  if (recognizer.kind == ModelKind::OnlineTransducer) {
    const auto hotwords = recognizer.native_hotwords ? transducer_hotwords(options.hotwords, recognizer.tokens) : std::string();
    impl_->online_stream = hotwords.empty()
                               ? api.SherpaOnnxCreateOnlineStream(recognizer.online)
                               : api.SherpaOnnxCreateOnlineStreamWithHotwords(recognizer.online, hotwords.c_str());
    if (!impl_->online_stream)
      throw VoiceError("Local speech recognition failed");
    return;
  }
  SherpaOnnxVadModelConfig vad{};
  vad.silero_vad.model = recognizer.vad_model.c_str();
  vad.silero_vad.threshold = 0.5f;
  vad.silero_vad.min_silence_duration = 0.5f;
  vad.silero_vad.min_speech_duration = 0.25f;
  vad.silero_vad.window_size = kVadWindow;
  // FunASR-nano shares a 512-token context between audio and text; past roughly 25 seconds it returns nothing at all.
  vad.silero_vad.max_speech_duration = recognizer.kind == ModelKind::OfflineFunAsrNano ? 20.0f : 28.0f;
  vad.sample_rate = kSampleRate;
  vad.num_threads = 1;
  vad.provider = "cpu";
  impl_->vad = api.SherpaOnnxCreateVoiceActivityDetector(&vad, 60.0f);
  if (!impl_->vad)
    throw VoiceError("Local voice activity detector could not be loaded");
}

LocalAsrSession::~LocalAsrSession() = default;

void LocalAsrSession::accept(const float *samples, std::size_t count) {
  if (impl_->finished)
    throw VoiceError("Local speech session already finished");
  impl_->check_cancelled();
  if (count == 0)
    return;
  const auto &api = *impl_->recognizer->api;
  // The C API takes an int32 count; feed long buffers in slices.
  constexpr std::size_t slice = kSampleRate * 10;
  for (std::size_t offset = 0; offset < count; offset += slice) {
    const auto n = static_cast<int32_t>(std::min(slice, count - offset));
    if (impl_->online_stream) {
      api.SherpaOnnxOnlineStreamAcceptWaveform(impl_->online_stream, kSampleRate, samples + offset, n);
      impl_->decode_online();
    } else {
      // The detector judges each call as a whole (speech if any window in it is speech), so it is fed one window at a time; a long buffer in one call would merge every pause into a single segment.
      for (int32_t at = 0; at < n; at += kVadWindow) {
        api.SherpaOnnxVoiceActivityDetectorAcceptWaveform(impl_->vad, samples + offset + at, std::min(kVadWindow, n - at));
        impl_->drain_vad();
      }
    }
  }
  impl_->report();
}

std::string LocalAsrSession::finish() {
  if (impl_->finished)
    throw VoiceError("Local speech session already finished");
  impl_->finished = true;
  impl_->check_cancelled();
  const auto &api = *impl_->recognizer->api;
  if (impl_->online_stream) {
    // Trailing silence lets the last chunk through the encoder's look-ahead before input ends.
    const std::vector<float> tail(kSampleRate * 6 / 10, 0.0f);
    api.SherpaOnnxOnlineStreamAcceptWaveform(impl_->online_stream, kSampleRate, tail.data(), static_cast<int32_t>(tail.size()));
    api.SherpaOnnxOnlineStreamInputFinished(impl_->online_stream);
    impl_->decode_online();
  } else {
    api.SherpaOnnxVoiceActivityDetectorFlush(impl_->vad);
    impl_->drain_vad();
  }
  return impl_->transcript();
}

void set_sherpa_library_path(std::string path) {
  std::lock_guard<std::mutex> lock(runtime_mutex);
  if (runtime_loaded)
    return;
  configured_library = std::move(path);
  runtime_tried = false;
}

bool sherpa_runtime_available() { return load_runtime() != nullptr; }

std::string sherpa_runtime_error() {
  std::lock_guard<std::mutex> lock(runtime_mutex);
  return runtime_error;
}

bool is_local_model_dir(std::string_view path) {
  if (path.empty())
    return false;
  std::error_code error;
  const auto directory = fs::u8path(std::string(path));
  const auto manifest = directory / fs::u8path(std::string(local_model_manifest));
  return fs::is_directory(directory, error) && fs::is_regular_file(manifest, error) &&
         fs::file_size(manifest, error) <= kMaxManifestBytes && !error;
}

std::string recognize_local_model(const std::vector<float> &samples, const LocalAsrOptions &options,
                                  const std::shared_ptr<std::atomic_bool> &cancelled) {
  LocalAsrSession session(options, nullptr, cancelled);
  session.accept(samples.data(), samples.size());
  return session.finish();
}

std::size_t release_idle_local_models(std::chrono::steady_clock::duration idle) {
  const auto now = std::chrono::steady_clock::now();
  std::lock_guard<std::mutex> lock(cache_mutex);
  std::size_t released = 0;
  for (auto it = cache.begin(); it != cache.end();) {
    if (it->second && it->second.use_count() == 1 &&
        now.time_since_epoch().count() - it->second->last_used_ticks.load(std::memory_order_relaxed) >= std::chrono::steady_clock::duration(idle).count()) {
      it = cache.erase(it);
      ++released;
    } else {
      ++it;
    }
  }
  return released;
}

std::size_t release_local_models() { return release_idle_local_models(std::chrono::steady_clock::duration::zero()); }

std::string tidy_local_transcript(std::string_view text) {
  const auto characters = utf8_characters(text);
  std::string out;
  for (std::size_t i = 0; i < characters.size(); ++i) {
    const auto &character = characters[i];
    if (character == " ") {
      const bool after_mark = i > 0 && is_cjk_punctuation(characters[i - 1]);
      const bool before_mark = i + 1 < characters.size() && is_cjk_punctuation(characters[i + 1]);
      const bool inside_initialism = i > 0 && is_single_capital(characters, i - 1) && is_single_capital(characters, i + 1);
      const bool leading = out.empty();
      const bool repeated = !out.empty() && out.back() == ' ';
      if (after_mark || before_mark || inside_initialism || leading || repeated)
        continue;
    }
    out += character;
  }
  while (!out.empty() && out.back() == ' ')
    out.pop_back();
  return out;
}

} // namespace msime::voice
