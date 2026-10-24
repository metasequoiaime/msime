#pragma once

#include "ControlText.h"
#include "ThemeData.h"
#include "../LexiconImportModel.h"
#include "../SettingsDocumentFile.h"
#include <objbase.h>
#include <wincrypt.h>
#include <limits>
#include <functional>

namespace msime::settings::win7::dictionary_data {
namespace model = msime::settings::lexicon_import;

inline Json failure(const char* code) { return Json{{"ok", false}, {"error", code}}; }
inline std::string request_id() {
  GUID guid{}; wchar_t value[40]{};
  if (FAILED(CoCreateGuid(&guid)) || !StringFromGUID2(guid, value, 40)) return {};
  return control_text::utf8(std::wstring_view(value + 1, 36)).value_or("");
}

// 同一内容的模糊失败重试沿用回执编号；文件字节或操作变化时才产生新编号。
class Requests {
public:
  using Transport = std::function<std::string(const std::string&)>;
  Json call(const Json& options, Json action, const Transport& transport = {}) {
    const auto operation = action.value("operation", std::string{});
    if (operation == "edit" || operation == "import") {
      const Json signature{{"options", options}, {"action", action}};
      if (signature != previous_) { previous_ = signature; id_ = request_id(); }
      if (id_.empty()) return failure("storage");
      action["request_id"] = id_;
    }
    const Json request{{"options", options}, {"action", action}};
    auto response = transport ? Json::parse(transport(request.dump()), nullptr, false) : theme_data::call(msime_client_settings_dictionary, request);
    if ((operation == "edit" || operation == "import") && theme_data::value(response)) previous_ = Json();
    return response;
  }
private:
  Json previous_;
  std::string id_;
};

inline std::optional<std::string> gb18030(std::string_view bytes) {
  if (bytes.empty()) return std::string{};
  if (bytes.size() > static_cast<size_t>((std::numeric_limits<int>::max)())) return std::nullopt;
  const int size = static_cast<int>(bytes.size());
  const int length = MultiByteToWideChar(54936, 0, bytes.data(), size, nullptr, 0);
  if (!length) return std::nullopt;
  std::wstring text(static_cast<size_t>(length), L'\0');
  if (MultiByteToWideChar(54936, 0, bytes.data(), size, text.data(), length) != length) return std::nullopt;
  return control_text::utf8(text);
}

inline Json import_file(Requests& requests, const Json& options, Json action,
    const std::filesystem::path& path, const Requests::Transport& transport = {}) {
  if (!path.is_absolute()) return failure("dictionary_read_rejected");
  const auto chosen = action.at("format").get<std::string>();
  auto format = control_text::utf8(model::format_for_file(path.filename().wstring(), control_text::wide(chosen))).value_or(chosen);
  const auto kind = action.at("kind").get<std::string>();
  const auto bytes = msime::settings::read_settings_document(path, static_cast<size_t>(model::max_text_file_bytes));
  if (!bytes) return failure("dictionary_too_large");
  if (model::is_scel_bytes(*bytes)) format = "scel";
  std::optional<std::string> text;
  if (format == "scel") {
    if (kind != "pinyin") return failure("dictionary_invalid_entry");
    if (bytes->size() > model::max_scel_file_bytes) return failure("dictionary_too_large");
    DWORD length = 0;
    const DWORD flags = CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF;
    const auto data = reinterpret_cast<const BYTE*>(bytes->data());
    if (!CryptBinaryToStringA(data, static_cast<DWORD>(bytes->size()), flags, nullptr, &length)) return failure("storage");
    std::string encoded(length, '\0');
    if (!CryptBinaryToStringA(data, static_cast<DWORD>(bytes->size()), flags, encoded.data(), &length)) return failure("storage");
    encoded.resize(length);
    const auto converted = theme_data::call(msime_client_dictionary_scel_text, {{"bytes_base64", encoded}});
    const auto value = theme_data::value(converted);
    if (!value) return converted;
    if (!value->is_object() || !value->contains("text") || !value->at("text").is_string()) return failure("dictionary_read_rejected");
    text = value->at("text").get<std::string>(); format = "standard";
  } else text = model::decode_text(*bytes, gb18030);
  if (!text) return failure("dictionary_encoding");
  action["operation"] = "import"; action["format"] = format; action["text"] = *text;
  return requests.call(options, std::move(action), transport);
}

inline Json export_file(Requests& requests, const Json& options, const Json& action,
    const std::filesystem::path& path, const Requests::Transport& transport = {}) {
  const auto kind = action.at("kind").get<std::string>();
  const auto format = action.at("format").get<std::string>();
  std::string contents;
  size_t offset = 0;
  for (;;) {
    const auto response = requests.call(options, {{"operation", "export"}, {"kind", kind}, {"format", format}, {"offset", offset}, {"limit", 1000}}, transport);
    const auto value = theme_data::value(response);
    if (!value) return response;
    if (!value->is_object() || !value->contains("text") || !value->at("text").is_string() || !value->contains("has_more") || !value->at("has_more").is_boolean()) return failure("dictionary_read_rejected");
    const auto page = value->at("text").get<std::string>();
    if (page.size() > model::max_export_bytes || contents.size() > model::max_export_bytes - page.size()) return failure("dictionary_export_limit");
    contents += page;
    if (value->at("has_more") == false) break;
    const auto rows = model::export_page_rows(page);
    if (!rows || rows > 1000 || offset > 1000000 - rows) return failure("dictionary_export_limit");
    offset += rows;
  }
  const auto payload = model::export_payload(control_text::wide(kind), control_text::wide(format), contents);
  if (payload.too_large) return failure("dictionary_export_limit");
  if (!payload.rows) return failure("dictionary_export_empty");
  if (!msime::settings::write_settings_document(path, payload.body)) return failure("storage");
  return Json{{"ok", true}, {"value", {{"rows", payload.rows}, {"path", path.u8string()}}}};
}
} // namespace msime::settings::win7::dictionary_data
