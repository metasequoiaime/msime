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
  // ended：上屏或取消之后的那一键，Engine 的 view 里组字已经结束（editing_text 为空）。
  auto publish = [&](const FocusLease &lease, uint64_t generation,
                     bool uiless = false, uint32_t metadata = 0, bool ended = false) {
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
                                 {"editing_text", ended ? "" : "U4"},
                                 {"preedit", ended ? "" : "U4"},
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
  // 组字正常结束（上屏、取消）的快照带着可以播退场的原因，随后跟来的隐藏只是确认，不改它。
  require(publish(first, 1, false, 0, true));
  const auto ended = mailbox.snapshot(gate);
  require(ended && !ended->visible && ended->composition_ended);
  require(visual_event(first, FanyImePipeEventType::HideCandidateWnd));
  require(mailbox.snapshot(gate) && mailbox.snapshot(gate)->composition_ended);
  require(publish(first, 1));
  require(mailbox.snapshot(gate)->visible && !mailbox.snapshot(gate)->composition_ended);
  // 组字还开着时宿主发来的隐藏（焦点变化之类）要立即收起，不播退场。
  require(visual_event(first, FanyImePipeEventType::HideCandidateWnd));
  const auto immediate = mailbox.snapshot(gate);
  require(immediate && !immediate->visible && immediate->preedit.empty() &&
          !immediate->composition_ended);
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
    // 切到英文（IMESwitch）和上屏一样算正常结束，可以播退场；状态快照和焦点恢复是焦点转移，立即收起。
    require(hidden_now && !hidden_now->visible && hidden_now->preedit.empty() &&
            hidden_now->composition_ended ==
                (mode_event == FanyImePipeEventType::IMESwitch));
    require(visual_event(first, mode_event, 0, 1));
    require(visual_event(first, FanyImePipeEventType::ShowCandidateWnd));
    require(!mailbox.snapshot(gate)->visible);
  }
  // 读快照读不到既可能是没抢到锁也可能是焦点换走了；still_current 不加锁地分清后者，候选窗据此决定退场要不要播完。
  require(gate.still_current(first) && !gate.still_current(FocusLease{}));
  {
    // 闸门被占着时 try_with_active 读不到，still_current 照样答得出租约仍是当前焦点。
    std::promise<void> held, release;
    auto holder = std::async(std::launch::async, [&] {
      gate.with_active(first, [&] {
        held.set_value();
        release.get_future().wait();
      });
    });
    held.get_future().wait();
    require(!gate.try_with_active(first, [] {}) && gate.still_current(first));
    release.set_value();
    holder.get();
  }
  const auto pending = gate.begin(b, 2);
  // 新的激活一开始，旧租约就不再是当前焦点，哪怕新租约还没确认。
  require(pending.has_value() && !mailbox.snapshot(gate) && !gate.still_current(first) &&
          gate.still_current(pending->pending));
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
  // 宿主接管绘制（UILess）的快照立即收起。
  require(hidden && !hidden->visible && hidden->preedit.empty() && !hidden->composition_ended);
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
  require(!mailbox.snapshot(gate) && !gate.still_current(next));
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
  // 读音只用于显示：单独到达时挂到眼下的候选上并换一个快照编号让候选窗重画；读音只跟着算它时的那条释义；偏好变了就摘掉。
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
    readings["你好"] = {"hello", "/həˈləʊ/"};
    mailbox.readings(relaunched, readings);
    const auto after = mailbox.snapshot(gate);
    require(after && after->render_serial > before->render_serial);
    require(after->candidates[0].pronunciation == "/həˈləʊ/" &&
            candidate_secondary_text(after->candidates[0]) == "hello\n/həˈləʊ/");
    // 释义变了的候选不挂旧读音。
    readings["你好"].translation = "hi";
    mailbox.readings(relaunched, readings);
    require(mailbox.snapshot(gate)->candidates[0].pronunciation.empty());
    // 下一个键的快照同样挂得上，直到偏好变化把它们摘掉。
    mailbox.readings(relaunched, {{"你好", {"hello", "/həˈləʊ/"}}});
    packet.request_id = reply.source.request_id = 213;
    require(gate.with_active(relaunched,
                             [&] { mailbox.delivered(relaunched, reply, packet); }));
    require(mailbox.snapshot(gate)->candidates[0].pronunciation == "/həˈləʊ/");
    const auto serial = mailbox.snapshot(gate)->render_serial;
    mailbox.clear_readings();
    const auto cleared = mailbox.snapshot(gate);
    require(cleared->candidates[0].pronunciation.empty() && cleared->render_serial > serial);
  }
  // 组字锚点只认 Show/Move 报来的坐标：按键包的 x/y 是线上结构的默认值 (100,100)，不能拿来摆输入模式提示。锚点在同一租约里跨按键沿用，新租约从没有开始。
  require(!mailbox.snapshot(gate)->caret_anchor_known);
  require(publish(relaunched, 214));
  require(!mailbox.snapshot(gate)->caret_anchor_known);
  require(visual_event(relaunched, FanyImePipeEventType::MoveCandidateWnd));
  {
    const auto moved = mailbox.snapshot(gate);
    require(moved->caret_anchor_known && moved->caret_anchor_x == -200 &&
            moved->caret_anchor_y == 300);
  }
  require(publish(relaunched, 215));
  {
    // 按键包的 (100,100) 也不能拿来摆卡片：随后的 Show/Move 到达之前候选窗刷新一次，卡片就会跳到屏幕左上。同一租约里沿用 Move 报过的位置。
    const auto typed = mailbox.snapshot(gate);
    require(typed->x == -200 && typed->y == 300 && typed->caret_anchor_known &&
            typed->caret_anchor_x == -200 && typed->caret_anchor_y == 300);
  }
  // 宿主报的是 INVALID_Y（还没有文字范围）时，下一个按键同样不能用默认值把藏着的卡片顶出来。
  {
    FanyImeNamedpipeData move{};
    move.client_id = relaunched.transport.client;
    move.event_type = FanyImePipeEventType::MoveCandidateWnd;
    move.point[0] = 0;
    move.point[1] = invalid_candidate_anchor_y;
    require(gate.with_active(relaunched, [&] { mailbox.event(relaunched, move); }));
  }
  require(publish(relaunched, 215));
  {
    const auto unanchored = mailbox.snapshot(gate);
    require(unanchored->visible && unanchored->y == invalid_candidate_anchor_y &&
            unanchored->caret_anchor_known && unanchored->caret_anchor_x == -200 &&
            unanchored->caret_anchor_y == 300);
  }
  // 游戏会话的按键包带着 TIP 补上的锚点，照用包里的坐标。
  {
    FanyImeNamedpipeData packet{};
    packet.client_id = relaunched.transport.client;
    packet.request_id = 215;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.modifiers_down = PipeMetadata::GameHost;
    packet.point[0] = 640;
    packet.point[1] = 480;
    PendingReply reply{};
    reply.source.client_id = packet.client_id;
    reply.source.activation_epoch = relaunched.epoch;
    reply.source.request_id = packet.request_id;
    reply.source.transition = {{"view",
                                {{"session", 1},
                                 {"generation", 215},
                                 {"focused", true},
                                 {"editing_text", "U4"},
                                 {"preedit", "U4"},
                                 {"candidates", nlohmann::json::array()}}}};
    require(gate.with_active(relaunched,
                             [&] { mailbox.delivered(relaunched, reply, packet); }));
    const auto game = mailbox.snapshot(gate);
    require(game->x == 640 && game->y == 480);
  }
  // 回到 -200,300，下面的释义刷新照旧从这里继续。
  require(visual_event(relaunched, FanyImePipeEventType::MoveCandidateWnd));
  // 释义或云候选到达时整个展示按 Engine 视图重建，锚点也要跟着留下：真机上打开英文释义后，提示每次都退到了屏幕下方居中。
  mailbox.translations(relaunched, {{"session", 1},
                                    {"generation", 215},
                                    {"focused", true},
                                    {"editing_text", "U4"},
                                    {"preedit", "U4"},
                                    {"candidates", nlohmann::json::array()}});
  {
    const auto glossed = mailbox.snapshot(gate);
    require(glossed->caret_anchor_known && glossed->caret_anchor_x == -200 &&
            glossed->caret_anchor_y == 300);
  }
  // 画着的是 Ctrl+Enter 的释义页时，同一代次的释义刷新和换了代次的云候选、AI 候选都不能拿 Engine 的列表把它盖掉：释义页在 Server 里还开着，数字键、空格上屏的是它的义项。按键把释义页关掉时，那个按键的投递照常换回 Engine 的列表。
  {
    const auto sense = [](size_t index, const char *text) {
      return nlohmann::json{{"id", {{"session", 1}, {"generation", 300}, {"index", index}}},
                            {"text", text},
                            {"highlighted", index == 0}};
    };
    FanyImeNamedpipeData packet{};
    packet.client_id = relaunched.transport.client;
    packet.request_id = 300;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    PendingReply reply{};
    reply.source.client_id = packet.client_id;
    reply.source.activation_epoch = relaunched.epoch;
    reply.source.request_id = packet.request_id;
    reply.source.transition = {{"view",
                                {{"session", 1},
                                 {"generation", 300},
                                 {"focused", true},
                                 {"editing_text", "U4"},
                                 {"preedit", "U4"},
                                 {"translation_page", 7},
                                 {"candidates", nlohmann::json::array(
                                                    {sense(0, "centre"), sense(1, "middle")})}}}};
    require(gate.with_active(relaunched,
                             [&] { mailbox.delivered(relaunched, reply, packet); }));
    const auto engine_view = [](uint64_t generation) {
      return nlohmann::json{{"session", 1},
                            {"generation", generation},
                            {"focused", true},
                            {"editing_text", "U4"},
                            {"preedit", "U4"},
                            {"candidates", nlohmann::json::array()}};
    };
    const auto shown_page = [&] {
      const auto value = mailbox.snapshot(gate);
      return value && value->translation_page == 7 && value->candidates.size() == 2 &&
             value->candidates[1].text == "middle";
    };
    require(shown_page());
    mailbox.translations(relaunched, engine_view(300));
    require(shown_page());
    mailbox.online(relaunched, engine_view(301));
    require(shown_page());
    mailbox.action(relaunched, engine_view(302));
    require(shown_page());
    // 关掉释义页的按键带回 Engine 的列表。
    require(publish(relaunched, 303));
    require(mailbox.snapshot(gate)->translation_page == 0);
  }
  const auto anchored_again = activate(replacement, 6);
  require(publish(anchored_again, 216));
  require(!mailbox.snapshot(gate)->caret_anchor_known);
  mailbox.stop();
  require(publish(anchored_again, 217));
  require(
      !mailbox.snapshot(gate)); // A late delivery cannot reopen a stopped UI.
  // 卷轴展开：候选窗点开「∨」后，Server 为眼下这一代取完整候选列表，快照只在同一代时挂着它；换了代次就要再取，只补上释义的同一代不用；收起、隐藏、断开时清掉。
  {
    FocusGate expansion_gate;
    CandidateMailbox expansions;
    const auto change = expansion_gate.begin(a, 41);
    require(change.has_value() &&
            expansion_gate.acknowledge(change->pending, [] { return true; }));
    const auto lease = change->pending;
    const auto view = [](uint64_t generation) {
      nlohmann::json candidates = nlohmann::json::array();
      for (size_t index = 0; index < 2; ++index)
        candidates.push_back({{"id", {{"session", 1}, {"generation", generation}, {"index", index}}},
                              {"text", index == 0 ? "first" : "second"},
                              {"highlighted", index == 0}});
      return nlohmann::json{{"session", 1},     {"generation", generation},
                            {"focused", true},  {"editing_text", "ni"},
                            {"preedit", "ni"},  {"page", 0},
                            {"page_count", 3},  {"page_size", 2},
                            {"candidates", candidates}};
    };
    const auto deliver = [&](uint64_t generation, bool traditional = false) {
      FanyImeNamedpipeData packet{};
      packet.client_id = lease.transport.client;
      packet.request_id = generation;
      packet.event_type = FanyImePipeEventType::KeyEvent;
      PendingReply reply{};
      reply.source.client_id = packet.client_id;
      reply.source.activation_epoch = lease.epoch;
      reply.source.request_id = packet.request_id;
      reply.source.transition = {{"view", view(generation)}};
      reply.traditional_output = traditional;
      return expansion_gate.with_active(lease,
                                        [&] { expansions.delivered(lease, reply, packet); });
    };
    const auto snapshot = [](uint64_t generation, size_t count) {
      nlohmann::json candidates = nlohmann::json::array();
      for (size_t index = 0; index < count; ++index)
        candidates.push_back({{"id", {{"session", 1}, {"generation", generation}, {"index", index}}},
                              {"text", "w" + std::to_string(index)},
                              {"source", index == 3 ? 12 : 0}});
      return nlohmann::json{{"session", 1}, {"generation", generation}, {"candidates", candidates}};
    };
    require(deliver(50));
    auto shown = expansions.snapshot(expansion_gate);
    require(shown && shown->page_size == 2 && !shown->expanded && !shown->expansion_open);
    // 没打开时不取。
    require(!expansions.expansion_due(lease));
    expansions.expansion(lease, true);
    require(expansions.expansion_due(lease));
    // 快照带着 Server 这一侧开没开，候选窗按它收起。
    require(expansions.snapshot(expansion_gate)->expansion_open);
    // 解析：身份对不上的整份不要，占位行跳过。
    require(expanded_candidates_from_snapshot(snapshot(50, 6), 1, 51).empty());
    auto items = expanded_candidates_from_snapshot(snapshot(50, 6), 1, 50);
    require(items.size() == 5 && items[3].index == 4 && items[3].text == "w4");
    // 别的代次的列表不留。
    expansions.expanded(lease, 1, 49, items);
    require(expansions.expansion_due(lease) && !expansions.snapshot(expansion_gate)->expanded);
    const auto serial = expansions.snapshot(expansion_gate)->render_serial;
    expansions.expanded(lease, 1, 50, items);
    shown = expansions.snapshot(expansion_gate);
    require(shown->expanded && shown->expanded->size() == 5 && shown->render_serial > serial);
    require(!expansions.expansion_due(lease));
    // 同一代补上释义：列表还在，不用再取。
    expansions.translations(lease, view(50));
    require(expansions.snapshot(expansion_gate)->expanded && !expansions.expansion_due(lease));
    // 接着打字换了代次：这一页没有网格，直到为新的一代取到。
    require(deliver(51));
    require(!expansions.snapshot(expansion_gate)->expanded && expansions.expansion_due(lease));
    expansions.expanded(lease, 1, 51, expanded_candidates_from_snapshot(snapshot(51, 4), 1, 51));
    require(expansions.snapshot(expansion_gate)->expanded != nullptr);
    // 切换简繁只换投影、不换代次：手里的列表是另一种字形，不挂到快照上，要再取；按新的字形取到后才挂上。
    require(deliver(51, true));
    require(!expansions.snapshot(expansion_gate)->expanded && expansions.expansion_due(lease));
    expansions.expanded(lease, 1, 51, expanded_candidates_from_snapshot(snapshot(51, 4), 1, 51));
    require(expansions.snapshot(expansion_gate)->expanded != nullptr &&
            !expansions.expansion_due(lease));
    require(deliver(51, false));
    require(!expansions.snapshot(expansion_gate)->expanded && expansions.expansion_due(lease));
    expansions.expanded(lease, 1, 51, expanded_candidates_from_snapshot(snapshot(51, 4), 1, 51));
    // 别的租约既不取，也拿不到。
    auto other = lease;
    ++other.token;
    require(!expansions.expansion_due(other));
    // 收起时清掉。
    expansions.expansion(lease, false);
    require(!expansions.expansion_due(lease) && !expansions.snapshot(expansion_gate)->expanded);
    // 候选窗隐藏（上屏或取消）时也清掉。
    expansions.expansion(lease, true);
    expansions.expanded(lease, 1, 51, expanded_candidates_from_snapshot(snapshot(51, 4), 1, 51));
    FanyImeNamedpipeData hide{};
    hide.client_id = lease.transport.client;
    hide.event_type = FanyImePipeEventType::HideCandidateWnd;
    require(expansion_gate.with_active(lease, [&] { expansions.event(lease, hide); }));
    require(deliver(52));
    // 候选窗还以为开着时，下一帧的快照告诉它 Server 已经收起（上屏后马上接着打字，候选窗没来得及收起）。
    require(!expansions.expansion_due(lease) && !expansions.snapshot(expansion_gate)->expanded &&
            !expansions.snapshot(expansion_gate)->expansion_open);
    // 断开时清掉。
    expansions.expansion(lease, true);
    require(expansions.expansion_due(lease));
    expansions.disconnected(a);
    require(!expansions.expansion_due(lease));
  }
  // 上屏后联想：送达时先藏着，等 TIP 在光标处发 Show 才露出来；Server 会话留着联想时的隐藏（internal_prediction_hide）只是先藏起来，内容留给随后的 Show；普通隐藏照旧清空。
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
}
