#pragma once

namespace Global
{
// 转发给应用的按键在智能标点前文影子里记成什么。`character` 是键盘布局在当时的修饰键状态下给这颗键的字符，没有就是 0；`modifiers` 是 IPC 修饰位（Shift 1、Ctrl 2、Alt 4）；`windowsKeyDown` 是 Win 键。返回 0 表示这颗键之后光标前是什么无从确认，影子要失效，下一个标点去读文档。
//
// 布局给了字符就以它为准：ASCII 字母数字记下（AltGr 打出的也算，那是用户打的字），其他字符一律失效，包括 Shift+数字的符号、Ctrl+字母的控制字符，以及 AZERTY 这类布局上数字行不按 Shift 打出的符号。
//
// 布局没给字符时才看虚拟键码，而且要先看修饰键：带 Ctrl、Alt 或 Win 的是快捷键，不产生字符，还可能粘贴、移动光标或切换标签页；Shift+数字行打出的是符号。只有不带这些修饰键的字母键、数字键和数字小键盘才补回字符。不依赖 `<windows.h>`，测试在任何主机上都能跑。
inline wchar_t ForwardedSmartPunctuationContext(unsigned code, wchar_t character, unsigned modifiers,
                                                bool windowsKeyDown)
{
    if ((character >= L'0' && character <= L'9') || (character >= L'A' && character <= L'Z') ||
        (character >= L'a' && character <= L'z'))
    {
        return character;
    }
    if (character != L'\0')
    {
        return L'\0';
    }
    if ((modifiers & 0b110u) != 0 || windowsKeyDown)
    {
        return L'\0';
    }
    // VK '0'..'9'
    if (code >= 0x30u && code <= 0x39u)
    {
        return (modifiers & 0b001u) != 0 ? L'\0' : static_cast<wchar_t>(L'0' + (code - 0x30u));
    }
    // VK 'A'..'Z'
    if (code >= 0x41u && code <= 0x5Au)
    {
        return static_cast<wchar_t>(code);
    }
    // VK_NUMPAD0..VK_NUMPAD9
    if (code >= 0x60u && code <= 0x69u)
    {
        return static_cast<wchar_t>(L'0' + (code - 0x60u));
    }
    return L'\0';
}
} // namespace Global
