#pragma once

#include "AppInputModeRules.h"

#include <cstdint>
#include <nlohmann/json.hpp>
#include <string>
#include <string_view>
#include <vector>

// Vim 模式（共享偏好 `vim_mode_apps`）在 Windows 上的读法和按键判定：名单里是进程的可执行文件基名，前台进程命中时，交给应用的无修饰 Esc 同时把水杉切到英文，Vim 用户随后可以直接输入 `hjkl`。共享文档里同一张名单也装着 macOS 的 bundle id，它们不会和任何 `.exe` 基名相等，在这里自然不命中。标识比较与应用例外同一口径（AppInputModeRules.h 的 SameAppInputModeRuleId）。只依赖标准库和 nlohmann，TIP 和主机上的单测都能直接包含。
namespace msime::windows
{
using VimModeApps = std::vector<std::string>;

// 从共享偏好的 `preferences` 对象读出名单。不是字符串或为空的项跳过，整段不是数组时当作没有名单：偏好库已经校验过文档，这里只防一份手改坏的文件。
inline VimModeApps ReadVimModeApps(const nlohmann::json &preferences)
{
    VimModeApps apps;
    if (!preferences.is_object())
        return apps;
    const auto section = preferences.find("vim_mode_apps");
    if (section == preferences.end() || !section->is_array())
        return apps;
    for (const auto &entry : *section)
    {
        if (!entry.is_string())
            continue;
        const auto &app = entry.get_ref<const std::string &>();
        if (!app.empty())
            apps.push_back(app);
    }
    return apps;
}

// 进程基名是否在名单里；进程名为空（取不到）时一律不命中。
inline bool VimModeListContains(const VimModeApps &apps, std::string_view process)
{
    if (process.empty())
        return false;
    for (const auto &app : apps)
    {
        if (SameAppInputModeRuleId(app, process))
            return true;
    }
    return false;
}

struct VimModeEscape
{
    std::uint32_t virtualKey = 0;
    // TIP 自己吞下这次按键（取消组字、关掉联想等），Esc 不会到应用。
    bool eaten = false;
    bool autoRepeat = false;
    // Shift、Ctrl、Alt 或 Win 任一按着。
    bool modifierHeld = false;
    // 组字、候选或造词还开着。
    bool compositionActive = false;
    // 还有排队或在飞的异步按键：抢先切换会改变队列里按键的含义。
    bool deferredKeysPending = false;
    bool listed = false;
    // 水杉当前是中文模式；已经是英文时无事可做。
    bool chineseMode = false;
};

// 这次 Esc 是否搭车把水杉切到英文。只在 Esc 已经决定原样交给应用时才切：组字开着时 Esc 照旧只取消组字，取消后再按一次才切；长按的重复事件和带修饰键的 Esc 保持原行为。
inline bool VimModeEscapeSwitchesToEnglish(const VimModeEscape &key)
{
    constexpr std::uint32_t kEscape = 0x1B;
    return key.virtualKey == kEscape && !key.eaten && !key.autoRepeat && !key.modifierHeld &&
           !key.compositionActive && !key.deferredKeysPending && key.listed && key.chineseMode;
}
} // namespace msime::windows
