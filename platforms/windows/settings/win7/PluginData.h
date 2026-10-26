#pragma once

#include "PluginPreferencesModel.h"
#include "ThemeData.h"

namespace msime::settings::win7::plugin_data {
inline std::optional<std::filesystem::path> executable_directory() {
  std::wstring name(32768, L'\0');
  const auto length = GetModuleFileNameW(nullptr, name.data(), static_cast<DWORD>(name.size()));
  if (!length || length >= name.size()) return std::nullopt;
  name.resize(length); return std::filesystem::path(name).parent_path();
}
inline Json call(const std::filesystem::path& directory, const Json& options, Json action) {
  const auto executable = executable_directory();
  if (!executable) return Json{{"ok", false}, {"error", "plugin_storage"}};
  Json request{{"state_root", directory.u8string()}, {"script_page", true}, {"sound_packs", (directory / L"sound-packs").u8string()},
      {"plugin_packs", (*executable / L"plugin-packs").u8string()}, {"action", std::move(action)}};
  for (const auto* key : {"sound_packs", "plugin_packs", "user_data"})
    if (options.is_object() && options.contains(key) && options.at(key).is_string() && std::filesystem::u8path(options.at(key).get<std::string>()).is_absolute()) request[key] = options.at(key);
  return theme_data::call(msime_client_plugins, request);
}
inline std::optional<std::vector<plugin_preferences::model::Package>> packages(const Json& response) {
  const auto value = theme_data::value(response);
  if (!value || !value->is_object() || !value->contains("packages") || !value->at("packages").is_array() ||
      !value->contains("issues") || !value->at("issues").is_array()) return std::nullopt;
  std::vector<plugin_preferences::model::Package> result;
  for (const auto& item : value->at("packages")) {
    if (!item.is_object()) return std::nullopt;
    for (const auto* key : {"id", "name", "kind", "version"}) if (!item.contains(key) || !item.at(key).is_string()) return std::nullopt;
    if (!item.contains("builtin") || !item.at("builtin").is_boolean()) return std::nullopt;
    plugin_preferences::model::Package pack;
    bool valid = true;
    const auto text = [&](const char* key) {
      if (!item.contains(key) || item.at(key).is_null()) return std::wstring{};
      if (!item.at(key).is_string()) { valid = false; return std::wstring{}; }
      return control_text::wide(item.at(key).get<std::string>());
    };
    pack.id = text("id"); pack.name = text("name"); pack.kind = text("kind"); pack.version = text("version");
    pack.license = text("license"); pack.author = text("author"); pack.description = text("description"); pack.mode = text("mode");
    const auto strings = [&](const char* key, std::vector<std::wstring>& output) {
      if (!item.contains(key)) return;
      if (!item.at(key).is_array()) { valid = false; return; }
      for (const auto& entry : item.at(key)) { if (!entry.is_string()) { valid = false; return; } output.push_back(control_text::wide(entry.get<std::string>())); }
    };
    strings("hooks", pack.hooks); strings("permissions", pack.permissions); pack.content_hash = text("content_hash");
    if (pack.kind == L"script" && pack.content_hash.empty()) valid = false;
    if (item.contains("commands")) {
      if (!item.at("commands").is_array()) valid = false;
      else for (const auto& command : item.at("commands")) {
        if (!command.is_object() || !command.contains("trigger") || !command.at("trigger").is_string() || !command.contains("title") || !command.at("title").is_string() ||
            (command.contains("template") && !command.at("template").is_string()) || (command.contains("arguments") && !command.at("arguments").is_string())) { valid = false; break; }
        pack.commands.push_back({control_text::wide(command.at("trigger").get<std::string>()), control_text::wide(command.at("title").get<std::string>()), control_text::wide(command.value("template", std::string{}))});
        if (command.value("arguments", std::string{}) == "extended") pack.extended_commands.push_back(pack.commands.back().trigger);
      }
    }
    // 脚本插件清单 `[[script.settings]]`、随包数据文件名和合计字节数；没有时都不出现。
    if (item.contains("settings")) {
      if (!item.at("settings").is_array()) valid = false;
      else for (const auto& entry : item.at("settings")) {
        if (!entry.is_object() || !entry.contains("key") || !entry.at("key").is_string() || !entry.contains("title") || !entry.at("title").is_string() ||
            !entry.contains("type") || !entry.at("type").is_string() || !entry.contains("default")) { valid = false; break; }
        plugin_preferences::model::ScriptSetting setting;
        setting.key = control_text::wide(entry.at("key").get<std::string>()); setting.title = control_text::wide(entry.at("title").get<std::string>());
        setting.description = control_text::wide(entry.value("description", std::string{})); setting.type = control_text::wide(entry.at("type").get<std::string>());
        const auto& fallback = entry.at("default");
        if (fallback.is_boolean()) setting.bool_default = fallback.get<bool>();
        else if (fallback.is_number_integer()) setting.number_default = fallback.get<std::int64_t>();
        else if (fallback.is_string()) setting.text_default = control_text::wide(fallback.get<std::string>());
        else { valid = false; break; }
        setting.min = entry.value("min", std::int64_t{0}); setting.max = entry.value("max", std::int64_t{0}); setting.max_length = entry.value("max_length", std::size_t{0});
        if (entry.contains("options")) {
          if (!entry.at("options").is_array()) { valid = false; break; }
          for (const auto& option : entry.at("options")) {
            if (!option.is_object() || !option.contains("value") || !option.at("value").is_string() || !option.contains("title") || !option.at("title").is_string()) { valid = false; break; }
            setting.options.push_back({control_text::wide(option.at("value").get<std::string>()), control_text::wide(option.at("title").get<std::string>())});
          }
        }
        pack.settings.push_back(std::move(setting));
      }
    }
    strings("data", pack.data);
    if (item.contains("data_bytes")) { if (item.at("data_bytes").is_number()) pack.data_bytes = item.at("data_bytes").get<double>(); else valid = false; }
    strings("tracks", pack.tracks); strings("first_words", pack.first_words); strings("colors", pack.colors);
    pack.table = text("table"); pack.file = text("file"); pack.style = text("style");
    const auto number = [&](const char* field, std::optional<double>& output) {
      if (!item.contains(field) || item.at(field).is_null()) return;
      if (!item.at(field).is_number()) { valid = false; return; } output = item.at(field).get<double>();
    };
    number("entries", pack.entries); number("word_count", pack.word_count); number("intensity", pack.intensity);
    number("duration_ms", pack.duration_ms); number("particles", pack.particles);
    const auto pairs = [&](const char* field, const char* first, const char* second, const auto& accept) {
      if (!item.contains(field)) return;
      if (!item.at(field).is_array()) { valid = false; return; }
      for (const auto& entry : item.at(field)) {
        if (!entry.is_object() || !entry.contains(first) || !entry.at(first).is_string() || !entry.contains(second) || !entry.at(second).is_string()) { valid = false; return; }
        accept(control_text::wide(entry.at(first).template get<std::string>()), control_text::wide(entry.at(second).template get<std::string>()));
      }
    };
    pairs("phrases", "key", "text", [&](std::wstring key, std::wstring text) { pack.phrases.push_back({std::move(key), std::move(text)}); });
    pairs("preview", "character", "code", [&](std::wstring character, std::wstring code) { pack.preview.push_back({std::move(character), std::move(code)}); });
    if (item.contains("groups")) {
      if (!item.at("groups").is_array()) valid = false;
      else for (const auto& group : item.at("groups")) {
        if (!group.is_object() || !group.contains("tab") || !group.at("tab").is_string() || !group.contains("title") || !group.at("title").is_string() ||
            !group.contains("items") || !group.at("items").is_array() || (group.contains("keywords") && !group.at("keywords").is_string())) { valid = false; break; }
        plugin_preferences::model::SymbolGroup output{control_text::wide(group.at("tab").get<std::string>()), control_text::wide(group.at("title").get<std::string>()), control_text::wide(group.value("keywords", std::string{})), {}};
        for (const auto& symbol : group.at("items")) { if (!symbol.is_string()) { valid = false; break; } output.items.push_back(control_text::wide(symbol.get<std::string>())); }
        pack.groups.push_back(std::move(output));
      }
    }
    if (!valid) return std::nullopt;
    pack.builtin = item.at("builtin").get<bool>();
    result.push_back(std::move(pack));
  }
  return result;
}
inline std::optional<plugin_preferences::model::ScriptGrants> grants(const Json& response) {
  const auto value = theme_data::value(response);
  if (!value || !value->is_object()) return std::nullopt;
  for (const auto* field : {"plugins", "health", "commands"}) if (!value->contains(field) || !value->at(field).is_array()) return std::nullopt;
  plugin_preferences::model::ScriptGrants output;
  bool valid = true;
  const auto text = [&](const Json& item, const char* field, bool required = true) {
    if (!item.is_object() || !item.contains(field)) { if (required) valid = false; return std::wstring{}; }
    if (!item.at(field).is_string()) { valid = false; return std::wstring{}; }
    return control_text::wide(item.at(field).get<std::string>());
  };
  for (const auto& item : value->at("plugins")) {
    plugin_preferences::model::ScriptGrant grant;
    grant.id = text(item, "id"); grant.hash = text(item, "hash");
    if (!item.is_object() || !item.contains("enabled") || !item.at("enabled").is_boolean() || !item.contains("granted") || !item.at("granted").is_array()) return std::nullopt;
    grant.enabled = item.at("enabled").get<bool>();
    for (const auto& entry : item.at("granted")) { if (!entry.is_string()) return std::nullopt; grant.granted.push_back(control_text::wide(entry.get<std::string>())); }
    output.plugins.push_back(std::move(grant));
  }
  for (const auto& item : value->at("health")) output.health.push_back({text(item, "id"), text(item, "hash"), text(item, "disabled")});
  for (const auto& item : value->at("commands")) output.commands.push_back({text(item, "id"), text(item, "trigger"), text(item, "title"), text(item, "status"), text(item, "conflict", false), text(item, "conflict_id", false)});
  return valid ? std::make_optional(std::move(output)) : std::nullopt;
}
// script_plugin_state、set_script_settings 和 clear_script_data 的回答（client-services 的 ScriptPluginStateView）；读不懂时为空。值按 JSON 类型换成布尔、整数或文字。
inline std::optional<plugin_preferences::model::ScriptPluginState> script_state(const Json& response) {
  const auto value = theme_data::value(response);
  if (!value || !value->is_object() || !value->contains("values") || !value->at("values").is_object()) return std::nullopt;
  plugin_preferences::model::ScriptPluginState state;
  if (value->contains("storage_bytes") && value->at("storage_bytes").is_number()) state.storage_bytes = value->at("storage_bytes").get<double>();
  for (const auto& [key, item] : value->at("values").items()) {
    const auto name = control_text::wide(key);
    if (item.is_boolean()) state.values[name] = item.get<bool>();
    else if (item.is_number_integer()) state.values[name] = item.get<std::int64_t>();
    else if (item.is_string()) state.values[name] = control_text::wide(item.get<std::string>());
  }
  return state;
}
} // namespace msime::settings::win7::plugin_data
