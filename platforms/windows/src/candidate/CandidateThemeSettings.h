#pragma once
#include "CandidatePalette.h"
#include "CandidateSkinAssets.h"
#include <filesystem>
#include <mutex>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <utility>
#include <vector>

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
// 所选主题可能绘制的外部皮肤包：自定义主题浅色槽位 `candidate_skin` 与深色槽位 `candidate_skin_dark` 指名的包，去重，最多两个；其他主题一个也没有。候选窗、工具条、语音浮层和菜单各按自己的明暗解析主题，会画到其中任一个，所以皮肤根目录下这两个包都要监视，任一个被编辑都重新绘制。
inline std::vector<std::string>
candidate_theme_packages(const nlohmann::json &values) {
  std::vector<std::string> packages;
  if (!values.contains("global_theme") || values.at("global_theme") != "custom" ||
      !values.contains("custom_theme") || !values.at("custom_theme").is_object())
    return packages;
  const auto &custom = values.at("custom_theme");
  for (const char *slot : {"candidate_skin", "candidate_skin_dark"}) {
    if (!custom.contains(slot) || !custom.at(slot).is_string())
      continue;
    auto id = custom.at(slot).get<std::string>();
    if (valid_candidate_skin_id(id) &&
        (packages.empty() || packages.front() != id))
      packages.push_back(std::move(id));
  }
  return packages;
}
// 一个界面的 msime_client_resolve_theme 请求。`dark` 是这个界面自己的明暗，`horizontal` 是正在绘制的候选布局。`custom_theme` 原样传过去，`candidate_skin_dark` 也在其中；皮肤根目录只要是绝对路径就发送，共享层只在自定义主题的当前明暗槽位指名皮肤包时才读它（深色取 `candidate_skin_dark`，没设时取 `candidate_skin`），宿主自己不挑包。
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
// 托盘主题页列出的主题：msime_client_theme_catalog 回答里本平台的主题 id 和标题，按选择器顺序。id 要写进偏好，只收短的小写 ASCII 标识；标题和 theme_catalog_title 一样不收空的或过长的。回答不是目录时为空。
inline std::vector<std::pair<std::string, std::string>>
theme_catalog_entries(const nlohmann::json &response) {
  std::vector<std::pair<std::string, std::string>> entries;
  if (!response.is_object() || !response.contains("ok") ||
      response.at("ok") != true || !response.contains("value") ||
      !response.at("value").is_object())
    return entries;
  const auto &value = response.at("value");
  if (!value.contains("themes") || !value.at("themes").is_array())
    return entries;
  for (const auto &theme : value.at("themes")) {
    if (!theme.is_object() || !theme.contains("id") || !theme.at("id").is_string() ||
        !theme.contains("title") || !theme.at("title").is_string())
      continue;
    auto id = theme.at("id").get<std::string>();
    auto title = theme.at("title").get<std::string>();
    if (id.empty() || id.size() > 64 ||
        id.find_first_not_of("abcdefghijklmnopqrstuvwxyz0123456789-_") !=
            std::string::npos ||
        title.empty() || title.size() > 64)
      continue;
    entries.emplace_back(std::move(id), std::move(title));
  }
  return entries;
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
