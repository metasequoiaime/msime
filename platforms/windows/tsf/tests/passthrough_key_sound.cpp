#include "../../common/KeySoundClass.h"

#include <cstdio>

namespace
{
int failures = 0;

void Require(bool value, const char *what)
{
    if (!value)
    {
        std::fprintf(stderr, "passthrough key sound: %s\n", what);
        ++failures;
    }
}

msime::windows::PassthroughKeyState Typed(unsigned virtualKey)
{
    msime::windows::PassthroughKeyState key;
    key.virtual_key = virtualKey;
    key.keyboard_open = true;
    return key;
}
} // namespace

int main()
{
    using msime::windows::passthrough_key_sound_class;
    // 和 Server 处理的键同一张表：空格 1、回车 2、退格 3，数字、方向键、Esc 这些都是 0。
    Require(passthrough_key_sound_class(Typed(0x20)) == 1u, "space is class 1");
    Require(passthrough_key_sound_class(Typed(0x0D)) == 2u, "enter is class 2");
    Require(passthrough_key_sound_class(Typed(0x08)) == 3u, "backspace is class 3");
    Require(passthrough_key_sound_class(Typed('1')) == 0u, "a digit is any other key");
    Require(passthrough_key_sound_class(Typed(0x25)) == 0u, "an arrow is any other key");
    Require(passthrough_key_sound_class(Typed(0x1B)) == 0u, "escape is any other key");
    for (unsigned modifier : {0x10u, 0x11u, 0x12u, 0x14u, 0x5Bu, 0x5Cu, 0xA0u, 0xA5u, 0u})
    {
        Require(!passthrough_key_sound_class(Typed(modifier)), "a bare modifier makes no sound");
    }

    // 按住不放只算一次按下，和 macOS 一样。
    auto held = Typed(0x08);
    held.auto_repeat = true;
    Require(!passthrough_key_sound_class(held), "auto-repeat is silent");
    // Ctrl、Alt、Windows 组合键是快捷键；Shift 照样是打字。
    auto chord = Typed('C');
    chord.ctrl = true;
    Require(!passthrough_key_sound_class(chord), "Ctrl chords are shortcuts");
    chord = Typed('F');
    chord.alt = true;
    Require(!passthrough_key_sound_class(chord), "Alt chords are shortcuts");
    chord = Typed('D');
    chord.win = true;
    Require(!passthrough_key_sound_class(chord), "Windows chords are shortcuts");
    // 英文模式、停用的键盘（密码框）、安全模式和面板注入的文字都不出声。
    auto english = Typed('a');
    english.keyboard_open = false;
    Require(!passthrough_key_sound_class(english), "English mode is silent");
    auto disabled = Typed('1');
    disabled.keyboard_disabled = true;
    Require(!passthrough_key_sound_class(disabled), "a disabled keyboard is silent");
    auto secure = Typed('1');
    secure.secure = true;
    Require(!passthrough_key_sound_class(secure), "secure mode is silent");
    auto injected = Typed('1');
    injected.injected = true;
    Require(!passthrough_key_sound_class(injected), "text a panel injected is silent");

    if (failures == 0)
    {
        std::puts("passthrough key sound checks passed");
    }
    return failures == 0 ? 0 : 1;
}
