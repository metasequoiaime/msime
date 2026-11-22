#pragma once

#include <string_view>

namespace msime::settings::learning_reset {
// 清除已成功时，集合写回失败不能误报为尚未清除，也不能显示全部完成。
inline constexpr std::wstring_view completed = L"已清除所有学习数据；输入方案和设置保持不变。";
inline constexpr std::wstring_view collections_pending = L"已清除学习数据，但已添加词库的词尚未写回，请稍后重试。";
inline constexpr std::wstring_view unconfirmed = L"无法确认清除结果，请刷新核对学习数据。";
}
