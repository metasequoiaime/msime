#pragma once

#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 应用例外（共享偏好 `app_input_mode_rules`）在 Windows 上的读法：键是进程的可执行文件基名，值是切到该进程时的起始模式。TIP 激活时用它定起始模式，Server 每次焦点进入一个新应用时用它推模式。共享文档里同一张表也装着 macOS 的 bundle id，它们不会和任何 `.exe` 基名相等，在这里自然不命中。只依赖标准库和 nlohmann，TIP、Server 和主机上的单测都能直接包含。
namespace msime::windows
{
struct AppInputModeRule
{
    // UTF-8 的应用标识，按文档原样保存。
    std::string app;
    bool chinese = true;
};
using AppInputModeRules = std::vector<AppInputModeRule>;

// 只折叠 ASCII 字母，和偏好库 `to_ascii_lowercase` 的去重、TIP 的 `CompareStringOrdinal(..., TRUE)` 在 ASCII 程序名上的结果一致；非 ASCII 字符按原样比较。
inline bool SameAppInputModeRuleId(std::string_view left, std::string_view right)
{
    if (left.size() != right.size())
        return false;
    for (size_t i = 0; i < left.size(); ++i)
    {
        auto fold = [](char c) { return c >= 'A' && c <= 'Z' ? static_cast<char>(c - 'A' + 'a') : c; };
        if (fold(left[i]) != fold(right[i]))
            return false;
    }
    return true;
}

// 进程基名命中的规则：true 是中文、false 是英文，没有规则时为空。进程名为空（取不到）时一律不命中。
inline std::optional<bool> AppInputModeRuleFor(const AppInputModeRules &rules, std::string_view process)
{
    if (process.empty())
        return std::nullopt;
    for (const auto &rule : rules)
    {
        if (SameAppInputModeRuleId(rule.app, process))
            return rule.chinese;
    }
    return std::nullopt;
}

// 从共享偏好的 `preferences` 对象读出规则表。键不是字符串到 chinese/english 的条目一律跳过，整段不是对象时当作没有规则：偏好库已经校验过文档，这里只防一份手改坏的文件。
inline AppInputModeRules ReadAppInputModeRules(const nlohmann::json &preferences)
{
    AppInputModeRules rules;
    if (!preferences.is_object())
        return rules;
    const auto section = preferences.find("app_input_mode_rules");
    if (section == preferences.end() || !section->is_object())
        return rules;
    for (const auto &[app, mode] : section->items())
    {
        if (app.empty() || !mode.is_string())
            continue;
        const auto &value = mode.get_ref<const std::string &>();
        if (value != "chinese" && value != "english")
            continue;
        rules.push_back(AppInputModeRule{app, value == "chinese"});
    }
    return rules;
}
} // namespace msime::windows
