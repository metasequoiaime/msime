#pragma once

namespace Global {
// The modifiers a key is classified and sent with, from the IPC bits (Shift 1, Ctrl 2, Alt 4) and the character the keyboard layout produced with the full key state. AltGr is reported as Ctrl+Alt (Windows synthesizes a left Ctrl for the right Alt), so a Ctrl+Alt chord that still produces a printable character is AltGr typing that character, as AltGr+0 types '@' on AZERTY, not an application shortcut: Ctrl and Alt are dropped so the key is input and the Server takes it as typing. Shift stays. A Ctrl+Alt chord with no character, a control character (Ctrl+letter) or a space (the Ctrl+Alt+Space input hotkey) keeps both and stays a shortcut.
//
// The digit keys 1-9, on the main row (VK '1'-'9') and the numpad (VK_NUMPAD1-VK_NUMPAD9), keep both too: the TSF and the Server pick a candidate by the virtual key of an unmodified digit, so AltGr+3 typing '#' on AZERTY or AltGr+7 typing '{' on a German layout would commit a candidate instead and lose the character. They stay Ctrl+Alt chords that reach the application, as they did before AltGr typing was recognised. AltGr+0 ('@' on AZERTY) is not one of them, because 0 never selects.
// Ctrl+Alt 按出了一个可打印字符：这是 AltGr 打字，不是快捷键。数字键也算，下面 CharacterModifiers 为选候选留下的例外由它自己判断；已经交给应用的键（按键音、打字特效）不会选候选，直接用这一条。
inline bool IsAltGrCharacter(unsigned modifiers, wchar_t character)
{
    return (modifiers & 0b110u) == 0b110u && character > 0x20 && character != 0x7F;
}

inline unsigned CharacterModifiers(unsigned modifiers, wchar_t character, unsigned virtualKey)
{
    const bool selectionDigit =
        (virtualKey >= 0x31u && virtualKey <= 0x39u) || (virtualKey >= 0x61u && virtualKey <= 0x69u);
    return IsAltGrCharacter(modifiers, character) && !selectionDigit ? (modifiers & ~0b110u) : modifiers;
}
} // namespace Global
