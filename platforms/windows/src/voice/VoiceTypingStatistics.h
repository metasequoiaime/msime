#pragma once
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <nlohmann/json.hpp>
#include <string>
#include <string_view>

namespace msime::windows {
// 流式识别不保留整份音频，时长来自实际采集帧数，不含识别与润色等待。
inline uint64_t voice_statistics_milliseconds(std::size_t frames,
                                              std::size_t sample_rate) {
  if (sample_rate == 0 ||
      sample_rate > (std::numeric_limits<std::size_t>::max)() / 600 ||
      sample_rate > (std::numeric_limits<uint64_t>::max)() / 600'000)
    return 0;
  // 与共享 record_voice 的单次 10 分钟上限一致，先截帧数再乘，避免溢出。
  return static_cast<uint64_t>((std::min)(frames, sample_rate * 600)) * 1000 /
         sample_rate;
}

inline std::string typing_statistics_record_voice_request(
    std::string_view directory, const std::string &day, uint64_t milliseconds) {
  if (directory.empty() || day.size() != 10 || milliseconds == 0)
    return {};
  return nlohmann::json{{"directory", std::string(directory)},
                        {"action",
                         {{"operation", "record_voice"},
                          {"day", day},
                          {"milliseconds", milliseconds}}}}
      .dump();
}
} // namespace msime::windows
