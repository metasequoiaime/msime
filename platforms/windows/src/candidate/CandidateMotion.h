#pragma once
#include "CandidateSkin.h"
#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>

namespace msime::windows {
// 皮肤动效里候选窗进场、退场的时间规则，照 macOS 的 platforms/macos/src/candidate/CandidateMotion.mm。macOS 交给 Core Animation，Windows 的候选窗按 16ms 的定时器在 paint 里自己插值，这里只回答「这一帧该是多少」，不碰 Direct2D，测得到。

// 收起后这么短时间内再次显示，算同一段连续输入，不播进场（kTypingBurstInterval）：连续打字时上屏后紧接着打下一个词，每个词开头都重播一次进场只会让卡片闪。
inline constexpr uint64_t candidate_typing_burst_ms = 400;
// 逐行错峰的总延迟上限（kStaggerCap）：再长的一页，最后一行也只比第一行晚这么久开始。
inline constexpr uint64_t candidate_stagger_cap_ms = 80;
// 逐行浮现时每一行从下方多远升到原位（kRowRevealDistance），DIP。
inline constexpr double candidate_row_reveal_distance = 3.0;

// 候选窗这次出现能不能播进场：上次收起（last_hidden，GetTickCount64 的毫秒）已经过去 candidate_typing_burst_ms 以上，或者从没收起过。
inline bool candidate_enter_allowed(uint64_t now, std::optional<uint64_t> last_hidden) {
  return !last_hidden || now < *last_hidden || now - *last_hidden >= candidate_typing_burst_ms;
}

// 哪些进场预设让候选行逐行浮现（StaggersRows）：fade_scale 和 slide_up。
inline bool candidate_motion_staggers_rows(CandidateSkinAnimationPreset preset) {
  return preset == CandidateSkinAnimationPreset::fade_scale ||
         preset == CandidateSkinAnimationPreset::slide_up;
}

// 第 row 行（从 0 数）比第一行晚多久开始浮现：min(row × stagger_ms, candidate_stagger_cap_ms)。
inline uint64_t candidate_row_reveal_delay(size_t row, unsigned stagger_ms) {
  // 一页最多 9 行，皮肤的 staggerMs 不超过 80（candidate_skin_animation），乘起来不会溢出。
  return (std::min)(static_cast<uint64_t>(row) * stagger_ms, candidate_stagger_cap_ms);
}

// 一次进场一共播多久：卡片的时长，逐行浮现时再加上最后一行的延迟。rows 是这一页的行数。
inline uint64_t candidate_enter_total_ms(CandidateSkinAnimationPreset preset, unsigned duration_ms,
                                         unsigned stagger_ms, size_t rows) {
  if (!candidate_motion_staggers_rows(preset) || rows == 0)
    return duration_ms;
  return duration_ms + candidate_row_reveal_delay(rows - 1, stagger_ms);
}

// 第 row 行浮现到了哪里（0 到 1，未加缓动）：进场开始后 elapsed 毫秒，减去它的延迟，再按卡片的时长算。
inline float candidate_row_reveal_progress(uint64_t elapsed, size_t row, unsigned stagger_ms,
                                           unsigned duration_ms) {
  const uint64_t delay = candidate_row_reveal_delay(row, stagger_ms);
  if (elapsed <= delay)
    return 0.0f;
  if (duration_ms == 0)
    return 1.0f;
  return static_cast<float>((std::min)(elapsed - delay, static_cast<uint64_t>(duration_ms))) /
         static_cast<float>(duration_ms);
}

// CSS / Core Animation 的 cubic-bezier(x1, y1, x2, y2) 在 t 处的值：先二分求出横坐标为 t 的参数，再取纵坐标。两个控制点的横坐标都在 0 到 1 之间，横坐标随参数单调，二分总能收敛。
inline float candidate_motion_curve(float t, double x1, double y1, double x2, double y2) {
  if (!(t > 0.0f))
    return 0.0f;
  if (!(t < 1.0f))
    return 1.0f;
  const auto bezier = [](double s, double p1, double p2) {
    const double r = 1.0 - s;
    return 3.0 * r * r * s * p1 + 3.0 * r * s * s * p2 + s * s * s;
  };
  double low = 0.0, high = 1.0;
  for (int step = 0; step < 32; ++step) {
    const double middle = (low + high) * 0.5;
    if (bezier(middle, x1, x2) < t)
      low = middle;
    else
      high = middle;
  }
  return static_cast<float>(bezier((low + high) * 0.5, y1, y2));
}
// 进场和选中动效的缓动：标准 ease-out，和 macOS 的 kCAMediaTimingFunctionEaseOut（cubic-bezier(0, 0, 0.58, 1)）一样，200ms 的进场第二帧约走完 26%、第五帧约 59%，看得见过程又不拖尾。
inline float candidate_ease_out(float t) { return candidate_motion_curve(t, 0.0, 0.0, 0.58, 1.0); }
// 退场的缓动：标准 ease-in（kCAMediaTimingFunctionEaseIn，cubic-bezier(0.42, 0, 1, 1)），先慢后快地收走。
inline float candidate_ease_in(float t) { return candidate_motion_curve(t, 0.42, 0.0, 1.0, 1.0); }
} // namespace msime::windows
