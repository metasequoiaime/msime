#pragma once
#include "CandidatePalette.h"
#include "CandidateSkinAssets.h"
#include <filesystem>
#include <mutex>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>

namespace msime::windows {
// The preference fields the card, the toolbar and the menus are coloured from: the light/dark modes, the global theme and what a custom theme is made of. The custom keyboard design (which may carry a photo) colours nothing drawn here, so it is left behind rather than copied across threads.
inline nlohmann::json
candidate_theme_values(const nlohmann::json &preferences) {
  nlohmann::json result = nlohmann::json::object();
  for (const char *key : {"theme", "candidate_theme", "global_theme"}) {
    if (preferences.contains(key) && preferences.at(key).is_string()) {
      const auto value = preferences.at(key).get<std::string>();
      if (value.size() <= 32)
        result[key] = value;
    }
  }
  if (preferences.contains("custom_theme") &&
      preferences.at("custom_theme").is_object()) {
    nlohmann::json custom = nlohmann::json::object();
    const auto &stored = preferences.at("custom_theme");
    for (auto entry = stored.begin(); entry != stored.end(); ++entry)
      if (entry.key() != "keyboard")
        custom[entry.key()] = entry.value();
    result["custom_theme"] = std::move(custom);
  }
  return result;
}
inline bool candidate_theme_dark(const nlohmann::json &values,
                                 bool system_dark) {
  const auto theme = values.value("candidate_theme", std::string("follow"));
  if (theme == "dark" || theme == "light")
    return theme == "dark";
  const auto global = values.value("theme", std::string("dark"));
  return global == "system" ? system_dark : global != "light";
}
// The external package the selected theme draws, or empty. Only a custom theme names one; the skin root is watched for it so an edited package is redrawn.
inline std::string candidate_theme_package(const nlohmann::json &values) {
  if (!values.contains("global_theme") || values.at("global_theme") != "custom" ||
      !values.contains("custom_theme") || !values.at("custom_theme").is_object())
    return {};
  const auto &custom = values.at("custom_theme");
  if (!custom.contains("candidate_skin") ||
      !custom.at("candidate_skin").is_string())
    return {};
  auto id = custom.at("candidate_skin").get<std::string>();
  return valid_candidate_skin_id(id) ? id : std::string{};
}
// The msime_client_resolve_theme request for one surface. `dark` is that surface's own mode and `horizontal` the candidate layout being drawn. The skin root is sent whenever it is absolute; the shared layer only reads it for a custom theme that names a package.
inline nlohmann::json
candidate_theme_request(const nlohmann::json &values, bool dark,
                        bool horizontal,
                        const std::filesystem::path &skins_directory) {
  nlohmann::json request{
      {"global_theme",
       values.contains("global_theme") && values.at("global_theme").is_string()
           ? values.at("global_theme")
           : nlohmann::json("system")},
      {"dark", dark},
      {"layout", horizontal ? "horizontal" : "vertical"},
  };
  if (values.contains("custom_theme") && values.at("custom_theme").is_object())
    request["custom_theme"] = values.at("custom_theme");
  if (!skins_directory.empty() && skins_directory.is_absolute())
    request["skins_directory"] = skins_directory.u8string();
  return request;
}
// What the card draws from a resolved theme: the fixed mode, if the theme has one, the candidate slots it sets, and the package whose decoration and minimum width are drawn.
struct CandidateThemeResolution {
  std::optional<bool> dark;
  CandidatePaletteOverrides candidate;
  std::string candidate_skin;
};
// Read the `value` of a msime_client_resolve_theme response. A shape this reader does not recognise is refused as a whole, and the caller draws the native tokens.
inline std::optional<CandidateThemeResolution>
candidate_theme_resolution(const nlohmann::json &value) {
  if (!value.is_object())
    return std::nullopt;
  CandidateThemeResolution theme;
  if (value.contains("appearance") && !value.at("appearance").is_null()) {
    const auto &appearance = value.at("appearance");
    if (appearance == "dark")
      theme.dark = true;
    else if (appearance == "light")
      theme.dark = false;
    else
      return std::nullopt;
  }
  if (value.contains("candidate") && !value.at("candidate").is_null()) {
    const auto &candidate = value.at("candidate");
    if (!candidate.is_object())
      return std::nullopt;
    bool valid = true;
    auto color = [&candidate, &valid](const char *key,
                                      std::optional<std::string> &target) {
      if (!candidate.contains(key) || candidate.at(key).is_null())
        return;
      // The shared layer only emits #RRGGBB or #RRGGBBAA.
      if (!candidate.at(key).is_string() ||
          candidate.at(key).get<std::string>().size() > 16) {
        valid = false;
        return;
      }
      target = candidate.at(key).get<std::string>();
    };
    auto &slots = theme.candidate;
    color("surface", slots.surface);
    color("border", slots.border);
    color("text", slots.text);
    color("number", slots.number);
    color("accent", slots.accent);
    color("selected", slots.selected);
    color("selected_text", slots.selected_text);
    color("selected_number", slots.selected_number);
    color("hover", slots.hover);
    color("secondary", slots.secondary);
    if (candidate.contains("show_selected_bar") &&
        !candidate.at("show_selected_bar").is_null()) {
      if (!candidate.at("show_selected_bar").is_boolean())
        return std::nullopt;
      slots.show_selected_bar = candidate.at("show_selected_bar").get<bool>();
    }
    if (!valid)
      return std::nullopt;
  }
  if (value.contains("candidate_skin") && !value.at("candidate_skin").is_null()) {
    if (!value.at("candidate_skin").is_string())
      return std::nullopt;
    auto id = value.at("candidate_skin").get<std::string>();
    if (!valid_candidate_skin_id(id))
      return std::nullopt;
    theme.candidate_skin = std::move(id);
  }
  return theme;
}
// The card's palette for a resolved theme: the native tokens of the theme's own mode (or of the surface's, when the theme follows it), with the slots the theme sets drawn over them.
inline CandidatePalette
candidate_theme_palette(const CandidateThemeResolution &theme, bool dark) {
  return candidate_palette(theme.candidate,
                           candidate_native_palette(theme.dark.value_or(dark)));
}
// The title the theme picker shows for a global theme id, read from a msime_client_theme_catalog response so no host keeps its own copy of the names. Empty when the response is not a catalog, does not list the id or carries an implausible title, so the tray menu shows no hint rather than a raw id.
inline std::string theme_catalog_title(const nlohmann::json &response,
                                       const std::string &id) {
  if (!response.is_object() || !response.contains("ok") ||
      response.at("ok") != true || !response.contains("value") ||
      !response.at("value").is_object())
    return {};
  const auto &value = response.at("value");
  if (!value.contains("themes") || !value.at("themes").is_array())
    return {};
  for (const auto &theme : value.at("themes")) {
    if (!theme.is_object() || !theme.contains("id") ||
        theme.at("id") != id || !theme.contains("title") ||
        !theme.at("title").is_string())
      continue;
    auto title = theme.at("title").get<std::string>();
    return title.size() <= 64 ? title : std::string{};
  }
  return {};
}
// Only the small display projection crosses the preference/UI thread boundary.
class CandidateThemeMailbox {
public:
  void publish(const nlohmann::json &preferences) {
    auto values = candidate_theme_values(preferences);
    std::lock_guard lock(mutex_);
    pending_ = std::move(values);
  }
  std::optional<nlohmann::json> take() {
    std::lock_guard lock(mutex_);
    auto result = std::move(pending_);
    pending_.reset();
    return result;
  }

private:
  std::mutex mutex_;
  std::optional<nlohmann::json> pending_;
};
} // namespace msime::windows
