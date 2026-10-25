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
            (command.contains("template") && !command.at("template").is_string())) { valid = false; break; }
        pack.commands.push_back({control_text::wide(command.at("trigger").get<std::string>()), control_text::wide(command.at("title").get<std::string>()), control_text::wide(command.value("template", std::string{}))});
      }
    }
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
} // namespace msime::settings::win7::plugin_data
