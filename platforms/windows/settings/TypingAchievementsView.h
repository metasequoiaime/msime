#pragma once

#include "TypingAchievementsData.h"
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>

namespace msime::settings::typing {

// 窗口提供现有文字样式；此组件不持有异步状态，也不另算徽章阈值。
template <class MakeLabel>
void append_achievements(
    winrt::Microsoft::UI::Xaml::Controls::StackPanel const &body,
    std::optional<AchievementsSnapshot> const &snapshot,
    MakeLabel const &label) {
  using namespace winrt::Microsoft::UI::Xaml;
  using namespace winrt::Microsoft::UI::Xaml::Controls;
  if (!snapshot) {
    body.Children().Append(label(L"成就暂时读不到，请稍后刷新。", false));
    return;
  }
  if (snapshot->badges.empty()) {
    body.Children().Append(label(L"暂无成就定义。", false));
    return;
  }
  body.Spacing(16);
  for (auto const &badge : snapshot->badges) {
    StackPanel card;
    card.Spacing(5);
    const auto title = badge.glyph + L" · " + badge.title;
    card.Children().Append(label(title, true));
    card.Children().Append(label(badge.description, false));
    card.Children().Append(label(achievement_status(badge), false));
    ProgressBar progress;
    progress.Minimum(0);
    progress.Maximum(1);
    progress.Value(achievement_progress(badge));
    Automation::AutomationProperties::SetName(
        progress, winrt::hstring(title + L"，" + achievement_status(badge)));
    card.Children().Append(progress);
    body.Children().Append(card);
  }
}

} // namespace msime::settings::typing
