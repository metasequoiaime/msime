#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "../AiSettings.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {

// AI 页面只提交明确字段；服务目录、地址校验、网络请求和凭据测试仍由共享 C ABI 负责。
class AiForm {
public:
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    std::optional<Json> request;
    bool models = false;
  };

  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height);
      if (id >= ai_enabled) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return control;
    };
    add(ai_enabled, L"BUTTON", L"启用 AI 辅助，为拼音联想提供共享配置", WS_TABSTOP | BS_AUTOCHECKBOX);
    add(-1, L"STATIC", L"服务提供商", SS_LEFT);
    const auto provider = add(ai_provider, WC_COMBOBOXW, L"AI 服务提供商", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 260);
    for (const auto& entry : ai::providers())
      SendMessageW(provider, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(entry.title.data()));
    add(-1, L"STATIC", L"接口地址", SS_LEFT);
    add(ai_endpoint, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(ai_endpoint), EM_LIMITTEXT, 65536, 0);
    add(ai_endpoint_hint, L"STATIC", L"", SS_LEFT, 56);
    add(-1, L"STATIC", L"API Token（按接口地址保存）", SS_LEFT);
    add(ai_token, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL | ES_PASSWORD);
    SendMessageW(get(ai_token), EM_LIMITTEXT, 65536, 0);
    SendMessageW(get(ai_token), EM_SETPASSWORDCHAR, L'*', 0);
    add(ai_token_reveal, L"BUTTON", L"显示密钥", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(ai_token_save, L"BUTTON", L"保存当前地址的密钥", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", L"模型（可输入自定义模型）", SS_LEFT);
    add(ai_model, WC_COMBOBOXW, L"AI 模型", WS_TABSTOP | CBS_DROPDOWN | CBS_AUTOHSCROLL | WS_VSCROLL, 260);
    SendMessageW(get(ai_model), CB_LIMITTEXT, 256, 0);
    add(ai_service_save, L"BUTTON", L"保存接口地址与模型", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(ai_models_fetch, L"BUTTON", L"获取模型列表", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(ai_models_status, L"STATIC", L"", SS_LEFT, 48);
    add(-1, L"STATIC", L"自定义提示词（留空使用内置提示词）", SS_LEFT);
    add(ai_prompt, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL, 132);
    SendMessageW(get(ai_prompt), EM_LIMITTEXT, 16384, 0);
    add(ai_prompt_save, L"BUTTON", L"保存自定义提示词", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(ai_test, L"BUTTON", L"测试 AI 辅助配置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(ai_test_result, L"STATIC", L"", SS_LEFT, 72);
    add(-1, L"STATIC", L"获取模型列表会连接当前地址；测试配置会发送一条合成请求。结果绑定当前表单，编辑配置后会清除旧结果。", SS_LEFT, 66);
    return top;
  }

  static bool valid(const Json& preferences) {
    const auto it = preferences.find("ai_assistant");
    if (it == preferences.end() || !it->is_object()) return false;
    const auto& config = *it;
    if (!config.contains("enabled") || !config.at("enabled").is_boolean()) return false;
    for (const auto* key : {"provider", "endpoint", "model", "token", "prompt_id", "prompt_custom_1", "prompt_custom_2", "prompt_custom_3"})
      if (!config.contains(key) || !config.at(key).is_string()) return false;
    for (const auto* key : {"tokens", "endpoints", "models"}) {
      if (!config.contains(key) || !config.at(key).is_object()) return false;
      for (const auto& entry : config.at(key)) if (!entry.is_string()) return false;
    }
    return true;
  }

  void refresh(const Json& preferences) {
    updating_ = true;
    saved_ = preferences.at("ai_assistant");
    provider_ = saved_.at("provider").get<std::string>();
    SendMessageW(get(ai_enabled), BM_SETCHECK, saved_.at("enabled").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    int provider_index = -1;
    for (size_t index = 0; index < ai::providers().size(); ++index)
      if (narrow(ai::providers()[index].id) == provider_) provider_index = static_cast<int>(index);
    SendMessageW(get(ai_provider), CB_SETCURSEL, provider_index, 0);
    SetWindowTextW(get(ai_endpoint), wide(saved_.at("endpoint").get<std::string>()).c_str());
    update_origin();
    fill_models(saved_.at("model").get<std::string>());
    prompt_key_ = "prompt_" + narrow(ai::prompt_slot(wide(saved_.at("prompt_id").get<std::string>())).id);
    SetWindowTextW(get(ai_prompt), wide(saved_.at(prompt_key_).get<std::string>()).c_str());
    SetWindowTextW(get(ai_models_status), L"");
    SetWindowTextW(get(ai_test_result), L"");
    tested_.reset();
    updating_ = false;
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& item : controls_) EnableWindow(item.second, enabled);
    update();
  }

  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (id < ai_enabled || id > ai_test_result) return {};
    Command result{true, {}, std::nullopt, false};
    if (updating_ || !enabled_ || saved_.empty()) return result;
    if (id == ai_enabled && notification == BN_CLICKED) {
      result.changes.push_back({{"ai_assistant", "enabled"}, SendMessageW(get(ai_enabled), BM_GETCHECK, 0, 0) == BST_CHECKED});
      return result;
    }
    if (id == ai_provider && notification == CBN_SELCHANGE) {
      const auto selected = SendMessageW(get(ai_provider), CB_GETCURSEL, 0, 0);
      if (selected < 0 || static_cast<size_t>(selected) >= ai::providers().size()) return result;
      const auto next = ai::providers()[static_cast<size_t>(selected)];
      auto endpoints = saved_.at("endpoints");
      auto models = saved_.at("models");
      const auto old_endpoint = saved_.at("endpoint").get<std::string>();
      const auto old_model = saved_.at("model").get<std::string>();
      const auto old = ai::ProviderFields{wide(provider_), wide(old_endpoint), wide(old_model)};
      std::map<std::wstring, std::wstring> endpoint_slots, model_slots;
      for (const auto& entry : endpoints.items()) endpoint_slots[wide(entry.key())] = wide(entry.value().get<std::string>());
      for (const auto& entry : models.items()) model_slots[wide(entry.key())] = wide(entry.value().get<std::string>());
      const auto updated = ai::provider_update(next.id, old, endpoint_slots, model_slots);
      endpoints = Json::object(); models = Json::object();
      for (const auto& entry : endpoint_slots) endpoints[narrow(entry.first)] = narrow(entry.second);
      for (const auto& entry : model_slots) models[narrow(entry.first)] = narrow(entry.second);
      result.changes = {{{"ai_assistant", "provider"}, narrow(updated.provider)},
          {{"ai_assistant", "endpoint"}, narrow(updated.endpoint)}, {{"ai_assistant", "model"}, narrow(updated.model)},
          {{"ai_assistant", "endpoints"}, endpoints}, {{"ai_assistant", "models"}, models}};
      pending_ = Pending::service;
      return result;
    }
    if (id == ai_endpoint && notification == EN_CHANGE) {
      update_origin(); clear_result(); return result;
    }
    if (id == ai_model && (notification == CBN_EDITCHANGE || notification == CBN_SELCHANGE)) {
      clear_result(); return result;
    }
    if (id == ai_token && notification == EN_CHANGE) { clear_result(); update(); return result; }
    if (id == ai_token_reveal && notification == BN_CLICKED) {
      revealed_ = !revealed_;
      SendMessageW(get(ai_token), EM_SETPASSWORDCHAR, revealed_ ? 0 : L'*', 0);
      SetWindowTextW(get(ai_token_reveal), revealed_ ? L"隐藏密钥" : L"显示密钥");
      InvalidateRect(get(ai_token), nullptr, TRUE); return result;
    }
    if (notification != BN_CLICKED || !IsWindowEnabled(get(id))) return result;
    if (id == ai_token_save) {
      if (!origin_) return result;
      const auto value = control_text::read(get(ai_token), 65536); if (!value) return result;
      auto tokens = saved_.at("tokens");
      if (value->empty()) tokens.erase(*origin_); else tokens[*origin_] = *value;
      result.changes = {{{"ai_assistant", "tokens"}, tokens}, {{"ai_assistant", "token"}, ""}};
      pending_ = Pending::token;
    } else if (id == ai_service_save) {
      const auto endpoint = control_text::read(get(ai_endpoint), 65536);
      const auto model = control_text::read(get(ai_model), 256);
      if (!endpoint || !model || !endpoint_ok_) { SetWindowTextW(get(ai_endpoint_hint), L"请先填写有效的接口地址。"); return result; }
      auto endpoints = saved_.at("endpoints"), models = saved_.at("models");
      const auto* preset = ai::find_provider(wide(provider_));
      if (preset && *endpoint == narrow(preset->endpoint)) endpoints.erase(provider_); else if (endpoint->empty()) endpoints.erase(provider_); else endpoints[provider_] = *endpoint;
      if (preset && *model == narrow(preset->model)) models.erase(provider_); else if (model->empty()) models.erase(provider_); else models[provider_] = *model;
      result.changes = {{{"ai_assistant", "endpoint"}, *endpoint}, {{"ai_assistant", "model"}, *model},
          {{"ai_assistant", "endpoints"}, endpoints}, {{"ai_assistant", "models"}, models}};
      pending_ = Pending::service;
    } else if (id == ai_prompt_save) {
      const auto value = control_text::read(get(ai_prompt), 16384); if (!value) return result;
      result.changes = {{{"ai_assistant", prompt_key_}, *value}}; pending_ = Pending::prompt;
    } else if (id == ai_models_fetch) {
      const auto config = configuration(); if (!config || !endpoint_ok_) return result;
      result.request = Json{{"endpoint", config->at("endpoint")}, {"token", config->at("token")}}; result.models = true;
    } else if (id == ai_test) {
      const auto config = configuration(); if (!config || !endpoint_ok_) return result;
      result.request = Json{{"service", "ai.assistant"}, {"config", *config}};
    }
    return result;
  }

  void completed(const Json&, const Json& response, bool models) {
    if (models) {
      SetWindowTextW(get(ai_models_status), L"获取模型失败，请检查地址、密钥和网络。");
      if (response.is_object() && response.value("ok", false) && response.contains("value") && response.at("value").is_array()) {
        fetched_.clear();
        for (const auto& entry : response.at("value")) if (entry.is_string()) fetched_.push_back(wide(entry.get<std::string>()));
        const auto config = configuration();
        const auto current = config ? config->at("model").get<std::string>() : std::string{};
        fill_models(current);
        SetWindowTextW(get(ai_models_status), ai::models_status(fetched_.size()).c_str());
      }
      return;
    }
    tested_ = configuration();
    test_message_ = L"无法连接服务，请检查地址、密钥和网络。";
    if (response.is_object() && response.value("ok", false) && response.contains("value") && response.at("value").is_object())
      test_message_ = wide(response.at("value").value("message", std::string{}));
    update();
  }

  std::optional<Json> model_request() const { return configuration(); }

private:
  enum class Pending { none, service, token, prompt };
  HWND get(int id) const { const auto it = controls_.find(id); return it == controls_.end() ? nullptr : it->second; }
  static std::string narrow(std::wstring_view value) {
    if (value.empty()) return {};
    const int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
    if (size <= 0) return {};
    std::string result(static_cast<size_t>(size), '\0');
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), result.data(), size, nullptr, nullptr);
    return result;
  }
  static std::wstring wide(std::string_view value) { return control_text::wide(std::string(value)); }
  std::optional<Json> configuration() const {
    const auto endpoint = control_text::read(get(ai_endpoint), 65536), model = control_text::read(get(ai_model), 256), token = control_text::read(get(ai_token), 65536);
    if (!endpoint || !model || !token) return std::nullopt;
    return Json{{"provider", provider_}, {"endpoint", *endpoint}, {"model", *model},
        {"token", ai::provider_uses_token(wide(provider_)) ? *token : ""}};
  }
  void update_origin() {
    const auto endpoint = control_text::read(get(ai_endpoint), 65536);
    origin_.reset(); endpoint_ok_ = false;
    std::wstring hint = L"请填写服务提供商给出的完整接口地址。";
    if (endpoint) {
      auto owned = msime::host_api::own_string(msime_client_ai_endpoint_check(reinterpret_cast<const uint8_t*>(endpoint->data()), endpoint->size()));
      const auto response = owned ? Json::parse(owned.get(), nullptr, false) : Json();
      if (response.is_object() && response.value("ok", false) && response.contains("value") && response.at("value").is_object()) {
        const auto& value = response.at("value");
        endpoint_ok_ = value.value("problem", Json()).is_null() && value.contains("origin") && value.at("origin").is_string();
        if (endpoint_ok_) origin_ = value.at("origin").get<std::string>();
        hint = wide(value.value("hint", std::string{}));
      }
    }
    SetWindowTextW(get(ai_endpoint_hint), hint.c_str());
    const auto token = origin_ ? saved_.at("tokens").value(*origin_, std::string{}) : std::string{};
    SetWindowTextW(get(ai_token), wide(token).c_str());
    update();
  }
  void fill_models(const std::string& current) {
    SendMessageW(get(ai_model), CB_RESETCONTENT, 0, 0);
    std::vector<std::wstring> values;
    if (const auto* preset = ai::find_provider(wide(provider_))) for (const auto& model : preset->models) values.emplace_back(model);
    values.insert(values.end(), fetched_.begin(), fetched_.end());
    std::vector<std::wstring> unique;
    for (const auto& value : values) if (std::find(unique.begin(), unique.end(), value) == unique.end()) { unique.push_back(value); SendMessageW(get(ai_model), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(value.c_str())); }
    SetWindowTextW(get(ai_model), wide(current).c_str());
  }
  void clear_result() { tested_.reset(); SetWindowTextW(get(ai_test_result), L""); }
  void update() {
    if (saved_.empty()) return;
    const bool uses_token = ai::provider_uses_token(wide(provider_));
    EnableWindow(get(ai_token), enabled_ && uses_token && origin_.has_value());
    EnableWindow(get(ai_token_reveal), enabled_ && uses_token && origin_.has_value());
    EnableWindow(get(ai_token_save), enabled_ && uses_token && origin_.has_value());
    EnableWindow(get(ai_service_save), enabled_ && endpoint_ok_);
    const auto config = configuration();
    const bool complete = config && endpoint_ok_ && !trim(wide(config->at("model").get<std::string>())).empty() &&
        (!uses_token || !trim(wide(config->at("token").get<std::string>())).empty());
    EnableWindow(get(ai_models_fetch), enabled_ && endpoint_ok_ && config && (!uses_token || complete));
    EnableWindow(get(ai_test), enabled_ && saved_.at("enabled").get<bool>() && complete);
    const bool show = config && tested_ && *config == *tested_;
    SetWindowTextW(get(ai_test_result), show ? test_message_.c_str() : L"");
  }

  std::map<int, HWND> controls_;
  Json saved_;
  std::string provider_, prompt_key_;
  std::optional<std::string> origin_;
  std::vector<std::wstring> fetched_;
  std::optional<Json> tested_;
  std::wstring test_message_;
  Pending pending_ = Pending::none;
  bool enabled_ = false, updating_ = false, revealed_ = false, endpoint_ok_ = false;

  static std::wstring trim(std::wstring value) {
    const auto first = value.find_first_not_of(L" \t\r\n");
    if (first == std::wstring::npos) return {};
    const auto last = value.find_last_not_of(L" \t\r\n");
    return value.substr(first, last - first + 1);
  }
};

} // namespace msime::settings::win7
