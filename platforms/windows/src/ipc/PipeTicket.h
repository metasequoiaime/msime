#pragma once
#include <array>
#include <cstdint>

namespace msime::windows {
// Transport registration generations, not Engine candidate/focus epochs.
struct PipeTicket {
  uint64_t client = 0;
  std::array<uint64_t, 3> generations{};
};
inline bool same_ticket(const PipeTicket &a, const PipeTicket &b) {
  return a.client == b.client && a.generations == b.generations;
}
// client 是 TSF 拼出的 pid << 32 | tid（tsf/IPC/Ipc.cpp），高 32 位已在 PipePeer::bind 里对过管道对端的 pid。
inline uint32_t client_pid(const PipeTicket &ticket) {
  return static_cast<uint32_t>(ticket.client >> 32);
}
} // namespace msime::windows
