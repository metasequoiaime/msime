#include "FocusRouter.h"
#include "../../../common/HostApiString.h"
#include "FocusedSession.h"
#include "InputQueue.h"
#include "KeyEvent.h"
#include "ReplyCodec.h"
#include "ReplyComposer.h"
#include "PipeMetadata.h"
#include "ServerSession.h"
#include "../core/TestHostOptions.h"
#include "ipc_negotiation.h"
#include <chrono>
#include <filesystem>
#include <fstream>
#include <future>
#include <iostream>
#include <memory>
#include <cstdio>
#include <stdexcept>
#ifdef _WIN32
#include <windows.h>
static_assert(VK_BACK == 0x08 && VK_RETURN == 0x0D && VK_SPACE == 0x20);
static_assert(VK_PRIOR == 0x21 && VK_NEXT == 0x22 && VK_DELETE == 0x2E);
static_assert(VK_NUMPAD0 == 0x60 && VK_NUMPAD9 == 0x69 && VK_LSHIFT == 0xA0);
#endif

using Json = nlohmann::json;
using msime::windows::ServerSession;
void session_pump_tests(const std::string &options);
void candidate_mailbox_tests();
void ui_selection_tests(const std::string &options, bool dictionary);
void session_worker_tests(const std::string &options);
void preference_monitor_tests(const std::string &options,
                              const std::string &directory);
namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}
template <class F> void rejected(F action) {
  bool failed = false;
  try {
    action();
  } catch (const std::exception &) {
    failed = true;
  }
  require(failed, "Invalid request was accepted");
}
} // namespace

void local_mode_tests(const std::string &options) {
  using msime::windows::ServerSession;
  struct ModeCase {
    char shortcut;
    const char *name;
    const char *input;
  };
  // These are the documented Windows temporary-mode shortcuts. The Engine
  // owns their semantics; this test verifies that the Server boundary keeps
  // the Shift entry gate, candidate view, and commit path intact.
  constexpr ModeCase cases[] = {
      {'U', "unicode", "4e2d"},
      {'T', "date_time", "rq"},
      {'K', "quick_phrase", "a"},
      {'E', "emoji", "XIAOLIAN"},
      {'M', "kaomoji", "hx"},
      {'J', "super_jianpin", "nh"},
      {'Y', "temporary_english", "he"},
      {'R', "temporary_japanese", "ka"},
  };
  uint64_t request = 1;
  for (const auto &mode : cases) {
    ServerSession session(77, options);
    uint64_t epoch = 1;
    session.activate(epoch);
    auto key = [&](uint32_t keycode, uint32_t text, uint32_t modifiers) {
      FanyImeNamedpipeData packet{};
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.client_id = 77;
      packet.request_id = request++;
      packet.keycode = keycode;
      packet.wch = static_cast<FanyImeWireChar>(text);
      packet.modifiers_down = modifiers;
      return session.key(packet, epoch);
    };

    key(static_cast<uint32_t>(mode.shortcut),
        static_cast<uint32_t>(mode.shortcut), 1);
    auto entered = session.view();
    require(entered.at("local_mode") == mode.name,
            "Windows shortcut did not enter the expected local mode");
    for (const unsigned char character : std::string(mode.input)) {
      const auto upper = character >= 'a' && character <= 'z'
                             ? static_cast<uint32_t>(character - 'a' + 'A')
                             : static_cast<uint32_t>(character);
      key(upper, character, 0);
    }
    auto candidates = session.view();
    require(!candidates.at("candidates").empty(),
            "Local mode produced no candidates from the locked dictionary");
    const auto generation = candidates.at("generation").get<uint64_t>();
    const auto selected = session.select(epoch, generation, 0);
    require(selected.at("commit").is_string() &&
                !selected.at("commit").get<std::string>().empty(),
            "Local mode candidate selection did not commit text");
    require(selected.at("view").at("local_mode") == "none",
            "Local mode remained active after candidate commit");
  }
}

// V, "/" and "@" through the production key route (ReplyComposer::configured_key), which is where the TIP's keys arrive. The Engine lists the keys it spells in View.spelling_symbols; the Server must hand those to it as input instead of reading a digit as a selection, an operator as paging or a "/" as punctuation. No dictionary is needed: the modes generate their rows.
void local_mode_trigger_tests(const std::string &options) {
  using namespace msime::windows;
  ServerSession session(43, options);
  uint64_t epoch = 1;
  session.activate(epoch);
  require(session.view().at("spelling_symbols") == "",
          "Switched-off modes listed trigger symbols");
  auto preferences = Json::parse(options).at("preferences");
  preferences["local_modes"] = {
      {"unicode", true},       {"date_time", true},
      {"quick_phrase", true},  {"emoji", true},
      {"kaomoji", true},       {"super_jianpin", true},
      {"temporary_english", true}, {"temporary_japanese", true},
      {"expression", true},    {"command", true},
      {"mention", true}};
  preferences["chinese_punctuation"] = true;
  session.update_preferences(
      epoch, Json{{"format_version", 1}, {"revision", 1}, {"preferences", preferences}}
                 .dump());
  ReplyComposer composer(43, epoch);
  const auto bindings = preference_navigation(Json::object());
  uint64_t request = 1;
  auto press = [&](uint32_t vk, uint32_t text, uint32_t modifiers = 0) {
    FanyImeNamedpipeData packet{};
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.client_id = 43;
    packet.request_id = request++;
    packet.keycode = vk;
    packet.wch = static_cast<FanyImeWireChar>(text);
    packet.modifiers_down = modifiers;
    auto reply = composer.configured_key(session, packet, epoch,
                                         TsfPreeditStyle::Local, bindings);
    require(reply.has_value(), "A local-mode key was not taken");
    composer.confirm_delivery(43, epoch, packet.request_id);
    return *reply;
  };
  require(session.view().at("spelling_symbols") == "/@",
          "An idle pinyin session did not list the trigger symbols");

  // V: digits and operators compose, including '-' while minus/equal paging is on and Shift+8's '*'.
  press('V', 'V', 1);
  require(session.view().at("local_mode") == "expression" &&
              session.view().at("spelling_symbols") == "0123456789+-*/.()%^",
          "Shift+V did not open the expression mode");
  press('1', '1');
  press(0xBB, '+', 1);
  press('2', '2');
  press('8', '*', 1);
  press('3', '3');
  press(0xBD, '-');
  press('1', '1');
  const auto expression = session.view();
  require(expression.at("editing_text") == "V1+2*3-1" &&
              !expression.at("candidates").empty() &&
              expression.at("candidates").at(0).at("text") == "6",
          "Digits or operators were not composed in the expression mode");
  // Shift+1 prints '!', which the mode does not spell, so it picks the first row; the commit is generated text and not typing.
  const auto chosen = press('1', '!', 1);
  require(chosen.source.transition.at("commit") == "6" &&
              chosen.committed_text == std::optional<std::string>("6") &&
              !transition_counts_as_typing(chosen.source.transition) &&
              session.view().at("local_mode") == "none",
          "Shift+1 did not commit the expression result");

  // "/" on an empty composition opens the command mode instead of typing a mark, and letters narrow it.
  press(0xBF, '/');
  require(session.view().at("local_mode") == "command" &&
              session.view().at("editing_text") == "/",
          "'/' did not open the command mode");
  press('R', 'r');
  press('Q', 'q');
  const auto command = press(0x20, ' ');
  require(command.committed_text && !command.committed_text->empty() &&
              !transition_counts_as_typing(command.source.transition),
          "Space did not commit the command row");

  // "@" is Shift+2: the mode's key, not the second candidate. Backspace leaves it again.
  press('2', '@', 1);
  require(session.view().at("local_mode") == "mention" &&
              session.view().at("editing_text") == "@",
          "'@' did not open the mention mode");
  press(0x08, '\b');
  require(session.view().at("editing_text") == "" &&
              session.view().at("local_mode") == "none",
          "Backspace did not leave the mention mode");

  // With ASCII punctuation "/" is the literal mark the user chose, and the Engine lists no trigger.
  preferences["chinese_punctuation"] = false;
  session.update_preferences(
      epoch, Json{{"format_version", 1}, {"revision", 2}, {"preferences", preferences}}
                 .dump());
  session.set_chinese_punctuation(epoch, false);
  require(session.view().at("spelling_symbols") == "",
          "ASCII punctuation still listed the trigger symbols");
  session.deactivate(epoch);
}

