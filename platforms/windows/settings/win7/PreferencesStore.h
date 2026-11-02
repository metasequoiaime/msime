#pragma once

#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <nlohmann/json.hpp>
#include <cstdint>
#include <algorithm>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace msime::settings::win7 {

using Json = nlohmann::json;

struct Snapshot {
  Json document;
  uint64_t revision() const { return document.at("revision").get<uint64_t>(); }
  const Json& preferences() const { return document.at("preferences"); }
};

enum class Outcome { loaded, saved, conflict, storage, invalid_response, invalid_change };

struct Result {
  Outcome outcome = Outcome::storage;
  std::optional<Snapshot> snapshot;
  std::string error{};
};

struct Change {
  std::vector<std::string> path;
  Json value;
  bool remove = false;
};

// 只转换 C ABI 的文档与所有权；默认值、版本过滤、校验和持久化仍归共享 Rust。
class PreferencesStore {
public:
  explicit PreferencesStore(std::string directory) : directory_(std::move(directory)) {}

  Result load() const {
    return response(msime_client_load_preferences(bytes(directory_), directory_.size()), Outcome::loaded);
  }

  Result save(const Snapshot& previous, const std::vector<Change>& changes) const {
    if (!valid(previous.document) || changes.empty()) return {Outcome::invalid_change, std::nullopt};
    Json next = previous.document;
    for (const auto& change : changes) {
      if (change.path.empty()) return {Outcome::invalid_change, std::nullopt};
      Json* parent = &next.at("preferences");
      // 屏幕键盘的显式恢复默认只删除这五项，由共享反序列化恢复当前默认；不能泛化为任意字段删除。
      if (change.remove) {
        if (change.path.size() != 1 || std::find(reset_keyboard_fields.begin(),reset_keyboard_fields.end(),change.path.front()) == reset_keyboard_fields.end())
          return {Outcome::invalid_change, std::nullopt};
        parent->erase(change.path.front()); continue;
      }
      // 仅放行共享格式明确省略的叶字段及其类型；未知路径和缺少中间对象仍不能补造。
      const bool optional_boolean = change.value.is_boolean() &&
          (change.path == std::vector<std::string>{"navigation", "mouse_wheel"} ||
           change.path == std::vector<std::string>{"backspace_reverts_selection"} ||
           change.path == std::vector<std::string>{"chinese_english_spacing"} ||
           change.path == std::vector<std::string>{"caret_edit_suffix"} ||
           change.path == std::vector<std::string>{"next_word_prediction"} ||
           change.path == std::vector<std::string>{"shuangpin_keymap_hint"} ||
           change.path == std::vector<std::string>{"mixed_input", "enabled"} ||
           change.path == std::vector<std::string>{"mixed_input", "symbols"} ||
           change.path == std::vector<std::string>{"mixed_input", "japanese"} ||
           change.path == std::vector<std::string>{"local_modes", "chaizi"} ||
           change.path == std::vector<std::string>{"local_modes", "symbol"} ||
           change.path == std::vector<std::string>{"fuzzy_pinyin", "seeded"});
      const bool optional_style = change.value.is_string() &&
          (change.path == std::vector<std::string>{"quote_style"} ||
           change.path == std::vector<std::string>{"bracket_style"} ||
           change.path == std::vector<std::string>{"last_chinese_scheme"});
      const bool optional_font = change.value.is_string() && change.path == std::vector<std::string>{"candidate_english_font"};
      const bool optional_radius = (change.value.is_number_integer() || change.value.is_null()) &&
          change.path == std::vector<std::string>{"candidate_corner_radius"};
      const bool optional_count = change.value.is_number_integer() &&
          change.path == std::vector<std::string>{"mixed_input", "first_page_count"};
      const bool optional_language = (change.value.is_string() || change.value.is_null()) &&
          change.path == std::vector<std::string>{"translation_secondary_language"};
      const bool optional_object = change.value.is_object() &&
          (change.path == std::vector<std::string>{"app_input_mode_rules"} ||
           change.path == std::vector<std::string>{"plugins"} ||
           change.path == std::vector<std::string>{"cloud_clipboard_relay"} ||
           change.path == std::vector<std::string>{"cloud_clipboard_webdav"} ||
           change.path == std::vector<std::string>{"dictionary_sync_webdav"} ||
           change.path == std::vector<std::string>{"game_compatibility"} ||
           change.path == std::vector<std::string>{"multilingual_input"} ||
           change.path == std::vector<std::string>{"wubi_candidate_selection"});
      // 插件默认组会被共享序列化省略；只允许页面已知字段补齐其路径。
      const bool optional_plugin = plugin_field(change);
      for (size_t index = 0; index < change.path.size(); ++index) {
        const auto& segment = change.path[index];
        if (!parent->is_object()) return {Outcome::invalid_change, std::nullopt};
        auto child = parent->find(segment);
        if (child == parent->end()) {
          if (!optional_plugin && ((!optional_boolean && !optional_style && !optional_font && !optional_radius && !optional_count && !optional_language && !optional_object) || index + 1 != change.path.size()))
            return {Outcome::invalid_change, std::nullopt};
          child = parent->emplace(segment, index + 1 == change.path.size() ? change.value : Json::object()).first;
        }
        parent = &*child;
      }
      *parent = change.value;
    }
    const auto body = next.dump();
    return response(msime_client_save_preferences(bytes(directory_), directory_.size(), previous.revision(),
        bytes(body), body.size()), Outcome::saved);
  }

