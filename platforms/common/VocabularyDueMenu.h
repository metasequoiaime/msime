#pragma once

#include <cstdint>
#include <ctime>
#include <optional>
#include <string>

// 菜单栏、托盘菜单和 IBus/Fcitx 菜单里那一行「背单词（今日待复习 N）…」的共同规则，macOS、Windows 和 Linux 三个宿主各自建菜单，文案、显示条件和要打开的面板只写在这里。计数来自 msime_client_vocabulary_due_count，见 .agents/notes/implemented/feature/2026-11-01-vocabulary-due-count.md。
namespace msime::vocabulary_due {

// 共享应用的背单词面板（client-core 的 SurfaceRoute::Vocabulary）。
inline constexpr const char *kPanelRoute = "vocabulary";

// 这一行的文字。计数不大于 0（没有待复习、没开过背单词、读不出来时的 -1）或正在组字时不显示这一行，返回空：提醒只能在用户自己点开的菜单里被动地出现，组字期间从不出现。
inline std::optional<std::string> menu_label(int32_t count, bool composing) {
  if (count <= 0 || composing)
    return std::nullopt;
  return "背单词（今日待复习 " + std::to_string(count) + "）…";
}

// 本机时区的日期 YYYY-MM-DD，计数请求的 day。共享层不推算日期，由宿主按本机时区给出，和设置应用的背单词页一致。
inline std::string local_day(std::time_t now) {
  std::tm local{};
#if defined(_WIN32)
  if (localtime_s(&local, &now) != 0)
    return {};
#else
  if (!localtime_r(&now, &local))
    return {};
#endif
  char day[16]{};
  if (std::strftime(day, sizeof(day), "%Y-%m-%d", &local) != 10)
    return {};
  return day;
}

} // namespace msime::vocabulary_due
