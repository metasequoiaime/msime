// 用真实运行库、已安装模型和一段 16 kHz 单声道 16 位 PCM 录音跑一次会话：识别出文字，并且经过 VAD 的模型在说话时报 speech_detected()、说完补足静音后不再报。经过 VAD 的模型再把同一段录音压到很低的电平重识别一次，句首不能丢字；最后一次会话只送静音，不能带出前面会话的文字。
// 用法：shared-voice-local-asr-model <runtime> <model dir> <wav>
#include "LocalAsr.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace {

// 录音压低的倍数（-26 dB）。sherpa-onnx 的 VAD 在这个电平下越过阈值比起音晚零点几秒，修正前 X-ASR 自带的 test_wavs/0.wav、2.wav 在中英整句和快速整句上都丢句首的字。
constexpr float kQuietGain = 0.05f;

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

std::string first_character(const std::string &text) {
  if (text.empty())
    return {};
  const auto lead = static_cast<unsigned char>(text[0]);
  const std::size_t width = lead < 0x80 ? 1 : (lead >> 5) == 0x6 ? 2 : (lead >> 4) == 0xE ? 3 : 4;
  return text.substr(0, width);
}

std::string transcribe(const msime::voice::LocalAsrOptions &options, const std::vector<float> &samples, bool *heard_speech, bool *still_speaking) {
  msime::voice::LocalAsrSession session(options, nullptr);
  constexpr std::size_t chunk = 1600;
  for (std::size_t at = 0; at < samples.size(); at += chunk) {
    session.accept(samples.data() + at, std::min(chunk, samples.size() - at));
    if (heard_speech)
      *heard_speech = *heard_speech || session.speech_detected();
  }
  // 两秒静音：比 VAD 的最短静音（0.5 秒）长得多，之后不该还在「说话」。
  const std::vector<float> silence(chunk, 0.0f);
  for (int i = 0; i < 20; ++i)
    session.accept(silence.data(), silence.size());
  if (still_speaking)
    *still_speaking = session.speech_detected();
  return session.finish();
}

// 前面留 0.6 秒，整段压低到 kQuietGain，再像宿主送来的 16 位 PCM 一样量化。不量化的浮点信号没有量化噪声，比真实录音还干净，Silero 反而整段都判不成语音，测不出宿主上的情形。
std::vector<float> quiet_copy(const std::vector<float> &samples) {
  std::vector<float> out(16000 * 6 / 10, 0.0f);
  for (const auto sample : samples)
    out.push_back(std::round(sample * kQuietGain * 32767.0f) / 32768.0f);
  return out;
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
  bool heard_speech = false;
  bool still_speaking = false;
  const auto text = transcribe(options, samples, &heard_speech, &still_speaking);
  std::printf("%s\n", text.c_str());
  if (text.empty())
    return fail("empty transcript");
  if (streaming && heard_speech)
    return fail("a streaming model has no VAD and must not report speech");
  if (!streaming && !heard_speech)
    return fail("the VAD never reported speech");
  if (still_speaking)
    return fail("the VAD still reports speech after two seconds of silence");
  if (!streaming) {
    const auto quiet = transcribe(options, quiet_copy(samples), nullptr, nullptr);
    std::printf("%s\n", quiet.c_str());
    if (first_character(quiet) != first_character(text))
      return fail("the quiet recording lost the start of the sentence");
  }
  if (!transcribe(options, std::vector<float>(16000, 0.0f), nullptr, nullptr).empty())
    return fail("a session of silence returned text");
  return 0;
}
