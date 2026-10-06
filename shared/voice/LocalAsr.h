#pragma once
// On-device speech recognition through the pinned sherpa-onnx runtime (resources/voice-runtime.lock.json).
//
// The runtime is loaded at first use with dlopen/LoadLibrary rather than linked, for two reasons. A host whose package does not carry the library still starts and simply reports local recognition as unavailable, instead of failing to load. And the upstream Windows binaries are MSVC builds that the MinGW cross toolchain cannot link against through an import library, while calling through function pointers has no such problem.
//
// A model is a directory holding the files of one catalog entry (resources/local-asr-models.json) and an `msime-model.json` copy of that entry, written last by the installer so a half-installed directory never looks usable. Nothing here downloads anything.

#include "VoiceError.h"

#include <atomic>
#include <chrono>
#include <cstddef>
#include <functional>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

namespace msime::voice {

// The file the installer writes into a model directory once every other file is in place.
inline constexpr std::string_view local_model_manifest = "msime-model.json";

// Overrides where the runtime library is looked for. Call before the first recognition; later calls have no effect once the library is loaded. Without it the library is looked for in MSIME_SHERPA_ONNX_LIBRARY, beside the executable, in the macOS bundle's Frameworks directory, and finally by bare name.
void set_sherpa_library_path(std::string path);
// Whether the runtime library could be loaded. Loads it on the first call.
bool sherpa_runtime_available();
// The reason the last load failed, for logs. Empty when the runtime is loaded or no load was tried.
std::string sherpa_runtime_error();
// Whether `path` is an installed model directory: it exists and holds the manifest.
bool is_local_model_dir(std::string_view path);
// 已安装模型的清单是否要求宿主按拼音纠正最终文本。
bool local_model_uses_pinyin_hotwords(std::string_view path);
// 规范宿主传入的线程数：零和负数使用基于硬件的小默认值，显式值也限制上限以免拖慢设备。
int local_asr_thread_count(int requested);

struct LocalAsrOptions {
  std::string model_dir;
  // Host language tag ("zh-CN", "en", "auto", ...). Models that cannot take a hint ignore it.
  std::string language;
  // Words to bias recognition toward. Transducer and FunASR-nano models use them natively; others ignore them and the host corrects the text afterwards (client-core voice::hotwords).
  std::vector<std::string> hotwords;
  // 0 picks from the hardware.
  int threads = 0;
};

// One dictation. Samples are 16 kHz mono floats in [-1, 1]. Streaming models decode as audio arrives; whole-utterance models are fed through Silero VAD and decode each finished speech segment, so both report partial text before finish(). Not thread-safe: one thread feeds and finishes a session. Throws VoiceError on a missing runtime, an unusable model directory or a cancelled request.
class LocalAsrSession {
public:
  using PartialCallback = std::function<void(const std::string &)>;
  LocalAsrSession(const LocalAsrOptions &options, PartialCallback on_partial,
                  std::shared_ptr<std::atomic_bool> cancelled = nullptr);
  ~LocalAsrSession();
  LocalAsrSession(const LocalAsrSession &) = delete;
  LocalAsrSession &operator=(const LocalAsrSession &) = delete;

  void accept(const float *samples, std::size_t count);
  // Flushes the remaining audio and returns the whole transcript. The session is spent afterwards.
  std::string finish();

private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

// Whole-utterance convenience over LocalAsrSession.
std::string recognize_local_model(const std::vector<float> &samples,
                                  const LocalAsrOptions &options,
                                  const std::shared_ptr<std::atomic_bool> &cancelled);

// Drops loaded recognizers that have not been used for `idle`. A loaded model holds hundreds of megabytes to a gigabyte, so hosts call this from a timer; the next dictation reloads it. Returns how many were dropped.
std::size_t release_idle_local_models(std::chrono::steady_clock::duration idle);
// Drops every loaded recognizer not in use by a live session.
std::size_t release_local_models();

// Joins what the models print into what a person would type: no space before or after CJK punctuation, and runs of single capital letters that a BPE model spells out ("P R", "C I") closed up. Exposed for tests.
std::string tidy_local_transcript(std::string_view text);

} // namespace msime::voice
