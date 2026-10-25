#include "preference_transition.h"
#include "ipc_negotiation.h"
#include "../../tsf/PreferenceBarrier.h"
#include <algorithm>
#include <iostream>
#include <stdexcept>

namespace {
void require(bool value) { if (!value) throw std::runtime_error("Preference transition assertion failed"); }
}
int main() {
  using namespace FanyImePreferencePipe;
  try {
    for (const auto value : {Transition{0, 0, 0, 0}, Transition{42, 12, 77, 3},
                            Transition{(std::numeric_limits<std::uint64_t>::max)(), 0, 0, 0}}) {
      const auto frame = encode(value); require(frame.has_value());
      const auto decoded = parse(*frame); require(decoded.has_value());
      require(decoded->revision == value.revision && decoded->request == value.request &&
              decoded->focus == value.focus && decoded->epoch == value.epoch);
    }
    require(!encode({1, 12, 0, 3}) && !encode({1, 0, 77, 3}));
    auto frame = *encode({42, 12, 77, 3});
    frame.data[0] = '-'; require(!parse(frame));
    frame = *encode({42, 12, 77, 3}); frame.data[1] = 0; require(!parse(frame));
    frame = *encode({42, 12, 77, 3}); frame.data[100] = '1'; require(!parse(frame));
    frame = *encode({42, 12, 77, 3}); std::fill(std::begin(frame.data), std::end(frame.data), '9');
    require(!parse(frame));
    const auto old = FanyImeProtocol::Negotiate(FanyImeProtocol::Hello(1, 2, FanyImeProtocol::Capabilities | FanyImeProtocol::OrderedPreferences));
    require(old.accepted && !(old.capabilities & FanyImeProtocol::OrderedPreferences));
    const auto current = FanyImeProtocol::Negotiate(FanyImeProtocol::Hello(1, 2,
        FanyImeProtocol::Capabilities | FanyImeProtocol::OrderedPreferences),
        FanyImeProtocol::Capabilities | FanyImeProtocol::OrderedPreferences);
    require(current.accepted && (current.capabilities & FanyImeProtocol::OrderedPreferences));
    msime::tsf::PreferenceBarrier barrier;
    barrier.begin(77); barrier.sent(12);
    require(barrier.active() && barrier.receive({44, 0, 0, 0}, 77));
    require(!barrier.complete(44));
    require(!barrier.receive({43, 13, 77, 3}, 77));
    require(!barrier.receive({43, 12, 77, 3}, 78));
    require(barrier.receive({43, 12, 77, 3}, 77));
    require(!barrier.complete(43) && barrier.active());
    require(barrier.complete(44) && !barrier.active());
    require(barrier.receive({41, 0, 0, 0}, 77) && !barrier.active());
    require(barrier.receive({45, 0, 0, 0}, 77) && barrier.active());
    require(!barrier.complete(44) && barrier.complete(45));
    barrier.begin(77); barrier.sent(14); barrier.reset();
    require(!barrier.receive({46, 14, 77, 3}, 77) && !barrier.active());
    barrier.begin(78); barrier.sent(15);
    require(barrier.receive({0, 15, 78, 4}, 78));
    require(barrier.complete(45) && !barrier.active());
    std::cout << "Preference transition tests passed\n";
    return 0;
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
