#pragma once
#include "windows_ipc.h"
#include <array>
#include <limits>
#include <optional>
#include <string>

namespace FanyImePreferencePipe {
struct Transition {
  std::uint64_t revision = 0;
  std::uint64_t request = 0;
  std::uint64_t focus = 0;
  std::uint64_t epoch = 0;
};
inline bool valid(const Transition &value) {
  return (value.request == 0 && value.focus == 0 && value.epoch == 0) ||
      (value.request != 0 && value.request != FANY_IME_NO_REQUEST_ID &&
       value.focus != 0 && value.focus != FANY_IME_NO_REQUEST_ID && value.epoch != 0);
}
inline std::optional<FanyImeNamedpipeDataToTsfWorkerThread> encode(const Transition &value) {
  if (!valid(value)) return std::nullopt;
  FanyImeNamedpipeDataToTsfWorkerThread frame{};
  frame.msg_type = FanyImeWorkerReplyType::PreferenceTransition;
  const auto text = std::to_string(value.revision) + "\t" + std::to_string(value.request) +
      "\t" + std::to_string(value.focus) + "\t" + std::to_string(value.epoch);
  if (text.size() >= std::size(frame.data)) return std::nullopt;
  for (std::size_t i = 0; i < text.size(); ++i) frame.data[i] = text[i];
  return frame;
}
inline std::optional<Transition> parse(const FanyImeNamedpipeDataToTsfWorkerThread &frame) {
  if (frame.msg_type != FanyImeWorkerReplyType::PreferenceTransition) return std::nullopt;
  std::array<std::uint64_t, 4> values{};
  std::size_t field = 0;
  bool digit = false, terminated = false;
  for (const auto unit : frame.data) {
    if (terminated) { if (unit != 0) return std::nullopt; continue; }
    if (unit == 0) {
      if (field != 3 || !digit) return std::nullopt;
      terminated = true;
    } else if (unit == '\t') {
      if (!digit || field == 3) return std::nullopt;
      ++field; digit = false;
    } else {
      if (unit < '0' || unit > '9') return std::nullopt;
      const auto next = static_cast<std::uint64_t>(unit - '0');
      if (values[field] > ((std::numeric_limits<std::uint64_t>::max)() - next) / 10)
        return std::nullopt;
      values[field] = values[field] * 10 + next; digit = true;
    }
  }
  Transition result{values[0], values[1], values[2], values[3]};
  return terminated && valid(result) ? std::optional<Transition>(result) : std::nullopt;
}
} // namespace FanyImePreferencePipe
