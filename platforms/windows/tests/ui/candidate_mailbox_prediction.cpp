// 候选窗信箱里的上屏后联想：送达时先藏着，等 TIP 在光标处发 Show 才露出来；Server 会话留着联想时的隐藏（internal_prediction_hide）只是先藏起来，内容留给随后的 Show；普通隐藏照旧清空；UI-less 的 Show 不让它露出来。
#include "CandidateMailbox.h"
#include "ipc/InternalEventFlags.h"
#include <iostream>
#include <stdexcept>

using namespace msime::windows;
namespace {
void require(bool condition) {
  if (!condition)
    throw std::runtime_error("Candidate mailbox prediction test failed");
}
} // namespace
int main() {
  try {
    {
      FocusGate prediction_gate;
      CandidateMailbox predictions;
      const PipeTicket ticket{44, {7, 8, 9}};
      const auto change = prediction_gate.begin(ticket, 3);
      require(change.has_value());
      require(prediction_gate.acknowledge(change->pending, [] { return true; }));
      const auto lease = change->pending;
      auto deliver = [&](uint64_t generation, bool prediction) {
        FanyImeNamedpipeData packet{};
        packet.client_id = lease.transport.client;
        packet.request_id = generation;
        packet.event_type = FanyImePipeEventType::KeyEvent;
        PendingReply reply{};
        reply.source.client_id = packet.client_id;
        reply.source.activation_epoch = lease.epoch;
        reply.source.request_id = packet.request_id;
        auto candidates = nlohmann::json::array();
        for (size_t index = 0; index < 2; ++index)
          candidates.push_back({{"id", {{"session", 1}, {"generation", generation}, {"index", index}}},
                                {"text", index == 0 ? "饭" : "东西"},
                                {"highlighted", index == 0}});
        nlohmann::json view{{"session", 1}, {"generation", generation}, {"focused", true},
                            {"editing_text", ""}, {"preedit", ""}, {"candidates", candidates}};
        if (prediction)
          view["prediction"] = true;
        reply.source.transition = {{"view", view}};
        return prediction_gate.with_active(lease, [&] { predictions.delivered(lease, reply, packet); });
      };
      auto event = [&](uint32_t type, uint32_t modifiers = 0) {
        FanyImeNamedpipeData packet{};
        packet.client_id = lease.transport.client;
        packet.event_type = type;
        packet.modifiers_down = modifiers;
        packet.point[0] = 640;
        packet.point[1] = 480;
        return prediction_gate.with_active(lease, [&] { predictions.event(lease, packet); });
      };
      // 没有 prediction 标记、editing_text 为空的候选不是联想，不画。
      require(deliver(60, false));
      require(!predictions.snapshot(prediction_gate)->visible);
      // 联想送达：内容在，但先藏着。
      require(deliver(61, true));
      auto value = predictions.snapshot(prediction_gate);
      require(value && !value->visible && value->candidates.empty());
      // 上屏顺带的隐藏：先藏着，内容不丢。
      require(event(FanyImePipeEventType::HideCandidateWnd, internal_prediction_hide));
      require(!predictions.snapshot(prediction_gate)->visible);
      // TIP 在光标处发 Show：露出来，位置是 Show 带来的，没有拼音，候选是联想。
      require(event(FanyImePipeEventType::ShowCandidateWnd));
      value = predictions.snapshot(prediction_gate);
      require(value && value->visible && value->prediction && value->preedit.empty() &&
              value->candidates.size() == 2 && value->candidates[0].text == "饭" && value->x == 640 &&
              value->y == 480 && value->generation == 61);
      // 关掉联想的隐藏（Esc、光标移走）：清空，之后的 Show 也不能让它回来。
      require(event(FanyImePipeEventType::HideCandidateWnd));
      value = predictions.snapshot(prediction_gate);
      require(value && !value->visible && value->candidates.empty());
      require(event(FanyImePipeEventType::ShowCandidateWnd));
      require(!predictions.snapshot(prediction_gate)->visible);
      // 点选联想后 Server 送来下一代联想：同样等 Show。
      require(deliver(62, true));
      require(!predictions.snapshot(prediction_gate)->visible);
      require(event(FanyImePipeEventType::ShowCandidateWnd));
      require(predictions.snapshot(prediction_gate)->visible);
      // UI-less 宿主的 Show 不让它露出来。
      require(deliver(63, true));
      require(event(FanyImePipeEventType::ShowCandidateWnd, FanyImePipeFlags::UiLess));
      require(!predictions.snapshot(prediction_gate)->visible);
    }
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
