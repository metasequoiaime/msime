#include "../../Global/AltGrKeyPolicy.h"
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

    // AltGr arrives as Ctrl+Alt. A character it produces is typing, not a shortcut.
    check(CharacterModifiers(0b110u, L'@', 0x30u) == 0u, "AZERTY AltGr+0 types @");
    check(CharacterModifiers(0b110u, L'{', 0x51u) == 0u, "AltGr+Q types { on a layout that puts it there");
    check(CharacterModifiers(0b110u, L'\u20AC', 0x45u) == 0u, "AltGr+E types the euro sign");
    check(CharacterModifiers(0b111u, L'|', 0x41u) == 1u, "Shift stays with an AltGr character");

    // On the digit keys that select a candidate the chord stays Ctrl+Alt, so it is never taken as a bare digit that commits one.
    check(CharacterModifiers(0b110u, L'#', 0x33u) == 0b110u, "AZERTY AltGr+3 is not a bare 3");
    check(CharacterModifiers(0b110u, L'[', 0x35u) == 0b110u, "AZERTY AltGr+5 is not a bare 5");
    check(CharacterModifiers(0b110u, L'{', 0x37u) == 0b110u, "German AltGr+7 is not a bare 7");
    check(CharacterModifiers(0b110u, L']', 0x39u) == 0b110u, "German AltGr+9 is not a bare 9");
    check(CharacterModifiers(0b110u, L'#', 0x63u) == 0b110u, "a numpad digit is not a bare digit either");
    check(CharacterModifiers(0b110u, L'}', 0x30u) == 0u, "AltGr+0 types, since 0 never selects");

    // Without a printable character the chord is still a shortcut.
    check(CharacterModifiers(0b110u, L'\0', 0x41u) == 0b110u, "Ctrl+Alt+letter with no character is a shortcut");
    check(CharacterModifiers(0b110u, L' ', 0x20u) == 0b110u, "Ctrl+Alt+Space is the input hotkey");
    check(CharacterModifiers(0b110u, wchar_t(0x01), 0x41u) == 0b110u, "a control character is a shortcut");
    check(CharacterModifiers(0b110u, wchar_t(0x7F), 0x41u) == 0b110u, "DEL is a shortcut");

    // Ctrl or Alt alone is never AltGr, whatever the layout produced.
    check(CharacterModifiers(0b010u, L'a', 0x41u) == 0b010u, "Ctrl alone stays");
    check(CharacterModifiers(0b100u, L'a', 0x41u) == 0b100u, "Alt alone stays");
    check(CharacterModifiers(0b011u, L'A', 0x41u) == 0b011u, "Ctrl+Shift stays");
    check(CharacterModifiers(0u, L'a', 0x41u) == 0u, "a plain key has no modifiers");
    check(CharacterModifiers(1u, L'A', 0x41u) == 1u, "Shift alone stays");

    // 已经交给应用的键：AltGr 打出的字符都是打字，选候选的数字键也一样，因为它们不会再选候选。
    check(IsAltGrCharacter(0b110u, L'@'), "German AltGr+Q types @");
    check(IsAltGrCharacter(0b111u, L'|'), "Shift does not change an AltGr character");
    check(IsAltGrCharacter(0b110u, L'#'), "AZERTY AltGr+3 is typing once the key went to the application");
    check(!IsAltGrCharacter(0b110u, L'\0'), "Ctrl+Alt with no character is a shortcut");
    check(!IsAltGrCharacter(0b110u, L' '), "Ctrl+Alt+Space is the input hotkey");
    check(!IsAltGrCharacter(0b110u, wchar_t(0x7F)), "DEL is not a character");
    check(!IsAltGrCharacter(0b010u, L'a'), "Ctrl alone is not AltGr");
    check(!IsAltGrCharacter(0b100u, L'a'), "Alt alone is not AltGr");

    if (failures != 0)
        return 1;
    std::puts("altgr key policy: ok");
    return 0;
}
