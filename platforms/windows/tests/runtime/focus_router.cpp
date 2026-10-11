#include "FocusRouter.h"
#include <future>

using namespace msime::windows;
using namespace FanyImePipeEventType;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Focus routing test failed");
}
FanyImeNamedpipeData packet(uint64_t client, uint32_t event,
                            uint64_t request = 1) {
  FanyImeNamedpipeData value{};
  value.client_id = client;
  value.event_type = event;
  value.request_id = request;
  return value;
}
int main() {
  FocusGate gate;
  FocusRouter router(gate, 2);
  PipeTicket a{42, {1, 2, 3}}, b{43, {4, 5, 6}};
  require(router.connected(a).accepted && router.connected(b).accepted);
  require(!router.connected({44, {7, 8, 9}}).accepted);
  require(!router.connected({44, {0, 8, 9}}).accepted);
  require(router.dispatch(a, packet(42, ClientHello)).accepted);
  // 字段属性不论有没有焦点都收下，但不认领焦点、不带路由，之后的按键照旧要先激活。
  {
    const auto traits = router.dispatch(a, packet(42, FieldTraits));
    require(traits.accepted && !traits.route && !traits.activation &&
            !traits.fence && !traits.cleanup);
    require(!router.dispatch(a, packet(43, FieldTraits)).accepted);
  }
  require(!router.dispatch(a, packet(42, KeyEvent)).accepted);
  require(!router.dispatch(a, packet(42, FocusRestored)).accepted);
  require(!router.dispatch(a, packet(42, StatusSnapshot)).accepted);
  require(!router.dispatch(a, packet(42, ShowCandidateWnd)).accepted);
  require(!router.dispatch(a, packet(42, ClientSuspended)).accepted);
  require(!router.dispatch(a, packet(43, ClientActivated)).accepted);
  auto first = router.dispatch(a, packet(42, ClientActivated, 77));
  require(first.accepted && first.activation && first.fence && first.route &&
          !first.cleanup && first.route->token == 77);
  require(!router.confirmed(*first.route));
  auto pending_key = router.dispatch(a, packet(42, KeyEvent));
  require(pending_key.accepted && !pending_key.activation &&
          pending_key.route->epoch == first.route->epoch);
  require(gate.acknowledge(*first.route, [] { return true; }));
  // Deliberately omit confirmed(): the displaced lease's successful fence must
  // be remembered even if its receipt is still queued behind the next client.
  auto other = router.dispatch(b, packet(43, ClientActivated, 88));
  require(other.activation && other.activation->previous_ready &&
          other.cleanup->epoch == first.route->epoch);
  require(!router.confirmed(*first.route));
  require(gate.acknowledge(*other.route, [] { return true; }));
  require(router.confirmed(*other.route));
  require(!router.dispatch(a, packet(42, StatusSnapshot)).accepted);
  auto restored = router.dispatch(a, packet(42, FocusRestored));
  require(restored.activation && restored.route->token == 77 &&
          restored.route->epoch > other.route->epoch && restored.fence);
  require(gate.acknowledge(*restored.route, [] { return true; }));
  require(router.confirmed(*restored.route));
  auto repeated = router.dispatch(a, packet(42, ClientActivated, 77));
  require(repeated.accepted && !repeated.activation && repeated.fence &&
          repeated.route->epoch == restored.route->epoch);
  auto renewed = router.dispatch(a, packet(42, ClientActivated, UINT64_MAX));
  require(renewed.activation &&
          renewed.cleanup->epoch == restored.route->epoch &&
          renewed.route->token == UINT64_MAX);
  require(!router.failed(*restored.route).accepted);
  require(gate.acknowledge(*renewed.route, [] { return true; }));
  require(router.confirmed(*renewed.route));
  auto suspended = router.dispatch(a, packet(42, ClientSuspended));
  require(suspended.accepted && suspended.cleanup && !suspended.terminal);
  require(!gate.with_active(*renewed.route, [] {}));
  require(!router.dispatch(a, packet(42, KeyEvent)).accepted);
  require(!router.dispatch(a, packet(42, FocusRestored)).accepted);
  auto terminal = router.dispatch(a, packet(42, ClientDeactivated));
  require(terminal.accepted && terminal.terminal &&
          terminal.cleanup->epoch == renewed.route->epoch);
  require(!router.dispatch(a, packet(42, ClientDeactivated)).accepted);
  auto resumed = router.dispatch(a, packet(42, ClientActivated, 99));
  require(resumed.activation.has_value());
  require(!gate.acknowledge(*resumed.route, [] { return false; }));
  require(!router.dispatch(a, packet(42, KeyEvent)).accepted);
  auto retry = router.dispatch(a, packet(42, ClientActivated, 100));
  require(retry.activation && retry.cleanup->epoch == resumed.route->epoch);
  require(router.failed(*retry.route).accepted);
  require(!router.dispatch(a, packet(42, FocusRestored)).accepted);
  auto activated = router.dispatch(a, packet(42, ClientActivated, 101));
  require(gate.acknowledge(*activated.route, [] { return true; }));
  require(router.confirmed(*activated.route));
  PipeTicket newer{42, {7, 2, 3}};
  auto replaced = router.connected(newer);
  require(replaced.accepted &&
          replaced.cleanup->epoch == activated.route->epoch);
  require(!router.connected(a).accepted);
  require(!router.disconnected(a).accepted);
  require(!router.dispatch(a, packet(42, ClientDeactivated)).accepted);
  require(!router.dispatch(newer, packet(42, KeyEvent)).accepted);
  auto current = router.dispatch(newer, packet(42, ClientActivated, 102));
  require(current.activation.has_value());
  require(!router.failed(*activated.route).accepted);
  auto background = router.disconnected(b);
  require(background.accepted && background.cleanup);
  require(gate.with_pending(*current.route, [] {}));
  auto wrong_thread = std::async(std::launch::async, [&] {
    try {
      router.disconnected(newer);
    } catch (const std::logic_error &) {
      return true;
    }
    return false;
  });
  require(wrong_thread.get());
  require(router.disconnected(newer).accepted);
  require(!gate.with_pending(*current.route, [] {}));
  require(router.connected({44, {8, 9, 10}}).accepted);
  {
    FocusGate pending_gate;
    FocusRouter pending_router(pending_gate, 2);
    require(pending_router.connected(a).accepted &&
            pending_router.connected(b).accepted);
    auto unconfirmed =
        pending_router.dispatch(a, packet(42, ClientActivated, 3));
    auto takeover = pending_router.dispatch(b, packet(43, ClientActivated, 4));
    require(unconfirmed.route && takeover.route &&
            !takeover.activation->previous_ready);
    require(!pending_router.dispatch(a, packet(42, KeyEvent)).accepted);
    require(!pending_gate.acknowledge(*unconfirmed.route, [] { return true; }));
    require(pending_gate.acknowledge(*takeover.route, [] { return true; }));
    require(pending_router.confirmed(*takeover.route));
    auto explicit_return =
        pending_router.dispatch(a, packet(42, ClientActivated, 5));
    require(explicit_return.activation.has_value());
    auto key_return = pending_router.dispatch(b, packet(43, KeyEvent, 99));
    require(key_return.activation && key_return.route->token == 4 &&
            key_return.route->token != 99);
  }
}
