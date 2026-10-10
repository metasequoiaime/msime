#include "../../Global/FullwidthChordPolicy.h"
#include <cstdio>

namespace {
int failures = 0;

void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // namespace

int main() {
    using namespace Global;
    const unsigned H = 'H';
    check(IsFullwidthAltShiftH(H, true, false, true, false, true, true), "Alt+Shift+H toggles the width in Chinese mode");
    check(!IsFullwidthAltShiftH(H, true, false, true, false, false, true), "English mode hands the chord to the application");
    check(!IsFullwidthAltShiftH(H, true, false, true, false, true, false), "the switch gives the chord back to the application");
    check(!IsFullwidthAltShiftH(H, true, true, true, false, true, true), "AltGr (Ctrl+Alt) is not the chord");
    check(!IsFullwidthAltShiftH(H, true, false, true, true, true, true), "Win+Alt+Shift+H is not the chord");
    check(!IsFullwidthAltShiftH(H, false, false, true, false, true, true), "Alt+H without Shift is not the chord");
    check(!IsFullwidthAltShiftH(H, true, false, false, false, true, true), "Shift+H types a capital letter");
    check(!IsFullwidthAltShiftH('J', true, false, true, false, true, true), "another letter is not the chord");
    if (failures == 0)
        std::puts("Alt+Shift+H: fullwidth toggle in Chinese mode only");
    return failures == 0 ? 0 : 1;
}
