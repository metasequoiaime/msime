#pragma once

#include "DictionaryData.h"
#include "../CommunityPageModel.h"
#include <algorithm>
#include <vector>

namespace msime::settings::win7::community_resource_data {
inline std::optional<std::vector<community::SharedWord>> entries(const Json& values) {
  if (!values.is_array() || values.empty() || values.size() > community::shared_word_limit) return std::nullopt;
  std::vector<community::SharedWord> result;
  for (const auto& value : values) {
    if (!value.is_object()) return std::nullopt;
    for (const auto* field : {"kind", "code", "word"})
      if (!value.contains(field) || !value.at(field).is_string()) return std::nullopt;
    if (!value.contains("weight") || !value.at("weight").is_number_integer()) return std::nullopt;
    const auto kind = control_text::wide(value.at("kind").get<std::string>());
    if (std::none_of(community::shared_word_kinds.begin(), community::shared_word_kinds.end(),
        [&](const auto& option) { return option.value == kind; })) return std::nullopt;
    const auto code = control_text::wide(value.at("code").get<std::string>());
    const auto word = control_text::wide(value.at("word").get<std::string>());
    // 标准格式以制表和换行分隔；不能让一条社区词条变成多行导入。
    for (const auto& text : {code, word})
      if (std::any_of(text.begin(), text.end(), [](wchar_t character) { return character < L' ' || character == 127; })) return std::nullopt;
    const auto entry = community::shared_word(result, kind, code, word,
        std::to_wstring(value.at("weight").get<int64_t>()));
    if (!entry) return std::nullopt;
    result.push_back(*entry);
  }
  return result;
}

inline Json import_local(const Json& options, const Json& action, const dictionary_data::Requests::Transport& transport) {
  const auto words = entries(action.at("entries"));
  const auto id = action.value("request_id", std::string{});
  if (!words || id.empty() || id.size() > 80) return dictionary_data::failure("dictionary_invalid_entry");
  uint64_t applied = 0, failed = 0;
  for (const auto& group : community::local_import_groups(*words)) {
    const Json request{{"options", options}, {"action", {{"operation", "import"},
      {"kind", control_text::utf8(group.kind).value()}, {"format", "standard"},
      {"text", control_text::utf8(group.text).value()},
      {"request_id", id + "-" + control_text::utf8(group.source).value()}}}};
    auto response = transport ? Json::parse(transport(request.dump()), nullptr, false) :
        theme_data::call(msime_client_settings_dictionary, request);
    const auto value = theme_data::value(response);
    if (!value || !value->is_object() || !value->contains("applied") || !value->at("applied").is_number_unsigned() ||
        !value->contains("failed") || !value->at("failed").is_number_unsigned()) {
      auto error = dictionary_data::failure(response.is_object() && response.contains("error") && response.at("error").is_string() ?
          response.at("error").get_ref<const std::string&>().c_str() : "dictionary_read_rejected");
      error["partial"] = {{"applied", applied}, {"failed", failed}}; return error;
    }
    applied += value->at("applied").get<uint64_t>(); failed += value->at("failed").get<uint64_t>();
    // 一组存在拒绝就停止；明确显示已应用部分，重试仍沿用每组的原回执编号。
    if (failed) return Json{{"ok", false}, {"error", "dictionary_invalid_entry"}, {"partial", {{"applied", applied}, {"failed", failed}}}};
  }
  return Json{{"ok", true}, {"value", {{"applied", applied}, {"failed", failed}}}};
}
} // namespace msime::settings::win7::community_resource_data
