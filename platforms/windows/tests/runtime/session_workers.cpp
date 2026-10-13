#include "SessionController.h"
#include "SessionWorkers.h"
#include <algorithm>
#include <chrono>
#include <map>

using namespace msime::windows;
namespace {
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Session worker test failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)
class IdleTransport final : public MainTransport {
public:
  std::atomic<int> write_failure{0};
  std::vector<std::pair<uint32_t, std::vector<uint8_t>>> writes() {
    std::lock_guard lock(mutex_);
    return writes_;
  }
  void add(const PipeTicket &ticket) {
    std::lock_guard lock(mutex_);
    current_[ticket.client] = ticket;
    packets_[ticket.client].clear();
    ready_.notify_all();
  }
  bool current(const PipeTicket &ticket) override {
    std::lock_guard lock(mutex_);
    return matches(ticket);
  }
  bool try_current(const PipeTicket &ticket) override {
    std::unique_lock lock(mutex_, std::try_to_lock);
    return lock && matches(ticket);
  }
  void hold_registration(std::promise<void> &entered,
                         std::future<void> &release) {
    std::lock_guard lock(mutex_);
    entered.set_value();
    require(release.wait_for(std::chrono::seconds(10)) ==
            std::future_status::ready);
  }
  std::optional<FanyImeNamedpipeData> read(const PipeTicket &ticket) override {
    std::unique_lock lock(mutex_);
    if (!matches(ticket))
      return std::nullopt;
    started_.push_back({ticket, std::this_thread::get_id()});
    ++reading_;
    peak_ = std::max(peak_, reading_);
    ready_.notify_all();
    ready_.wait(lock, [&] {
      return !matches(ticket) || !packets_[ticket.client].empty();
    });
    --reading_;
    if (!matches(ticket))
      return std::nullopt;
    auto packet = packets_[ticket.client].front();
    packets_[ticket.client].pop_front();
    return packet;
  }
  KeyEventSendResult send(const PipeTicket &ticket, uint32_t role,
                          const std::vector<uint8_t> &bytes) override {
    std::lock_guard lock(mutex_);
    if (!matches(ticket)) return KeyEventSendResult::DefinitelyNotSent;
    writes_.emplace_back(role, bytes);
    if (write_failure == 2)
      throw std::runtime_error("Synthetic write failure");
    return write_failure == 0 ? KeyEventSendResult::Sent
                               : KeyEventSendResult::DeliveryAmbiguous;
  }
  void push(FanyImeNamedpipeData packet) {
    std::lock_guard lock(mutex_);
    packets_[packet.client_id].push_back(packet);
    ready_.notify_all();
  }
  void close(const PipeTicket &ticket) noexcept override {
    std::lock_guard lock(mutex_);
    if (matches(ticket))
      current_.erase(ticket.client);
    ready_.notify_all();
  }
  void wait_started(size_t count) {
    std::unique_lock lock(mutex_);
    require(ready_.wait_for(lock, std::chrono::seconds(10),
                            [&] { return started_.size() >= count; }));
  }
  std::thread::id reader(const PipeTicket &ticket) {
    std::lock_guard lock(mutex_);
    for (const auto &[started, thread] : started_)
      if (same_ticket(started, ticket))
        return thread;
    return {};
  }
  size_t peak() {
    std::lock_guard lock(mutex_);
    return peak_;
  }

private:
  bool matches(const PipeTicket &ticket) const {
    auto found = current_.find(ticket.client);
    return found != current_.end() && same_ticket(found->second, ticket);
  }
  std::mutex mutex_;
  std::vector<std::pair<uint32_t, std::vector<uint8_t>>> writes_;
  std::condition_variable ready_;
  std::map<uint64_t, PipeTicket> current_;
  std::map<uint64_t, std::deque<FanyImeNamedpipeData>> packets_;
  std::vector<std::pair<PipeTicket, std::thread::id>> started_;
  size_t reading_ = 0;
  size_t peak_ = 0;
};
} // namespace
void session_worker_tests(const std::string &options) {
  for (int write_failure : {0, 1, 2}) {
    IdleTransport transport;
    RegistrationInbox inbox(2);
    PipeTicket ticket{42, {1, 2, 3}};
    SessionController *owner = nullptr;
    std::atomic<size_t> observed{0};
    std::promise<void> ui_entered, release_ui;
    auto entered = ui_entered.get_future();
    auto release = release_ui.get_future();
    SessionPump::Presentation presentation;
    presentation.delivered = [&](const FocusLease &, const PendingReply &reply,
                                 const FanyImeNamedpipeData &) {
      bool rejected = false;
      try {
        (void)owner->candidate_view();
      } catch (const std::logic_error &) {
        rejected = true;
      }
      require(rejected);
      ++observed;
      if (reply.ui_selection) {
        ui_entered.set_value();
        require(release.wait_for(std::chrono::seconds(10)) ==
                std::future_status::ready);
      }
    };
    SessionController controller(
        transport, inbox, 1, 8, options,
        [](InputState &state, const FocusLease &lease,
           const FanyImeNamedpipeData &packet) {
          return state.configured_key(lease, packet, TsfPreeditStyle::Local,
                                      {});
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        [] { return true; }, [&] { transport.close(ticket); },
        std::chrono::milliseconds(10), {}, presentation);
    owner = &controller;
    require(!controller.candidate_view());
    transport.add(ticket);
    require(inbox.push(ticket));
    FanyImeNamedpipeData packet{};
    packet.client_id = ticket.client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    transport.push(packet);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 2;
    packet.keycode = 'U';
    packet.wch = 'U';
    packet.modifiers_down = 1;
    transport.push(packet);
    transport.wait_started(3);
    const auto first = controller.candidate_view();
    require(first && first->visible && observed == 1);
    packet.request_id = 3;
    packet.keycode = '4';
    packet.wch = '4';
    packet.modifiers_down = 0;
    transport.push(packet);
    transport.wait_started(4);
    const auto second = controller.candidate_view();
    require(second && second->visible && observed == 2);
    require(second->generation > first->generation);
    packet.event_type = FanyImePipeEventType::HideCandidateWnd;
    transport.push(packet);
    transport.wait_started(5);
    require(controller.candidate_view() &&
            !controller.candidate_view()->visible);
    const auto hidden_candidate = second->candidates.at(0);
    require(controller.request_selection(second->lease, hidden_candidate.session,
        hidden_candidate.generation, hidden_candidate.index, second->translation_page) == SelectionRequestResult::Rejected);
    packet.event_type = FanyImePipeEventType::MoveCandidateWnd;
    packet.point[0] = -250;
    packet.point[1] = 380;
    transport.push(packet);
    transport.wait_started(6);
    require(!controller.candidate_view()->visible &&
            controller.candidate_view()->x == -250);
    packet.event_type = FanyImePipeEventType::ShowCandidateWnd;
    transport.push(packet);
    transport.wait_started(7);
    const auto restored = controller.candidate_view();
    require(restored && !restored->visible && restored->preedit.empty() &&
            restored->candidates.empty() && restored->y == 380 &&
            restored->generation == second->generation);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    for (char c : std::string("U4e2d")) {
      ++packet.request_id;
      packet.keycode = c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c;
      packet.wch = c;
      packet.modifiers_down = c == 'U' ? 1 : 0;
      transport.push(packet);
    }
    transport.wait_started(12);
    const auto shown = controller.candidate_view();
    require(shown && shown->candidates.at(0).text == "中");
    const auto candidate = shown->candidates.at(0);
    {
      std::promise<void> held, release_probe;
      auto holding = held.get_future();
      auto resume = release_probe.get_future();
      auto registering = std::async(std::launch::async,
          [&] { transport.hold_registration(held, resume); });
      require(holding.wait_for(std::chrono::seconds(10)) ==
              std::future_status::ready);
      auto reading = std::async(std::launch::async,
                                [&] { return controller.candidate_view(); });
      const bool ready = reading.wait_for(std::chrono::seconds(1)) ==
                         std::future_status::ready;
      release_probe.set_value();
      registering.get();
      const auto while_busy = reading.get();
      require(ready && !while_busy);
      const auto recovered = controller.candidate_view();
      require(recovered && recovered->visible &&
              recovered->generation == shown->generation);
      require(transport.current(ticket) &&
              controller.failure() == ControllerFailure::None);
    }
    if (write_failure) {
      transport.write_failure = write_failure;
      require(controller.request_selection(shown->lease, candidate.session, candidate.generation, candidate.index, shown->translation_page) == SelectionRequestResult::Failed);
      require(observed == 7 && !transport.current(ticket) && !controller.candidate_view());
      controller.stop();
      require(controller.failure() != ControllerFailure::None);
      continue;
    }
    require(controller.request_selection(
                shown->lease, candidate.session, candidate.generation + 1,
                candidate.index, shown->translation_page) == SelectionRequestResult::Rejected);
    auto selection = std::async(std::launch::async, [&] {
      return controller.request_selection(shown->lease, candidate.session,
                                          candidate.generation,
                                          candidate.index,
                                          shown->translation_page);
    });
    require(entered.wait_for(std::chrono::seconds(10)) ==
            std::future_status::ready);
    const auto busy = controller.request_selection(
        shown->lease, candidate.session, candidate.generation, candidate.index,
        shown->translation_page);
    auto reading = std::async(std::launch::async,
                              [&] { return controller.candidate_view(); });
    const bool read_ready = reading.wait_for(std::chrono::seconds(1)) ==
                            std::future_status::ready;
    ++packet.request_id;
    packet.keycode = 'U';
    packet.wch = 'U';
    packet.modifiers_down = 1;
    transport.push(packet); // Must wait until the UI transaction finishes.
    release_ui.set_value();
    const auto during_delivery = reading.get();
    require(read_ready && !during_delivery);
    require(busy == SelectionRequestResult::Busy &&
            selection.get() == SelectionRequestResult::Sent);
    transport.wait_started(13);
    require(controller.failure() == ControllerFailure::None && observed == 9);
    const auto after_click = controller.candidate_view();
    require(after_click && after_click->visible &&
            after_click->generation > shown->generation);
    const auto writes = transport.writes();
    size_t commits = 0;
    for (const auto &write : writes)
      if (write.first == FanyImePipeRole::ToTsfWorkerThread &&
          write.second == ui_complete_selection("中")->worker)
        ++commits;
    require(commits == 1);
    transport.close(ticket);
    require(!controller.candidate_view());
    controller.stop();
    require(!controller.candidate_view());
  }
  for (auto notification : {FanyImePipeEventType::IMESwitch,
                            FanyImePipeEventType::DoubleSingleByteSwitch,
                            FanyImePipeEventType::StatusSnapshot,
                            FanyImePipeEventType::FocusRestored}) {
    IdleTransport transport;
    RegistrationInbox inbox(2);
    PipeTicket ticket{42, {1, 2, 3}};
    SessionController controller(
        transport, inbox, 1, 8, options,
        [](InputState &state, const FocusLease &lease,
           const FanyImeNamedpipeData &packet) {
          return state.configured_key(lease, packet, TsfPreeditStyle::Local, {});
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        [] { return true; }, [&] { transport.close(ticket); });
    transport.add(ticket);
    require(inbox.push(ticket));
    FanyImeNamedpipeData packet{};
    packet.client_id = ticket.client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    transport.push(packet);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 2;
    packet.keycode = 'U';
    packet.wch = 'U';
    packet.modifiers_down = 1;
    transport.push(packet);
    transport.wait_started(3);
    packet.request_id = 3;
    packet.keycode = '4';
    packet.wch = '4';
    packet.modifiers_down = 0;
    transport.push(packet);
    transport.wait_started(4);
    const auto before = controller.candidate_view();
    require(before && before->visible && !before->candidates.empty());
    const auto candidate = before->candidates.at(0);
    packet.event_type = FanyImePipeEventType::MoveCandidateWnd;
    packet.modifiers_down = FanyImePipeFlags::UiLess;
    packet.point[0] = 240;
    transport.push(packet);
    transport.wait_started(5);
    const auto host_drawn = controller.candidate_view();
    require(host_drawn && !host_drawn->visible && host_drawn->preedit.empty() &&
            host_drawn->candidates.empty() && host_drawn->x == 240);
    require(controller.request_selection(before->lease, candidate.session,
                candidate.generation, candidate.index,
                before->translation_page) ==
            SelectionRequestResult::Rejected);
    packet.modifiers_down = 0;
    transport.push(packet);
    transport.wait_started(6);
    require(!controller.candidate_view()->visible);
    packet.event_type = FanyImePipeEventType::ShowCandidateWnd;
    transport.push(packet);
    transport.wait_started(7);
    const auto returned = controller.candidate_view();
    require(returned && returned->visible &&
            returned->generation == before->generation &&
            returned->preedit == before->preedit);
    packet.event_type = notification;
    packet.request_id = 77;
    packet.keycode = 0;
    packet.wch = 0;
    packet.modifiers_down = 0;
    transport.push(packet);
    transport.wait_started(8);
    const auto disabled = controller.candidate_view();
    const auto disabled_mode = controller.mode_view();
    require(disabled_mode.has_value() && disabled.has_value());
    // Full/half-width is the one notification here that carries no CN/EN
    // state. InputState::synchronize_input_mode returns early for it without
    // touching input-enabled, and ModeMailbox sets only `fullwidth` - both say
    // so in their own comments, and ModePresentation's optionals exist exactly
    // so "unknown" is not reported as "off". Asserting chinese == false for it
    // contradicts all three, and toggling 全角 mid-composition dismissing the
    // candidate window would be a bug rather than the behaviour to pin.
    const bool carries_input_state =
        notification != FanyImePipeEventType::DoubleSingleByteSwitch;
    if (carries_input_state) {
      require(disabled_mode->chinese == false);
      require(!disabled->visible && disabled->preedit.empty() &&
              disabled->candidates.empty());
      require(controller.request_selection(before->lease, candidate.session,
                  candidate.generation, candidate.index,
                  before->translation_page) ==
              SelectionRequestResult::Rejected);
    } else {
      require(!disabled_mode->chinese.has_value());
      require(disabled_mode->fullwidth == false);
      // The composition is untouched, so everything below - which checks what
      // survives a teardown and how typing restarts after one - is about a
      // different event than this one.
      require(disabled->visible && disabled->preedit == before->preedit);
      require(transport.current(ticket) &&
              controller.failure() == ControllerFailure::None);
      controller.stop();
      continue;
    }
    packet.event_type = FanyImePipeEventType::ShowCandidateWnd;
    transport.push(packet);
    transport.wait_started(9);
    require(!controller.candidate_view()->visible);
    packet.event_type = notification;
    packet.keycode = 1;
    transport.push(packet);
    transport.wait_started(10);
    require(!controller.candidate_view()->visible);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 4;
    packet.keycode = 'U';
    packet.wch = 'U';
    packet.modifiers_down = 1;
    transport.push(packet);
    packet.request_id = 5;
    packet.keycode = '4';
    packet.wch = '4';
    packet.modifiers_down = 0;
    transport.push(packet);
    transport.wait_started(12);
    const auto fresh = controller.candidate_view();
    require(fresh && fresh->visible && fresh->generation > before->generation &&
            fresh->preedit == before->preedit);
    require(transport.current(ticket) &&
            controller.failure() == ControllerFailure::None);
    controller.stop();
  }
  {
    IdleTransport transport;
    RegistrationInbox inbox(2);
    PipeTicket ticket{42, {1, 2, 3}};
    SessionController controller(
        transport, inbox, 1, 8, options,
        [](InputState &state, const FocusLease &lease,
           const FanyImeNamedpipeData &packet) {
          return state.configured_key(lease, packet, TsfPreeditStyle::Local, {});
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        [] { return true; }, [&] { transport.close(ticket); });
    transport.add(ticket);
    require(inbox.push(ticket));
    FanyImeNamedpipeData packet{};
    packet.client_id = ticket.client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    packet.keycode = 1; // Activation owns rendering even without per-key flags.
    transport.push(packet);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 2;
    packet.keycode = 'U';
    packet.wch = 'U';
    packet.modifiers_down = 1;
    transport.push(packet);
    transport.wait_started(3);
    const auto hidden = controller.candidate_view();
    require(hidden && !hidden->visible && hidden->preedit.empty());
    // UILess editing replies to TSF instead of taking the local-only path.
    bool replied = false;
    for (const auto &write : transport.writes())
      replied |= write.first == FanyImePipeRole::ToTsf;
    require(replied);
    packet.event_type = FanyImePipeEventType::ShowCandidateWnd;
    packet.modifiers_down = 0;
    transport.push(packet);
    transport.wait_started(4);
    require(!controller.candidate_view()->visible);
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    packet.keycode = 0;
    transport.push(packet);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 3;
    packet.keycode = '4';
    packet.wch = '4';
    transport.push(packet);
    transport.wait_started(6);
    require(controller.candidate_view()->visible);
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    packet.keycode = 1;
    transport.push(packet);
    transport.wait_started(7);
    require(!controller.candidate_view()->visible);
    require(controller.failure() == ControllerFailure::None);
    packet.event_type = FanyImePipeEventType::ClientSuspended;
    transport.push(packet);
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 78;
    packet.keycode = 0;
    transport.push(packet);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 4;
    packet.keycode = 'U';
    packet.wch = 'U';
    packet.modifiers_down = 1;
    transport.push(packet);
    transport.wait_started(10);
    require(controller.candidate_view()->visible);
    controller.stop();
  }
  {
    // Ctrl+Enter 的释义页沿用 Engine 列表的 session 和 generation，义项序号也从 0 起。从列表那一帧点下去、在释义页打开之后才到的点击，不能上屏释义页同一位置的义项，候选动作也不能落到列表上；释义页自己的点击照常上屏它点的义项。
    IdleTransport transport;
    RegistrationInbox inbox(2);
    PipeTicket ticket{42, {1, 2, 3}};
    std::atomic<uint64_t> translate{0};
    std::atomic<bool> translated{false};
    SessionController controller(
        transport, inbox, 1, 8, options,
        [&](InputState &state, const FocusLease &lease,
            const FanyImeNamedpipeData &packet) {
          // 夹具没有释义服务：Ctrl+Enter 之前在输入线程上给这一代的「中」挂上三条义项，走的是 TranslationWorker 送达时的同一个入口。
          if (packet.keycode == 0x0D && translate)
            translated = state
                             .apply_translations(
                                 lease, translate,
                                 R"([{"text":"中","translation":"centre; middle; China"}])")
                             .has_value();
          return state.configured_key(lease, packet, TsfPreeditStyle::Local, {});
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        [] { return true; }, [&] { transport.close(ticket); });
    transport.add(ticket);
    require(inbox.push(ticket));
    FanyImeNamedpipeData packet{};
    packet.client_id = ticket.client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    transport.push(packet);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 1;
    for (char c : std::string("U4e2d")) {
      ++packet.request_id;
      packet.keycode = c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c;
      packet.wch = c;
      packet.modifiers_down = c == 'U' ? 1 : 0;
      transport.push(packet);
    }
    transport.wait_started(7);
    const auto listed = controller.candidate_view();
    require(listed && listed->visible && listed->translation_page == 0 &&
            !listed->candidates.empty() && listed->candidates.at(0).text == "中");
    // 用户瞄准的是列表的第一行「中」，点击按这一帧拼好。
    const auto aimed = listed->candidates.at(0);
    translate = listed->generation;
    ++packet.request_id;
    packet.keycode = 0x0D;
    packet.wch = 0;
    packet.modifiers_down = PipeMetadata::CandidateActive | 2u;
    transport.push(packet);
    transport.wait_started(8);
    require(translated);
    const auto page = controller.candidate_view();
    require(page && page->visible && page->translation_page != 0 &&
            page->session == listed->session &&
            page->generation == listed->generation &&
            page->candidates.size() == 3 &&
            page->candidates.at(0).text == "centre" &&
            page->candidates.at(0).index == aimed.index &&
            !page->candidates.at(0).actions_available);
    // 激活时 Server 已经往 worker 管道写过焦点栅栏，所以只看这之后有没有新写出的帧。
    const auto written = transport.writes().size();
    require(controller.request_selection(listed->lease, aimed.session,
                                         aimed.generation, aimed.index,
                                         listed->translation_page) ==
            SelectionRequestResult::Rejected);
    require(controller.request_candidate_action(
                listed->lease, aimed.session, aimed.generation, aimed.index,
                listed->translation_page, CandidateAction::Remove) ==
            CandidateActionRequestResult::Rejected);
    require(controller.request_candidate_action(
                page->lease, aimed.session, aimed.generation, aimed.index,
                page->translation_page, CandidateAction::Remove) ==
            CandidateActionRequestResult::Rejected);
    require(transport.writes().size() == written && transport.current(ticket) &&
            controller.failure() == ControllerFailure::None);
    // 释义页还开着，它自己的点击上屏点到的那条义项。
    const auto sense = page->candidates.at(1);
    require(controller.request_selection(page->lease, sense.session,
                                         sense.generation, sense.index,
                                         page->translation_page) ==
            SelectionRequestResult::Sent);
    size_t commits = 0;
    const auto writes = transport.writes();
    for (size_t at = written; at < writes.size(); ++at) {
      require(writes[at].second != ui_complete_selection("centre")->worker);
      commits += writes[at].first == FanyImePipeRole::ToTsfWorkerThread &&
                 writes[at].second == ui_complete_selection("middle")->worker;
    }
    require(commits == 1 && controller.failure() == ControllerFailure::None);
    controller.stop();
  }
  {
    // 释义页上翻页不起作用，和方向键、翻页键在释义页上一样。曾经鼠标箭头和滚轮会翻底下的 Engine 列表：候选窗换成列表的下一页，释义页却还开着，接下来的数字键上屏的是看不见的义项。
    IdleTransport transport;
    RegistrationInbox inbox(2);
    PipeTicket ticket{42, {1, 2, 3}};
    std::atomic<uint64_t> translate{0};
    std::atomic<bool> translated{false};
    std::string translate_text;
    SessionController controller(
        transport, inbox, 1, 8, options,
        [&](InputState &state, const FocusLease &lease,
            const FanyImeNamedpipeData &packet) {
          // 夹具没有释义服务：Ctrl+Enter 之前在输入线程上给高亮的候选挂上三条义项，和上一段一样。
          if (packet.keycode == 0x0D && translate)
            translated = state
                             .apply_translations(
                                 lease, translate,
                                 nlohmann::json::array(
                                     {{{"text", translate_text},
                                       {"translation", "centre; middle; China"}}})
                                     .dump())
                             .has_value();
          return state.configured_key(lease, packet, TsfPreeditStyle::Local, {});
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        [] { return true; }, [&] { transport.close(ticket); });
    transport.add(ticket);
    require(inbox.push(ticket));
    FanyImeNamedpipeData packet{};
    packet.client_id = ticket.client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    transport.push(packet);
    // Shift+T 进日期时间模式，rq 列出几种日期写法；每页两个候选，所以有好几页，不需要词库。
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 1;
    for (char c : std::string("Trq")) {
      ++packet.request_id;
      packet.keycode = c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c;
      packet.wch = c;
      packet.modifiers_down = c == 'T' ? 1 : 0;
      transport.push(packet);
    }
    transport.wait_started(5);
    const auto listed = controller.candidate_view();
    require(listed && listed->visible && listed->translation_page == 0 &&
            listed->page_count > 1 && !listed->candidates.empty());
    for (const auto &candidate : listed->candidates)
      if (candidate.highlighted)
        translate_text = candidate.text;
    require(!translate_text.empty());
    translate = listed->generation;
    ++packet.request_id;
    packet.keycode = 0x0D;
    packet.wch = 0;
    packet.modifiers_down = PipeMetadata::CandidateActive | 2u;
    transport.push(packet);
    transport.wait_started(6);
    require(translated);
    const auto page = controller.candidate_view();
    // 释义页只有一页，不画 Engine 列表的页码和翻页箭头。
    require(page && page->visible && page->translation_page != 0 &&
            page->generation == listed->generation && page->candidates.size() == 3 &&
            page->candidates.at(1).text == "middle" && page->page_count == 0);
    // 按列表那一帧发起、在释义页打开之后才到的翻页（箭头或滚轮），和从释义页那一帧发起的翻页，都不起作用。
    CandidatePage from_list{listed->lease, listed->session, listed->generation, false, 1u};
    require(controller.request_page(from_list) == CandidatePageRequestResult::Rejected);
    CandidatePage from_page{page->lease, page->session, page->generation, false, 1u, true};
    from_page.translation_page = page->translation_page;
    require(controller.request_page(from_page) == CandidatePageRequestResult::Rejected);
    const auto still = controller.candidate_view();
    require(still && still->visible && still->translation_page == page->translation_page &&
            still->generation == page->generation && still->candidates.at(1).text == "middle");
    // 数字键上屏的是画着的那一页上的义项。
    const auto written = transport.writes().size();
    ++packet.request_id;
    packet.keycode = '2';
    packet.wch = '2';
    packet.modifiers_down = PipeMetadata::CandidateActive;
    transport.push(packet);
    transport.wait_started(7);
    const auto expected = *wire_bytes(exact_commit(packet.request_id, "middle"));
    size_t commits = 0;
    const auto writes = transport.writes();
    for (size_t at = written; at < writes.size(); ++at)
      commits += writes[at].first == FanyImePipeRole::ToTsf &&
                 writes[at].second == std::vector<uint8_t>(expected.begin(), expected.end());
    require(commits == 1 && controller.failure() == ControllerFailure::None);
    controller.stop();
  }
  FocusGate gate;
  InputQueue queue(gate, 2, 16, options);
  IdleTransport transport;
  auto key = [](InputState &, const FocusLease &,
                const FanyImeNamedpipeData &) -> std::optional<PendingReply> {
    return std::nullopt;
  };
  auto event = [](const FocusRoute &, const FanyImeNamedpipeData &) {
    return true;
  };
  SessionWorkers workers(transport, queue, gate, 2, key, event);
  PipeTicket a{42, {1, 2, 3}}, b{43, {4, 5, 6}}, c{44, {7, 8, 9}};
  transport.add(a);
  transport.add(b);
  require(workers.submit(a) && workers.submit(a) && workers.submit(b));
  transport.wait_started(2);
  require(workers.stats().active == 2 && workers.stats().pending == 0);
  transport.add(c);
  require(!workers.submit(c) && !transport.current(c));
  PipeTicket replacement{42, {10, 2, 3}};
  transport.add(replacement);
  require(workers.submit(replacement));
  transport.wait_started(3);
  require(transport.reader(a) == transport.reader(replacement));
  require(!workers.submit(a) && transport.current(replacement));
  auto forbidden = queue.submit([&](InputState &) {
    bool rejected = false;
    try {
      workers.stop();
    } catch (const std::logic_error &) {
      rejected = true;
    }
    require(rejected);
  });
  require(forbidden && forbidden->get() == InputTaskStatus::Completed);
  PipeTicket latest{42, {12, 2, 3}};
  {
    struct Release {
      std::promise<void> promise;
      bool done = false;
      void release() {
        if (!done) {
          done = true;
          promise.set_value();
        }
      }
      ~Release() { release(); }
    } release;
    auto resume = release.promise.get_future().share();
    std::promise<void> entered;
    auto blocked = queue.submit([&, resume](InputState &) {
      entered.set_value();
      resume.wait(); // Test-only barrier, not production queue behavior.
    });
    require(blocked.has_value());
    entered.get_future().wait();
    PipeTicket superseded{42, {11, 2, 3}};
    transport.add(superseded);
    require(workers.submit(superseded));
    transport.add(latest);
    require(workers.submit(latest));
    require(workers.stats().active == 2 && workers.stats().pending == 1);
    require(transport.current(latest) && !transport.current(superseded));
    release.release();
    require(blocked->get() == InputTaskStatus::Completed);
    transport.wait_started(4);
    require(transport.reader(superseded) == std::thread::id{} &&
            transport.reader(latest) == transport.reader(replacement));
  }
  auto stopping = std::async(std::launch::async, [&] { workers.stop(); });
  workers.stop();
  stopping.get();
  const auto stats = workers.stats();
  require(stats.active == 0 && stats.pending == 0 && stats.finished == 4 &&
          stats.failed == 0 && stats.stopping && transport.peak() == 2);
  require(!transport.current(latest) && !transport.current(b));
  require(queue.stats().accepting); // Pumps finish cleanup before queue stop.
  transport.add(c);
  require(!workers.submit(c) && !transport.current(c));
  queue.stop();
  for (int mode = 0; mode < 3; ++mode) {
    IdleTransport mode_transport;
    RegistrationInbox inbox(2);
    std::promise<FocusLease> activated;
    auto activation = activated.get_future();
    SessionController *owner = nullptr;
    SessionController controller(
        mode_transport, inbox, 1, 8, options,
        [](InputState &, const FocusLease &, const FanyImeNamedpipeData &)
            -> std::optional<PendingReply> { return std::nullopt; },
        [&](const FocusRoute &route, const FanyImeNamedpipeData &packet) {
          if (packet.event_type == FanyImePipeEventType::ClientActivated) {
            bool rejected = false;
            try {
              owner->request_mode(*route.route, WorkerMode::Fullwidth);
            } catch (const std::logic_error &) {
              rejected = true;
            }
            require(rejected); // No recursive focus lock from event callback.
            if (packet.request_id == 77)
              activated.set_value(*route.route);
          }
          return true;
        },
        [] { return true; }, [&] { mode_transport.close(a); });
    owner = &controller;
    require(controller.request_mode({}, WorkerMode::Fullwidth) ==
            ModeRequestResult::Rejected);
    mode_transport.add(a);
    require(inbox.push(a));
    FanyImeNamedpipeData packet{};
    packet.client_id = a.client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    mode_transport.push(packet);
    require(activation.wait_for(std::chrono::seconds(10)) ==
            std::future_status::ready);
    const auto lease = activation.get();
    mode_transport.wait_started(2); // Activation transaction has released its lock.
    const auto initial_mode = controller.mode_view();
    require(initial_mode && !initial_mode->chinese &&
            !initial_mode->chinese_punctuation && !initial_mode->fullwidth &&
            !controller.candidate_view());
    packet.event_type = FanyImePipeEventType::StatusSnapshot;
    packet.keycode = 1;
    packet.modifiers_down = 1;
    packet.pinyin_length = 1;
    mode_transport.push(packet);
    mode_transport.wait_started(3);
    const auto known_mode = controller.mode_view();
    require(known_mode && known_mode->chinese == true &&
            known_mode->chinese_punctuation == true &&
            known_mode->fullwidth == true);
    packet.event_type = FanyImePipeEventType::PuncSwitch;
    packet.keycode = 0;
    mode_transport.push(packet);
    packet.event_type = FanyImePipeEventType::DoubleSingleByteSwitch;
    mode_transport.push(packet);
    mode_transport.wait_started(5);
    const auto changed_mode = controller.mode_view();
    require(changed_mode && changed_mode->chinese == true &&
            changed_mode->chinese_punctuation == false &&
            changed_mode->fullwidth == false);
    auto stale = lease;
    ++stale.epoch;
    require(controller.request_mode(stale, WorkerMode::Fullwidth) ==
            ModeRequestResult::Rejected);
    stale = lease;
    ++stale.transport.generations[0];
    require(controller.request_mode(stale, WorkerMode::Fullwidth) ==
            ModeRequestResult::Rejected);
    require(controller.request_mode(lease, static_cast<WorkerMode>(99)) ==
            ModeRequestResult::Rejected);
    require(mode_transport.writes().size() == 1); // Only the initial focus fence.
    mode_transport.write_failure = mode;
    if (mode == 0) {
      for (auto command :
           {WorkerMode::English, WorkerMode::Chinese,
            WorkerMode::AsciiPunctuation, WorkerMode::ChinesePunctuation,
            WorkerMode::Fullwidth, WorkerMode::Halfwidth}) {
        require(controller.request_mode(lease, command) ==
                ModeRequestResult::Sent);
        const auto writes = mode_transport.writes();
        require(writes.back().first == FanyImePipeRole::ToTsfWorkerThread &&
                writes.back().second == *worker_mode_bytes(command));
      }
      require(mode_transport.writes().size() == 7);
      require(controller.mode_view()->chinese == true &&
              controller.mode_view()->fullwidth == false);
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.request_id = 78;
      packet.keycode = 0;
      mode_transport.push(packet);
      mode_transport.wait_started(6);
      const auto next_mode = controller.mode_view();
      require(next_mode && next_mode->lease.epoch != lease.epoch &&
              !next_mode->chinese && !next_mode->chinese_punctuation &&
              !next_mode->fullwidth);
      require(controller.request_mode(lease, WorkerMode::Chinese) ==
              ModeRequestResult::Rejected);
    } else {
      require(controller.request_mode(lease, WorkerMode::Fullwidth) ==
              ModeRequestResult::WriteFailed);
      require(!mode_transport.current(a));
      require(controller.request_mode(lease, WorkerMode::Fullwidth) ==
              ModeRequestResult::Rejected);
      require(mode_transport.writes().size() ==
              2); // No replay after uncertain write.
    }
    controller.stop();
    require(controller.request_mode(lease, WorkerMode::Chinese) ==
            ModeRequestResult::Rejected);
    require(!controller.mode_view());
  }
  for (int mode = 0; mode < 3; ++mode) {
    IdleTransport supervised;
    RegistrationInbox inbox(2);
    std::atomic<bool> healthy{true};
    std::promise<void> stopped;
    auto stopped_future = stopped.get_future();
    SessionController *owner = nullptr;
    SessionController controller(
        supervised, inbox, 2, 16, options,
        [](InputState &, const FocusLease &,
           const FanyImeNamedpipeData &) -> std::optional<PendingReply> {
          throw std::runtime_error("Synthetic supervised input failure");
        },
        [&](const FocusRoute &, const FanyImeNamedpipeData &) {
          if (mode == 2)
            owner->request_stop();
          return true;
        },
        [&] { return healthy.load(); },
        [&] {
          supervised.close(a);
          supervised.close(b);
          stopped.set_value();
        },
        std::chrono::milliseconds(10),
        nlohmann::json::parse(options).at("cache").get<std::string>());
    owner = &controller;
    const auto settings_deadline =
        std::chrono::steady_clock::now() + std::chrono::seconds(5);
    while (controller.preferences_status() !=
           PreferenceMonitorStatus::Current) {
      require(std::chrono::steady_clock::now() < settings_deadline);
      std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }
    supervised.add(a);
    supervised.add(b);
    require(inbox.push(a) && inbox.push(b));
    supervised.wait_started(2);
    if (mode != 0) {
      FanyImeNamedpipeData packet{};
      packet.client_id = a.client;
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.request_id = 77;
      supervised.push(packet);
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.request_id = 2;
      packet.keycode = 'U';
      packet.wch = 'U';
      if (mode == 1)
        supervised.push(packet);
    } else {
      healthy = false;
    }
    require(stopped_future.wait_for(std::chrono::seconds(10)) ==
            std::future_status::ready);
    controller.stop();
    require(inbox.closed() && !supervised.current(a) && !supervised.current(b));
    require(controller.failure() == (mode == 1   ? ControllerFailure::InputQueue
                                     : mode == 0 ? ControllerFailure::Service
                                                 : ControllerFailure::None));
  }
}
