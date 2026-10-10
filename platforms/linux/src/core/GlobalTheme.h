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
  // 皮肤包所属的槽位：清单 base 在共享主题目录里的 appearance，`light` 放 `candidate_skin`，`dark` 放 `candidate_skin_dark`；base 跟随系统（appearance 为 null）时为空串，两个槽位都放。只对皮肤包有意义。
  std::string package_slot;
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
  // 各全局主题的 appearance，与 choices 前段一一对应，用来给皮肤包定槽位。
  std::vector<std::string> appearances;
  appearances.reserve(theme_capacity);
  if (themes != theme_catalog.end() && themes->is_array())
    for (const auto &theme : *themes) {
      if (!theme.is_object()) continue;
      const auto id = theme.find("id");
      const auto title = theme.find("title");
      if (id == theme.end() || !id->is_string() || title == theme.end() || !title->is_string()) continue;
      choices.push_back({id->get<std::string>(), title->get<std::string>(), std::nullopt, {}});
      const auto appearance = theme.find("appearance");
      appearances.push_back(appearance != theme.end() && appearance->is_string() ? appearance->get<std::string>()
                                                                                  : std::string{});
    }
  const auto theme_count = choices.size();
  for (const auto &package : packages) {
    bool reserved = false;
    const std::string *base_appearance = nullptr;
    for (std::size_t index = 0; index < theme_count; ++index) {
      reserved = reserved || choices[index].id == package.id;
      if (choices[index].id == package.base && package.base != "custom") base_appearance = &appearances[index];
    }
    if (!reserved && base_appearance) choices.push_back({package.id, package.title, package.base, *base_appearance});
  }
  return choices;
}

inline const ThemeChoice *find_theme_choice(const std::vector<ThemeChoice> &choices, std::string_view id) {
  for (const auto &choice : choices)
    if (choice.id == id) return &choice;
  return nullptr;
}

// 菜单勾选的条目：自定义主题的任一槽位指名已列出的皮肤包时是那个包，否则是全局主题本身。单选菜单只能勾一项，两个槽位各放一款时先取 `dark`（候选窗当前的明暗）下画的那款，这个模式的槽位没有可列出的包时再取另一个槽位的。
inline std::string current_theme_choice(const nlohmann::json &preferences, const std::vector<ThemeChoice> &choices,
                                        bool dark) {
  const auto theme = preferences.find("global_theme");
  const std::string id = theme != preferences.end() && theme->is_string() ? theme->get<std::string>() : "system";
  if (id != "custom") return id;
  const auto custom = preferences.find("custom_theme");
  if (custom == preferences.end() || !custom->is_object()) return id;
  for (const auto &skin : {candidate_skin_for(*custom, dark), candidate_skin_for(*custom, !dark)}) {
    const auto *choice = find_theme_choice(choices, skin);
    if (choice && choice->package_base) return choice->id;
  }
  return id;
}

// 选中一个条目对偏好的改动，形如 {"global_theme": id, "custom_theme": {key: 值或 null}}，null 表示删掉这个键。`preferences` 是改动前的偏好，皮肤包按它当前的两个槽位决定写哪些键。选皮肤包即选中以它为底的自定义主题（设置页的皮肤卡片），规则与设置页的 `applyCandidateSkin`（packages/ui/src/theme/global-theme.ts）逐条一致：深色皮肤写 `candidate_skin_dark`，并清掉旧文档放在 `candidate_skin` 里的深色皮肤；浅色皮肤写 `candidate_skin`，但深色槽位空着、而原来的 `candidate_skin` 确知是深色或跟随系统的皮肤时，先把它挪进深色槽位（不在菜单里、不知道明暗的直接覆盖，不挪），免得只设过一款深色皮肤的旧文档悄悄丢掉它；base 跟随系统的皮肤两个槽位都写。`base` 照旧写成包清单的 base。自定义按原样选中自定义主题，与设置页的「自定义」卡片一样（THEME_CONTRACT 第 5 节）：不改动存着的自定义主题，所以它正以某个已列出的皮肤包绘制时，菜单随后勾选的是那个包的条目，因为画出来的就是它；只有设置页的「自定义主题不使用外部皮肤」才停止画皮肤包。id 不是菜单条目时什么也不返回。
inline std::optional<nlohmann::json> theme_choice_change(const nlohmann::json &preferences,
                                                         const std::vector<ThemeChoice> &choices,
                                                         std::string_view id) {
  using Json = nlohmann::json;
  const auto *choice = find_theme_choice(choices, id);
  if (!choice) return std::nullopt;
  if (!choice->package_base) return Json{{"global_theme", choice->id}};
  const auto stored = preferences.is_object() ? preferences.find("custom_theme") : preferences.end();
  const auto slot_value = [&](const char *key) {
    if (stored == preferences.end() || !stored->is_object()) return std::string{};
    const auto value = stored->find(key);
    return value != stored->end() && value->is_string() ? value->get<std::string>() : std::string{};
  };
  // 一款皮肤所在的槽位；菜单没列出它时不知道（nullopt）。
  const auto slot_of = [&](const std::string &skin) -> std::optional<std::string> {
    const auto *listed = find_theme_choice(choices, skin);
    if (!listed || !listed->package_base) return std::nullopt;
    return listed->package_slot;
  };
  const auto previous = slot_value("candidate_skin");
  const auto previous_dark = slot_value("candidate_skin_dark");
  const auto &slot = choice->package_slot;
  Json custom{{"base", *choice->package_base}};
  if (slot != "light") custom["candidate_skin_dark"] = choice->id;
  // 旧文档放在 `candidate_skin` 里的深色皮肤被新的深色皮肤取代：浅色模式本来就不画它，留着只会让它看起来还在用。
  if (slot == "dark" && !previous.empty() && slot_of(previous) == "dark") custom["candidate_skin"] = nullptr;
  if (slot != "dark") {
    if (const auto known = slot_of(previous);
        slot == "light" && previous_dark.empty() && !previous.empty() && known && *known != "light")
      custom["candidate_skin_dark"] = previous;
    custom["candidate_skin"] = choice->id;
  }
  return Json{{"global_theme", "custom"}, {"custom_theme", std::move(custom)}};
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
