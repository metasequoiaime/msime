#include "LocalAsr.h"

#include <nlohmann/json.hpp>
#include <sherpa-onnx/c-api.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <cctype>
#include <cerrno>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <iterator>
#include <map>
#include <mutex>
#include <set>
#include <sstream>
#include <thread>
#include <tuple>
#include <utility>
#include <vector>

#if defined(_WIN32)
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#else
#include <dlfcn.h>
#if !defined(_WIN32)
#include <fcntl.h>
#include <sys/stat.h>
#endif
#include <unistd.h>
#endif
#if defined(__APPLE__)
#include <mach-o/dyld.h>
#endif

namespace msime::voice {
namespace {

namespace fs = std::filesystem;

constexpr int kSampleRate = 16000;
constexpr int32_t kVadWindow = 512;
// 段首往前补（local_vad_segment_start）：10 ms 一帧判断能量，最多往前 3 秒，低能量间隙不超过 0.25 秒，找到的起音前再留 0.3 秒。
constexpr std::int64_t kOnsetFrame = kSampleRate / 100;
constexpr std::int64_t kOnsetLookback = kSampleRate * 3;
constexpr std::int64_t kOnsetGap = kSampleRate / 4;
constexpr std::int64_t kOnsetPreRoll = kSampleRate * 3 / 10;
// 帧能量（RMS）高出噪声底 4 倍（12 dB）算作语音；噪声底不低于 -100 dBFS，数字静音之后的第一个非零采样不至于都算成语音。
constexpr float kOnsetRise = 4.0f;
constexpr float kOnsetFloor = 1e-5f;
// VAD 之前的录音最多比一段语音的上限多留这么久，够往前补 3 秒加上段尾那 0.5 秒静音。
constexpr std::int64_t kHistorySlack = kSampleRate * 5;
constexpr size_t kMaxManifestBytes = 256 * 1024;
constexpr size_t kMaxTokenBytes = 16 * 1024 * 1024;

#if defined(_WIN32)
std::string read_windows_regular_file(const fs::path &path, size_t max_bytes) {
  const HANDLE handle = CreateFileW(path.c_str(), GENERIC_READ,
                                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                                    nullptr, OPEN_EXISTING,
                                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                                    nullptr);
  if (handle == INVALID_HANDLE_VALUE)
    throw VoiceError("Not an installed local speech model");
  struct CloseOnExit {
    HANDLE handle;
    ~CloseOnExit() { CloseHandle(handle); }
  } close_on_exit{handle};
  FILE_ATTRIBUTE_TAG_INFO attributes{};
  FILE_STANDARD_INFO standard{};
  if (GetFileType(handle) != FILE_TYPE_DISK ||
      !GetFileInformationByHandleEx(handle, FileAttributeTagInfo, &attributes,
                                     sizeof(attributes)) ||
      !GetFileInformationByHandleEx(handle, FileStandardInfo, &standard,
                                     sizeof(standard)) ||
      (attributes.FileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0 ||
      ((attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 &&
       IsReparseTagNameSurrogate(attributes.ReparseTag)) ||
      standard.NumberOfLinks != 1)
    throw VoiceError("Not an installed local speech model");
  LARGE_INTEGER size{};
  if (!GetFileSizeEx(handle, &size) || size.QuadPart < 0 ||
      static_cast<unsigned long long>(size.QuadPart) > max_bytes)
    throw VoiceError("Local speech model file is too large");
  const auto byte_count = static_cast<size_t>(size.QuadPart);
  std::string payload(byte_count, '\0');
  size_t offset = 0;
  while (offset < payload.size()) {
    const DWORD request = static_cast<DWORD>((std::min)(payload.size() - offset, sizeof(std::array<char, 8192>)));
    DWORD count = 0;
    if (!ReadFile(handle, payload.data() + offset, request, &count, nullptr) || count == 0)
      throw VoiceError("Local speech model file could not be read");
    offset += count;
  }
  return payload;
}
#endif

nlohmann::json read_manifest(const fs::path &directory) {
#if defined(_WIN32)
  const auto payload = read_windows_regular_file(
      directory / fs::u8path(std::string(local_model_manifest)), kMaxManifestBytes);
#else
  const auto path = directory / fs::u8path(std::string(local_model_manifest));
  const int descriptor = ::open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0)
    throw VoiceError("Not an installed local speech model");
  struct CloseOnExit {
    int descriptor;
    ~CloseOnExit() { ::close(descriptor); }
  } close_on_exit{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) ||
      metadata.st_nlink != 1)
    throw VoiceError("Not an installed local speech model");
  std::array<char, 8192> buffer{};
  std::string payload;
  for (;;) {
    const ssize_t count = ::read(descriptor, buffer.data(), buffer.size());
    if (count > 0) {
      const auto bytes = static_cast<std::size_t>(count);
      if (payload.size() > kMaxManifestBytes - bytes)
        throw VoiceError("Local speech model manifest is too large");
      payload.append(buffer.data(), bytes);
      continue;
    }
    if (count == 0) break;
    if (errno == EINTR) continue;
    throw VoiceError("Local speech model manifest could not be read");
  }
#endif
  auto manifest = nlohmann::json::parse(payload, nullptr, false);
  if (manifest.is_discarded())
    throw VoiceError("Local speech model manifest is malformed");
  return manifest;
}

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
  X(SherpaOnnxCreateOfflineStreamWithHotwords)                                                    \
  X(SherpaOnnxDestroyOfflineStream)                                                               \
  X(SherpaOnnxAcceptWaveformOffline)                                                              \
  X(SherpaOnnxDecodeOfflineStream)                                                                \
  X(SherpaOnnxGetOfflineStreamResult)                                                             \
  X(SherpaOnnxDestroyOfflineRecognizerResult)                                                     \
  X(SherpaOnnxCreateVoiceActivityDetector)                                                        \
  X(SherpaOnnxDestroyVoiceActivityDetector)                                                       \
  X(SherpaOnnxVoiceActivityDetectorAcceptWaveform)                                                \
  X(SherpaOnnxVoiceActivityDetectorEmpty)                                                         \
  X(SherpaOnnxVoiceActivityDetectorDetected)                                                      \
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
  candidates.reserve(5);
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
  // 相邻依赖搜索要求标准 Windows 绝对路径；正斜杠可能使它拾取系统的另一个 ONNX。
  HMODULE module = nullptr;
  if (path.has_parent_path()) {
    std::error_code failure;
    auto location = fs::absolute(path, failure);
    if (failure) {
      error = path.u8string() + ": cannot resolve runtime path";
      return nullptr;
    }
    location.make_preferred();
    module = LoadLibraryExW(location.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
  } else {
    module = LoadLibraryW(path.c_str());
  }
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

enum class ModelKind { OnlineTransducer, OfflineTransducer, OfflineSenseVoice, OfflineFunAsrNano };

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

// 与 Rust 安装器一致，路径最多经过一个系统别名。Android 的固定别名先于 Linux 分支处理：
// /data/user 属于 system，不能用要求父目录属于 root 的 Linux 规则判断。
// Linux 发行版可能链接 /home 或 /bin，因此只信任链接和不可被组、其他用户写入的父目录均属于 root 的链接。
bool trusted_model_path_link(const fs::path &path) {
#if defined(__ANDROID__)
  if (path != fs::path("/data/user/0"))
    return false;
  std::error_code error;
  const auto target = fs::read_symlink(path, error);
  return !error && (path.parent_path() / target).lexically_normal() == fs::path("/data/data");
#elif defined(__linux__)
  struct stat link {};
  if (::lstat(path.c_str(), &link) != 0 || !S_ISLNK(link.st_mode) || link.st_uid != 0)
    return false;
  struct stat parent {};
  const auto parent_path = path.parent_path();
  return ::stat(parent_path.c_str(), &parent) == 0 && S_ISDIR(parent.st_mode) && parent.st_uid == 0 &&
         (parent.st_mode & 022) == 0;
#elif defined(__APPLE__)
  const fs::path expected = path == fs::path("/var")   ? fs::path("/private/var")
                            : path == fs::path("/tmp") ? fs::path("/private/tmp")
                                                       : fs::path();
  if (expected.empty())
    return false;
  std::error_code error;
  const auto target = fs::read_symlink(path, error);
  if (error)
    return false;
  return (path.parent_path() / target).lexically_normal() == expected;
#else
  (void)path;
  return false;
#endif
}

// Check the path itself and every existing ancestor without following a link. The model path comes
// from preferences, so checking only the final directory would let a link in the middle redirect
// the recognizer to a different tree. The one trusted alias is never the final path component.
bool model_directory_has_real_ancestors(const fs::path &directory) {
  if (!directory.is_absolute())
    return false;
  std::vector<fs::path> ancestors;
  ancestors.reserve(
      static_cast<std::size_t>(std::distance(directory.begin(), directory.end())));
  for (fs::path current = directory; !current.empty(); current = current.parent_path()) {
    ancestors.push_back(current);
    if (current.parent_path() == current)
      break;
  }
  std::error_code error;
  bool saw_trusted_link = false;
  for (auto it = ancestors.rbegin(); it != ancestors.rend(); ++it) {
    const auto status = fs::symlink_status(*it, error);
    if (error)
      return false;
    if (fs::is_symlink(status)) {
      const bool last = std::next(it) == ancestors.rend();
      if (last || saw_trusted_link || !trusted_model_path_link(*it))
        return false;
      saw_trusted_link = true;
      continue;
    }
    if (!fs::is_directory(status))
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
  // 清单写明的 sherpa-onnx 模型类型。留空时 sherpa 为了读 encoder 元数据里的类型，会先多建一个完整的推理会话再丢掉，冷加载因此多读一遍 encoder；写了不认识的值只会退回这种探测。
  std::string model_type;

  std::string file(const std::string &key) const {
    const auto found = files.find(key);
    if (found == files.end())
      throw VoiceError("Local model manifest names no " + key + " file");
    const auto path = directory / fs::u8path(found->second);
    std::error_code error;
    const auto status = fs::symlink_status(path, error);
    if (error || !fs::is_regular_file(status) ||
        fs::hard_link_count(path, error) != 1 || error ||
        !model_path_inside(directory, path))
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
  try {
    const auto manifest = read_manifest(model.directory);
    const auto kind = manifest.at("kind").get<std::string>();
    if (kind == "online_transducer")
      model.kind = ModelKind::OnlineTransducer;
    else if (kind == "offline_transducer")
      model.kind = ModelKind::OfflineTransducer;
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
    model.model_type = manifest.value("model_type", std::string());
  } catch (const nlohmann::json::exception &) {
    throw VoiceError("Local speech model manifest is malformed");
  }
  return model;
}

} // namespace

int local_asr_thread_count(int requested) {
  if (requested > 0)
    return (std::min)(requested, 4);
  const unsigned hardware = std::max(1u, std::thread::hardware_concurrency());
  return static_cast<int>(std::clamp(hardware / 2u, 1u, 4u));
}

float local_asr_vad_threshold(int sensitivity_percent) {
  if (sensitivity_percent < 10 || sensitivity_percent > 90)
    return 0.5f;
  return static_cast<float>(100 - sensitivity_percent) / 100.0f;
}

namespace {

int thread_count(int requested) { return local_asr_thread_count(requested); }

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
#if defined(_WIN32)
  std::istringstream input(read_windows_regular_file(fs::u8path(tokens_path), kMaxTokenBytes));
  std::string line;
  while (std::getline(input, line)) {
    const auto space = line.find(' ');
    tokens.insert(line.substr(0, space));
  }
#else
  const int descriptor = ::open(fs::u8path(tokens_path).c_str(),
                                O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0) return tokens;
  struct CloseOnExit {
    int descriptor;
    ~CloseOnExit() { ::close(descriptor); }
  } close_on_exit{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) ||
      metadata.st_nlink != 1)
    return tokens;
  std::array<char, 8192> buffer{};
  std::string line;
  std::size_t total = 0;
  for (;;) {
    const ssize_t count = ::read(descriptor, buffer.data(), buffer.size());
    if (count > 0) {
      const auto bytes = static_cast<std::size_t>(count);
      if (total > kMaxTokenBytes - bytes) return {};
      total += bytes;
      for (ssize_t index = 0; index < count; ++index) {
        if (buffer[static_cast<std::size_t>(index)] == '\n') {
          const auto space = line.find(' ');
          tokens.insert(line.substr(0, space));
          line.clear();
        } else {
          line.push_back(buffer[static_cast<std::size_t>(index)]);
        }
      }
      continue;
    }
    if (count == 0) {
      if (!line.empty()) {
        const auto space = line.find(' ');
        tokens.insert(line.substr(0, space));
      }
      break;
    }
    if (errno == EINTR) continue;
    break;
  }
#endif
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

// SentencePiece 的词首标记。X-ASR 训练时汉字之间都隔着空格，于是它的 token 表里每个汉字都是带这个前缀的独立的词（`▁写`），一个不带前缀的汉字都没有。
constexpr std::string_view kWordStart = "\xE2\x96\x81";

// token 表里的汉字是否都写成独立的词。是的话，热词要按 `bpe` 编码、字与字之间隔空格；按清单原来写的 `cjkchar+bpe` 编码时，sherpa-onnx 找不到不带前缀的汉字，每个中文热词都编码失败。旧安装的清单仍写着 `cjkchar+bpe`，所以以 token 表为准，不重装也能用上热词。
bool cjk_spelled_as_words(const std::set<std::string> &tokens) {
  bool spelled = false;
  for (const auto &token : tokens) {
    if (token.size() == 3 && static_cast<unsigned char>(token[0]) >= 0xE4 && static_cast<unsigned char>(token[0]) <= 0xE9)
      return false;
    if (!spelled && token.size() == kWordStart.size() + 3 && token.compare(0, kWordStart.size(), kWordStart) == 0 &&
        static_cast<unsigned char>(token[kWordStart.size()]) >= 0xE4 && static_cast<unsigned char>(token[kWordStart.size()]) <= 0xE9)
      spelled = true;
  }
  return spelled;
}

// sherpa-onnx drops every hotword of a stream when a single one fails to encode, so a word with a character the model has no token for is left out here rather than taking the others with it. ASCII goes through the BPE model, which can spell anything.
std::string transducer_hotwords(const std::vector<std::string> &words, const std::set<std::string> &tokens, bool spaced_cjk) {
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
      } else if (spaced_cjk) {
        if (!tokens.count(std::string(kWordStart) + character)) {
          usable = false;
          break;
        }
        // 每个非 ASCII 字自成一个词，前后都隔开。
        if (!previous_space)
          clean += ' ';
        clean += character;
        clean += ' ';
        previous_space = true;
        continue;
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
  // token 表里的汉字是独立的词，热词按 `bpe` 编码、字间隔空格（cjk_spelled_as_words）。
  bool spaced_cjk = false;
  std::string modeling_unit;
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

// 原生热词要用的 token 表与实际的建模单位。
void use_hotword_tokens(LoadedRecognizer &loaded, const ModelDescription &model, const std::string &tokens_path) {
  loaded.tokens = read_token_set(tokens_path);
  loaded.spaced_cjk = cjk_spelled_as_words(loaded.tokens);
  loaded.modeling_unit = loaded.spaced_cjk ? "bpe" : model.modeling_unit;
}

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
    if (loaded->native_hotwords)
      use_hotword_tokens(*loaded, model, tokens);
    SherpaOnnxOnlineRecognizerConfig config{};
    config.feat_config.sample_rate = kSampleRate;
    config.feat_config.feature_dim = 80;
    config.model_config.transducer.encoder = encoder.c_str();
    config.model_config.transducer.decoder = decoder.c_str();
    config.model_config.transducer.joiner = joiner.c_str();
    config.model_config.tokens = tokens.c_str();
    config.model_config.num_threads = threads;
    config.model_config.provider = "cpu";
    if (!model.model_type.empty())
      config.model_config.model_type = model.model_type.c_str();
    if (loaded->native_hotwords) {
      config.model_config.modeling_unit = loaded->modeling_unit.c_str();
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
    return loaded;
  }
  loaded->vad_model = model.file("vad");
  SherpaOnnxOfflineRecognizerConfig config{};
  config.feat_config.sample_rate = kSampleRate;
  config.feat_config.feature_dim = 80;
  config.model_config.num_threads = threads;
  config.model_config.provider = "cpu";
  config.decoding_method = "greedy_search";
  if (!model.model_type.empty())
    config.model_config.model_type = model.model_type.c_str();
  std::string tokens, sense_voice, language, adaptor, llm, embedding, tokenizer;
  std::string encoder, decoder, joiner, bpe_vocab;
  if (model.kind == ModelKind::OfflineTransducer) {
    encoder = model.file("encoder");
    decoder = model.file("decoder");
    joiner = model.file("joiner");
    tokens = model.file("tokens");
    bpe_vocab = model.optional_file("bpe_vocab");
    loaded->native_hotwords = model.hotwords == "native" && !bpe_vocab.empty() && !model.modeling_unit.empty();
    if (loaded->native_hotwords)
      use_hotword_tokens(*loaded, model, tokens);
    config.model_config.transducer.encoder = encoder.c_str();
    config.model_config.transducer.decoder = decoder.c_str();
    config.model_config.transducer.joiner = joiner.c_str();
    config.model_config.tokens = tokens.c_str();
    if (loaded->native_hotwords) {
      config.model_config.modeling_unit = loaded->modeling_unit.c_str();
      config.model_config.bpe_vocab = bpe_vocab.c_str();
    }
    // 整段解码时 encoder 只跑一遍，beam 的代价只落在 decoder/joiner 上，所以不论有没有热词都用宽 beam。
    config.decoding_method = "modified_beam_search";
    config.max_active_paths = 16;
    config.hotwords_score = 2.0f;
  } else if (model.kind == ModelKind::OfflineSenseVoice) {
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

// 汉字（CJK 统一表意文字及其扩展区、兼容区）。
bool is_cjk_ideograph(std::string_view character) {
  if (character.size() < 3)
    return false;
  const auto byte = [&](std::size_t i) { return static_cast<char32_t>(static_cast<unsigned char>(character[i])); };
  const char32_t code = character.size() == 3
                            ? ((byte(0) & 0x0F) << 12) | ((byte(1) & 0x3F) << 6) | (byte(2) & 0x3F)
                            : ((byte(0) & 0x07) << 18) | ((byte(1) & 0x3F) << 12) | ((byte(2) & 0x3F) << 6) | (byte(3) & 0x3F);
  return (code >= 0x3400 && code <= 0x9FFF) || (code >= 0xF900 && code <= 0xFAFF) || (code >= 0x20000 && code <= 0x3FFFF);
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
  // 离线 transducer 每段新建一个流，热词随流传入，所以在会话开始时整理一次。
  std::string segment_hotwords;
  // 送进 VAD 的最近一段录音，给段首往前补（local_vad_segment_start）。history[0] 是会话里第 history_start 个采样；最多留 history_limit 个，sherpa-onnx 自己的缓冲只留到段首前 2 个窗口。
  std::vector<float> history;
  std::int64_t history_start = 0;
  std::size_t history_limit = 0;
  // 上一段的段尾。往前补不越过它，否则两段的识别结果会重复同一段音频里的字。
  std::int64_t decoded_end = 0;
  std::vector<float> padded;
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
    const auto *stream = segment_hotwords.empty()
                             ? api.SherpaOnnxCreateOfflineStream(recognizer->offline)
                             : api.SherpaOnnxCreateOfflineStreamWithHotwords(recognizer->offline, segment_hotwords.c_str());
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

  void remember(const float *samples, int32_t count) {
    history.insert(history.end(), samples, samples + count);
    // 多攒一秒再整块丢掉最旧的，免得每个窗口都搬一遍整段缓冲。
    if (history.size() > history_limit + kSampleRate) {
      const auto drop = history.size() - history_limit;
      history.erase(history.begin(), history.begin() + static_cast<std::ptrdiff_t>(drop));
      history_start += static_cast<std::int64_t>(drop);
    }
  }

  // VAD 的段首往前补上它漏掉的起音再解码，见 local_vad_segment_start。
  void decode_vad_segment(const SherpaOnnxSpeechSegment &segment) {
    const std::int64_t start = segment.start;
    const auto begin = local_vad_segment_start(history.data(), history.size(), history_start, start, decoded_end);
    decoded_end = start + segment.n;
    if (begin >= start) {
      decode_segment(segment.samples, segment.n);
      return;
    }
    padded.assign(history.begin() + static_cast<std::ptrdiff_t>(begin - history_start),
                  history.begin() + static_cast<std::ptrdiff_t>(start - history_start));
    padded.insert(padded.end(), segment.samples, segment.samples + segment.n);
    decode_segment(padded.data(), static_cast<int32_t>(padded.size()));
  }

  void drain_vad() {
    const auto &api = *recognizer->api;
    while (!api.SherpaOnnxVoiceActivityDetectorEmpty(vad)) {
      const auto *segment = api.SherpaOnnxVoiceActivityDetectorFront(vad);
      if (segment && segment->n > 0) {
        try {
          decode_vad_segment(*segment);
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
  if (!is_local_model_dir(options.model_dir))
    throw VoiceError("Not an installed local speech model");
  const auto &api = require_runtime();
  const auto model = read_model(options.model_dir);
  impl_->on_partial = std::move(on_partial);
  impl_->cancelled = std::move(cancelled);
  impl_->check_cancelled();
  impl_->recognizer = acquire(api, model, options);
  auto &recognizer = *impl_->recognizer;
  if (recognizer.kind == ModelKind::OnlineTransducer) {
    const auto hotwords = recognizer.native_hotwords ? transducer_hotwords(options.hotwords, recognizer.tokens, recognizer.spaced_cjk) : std::string();
    impl_->online_stream = hotwords.empty()
                               ? api.SherpaOnnxCreateOnlineStream(recognizer.online)
                               : api.SherpaOnnxCreateOnlineStreamWithHotwords(recognizer.online, hotwords.c_str());
    if (!impl_->online_stream)
      throw VoiceError("Local speech recognition failed");
    return;
  }
  if (recognizer.kind == ModelKind::OfflineTransducer && recognizer.native_hotwords)
    impl_->segment_hotwords = transducer_hotwords(options.hotwords, recognizer.tokens, recognizer.spaced_cjk);
  SherpaOnnxVadModelConfig vad{};
  vad.silero_vad.model = recognizer.vad_model.c_str();
  vad.silero_vad.threshold = local_asr_vad_threshold(options.vad_sensitivity_percent);
  vad.silero_vad.min_silence_duration = 0.5f;
  vad.silero_vad.min_speech_duration = 0.25f;
  vad.silero_vad.window_size = kVadWindow;
  // FunASR-nano shares a 512-token context between audio and text; past roughly 25 seconds it returns nothing at all.
  vad.silero_vad.max_speech_duration = recognizer.kind == ModelKind::OfflineFunAsrNano ? 20.0f : 28.0f;
  impl_->history_limit = static_cast<std::size_t>(vad.silero_vad.max_speech_duration) * kSampleRate + static_cast<std::size_t>(kHistorySlack);
  vad.sample_rate = kSampleRate;
  vad.num_threads = 1;
  vad.provider = "cpu";
  impl_->vad = api.SherpaOnnxCreateVoiceActivityDetector(&vad, 60.0f);
  if (!impl_->vad)
    throw VoiceError("Local voice activity detector could not be loaded");
}

LocalAsrSession::~LocalAsrSession() = default;

bool LocalAsrSession::speech_detected() const {
  return impl_->vad && impl_->recognizer->api->SherpaOnnxVoiceActivityDetectorDetected(impl_->vad) != 0;
}

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
        const auto window = std::min(kVadWindow, n - at);
        impl_->remember(samples + offset + at, window);
        api.SherpaOnnxVoiceActivityDetectorAcceptWaveform(impl_->vad, samples + offset + at, window);
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
    // 尾部补静音，让最后一块音频越过 encoder 的前瞻再结束输入。X-ASR 480 ms 流式模型补 0.6 秒时，sherpa-onnx 自带的 test_wavs 四条里有两条丢掉最后一两个字（「是星期三」只出「是星期」），补到 1.2 秒四条都完整；松手即结束的按住说话没有句末停顿，更依赖这一段。
    const std::vector<float> tail(kSampleRate * 12 / 10, 0.0f);
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
  if (!model_directory_has_real_ancestors(directory))
    return false;
  const auto manifest = directory / fs::u8path(std::string(local_model_manifest));
  const auto directory_status = fs::symlink_status(directory, error);
  if (error || !fs::is_directory(directory_status))
    return false;
  const auto manifest_status = fs::symlink_status(manifest, error);
  if (error || !fs::is_regular_file(manifest_status))
    return false;
  if (fs::hard_link_count(manifest, error) != 1 || error)
    return false;
  return fs::file_size(manifest, error) <= kMaxManifestBytes && !error;
}

bool local_model_uses_pinyin_hotwords(std::string_view path) {
  if (!is_local_model_dir(path))
    return false;
  try {
    const auto manifest = read_manifest(fs::u8path(std::string(path)));
    if (!manifest.is_object())
      return false;
    const auto hotwords = manifest.find("hotwords");
    return hotwords != manifest.end() && hotwords->is_string() &&
           hotwords->get<std::string>() == "pinyin";
  } catch (const VoiceError &) {
    return false;
  }
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
    // X-ASR 离线模型在中文后面也会打半角句末标点（「是星期三.」），在英文句子之间不留空格（「monday.Today」）。
    if (character.size() == 1 && (character == "." || character == "?" || character == "!" || character == ",")) {
      const bool after_cjk = i > 0 && is_cjk_ideograph(characters[i - 1]);
      if (after_cjk) {
        out += character == "." ? "。" : character == "?" ? "？" : character == "!" ? "！" : "，";
        continue;
      }
      const bool between_sentences = i > 0 && characters[i - 1].size() == 1 && std::islower(static_cast<unsigned char>(characters[i - 1][0])) &&
                                     i + 1 < characters.size() && characters[i + 1].size() == 1 &&
                                     std::isupper(static_cast<unsigned char>(characters[i + 1][0])) && character != ",";
      if (between_sentences) {
        out += character;
        out += ' ';
        continue;
      }
    }
    out += character;
  }
  while (!out.empty() && out.back() == ' ')
    out.pop_back();
  return out;
}

std::int64_t local_vad_segment_start(const float *audio, std::size_t count, std::int64_t audio_start,
                                     std::int64_t vad_start, std::int64_t earliest) {
  const auto audio_end = audio_start + static_cast<std::int64_t>(count);
  const auto lower = (std::max)({earliest, audio_start, vad_start - kOnsetLookback});
  if (lower >= vad_start || vad_start > audio_end)
    return vad_start;
  const auto frame_rms = [&](std::int64_t at) {
    double energy = 0.0;
    for (std::int64_t i = at; i < at + kOnsetFrame; ++i) {
      const double sample = audio[static_cast<std::size_t>(i - audio_start)];
      energy += sample * sample;
    }
    return static_cast<float>(std::sqrt(energy / static_cast<double>(kOnsetFrame)));
  };
  // 噪声底取手里全部录音的帧能量第 10 百分位：说话之间的停顿、段尾那 0.5 秒静音都在里面，比只看段首之前稳。
  std::vector<float> levels;
  levels.reserve(count / static_cast<std::size_t>(kOnsetFrame));
  for (auto at = audio_start; at + kOnsetFrame <= audio_end; at += kOnsetFrame)
    levels.push_back(frame_rms(at));
  float noise_floor = kOnsetFloor;
  if (!levels.empty()) {
    const auto nth = levels.begin() + static_cast<std::ptrdiff_t>(levels.size() / 10);
    std::nth_element(levels.begin(), nth, levels.end());
    noise_floor = (std::max)(noise_floor, *nth);
  }
  const float speech = noise_floor * kOnsetRise;
  auto onset = vad_start;
  std::int64_t gap = 0;
  for (auto at = vad_start - kOnsetFrame; at >= lower; at -= kOnsetFrame) {
    if (frame_rms(at) > speech) {
      onset = at;
      gap = 0;
    } else if ((gap += kOnsetFrame) > kOnsetGap) {
      break;
    }
  }
  return (std::max)(lower, onset - kOnsetPreRoll);
}

std::string transducer_hotword_lines(const std::vector<std::string> &words, const std::vector<std::string> &tokens,
                                     const std::string &manifest_unit, std::string &modeling_unit) {
  const std::set<std::string> token_set(tokens.begin(), tokens.end());
  const bool spaced = cjk_spelled_as_words(token_set);
  modeling_unit = spaced ? "bpe" : manifest_unit;
  return transducer_hotwords(words, token_set, spaced);
}

} // namespace msime::voice
