#include "../../src/input/InputQueue.h"
#include "../core/TestHostOptions.h"
#include <cassert>
#include <chrono>
using namespace msime::windows;
int main() {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-dedicated-english-" + std::to_string(
          std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() { std::error_code ec; std::filesystem::remove_all(root, ec); }
  } cleanup{root};
  auto options = test_host_options(root);
  // 默认输入状态 picks the state a new focus session starts in, and the host
  // applies it as its own English passthrough. It must not start the Engine in
  // dedicated English, whose candidates are English words and which the CN/EN
  // switch cannot leave.
  options["preferences"]["default_ime_mode"] = "english";
  const auto serialized = options.dump();
  {
    FocusGate gate;
    InputState state(gate, 2, serialized);
    const PipeTicket ticket{42, {1, 2, 3}};
    assert(state.connected(ticket).accepted);
    // Maintenance reaches an idle session without a candidate snapshot.
    assert(state.reset_cache());
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.event_type = FanyImePipeEventType::ClientActivated;
    packet.request_id = 77;
    const auto route = state.dispatch(ticket, packet);
    assert(route.route);
    const auto lease = *route.route;
    assert(!state.dedicated_english(lease, true));
    assert(gate.acknowledge(lease, [] { return true; }));
    assert(state.confirmed(lease));
    const auto initial = state.dedicated_english(lease, false);
    assert(initial && initial->at("dedicated_english") == false);
    auto stale = lease;
    ++stale.token;
    assert(!state.dedicated_english(stale, true));
    ++stale.transport.generations[0];
    assert(!state.dedicated_english(stale, false));
    packet.event_type = FanyImePipeEventType::KeyEvent;
    packet.request_id = 2;
    packet.keycode = 'A';
    packet.wch = 'a';
    auto pending = state.key(lease, packet, ReplyPath::Composition);
    assert(pending);
    assert(!state.dedicated_english(lease, true));
    assert(state.delivered(lease, packet.request_id));
    // The letter opens a pinyin composition instead of being answered with
    // English words.
    const auto typed = state.dedicated_english(lease, false);
    assert(typed && typed->at("dedicated_english") == false);
    assert(typed->at("editing_text") == "a");
    // Exit is a no-op while the mode is off: it neither cancels the
    // composition nor moves the Engine on.
    const auto idle = state.dedicated_english(lease, true);
    assert(idle && idle->at("dedicated_english") == false);
    assert(idle->at("editing_text") == "a");
    assert(idle->at("generation") == typed->at("generation"));
    // 托盘「英文候选模式」显式设置模式：组字中和过期的租约什么也不动，重复设置同一个状态不推进 Engine。
    assert(!state.set_dedicated_english(stale, true));
    assert(!state.set_dedicated_english(lease, true));
    const auto still = state.dedicated_english(lease, false);
    assert(still && still->at("dedicated_english") == false &&
           still->at("editing_text") == "a");
    assert(state.cancel_composition(lease));
    const auto entered = state.set_dedicated_english(lease, true);
    assert(entered && entered->at("dedicated_english") == true);
    assert(entered->at("editing_text") == "");
    const auto again = state.set_dedicated_english(lease, true);
    assert(again && again->at("dedicated_english") == true);
    assert(again->at("generation") == entered->at("generation"));
    const auto read = state.dedicated_english(lease, false);
    assert(read && read->at("dedicated_english") == true);
    const auto left = state.set_dedicated_english(lease, false);
    assert(left && left->at("dedicated_english") == false);
    assert(state.quiesce_dictionaries() == 1);
    assert(!state.dedicated_english(lease, true));
  }
  // A fresh session starts in Chinese as well; the saved default decides the
  // host's passthrough and never the Engine's English candidates.
  ServerSession fresh(43, serialized);
  fresh.activate(1);
  assert(fresh.dedicated_english(1, false).at("dedicated_english") == false);
  assert(options["preferences"]["default_ime_mode"] == "english");
  // Ctrl+Shift+E flips the dedicated English mode and discards the open composition, as the reference Server does. The TSF has already cancelled locally, so the key stages a frame-less LocalCancel.
  {
    ReplyComposer composer(43, 1);
    FanyImeNamedpipeData key{};
    key.client_id = 43;
    key.event_type = FanyImePipeEventType::KeyEvent;
    key.request_id = 5;
    key.keycode = 'A';
    key.wch = 'a';
    assert(composer.basic_key(fresh, key, 1, TsfPreeditStyle::Pinyin));
    composer.confirm_delivery(43, 1, 5);
    assert(fresh.view().at("editing_text") == "a");
    FanyImeNamedpipeData toggle{};
    toggle.client_id = 43;
    toggle.event_type = FanyImePipeEventType::KeyEvent;
    toggle.request_id = 6;
    toggle.keycode = 'E';
    toggle.wch = 0x05;
    toggle.modifiers_down = 3;
    const auto entered = composer.basic_key(fresh, toggle, 1, TsfPreeditStyle::Pinyin);
    assert(entered && !entered->encoded);
    composer.confirm_delivery(43, 1, 6);
    assert(fresh.view().at("dedicated_english") == true);
    assert(fresh.view().at("editing_text") == "");
    toggle.request_id = 7;
    assert(composer.basic_key(fresh, toggle, 1, TsfPreeditStyle::Pinyin));
    composer.confirm_delivery(43, 1, 7);
    assert(fresh.view().at("dedicated_english") == false);
    // With Alt the chord is not the toggle and stays with the application.
    toggle.request_id = 8;
    toggle.modifiers_down = 7;
    assert(!composer.basic_key(fresh, toggle, 1, TsfPreeditStyle::Pinyin));
    assert(fresh.view().at("dedicated_english") == false);
  }
}
