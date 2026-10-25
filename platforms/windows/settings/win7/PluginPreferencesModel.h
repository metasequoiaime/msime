#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "../PluginPageModel.h"
#include <functional>

namespace msime::settings::win7::plugin_preferences {
namespace model = msime::settings::plugins;
using Path = std::vector<std::string>;

// 页面模型持有既有默认值；这里仅将共享偏好投影成同一模型，并比较明确修改的字段。
inline std::vector<std::pair<Path, Json>> values(const model::Preferences& preferences) {
  const auto word = [](const std::wstring& text) { return control_text::utf8(text).value(); };
  const auto list = [&](const std::vector<std::wstring>& strings) {
    Json output = Json::array(); for (const auto& text : strings) output.push_back(word(text)); return output;
  };
  return {
    {{"key_sound", "enabled"}, preferences.key_sound_enabled}, {{"key_sound", "mode"}, word(preferences.key_sound_mode)},
    {{"key_sound", "pack"}, word(preferences.key_sound_pack)}, {{"key_sound", "volume"}, static_cast<int>(preferences.key_sound_volume)},
    {{"commit_sound", "enabled"}, preferences.commit_sound_enabled}, {{"melody", "pack"}, word(preferences.melody_pack)},
    {{"music", "enabled"}, preferences.music_enabled}, {{"music", "pack"}, word(preferences.music_pack)},
    {{"music", "volume"}, static_cast<int>(preferences.music_volume)}, {{"achievements", "enabled"}, preferences.achievements_enabled},
    {{"command_tables"}, list(preferences.command_tables)}, {{"effect_style"}, word(preferences.effect_style)},
    {{"effect_intensity"}, static_cast<int>(preferences.effect_intensity)}, {{"effect_pack"}, word(preferences.effect_pack)},
    {{"combo_counter"}, preferences.combo_counter}, {{"combo_tier_sound"}, preferences.combo_tier_sound},
    {{"phrase_tables"}, list(preferences.phrase_tables)}, {{"helpcode_pack_quanpin"}, word(preferences.helpcode_pack_quanpin)},
    {{"helpcode_pack_shuangpin"}, word(preferences.helpcode_pack_shuangpin)},
  };
}
inline const Json* find(const Json& root, const Path& path) {
  const auto* current = &root;
  for (const auto& key : path) {
    if (!current->is_object() || !current->contains(key)) return nullptr;
    current = &current->at(key);
  }
  return current;
}
inline std::optional<model::Preferences> read(const Json& document) {
  model::Preferences preferences;
  if (!document.contains("plugins")) return preferences;
  const auto& source = document.at("plugins");
  if (!source.is_object()) return std::nullopt;
  for (const auto* key : {"key_sound", "commit_sound", "melody", "music", "achievements"})
    if (source.contains(key) && !source.at(key).is_object()) return std::nullopt;
  bool valid = true;
  const auto flag = [&](const Path& path, bool& destination) {
    if (const auto* value = find(source, path)) { if (value->is_boolean()) destination = value->get<bool>(); else valid = false; }
  };
  const auto word = [&](const Path& path, std::wstring& destination) {
    if (const auto* value = find(source, path)) { if (value->is_string()) destination = control_text::wide(value->get<std::string>()); else valid = false; }
  };
  const auto number = [&](const Path& path, double& destination) {
    if (const auto* value = find(source, path)) { if (value->is_number_integer() && value->get<double>() >= 0 && value->get<double>() <= 100) destination = value->get<double>(); else valid = false; }
  };
  const auto list = [&](const Path& path, std::vector<std::wstring>& destination) {
    if (const auto* value = find(source, path)) {
      if (!value->is_array()) { valid = false; return; }
      destination.clear();
      for (const auto& item : *value) { if (!item.is_string()) { valid = false; return; } destination.push_back(control_text::wide(item.get<std::string>())); }
    }
  };
  flag({"key_sound", "enabled"}, preferences.key_sound_enabled); word({"key_sound", "mode"}, preferences.key_sound_mode);
  word({"key_sound", "pack"}, preferences.key_sound_pack); number({"key_sound", "volume"}, preferences.key_sound_volume);
  flag({"commit_sound", "enabled"}, preferences.commit_sound_enabled); word({"melody", "pack"}, preferences.melody_pack);
  flag({"music", "enabled"}, preferences.music_enabled); word({"music", "pack"}, preferences.music_pack); number({"music", "volume"}, preferences.music_volume);
  flag({"achievements", "enabled"}, preferences.achievements_enabled); list({"command_tables"}, preferences.command_tables);
  word({"effect_style"}, preferences.effect_style); number({"effect_intensity"}, preferences.effect_intensity); word({"effect_pack"}, preferences.effect_pack);
  flag({"combo_counter"}, preferences.combo_counter); flag({"combo_tier_sound"}, preferences.combo_tier_sound);
  list({"phrase_tables"}, preferences.phrase_tables); word({"helpcode_pack_quanpin"}, preferences.helpcode_pack_quanpin); word({"helpcode_pack_shuangpin"}, preferences.helpcode_pack_shuangpin);
  return valid ? std::make_optional(std::move(preferences)) : std::nullopt;
}
inline std::vector<Change> changes(const model::Preferences& before, const model::Preferences& after) {
  const auto previous = values(before), next = values(after);
  std::vector<Change> result;
  for (size_t index = 0; index < next.size(); ++index) {
    if (previous[index].second == next[index].second) continue;
    auto path = next[index].first; path.insert(path.begin(), "plugins");
    result.push_back({std::move(path), next[index].second});
  }
  return result;
}
} // namespace msime::settings::win7::plugin_preferences
