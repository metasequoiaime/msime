#pragma once

#include "CloudDictionaryForm.h"
#include "DictionaryData.h"

namespace msime::settings::win7 {
struct CloudDictionaryServices {
  using Transport = std::function<std::string(const std::string&)>;
  Transport account, dictionary;
};

// 查询的身份绑定随结果返回。写入前重新核对，传输期间换号的结果也不能重新启用旧控件。
inline Json run_cloud_dictionary(Json& envelope, const Json& account_request, const Json& options,
    const std::filesystem::path& scratch, const std::filesystem::path& file, const CloudDictionaryServices& services) {
  const auto failure = [](const char* code) { return Json{{"response", {{"ok", false}, {"error", code}}}}; };
  const auto account_id = [&]() -> std::string {
    const auto response = services.account ? Json::parse(services.account(account_request.dump()), nullptr, false) : theme_data::call(msime_client_account, account_request);
    const auto value = theme_data::value(response);
    if (!value || !value->is_object() || !value->contains("user") || !value->at("user").is_object()) return {};
    const auto user = value->at("user").find("id");
    return user != value->at("user").end() && user->is_string() ? user->get<std::string>() : std::string{};
  };
  const auto binding = account_id(); if (binding.empty()) return failure("account_unauthorized");
  auto& action = envelope.at("action"); const auto operation = action.at("operation").get<std::string>();
  const bool query = operation == "list" || operation == "catalog" || operation == "candidates" || operation == "fixed_positions";
  if ((envelope.contains("binding") && envelope.at("binding") != binding) || (!envelope.contains("binding") && !query)) return failure("dictionary_account_changed");
  if (!file.empty()) {
    if (operation == "import") {
      const auto bytes = msime::settings::read_settings_document(file, 65536);
      const auto text = bytes ? dictionary_data::model::decode_text(*bytes, dictionary_data::gb18030) : std::nullopt;
      if (!text || text->empty() || text->find('\0') != std::string::npos) return failure("dictionary_file_invalid");
      action["text"] = *text;
    } else action["path"] = file.u8string();
  }
  // 文件选择可能经过很久，检查输入文件后再次确认账号，不能以初次读出的身份替代当前会话。
  if (account_id() != binding) return failure("dictionary_account_changed");
  auto request = account_request; request.erase("client_agent"); request["action"] = action;
  request["options"] = options; request["snapshot_directory"] = scratch.u8string();
  auto response = services.dictionary ? Json::parse(services.dictionary(request.dump()), nullptr, false) : theme_data::call(msime_client_cloud_dictionary, request);
  if (account_id() != binding) return failure("dictionary_account_changed");
  return {{"binding", binding}, {"response", std::move(response)}};
}
} // namespace msime::settings::win7
