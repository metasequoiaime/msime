#pragma once

#include "ControlText.h"
#include "ThemeData.h"
#include "../VoiceSettings.h"
#include <commctrl.h>
#include <functional>
#include <map>
#include <set>
#include <vector>

namespace msime::settings::win7 {
class VoiceForm {
public:
  enum Id { enabled = 3400, sound, mute, inline_text, hold_key, f9, language, commit, asr_provider,
    asr_endpoint, asr_model, asr_token, asr_app_key, auth_mode, resource, punc, itn, ddc, boosting,
    model_path, mirror, polish_enabled, polish_provider, polish_endpoint, polish_model, polish_token,
    polish_thinking, prompt_id, prompt_one, prompt_two, prompt_three, save, test_asr, test_polish,
    test_result, show_secrets, stream_endpoint, devices, device_refresh, device_status, models_open };
  struct Command { bool handled = false; std::vector<Change> changes; std::optional<Json> request; std::wstring message; bool refresh_devices = false, models = false; };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height); if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12; return control;
    };
    const auto note = [&](const wchar_t* title, int height = 36) { add(-1, L"STATIC", title, SS_LEFT, height); };
    const auto button = [&](int id, const wchar_t* title) { add(id, L"BUTTON", title, WS_TABSTOP | BS_PUSHBUTTON, 32); };
    const auto combo = [&](int id, const wchar_t* title) { note(title); add(id, WC_COMBOBOXW, title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 240); };
    for (const auto& field : booleans()) add(field.id, L"BUTTON", field.title, WS_TABSTOP | BS_AUTOCHECKBOX);
    note(L"修改后点击保存。网络配置测试使用当前表单；打开页面不会录音或测试服务。", 56);
    combo(hold_key, L"长按录音快捷键"); for (const auto& key : voice::hold_keys) option(hold_key, key.label); option(hold_key, L"不使用");
    note(L"按住期间按空格锁定录音，再次按快捷键或点完成结束，Escape 取消。", 56);
    combo(commit, L"识别结果提交方式"); for (const auto title : {L"输入法会话", L"系统按键", L"剪贴板粘贴"}) option(commit, title);
    combo(asr_provider, L"识别服务"); combo(polish_provider, L"文本润色服务");
    combo(auth_mode, L"豆包鉴权方式"); option(auth_mode, L"新版 API Key"); option(auth_mode, L"旧版 App ID + Access Token");
    combo(stream_endpoint, L"豆包流式接口预设"); option(stream_endpoint, L"自定义地址（在接口地址中填写）");
    combo(polish_thinking, L"DeepSeek 润色思考强度"); for (const auto title : {L"关闭", L"低", L"高", L"最高"}) option(polish_thinking, title);
    combo(prompt_id, L"文本润色方案"); for (const auto& item : voice::polish_presets) { prompt_ids_.push_back(item.id); option(prompt_id, item.name); }
    for (const auto& item : voice::polish_custom_slots) { prompt_ids_.push_back(item.value); option(prompt_id, item.label); }
    for (const auto& field : texts()) {
      note(field.title); const auto control = add(field.id, L"EDIT", L"", WS_TABSTOP | WS_BORDER |
          (field.multiline ? ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL : ES_AUTOHSCROLL) | (field.secret ? ES_PASSWORD : 0), field.multiline ? 130 : 28);
      SendMessageW(control, EM_SETLIMITTEXT, field.multiline ? 16384 : field.secret ? 16384 : 4096, 0);
    }
    button(show_secrets, L"显示密钥"); button(save, L"保存语音设置");
    button(test_asr, L"测试识别配置（发送合成静音）"); button(test_polish, L"测试润色配置（发送合成文字）");
    add(test_result, L"STATIC", L"", SS_LEFT, 80);
    combo(devices, L"麦克风"); option(devices, L"系统默认设备"); button(device_refresh, L"刷新麦克风列表");
    add(device_status, L"STATIC", L"", SS_LEFT, 56); button(models_open, L"管理本地识别模型");
    load_catalog(); enable(false); return top;
  }
  void refresh(const Json& preferences) {
    const auto value = preferences.find("voice_input"); if (value == preferences.end() || !value->is_object()) { enable(false); return; }
    if (loaded_ && saved_ == *value) return;
    saved_ = *value; loaded_ = true; updating_ = true; dirty_.clear();
    for (const auto& field : texts()) SetWindowTextW(get(field.id), control_text::wide(saved_.value(field.key, std::string{})).c_str());
    for (const auto& field : booleans()) SendMessageW(get(field.id), BM_SETCHECK, saved_.value(field.key, false) ? BST_CHECKED : BST_UNCHECKED, 0);
    SendMessageW(get(polish_enabled), BM_SETCHECK, saved_.value("polish_enabled", false) || saved_.value("polish_text", false) ? BST_CHECKED : BST_UNCHECKED, 0);
    choose_provider(asr_provider, asr_, saved_.value("asr_provider", std::string{})); choose_provider(polish_provider, polish_, saved_.value("polish_provider", std::string{}));
    select(commit, saved_.value("commit_mode", std::string{}) == "sendinput" ? 1 : saved_.value("commit_mode", std::string{}) == "ctrl_v" ? 2 : 0);
    select(auth_mode, saved_.value("doubao_auth_mode", std::string{}) == "legacy" ? 1 : 0);
    const auto thinking = saved_.value("polish_thinking", std::string{}); select(polish_thinking, thinking == "low" ? 1 : thinking == "high" ? 2 : thinking == "max" ? 3 : 0);
    const auto stored_prompt = control_text::wide(saved_.value("polish_prompt_id", std::string{}));
    const auto prompt = voice::polish_prompt_slot(stored_prompt);
    for (size_t index = 0; index < prompt_ids_.size(); ++index) if (prompt_ids_[index] == prompt) select(prompt_id, static_cast<int>(index));
    std::map<std::wstring_view, bool> holds; for (const auto& key : voice::hold_keys) holds[key.key] = saved_.value(control_text::utf8(key.key).value(), key.fallback);
    const auto chosen = voice::selected_hold_key(holds); int index = 3; for (int current = 0; current < 3; ++current) if (voice::hold_keys[static_cast<size_t>(current)].key == chosen) index = current; select(hold_key, index);
    select(stream_endpoint, 0); for (size_t current = 0; current < endpoints_.size(); ++current)
      if (endpoints_[current].at("endpoint") == saved_.value("asr_endpoint", std::string{})) select(stream_endpoint, static_cast<int>(current + 1));
    render_devices(); SetWindowTextW(get(test_result), L""); updating_ = false; enable(enabled_);
  }
  void enable(bool enabled_value) {
    enabled_ = enabled_value; const bool ready = enabled_ && loaded_;
    for (const auto& item : controls_) EnableWindow(item.second, ready);
    EnableWindow(get(asr_provider), ready && catalog_ok_); EnableWindow(get(polish_provider), ready && catalog_ok_);
    const auto provider = selected_provider(asr_provider, asr_); const bool doubao = provider == "doubao", cloud = !provider.empty() && provider != "local" && provider != "system";
    for (const auto id : {asr_endpoint, asr_model, asr_token, test_asr}) EnableWindow(get(id), ready && cloud);
    for (const auto id : {auth_mode, resource, punc, itn, ddc, boosting, stream_endpoint}) EnableWindow(get(id), ready && doubao);
    EnableWindow(get(asr_app_key), ready && doubao && selected(auth_mode) == 1);
    EnableWindow(get(devices), ready && devices_loaded_); EnableWindow(get(device_refresh), ready);
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event); if (!controls_.count(id)) return {};
    Command result; result.handled = true; if (updating_ || !enabled_ || !loaded_ || !IsWindowEnabled(get(id))) return result;
    if (notification == EN_CHANGE || notification == CBN_SELCHANGE || notification == BN_CLICKED) { SetWindowTextW(get(test_result), L""); dirty_.insert(id); }
    if ((id == asr_provider || id == polish_provider) && notification == CBN_SELCHANGE) {
      const auto prefix = id == asr_provider ? "asr" : "polish"; const auto next = selected_provider(id, id == asr_provider ? asr_ : polish_);
      Json current{{"provider", saved_.value(std::string(prefix) + "_provider", std::string{})}};
      for (const auto* key : {"endpoint", "model", "token"}) current[key] = text(field_id(std::string(prefix) + "_" + key));
      current["tokens"] = saved_.value(std::string(prefix) + "_tokens", Json::object());
      const auto patch = theme_data::value(theme_data::call(msime_client_voice_provider_update,
        {{"service", id == asr_provider ? "voice.asr" : "voice.polish"}, {"provider", next}, {"current", current}}));
      if (!patch || !patch->is_object()) { result.message = L"服务配置无法切换，请重新读取。"; choose_provider(id, id == asr_provider ? asr_ : polish_, current.at("provider").get<std::string>()); return result; }
      for (const auto& field : patch->items()) result.changes.push_back({{"voice_input", std::string(prefix) + "_" + field.key()}, field.value()});
      return result;
    }
    if (id == auth_mode && notification == CBN_SELCHANGE) { enable(enabled_); return result; }
    if (id == stream_endpoint && notification == CBN_SELCHANGE) {
      const auto index = selected(stream_endpoint); if (index > 0 && static_cast<size_t>(index) <= endpoints_.size())
        { SetWindowTextW(get(asr_endpoint), control_text::wide(endpoints_[static_cast<size_t>(index - 1)].at("endpoint").get<std::string>()).c_str()); dirty_.insert(asr_endpoint); } return result;
    }
    if (id == devices && notification == CBN_SELCHANGE) {
      const auto index = selected(devices); if (index < 0 || static_cast<size_t>(index) > devices_.size()) return result;
      result.changes = {{{"voice_input", "capture_backend"}, index ? devices_[static_cast<size_t>(index - 1)].at("backend") : Json("")},
        {{"voice_input", "capture_device"}, index ? devices_[static_cast<size_t>(index - 1)].at("id") : Json("")}}; return result;
    }
    if (notification != BN_CLICKED) return result;
    if (id == show_secrets) {
      secrets_ = !secrets_; for (const auto& field : texts()) if (field.secret) { SendMessageW(get(field.id), EM_SETPASSWORDCHAR, secrets_ ? 0 : L'*', 0); InvalidateRect(get(field.id), nullptr, TRUE); }
      SetWindowTextW(get(show_secrets), secrets_ ? L"隐藏密钥" : L"显示密钥"); return result;
    }
    if (id == device_refresh) { result.refresh_devices = true; return result; }
    if (id == models_open) { result.models = true; return result; }
    if (id == test_asr || id == test_polish) {
      const bool asr = id == test_asr; const auto config = configuration(asr);
      if (!preflight(asr)) { result.message = L"接口地址或凭据格式无效，未发送测试请求。"; return result; }
      const auto provider = control_text::wide(config.at("provider").get<std::string>());
      if (asr ? voice::asr_test_disabled(provider, control_text::wide(config.value("auth_mode", std::string{})), control_text::wide(config.at("token").get<std::string>()), control_text::wide(config.value("app_id", std::string{}))) : voice::polish_test_disabled(control_text::wide(config.at("token").get<std::string>()))) {
        result.message = L"请先填写服务需要的凭据。"; return result;
      }
      result.request = Json{{"service", asr ? "voice.asr" : "voice.polish"}, {"config", config}}; return result;
    }
    if (id != save) return result;
    if (!preflight(true) || !preflight(false)) { result.message = L"接口地址或凭据格式无效，设置未保存。"; return result; }
    const auto check = theme_data::value(theme_data::call(msime_client_voice_model_setting_check, {{"asr_model_path", text(model_path)}, {"asr_model_mirror", text(mirror)}}));
    if (!check || !check->value("asr_model_path", false) || !check->value("asr_model_mirror", false)) { result.message = L"本地模型目录须为绝对路径，镜像须为空或有效 HTTPS 地址。"; return result; }
    Json patch = Json::object(); for (const auto& field : texts()) if (dirty_.count(field.id)) patch[field.key] = text(field.id);
    for (const auto& field : booleans()) if (dirty_.count(field.id)) patch[field.key] = checked(field.id);
    if (dirty_.count(polish_enabled)) patch["polish_text"] = checked(polish_enabled);
    if (dirty_.count(commit)) patch["commit_mode"] = selected(commit) == 1 ? "sendinput" : selected(commit) == 2 ? "ctrl_v" : "tsf";
    if (dirty_.count(auth_mode)) patch["doubao_auth_mode"] = selected(auth_mode) == 1 ? "legacy" : "api_key";
    constexpr const char* levels[] = {"disabled", "low", "high", "max"}; const auto thinking = selected(polish_thinking); if (dirty_.count(polish_thinking) && thinking >= 0 && thinking < 4) patch["polish_thinking"] = levels[thinking];
    const auto prompt = selected(prompt_id); if (dirty_.count(prompt_id) && prompt >= 0 && static_cast<size_t>(prompt) < prompt_ids_.size()) patch["polish_prompt_id"] = control_text::utf8(prompt_ids_[static_cast<size_t>(prompt)]).value();
    const auto hold = selected(hold_key); const auto key = hold >= 0 && hold < 3 ? voice::hold_keys[static_cast<size_t>(hold)].key : voice::no_hold_key;
    if (dirty_.count(hold_key)) for (const auto& item : voice::hold_key_patch(key)) patch[control_text::utf8(item.first).value()] = item.second;
    for (const auto* prefix : {"asr", "polish"}) {
      if (!dirty_.count(field_id(std::string(prefix) + "_token"))) continue;
      const auto provider = saved_.value(std::string(prefix) + "_provider", std::string{}); auto tokens = saved_.value(std::string(prefix) + "_tokens", Json::object());
      const auto token = text(field_id(std::string(prefix) + "_token")); if (!provider.empty()) { if (token.empty()) tokens.erase(provider); else tokens[provider] = token; } patch[std::string(prefix) + "_tokens"] = std::move(tokens);
    }
    for (const auto& item : patch.items()) if (!saved_.contains(item.key()) || saved_.at(item.key()) != item.value()) result.changes.push_back({{"voice_input", item.key()}, item.value()});
    return result;
  }
  void tested(const Json& request, const Json& response) {
    const bool asr = request.value("service", std::string{}) == "voice.asr";
    if (!request.contains("config") || request.at("config") != configuration(asr)) return;
    const auto value = theme_data::value(response); const auto message = value && value->is_object() ? control_text::wide(value->value("message", std::string{})) : L"服务测试失败，请检查配置和网络。";
    SetWindowTextW(get(test_result), message.c_str());
  }
  void capture_devices(const Json& response) {
    const auto value = theme_data::value(response);
    if (!value || !value->is_array() || value->size() > 128) { SetWindowTextW(get(device_status), L"麦克风列表读取失败，现有选择保留。"); return; }
    std::vector<Json> parsed; for (const auto& item : *value) {
      if (!item.is_object()) { SetWindowTextW(get(device_status), L"麦克风列表读取失败，现有选择保留。"); return; }
      for (const auto* key : {"backend", "id", "label"}) if (!item.contains(key) || !item.at(key).is_string()) {
        SetWindowTextW(get(device_status), L"麦克风列表读取失败，现有选择保留。"); return;
      }
      parsed.push_back(item);
    }
    devices_ = std::move(parsed); devices_loaded_ = true; render_devices(); enable(enabled_); SetWindowTextW(get(device_status), L"麦克风列表已刷新，切换设备会立即保存。");
  }
