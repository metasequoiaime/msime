#include "SettingsRoutes.h"
#include "../SettingsNavigation.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <utility>

namespace msime::settings::win7 {
// 只登记已经有完整原生承载的页面，开发中的其他页面不能悄悄落到无关设置。
std::optional<int> settings_page_for_route(std::string_view route) {
  if (route == "settings") return 3;
  constexpr std::string_view prefix = "settings:";
  if (route.substr(0, prefix.size()) != prefix) return std::nullopt;
  const auto category = route.substr(prefix.size());
  if (category == "vocabulary") return 14;
  if (category == "community") return 9;
  if (category == "keyboard-skin") return 22;
  if (category == "mcp") return 21;
  if (category == "voice") return 19;
  if (category == "screen-keyboard" || category == "osk") return 20;
  if (category == "handwriting") return MSIME_EDITION_HANDWRITING != 0 ? 20 : 3;
  if (category == "community-resources") return 18;
  if (category == "community-plugins") return 17;
  bool recognized = find_page(category) != nullptr;
  for (const auto& alias : route_aliases) if (alias.route == category) recognized = true;
  if (!recognized) return std::nullopt;
  const auto page = page_for_route(category);
  for (const auto& entry : {std::pair<std::string_view, int>{"typing", 3}, {"shortcuts", 0}, {"toolbar", 1},
      {"dev", 2}, {"expression", 4}, {"candidate", 5}, {"ai", 6}, {"account", 7}, {"themes", 8},
      {"stats", 10}, {"clip", 11}, {"lexicon", 13}, {"plugins", 15}, {"about", 16}})
    if (entry.first == page) return entry.second;
  return std::nullopt;
}
} // namespace msime::settings::win7
