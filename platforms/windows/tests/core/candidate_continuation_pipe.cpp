#include "candidate_continuation_pipe.h"
#include "ipc_negotiation.h"
#include "PipeTicket.h"
#include "UiSelectionDelivery.h"
#include <stdexcept>
#include <cassert>
#include <iostream>
using namespace FanyImeCandidateContinuationPipe;
using msime::windows::KeyEventSendResult;
namespace {
class Transport final : public msime::windows::MainTransport {
public:
  msime::windows::PipeTicket ticket{42, {1, 2, 3}};
  bool open = true, throws = false;
  size_t fail = 0;
  std::vector<std::vector<uint8_t>> writes;
  bool current(const msime::windows::PipeTicket &other) override {
    return open && msime::windows::same_ticket(ticket, other);
  }
  bool try_current(const msime::windows::PipeTicket &other) override { return current(other); }
  std::optional<FanyImeNamedpipeData> read(const msime::windows::PipeTicket &) override { return std::nullopt; }
  KeyEventSendResult send(const msime::windows::PipeTicket &, uint32_t role,
                         const std::vector<uint8_t> &frame) override {
    assert(role == FanyImePipeRole::ToTsfWorkerThread);
    writes.push_back(frame);
    if (writes.size() != fail) return KeyEventSendResult::Sent;
    if (throws) throw std::runtime_error("Synthetic write failure");
    return KeyEventSendResult::DeliveryAmbiguous;
  }
  void close(const msime::windows::PipeTicket &) noexcept override { open = false; }
};
void delivery_failures() {
  using namespace msime::windows;
  const auto frames = ui_continued_selection(std::string(900, 'a'), 7);
  assert(frames && frames->worker_prefix.size() > 1);
  const auto count = frames->worker_prefix.size() + 1;
  for (bool throws : {false, true}) for (size_t failure = 0; failure <= count; ++failure) {
    FocusGate gate;
    Transport transport;
    const auto change = gate.begin(transport.ticket, 77);
    assert(change && gate.acknowledge(change->pending, [] { return true; }));
    transport.fail = failure; transport.throws = throws;
    const auto result = deliver_ui_selection(transport, gate, change->pending, *frames);
    assert(result == (failure ? UiDeliveryResult::WriteFailed : UiDeliveryResult::Sent));
    assert(transport.writes.size() == (failure ? failure : count));
    assert(transport.open == !failure);
    for (size_t n = 0; n < transport.writes.size(); ++n)
      assert(transport.writes[n] == (n < frames->worker_prefix.size() ? frames->worker_prefix[n] : frames->worker));
    if (failure) {
      assert(deliver_ui_selection(transport, gate, change->pending, *frames) == UiDeliveryResult::Stale);
      assert(transport.writes.size() == failure);
    }
  }
}
auto accept(Assembler &assembler, const FanyImeNamedpipeDataToTsfWorkerThread &frame,
            uint64_t focus = 12, uint64_t epoch = 34) {
  return assembler.accept(frame.data, std::size(frame.data), focus, epoch);
}
}
int main() {
  delivery_failures();
  static_assert(sizeof(FanyImeNamedpipeDataToTsfWorkerThread) == 404);
  for (size_t length : {size_t{1}, ChunkChars, ChunkChars + 1, MaxPayloadChars}) {
    const std::string payload(length, 'a');
    const auto frames = Encode(payload, 7); assert(frames);
    Assembler assembler;
    for (size_t n = 0; n < frames->size(); ++n) {
      const auto result = accept(assembler, frames->at(n));
      assert(bool(result) == (n + 1 == frames->size()));
      if (result) assert(result->text == payload && result->focus == 12 && result->epoch == 34);
    }
  }
  assert(!Encode("", 1) && !Encode("a", 0));
  assert(!Encode(std::string("a\0b", 3), 1) && !Encode("\xc3\xa9", 1));
  assert(!Encode(std::string(MaxPayloadChars + 1, 'a'), 1));
  const auto frames = *Encode(std::string(ChunkChars * 2 + 1, 'a'), 7);
  Assembler assembler;
  assert(!accept(assembler, frames[1]) && !accept(assembler, frames[2]));
  assert(!accept(assembler, frames[0]) && !accept(assembler, frames[2]));
  assert(!accept(assembler, frames[1]));
  assert(!accept(assembler, frames[0]) && !accept(assembler, frames[0]));
  assert(!accept(assembler, frames[1]) && !accept(assembler, frames[2]));
  for (int fault = 0; fault < 7; ++fault) {
    assembler.reset(); assert(!accept(assembler, frames[0]));
    auto frame = frames[1];
    if (fault == 0) ++frame.data[0];
    if (fault == 1) --frame.data[2];
    if (fault == 2) frame.data[HeaderChars] = 0;
    if (fault == 3) frame.data[HeaderChars] = 0x80;
    if (fault == 4) frame.data[std::size(frame.data) - 1] = 'a';
    assert(!accept(assembler, frame, fault == 5 ? 13 : 12, fault == 6 ? 35 : 34));
    assert(!accept(assembler, frames[2]));
  }
  // 中途换 owner 后第一个失败仍属于首帧，孤儿尾帧不获得新 owner 的身份。
  assembler.reset(); assert(!accept(assembler, frames[0]));
  assert(!accept(assembler, frames[1], 12, 35));
  assert(assembler.failed_owner() && assembler.failed_owner()->focus == 12 && assembler.failed_owner()->epoch == 34);
  assert(!accept(assembler, frames[2], 12, 35) && !assembler.failed_owner());
  const auto single = Encode("z", 8)->front();
  assert(!accept(assembler, single, 0) && !accept(assembler, single, 12, 0));
  auto result = accept(assembler, single); assert(result && result->text == "z");
  // 能力是双向交集，不能把新帧发送给没有实现该协议的旧端。
  const auto oldCaps = FanyImeProtocol::Capabilities;
  const auto newCaps = oldCaps | FanyImeProtocol::CandidateContinuation;
  for (auto server : {oldCaps, newCaps}) for (auto tip : {oldCaps, newCaps}) {
    const auto negotiated = FanyImeProtocol::Negotiate(FanyImeProtocol::Hello(1, 2, tip), server);
    assert(negotiated.accepted);
    assert(bool(negotiated.capabilities & FanyImeProtocol::CandidateContinuation) == (server == newCaps && tip == newCaps));
  }
  msime::windows::PipeTicket ticket{1, {2, 3, 4}, newCaps};
  auto stale = ticket; stale.capabilities = oldCaps;
  assert(msime::windows::same_ticket(ticket, ticket) && !msime::windows::same_ticket(ticket, stale));
  std::cout << "candidate continuation pipe: passed\n";
}
