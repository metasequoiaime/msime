// 候选窗皮肤动效的时间规则（CandidateMotion.h），和 macOS 的 CandidateMotion.mm 一致：连续输入不重播进场、逐行错峰、ease-out 进场和 ease-in 退场。
#include "CandidateMotion.h"
#include <cmath>
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Candidate motion test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
bool near(double value, double expected, double tolerance = 0.005) {
  return std::fabs(value - expected) < tolerance;
}
} // namespace
int main() {
  try {
    // 收起后 400ms 内再出现算同一段连续输入，不播进场；从没收起过、或停下来超过 400ms 后照常播。
    require(candidate_enter_allowed(1000, std::nullopt));
    require(!candidate_enter_allowed(1000, 1000));
    require(!candidate_enter_allowed(1399, 1000));
    require(candidate_enter_allowed(1400, 1000));
    require(candidate_enter_allowed(5000, 1000));

    // 逐行浮现只属于 fade_scale 和 slide_up 的进场。
    require(candidate_motion_staggers_rows(CandidateSkinAnimationPreset::fade_scale) &&
            candidate_motion_staggers_rows(CandidateSkinAnimationPreset::slide_up));
    for (const auto preset : {CandidateSkinAnimationPreset::none, CandidateSkinAnimationPreset::fade,
                              CandidateSkinAnimationPreset::spring, CandidateSkinAnimationPreset::glow})
      require(!candidate_motion_staggers_rows(preset));

    // 第 i 行晚 min(i × stagger, 80ms) 开始；整次进场等最后一行播完。
    require(candidate_row_reveal_delay(0, 14) == 0 && candidate_row_reveal_delay(1, 14) == 14 &&
            candidate_row_reveal_delay(5, 14) == 70 && candidate_row_reveal_delay(6, 14) == 80 &&
            candidate_row_reveal_delay(8, 14) == 80 && candidate_row_reveal_delay(8, 0) == 0);
    require(candidate_enter_total_ms(CandidateSkinAnimationPreset::fade_scale, 180, 14, 5) == 180 + 56);
    require(candidate_enter_total_ms(CandidateSkinAnimationPreset::slide_up, 180, 14, 9) == 180 + 80);
    require(candidate_enter_total_ms(CandidateSkinAnimationPreset::fade, 180, 14, 9) == 180);
    require(candidate_enter_total_ms(CandidateSkinAnimationPreset::fade_scale, 180, 14, 0) == 180);
    // 每一行按自己的延迟和卡片的时长走完。
    require(candidate_row_reveal_progress(0, 0, 14, 180) == 0.0f);
    require(near(candidate_row_reveal_progress(90, 0, 14, 180), 0.5));
    require(candidate_row_reveal_progress(14, 1, 14, 180) == 0.0f);
    require(near(candidate_row_reveal_progress(104, 1, 14, 180), 0.5));
    require(candidate_row_reveal_progress(180 + 80, 8, 14, 180) == 1.0f);
    require(candidate_row_reveal_progress(5000, 3, 14, 180) == 1.0f);
    require(candidate_row_reveal_progress(1, 0, 14, 0) == 1.0f);

    // ease-out 是 Core Animation 的 kCAMediaTimingFunctionEaseOut：200ms 的进场第二帧（约 33ms）约 26%，第五帧（约 83ms）约 59%，170ms 到 96%。
    require(candidate_ease_out(0.0f) == 0.0f && candidate_ease_out(1.0f) == 1.0f);
    require(near(candidate_ease_out(1.0f / 6.0f), 0.26, 0.01));
    require(near(candidate_ease_out(5.0f / 12.0f), 0.591, 0.01));
    require(near(candidate_ease_out(0.85f), 0.963, 0.01));
    require(near(candidate_ease_out(0.5f), 0.685, 0.01));
    // ease-in 先慢后快，退场开头不会一下淡掉。
    require(near(candidate_ease_in(0.5f), 0.315, 0.01) && near(candidate_ease_in(0.85f), 0.764, 0.01));
    // 两条曲线都单调，越界的输入落在两端。
    float previous_out = 0.0f, previous_in = 0.0f;
    for (int step = 1; step <= 100; ++step) {
      const float t = static_cast<float>(step) / 100.0f;
      require(candidate_ease_out(t) >= previous_out && candidate_ease_in(t) >= previous_in);
      previous_out = candidate_ease_out(t);
      previous_in = candidate_ease_in(t);
    }
    require(candidate_ease_out(-1.0f) == 0.0f && candidate_ease_in(2.0f) == 1.0f &&
            candidate_ease_out(std::nanf("")) == 0.0f);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "Candidate motion checks passed\n";
  return 0;
}
