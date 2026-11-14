#pragma once

#include <string_view>

// Windows 原生设置页的快捷短语混输行，与共享偏好 `mixed_input.quick_phrase` 共用同一份文案和回退值。
namespace msime::settings::quick_phrase {

inline constexpr std::wstring_view title = L"快捷短语混输";
inline constexpr std::wstring_view description =
    L"中文输入时，在普通候选中加入编码相同的快捷短语；关闭后仍可用 Shift+K 进入快捷短语模式";
inline constexpr std::wstring_view preference = L"mixed_input.quick_phrase";
inline constexpr bool fallback = false;

} // namespace msime::settings::quick_phrase
