#include "CandidateClickWorker.h"
#include "CandidateMailbox.h"
#include "ModeMailbox.h"
#include <future>

using namespace msime::windows;
namespace {
class ModeTransport final : public MainTransport {
public:
  bool current(const PipeTicket &) override { return open; }
  bool try_current(const PipeTicket &) override { return open; }
  std::optional<FanyImeNamedpipeData> read(const PipeTicket &) override {
    return std::nullopt;
  }
  KeyEventSendResult send(const PipeTicket &, uint32_t,
                          const std::vector<uint8_t> &) override {
    return open ? KeyEventSendResult::Sent
                : KeyEventSendResult::DefinitelyNotSent;
  }
  void close(const PipeTicket &) noexcept override { open = false; }
  bool open = true;
};
void require(bool condition) {
  if (!condition)
    throw std::runtime_error("Candidate mailbox test failed");
}
} // namespace
void candidate_mailbox_tests() {
  {
    CandidateClick click{{{42, {1, 2, 3}}, 1, 1}, 2, 3, 4};
    std::promise<void> entered, release;
    auto started = entered.get_future();
    auto resume = release.get_future();
    size_t calls = 0;
    const auto caller = std::this_thread::get_id();
    CandidateClickWorker worker([&](const CandidateClick &value) {
      require(std::this_thread::get_id() != caller && value.index == 4);
      ++calls;
      entered.set_value();
      require(resume.wait_for(std::chrono::seconds(10)) ==
              std::future_status::ready);
    });
    require(worker.submit(click));
    require(started.wait_for(std::chrono::seconds(10)) ==
            std::future_status::ready);
    const bool busy_rejected = !worker.submit(click);
    worker.request_stop();
    release.set_value();
    worker.stop();
    require(busy_rejected && calls == 1 && !worker.failed() &&
            !worker.submit(click));
    std::promise<void> failing;
    auto began = failing.get_future();
    CandidateClickWorker failed([&](const CandidateClick &) {
      failing.set_value();
      throw std::runtime_error("Synthetic click failure");
    });
    require(failed.submit(click));
    require(began.wait_for(std::chrono::seconds(10)) ==
            std::future_status::ready);
    failed.stop();
    require(failed.failed() && !failed.submit(click));
  }
  FocusGate gate;
  CandidateMailbox mailbox;
  PipeTicket a{42, {1, 2, 3}}, b{43, {4, 5, 6}};
  auto activate = [&](const PipeTicket &ticket, uint64_t token) {
    const auto change = gate.begin(ticket, token);
    require(change.has_value());
    require(gate.acknowledge(change->pending, [] { return true; }));
    return change->pending;
  };
  auto publish = [&](const FocusLease &lease, uint64_t generation,
                     bool uiless = false, uint32_t metadata = 0) {
    FanyImeNamedpipeData packet{};
    packet.client_id = lease.transport.client;
    packet.request_id = generation;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.modifiers_down = (uiless ? FanyImePipeFlags::UiLess : 0) | metadata;
    PendingReply reply{};
    reply.source.client_id = packet.client_id;
    reply.source.activation_epoch = lease.epoch;
    reply.source.request_id = packet.request_id;
    reply.source.transition = {{"view",
                                {{"session", 1},
                                 {"generation", generation},
                                 {"focused", true},
                                 {"editing_text", "U4"},
                                 {"preedit", "U4"},
                                 {"candidates", nlohmann::json::array()}}}};
    return gate.with_active(lease,
                            [&] { mailbox.delivered(lease, reply, packet); });
  };
  {
    ModeMailbox modes;
    ModeTransport transport;
    require(!modes.snapshot(gate, transport));
    const auto lease = activate(a, 9);
    FanyImeNamedpipeData packet{};
    packet.client_id = a.client;
    packet.event_type = FanyImePipeEventType::StatusSnapshot;
    packet.keycode = 1;
    packet.modifiers_down = 1;
    packet.pinyin_length = 1;
    gate.with_active(lease, [&] { modes.event(lease, packet); });
    auto value = modes.snapshot(gate, transport);
    require(value && value->chinese == true && value->fullwidth == true &&
            value->chinese_punctuation == true);
    modes.disconnected(a);
    require(!modes.snapshot(gate, transport));
    auto replacement = a;
    ++replacement.generations[0];
    const auto next = activate(replacement, 10);
    packet.event_type = FanyImePipeEventType::ClientActivated;
    gate.with_active(next, [&] { modes.event(next, packet); });
    modes.disconnected(a); // Late old-stream cleanup cannot erase replacement.
    value = modes.snapshot(gate, transport);
    require(value && same_ticket(value->lease.transport, replacement) &&
            !value->chinese && !value->chinese_punctuation &&
            !value->fullwidth);
    transport.open = false;
    require(!modes.snapshot(gate, transport));
    transport.open = true;
    require(modes.snapshot(gate, transport).has_value());
    modes.stop();
    packet.event_type = FanyImePipeEventType::StatusSnapshot;
    gate.with_active(next, [&] { modes.event(next, packet); });
    require(!modes.snapshot(gate, transport));
  }
  require(!mailbox.snapshot(gate));
  const auto first = activate(a, 1);
  require(publish(first, 1));
  require(mailbox.snapshot(gate)->visible);
  // Keyboard selection must not assume that delivery means the window has
  // presented the same generation yet. A bounded timeout is a miss until the
  // exact lease-bound receipt arrives.
  require(!mailbox.wait_rendered(first, 1, std::chrono::milliseconds(2)));
  auto stale_receipt = first;
  ++stale_receipt.token;
  mailbox.rendered(stale_receipt, 1);
  require(!mailbox.wait_rendered(first, 1, std::chrono::milliseconds(2)));
  mailbox.rendered(first, 1);
  require(mailbox.wait_rendered(first, 1, std::chrono::milliseconds(2)));
  // A same-generation asynchronous refresh receives a new render serial;
  // the old painted frame must not satisfy its gate.
  require(publish(first, 1));
  require(!mailbox.wait_rendered(first, 2, std::chrono::milliseconds(2)));
  mailbox.rendered(first, 2);
  require(mailbox.wait_rendered(first, 2, std::chrono::milliseconds(2)));
  {
    std::promise<void> held, release;
    auto holding = held.get_future();
    auto resume = release.get_future();
    auto writer = std::async(std::launch::async, [&] {
      return gate.with_active(first, [&] {
        held.set_value();
        require(resume.wait_for(std::chrono::seconds(10)) ==
                std::future_status::ready);
      });
    });
    require(holding.wait_for(std::chrono::seconds(10)) ==
            std::future_status::ready);
    auto reader = std::async(std::launch::async,
                             [&] { return mailbox.snapshot(gate, false); });
    const bool ready =
        reader.wait_for(std::chrono::seconds(1)) == std::future_status::ready;
    release.set_value();
    require(writer.get());
    const auto blocked = reader.get();
    require(ready && !blocked);
    require(mailbox.snapshot(gate, false)->visible);
    require(!mailbox.snapshot(gate, false,
                              [](const FocusLease &) { return false; }));
  }
  auto visual_event = [&](const FocusLease &lease, uint32_t type,
                          uint32_t modifiers = 0, uint32_t keycode = 0,
                          bool late = false) {
    FanyImeNamedpipeData packet{};
    packet.client_id = lease.transport.client;
    packet.event_type = type;
    packet.modifiers_down = modifiers;
    if (late)
      packet.modifiers_down |= internal_late_event;
    packet.keycode = keycode;
    packet.point[0] = -200;
    packet.point[1] = 300;
    return gate.with_active(lease, [&] { mailbox.event(lease, packet); });
  };
  auto settled_hide = [&] {
    std::this_thread::sleep_for(std::chrono::milliseconds(30));
    return mailbox.snapshot(gate);
  };
  require(visual_event(first, FanyImePipeEventType::HideCandidateWnd));
  const auto immediate = mailbox.snapshot(gate);
  require(immediate && !immediate->visible && immediate->preedit.empty());
  require(publish(first, 1));
  require(
      visual_event(first, FanyImePipeEventType::HideCandidateWnd, 0, 0, true));
  const auto grace = mailbox.snapshot(gate);
  require(grace && grace->visible && !grace->preedit.empty());
  const auto suppressed = settled_hide();
  require(suppressed && !suppressed->visible && suppressed->preedit.empty() &&
          suppressed->candidates.empty());
  require(visual_event(first, FanyImePipeEventType::MoveCandidateWnd));
  require(!mailbox.snapshot(gate)->visible &&
          mailbox.snapshot(gate)->x == -200);
  require(visual_event(first, FanyImePipeEventType::ShowCandidateWnd));
  require(!mailbox.snapshot(gate)->visible &&
          mailbox.snapshot(gate)->generation == 1);
  require(visual_event(first, FanyImePipeEventType::HideCandidateWnd));
  require(publish(first, 2));
  require(mailbox.snapshot(gate)->visible); // A new confirmed key refreshes UI.
  require(visual_event(first, FanyImePipeEventType::HideCandidateWnd,
                       internal_continuation_hide));
  const auto continued = mailbox.snapshot(gate);
  require(continued && continued->visible && continued->generation == 2 &&
          !continued->preedit.empty());
  auto stale_continuation = first;
  ++stale_continuation.token;
  require(!visual_event(stale_continuation,
                        FanyImePipeEventType::HideCandidateWnd,
                        internal_continuation_hide));
  require(mailbox.snapshot(gate)->visible);
  require(visual_event(first, FanyImePipeEventType::MoveCandidateWnd,
                       FanyImePipeFlags::UiLess));
  const auto host_drawn = mailbox.snapshot(gate);
  require(host_drawn && !host_drawn->visible && host_drawn->preedit.empty() &&
          host_drawn->candidates.empty() && host_drawn->x == -200);
  require(visual_event(first, FanyImePipeEventType::MoveCandidateWnd));
  require(!mailbox.snapshot(gate)->visible);
  require(visual_event(first, FanyImePipeEventType::ShowCandidateWnd));
  require(mailbox.snapshot(gate)->visible &&
          mailbox.snapshot(gate)->generation == 2);
  require(visual_event(first, FanyImePipeEventType::ShowCandidateWnd,
                       FanyImePipeFlags::UiLess));
  require(!mailbox.snapshot(gate)->visible);
  for (auto mode_event :
       {FanyImePipeEventType::IMESwitch, FanyImePipeEventType::StatusSnapshot,
        FanyImePipeEventType::FocusRestored}) {
    require(publish(first, 2));
    require(visual_event(first, mode_event, 0, 1));
    require(mailbox.snapshot(gate)->visible);
    require(visual_event(first, FanyImePipeEventType::PuncSwitch));
    require(mailbox.snapshot(gate)->visible);
    auto stale = first;
    ++stale.token;
    require(!visual_event(stale, mode_event));
    require(mailbox.snapshot(gate)->visible);
    require(visual_event(first, mode_event));
    const auto hidden_now = mailbox.snapshot(gate);
    require(hidden_now && !hidden_now->visible && hidden_now->preedit.empty());
    require(visual_event(first, mode_event, 0, 1));
    require(visual_event(first, FanyImePipeEventType::ShowCandidateWnd));
    require(!mailbox.snapshot(gate)->visible);
  }
  const auto pending = gate.begin(b, 2);
  require(pending.has_value() && !mailbox.snapshot(gate));
  require(gate.acknowledge(pending->pending, [] { return true; }));
  require(!mailbox.snapshot(gate)); // No old owner's frame on the new focus.
  require(!publish(first, 2));
  require(publish(pending->pending, 3));
  require(!visual_event(first, FanyImePipeEventType::HideCandidateWnd));
  require(mailbox.snapshot(gate)->visible);
  mailbox.disconnected(a);
  require(mailbox.snapshot(gate)->generation == 3);
  auto replacement = b;
  ++replacement.generations[0];
  const auto next = activate(replacement, 3);
  require(!mailbox.snapshot(gate));
  require(publish(next, 4, true));
  mailbox.disconnected(b);
  const auto hidden = mailbox.snapshot(gate);
  require(hidden && !hidden->visible && hidden->preedit.empty());
  require(visual_event(next, FanyImePipeEventType::ShowCandidateWnd));
  require(!mailbox.snapshot(gate)->visible);
  auto reader = std::async(std::launch::async, [&] {
    for (size_t i = 0; i < 200; ++i) {
      const auto value = mailbox.snapshot(gate);
      require(value && same_ticket(value->lease.transport, replacement));
    }
  });
  for (uint64_t i = 5; i < 205; ++i)
    require(publish(next, i));
  reader.get();
  require(mailbox.snapshot(gate)->generation == 204);
  require(gate.deactivate(next));
  require(!mailbox.snapshot(gate));
  const auto final = activate(replacement, 4);
  require(publish(final, 205));
  mailbox.disconnected(replacement);
  require(!mailbox.snapshot(gate));
  require(publish(final, 206));
  // 游戏会话标记：首个按键没带位、随后的 Show 带位时补上；同一租约里后续不带位的按键、Move、隐藏和异步刷新都不会清掉它。
  require(!mailbox.snapshot(gate)->game_host);
  require(visual_event(final, FanyImePipeEventType::ShowCandidateWnd,
                       PipeMetadata::GameHost));
  require(mailbox.snapshot(gate)->game_host && mailbox.snapshot(gate)->visible);
  require(publish(final, 207));
  require(mailbox.snapshot(gate)->game_host);
  const auto before_refresh = mailbox.snapshot(gate)->render_serial;
  mailbox.translations(final, {{"session", 1},
                               {"generation", 207},
                               {"focused", true},
                               {"editing_text", "U4"},
                               {"preedit", "U4"},
                               {"candidates", nlohmann::json::array()}});
  const auto refreshed = mailbox.snapshot(gate);
  require(refreshed && refreshed->game_host &&
          refreshed->render_serial > before_refresh);
  require(visual_event(final, FanyImePipeEventType::MoveCandidateWnd));
  require(mailbox.snapshot(gate)->game_host);
  require(visual_event(final, FanyImePipeEventType::HideCandidateWnd));
  require(publish(final, 208));
  require(mailbox.snapshot(gate)->game_host);
  // 新的激活是新的租约，标记不跨租约沿用；按键自己带位时直接生效。
  const auto relaunched = activate(replacement, 5);
  require(publish(relaunched, 209));
  require(!mailbox.snapshot(gate)->game_host);
  require(publish(relaunched, 210, false, PipeMetadata::GameHost));
  require(mailbox.snapshot(gate)->game_host);
  // 读音和逐词拆解只用于显示：单独到达时挂到眼下的候选上并换一个快照编号让候选窗重画；读音只跟着算它时的那条释义，拆解只看候选文字；偏好变了就摘掉。
  {
    FanyImeNamedpipeData packet{};
    packet.client_id = relaunched.transport.client;
    packet.request_id = 212;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    PendingReply reply{};
    reply.source.client_id = packet.client_id;
    reply.source.activation_epoch = relaunched.epoch;
    reply.source.request_id = packet.request_id;
    const auto row = [](size_t index, const char *text, const char *translation) {
      return nlohmann::json{{"id", {{"session", 1}, {"generation", 212}, {"index", index}}},
                            {"text", text},
                            {"highlighted", index == 0},
                            {"translation", translation}};
    };
    reply.source.transition = {{"view",
                                {{"session", 1},
                                 {"generation", 212},
                                 {"focused", true},
                                 {"editing_text", "nihao"},
                                 {"preedit", "nihao"},
                                 {"candidates",
                                  {row(0, "你好", "hello"), row(1, "我喜欢你", "")}}}}};
    require(gate.with_active(relaunched,
                             [&] { mailbox.delivered(relaunched, reply, packet); }));
    const auto before = mailbox.snapshot(gate);
    require(before && before->candidates.size() == 2 &&
            before->candidates[0].pronunciation.empty());
    CandidateReadings readings;
    readings["你好"] = {"hello", "/həˈləʊ/", ""};
    readings["我喜欢你"] = {"", "", "我 I · 喜欢 to like · 你 you"};
    mailbox.readings(relaunched, readings);
    const auto after = mailbox.snapshot(gate);
    require(after && after->render_serial > before->render_serial);
    require(after->candidates[0].pronunciation == "/həˈləʊ/" &&
            candidate_secondary_text(after->candidates[0]) == "hello  /həˈləʊ/");
    require(after->candidates[1].breakdown == "我 I · 喜欢 to like · 你 you");
    // 释义变了的候选不挂旧读音。
    readings["你好"].translation = "hi";
    mailbox.readings(relaunched, readings);
    require(mailbox.snapshot(gate)->candidates[0].pronunciation.empty());
    // 下一个键的快照同样挂得上，直到偏好变化把它们摘掉。
    mailbox.readings(relaunched, {{"你好", {"hello", "/həˈləʊ/", ""}}});
    packet.request_id = reply.source.request_id = 213;
    require(gate.with_active(relaunched,
                             [&] { mailbox.delivered(relaunched, reply, packet); }));
    require(mailbox.snapshot(gate)->candidates[0].pronunciation == "/həˈləʊ/");
    const auto serial = mailbox.snapshot(gate)->render_serial;
    mailbox.clear_readings();
    const auto cleared = mailbox.snapshot(gate);
    require(cleared->candidates[0].pronunciation.empty() &&
            cleared->candidates[1].breakdown.empty() && cleared->render_serial > serial);
  }
  mailbox.stop();
  require(publish(relaunched, 211));
  require(
      !mailbox.snapshot(gate)); // A late delivery cannot reopen a stopped UI.
}
