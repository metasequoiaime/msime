#pragma once

#include "DictionaryData.h"

namespace msime::settings::win7::dictionary_data {
inline Json export_all_file(Requests& requests, const Json& options,
    const std::filesystem::path& path, const Requests::Transport& transport = {}) {
  auto contents = control_text::utf8(model::personal_export_header).value_or("");
  uint64_t rows = 0;
  for (const auto kind : model::personal_export_kinds) {
    uint32_t offset = 0;
    for (;;) {
      const auto name = control_text::utf8(kind).value_or("");
      const auto response = requests.call(options, {{"operation", "list"}, {"kind", name}, {"query", ""},
          {"offset", offset}, {"limit", 1000}, {"user_only", true}}, transport);
      const auto value = theme_data::value(response);
      if (!value) return response;
      if (!value->is_object() || !value->contains("entries") || !value->at("entries").is_array() ||
          !value->contains("has_more") || !value->at("has_more").is_boolean() || value->at("entries").size() > 1000)
        return failure("dictionary_read_rejected");
      const auto& entries = value->at("entries");
      for (const auto& entry : entries) {
        if (!entry.is_object() || !entry.contains("kind") || entry.at("kind") != name ||
            !entry.contains("key") || !entry.at("key").is_string() || !entry.contains("value") || !entry.at("value").is_string() ||
            !entry.contains("source") || (entry.at("source") != "user" && entry.at("source") != "bundled") ||
            !entry.contains("weight") || !entry.at("weight").is_number_integer()) return failure("dictionary_read_rejected");
        if (entry.at("source") == "bundled") continue;
        const auto& weight = entry.at("weight");
        if (weight.is_number_unsigned() && weight.get<uint64_t>() > 100000000) return failure("dictionary_read_rejected");
        const auto number = weight.get<int64_t>();
        if (number < 1 || number > 100000000) return failure("dictionary_read_rejected");
        const auto row = control_text::utf8(model::personal_export_row(kind, control_text::wide(entry.at("key").get<std::string>()),
            control_text::wide(entry.at("value").get<std::string>()), number));
        if (!row) return failure("dictionary_encoding");
        if (row->size() >= model::max_export_bytes || contents.size() > model::max_export_bytes - row->size() - 1)
          return failure("dictionary_export_limit");
        contents += *row; contents += '\n'; ++rows;
      }
      if (value->at("has_more") == false) break;
      if (entries.empty() || entries.size() > 1000000 - offset) return failure("dictionary_export_limit");
      offset += static_cast<uint32_t>(entries.size());
    }
  }
  if (!rows) return failure("dictionary_export_empty");
  if (!msime::settings::write_settings_document(path, contents)) return failure("storage");
  return Json{{"ok", true}, {"value", {{"rows", rows}, {"path", path.u8string()}}}};
}
} // namespace msime::settings::win7::dictionary_data
