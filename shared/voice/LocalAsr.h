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
#include <cstdint>
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
// 灵敏度越高，语音活动阈值越低；未设置或越界时保留原来的 0.5。
float local_asr_vad_threshold(int sensitivity_percent);

struct LocalAsrOptions {
  std::string model_dir;
  // Host language tag ("zh-CN", "en", "auto", ...). Models that cannot take a hint ignore it.
  std::string language;
  // 让识别偏向的词。Transducer（流式与离线）和 FunASR-nano 模型原生支持它们；其他模型忽略它们，由宿主事后修正文本（client-providers 的 `voice::hotwords`）。
  std::vector<std::string> hotwords;
  // 0 picks from the hardware.
  int threads = 0;
  // 仅经过 Silero VAD 的整段模型使用，10–90；0 沿用默认灵敏度。
  int vad_sensitivity_percent = 0;
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
  // 经过 Silero VAD 的整段模型：VAD 此刻是否判定在说话（处在一段语音之中）。这类模型只在一段说完后才出文字，宿主按「多久没有新文字」判断说完会在用户连续说话时提前结束录音，应改看这里。流式模型没有 VAD，始终为 false，它的文字本身随说话更新。
  bool speech_detected() const;
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

// 整段模型解码一段语音时实际的起点（会话里的采样序号）。sherpa-onnx 的 Silero VAD 从概率越过阈值的那一窗往前只留约 26 ms，输入电平低时概率越过阈值比真正起音晚零点几秒到几秒，句首的字就落在段外。这里从 VAD 的段首往前找：10 ms 一帧，比本段录音的噪声底（帧能量的第 10 百分位）高 12 dB 的帧算作语音，中间允许不超过 0.25 秒的低能量间隙，找到的起音再往前留 0.3 秒；整体最多往前 3 秒，不早于 `earliest`（上一段的段尾，第一段为 0），也不早于手里还留着的录音。`audio` 是会话最近的录音，`audio[0]` 是第 `audio_start` 个采样。Exposed for tests.
std::int64_t local_vad_segment_start(const float *audio, std::size_t count, std::int64_t audio_start,
                                     std::int64_t vad_start, std::int64_t earliest);

// 按模型的 token 表整理 transducer 的原生热词（每行一个，交给 sherpa-onnx），并在 `modeling_unit` 里给出编码它们实际要用的建模单位：token 表里的汉字都写成独立的词（`▁写`）时为 `bpe`，热词的汉字之间隔空格；否则沿用清单的 `manifest_unit`。编码不了的词整个丢掉，不连累其他词。Exposed for tests.
std::string transducer_hotword_lines(const std::vector<std::string> &words, const std::vector<std::string> &tokens,
                                     const std::string &manifest_unit, std::string &modeling_unit);

} // namespace msime::voice
