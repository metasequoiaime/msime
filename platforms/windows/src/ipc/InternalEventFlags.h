#pragma once

#include <cstdint>

namespace msime::windows {
// This bit is attached only to the in-process event copy. It is never encoded
// on a pipe and lets the candidate surface distinguish a queue-backlogged hide
// from an ordinary commit/focus hide.
inline constexpr uint32_t internal_late_event = 1u << 31;
inline constexpr uint32_t internal_continuation_hide = 1u << 30;
// 上屏后联想还没被 TIP 显示出来时到达的隐藏（上屏结束组字顺带发出的那一个）：会话保留联想，候选窗只是先藏着，等 TIP 随后的 Show 再露出来，见 FocusedSession::hide_candidate。
inline constexpr uint32_t internal_prediction_hide = 1u << 25;
} // namespace msime::windows
