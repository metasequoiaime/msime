#pragma once

#include <windows.h>

namespace Global
{
// 读取不到键盘布局字符时，从标准字母/数字键和数字小键盘补回能确认的 ASCII 前文。
inline WCHAR ForwardedSmartPunctuationContext(UINT code, WCHAR wch)
{
    if ((wch >= L'0' && wch <= L'9') || (wch >= L'A' && wch <= L'Z') || (wch >= L'a' && wch <= L'z'))
    {
        return wch;
    }
    if (wch != L'\0')
    {
        return L'\0';
    }
    if (code >= VK_0 && code <= VK_9)
    {
        return static_cast<WCHAR>(L'0' + (code - VK_0));
    }
    if (code >= L'A' && code <= L'Z')
    {
        return static_cast<WCHAR>(code);
    }
    if (code >= VK_NUMPAD0 && code <= VK_NUMPAD9)
    {
        return static_cast<WCHAR>(L'0' + (code - VK_NUMPAD0));
    }
    return L'\0';
}
} // namespace Global