#ifdef _WIN32
int wmain(int argc, wchar_t **argv) {
#else
int main(int argc, char **argv) {
#endif
  try {
    require(argc == 1 || argc == 2, "Expected at most one resource directory");
    auto root =
        std::filesystem::temp_directory_path() /
        ("msime-windows-session-" +
         std::to_string(
             std::chrono::steady_clock::now().time_since_epoch().count()));
    require(std::filesystem::create_directory(root),
            "Cannot create isolated test root");
    struct Cleanup {
      std::filesystem::path path;
      ~Cleanup() {
        std::error_code error;
        std::filesystem::remove_all(path, error);
      }
    } cleanup{root};
    Json preferences = {{"scheme", "quanpin"},
                        {"learning", false},
                        {"chinese_punctuation", true},
                        {"candidate_page_size", 2}};
    auto directory = [&](const char *name) {
      auto path = root / name;
      std::filesystem::create_directory(path);
      return path.u8string();
    };
    Json options = {{"api_version", 1},
                    {"resources", directory("resources")},
                    {"user_data", directory("user")},
                    {"cache", directory("cache")},
                    {"dictionaries", directory("dictionaries")},
                    {"preferences", preferences}};
    if (argc == 2) {
      const auto resources = std::filesystem::path(argv[1]);
      require(resources.is_absolute(), "Expected absolute resource directory");
      auto input = Json{{"resources", resources.u8string()},
                        {"state_root", (root / "prepared").u8string()}}
                       .dump();
      auto prepared = msime::host_api::own_string(msime_client_prepare_host(
          reinterpret_cast<const uint8_t *>(input.data()), input.size()));
      auto document = Json::parse(prepared.get());
      if (!document.at("ok").get<bool>()) {
        // The reply carries why. Asserting only on `ok` throws that away and
        // leaves one message standing for every way preparation can fail.
        std::fprintf(stderr, "dictionary preparation refused: %s\n  asked: %s\n",
                     document.dump().c_str(), input.c_str());
      }
      require(document.at("ok").get<bool>(),
              "Locked dictionary preparation failed");
      options = document.at("value");
      options["preferences"] = preferences;
    }
    // Protocol definitions and x86/x64 layout assertions are imported, not
    // copied.
    auto hello =
        FanyImeProtocol::Hello(42, 1, FanyImeProtocol::RequiredCapabilities);
    require(
        FanyImeProtocol::Negotiate(hello, FanyImeProtocol::RequiredCapabilities)
            .accepted,
        "Shared protocol negotiation failed");
    {
      using namespace msime::windows;
      FocusGate gate;
      FocusedSession focused(gate, 42, options.dump());
      PipeTicket ticket{42, {1, 2, 3}};
      auto first = *gate.begin(ticket, 77);
      FanyImeNamedpipeData packet{};
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.client_id = 42;
      packet.request_id = 2;
      packet.keycode = 'U';
      packet.wch = 'U';
      packet.modifiers_down = 1;
      require(!focused.key(first.pending, packet, ReplyPath::Composition),
              "Unprepared focus entered Engine");
      require(focused.prepare(first.pending), "Focus preparation failed");
      require(!focused.key(first.pending, packet, ReplyPath::Composition),
              "Unacknowledged focus entered Engine");
      require(focused.view().at("editing_text") == "",
              "Pending focus changed composition");
      require(gate.acknowledge(first.pending, [] { return true; }),
              "Synthetic queue-test fence failed");
      packet.keycode = 'F';
      packet.wch = 'f';
      packet.modifiers_down = 0b11;
      size_t character_set_persists = 0;
      auto persist_character_set = [&](bool desired) {
        require(desired, "Character-set persistence target was inverted");
        ++character_set_persists;
        return true;
      };
      auto disabled_shortcut = focused.toggle_character_set(
          first.pending, packet, false, persist_character_set);
      require(disabled_shortcut && !disabled_shortcut->encoded &&
                  !disabled_shortcut->traditional_output &&
                  character_set_persists == 0 &&
                  focused.confirm(first.pending, packet.request_id),
              "Disabled character-set shortcut changed the session");
      ++packet.request_id;
      require(focused.set_input_enabled(first.pending, false),
              "English-mode fixture failed");
      auto english_shortcut = focused.toggle_character_set(
          first.pending, packet, true, persist_character_set);
      require(english_shortcut && !english_shortcut->encoded &&
                  !english_shortcut->traditional_output &&
                  character_set_persists == 0 &&
                  focused.confirm(first.pending, packet.request_id),
              "English mode changed the character set");
      ++packet.request_id;
      require(focused.set_input_enabled(first.pending, true),
              "Chinese-mode fixture failed");
      auto busy_shortcut = focused.toggle_character_set(
          first.pending, packet, true, [&](bool desired) {
            require(desired, "Busy character-set target was inverted");
            ++character_set_persists;
            return false;
          });
      require(busy_shortcut && !busy_shortcut->encoded &&
                  !busy_shortcut->traditional_output &&
                  character_set_persists == 1 &&
                  focused.confirm(first.pending, packet.request_id),
              "Rejected persistence changed the character set");
      ++packet.request_id;
      auto accepted_shortcut = focused.toggle_character_set(
          first.pending, packet, true, persist_character_set);
      require(accepted_shortcut && !accepted_shortcut->encoded &&
                  accepted_shortcut->traditional_output &&
                  character_set_persists == 2 &&
                  focused.confirm(first.pending, packet.request_id),
              "Accepted character-set shortcut was not applied");
      ++packet.request_id;
      auto restored_shortcut = focused.toggle_character_set(
          first.pending, packet, true, {});
      require(restored_shortcut && !restored_shortcut->encoded &&
                  !restored_shortcut->traditional_output &&
                  focused.confirm(first.pending, packet.request_id),
              "Character-set shortcut did not restore the projection");
      ++packet.request_id;
      packet.keycode = 'U';
      packet.wch = 'U';
      packet.modifiers_down = 1;
      auto initial = focused.key(first.pending, packet, ReplyPath::Composition);
      require(initial && focused.view().at("editing_text") == "U",
              "Focused key did not reach Engine");
      const auto pending_view = focused.view();
      auto updated_preferences = preferences;
      updated_preferences["candidate_page_size"] = 3;
      const auto focused_snapshot = Json{
          {"format_version", 1},
          {"revision", 1},
          {"preferences",
           updated_preferences}}.dump();
      require(!focused.update_preferences(first.pending, focused_snapshot) &&
                  focused.view() == pending_view,
              "Configuration bypassed pending delivery gate");
      ++packet.request_id;
      rejected(
          [&] { focused.key(first.pending, packet, ReplyPath::Composition); });
      require(focused.view() == pending_view,
              "Pending reply allowed another Engine action");
      rejected([&] { focused.set_input_enabled(first.pending, false); });
      rejected([&] { focused.cancel_composition(first.pending); });
      rejected([&] { focused.set_chinese_punctuation(first.pending, false); });
      require(focused.view() == pending_view,
              "Input mode bypassed pending delivery gate");
      rejected([&] { focused.edit(first.pending, packet, TsfPreeditStyle::Pinyin); });
      require(focused.view() == pending_view, "Editing bypassed pending reply gate");
      rejected([&] { focused.basic_key(first.pending, packet, TsfPreeditStyle::Pinyin); });
      rejected([&] {
        focused.configured_key(first.pending, packet, TsfPreeditStyle::Pinyin,
                               {});
      });
      require(focused.view() == pending_view,
              "Configured key bypassed pending reply gate");
      require(focused.pending(first.pending)->source.request_id ==
                  initial->source.request_id,
              "Staged reply could not be recovered without Engine replay");
      require(focused.confirm(first.pending, initial->source.request_id),
              "Delivery confirmation failed");
      require(!focused.pending(first.pending),
              "Confirmed reply remained staged");
      const auto deferred =
          focused.update_preferences(first.pending, focused_snapshot);
      require(
          deferred && deferred->at("deferred") == true,
          "Focused configuration did not reuse shared composition deferral");
      for (char c : std::string("4e2d")) {
        packet.keycode =
            static_cast<uint32_t>(c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c);
        packet.wch = c;
        packet.modifiers_down = 0;
        auto result =
            focused.key(first.pending, packet, ReplyPath::Composition);
        require(result && focused.confirm(first.pending, packet.request_id),
                "Focused Unicode edit failed");
        ++packet.request_id;
      }
      packet.keycode = 0x20;
      packet.wch = 0;
      auto committed = focused.key(first.pending, packet, ReplyPath::Selection);
      require(committed && committed->source.transition.at("commit") == "中" &&
                  committed->encoded &&
                  committed->encoded->packet.candidate_string[0] == 0x4e2d,
              "Focused Unicode commit/reply failed");
      require(focused.confirm(first.pending, packet.request_id),
              "Final delivery confirmation failed");
      const auto applied =
          focused.update_preferences(first.pending, focused_snapshot);
      require(applied && applied->at("deferred") == false,
              "Deferred configuration did not apply after composition ended");
      ++packet.request_id;
      packet.keycode = 'U';
      packet.wch = 'U';
      packet.modifiers_down = 1;
      require(focused.key(first.pending, packet, ReplyPath::Composition)
                  .has_value(),
              "Pending edit failed");
      auto second = *gate.begin(ticket, 78);
      const auto old_view = focused.view();
      require(!focused.key(first.pending, packet, ReplyPath::Composition) &&
                  !focused.confirm(first.pending, packet.request_id) &&
                  focused.view() == old_view,
              "Obsolete focus task reached Engine or confirmed output");
      require(!focused.update_preferences(first.pending, focused_snapshot) &&
                  focused.view() == old_view,
              "Old focus configuration changed Engine state");
      require(!focused.set_input_enabled(first.pending, false) &&
                  focused.view() == old_view,
              "Old focus mode notification changed Engine state");
      require(!focused.set_chinese_punctuation(first.pending, false) &&
                  focused.view() == old_view,
              "Old focus punctuation notification changed Engine state");
      require(!focused.edit(first.pending, packet, TsfPreeditStyle::Pinyin) &&
                  focused.view() == old_view,
              "Old focus edit changed Engine state");
      require(!focused.basic_key(first.pending, packet, TsfPreeditStyle::Pinyin) &&
                  focused.view() == old_view,
              "Old basic key changed Engine state");
      require(!focused.configured_key(first.pending, packet,
                                      TsfPreeditStyle::Pinyin, {}) &&
                  focused.view() == old_view,
              "Old configured key changed Engine state");
      require(focused.prepare(second.pending) &&
                  focused.view().at("editing_text") == "",
              "New activation retained old composition");
      require(!focused.cancel(first.pending),
              "Old cancellation cleared new prepared session");
      auto wrong_thread = std::async(std::launch::async, [&] {
        try {
          focused.prepare(second.pending);
        } catch (const std::logic_error &) {
          return true;
        }
        return false;
      });
      require(wrong_thread.get(),
              "Focused adapter accepted wrong queue thread");
      auto other = *gate.begin({99, {4, 5, 6}}, 79);
      require(focused.cancel(second.pending), "Previous owner cleanup failed");
      require(gate.with_pending(other.pending, [] {}),
              "Old owner cleanup invalidated new focus");
    }
    {
      using namespace msime::windows;
      FocusGate gate;
      FocusRouter router(gate, 2);
      FocusedSession first(gate, 42, options.dump());
      FocusedSession second(gate, 43, options.dump());
      PipeTicket a{42, {1, 2, 3}}, b{43, {4, 5, 6}};
      require(router.connected(a).accepted && router.connected(b).accepted,
              "Queue router registration failed");
      FanyImeNamedpipeData packet{};
      packet.client_id = 42;
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.request_id = 77;
      auto activation = router.dispatch(a, packet);
      require(activation.route && first.prepare(*activation.route),
              "Routed activation did not prepare Engine");
      require(gate.acknowledge(*activation.route, [] { return true; }) &&
                  router.confirmed(*activation.route),
              "Synthetic routed fence failed");
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.request_id = 2;
      packet.keycode = 'U';
      packet.wch = 'U';
      packet.modifiers_down = 1;
      auto key_route = router.dispatch(a, packet);
      require(key_route.route &&
                  first.key(*key_route.route, packet, ReplyPath::Composition) &&
                  first.view().at("editing_text") == "U",
              "Routed key did not reach shared Engine");
      packet.client_id = 43;
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.request_id = 88;
      auto takeover = router.dispatch(b, packet);
      require(takeover.cleanup && first.cancel(*takeover.cleanup) &&
                  first.view().at("editing_text") == "" &&
                  takeover.route && second.prepare(*takeover.route),
              "Focus takeover failed to clean old Engine before preparation");
      require(!first.confirm(*key_route.route, 2),
              "Displaced pending reply was acknowledged");
      require(router.disconnected(b).accepted &&
                  second.cancel(*takeover.route),
              "Disconnected queue session cleanup failed");
    }
    {
      using namespace msime::windows;
      FocusGate gate;
      InputQueue queue(gate, 2, 8, options.dump());
      const auto run = [&](InputQueue::Task task) {
        auto completion = queue.submit(std::move(task));
        require(completion && completion->get() == InputTaskStatus::Completed,
                "Shared input queue task failed");
      };
      PipeTicket a{42, {1, 2, 3}}, b{43, {4, 5, 6}};
      FocusRoute activation;
      FanyImeNamedpipeData packet{};
      packet.client_id = 42;
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.request_id = 77;
      run([&](InputState &state) {
        require(state.connected(a).accepted && state.connected(b).accepted,
                "Worker session creation failed");
        activation = state.dispatch(a, packet);
        require(activation.route.has_value(), "Worker activation failed");
      });
      // Synthetic external I/O completion: never perform pipe I/O in a task.
      require(gate.acknowledge(*activation.route, [] { return true; }),
              "Worker queue synthetic fence failed");
      run([&](InputState &state) {
        require(state.confirmed(*activation.route), "Worker fence receipt failed");
      });
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.request_id = 2;
      for (char c : std::string("U4e2d")) {
        packet.keycode = static_cast<uint32_t>(c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c);
        packet.wch = c;
        packet.modifiers_down = c == 'U' ? 1 : 0;
        run([&](InputState &state) {
          auto route = state.dispatch(a, packet);
          require(route.route.has_value(), "Worker key route missing");
          auto result = state.key(*route.route, packet, ReplyPath::Composition);
          require(result && result->encoded && *result->encoded,
                  "Worker composition failed");
        });
        run([&](InputState &state) {
          require(state.delivered(*activation.route, packet.request_id),
                  "Worker reply receipt failed");
        });
        ++packet.request_id;
      }

      packet.keycode = 0xBB;
      packet.wch = '+';
      NavigationBindings navigation_bindings{true, true, true,
                                             true, true, true};
      run([&](InputState &state) {
        require(!state.navigate(*activation.route, packet, navigation_bindings),
                "Unicode plus was consumed by paging");
      });
      packet.keycode = 0x09;
      packet.wch = 0;
      run([&](InputState &state) {
        auto stale = *activation.route;
        ++stale.epoch;
        require(!state.navigate(stale, packet, navigation_bindings),
                "Stale focus navigated candidates");
        auto result =
            state.navigate(*activation.route, packet, navigation_bindings);
        require(result && result->encoded->packet.msg_type ==
                              FanyImeReplyType::MovePageNext,
                "Queue navigation did not encode a boundary reply");
        rejected([&] {
          state.navigate(*activation.route, packet, navigation_bindings);
        });
        require(state.delivered(*activation.route, packet.request_id),
                "Queue navigation receipt failed");
      });
      ++packet.request_id;
      packet.keycode = 0x20;
      packet.wch = 0;
      run([&](InputState &state) {
        auto result = state.key(*activation.route, packet, ReplyPath::Selection);
        require(result && result->source.transition.at("commit") == "中",
                "Dedicated worker Unicode commit failed");
        require(state.delivered(*activation.route, packet.request_id),
                "Worker commit receipt failed");
      });
      packet.client_id = 43;
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.request_id = 88;
      FocusRoute other;
      run([&](InputState &state) {
        other = state.dispatch(b, packet);
        require(other.activation && other.cleanup &&
                    !state.delivered(*activation.route, 2),
                "Worker focus takeover failed");
        require(!state.disconnected({42, {7, 2, 3}}).accepted,
                "Stale worker disconnect accepted");
      });
      queue.stop(); // Destroys both actual Rust/C++ sessions on the worker.
      require(!gate.with_pending(*other.route, [] {}),
              "Stopped worker retained focus authorization");
      InputQueue failing(gate, 1, 2, options.dump());
      packet.client_id = 42;
      auto failure = failing.submit([&](InputState &state) {
        require(state.connected(a).accepted, "Failure fixture registration failed");
        activation = state.dispatch(a, packet);
        require(activation.route.has_value(), "Failure fixture activation failed");
        throw std::runtime_error("Synthetic active-session failure");
      });
      require(failure && failure->get() == InputTaskStatus::Failed &&
                  !gate.with_pending(*activation.route, [] {}),
              "Failed task reported before withdrawing Engine authorization");
      failing.stop();
    }
    ui_selection_tests(options.dump(), argc == 2);
    candidate_mailbox_tests();
    session_pump_tests(options.dump());
    {
      const auto native_options = test_host_options(root / "native-config");
      auto incomplete = native_options;
      incomplete["preferences"].erase("candidate_page_size");
      incomplete["preferences"].erase("chinese_punctuation");
      rejected([&] { ServerSession invalid(42, incomplete.dump()); });
      ServerSession native(42, native_options.dump());
      native.activate(1);
      require(native.view().at("editing_text") == "",
              "Native fixture options failed shared host initialization");
    }
    session_worker_tests(options.dump());
    preference_monitor_tests(options.dump(), directory("monitor-preferences"));
    {
      using namespace msime::windows;
      FocusGate gate;
      InputQueue queue(gate, 1, 8, options.dump());
      // A require() inside a queued task throws on the worker thread, where the
      // queue turns it into a status and the message is lost - every failure in
      // here then reported as "Preference retry task failed" whatever actually
      // went wrong. Say what it was before letting the status speak.
      auto run = [&](InputQueue::Task task) {
        InputQueue::Task reported =
            [task = std::move(task)](InputState &state) mutable {
              try {
                task(state);
              } catch (const std::exception &error) {
                std::fprintf(stderr, "queued task failed: %s\n", error.what());
                throw;
              }
            };
        auto result = queue.submit(std::move(reported));
        require(result && result->get() == InputTaskStatus::Completed,
                "Preference retry task failed");
      };
      PipeTicket ticket{42, {21, 22, 23}};
      FanyImeNamedpipeData packet{};
      packet.event_type = FanyImePipeEventType::ClientActivated;
      packet.client_id = 42;
      packet.request_id = 77;
      FocusLease lease;
      run([&](InputState &state) {
        require(state.connected(ticket).accepted, "Preference client failed");
        lease = *state.dispatch(ticket, packet).route;
      });
      require(gate.acknowledge(lease, [] { return true; }),
              "Preference fence failed");
      auto first_preferences = preferences;
      first_preferences["candidate_page_size"] = 3;
      first_preferences["candidate_translations"] = true;
      auto latest_preferences = preferences;
      latest_preferences["candidate_page_size"] = 4;
      const auto first = Json{{"format_version", 1},
                              {"revision", 1},
                              {"preferences", first_preferences}}
                             .dump();
      const auto latest = Json{{"format_version", 1},
                               {"revision", 2},
                               {"preferences", latest_preferences}}
                              .dump();
      const auto preference_directory = directory("preferences-source");
      auto load_snapshot = [&](const std::string &contents) {
        std::ofstream file(std::filesystem::path(preference_directory) /
                           "preferences.json");
        file << contents;
        file.close();
        require(static_cast<bool>(file), "Synthetic preference write failed");
        return PreferenceSnapshot::load(preference_directory);
      };
      const auto first_snapshot = load_snapshot(first);
      const auto latest_snapshot = load_snapshot(latest);
      const auto attempted = PreferenceSnapshot::try_load(preference_directory);
      require(attempted && attempted->serialized() == latest_snapshot.serialized(),
              "Try-load did not return shared validated snapshot");
      auto conflicting = Json::parse(latest);
      conflicting["preferences"] = first_preferences;
      const auto conflicting_snapshot = load_snapshot(conflicting.dump());
      {
        FocusGate maintenance_gate;
        InputQueue maintenance_queue(maintenance_gate, 1, 2, options.dump());
        PipeTicket maintenance_ticket{314, {41, 42, 43}};
        auto maintenance = maintenance_queue.submit(
            [&](InputState &state) {
              require(state.connected(maintenance_ticket).accepted,
                      "Maintenance preference client failed");
              require(state.quiesce_dictionaries() == 1,
                      "Maintenance did not release the session");
              state.publish_preferences(latest_snapshot);
              require(state.resume_dictionaries() == 1,
                      "Maintenance did not rebuild the session");
            });
        require(maintenance && maintenance->get() == InputTaskStatus::Completed,
                "Preference publication during maintenance failed");
        maintenance_queue.stop();
      }
      rejected([&] { load_snapshot("broken"); });
      auto invalid = Json::parse(latest);
      invalid["preferences"]["candidate_page_size"] = 0;
      rejected([&] { load_snapshot(invalid.dump()); });
      run([&](InputState &state) {
        require(state.confirmed(lease), "Preference focus receipt failed");
        state.publish_preferences(first_snapshot);
        packet.event_type = FanyImePipeEventType::KeyEvent;
        packet.request_id = 2;
        packet.keycode = 'N';
        packet.wch = 'n';
        packet.modifiers_down = 0;
        auto first_pending = state.key(lease, packet, ReplyPath::Composition);
        require(first_pending.has_value(), "Preference pending key missing");
        require(state.delivered(lease, packet.request_id),
                "First preference key receipt failed");
        packet.request_id = 3;
        packet.keycode = 'I';
        packet.wch = 'i';
        packet.modifiers_down = 0;
        auto pending = state.key(lease, packet, ReplyPath::Composition);
        require(pending.has_value(), "Second preference pending key missing");
        const auto translation_request = state.current_translation_request();
        require(translation_request.has_value(),
                "Active candidate translation was not re-dispatched");
        const auto translation_document =
            Json::parse(translation_request->second);
        require(translation_document.at("generation").is_number_unsigned() &&
                    translation_document.at("candidates").is_array() &&
                    !translation_document.at("candidates").empty(),
                "Re-dispatched translation query was incomplete");
        state.publish_preferences(first_snapshot);
        state.publish_preferences(latest_snapshot);
        state.publish_preferences(latest_snapshot);
        rejected([&] { state.publish_preferences(first_snapshot); });
        rejected([&] { state.publish_preferences(conflicting_snapshot); });
        rejected([&] { state.queue_preferences(lease, first); });
        auto conflict = Json::parse(latest);
        conflict["preferences"] = first_preferences;
        rejected([&] { state.queue_preferences(lease, conflict.dump()); });
        rejected(
            [&] { state.queue_preferences(lease, std::string(16385, 'x')); });
        auto stale = lease;
        ++stale.epoch;
        require(!state.queue_preferences(stale, latest),
                "Stale preferences accepted");
        rejected([&] { state.delivered(lease, 99); });
        require(state.delivered(lease, packet.request_id),
                "Preference retry receipt failed");
        // The host must have received revision 2 automatically on delivery,
        // even though its application waits for the active composition to end.
        rejected([&] { state.update_preferences(lease, first); });
        const auto deferred = state.update_preferences(lease, latest);
        require(deferred && deferred->at("deferred") == true,
                "Queued preferences were not handed to shared deferral");
        packet.request_id = 4;
        packet.keycode = 0x1B;
        packet.wch = 0;
        auto cancelled = state.key(lease, packet, ReplyPath::LocalCancel);
        require(cancelled && state.delivered(lease, packet.request_id),
                "Preference completion reset failed");
        const auto applied = state.update_preferences(lease, latest);
        require(applied && applied->at("deferred") == false,
                "Latest preferences did not apply after reset");
        packet.request_id = 5;
        packet.keycode = 'U';
        packet.wch = 'U';
        require(state.key(lease, packet, ReplyPath::Composition).has_value(),
                "Second preference composition missing");
        auto abandoned = Json::parse(latest);
        abandoned["revision"] = 3;
        abandoned["preferences"]["candidate_page_size"] = 5;
        require(state.queue_preferences(lease, abandoned.dump()),
                "Abandoned snapshot not queued");
        state.failed(lease);
        require(!state.delivered(lease, 5), "Cancelled reply was confirmed");
        packet.event_type = FanyImePipeEventType::ClientActivated;
        packet.request_id = 88;
        lease = *state.dispatch(ticket, packet).route;
      });
      require(gate.acknowledge(lease, [] { return true; }),
              "Replacement preference fence failed");
      run([&](InputState &state) {
        require(state.confirmed(lease),
                "Replacement preference receipt failed");
        require(state.queue_preferences(lease, latest),
                "Old focus leaked a newer pending snapshot");
        require(state.disconnected(ticket).accepted,
                "Preference disconnect failed");
        ticket.generations = {31, 32, 33};
        require(state.connected(ticket).accepted,
                "Fresh preference client failed");
        packet.request_id = 99;
        lease = *state.dispatch(ticket, packet).route;
        // Publication during pending activation is retained for confirmation,
        // without granting input authorization before the fence.
        state.publish_preferences(latest_snapshot);
      });
      require(gate.acknowledge(lease, [] { return true; }),
              "Fresh preference fence failed");
      run([&](InputState &state) {
        require(state.confirmed(lease), "Fresh preference confirmation failed");
        rejected([&] { state.update_preferences(lease, first); });
        auto applied = state.update_preferences(lease, latest);
        require(applied && applied->at("deferred") == false,
                "Fresh session did not inherit latest published preferences");
      });
      queue.stop();
    }
    ServerSession session(42, options.dump());
    uint64_t request = 2;
    uint64_t epoch = 1;
    auto key = [&](uint32_t vk, uint32_t text = 0, uint32_t modifiers = 0) {
      FanyImeNamedpipeData packet{};
      packet.event_type = FanyImePipeEventType::KeyEvent;
      packet.client_id = 42;
      packet.request_id = request++;
      packet.keycode = vk;
      packet.wch = static_cast<FanyImeWireChar>(text);
      packet.modifiers_down = modifiers;
      // TSF's preview text cannot overwrite the Engine's composition owner.
      packet.pinyin_length = 3;
      packet.pinyin_string[0] = 'x';
      auto result = session.key(packet, epoch);
      require(result.request_id == packet.request_id &&
                  result.client_id == 42 && result.activation_epoch == epoch,
              "Reply routing metadata lost");
      return result;
    };
    rejected([&] { key('U', 'U', 1); });
    session.activate(epoch);
    {
      const bool before = session.traditional_output();
      const auto toggled = session.toggle_traditional_output(epoch);
      require(toggled.at("session") == session.view().at("session") &&
                  session.traditional_output() == !before,
              "Character-set shortcut did not toggle the host output projection");
      (void)session.toggle_traditional_output(epoch);
      require(session.traditional_output() == before,
              "Character-set shortcut did not restore the host output projection");
    }
    {
      using namespace msime::windows;
      key('U', 'U', 1);
      const auto unchanged = session.view();
      ReplyComposer composer(42, epoch);
      FanyImeNamedpipeData enter{};
      enter.client_id = 42;
      enter.event_type = FanyImePipeEventType::KeyEvent;
      enter.request_id = request++;
      enter.keycode = 0x0D;
      rejected([&] { composer.dispatch(session, enter, epoch, ReplyPath::LocalCommit,
                                       false, "different"); });
      require(session.view() == unchanged && !composer.has_pending(),
              "Rejected local commit consumed Engine composition");
      rejected([&] { composer.dispatch(session, enter, epoch, ReplyPath::LocalCommit); });
      require(session.view() == unchanged && !composer.has_pending(),
              "Missing local text consumed Engine composition");
      auto wrong_key = enter;
      wrong_key.keycode = 'A';
      wrong_key.wch = 'a';
      rejected([&] { composer.dispatch(session, wrong_key, epoch, ReplyPath::LocalCommit,
                                       false, "U"); });
      require(session.view() == unchanged && !composer.has_pending(),
              "Non-Enter local commit advanced Engine");
      const auto local = composer.dispatch(session, enter, epoch, ReplyPath::LocalCommit,
                                            false, "U");
      require(!local.encoded && local.source.transition.at("commit") == "U" &&
                  session.view().at("editing_text") == "",
              "Validated local Enter changed its text or emitted a reply");
      composer.confirm_delivery(42, epoch, enter.request_id);
    }
    key('U', 'U', 1);
    session.set_input_enabled(epoch, false);
    const auto closed_view = session.view();
    require(closed_view.at("editing_text") == "" && !session.input_enabled(),
            "Closing input retained composition");
    session.set_input_enabled(epoch, false);
    require(session.view() == closed_view,
            "Repeated closed notification changed generation");
    session.deactivate(epoch);
    session.activate(++epoch);
    require(!session.input_enabled() &&
                key('U', 'U', 1).transition.at("handled") == false &&
                session.view().at("editing_text") == "",
            "Reactivation forgot closed input mode");
    session.set_input_enabled(epoch, true);
    key('U', 'U', 1 | FanyImePipeFlags::UiLess);
    for (char c : std::string("4e2d"))
      key(static_cast<uint32_t>(c >= 'a' && c <= 'z' ? c - 'a' + 'A' : c), c);
    auto before = session.view();
    require(before.at("editing_text") == "U4e2d" && before.at("local_mode") == "unicode",
            "TSF text overwrote shared composition");
    auto stale_generation = before.at("generation").get<uint64_t>();
    preferences["chinese_punctuation"] = false;
    auto snapshot = Json{
        {"format_version", 1},
        {"revision", 1},
        {"preferences",
         preferences}}.dump();
    require(session.update_preferences(epoch, snapshot).at("deferred") == true,
            "Active preferences not deferred");
    const auto no_selection = key('9', '(', 1);
    require(no_selection.transition.at("commit").is_null() &&
                session.view() == before,
            "Out-of-page Unicode selection changed composition");
    auto selected = key('1', '!', 1 | FanyImePipeFlags::UiLess);
    require(selected.transition.at("commit") == "中" && selected.reply_expected,
            "Unicode commit failed");
    require(selected.transition.at("view").at("local_mode") == "none",
            "Committed session retained Unicode mode");
    auto reply = msime::windows::candidate_commit(
        selected.request_id,
        selected.transition.at("commit").get<std::string>());
    require(reply && reply.packet.request_id == selected.request_id &&
                reply.packet.candidate_string[0] == 0x4E2D,
            "Shared result did not encode into the candidate reply");
    rejected([&] { session.select(epoch, stale_generation, 0); });
    require(key(0xBC, ',').transition.at("handled") == false,
            "Deferred ASCII punctuation not applied");
    require(key('1', '!', 1).transition.at("handled") == false,
            "Shift digit outside Unicode mode ignored translated punctuation");
    for (uint32_t flags : {1u, 1u | FanyImePipeFlags::UiLess}) {
      key('U', 'U', 1);
      key(0x64); // Numpad hex digits must compose without Shift.
      key('E', 'e');
      key(0x62);
      key('D', 'd');
      const auto composed = session.view();
      require(composed.at("editing_text") == "U4e2d",
              "Numpad digits did not compose Unicode input");
      const auto outside = key(0x69, 0, flags);
      require(outside.reply_expected &&
                  outside.transition.at("commit").is_null() && session.view() == composed,
              "Out-of-page numpad selection changed Unicode composition");
      const auto chosen = key(0x61, 0, flags);
      require(chosen.reply_expected && chosen.transition.at("commit") == "中" &&
                  session.view().at("local_mode") == "none",
              "Shift numpad did not select Unicode candidate");
    }
    key('U', 'U', 1);
    key(0x60);
    require(session.view().at("editing_text") == "U0",
            "Numpad zero stopped composing Unicode input");
    const auto numpad_shortcut = key(0x61, 0, 3 | FanyImePipeFlags::UiLess);
    require(!numpad_shortcut.transition.at("handled").get<bool>() &&
                numpad_shortcut.transition.at("commit").is_null() &&
                session.view().at("editing_text") == "",
            "Control Shift numpad shortcut selected a Unicode candidate");
    key('U', 'U', 1);
    auto shortcut = key('C', 0, 2);
    require(shortcut.transition.at("handled") == false &&
                session.view().at("editing_text") == "",
            "Shortcut did not cancel and forward");
    key('U', 'U', 1);
    FanyImeNamedpipeData reset{};
    reset.event_type = FanyImePipeEventType::KeyEvent;
    reset.client_id = 42;
    reset.keycode = 0x10;
    require(!session.key(reset, epoch).reply_expected &&
                session.view().at("editing_text") == "",
            "Local TSF reset emitted reply or kept composition");
    reset.keycode = 'A';
    rejected([&] { session.key(reset, epoch); });
    reset.request_id = 99;
    reset.client_id = 43;
    rejected([&] { session.key(reset, epoch); });
    bool wrong_thread = false;
    std::thread worker([&] {
      try {
        session.view();
      } catch (const std::logic_error &) {
        wrong_thread = true;
      }
    });
    worker.join();
    require(wrong_thread, "Wrong thread accessed the shared session");
    key('U', 'U', 1);
    session.deactivate(epoch);
    require(session.view().at("editing_text") == "",
            "Focus loss retained composition");
    rejected([&] { session.activate(epoch); });
    rejected([&] { key('U', 'U', 1); });
    session.activate(++epoch);
    rejected([&] { session.deactivate(epoch - 1); });
    if (argc == 2) {
      for (bool last : {false, true}) {
        using namespace msime::windows;
        ReplyComposer composer(42, epoch);
        for (char c : std::string("nihao"))
          key(c - 'a' + 'A', c);
        FanyImeNamedpipeData packet{};
        packet.client_id = 42;
        packet.event_type = FanyImePipeEventType::KeyEvent;
        packet.request_id = request++;
        packet.keycode = last ? 0xDD : 0xDB;
        packet.wch = last ? ']' : '[';
        const auto result = composer.configured_key(
            session, packet, epoch, TsfPreeditStyle::Local, {}, std::nullopt,
            WordCharacterBinding::Brackets);
        require(result &&
                    result->source.transition.at("commit") ==
                        (last ? "好" : "你") &&
                    result->encoded->packet.msg_type ==
                        FanyImeReplyType::CommitExactText,
                "Word-to-character did not select the Engine Han edge");
        const auto pending_view = session.view();
        rejected([&] {
          composer.configured_key(session, packet, epoch,
                                  TsfPreeditStyle::Local, {}, std::nullopt,
                                  WordCharacterBinding::Brackets);
        });
        require(session.view() == pending_view,
                "Word-to-character replay changed pending state");
        composer.confirm_delivery(42, epoch, packet.request_id);
      }
      {
        using namespace msime::windows;
        // Enter while the candidate page is active must select the
        // highlighted candidate. The same VK is raw-local commit only after
        // a partial prefix has already been retained by ReplyComposer.
        ReplyComposer enter_composer(42, epoch);
        for (char c : std::string("nihao"))
          key(c - 'a' + 'A', c);
        FanyImeNamedpipeData enter{};
        enter.client_id = 42;
        enter.event_type = FanyImePipeEventType::KeyEvent;
        enter.request_id = request++;
        enter.keycode = 0x0D;
        enter.modifiers_down = PipeMetadata::CandidateActive;
        const auto pending = enter_composer.configured_key(
            session, enter, epoch, TsfPreeditStyle::Pinyin, {});
        require(pending && pending->source.transition.at("commit") == "你好" &&
                    pending->encoded && *pending->encoded &&
                    pending->encoded->packet.msg_type == FanyImeReplyType::Normal &&
                    session.view().at("editing_text") == "",
                "Candidate-state Enter did not commit the highlighted candidate");
        enter_composer.confirm_delivery(42, epoch, enter.request_id);
      }
      {
        using namespace msime::windows;
        // Without the explicit candidate-mode metadata, Enter follows the
        // legacy TSF raw-commit path even when the Engine still publishes a
        // candidate page. The local observation proves the TSF-owned text.
        ReplyComposer raw_composer(42, epoch);
        for (char c : std::string("nihao"))
          key(c - 'a' + 'A', c);
        FanyImeNamedpipeData enter{};
        enter.client_id = 42;
        enter.event_type = FanyImePipeEventType::KeyEvent;
        enter.request_id = request++;
        enter.keycode = 0x0D;
        enter.pinyin_length = 5;
        for (size_t i = 0; i < 5; ++i)
          enter.pinyin_string[i] = "nihao"[i];
        const auto pending = raw_composer.configured_key(
            session, enter, epoch, TsfPreeditStyle::Pinyin, {},
            std::string("nihao"));
        require(pending && !pending->encoded &&
                    pending->source.transition.at("commit") == "nihao" &&
                    session.view().at("editing_text") == "",
                "Raw Enter without candidate metadata did not clear Engine");
        raw_composer.confirm_delivery(42, epoch, enter.request_id);
      }
      {
        using namespace msime::windows;
        for (char c : std::string("nihao")) key(c - 'a' + 'A', c);
        // Multiple dictionary senses use a short-lived candidate page. The
        // original Engine composition remains untouched until a sense is
        // chosen, so the page can be dismissed without losing the input.
        auto multi_view = session.view();
        require(session.apply_translations(
                    epoch, multi_view.at("generation").get<uint64_t>(),
                    R"([{"text":"你好","translation":"hello; greeting; salutation"}])").has_value(),
                "Synthetic multi-sense translation was not applied");
        FanyImeNamedpipeData translation_enter{};
        translation_enter.client_id = 42;
        translation_enter.event_type = FanyImePipeEventType::KeyEvent;
        translation_enter.keycode = 0x0D;
        translation_enter.modifiers_down = PipeMetadata::CandidateActive | 2u;
        ReplyComposer multi(42, epoch);
        translation_enter.request_id = request++;
        const auto multi_page = multi.configured_key(
            session, translation_enter, epoch, TsfPreeditStyle::Pinyin, {});
        require(multi_page && multi_page->encoded &&
                    multi_page->encoded->packet.msg_type ==
                        FanyImeReplyType::NavigationIgnored &&
                    multi_page->source.transition.at("view").at("candidates").size() == 3 &&
                    multi_page->source.transition.at("view").at("candidates").at(1).at("text") ==
                        "greeting",
                "Ctrl+Enter did not open the multi-sense translation page");
        multi.confirm_delivery(42, epoch, translation_enter.request_id);
        FanyImeNamedpipeData multi_next = translation_enter;
        multi_next.request_id = request++;
        multi_next.keycode = 0x22;
        multi_next.modifiers_down = PipeMetadata::CandidateActive;
        const auto multi_navigated = multi.basic_key(
            session, multi_next, epoch, TsfPreeditStyle::Pinyin);
        require(multi_navigated && multi_navigated->encoded &&
                    multi_navigated->encoded->packet.msg_type ==
                        FanyImeReplyType::NavigationIgnored &&
                    multi_navigated->source.transition.at("view").at("candidates").size() == 3,
                "Translation page navigation left the multi-sense page");
        multi.confirm_delivery(42, epoch, multi_next.request_id);
        FanyImeNamedpipeData multi_digit = translation_enter;
        multi_digit.request_id = request++;
        multi_digit.keycode = '2';
        multi_digit.modifiers_down = PipeMetadata::CandidateActive;
        const auto multi_selected = multi.basic_key(
            session, multi_digit, epoch, TsfPreeditStyle::Pinyin);
        require(multi_selected && multi_selected->encoded &&
                    multi_selected->encoded->packet.msg_type ==
                        FanyImeReplyType::CommitExactText,
                "Translation page digit did not commit exact sense");
        multi.confirm_delivery(42, epoch, multi_digit.request_id);
        session.deactivate(epoch);
        session.activate(++epoch);
        ReplyComposer basic(42, epoch);
        for (char c : std::string("nihao")) key(c - 'a' + 'A', c);
        const auto translated_view = session.view();
        require(session.apply_translations(
                    epoch, translated_view.at("generation").get<uint64_t>(),
                    R"([{"text":"你好","translation":"hello"}])").has_value(),
                "Synthetic candidate translation was not applied");
        translation_enter.request_id = request++;
        const auto translated = basic.configured_key(
            session, translation_enter, epoch, TsfPreeditStyle::Pinyin, {});
        // A single sense is committed as exact text, like the reference, and never selects the candidate the Engine would learn from.
        require(translated && !translated->ui_selection && translated->encoded &&
                    translated->encoded->packet.msg_type ==
                        FanyImeReplyType::CommitExactText &&
                    translated->next_prefix.empty() &&
                    session.view().at("editing_text") == "",
                "Ctrl+Enter did not commit the highlighted translation as exact text");
        basic.confirm_delivery(42, epoch, translation_enter.request_id);
        const auto unchanged = session.view();
        FanyImeNamedpipeData digit{};
        digit.client_id = 42;
        digit.event_type = FanyImePipeEventType::KeyEvent;
        digit.request_id = request++;
        digit.keycode = 'C';
        digit.modifiers_down = 2;
        require(!basic.basic_key(session, digit, epoch, TsfPreeditStyle::Pinyin) &&
                    session.view() == unchanged && !basic.has_pending(),
                "Basic dispatcher consumed a native shortcut");
        digit.keycode = 0xBC;
        digit.wch = ',';
        digit.modifiers_down = 0;
        require(!basic.basic_key(session, digit, epoch, TsfPreeditStyle::Pinyin) &&
                    session.view() == unchanged && !basic.has_pending(),
                "Basic dispatcher consumed priority punctuation");
        // The two checks above ran against an ended composition: Ctrl+Enter
        // committed the translation, so there was nothing left to disturb, and
        // "the view did not change" is what they are really asserting.
        //
        // Selecting a candidate needs one, so start a fresh composition here.
        // Without this the digit below has nothing to select, the Engine
        // commits nothing, and the reply is NavigationIgnored - which is
        // correct behaviour for an empty composition and was read as the
        // layout's punctuation having replaced the selection.
        for (char c : std::string("nihao")) key(c - 'a' + 'A', c);
        require(!session.view().at("candidates").empty(),
                "Retyped input produced no candidate to select");
        digit.request_id = request++;
        digit.keycode = '1';
        digit.wch = '&'; // An unshifted digit VK on a non-US layout.
        const auto layout_selected =
            basic.basic_key(session, digit, epoch, TsfPreeditStyle::Pinyin);
        require(layout_selected &&
                    layout_selected->source.transition.at("commit") == "你好" &&
                    layout_selected->encoded->packet.msg_type ==
                        FanyImeReplyType::Normal,
                "Layout-produced punctuation replaced digit selection");
        basic.confirm_delivery(42, epoch, digit.request_id);
      }
      for (char c : std::string("nihao"))
        key(c - 'a' + 'A', c);
      require(session.view().at("candidates").at(0).at("text") == "你好",
              "Published dictionary query failed");
      key(0x22);
      auto page = session.view();
      require(page.at("page") == 1, "Shared page did not advance");
      auto expected = page.at("candidates").at(0).at("text");
      require(key(0x61).transition.at("commit") == expected,
              "Numpad did not select shared page global index");
      // Start a clean activation and select a real partial candidate from the
      // locked dictionary. Do not synthesize an Engine remainder for this test.
      session.deactivate(epoch);
      session.activate(++epoch);
      msime::windows::ReplyComposer composer(42, epoch);
      auto send = [&](uint32_t vk, uint32_t text,
                      msime::windows::ReplyPath path, bool uiless = false) {
        FanyImeNamedpipeData packet{};
        packet.event_type = FanyImePipeEventType::KeyEvent;
        packet.client_id = 42;
        packet.request_id = request++;
        packet.keycode = vk;
        packet.wch = static_cast<FanyImeWireChar>(text);
        const auto &pending =
            composer.dispatch(session, packet, epoch, path, uiless);
        auto copy = pending;
        auto unchanged = session.view();
        rejected([&] { composer.dispatch(session, packet, epoch, path); });
        require(session.view() == unchanged,
                "Pending-reply gate advanced Engine");
        require(!pending.encoded || static_cast<bool>(*pending.encoded),
                "Real reply cannot be encoded");
        composer.confirm_delivery(42, epoch, packet.request_id);
        return copy;
      };
      for (char c : std::string("nihao"))
        send(c - 'a' + 'A', c, msime::windows::ReplyPath::Composition);
      {
        using namespace msime::windows;
        for (bool uiless : {false, true}) {
          for (auto keys : {std::pair<uint32_t, uint32_t>{0xBD, 0xBB},
                            {0xBC, 0xBE},
                            {0xDB, 0xDD},
                            {0x21, 0x22},
                            {0x09, 0x09},
                            {0x26, 0x28}}) {
            NavigationBindings bindings;
            bindings.minus_equal = keys.first == 0xBD;
            bindings.comma_period = keys.first == 0xBC;
            bindings.brackets = keys.first == 0xDB;
            bindings.page_up_down = keys.first == 0x21;
            bindings.tab = keys.first == 0x09;
            bindings.arrows = keys.first == 0x26;
            for (bool previous : {false, true}) {
              FanyImeNamedpipeData packet{};
              packet.event_type = FanyImePipeEventType::KeyEvent;
              packet.client_id = 42;
              packet.request_id = request++;
              packet.keycode = previous ? keys.first : keys.second;
              packet.modifiers_down =
                  (uiless ? FanyImePipeFlags::UiLess : 0) |
                  (previous && packet.keycode == 0x09 ? 1u : 0u);
              const auto before_navigation = session.view();
              const auto disabled =
                  composer.navigate(session, packet, epoch, {});
              require(disabled && disabled->encoded && *disabled->encoded &&
                          disabled->encoded->packet.msg_type ==
                              (uiless ? FanyImeReplyType::UiLessComposition
                                      : FanyImeReplyType::NavigationIgnored),
                      "Disabled navigation failed to reply");
              require(session.view() == before_navigation,
                      "Disabled binding mutated Engine");
              rejected(
                  [&] { composer.navigate(session, packet, epoch, bindings); });
              composer.confirm_delivery(42, epoch, packet.request_id);
              packet.request_id = request++;
              auto control_shortcut = packet;
              control_shortcut.modifiers_down |= 2;
              require(!composer.navigate(session, control_shortcut, epoch,
                                         bindings),
                      "Control shortcut was consumed as navigation");
              auto result = composer.navigate(session, packet, epoch, bindings);
              const auto expected_type =
                  bindings.arrows
                      ? (previous ? FanyImeReplyType::MoveSelectionPrevious
                                  : FanyImeReplyType::MoveSelectionNext)
                      : (previous ? FanyImeReplyType::MovePagePrevious
                                  : FanyImeReplyType::MovePageNext);
              require(result && result->encoded && *result->encoded &&
                          result->encoded->packet.msg_type ==
                              (uiless ? FanyImeReplyType::UiLessComposition
                                      : expected_type),
                      "Configured navigation reply incorrect");
              require(session.view().at("page") ==
                              (previous || bindings.arrows ? 0 : 1) &&
                          session.view().at("editing_text") == "nihao",
                      "Configured navigation did not use shared paging");
              if (bindings.arrows)
                require(session.view()
                                .at("candidates")
                                .at(previous ? 0 : 1)
                                .at("highlighted") == true,
                        "Configured arrows did not move shared highlight");
              rejected(
                  [&] { composer.navigate(session, packet, epoch, bindings); });
              composer.confirm_delivery(42, epoch, packet.request_id);
            }
          }
        }
      }
      for (bool uiless : {false, true}) {
        const auto original = session.view();
        auto navigate = [&](uint32_t vk, msime::windows::ReplyPath path,
                            uint32_t type) {
          auto reply = send(vk, 0, path, uiless);
          require(reply.encoded->packet.msg_type ==
                      (uiless ? FanyImeReplyType::UiLessComposition : type),
                  "Real navigation used wrong reply type");
          require(session.view().at("editing_text") ==
                      original.at("editing_text"),
                  "Navigation changed Engine composition");
          if (!uiless)
            require(reply.encoded->packet.candidate_string[0] == 0,
                    "Normal navigation included text");
        };
        navigate(0x21, msime::windows::ReplyPath::PreviousPage,
                 FanyImeReplyType::MovePagePrevious);
        require(session.view().at("page") == 0,
                "Previous page crossed first-page boundary");
        navigate(0x22, msime::windows::ReplyPath::NextPage,
                 FanyImeReplyType::MovePageNext);
        require(session.view().at("page") == 1,
                "Navigation reply did not use shared paging");
        navigate(0x21, msime::windows::ReplyPath::PreviousPage,
                 FanyImeReplyType::MovePagePrevious);
        navigate(0x28, msime::windows::ReplyPath::NextCandidate,
                 FanyImeReplyType::MoveSelectionNext);
        require(session.view().at("candidates").at(1).at("highlighted") == true,
                "Navigation reply did not use shared highlight");
        navigate(0x26, msime::windows::ReplyPath::PreviousCandidate,
                 FanyImeReplyType::MoveSelectionPrevious);
        require(session.view().at("candidates").at(0).at("highlighted") == true,
                "Previous navigation did not restore shared highlight");
      }
      bool found = false;
      size_t slot = 0;
      for (size_t attempts = 0; attempts < 100; ++attempts) {
        auto current = session.view();
        for (size_t i = 0; i < current.at("candidates").size(); ++i) {
          if (current.at("candidates").at(i).at("text") == "你") {
            slot = i;
            found = true;
            break;
          }
        }
        if (found || current.at("page").get<size_t>() + 1 >=
                         current.at("page_count").get<size_t>())
          break;
        send(0x22, 0, msime::windows::ReplyPath::NextPage);
      }
      require(found, "No real partial candidate in fixed dictionary");
      auto partial = send(static_cast<uint32_t>(0x61 + slot), 0,
                          msime::windows::ReplyPath::Selection);
      require(partial.source.transition.at("commit") == "你" &&
                  !partial.source.transition.at("view")
                       .at("editing_text")
                       .get<std::string>()
                       .empty(),
              "Fixture did not exercise a partial commit");
      require(partial.encoded->packet.msg_type ==
                      FanyImeReplyType::NeedToCreateWord &&
                  composer.selected_prefix() == "你",
              "Real partial prefix not retained");
      {
        const auto preserved = session.view();
        FanyImeNamedpipeData enter{};
        enter.client_id = 42;
        enter.event_type = FanyImePipeEventType::KeyEvent;
        enter.request_id = request++;
        enter.keycode = 0x0D;
        rejected([&] {
          composer.dispatch(session, enter, epoch,
                            msime::windows::ReplyPath::LocalCommit, false,
                            preserved.at("editing_text").get<std::string>());
        });
        require(session.view() == preserved && !composer.has_pending() &&
                    composer.selected_prefix() == "你",
                "Rejected raw-only Enter lost the selected prefix or remainder");
      }
      auto final = send(0x20, 0, msime::windows::ReplyPath::Selection);
      require(final.encoded->packet.msg_type == FanyImeReplyType::Normal &&
                  final.encoded->packet.candidate_string[0] == 0x4F60 &&
                  final.encoded->packet.candidate_string[1] == 0x597D &&
                  final.encoded->packet.candidate_string[2] == 0,
              "Legacy final reply lost or duplicated partial prefix");
      require(composer.selected_prefix().empty(),
              "Final reply retained stale prefix");
    }
    local_mode_trigger_tests(options.dump());
    if (argc == 2)
      local_mode_tests(options.dump());
    std::cout << "Windows Server boundary: shared session, routing and input "
                 "acceptance passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
