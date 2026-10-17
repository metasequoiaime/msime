// 用真实运行库、已安装模型和一段 16 kHz 单声道 16 位 PCM 录音跑一次会话：识别出文字，并且经过 VAD 的模型在说话时报 speech_detected()、说完补足静音后不再报。
// 用法：shared-voice-local-asr-model <runtime> <model dir> <wav>
#include "LocalAsr.h"

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace {

int fail(const std::string &message) {
  std::fprintf(stderr, "%s\n", message.c_str());
  return 1;
}

std::vector<float> read_wav(const std::string &path) {
  std::ifstream input(path, std::ios::binary);
  const std::string bytes((std::istreambuf_iterator<char>(input)), std::istreambuf_iterator<char>());
  std::vector<float> samples;
  if (bytes.size() < 12 || bytes.compare(0, 4, "RIFF") != 0 || bytes.compare(8, 4, "WAVE") != 0)
    return samples;
  for (std::size_t at = 12; at + 8 <= bytes.size();) {
    std::uint32_t size = 0;
    std::memcpy(&size, bytes.data() + at + 4, 4);
    if (bytes.compare(at, 4, "data") == 0) {
      const auto end = std::min(bytes.size(), at + 8 + size);
      for (std::size_t i = at + 8; i + 1 < end; i += 2) {
        std::int16_t value = 0;
        std::memcpy(&value, bytes.data() + i, 2);
        samples.push_back(static_cast<float>(value) / 32768.0f);
      }
      break;
    }
    at += 8 + size + (size & 1);
  }
  return samples;
}

} // namespace

int main(int argc, char **argv) {
  using namespace msime::voice;
  if (argc != 4)
    return fail("usage: shared-voice-local-asr-model <runtime> <model dir> <wav>");
  set_sherpa_library_path(argv[1]);
  if (!sherpa_runtime_available())
    return fail("runtime: " + sherpa_runtime_error());
  auto samples = read_wav(argv[3]);
  if (samples.empty())
    return fail("no audio in " + std::string(argv[3]));
  std::ifstream manifest_file(std::string(argv[2]) + "/" + std::string(local_model_manifest));
  const std::string manifest((std::istreambuf_iterator<char>(manifest_file)), std::istreambuf_iterator<char>());
  const bool streaming = manifest.find("\"online_transducer\"") != std::string::npos;

  LocalAsrOptions options;
  options.model_dir = argv[2];
  LocalAsrSession session(options, nullptr);
  bool heard_speech = false;
  constexpr std::size_t chunk = 1600;
  for (std::size_t at = 0; at < samples.size(); at += chunk) {
    session.accept(samples.data() + at, std::min(chunk, samples.size() - at));
    heard_speech = heard_speech || session.speech_detected();
  }
  // 两秒静音：比 VAD 的最短静音（0.5 秒）长得多，之后不该还在「说话」。
  const std::vector<float> silence(chunk, 0.0f);
  for (int i = 0; i < 20; ++i)
    session.accept(silence.data(), silence.size());
  const bool still_speaking = session.speech_detected();
  const auto text = session.finish();
  std::printf("%s\n", text.c_str());
  if (text.empty())
    return fail("empty transcript");
  if (streaming && heard_speech)
    return fail("a streaming model has no VAD and must not report speech");
  if (!streaming && !heard_speech)
    return fail("the VAD never reported speech");
  if (still_speaking)
    return fail("the VAD still reports speech after two seconds of silence");
  return 0;
}