private:
  struct Boolean { int id; const char* key; const wchar_t* title; };
  static const std::vector<Boolean>& booleans() {
    static const std::vector<Boolean> fields{{enabled,"enabled",L"启用语音输入"}, {sound,"sound_enabled",L"语音提示音"}, {mute,"mute_system_audio",L"录音时静音其他声音"},
      {inline_text,"stream_inline_preedit",L"边说边显示"}, {f9,"hotkey_ctrl_f9",L"Ctrl+F9 开始 / 结束"}, {punc,"doubao_enable_punc",L"豆包自动标点"},
      {itn,"doubao_enable_itn",L"豆包数字规范化"}, {ddc,"doubao_enable_ddc",L"豆包语气词处理"}, {polish_enabled,"polish_enabled",L"启用文本润色（使用云服务）"}}; return fields;
  }
  struct Text { int id; const char* key; const wchar_t* title; bool secret = false, multiline = false; };
  static const std::vector<Text>& texts() {
    static const std::vector<Text> fields{{language,"language",L"识别语言（系统识别如 zh-CN、en-US）"}, {asr_endpoint,"asr_endpoint",L"识别接口地址"},
      {asr_model,"asr_model",L"识别模型（可自定义）"}, {asr_token,"asr_token",L"识别服务 API Key / Access Token",true}, {asr_app_key,"asr_app_key",L"豆包旧版 App ID",true},
      {resource,"asr_resource_id",L"豆包资源 ID"}, {boosting,"doubao_boosting_table_id",L"豆包热词表 ID"}, {model_path,"asr_model_path",L"本地识别模型目录"}, {mirror,"asr_model_mirror",L"本地模型下载镜像（HTTPS 前缀）"},
      {polish_endpoint,"polish_endpoint",L"润色接口地址"}, {polish_model,"polish_model",L"润色模型（可自定义）"}, {polish_token,"polish_token",L"润色服务 API Key",true},
      {prompt_one,"polish_prompt_custom_1",L"润色自定义提示词一",false,true}, {prompt_two,"polish_prompt_custom_2",L"润色自定义提示词二",false,true}, {prompt_three,"polish_prompt_custom_3",L"润色自定义提示词三",false,true}}; return fields;
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  std::string text(int id) const { return control_text::read(get(id), 65536).value_or(""); }
  bool checked(int id) const { return SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED; }
  int selected(int id) const { return static_cast<int>(SendMessageW(get(id), CB_GETCURSEL, 0, 0)); }
  void select(int id, int index) { SendMessageW(get(id), CB_SETCURSEL, index, 0); }
  void option(int id, std::wstring_view title) { const std::wstring value(title); SendMessageW(get(id), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(value.c_str())); }
  static int field_id(const std::string& key) { for (const auto& field : texts()) if (field.key == key) return field.id; return -1; }
  std::string selected_provider(int id, const std::vector<Json>& table) const { const auto index = selected(id); return index >= 0 && static_cast<size_t>(index) < table.size() ? table[static_cast<size_t>(index)].at("id").get<std::string>() : std::string{}; }
  void choose_provider(int id, const std::vector<Json>& table, const std::string& provider) { select(id, -1); for (size_t index = 0; index < table.size(); ++index) if (table[index].at("id") == provider) select(id, static_cast<int>(index)); }
  void load_catalog() {
    const auto value = theme_data::value(theme_data::response(msime_client_voice_provider_catalog()));
    if (!value || !value->is_object()) return;
    try {
      asr_ = value->at("asr").get<std::vector<Json>>(); polish_ = value->at("polish").get<std::vector<Json>>(); endpoints_ = value->at("doubao_stream_endpoints").get<std::vector<Json>>();
      asr_.push_back({{"id","system"},{"title","Windows 系统识别"}}); asr_.push_back({{"id","local"},{"title","本地模型（离线）"}});
      for (const auto& entry : asr_) option(asr_provider, control_text::wide(entry.at("title").get<std::string>()));
      for (const auto& entry : polish_) option(polish_provider, control_text::wide(entry.at("title").get<std::string>()));
      for (const auto& entry : endpoints_) option(stream_endpoint, control_text::wide(entry.at("title").get<std::string>())); catalog_ok_ = true;
    } catch (const std::exception&) { catalog_ok_ = false; }
  }
  Json configuration(bool asr) const {
    const std::string prefix = asr ? "asr" : "polish"; const auto provider = selected_provider(asr ? asr_provider : polish_provider, asr ? asr_ : polish_);
    Json config{{"provider",provider}, {"endpoint",text(field_id(prefix + "_endpoint"))}, {"model",text(field_id(prefix + "_model"))}, {"token",text(field_id(prefix + "_token"))}};
    for (const auto& entry : asr ? asr_ : polish_) if (entry.at("id") == provider) for (const auto* key : {"endpoint","model"})
      if (msime::settings::js_trim(control_text::wide(config.at(key).get<std::string>())).empty()) config[key] = entry.value(key, std::string{});
    if (asr && provider == "doubao") {
      config["auth_mode"] = selected(auth_mode) == 1 ? "legacy" : "api_key"; config["app_id"] = selected(auth_mode) == 1 ? text(asr_app_key) : "";
      config["resource_id"] = control_text::utf8(voice::doubao_test_resource_id(control_text::wide(text(resource)))).value();
      for (const auto& item : {std::pair<int,const char*>{punc,"doubao_enable_punc"},{itn,"doubao_enable_itn"},{ddc,"doubao_enable_ddc"}}) config[item.second] = checked(item.first);
      config["doubao_boosting_table_id"] = text(boosting);
    }
    return config;
  }
  bool preflight(bool asr) const {
    const auto config = configuration(asr); const auto provider = config.at("provider").get<std::string>(); if (asr && (provider == "local" || provider == "system")) return true;
    const auto endpoint = config.at("endpoint").get<std::string>(); const bool websocket = asr && provider == "doubao";
    const auto accepted = theme_data::value(theme_data::response(msime_client_voice_endpoint_issue(reinterpret_cast<const uint8_t*>(endpoint.data()), endpoint.size(), websocket)));
    if (!accepted || !accepted->value("accepted", false)) return false;
    for (const auto& token : {config.at("token").get<std::string>(), asr ? text(asr_app_key) : std::string{}}) {
      const auto valid = theme_data::value(theme_data::response(msime_client_voice_credential_issue(reinterpret_cast<const uint8_t*>(provider.data()), provider.size(), reinterpret_cast<const uint8_t*>(token.data()), token.size())));
      if (!valid || !valid->value("accepted", false)) return false;
    }
    return true;
  }
  void render_devices() {
    SendMessageW(get(devices), CB_RESETCONTENT, 0, 0); option(devices, L"系统默认设备"); select(devices, 0);
    for (size_t index = 0; index < devices_.size(); ++index) {
      option(devices, control_text::wide(devices_[index].at("label").get<std::string>()));
      if (saved_.value("capture_backend", std::string{}) == devices_[index].at("backend") && saved_.value("capture_device", std::string{}) == devices_[index].at("id")) select(devices, static_cast<int>(index + 1));
    }
  }
  std::set<int> dirty_; std::map<int,HWND> controls_; Json saved_; std::vector<Json> asr_, polish_, endpoints_, devices_; std::vector<std::wstring_view> prompt_ids_;
  bool loaded_ = false, enabled_ = false, updating_ = false, catalog_ok_ = false, secrets_ = false, devices_loaded_ = false;
};
} // namespace msime::settings::win7
