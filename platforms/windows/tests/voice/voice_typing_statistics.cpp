#include "VoiceTypingStatistics.h"
#include <cassert>
#include <iostream>
#include <limits>

using namespace msime::windows;

int main() {
  // 按音频帧数计时：流式没有整份 PCM，识别完成的墙钟时间也不影响累计值。
  assert(voice_statistics_milliseconds(0, 16000) == 0);
  assert(voice_statistics_milliseconds(15, 16000) == 0);
  assert(voice_statistics_milliseconds(16, 16000) == 1);
  assert(voice_statistics_milliseconds(4000, 16000) == 250);
  assert(voice_statistics_milliseconds(16000 * 60 + 8000, 16000) == 60'500);
  assert(voice_statistics_milliseconds(48000 * 60, 48000) == 60'000);
  assert(voice_statistics_milliseconds(16000 * 600, 16000) == 600'000);
  assert(voice_statistics_milliseconds(16000 * 655, 16000) == 600'000);
  assert(voice_statistics_milliseconds(std::numeric_limits<std::size_t>::max(),
                                       16000) == 600'000);
  assert(voice_statistics_milliseconds(16000, 0) == 0);
  assert(voice_statistics_milliseconds(
             16000, std::numeric_limits<std::size_t>::max()) == 0);

  // 只向共享统计发送时长和本地日期，不携带音频、识别文本或服务凭据。
  const auto request =
      nlohmann::json::parse(typing_statistics_record_voice_request(
          "/synthetic/statistics", "2026-11-17", 60'500));
  assert(request == (nlohmann::json{{"directory", "/synthetic/statistics"},
                                    {"action",
                                     {{"operation", "record_voice"},
                                      {"day", "2026-11-17"},
                                      {"milliseconds", 60'500}}}}));
  assert(
      typing_statistics_record_voice_request("", "2026-11-17", 1000).empty());
  assert(
      typing_statistics_record_voice_request("/synthetic/statistics", "", 1000)
          .empty());
  assert(typing_statistics_record_voice_request("/synthetic/statistics",
                                                "2026-11-17", 0)
             .empty());
  std::cout << "Voice typing statistics passed\n";
}
