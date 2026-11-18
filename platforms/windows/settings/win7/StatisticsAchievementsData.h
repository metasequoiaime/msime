#pragma once

#include "../TypingAchievementsModel.h"
#include "ControlText.h"
#include "ThemeData.h"

namespace msime::settings::win7::statistics_achievements {

inline Json load(std::string const &directory, std::string const &day,
                 Json const &options) {
  Json action{{"operation", "summary"}, {"day", day}};
  if (options.is_object()) {
    const auto count = theme_data::value(theme_data::call(
        msime_client_dictionary,
        {{"options", options},
         {"action",
          {{"operation", "count"}, {"kind", "pinyin"}, {"user_only", true}}}}));
    if (count && count->is_object() && count->contains("count") &&
        count->at("count").is_number_unsigned())
      action["user_words"] = count->at("count");
  }
  return theme_data::call(msime_client_typing_statistics,
                          {{"directory", directory}, {"action", action}});
}

inline std::optional<std::vector<typing::Achievement>>
parse(Json const &response) {
  const auto value = theme_data::value(response);
  if (!value || !value->is_object() || !value->contains("achievements") ||
      !value->at("achievements").is_array())
    return std::nullopt;
  try {
    std::vector<typing::Achievement> badges;
    for (auto const &entry : value->at("achievements")) {
      typing::Achievement badge;
      badge.id = control_text::wide(entry.at("id").get<std::string>());
      badge.glyph = control_text::wide(entry.at("glyph").get<std::string>());
      badge.title = control_text::wide(entry.at("title").get<std::string>());
      badge.description =
          control_text::wide(entry.at("description").get<std::string>());
      badge.current = entry.at("current").get<double>();
      badge.target = entry.at("target").get<double>();
      if (!std::isfinite(badge.current) || badge.current < 0 ||
          !std::isfinite(badge.target) || badge.target <= 0)
        return std::nullopt;
      if (!entry.at("unlocked_day").is_null())
        badge.unlocked_day =
            control_text::wide(entry.at("unlocked_day").get<std::string>());
      badges.push_back(std::move(badge));
    }
    return badges;
  } catch (...) {
    return std::nullopt;
  }
}

} // namespace msime::settings::win7::statistics_achievements
