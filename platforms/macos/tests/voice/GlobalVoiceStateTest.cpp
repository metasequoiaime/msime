#include "../../src/voice/GlobalVoiceState.h"
#include <cassert>

int main() {
    using State = msime::mac::GlobalVoiceState;
    State state;
    assert(!state.begin("", 10, 0));
    assert(!state.begin("ABC", 0, 0));
    assert(state.begin("ABC", 10, 0));
    assert(!state.begin("Japanese", 20, 1));
    assert(!state.activate(20, "MSIME"));
    assert(!state.canRestore("MSIME"));
    assert(!state.timedOut(2.99));
    assert(state.timedOut(3));
    state.selected = "MSIME";
    assert(state.canRestore("MSIME"));
    state.clear();
    assert(!state.canRestore("MSIME"));
    assert(state.begin("Japanese.Katakana", 10, 5));
    assert(state.activate(10, "MSIME.Hans"));
    state.hostSelectedSource("MSIME.Roman");
    assert(state.canRestore("MSIME.Roman"));
    state.hostSelectedSource("MSIME.Hans");
    state.phase = State::Phase::Finishing;
    assert(state.canRestore("MSIME.Hans"));
    assert(state.original == "Japanese.Katakana");
    state.sourceChanged("ABC");
    state.hostSelectedSource("MSIME.Hans");
    state.sourceChanged("MSIME.Hans");
    assert(!state.canRestore("MSIME.Hans"));
    state.clear();
    assert(state.begin("MSIME.Hans", 10, 10));
    assert(state.activate(10, "MSIME.Hans"));
    assert(!state.canRestore("MSIME.Hans"));
    const auto generation = state.generation;
    state.clear();
    assert(state.begin("ABC", 10, 11));
    assert(state.generation > generation);
}
