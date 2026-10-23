#include "../../src/input/ToggleHotkeys.h"
#include "../../src/input/InputControllerPhysicalKeys.h"
#include <cassert>

int main() {
    const auto period = msime::mac::ToggleChordForKeyName("Period", true, false, false, false);
    const auto v = msime::mac::ToggleChordForKeyName("KeyV", true, false, false, false);
    // Dvorak 的物理句号键输出 v，只能命中字母槽位。
    assert(!msime::mac::ToggleChordMatches(period, 47, 'v', true, false, false, false));
    assert(msime::mac::ToggleChordMatches(v, 47, 'v', true, false, false, false));
    // Dvorak 的物理 E 键输出句号，只能命中标点槽位。
    assert(msime::mac::ToggleChordMatches(period, 14, '.', true, false, false, false));
    assert(!msime::mac::ToggleChordMatches(v, 14, '.', true, false, false, false));
    // 同一个规范绑定在美式布局仍命中原来的位置。
    assert(msime::mac::ToggleChordMatches(period, 47, '.', true, false, false, false));
    assert(msime::mac::ToggleChordMatches(v, 9, 'v', true, false, false, false));
    // 字符读取失败的合成事件仍保留物理回退，不能误收小键盘句号。
    assert(msime::mac::ToggleChordMatches(period, 47, '\0', true, false, false, false));
    assert(!msime::mac::ToggleChordMatches(period, 65, '\0', true, false, false, false));
    // 数字同样跟随无修饰布局字符，功能键继续按物理身份认。
    const auto digit = msime::mac::ToggleChordForKeyName("Digit4", true, false, true, false);
    assert(msime::mac::ToggleChordMatches(digit, 14, '4', true, false, true, false));
    assert(!msime::mac::ToggleChordMatches(digit, 21, 'e', true, false, true, false));
    const auto function = msime::mac::ToggleChordForKeyName("F9", true, false, false, false);
    assert(msime::mac::ToggleChordMatches(function, 101, 0xf708, true, false, false, false));
    // 无字符与非拉丁回退和既有固定字母快捷键使用相同的 ANSI 位置。
    for (const auto &entry : msime::mac::detail::kToggleChordKeyCodes) {
        if (entry.character >= 'a' && entry.character <= 'z')
            assert(msime::mac::PhysicalAnsiLetter(entry.keyCode) == entry.character);
    }
    const auto y = msime::mac::ToggleChordForKeyName("KeyY", true, false, false, false);
    assert(msime::mac::ToggleChordMatches(y, 16, 0x0430, true, false, false, false));
}
