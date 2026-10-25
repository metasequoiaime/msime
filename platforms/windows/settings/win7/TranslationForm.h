#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "../TranslationServiceSettings.h"
#include <commctrl.h>
#include <array>
#include <functional>

namespace msime::settings::win7 {

class TranslationForm {
public:
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    std::optional<Json> test;
  };

  void create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    const auto add = [&](size_t group, const wchar_t* type, const wchar_t* title, DWORD style, int id, int top, int height) {
      const auto control = row(type, title, style, id, top, height);
      rows_.push_back({control, group, top});
      return control;
    };
    add(0, L"STATIC", L"翻译服务", SS_LEFT, -1, 0, 24);
    selector_ = add(0, WC_COMBOBOXW, L"候选词翻译服务", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL,
        translation_provider_control, 34, 240);
    for (const auto* label : {L"不使用在线服务", L"腾讯云机器翻译", L"小牛翻译（NiuTrans）",
        L"自定义 DeepLX 兼容服务", L"水杉账号（候选词发送到 api.msime.app）"})
      SendMessageW(selector_, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label));
    add(0, L"STATIC", L"不使用在线服务时只显示离线词库查得到的释义。选择在线服务后，当前候选词会发送到该服务。修改凭据后点击保存；测试配置会向服务发送一条合成请求。", SS_LEFT, -1, 86, 96);
    for (size_t index = 0; index < groups_.size(); ++index) {
      auto& group = groups_[index];
      int top = 204;
      const auto definition = definitions[index];
      for (size_t field = 0; field < definition.fields.size(); ++field) {
        const auto& option = definition.fields[field];
        if (!option.key) continue;
        add(index + 1, L"STATIC", option.label, SS_LEFT, -1, top, 24);
        group.fields[field] = add(index + 1, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL | (field == 1 ? ES_PASSWORD : 0),
            definition.first + static_cast<int>(field), top + 32, 30);
        SendMessageW(group.fields[field], EM_SETLIMITTEXT, 4096, 0);
        if (field == 1) SendMessageW(group.fields[field], EM_SETPASSWORDCHAR, L'*', 0);
        top += 82;
      }
      group.reveal = add(index + 1, L"BUTTON", L"显示密钥", WS_TABSTOP | BS_PUSHBUTTON, definition.first + 12, top, 32);
      group.save = add(index + 1, L"BUTTON", L"保存服务配置", WS_TABSTOP | BS_PUSHBUTTON, definition.first + 10, top + 46, 32);
      group.test = add(index + 1, L"BUTTON", L"测试配置", WS_TABSTOP | BS_PUSHBUTTON, definition.first + 11, top + 92, 32);
      group.issue = add(index + 1, L"STATIC", L"", SS_LEFT, definition.first + 13, top + 138, 100);
      group.result = add(index + 1, L"STATIC", L"", SS_LEFT, definition.first + 14, top + 250, 100);
      group.height = top + 366;
    }
  }

  static bool valid(const Json& preferences) {
    if (!preferences.contains("translation_account") || !preferences.at("translation_account").is_boolean()) return false;
    for (const auto& definition : definitions) {
      if (!preferences.contains(definition.key) || !preferences.at(definition.key).is_object()) return false;
      const auto& group = preferences.at(definition.key);
      if (!group.contains("enabled") || !group.at("enabled").is_boolean()) return false;
      for (const auto& field : definition.fields)
        if (field.key && (!group.contains(field.key) || !group.at(field.key).is_string())) return false;
    }
    return true;
  }

  void refresh(const Json& preferences) {
    available_ = preferences.at("candidate_translations").get<bool>();
    TranslationServiceChoice choice;
    choice.account = preferences.at("translation_account").get<bool>();
    choice.tencent = preferences.at("tencent_tmt").at("enabled").get<bool>();
    choice.niutrans = preferences.at("niutrans").at("enabled").get<bool>();
    choice.custom = preferences.at("custom_translation").at("enabled").get<bool>();
    choice.tencent_chosen = tencent_chosen_;
    choice.tencent_secret_id = control_text::wide(preferences.at("tencent_tmt").at("secret_id").get<std::string>());
    choice.tencent_secret_key = control_text::wide(preferences.at("tencent_tmt").at("secret_key").get<std::string>());
    const auto provider = translation_provider(choice);
    selected_ = provider == L"tencent" ? 1 : provider == L"niutrans" ? 2 : provider == L"custom" ? 3 : provider == L"account" ? 4 : 0;
    SendMessageW(selector_, CB_SETCURSEL, selected_, 0);
    for (size_t index = 0; index < groups_.size(); ++index) {
      auto& group = groups_[index];
      Json config = Json::object();
      for (const auto& field : definitions[index].fields) if (field.key) config[field.key] = preferences.at(definitions[index].key).at(field.key);
      // 只比较凭据字段；服务启用位或其他偏好改变时保留各组尚未保存的草稿。
      if (!group.base || *group.base != config) {
        group.base = config;
        for (size_t field = 0; field < group.fields.size(); ++field)
          if (group.fields[field]) SetWindowTextW(group.fields[field], control_text::wide(config.at(definitions[index].fields[field].key).get<std::string>()).c_str());
      }
    }
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    EnableWindow(selector_, enabled && available_);
    for (size_t index = 0; index < groups_.size(); ++index) {
      auto& group = groups_[index];
      const bool active = enabled && available_ && selected_ == index + 1;
      for (const auto field : group.fields) if (field) EnableWindow(field, active);
      EnableWindow(group.reveal, active);
      update(index);
    }
  }

  bool handles(int id) const { return id >= translation_provider_control && id < translation_custom_first + 20; }
  std::optional<int> position(HWND control) const {
    for (const auto& row : rows_)
      if (row.control == control) return row.group == 0 || row.group == selected_ ? std::optional<int>(row.top) : std::nullopt;
    return std::nullopt;
  }
  int height() const { return selected_ >= 1 && selected_ <= groups_.size() ? groups_[selected_ - 1].height : 198; }

  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!handles(id)) return {};
    Command result{true, {}, std::nullopt};
    if (!enabled_ || !available_) return result;
    if (id == translation_provider_control && notification == CBN_SELCHANGE) {
      const auto selected = SendMessageW(selector_, CB_GETCURSEL, 0, 0);
      if (selected < 0 || selected > 4) return result;
      tencent_chosen_ = selected == 1;
      result.changes = {{{"tencent_tmt", "enabled"}, selected == 1}, {{"niutrans", "enabled"}, selected == 2},
          {{"custom_translation", "enabled"}, selected == 3}, {{"translation_account"}, selected == 4}};
      return result;
    }
    for (size_t index = 0; index < groups_.size(); ++index) {
      const auto& definition = definitions[index];
      auto& group = groups_[index];
      if (selected_ != index + 1 || id < definition.first || id >= definition.first + 20) continue;
      if (notification == EN_CHANGE && id < definition.first + 3) { update(index); return result; }
      if (notification != BN_CLICKED) return result;
      if (id == definition.first + 12) {
        group.revealed = !group.revealed;
        SendMessageW(group.fields[1], EM_SETPASSWORDCHAR, group.revealed ? 0 : L'*', 0);
        InvalidateRect(group.fields[1], nullptr, TRUE);
        SetWindowTextW(group.reveal, group.revealed ? L"隐藏密钥" : L"显示密钥");
      } else if (id == definition.first + 10 && IsWindowEnabled(group.save)) {
        if (const auto config = configuration(index))
          for (const auto& field : definition.fields) if (field.key) result.changes.push_back({{definition.key, field.key}, config->at(field.key)});
      } else if (id == definition.first + 11 && IsWindowEnabled(group.test)) {
        if (const auto config = configuration(index)) result.test = Json{{"service", definition.service}, {"config", *config}};
      }
    }
    return result;
  }

  void tested(const Json& request, const Json& response) {
    for (size_t index = 0; index < groups_.size(); ++index) {
      if (request.value("service", std::string{}) != definitions[index].service) continue;
      auto& group = groups_[index];
      group.test_config = request.at("config");
      group.test_message = L"无法连接服务，请确认配置和网络后重试。";
      if (response.is_object() && response.value("ok", false) && response.contains("value") && response.at("value").is_object())
        group.test_message = control_text::wide(response.at("value").value("message", std::string{}));
      update(index);
    }
  }

