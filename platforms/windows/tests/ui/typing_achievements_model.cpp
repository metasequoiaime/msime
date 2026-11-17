#include "../../settings/TypingAchievementsModel.h"
#include <cassert>

int main() {
  using namespace msime::settings::typing;
  Achievement badge{L"sample",    L"字", L"合成成就", L"合成说明",
                    std::nullopt, 25,    100};
  assert(achievement_progress(badge) == 0.25);
  assert(achievement_status(badge) == L"进度 25 / 100");
  badge.current = 150;
  assert(achievement_progress(badge) == 1);
  badge.unlocked_day = L"2026-11-17";
  badge.current = 0;
  assert(achievement_progress(badge) == 1);
  assert(achievement_status(badge) == L"已解锁 · 2026-11-17");
  assert(achievements_text(std::nullopt) == L"成就暂时读不到，请刷新统计。");
  assert(achievements_text(std::vector<Achievement>{badge}).find(L"合成说明") !=
         std::wstring::npos);
}
