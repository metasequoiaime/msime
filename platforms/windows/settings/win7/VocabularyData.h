#pragma once

#include "DictionaryData.h"
#include <cstdio>

namespace msime::settings::win7::vocabulary_data {
inline Json call(const std::filesystem::path& directory, const Json& options, Json action,
    const std::filesystem::path& file = {}) {
  if (!options.is_object() || !options.contains("resources") || !options.at("resources").is_string() ||
      !std::filesystem::u8path(options.at("resources").get<std::string>()).is_absolute()) return dictionary_data::failure("vocabulary_resources");
  if (action.value("operation", std::string{}) == "import_file") {
    const auto bytes = msime::settings::read_settings_document(file, static_cast<size_t>(dictionary_data::model::max_wordbook_file_bytes));
    if (!bytes) return dictionary_data::failure("vocabulary_import_file");
    const auto text = dictionary_data::model::decode_text(*bytes, dictionary_data::gb18030);
    const auto name = control_text::utf8(dictionary_data::model::wordbook_name(file.filename().wstring()));
    if (!text || !name) return dictionary_data::failure("vocabulary_encoding");
    action = Json{{"operation", "import"}, {"name", *name}, {"text", *text}};
  }
  SYSTEMTIME now{}; GetLocalTime(&now);
  char day[16]{};
  std::snprintf(day, sizeof(day), "%04u-%02u-%02u", static_cast<unsigned>(now.wYear), static_cast<unsigned>(now.wMonth), static_cast<unsigned>(now.wDay));
  return theme_data::call(msime_client_vocabulary_review, {{"directory", directory.u8string()}, {"resources", options.at("resources")},
      {"plugins", (directory / L"plugins").u8string()}, {"day", day}, {"action", std::move(action)}});
}
} // namespace msime::settings::win7::vocabulary_data
