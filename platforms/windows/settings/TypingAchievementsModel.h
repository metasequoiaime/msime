#pragma once

#include "TypingStatisticsModel.h"

// 徽章定义、阈值和解锁判定来自共享 Rust，这里只保存答复与格式化进度。
namespace msime::settings::typing {

struct Achievement {
  std::wstring id, glyph, title, description;
  std::optional<std::wstring> unlocked_day;
  double current = 0;
  double target = 0;
};

inline double achievement_progress(Achievement const &badge) {
  if (badge.unlocked_day)
    return 1;
  if (!std::isfinite(badge.current) || !std::isfinite(badge.target) ||
      badge.target <= 0)
    return 0;
  return std::clamp(badge.current / badge.target, 0.0, 1.0);
}

inline std::wstring achievement_status(Achievement const &badge) {
  if (badge.unlocked_day)
    return L"已解锁 · " + *badge.unlocked_day;
  return L"进度 " + zh_number(badge.current) + L" / " + zh_number(badge.target);
}

inline std::wstring
achievements_text(std::optional<std::vector<Achievement>> const &badges) {
  if (!badges)
    return L"成就暂时读不到，请刷新统计。";
  if (badges->empty())
    return L"暂无成就定义。";
  std::wstring result;
  for (auto const &badge : *badges)
    result += badge.glyph + L" · " + badge.title + L"　" +
              achievement_status(badge) + L"\r\n" + badge.description +
              L"\r\n\r\n";
  return result;
}

} // namespace msime::settings::typing
