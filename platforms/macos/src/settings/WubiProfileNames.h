#pragma once
#include <array>
#include <cstddef>
#include <string_view>
namespace msime::mac {
struct WubiProfileName
{
    const char *value;
    // 输入法菜单、悬浮工具栏和候选窗首行用的方案名，与「全拼」「双拼（小鹤）」并列，倒序写成「五笔 86」。
    const char *schemeTitle;
    // 设置窗口「五笔输入」旁的弹出菜单和五笔选项卡里「编码方案」一行用的版本名。
    const char *profileTitle;
};

// 共享偏好 `wubi_profile` 的取值与界面名称，顺序同 Engine 的编码（0 为 86，1 为 98，2 为新世纪），也是设置窗口弹出菜单的顺序。新世纪版写成「新世纪五笔」「五笔 新世纪」，界面上不用「王码」「五笔字型」这些商标。
inline constexpr std::array<WubiProfileName, 3> kWubiProfileNames{{
    {"wubi86", "五笔 86", "86 五笔"},
    {"wubi98", "五笔 98", "98 五笔"},
    {"wubixsj", "五笔 新世纪", "新世纪五笔"},
}};

// 取值在表里的位置，不认识的取值返回表长。
inline std::size_t WubiProfileIndex(std::string_view value)
{
    for (std::size_t index = 0; index < kWubiProfileNames.size(); ++index)
    {
        if (value == kWubiProfileNames[index].value)
        {
            return index;
        }
    }
    return kWubiProfileNames.size();
}

inline bool IsWubiProfile(std::string_view value) { return WubiProfileIndex(value) < kWubiProfileNames.size(); }

// 方案名按取值查表，不认识的版本只写「五笔」，不把它说成 86。
inline const char *WubiSchemeTitle(std::string_view value)
{
    const std::size_t index = WubiProfileIndex(value);
    return index < kWubiProfileNames.size() ? kWubiProfileNames[index].schemeTitle : "五笔";
}

}
