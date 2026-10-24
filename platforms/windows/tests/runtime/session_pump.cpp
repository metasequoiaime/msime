#include "SessionPump.h"
#include "PipeMetadata.h"
#include "ipc_negotiation.h"
#include "UiSelectionDelivery.h"
#include "CandidatePresentation.h"
#include "PreviewDispatcher.h"
#include "ProductionDispatcher.h"
#include "preference_transition.h"
#include <cstring>
#include <fstream>
#include <filesystem>
#include <atomic>
#include <exception>

using namespace msime::windows;
namespace {
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Session pump test failed at line " + std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)
class FixtureTransport final : public MainTransport {
public:
  PipeTicket ticket{42, {1, 2, 3}};
  std::atomic<bool> open{true};
  std::vector<FanyImeNamedpipeData> packets;
  std::vector<std::pair<uint32_t, std::vector<uint8_t>>> writes;
  size_t next = 0;
  size_t fail_write = 0;
  bool throw_write = false;
  const std::thread::id io_thread = std::this_thread::get_id();
  bool current(const PipeTicket &value) override {
    return open && same_ticket(ticket, value);
  }
  bool try_current(const PipeTicket &value) override { return current(value); }
  std::optional<FanyImeNamedpipeData> read(const PipeTicket &value) override {
    require(std::this_thread::get_id() == io_thread);
    if (!current(value) || next == packets.size())
      return std::nullopt;
    return packets[next++];
  }
  KeyEventSendResult send(const PipeTicket &value, uint32_t role,
                          const std::vector<uint8_t> &frame) override {
    require(std::this_thread::get_id() == io_thread);
    if (!current(value))
      return KeyEventSendResult::DefinitelyNotSent;
    writes.emplace_back(role, frame);
    if (throw_write && writes.size() == fail_write)
      throw std::runtime_error("Synthetic write exception");
    return writes.size() != fail_write ? KeyEventSendResult::Sent
                                       : KeyEventSendResult::DeliveryAmbiguous;
  }
  void close(const PipeTicket &value) noexcept override {
    if (same_ticket(ticket, value))
      open = false;
  }
  FixtureTransport() {
    FanyImeNamedpipeData packet{};
    packet.client_id = ticket.client;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    packets.push_back(packet);
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 2;
    for (char c : std::string("U4e2d")) {
      packet.keycode =
          static_cast<uint32_t>(c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c);
      packet.wch = c;
      packet.modifiers_down = c == 'U' ? 1 : 0;
      packets.push_back(packet);
      ++packet.request_id;
    }
    packet.keycode = 0x20;
    packet.wch = 0;
    packets.push_back(packet);
  }
};
} // namespace
void grouped_panel_pump_tests(const std::string &options) {
  // 未协商、带修饰键或键码与组码不一致时，必须在输入线程接收之前拒绝。
  for (unsigned mode = 0; mode < 3; ++mode) {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    transport.ticket.capabilities = mode == 0 ? 0 : FanyImeProtocol::GroupedPanelKeys;
    transport.packets.resize(2);
    auto &packet = transport.packets[1];
    packet.keycode = mode == 2 ? 'N' : 'B';
    packet.wch = 'b';
    packet.modifiers_down = PipeMetadata::GroupedPanelKey | (mode == 1 ? 2u : 0u);
    unsigned keys = 0;
    SessionPump pump(transport, queue, gate,
      [&](InputState &, const FocusLease &, const FanyImeNamedpipeData &) -> std::optional<PendingReply> {
        ++keys;
        return std::nullopt;
      }, [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    require(pump.run(transport.ticket) == PumpResult::DispatchFailed && keys == 0);
    queue.stop();
  }
  // 协商完成后的组码走真实主通道与会话，不能被当成 UILess 候选页标志。
  {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    transport.ticket.capabilities = FanyImeProtocol::GroupedPanelKeys;
    transport.packets.resize(2);
    auto &packet = transport.packets[1];
    packet.keycode = 'B'; packet.wch = 'b';
    packet.modifiers_down = PipeMetadata::GroupedPanelKey;
    unsigned keys = 0;
    SessionPump pump(transport, queue, gate,
      [&](InputState &state, const FocusLease &lease, const FanyImeNamedpipeData &key) {
        ++keys;
        auto reply = state.configured_key(lease, key, TsfPreeditStyle::Local, {});
        require(reply && reply->source.transition.at("view").at("key_grid") == "eighteen_key");
        return reply;
      }, [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    require(pump.run(transport.ticket) == PumpResult::Disconnected && keys == 1);
    queue.stop();
  }
}
void ordered_preference_pump_tests(const std::string &options) {
 for (bool capable : {false, true}) for (unsigned scenario = 0; scenario < 3; ++scenario) {
  const auto directory = std::filesystem::temp_directory_path() /
      ("msime-ordered-preferences-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directories(directory);
  struct Cleanup {
    std::filesystem::path directory;
    ~Cleanup() { std::filesystem::remove_all(directory); }
  } cleanup{directory};
  auto document = nlohmann::json{{"format_version", 1}, {"revision", 42},
      {"preferences", nlohmann::json::parse(options).at("preferences")}};
  document["preferences"]["keybindings"]["toggle_japanese_ctrl_shift_j"] = true;
  { std::ofstream file(directory / "preferences.json"); file << document; }
  const auto snapshot = PreferenceSnapshot::load(directory.u8string());
  std::promise<std::optional<PreferenceSnapshot>> saved;
  auto completion = saved.get_future().share();
  if (scenario != 1) saved.set_value(scenario == 2 ? std::optional<PreferenceSnapshot>{} : snapshot);
  auto japanese_options = nlohmann::json::parse(options);
  japanese_options["preferences"]["scheme"] = "japanese";
  FocusGate gate;
  InputQueue queue(gate, 2, 8, japanese_options.dump());
  FixtureTransport transport;
  transport.ticket.capabilities = capable ? FanyImeProtocol::OrderedPreferences : 0;
  transport.packets.resize(3);
  transport.packets[1].keycode = 'J';
  transport.packets[1].wch = 'j';
  transport.packets[1].modifiers_down = 3;
  transport.packets[2].keycode = 'N';
  transport.packets[2].wch = 'n';
  transport.packets[2].modifiers_down = 0;
  bool next_key_used_saved_preferences = false;
  std::promise<void> submitted;
  auto submitted_future = submitted.get_future();
  std::future<bool> writer;
  if (scenario == 1) writer = std::async(std::launch::async, [&] {
    submitted_future.wait();
    auto unrelated = queue.submit([](InputState &) {});
    const bool available = unrelated && unrelated->get() == InputTaskStatus::Completed;
    saved.set_value(snapshot);
    return available;
  });
  SessionPump pump(transport, queue, gate,
    [&](InputState &state, const FocusLease &lease, const FanyImeNamedpipeData &packet) {
      if (packet.keycode == 'J') {
        auto reply = state.toggle_japanese(lease, packet, true, [] { return true; });
        require(reply.has_value());
        reply->preference_change = completion;
        submitted.set_value();
        return reply;
      }
      next_key_used_saved_preferences = state.japanese_toggle_shortcut_enabled();
      auto reply = state.configured_key(lease, packet, TsfPreeditStyle::Local, {});
      require(reply.has_value());
      require(reply->source.transition.at("view").value("scheme", 0u) ==
          (scenario == 2 ? static_cast<unsigned>(scheme::Japanese) : static_cast<unsigned>(scheme::Quanpin)));
      return reply;
    }, [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
  require(pump.run(transport.ticket) == PumpResult::Disconnected);
  if (writer.valid()) require(writer.get());
  queue.stop();
  require(next_key_used_saved_preferences == (scenario != 2));
  std::optional<FanyImePreferencePipe::Transition> receipt;
  for (const auto &write : transport.writes) {
    if (write.first != FanyImePipeRole::ToTsfWorkerThread ||
        write.second.size() != sizeof(FanyImeNamedpipeDataToTsfWorkerThread)) continue;
    FanyImeNamedpipeDataToTsfWorkerThread frame{};
    std::memcpy(&frame, write.second.data(), sizeof(frame));
    if (const auto decoded = FanyImePreferencePipe::parse(frame); decoded && decoded->request != 0) receipt = decoded;
  }
  require(receipt.has_value() == capable);
  if (capable) {
    require(transport.writes.size() >= 2);
    FanyImeNamedpipeDataToTsfWorkerThread before_ready{}, ready{};
    std::memcpy(&before_ready, transport.writes[0].second.data(), sizeof(before_ready));
    std::memcpy(&ready, transport.writes[1].second.data(), sizeof(ready));
    require(FanyImePreferencePipe::parse(before_ready)->request == 0 &&
        ready.msg_type == FanyImeWorkerReplyType::FocusSessionReady);
  }
  if (receipt) require(receipt->revision == (scenario == 2 ? 0 : 42) &&
      receipt->request == 2 && receipt->focus == 77 && receipt->epoch != 0);
 }
}
void session_pump_tests(const std::string &options) {
  ordered_preference_pump_tests(options);
  grouped_panel_pump_tests(options);
  // 真实 pump 的主通道：协商后的本地 UI-less 只收到导航意图；旧宿主仍收到候选页。
  for (bool capable : {false, true}) for (bool serverPage : {false, true}) {
    for (auto key : {0x28u, 0x22u}) {
      FocusGate gate;
      InputQueue queue(gate, 2, 8, options);
      FixtureTransport transport;
      transport.ticket.capabilities = capable ? FanyImeProtocol::HostOnlineCandidates : 0;
      transport.packets.back().keycode = key;
      for (size_t n = 1; n < transport.packets.size(); ++n)
        transport.packets[n].modifiers_down |= FanyImePipeFlags::UiLess |
            (serverPage ? FanyImePipeFlags::ServerUiLessCandidates : 0);
      SessionPump pump(transport, queue, gate,
        [](InputState &state, const FocusLease &lease, const FanyImeNamedpipeData &packet) {
          NavigationBindings bindings; bindings.arrows = true; bindings.page_up_down = true;
          return state.configured_key(lease, packet, TsfPreeditStyle::Local, bindings);
        }, [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
      require(pump.run(transport.ticket) == PumpResult::Disconnected);
      const auto &last = transport.writes.back();
      require(last.first == FanyImePipeRole::ToTsf && last.second.size() >= 4);
      const uint32_t opcode = last.second[0] | (uint32_t(last.second[1]) << 8) |
          (uint32_t(last.second[2]) << 16) | (uint32_t(last.second[3]) << 24);
      require(opcode == (capable && !serverPage ?
          (key == 0x28 ? FanyImeReplyType::MoveSelectionNext : FanyImeReplyType::MovePageNext) :
          FanyImeReplyType::UiLessComposition));
      queue.stop();
    }
  }
  for (bool throws : {false, true}) {
    for (bool partial : {false, true}) {
      for (size_t failure = 0; failure <= (partial ? 2u : 1u); ++failure) {
        FocusGate gate;
        FixtureTransport transport;
        const auto change = gate.begin(transport.ticket, 77);
        require(change &&
                gate.acknowledge(change->pending, [] { return true; }));
        const auto frames = partial ? ui_partial_selection("hao", "你", "你好")
                                    : ui_complete_selection("你好");
        require(frames.has_value());
        transport.fail_write = failure;
        transport.throw_write = throws;
        auto stale = change->pending;
        ++stale.epoch;
        require(deliver_ui_selection(transport, gate, stale, *frames) ==
                UiDeliveryResult::Stale);
        require(transport.writes.empty());
        const auto result =
            deliver_ui_selection(transport, gate, change->pending, *frames);
        require(result == (failure ? UiDeliveryResult::WriteFailed
                                   : UiDeliveryResult::Sent));
        require(transport.writes.size() == (failure   ? failure
                                            : partial ? 2u
                                                      : 1u));
        if (partial) {
          require(transport.writes[0].first == FanyImePipeRole::ToTsf);
          require(transport.writes[0].second ==
                  std::vector<uint8_t>(frames->before_trigger->begin(),
                                       frames->before_trigger->end()));
        }
        if (!failure) {
          require(transport.writes.back().first ==
                  FanyImePipeRole::ToTsfWorkerThread);
          require(transport.writes.back().second == frames->worker);
        } else {
          require(!transport.open);
          const auto writes = transport.writes.size();
          require(deliver_ui_selection(transport, gate, change->pending,
                                       *frames) == UiDeliveryResult::Stale);
          require(transport.writes.size() == writes);
        }
      }
    }
  }
  for (bool local : {false, true}) {
    for (bool uiless : {false, true}) {
      for (bool fail_write : {false, true}) {
        if (local && !uiless && fail_write)
          continue;
        FocusGate gate;
        InputQueue queue(gate, 2, 8, options);
        FixtureTransport transport;
        if (uiless)
          for (size_t i = 1; i < transport.packets.size(); ++i)
            transport.packets[i].modifiers_down |= FanyImePipeFlags::UiLess;
        if (fail_write)
          transport.fail_write = 3; // Activation fence, key fence, key reply.
        std::vector<CandidatePresentation> frames;
        size_t closed = 0;
        std::exception_ptr failure;
        SessionPump::Presentation presentation;
        presentation.delivered = [&](const FocusLease &lease,
                                     const PendingReply &reply,
                                     const FanyImeNamedpipeData &packet) {
          try {
            require(std::this_thread::get_id() != transport.io_thread);
            require(!transport.writes.empty());
            if (reply.encoded) {
              const auto expected = wire_bytes(*reply.encoded);
              require(expected && transport.writes.back().second ==
                                      std::vector<uint8_t>(expected->begin(),
                                                           expected->end()));
            } else {
              require(local && !uiless && packet.keycode != 0x20);
            }
            auto frame = candidate_presentation(lease, reply, packet);
            require(frame.visible == (!uiless && packet.keycode != 0x20));
            if (uiless)
              require(frame.preedit.empty() && frame.candidates.empty());
            if (frame.visible) {
              auto prefixed = reply;
              prefixed.next_prefix = "prefix";
              require(candidate_presentation(lease, prefixed, packet).preedit ==
                      "prefix" + frame.preedit);
              prefixed.next_prefix = std::string(4097, 'a');
              bool rejected = false;
              try {
                (void)candidate_presentation(lease, prefixed, packet);
              } catch (const std::invalid_argument &) {
                rejected = true;
              }
              require(rejected);
            }
            frames.push_back(std::move(frame));
          } catch (...) {
            failure = std::current_exception();
            throw;
          }
        };
        presentation.disconnected = [&](const PipeTicket &ticket) {
          require(std::this_thread::get_id() != transport.io_thread);
          require(same_ticket(ticket, transport.ticket));
          ++closed;
        };
        SessionPump pump(
            transport, queue, gate,
            [local](InputState &state, const FocusLease &lease,
                    const FanyImeNamedpipeData &packet) {
              return state.configured_key(
                  lease, packet,
                  local ? TsfPreeditStyle::Local : TsfPreeditStyle::Pinyin, {});
            },
            [](const FocusRoute &, const FanyImeNamedpipeData &) {
              return true;
            },
            presentation);
        const auto result = pump.run(transport.ticket);
        if (failure)
          std::rethrow_exception(failure);
        require(result == (fail_write ? PumpResult::WriteFailed
                                      : PumpResult::Disconnected));
        require(closed == 1 && frames.size() == (fail_write ? 0u : 6u));
        queue.stop();
      }
    }
  }
  {
    // TSF owns the pure-punctuation edit session even when no composition is
    // active. The configured production handler must still consume the
    // request and return the Engine's exact punctuation result; an empty
    // reply would make SessionPump disconnect the client.
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    transport.packets.resize(4);
    auto &first_shortcut = transport.packets[1];
    first_shortcut.keycode = 'F';
    first_shortcut.wch = 'f';
    first_shortcut.modifiers_down = 0b11;
    first_shortcut.request_id = 2;
    auto &busy_shortcut = transport.packets[2];
    busy_shortcut = first_shortcut;
    busy_shortcut.request_id = 3;
    auto &punctuation = transport.packets.back();
    punctuation.keycode = 0xBC;
    punctuation.wch = ',';
    punctuation.request_id = 4;
    size_t persist_calls = 0, shortcut_deliveries = 0;
    const auto production = production_key_handler([&](bool desired) {
      require(desired == (persist_calls == 0));
      ++persist_calls;
      return persist_calls == 1;
    });
    SessionPump::Presentation presentation;
    presentation.delivered = [&](const FocusLease &, const PendingReply &reply,
                                 const FanyImeNamedpipeData &packet) {
      if (!FanyImeProtocol::IsCharacterSetShortcut(packet.keycode,
                                                   packet.modifiers_down))
        return;
      ++shortcut_deliveries;
      require(!reply.encoded && reply.traditional_output);
    };
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &lease,
            const FanyImeNamedpipeData &packet) {
          if (FanyImeProtocol::IsCharacterSetShortcut(packet.keycode,
                                                      packet.modifiers_down))
            return production(state, lease, packet);
          auto reply = state.configured_key(
              lease, packet, TsfPreeditStyle::Local, {});
          require(reply && reply->encoded && static_cast<bool>(*reply->encoded));
          require(reply->encoded->packet.msg_type ==
                  FanyImeReplyType::CommitExactText);
          require(reply->source.transition.at("commit") == "，");
          return reply;
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        presentation);
    require(pump.run(transport.ticket) == PumpResult::Disconnected);
    require(persist_calls == 2 && shortcut_deliveries == 2);
    require(transport.writes.size() == 5);
    require(transport.writes[0].first ==
                FanyImePipeRole::ToTsfWorkerThread &&
            transport.writes[1].first ==
                FanyImePipeRole::ToTsfWorkerThread &&
            transport.writes[2].first ==
                FanyImePipeRole::ToTsfWorkerThread &&
            transport.writes[3].first ==
                FanyImePipeRole::ToTsfWorkerThread &&
            transport.writes[4].first == FanyImePipeRole::ToTsf);
    queue.stop();
  }
  for (bool enabled : {false, true}) {
    // Ctrl+Shift+J 由生产路由交给 InputState：开关关着时不调用 persist，开着时一次按键调用一次；两种情况都回一个不带帧的 NoReply，TIP 不读它。
    auto configured = nlohmann::json::parse(options);
    configured["preferences"]["keybindings"]["toggle_japanese_ctrl_shift_j"] =
        enabled;
    FocusGate gate;
    InputQueue queue(gate, 2, 8, configured.dump());
    FixtureTransport transport;
    transport.packets.resize(4);
    auto &toggle = transport.packets[1];
    toggle.keycode = 'J';
    toggle.wch = 'j';
    toggle.modifiers_down = 0b11;
    toggle.request_id = 2;
    transport.packets[2] = toggle;
    transport.packets[2].request_id = 3;
    auto &punctuation = transport.packets.back();
    punctuation.keycode = 0xBC;
    punctuation.wch = ',';
    punctuation.modifiers_down = 0;
    punctuation.request_id = 4;
    size_t persist_calls = 0, toggle_deliveries = 0, character_sets = 0;
    const auto production = production_key_handler(
        [&](bool) {
          ++character_sets;
          return true;
        },
        [&] {
          ++persist_calls;
          return true;
        });
    SessionPump::Presentation presentation;
    presentation.delivered = [&](const FocusLease &, const PendingReply &reply,
                                 const FanyImeNamedpipeData &packet) {
      if (!FanyImeProtocol::IsJapaneseToggleShortcut(packet.keycode,
                                                     packet.modifiers_down))
        return;
      ++toggle_deliveries;
      require(!reply.encoded);
    };
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &lease,
            const FanyImeNamedpipeData &packet) {
          if (FanyImeProtocol::IsJapaneseToggleShortcut(packet.keycode,
                                                        packet.modifiers_down))
            return production(state, lease, packet);
          auto reply = state.configured_key(
              lease, packet, TsfPreeditStyle::Local, {});
          require(reply && reply->encoded && static_cast<bool>(*reply->encoded));
          require(reply->source.transition.at("commit") == "，");
          return reply;
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        presentation);
    require(pump.run(transport.ticket) == PumpResult::Disconnected);
    require(toggle_deliveries == 2 && character_sets == 0);
    require(persist_calls == (enabled ? 2u : 0u));
    queue.stop();
  }
  {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    size_t keys = 0, notifications = 0, closed = 0;
    SessionPump::Presentation presentation;
    presentation.delivered = [&](const FocusLease &, const PendingReply &,
                                 const FanyImeNamedpipeData &) {
      ++notifications;
      throw std::runtime_error("Synthetic presentation failure");
    };
    presentation.disconnected = [&](const PipeTicket &) { ++closed; };
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &lease,
            const FanyImeNamedpipeData &packet) {
          ++keys;
          return state.configured_key(lease, packet, TsfPreeditStyle::Pinyin,
                                      {});
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; },
        presentation);
    require(pump.run(transport.ticket) == PumpResult::QueueUnavailable);
    // A throwing input task stops the queue; cleanup cannot publish afterward.
    require(keys == 1 && notifications == 1 && closed == 0);
    require(!queue.stats().accepting);
    require(!transport.open && transport.writes.size() == 3);
    queue.stop();
  }
  for (bool microsoft : {false, true}) {
    for (bool local : {false, true}) {
      for (bool uiless : {false, true}) {
        auto configured = nlohmann::json::parse(options);
        configured["preferences"]["scheme"] = "shuangpin";
        configured["preferences"]["shuangpin_profile"] =
            microsoft ? "microsoft" : "xiaohe";
        FocusGate gate;
        InputQueue queue(gate, 2, 8, configured.dump());
        FixtureTransport transport;
        transport.packets.resize(3);
        transport.packets[1].keycode = 'B';
        transport.packets[1].wch = 'b';
        transport.packets[1].modifiers_down =
            uiless ? FanyImePipeFlags::UiLess : 0;
        transport.packets[2].keycode = 0xBA;
        transport.packets[2].wch = ';';
        transport.packets[2].modifiers_down =
            uiless ? FanyImePipeFlags::UiLess : 0;
        size_t keys = 0;
        std::exception_ptr failure;
        SessionPump pump(
            transport, queue, gate,
            [&](InputState &state, const FocusLease &lease,
                const FanyImeNamedpipeData &packet) {
              try {
                auto result = state.configured_key(
                    lease, packet,
                    local ? TsfPreeditStyle::Local : TsfPreeditStyle::Pinyin,
                    {});
                require(result.has_value());
                if (++keys == 2) {
                  const auto &transition = result->source.transition;
                  require(transition.at("view").at("microsoft_shuangpin") ==
                          microsoft);
                  if (microsoft) {
                    require(transition.at("commit").is_null() &&
                            transition.at("view").at("editing_text") == "b;");
                    require(result->encoded.has_value() == (!local || uiless));
                    if (result->encoded)
                      require(result->encoded->packet.msg_type ==
                              (uiless ? FanyImeReplyType::UiLessComposition
                                      : FanyImeReplyType::Preedit));
                  } else {
                    require(transition.at("view").at("editing_text") == "");
                    require(result->encoded &&
                            result->encoded->packet.msg_type ==
                                FanyImeReplyType::CommitExactText);
                  }
                }
                return result;
              } catch (...) {
                failure = std::current_exception();
                throw;
              }
            },
            [](const FocusRoute &, const FanyImeNamedpipeData &) {
              return true;
            });
        const auto completed = pump.run(transport.ticket);
        if (failure)
          std::rethrow_exception(failure);
        require(completed == PumpResult::Disconnected && keys == 2);
        queue.stop();
      }
    }
  }
  for (bool minus : {false, true}) {
    for (bool last : {false, true}) {
      for (bool han : {false, true}) {
        for (bool uiless : {false, true}) {
          for (bool shared : {false, true}) {
            FocusGate gate;
            InputQueue queue(gate, 2, 8, options);
            FixtureTransport transport;
            if (!han) {
              // U3041 is non-Han and avoids an embedded NUL in intermediate
              // pages.
              const std::string code = "3041";
              for (size_t i = 0; i < code.size(); ++i) {
                transport.packets[i + 2].keycode = code[i];
                transport.packets[i + 2].wch = code[i];
              }
            }
            auto &tail = transport.packets.back();
            tail.keycode = minus ? (last ? 0xBB : 0xBD) : (last ? 0xDD : 0xDB);
            tail.wch = minus ? (last ? '=' : '-') : (last ? ']' : '[');
            if (uiless)
              for (size_t i = 1; i < transport.packets.size(); ++i)
                transport.packets[i].modifiers_down |= FanyImePipeFlags::UiLess;
            const auto host = nlohmann::json::parse(options);
            nlohmann::json launch{
                {"format_version", 1},
                {"resources", host.at("resources")},
                {"state_root", host.at("user_data")},
                {"pipe_namespace", "binding-fixture"},
                {"preedit_style", "pinyin"},
                {"key_bindings",
                 {{"minus_equal", true},
                  {"brackets", true},
                  {"comma_period", false},
                  {"tab", false},
                  {"page_up_down", false},
                  {"arrows", false},
                  {"word_character", minus ? "minus_equal" : "brackets"}}}};
            std::optional<PreferenceSnapshot> publication;
            if (shared) {
              launch.erase("key_bindings");
              auto preferences = host.at("preferences");
              preferences["word_character"] = {
                  {"enabled", true},
                  {"keys", minus ? "minus_equal" : "brackets"}};
              preferences["navigation"] = {
                  {"minus_equal", false}, {"comma_period", true},
                  {"brackets", false},    {"tab", true},
                  {"page_up_down", true}, {"arrows", true}};
              const auto directory =
                  std::filesystem::u8path(
                      host.at("user_data").get<std::string>()) /
                  "word-publication";
              std::filesystem::create_directories(directory);
              std::ofstream file(directory / "preferences.json");
              file << nlohmann::json{{"format_version", 1},
                                     {"revision", 1},
                                     {"preferences", preferences}}
                          .dump();
              file.close();
              require(static_cast<bool>(file));
              publication = PreferenceSnapshot::load(directory.u8string());
            }
            auto config = PreviewConfig::parse(launch.dump());
            const auto handler = preview_key_handler(config);
            config.word_character = WordCharacterBinding::Disabled;
            config
                .navigation = {}; // Already-created handler owns its snapshot.
            size_t keys = 0;
            std::exception_ptr failure;
            SessionPump pump(
                transport, queue, gate,
                [&](InputState &state, const FocusLease &lease,
                    const FanyImeNamedpipeData &packet) {
                  try {
                    if (keys == 5 && publication)
                      state.publish_preferences(*publication);
                    auto result = handler(state, lease, packet);
                    if (!result || !result->encoded ||
                        !static_cast<bool>(*result->encoded))
                      throw std::runtime_error(
                          "Word fixture failed: key=" + std::to_string(keys) +
                          " han=" + std::to_string(han) +
                          " uiless=" + std::to_string(uiless));
                    if (++keys == 6) {
                      require(result->source.transition.at("commit") ==
                              (han ? "中" : "ぁ"));
                      require(result->source.transition.at("view").at(
                                  "editing_text") == "");
                      require(result->encoded->packet.msg_type ==
                              (han ? FanyImeReplyType::CommitExactText
                                   : FanyImeReplyType::Normal));
                    }
                    return result;
                  } catch (...) {
                    failure = std::current_exception();
                    throw;
                  }
                },
                [](const FocusRoute &, const FanyImeNamedpipeData &) {
                  return true;
                });
            const auto completed = pump.run(transport.ticket);
            if (failure)
              std::rethrow_exception(failure);
            require(completed == PumpResult::Disconnected && keys == 6 &&
                    transport.writes.size() == 13);
            queue.stop();
          }
        }
      }
    }
  }
  {
    // The launch preedit style decides which composition frames reach the
    // preview host, even when the shared preferences ask for another one: the
    // host renders composition the way it was started, and a later publication
    // must not start sending it frames it does not consume.
    auto host = nlohmann::json::parse(options);
    host["preferences"]["tsf_preedit_style"] = "pinyin";
    FocusGate gate;
    InputQueue queue(gate, 2, 8, host.dump());
    FixtureTransport transport;
    transport.packets.resize(2); // Activation and one composing character.
    const nlohmann::json launch{{"format_version", 1},
                                {"resources", host.at("resources")},
                                {"state_root", host.at("user_data")},
                                {"pipe_namespace", "style-fixture"},
                                {"preedit_style", "local"}};
    const auto handler =
        preview_key_handler(PreviewConfig::parse(launch.dump()));
    size_t keys = 0;
    std::exception_ptr failure;
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &lease,
            const FanyImeNamedpipeData &packet) {
          try {
            require(state.tsf_preedit_style() == TsfPreeditStyle::Pinyin);
            auto result = handler(state, lease, packet);
            ++keys;
            // Local composition belongs to the host; nothing is encoded.
            require(result && !result->encoded &&
                    result->source.transition.at("view").at("local_mode") ==
                        "unicode");
            return result;
          } catch (...) {
            failure = std::current_exception();
            throw;
          }
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    const auto completed = pump.run(transport.ticket);
    if (failure)
      std::rethrow_exception(failure);
    require(completed == PumpResult::Disconnected);
    require(keys == 1);
    // Both packets acknowledge focus; neither carries a composition frame.
    require(transport.writes.size() == 2);
    queue.stop();
  }
  for (bool brackets : {false, true}) {
    for (bool paging : {false, true}) {
      for (bool uiless : {false, true}) {
        for (bool shared : {false, true}) {
          FocusGate gate;
          auto host = nlohmann::json::parse(options);
          host["preferences"]["navigation"] = {
              {"minus_equal", false},
              {"brackets", brackets && !paging},
              {"comma_period", !brackets && !paging},
              {"tab", false},
              {"page_up_down", false},
              {"arrows", false}};
          // This fixture owns brackets/comma as navigation or punctuation;
          // keep the shared word-character binding out of the same key space.
          host["preferences"]["word_character"] = {
              {"enabled", false}, {"keys", "brackets"}};
          InputQueue queue(gate, 2, 8, host.dump());
          FixtureTransport transport;
          auto &last = transport.packets.back();
          last.keycode = brackets ? 0xDD : 0xBC;
          last.wch = brackets ? ']' : ',';
          if (uiless)
            for (size_t i = 1; i < transport.packets.size(); ++i)
              transport.packets[i].modifiers_down |= FanyImePipeFlags::UiLess;
          nlohmann::json launch{{"format_version", 1},
                                {"resources", host.at("resources")},
                                {"state_root", host.at("user_data")},
                                {"pipe_namespace", "paging-fixture"},
                                {"preedit_style", "pinyin"},
                                {"key_bindings",
                                 {{"minus_equal", false},
                                  {"brackets", brackets && paging},
                                  {"comma_period", !brackets && paging},
                                  {"tab", false},
                                  {"page_up_down", false},
                                  {"arrows", false},
                                  {"word_character", "disabled"}}}};
          if (shared)
            launch.erase("key_bindings");
          std::optional<PreferenceSnapshot> publication;
          if (shared) {
            auto preferences = host.at("preferences");
            preferences["navigation"]["brackets"] = brackets && paging;
            preferences["navigation"]["comma_period"] = !brackets && paging;
            const auto directory = std::filesystem::u8path(host.at("user_data").get<std::string>()) / "paging-publication";
            std::filesystem::create_directories(directory);
            std::ofstream file(directory / "preferences.json");
            file << nlohmann::json{{"format_version", 1}, {"revision", 1}, {"preferences", preferences}}.dump();
            file.close();
            require(static_cast<bool>(file));
            publication = PreferenceSnapshot::load(directory.u8string());
          }
          const auto handler =
              preview_key_handler(PreviewConfig::parse(launch.dump()));
          size_t keys = 0;
          std::exception_ptr failure;
          SessionPump pump(
              transport, queue, gate,
              [&](InputState &state, const FocusLease &lease,
                  const FanyImeNamedpipeData &packet) {
                try {
                  if (keys == 5 && publication)
                    state.publish_preferences(*publication);
                  auto result = handler(state, lease, packet);
                  require(result && result->encoded &&
                          static_cast<bool>(*result->encoded));
                  if (++keys == 6) {
                    const auto &transition = result->source.transition;
                    if (paging) {
                      require(transition.at("commit").is_null() &&
                              transition.at("view").at("editing_text") ==
                                  "U4e2d");
                      require(result->encoded->packet.msg_type ==
                              (uiless ? FanyImeReplyType::UiLessComposition
                               : brackets
                                   ? FanyImeReplyType::MovePageNext
                                   : FanyImeReplyType::MovePagePrevious));
                    } else {
                      require(transition.at("commit") ==
                                  (brackets ? "中】" : "中，") &&
                              transition.at("view").at("editing_text") == "");
                      require(result->encoded->packet.msg_type ==
                              FanyImeReplyType::CommitExactText);
                    }
                  }
                  return result;
                } catch (...) {
                  failure = std::current_exception();
                  throw;
                }
              },
              [](const FocusRoute &, const FanyImeNamedpipeData &) {
                return true;
              });
          const auto completed = pump.run(transport.ticket);
          if (failure)
            std::rethrow_exception(failure);
          require(completed == PumpResult::Disconnected && keys == 6);
          require(transport.writes.size() == 13);
          queue.stop();
        }
      }
    }
  }
  {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    const auto original = transport.packets;
    transport.packets.pop_back();
    auto packet = original.back();
    packet.keycode = 0x1B;
    packet.request_id = 88; // Main packets always carry a correlation ID.
    transport.packets.push_back(packet);
    transport.packets.insert(transport.packets.end(), original.begin() + 1, original.end() - 1);
    packet.keycode = 0x0D;
    packet.request_id = 90;
    transport.packets.push_back(packet);
    size_t keys = 0;
    SessionPump pump(transport, queue, gate,
        [&](InputState &state, const FocusLease &lease, const FanyImeNamedpipeData &key) {
          auto result = state.basic_key(lease, key, TsfPreeditStyle::Pinyin,
              key.keycode == 0x0D ? std::optional<std::string>("U4e2d") : std::nullopt);
          require(result.has_value());
          ++keys;
          if (key.keycode == 0x1B || key.keycode == 0x0D)
            require(!result->encoded && result->source.transition.at("view").at("editing_text") == "");
          if (key.keycode == 0x0D)
            require(result->source.transition.at("commit") == "U4e2d");
          return result;
        }, [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    require(pump.run(transport.ticket) == PumpResult::Disconnected && keys == 12);
    require(transport.writes.size() == 23);
    queue.stop();
  }
  {
    FanyImeNamedpipeData packet{};
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.keycode = '1';
    packet.wch = '1';
    require(edit_kind(packet, "none", true) == EditKind::None);
    require(edit_kind(packet, "unicode", true) == EditKind::Character);
    packet.modifiers_down = 1;
    packet.wch = '+';
    require(edit_kind(packet, "unicode", true) == EditKind::None);
    packet.keycode = 0xBB;
    require(edit_kind(packet, "unicode", true) == EditKind::Character);
    require(edit_kind(packet, "none", true) == EditKind::None);
    packet.keycode = 0x20;
    require(edit_kind(packet, "unicode", true) == EditKind::None);
    packet.keycode = 'A';
    packet.wch = 'A';
    packet.modifiers_down = 2;
    require(edit_kind(packet, "none", true) == EditKind::None);
    packet.modifiers_down = 0;
    require(edit_kind(packet, "unknown", true) == EditKind::None);
    packet.keycode = 0xDE;
    packet.wch = '\'';
    require(edit_kind(packet, "none", false) == EditKind::None);
    require(edit_kind(packet, "none", true) == EditKind::Character);
  }
  for (int mode = 0; mode < 4; ++mode) {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    transport.packets.pop_back(); // Editing only; no candidate commit.
    auto packet = transport.packets.back();
    packet.wch = 0;
    packet.modifiers_down = 0;
    for (uint32_t key : {0x25, 0x27, 0x08, 0x08, 0x08, 0x08, 0x08}) {
      packet.keycode = key;
      ++packet.request_id;
      transport.packets.push_back(packet);
    }
    const bool uiless = mode >= 2;
    const auto style = mode % 2 ? TsfPreeditStyle::Pinyin : TsfPreeditStyle::Local;
    if (uiless)
      for (size_t i = 1; i < transport.packets.size(); ++i)
        transport.packets[i].modifiers_down |= FanyImePipeFlags::UiLess;
    size_t keys = 0;
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &focus, const FanyImeNamedpipeData &key) {
          auto result = state.edit(focus, key, style);
          require(result.has_value());
          ++keys;
          const bool reply = uiless || (style == TsfPreeditStyle::Pinyin &&
                                       keys != 6 && keys != 7 && keys != 12);
          require(result->encoded.has_value() == reply);
          require(result->source.reply_expected == reply);
          if (result->encoded)
            require(result->encoded->packet.msg_type ==
                    (uiless ? FanyImeReplyType::UiLessComposition : FanyImeReplyType::Preedit));
          if (keys == 12)
            require(result->source.transition.at("view").at("editing_text") == "");
          return result;
        }, [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    require(pump.run(transport.ticket) == PumpResult::Disconnected && keys == 12);
    require(transport.writes.size() == 13u + (uiless ? 12u : mode == 1 ? 9u : 0u));
    queue.stop();
  }
  for (auto event : {FanyImePipeEventType::PuncSwitch,
                    FanyImePipeEventType::StatusSnapshot,
                    FanyImePipeEventType::FocusRestored}) {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    transport.packets.resize(1);
    auto status = transport.packets.front();
    status.event_type = event;
    status.keycode = event == FanyImePipeEventType::PuncSwitch ? 0 : 1;
    transport.packets.push_back(status);
    auto punctuation = transport.packets.front();
    punctuation.event_type = FanyImePipeEventType::KeyEvent;
    punctuation.request_id = 2;
    punctuation.keycode = 0xBC;
    punctuation.wch = ',';
    transport.packets.push_back(punctuation);
    status.keycode = 1;
    status.pinyin_length = event == FanyImePipeEventType::PuncSwitch ? 0 : 1;
    transport.packets.push_back(status);
    ++punctuation.request_id;
    transport.packets.push_back(punctuation);
    size_t keys = 0;
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &focus,
            const FanyImeNamedpipeData &packet) {
          auto reply = state.key(focus, packet, ReplyPath::Punctuation);
          require(reply.has_value());
          if (++keys == 1)
            require(reply->source.transition.at("handled") == false &&
                    reply->source.transition.at("commit").is_null());
          else
            require(reply->source.transition.at("commit") == "，");
          return reply;
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    require(pump.run(transport.ticket) == PumpResult::Disconnected && keys == 2);
    queue.stop();
  }
  {
    // The TSF auto-closed the 《 the Server's Engine resolved, so the notification pays the nesting back before the next '<' reaches the Engine; without it the second opening would be 〈.
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    transport.packets.resize(1);
    auto notice = transport.packets.front();
    notice.event_type = FanyImePipeEventType::PuncSwitch;
    notice.keycode = 1;
    transport.packets.push_back(notice);
    auto opening = transport.packets.front();
    opening.event_type = FanyImePipeEventType::KeyEvent;
    opening.request_id = 2;
    opening.keycode = 0xBC;
    opening.wch = '<';
    opening.modifiers_down = 1;
    transport.packets.push_back(opening);
    notice.event_type = FanyImePipeEventType::PairedPunctuationAutoClosed;
    notice.keycode = '<';
    transport.packets.push_back(notice);
    ++opening.request_id;
    transport.packets.push_back(opening);
    size_t keys = 0;
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &focus,
            const FanyImeNamedpipeData &packet) {
          auto reply = state.key(focus, packet, ReplyPath::Punctuation);
          require(reply.has_value());
          ++keys;
          require(reply->source.transition.at("commit") == "《");
          return reply;
        },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    require(pump.run(transport.ticket) == PumpResult::Disconnected && keys == 2);
    queue.stop();
  }
  {
    // 字段属性（FieldTraits）在激活之前和按键之间都收下：不断开连接、不交给事件处理器，后面的按键照常处理。Server 的会话选择加入了生命周期核心，上屏的回复里多一个 feedback 键，回复合成器照样读它。
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    auto traits = transport.packets.front();
    traits.event_type = FanyImePipeEventType::FieldTraits;
    traits.request_id = 0;
    traits.keycode = FanyImeFieldTraits::Elevated;
    transport.packets.insert(transport.packets.begin(), traits);
    traits.keycode = 0;
    transport.packets.insert(transport.packets.begin() + 3, traits);
    size_t keys = 0;
    bool committed = false;
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &lease,
            const FanyImeNamedpipeData &packet) {
          ++keys;
          auto reply = state.configured_key(lease, packet, TsfPreeditStyle::Pinyin, {});
          if (reply && !reply->source.transition.at("commit").is_null()) {
            committed = true;
            require(reply->source.transition.at("feedback").is_object());
          }
          return reply;
        },
        [&](const FocusRoute &, const FanyImeNamedpipeData &packet) {
          require(packet.event_type != FanyImePipeEventType::FieldTraits);
          return true;
        });
    require(pump.run(transport.ticket) == PumpResult::Disconnected && keys == 6 && committed);
    queue.stop();
  }
  for (auto event : {FanyImePipeEventType::StatusSnapshot,
                     FanyImePipeEventType::IMESwitch,
                     FanyImePipeEventType::FocusRestored}) {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    const auto original = transport.packets;
    auto status = original.front();
    status.event_type = event;
    status.keycode = 0;
    transport.packets.insert(transport.packets.end() - 1, status);
    transport.packets.insert(transport.packets.end() - 1, status);
    // Closed mode must also suppress a fresh Unicode-mode trigger.
    transport.packets.push_back(original[1]);
    status.keycode = event == FanyImePipeEventType::IMESwitch ? 9 : 1;
    transport.packets.push_back(status);
    transport.packets.insert(transport.packets.end(), original.begin() + 1,
                             original.end());
    size_t keys = 0;
    size_t notifications = 0;
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &focus,
            const FanyImeNamedpipeData &packet) {
          ++keys;
          auto reply =
              state.key(focus, packet,
                        packet.keycode == 0x20 ? ReplyPath::Selection
                                               : ReplyPath::Composition);
          require(reply.has_value());
          const auto &transition = reply->source.transition;
          if (keys == 5)
            require(transition.at("view").at("editing_text") == "U4e2d");
          if (keys == 6 || keys == 7)
            require(transition.at("commit").is_null() &&
                    transition.at("handled") == false &&
                    transition.at("view").at("editing_text") == "");
          if (keys == 13)
            require(transition.at("commit") == "中");
          return reply;
        },
        [&](const FocusRoute &, const FanyImeNamedpipeData &packet) {
          if (packet.event_type == event)
            ++notifications;
          return true;
        });
    require(pump.run(transport.ticket) == PumpResult::Disconnected);
    require(keys == 13 && notifications == 3);
    queue.stop();
  }
  for (int mode = 0; mode < 6; ++mode) {
    FocusGate gate;
    InputQueue queue(gate, 2, 8, options);
    FixtureTransport transport;
    if (mode == 1)
      transport.fail_write = 1; // Initial focus fence.
    if (mode == 2)
      transport.fail_write = 3; // First reply may have been delivered.
    if (mode == 4)
      transport.packets[1].client_id = 43; // Reject even a faulty transport.
    if (mode == 5)
      transport.packets.resize(2);
    if (mode == 0) {
      auto repeated = transport.packets.front();
      repeated.event_type = FanyImePipeEventType::ClientHello;
      transport.packets.push_back(repeated);
    }
    size_t keys = 0;
    std::optional<FocusLease> takeover;
    std::optional<FocusLease> lease;
    SessionPump pump(
        transport, queue, gate,
        [&](InputState &state, const FocusLease &focus,
            const FanyImeNamedpipeData &packet) {
          require(std::this_thread::get_id() != transport.io_thread);
          ++keys;
          // The shared basic dispatcher owns editing versus selection here.
          // Native configuration-priority routes are outside this fixture.
          auto result = state.basic_key(focus, packet, TsfPreeditStyle::Pinyin);
          if (mode == 3 && result)
            ++result->source.request_id;
          if (mode == 5 && result) {
            PipeTicket other{43, {4, 5, 6}};
            require(state.connected(other).accepted);
            auto activated = transport.packets.front();
            activated.client_id = 43;
            activated.request_id = 88;
            takeover = state.dispatch(other, activated).route;
          }
          return result;
        },
        [&](const FocusRoute &route, const FanyImeNamedpipeData &) {
          require(std::this_thread::get_id() != transport.io_thread);
          lease = route.route;
          return true;
        });
    const auto result = pump.run(transport.ticket);
    require(!transport.open);
    if (lease)
      require(!gate.with_active(*lease, [] {}));
    if (mode == 0) {
      require(result == PumpResult::Disconnected && keys == 6 &&
              transport.next == 8 && transport.writes.size() == 13);
      const auto fence = *focus_ready_bytes(7, 8, 77);
      require(transport.writes[0].first == FanyImePipeRole::ToTsfWorkerThread &&
              transport.writes[0].second == fence);
      for (size_t i = 0; i < 6; ++i) {
        require(transport.writes[1 + 2 * i].second == fence &&
                transport.writes[1 + 2 * i].first ==
                    FanyImePipeRole::ToTsfWorkerThread);
        require(transport.writes[2 + 2 * i].first == FanyImePipeRole::ToTsf &&
                transport.writes[2 + 2 * i].second.size() ==
                    sizeof(FanyImeNamedpipeDataToTsf));
      }
      const auto expected = *wire_bytes(candidate_commit(7, "中"));
      require(transport.writes.back().second ==
              std::vector<uint8_t>(expected.begin(), expected.end()));
    } else if (mode == 1) {
      require(result == PumpResult::WriteFailed && keys == 0 &&
              transport.next == 1);
    } else if (mode == 2) {
      require(result == PumpResult::WriteFailed && keys == 1 &&
              transport.next == 2 && transport.writes.size() == 3);
    } else if (mode == 3) {
      require(result == PumpResult::DispatchFailed && keys == 1 &&
              transport.writes.size() == 2);
    } else if (mode == 4) {
      require(result == PumpResult::DispatchFailed && keys == 0 &&
              transport.writes.size() == 1);
    } else {
      require(result == PumpResult::Disconnected && keys == 1 &&
              transport.writes.size() == 2 && takeover &&
              gate.with_pending(*takeover, [] {}));
    }
    queue.stop();
  }
  {
    FocusGate gate;
    InputQueue queue(gate, 1, 1, options);
    FixtureTransport transport;
    SessionPump pump(
        transport, queue, gate,
        [](InputState &, const FocusLease &, const FanyImeNamedpipeData &)
            -> std::optional<PendingReply> { return std::nullopt; },
        [](const FocusRoute &, const FanyImeNamedpipeData &) { return true; });
    auto rejected = queue.submit([&](InputState &) {
      require(pump.run(transport.ticket) == PumpResult::QueueUnavailable);
    });
    require(rejected && rejected->get() == InputTaskStatus::Completed);
    require(transport.open); // Rejected self-wait did not touch transport.
    queue.stop();
    require(pump.run(transport.ticket) == PumpResult::QueueUnavailable &&
            !transport.open && transport.writes.empty());
  }
}
