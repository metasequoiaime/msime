#pragma once

#include "SmartPunctuationForwardedKey.h"
#include "../../compat/WindowsVersion.h"

namespace Global
{
// 复用真实内核版本查询；不支持非破坏性转换时，让调用者回读文档，不碰死键状态。
inline WCHAR LayoutCharacterOfForwardedKey(UINT code)
{
    const auto &version = msime::windows::compat::kernel_version();
    if (!version)
    {
        return 0;
    }
    return QueryForwardedLayoutCharacter(version->dwMajorVersion, version->dwBuildNumber, code, [](unsigned key) {
        BYTE keyboardState[256] = {};
        if (!GetKeyboardState(keyboardState))
        {
            return WCHAR{0};
        }
        WCHAR character = 0;
        const int produced = ToUnicode(key, MapVirtualKey(key, MAPVK_VK_TO_VSC), keyboardState, &character, 1, 0x4);
        return produced == 1 ? character : WCHAR{0};
    });
}
} // namespace Global