  std::optional<std::string> export_document() const {
    auto owned = msime::host_api::own_string(msime_client_export_settings(bytes(directory_), directory_.size()));
    if (!owned) return std::nullopt;
    const auto body = Json::parse(owned.get(), nullptr, false);
    if (!body.is_object()) return std::nullopt;
    const auto ok = body.find("ok");
    const auto value = body.find("value");
    if (ok == body.end() || !ok->is_boolean() || !ok->get<bool>() ||
        value == body.end() || !value->is_string()) return std::nullopt;
    return value->get<std::string>();
  }

  Result import_document(const Snapshot& previous, const std::string& document) const {
    if (!valid(previous.document)) return {Outcome::invalid_change, std::nullopt, {}};
    return response(msime_client_import_settings(bytes(directory_), directory_.size(), previous.revision(),
        bytes(document), document.size()), Outcome::saved);
  }

private:
  inline static const std::vector<std::string> reset_keyboard_fields{"touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment", "touch_voice_shortcut", "touch_toolbar"};
  static bool plugin_field(const Change& change) {
    if (change.path.size() < 2 || change.path.front() != "plugins") return false;
    const auto& path = change.path;
    const auto& value = change.value;
    if (path.size() == 2) {
      const auto& field = path[1];
      if (field == "combo_counter" || field == "combo_tier_sound") return value.is_boolean();
      if (field == "effect_intensity") return value.is_number_integer();
      if (field == "effect_style" || field == "effect_pack" || field == "helpcode_pack_quanpin" || field == "helpcode_pack_shuangpin") return value.is_string();
      if (field == "command_tables" || field == "phrase_tables") return value.is_array() && std::all_of(value.begin(), value.end(), [](const Json& item) { return item.is_string(); });
      return false;
    }
    if (path.size() != 3) return false;
    const auto& group = path[1]; const auto& field = path[2];
    if (field == "enabled" && (group == "key_sound" || group == "commit_sound" || group == "music" || group == "achievements")) return value.is_boolean();
    if (field == "pack" && (group == "key_sound" || group == "melody" || group == "music")) return value.is_string();
    if (group == "key_sound" && field == "mode") return value.is_string();
    return field == "volume" && (group == "key_sound" || group == "music") && value.is_number_integer();
  }
  static const uint8_t* bytes(const std::string& value) {
    return reinterpret_cast<const uint8_t*>(value.data());
  }

  static bool valid(const Json& value) {
    if (!value.is_object()) return false;
    const auto version = value.find("format_version");
    const auto revision = value.find("revision");
    const auto preferences = value.find("preferences");
    return version != value.end() && version->is_number_unsigned() && *version == 1 &&
        revision != value.end() && revision->is_number_unsigned() &&
        preferences != value.end() && preferences->is_object();
  }

  static Result response(char* raw, Outcome success) {
    auto owned = msime::host_api::own_string(raw);
    if (!owned) return {Outcome::invalid_response, std::nullopt};
    const auto body = Json::parse(owned.get(), nullptr, false);
    if (!body.is_object()) return {Outcome::invalid_response, std::nullopt};
    const auto ok = body.find("ok");
    if (ok == body.end() || !ok->is_boolean()) return {Outcome::invalid_response, std::nullopt};
    if (!ok->get<bool>()) {
      const auto error = body.find("error");
      // 普通保存返回错误描述，设置文件导入返回稳定错误码，两者均不能覆盖并发修改。
      const auto code = error != body.end() && error->is_string() ? error->get<std::string>() : std::string{};
      return {code == "preferences changed; reload before saving" || code == "settings_conflict" ?
          Outcome::conflict : Outcome::storage, std::nullopt, code};
    }
    const auto value = body.find("value");
    if (value == body.end() || !valid(*value)) return {Outcome::invalid_response, std::nullopt};
    return {success, Snapshot{*value}};
  }

  std::string directory_;
};

} // namespace msime::settings::win7
