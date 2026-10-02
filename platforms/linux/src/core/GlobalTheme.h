#pragma once
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "CandidateSkinCatalog.h"

namespace msime::linux_host {

// One entry of the 主题 menu both frontends draw: a global theme from msime_client_theme_catalog, or an installed skin package, which selects the custom theme drawn over that package the way the settings page's package card does. The host keeps no copy of the theme ids or titles; they all come from the shared catalogue.
struct ThemeChoice {
  std::string id;
  std::string title;
  // Set for an installed package: the manifest base the custom theme is drawn over once it is chosen.
  std::optional<std::string> package_base;
};

// The menu entries: the global themes in the catalogue's picker order, then the installed packages. A package whose id is a global theme id is left out, as the shared layer refuses it (catalog::is_external_id). So is a package whose base is not a catalogue theme other than 自定义: the shared layer draws a package only over system or a built-in theme (GlobalTheme::is_base), so choosing it would write a custom theme the preferences refuse. The check reads the catalogue rather than a copy of the ids.
inline std::vector<ThemeChoice> theme_choices(const nlohmann::json &theme_catalog,
                                              const std::vector<CandidateSkin> &packages) {
  const auto themes = theme_catalog.is_object() ? theme_catalog.find("themes") : theme_catalog.end();
  const auto theme_capacity = themes != theme_catalog.end() && themes->is_array()
                                  ? themes->size()
                                  : 0;
  std::vector<ThemeChoice> choices;
  choices.reserve(theme_capacity + packages.size());
  if (themes != theme_catalog.end() && themes->is_array())
    for (const auto &theme : *themes) {
      if (!theme.is_object()) continue;
      const auto id = theme.find("id");
      const auto title = theme.find("title");
      if (id == theme.end() || !id->is_string() || title == theme.end() || !title->is_string()) continue;
      choices.push_back({id->get<std::string>(), title->get<std::string>(), std::nullopt});
    }
  const auto theme_count = choices.size();
  for (const auto &package : packages) {
    bool reserved = false;
    bool base_known = false;
    for (std::size_t index = 0; index < theme_count; ++index) {
      reserved = reserved || choices[index].id == package.id;
      base_known = base_known || (choices[index].id == package.base && package.base != "custom");
    }
    if (!reserved && base_known) choices.push_back({package.id, package.title, package.base});
  }
  return choices;
}

inline const ThemeChoice *find_theme_choice(const std::vector<ThemeChoice> &choices, std::string_view id) {
  for (const auto &choice : choices)
    if (choice.id == id) return &choice;
  return nullptr;
}

// The entry the menu shows as selected: the package when the custom theme is drawn over a listed one, otherwise the global theme itself.
inline std::string current_theme_choice(const nlohmann::json &preferences, const std::vector<ThemeChoice> &choices) {
  const auto theme = preferences.find("global_theme");
  const std::string id = theme != preferences.end() && theme->is_string() ? theme->get<std::string>() : "system";
  if (id != "custom") return id;
  const auto custom = preferences.find("custom_theme");
  if (custom == preferences.end() || !custom->is_object()) return id;
  const auto skin = custom->find("candidate_skin");
  if (skin == custom->end() || !skin->is_string()) return id;
  const auto *choice = find_theme_choice(choices, skin->get<std::string>());
  return choice && choice->package_base ? choice->id : id;
}

// The preferences change choosing one entry makes, as {"global_theme": id, "custom_theme": {key: value or null}}; null removes the key. A package selects the custom theme drawn over it and its manifest base (the package card). 自定义 selects the custom theme as it stands, as the settings page's 自定义 card does (THEME_CONTRACT section 5): the stored custom theme is not edited, so when it is drawn over a listed package the menu then checks that package's entry, which is what is drawn. Only the settings page's 自定义主题不使用外部皮肤 stops drawing a package. Nothing when the id is not an entry.
inline std::optional<nlohmann::json> theme_choice_change(const std::vector<ThemeChoice> &choices,
                                                         std::string_view id) {
  using Json = nlohmann::json;
  const auto *choice = find_theme_choice(choices, id);
  if (!choice) return std::nullopt;
  if (choice->package_base)
    return Json{{"global_theme", "custom"},
                {"custom_theme", {{"candidate_skin", choice->id}, {"base", *choice->package_base}}}};
  return Json{{"global_theme", choice->id}};
}

// Write a change from theme_choice_change into a preferences object.
inline void apply_theme_choice(nlohmann::json &preferences, const nlohmann::json &change) {
  preferences["global_theme"] = change.at("global_theme");
  const auto custom = change.find("custom_theme");
  if (custom == change.end()) return;
  auto &target = preferences["custom_theme"];
  if (!target.is_object()) target = nlohmann::json::object();
  for (const auto &[key, value] : custom->items()) {
    if (value.is_null())
      target.erase(key);
    else
      target[key] = value;
  }
}

}  // namespace msime::linux_host
