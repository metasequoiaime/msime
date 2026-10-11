#include "../../common/KeySoundClass.h"
#include "../../../../shared/contracts/windows_ipc.h"

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

    // Server 把这些键带 MSIME_KEY_PASSTHROUGH 交给生命周期核心：类别只占低 7 位里的 0-3，标志位留给 Server。
    for (unsigned virtualKey = 1; virtualKey < 0x100; ++virtualKey)
    {
        if (const auto keyClass = passthrough_key_sound_class(Typed(virtualKey)))
        {
            Require(*keyClass <= 3u, "a passthrough class is one msime_client_key takes");
        }
    }
    // 交给应用的键在哪里不出声，TIP 报给 Server 的字段属性就在哪里是 SECURE：停用的键盘（密码框）和安全桌面。两条路用同一个判据，核心才不会在 Server 处理的键上出声、而交给应用的键却静音，反之亦然。
    for (bool keyboardDisabled : {false, true})
    {
        for (bool secureMode : {false, true})
        {
            auto key = Typed('1');
            key.keyboard_disabled = keyboardDisabled;
            key.secure = secureMode;
            const bool fieldSecure =
                (FanyImeFieldTraits::Observe(keyboardDisabled, secureMode, false) & FanyImeFieldTraits::Secure) != 0;
            Require(passthrough_key_sound_class(key).has_value() == !fieldSecure,
                    "passthrough keys are silent exactly where the field is reported SECURE");
        }
    }

    if (failures == 0)
    {
        std::puts("passthrough key sound checks passed");
    }
    return failures == 0 ? 0 : 1;
}
