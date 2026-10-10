#pragma once

namespace msime::windows {
struct CandidateWheelSteps {
  int page_up = 0;
  int page_down = 0;
};

// Fold high-precision wheel deltas into whole WHEEL_DELTA notches. A direction
// reversal drops the old partial notch so the first notch in the new direction
// is not delayed by stale travel.
constexpr CandidateWheelSteps consume_candidate_wheel_delta(int &accumulator,
                                                            int delta,
                                                            int notch) {
  CandidateWheelSteps steps;
  if (notch <= 0)
    return steps;
  if ((accumulator > 0 && delta < 0) || (accumulator < 0 && delta > 0))
    accumulator = 0;
  accumulator += delta;
  while (accumulator >= notch) {
    accumulator -= notch;
    ++steps.page_up;
  }
  while (accumulator <= -notch) {
    accumulator += notch;
    ++steps.page_down;
  }
  return steps;
}

// 候选窗翻页请求能不能翻：滚轮翻页受「鼠标滚轮翻页」（`navigation.mouse_wheel`）管，首行 ‹ › 箭头是画出来的按钮，和 macOS 的 changeCandidatePage: 一样不看这个开关。之前两者共用一道闸，开关默认关，箭头画着、鼠标也变成手形，点了却没反应。
constexpr bool candidate_page_allowed(bool from_wheel, bool mouse_wheel) {
  return !from_wheel || mouse_wheel;
}
} // namespace msime::windows