private:
  struct Field { const char* key; const wchar_t* label; };
  struct Definition { const char* key; const char* service; int first; std::array<Field, 3> fields; };
  static constexpr std::array<Definition, 3> definitions{{
      {"tencent_tmt", "translation.tencent", translation_tencent_first, {{{"secret_id", L"腾讯云 SecretId"}, {"secret_key", L"腾讯云 SecretKey"}, {"region", L"腾讯云地域"}}}},
      {"niutrans", "translation.niutrans", translation_niutrans_first, {{{"app_id", L"NiuTrans App ID"}, {"apikey", L"NiuTrans API Key"}, {nullptr, L""}}}},
      {"custom_translation", "translation.custom", translation_custom_first, {{{"endpoint", L"自定义翻译 Endpoint"}, {"api_key", L"自定义翻译 API Key"}, {nullptr, L""}}}},
  }};
  struct Group {
    std::array<HWND, 3> fields{};
    HWND reveal = nullptr, save = nullptr, test = nullptr, issue = nullptr, result = nullptr;
    std::optional<Json> base, test_config;
    std::wstring test_message;
    bool revealed = false;
    int height = 0;
  };
  struct Row { HWND control; size_t group; int top; };

  std::optional<Json> configuration(size_t index) const {
    Json config = Json::object();
    for (size_t field = 0; field < groups_[index].fields.size(); ++field) {
      if (!groups_[index].fields[field]) continue;
      const auto value = control_text::read(groups_[index].fields[field], 4096);
      if (!value) return std::nullopt;
      config[definitions[index].fields[field].key] = *value;
    }
    return config;
  }

  void update(size_t index) {
    auto& group = groups_[index];
    const auto config = configuration(index);
    bool saveable = config.has_value(), testable = false;
    std::wstring issue;
    if (config) {
      const auto text = [&](const char* key) { return control_text::wide(config->at(key).get<std::string>()); };
      if (index == 0) {
        issue = tencent_credential_issue(text("secret_id"), text("secret_key"), text("region"));
        saveable = tencent_secret_id_saveable(text("secret_id")) && tencent_secret_key_saveable(text("secret_key")) && tencent_region_saveable(text("region"));
        testable = !issue_credential_test_disabled(true, issue);
        if (issue.empty() && tencent_missing_credentials_warning(false, text("secret_id"), text("secret_key")))
          issue = L"未填写腾讯云凭据，填好之前不会请求在线翻译。";
      } else if (index == 1) {
        saveable = niutrans_credential_saveable(text("app_id")) && niutrans_credential_saveable(text("apikey"));
        testable = !niutrans_credential_test_disabled(true, text("app_id"), text("apikey"));
        if (!saveable) issue = L"请填写有效的 App ID 和 API Key，不能使用示例值、空白或控制字符。";
      } else {
        const auto endpoint = config->at("endpoint").get<std::string>();
        auto owned = msime::host_api::own_string(msime_client_translation_endpoint_issue(
            reinterpret_cast<const uint8_t*>(endpoint.data()), endpoint.size()));
        const auto response = owned ? Json::parse(owned.get(), nullptr, false) : Json();
        saveable = false;
        if (response.is_object() && response.value("ok", false) && response.contains("value")) {
          const auto& value = response.at("value");
          issue = control_text::wide(value.value("issue", std::string{}));
          saveable = value.value("accepted", false) && custom_translation_api_key_saveable(text("api_key"));
          testable = !issue_credential_test_disabled(true, issue);
        }
      }
    }
    if (!saveable && issue.empty()) issue = L"请检查服务配置的长度与字符。";
    SetWindowTextW(group.issue, issue.c_str());
    const bool active = enabled_ && available_ && selected_ == index + 1;
    EnableWindow(group.save, active && saveable && group.base && config && *group.base != *config);
    EnableWindow(group.test, active && saveable && testable);
    SetWindowTextW(group.result, group.test_config && config && *group.test_config == *config ? group.test_message.c_str() : L"");
  }

  HWND selector_ = nullptr;
  std::array<Group, 3> groups_{};
  std::vector<Row> rows_;
  size_t selected_ = 0;
  bool enabled_ = false, available_ = false, tencent_chosen_ = false;
};

} // namespace msime::settings::win7
