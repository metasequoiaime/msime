#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "ThemeData.h"
#include "DataUrlImage.h"
#include "AccountManagementForm.h"
#include "SettingsWindow.h"
#include "../AccountPageModel.h"
#include <commctrl.h>
#include <functional>
#include <map>
#include <optional>
#include <chrono>

namespace msime::settings::win7 {

// 账号页只编排控件和稳定的 account action JSON；目录、会话迁移和网络全部由工作线程的 C ABI 调用完成。
class AccountForm {
public:
  struct Command {
    bool handled = false;
    std::optional<Json> request;
    std::wstring confirmation;
    bool choose_avatar = false;
    bool export_data = false;
  };

  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return control;
    };
    add(account_status, L"STATIC", L"正在读取账号状态…", SS_LEFT, 64);
    add(account_refresh, L"BUTTON", L"刷新登录方式", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_email, L"BUTTON", L"邮箱验证码登录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_phone, L"BUTTON", L"手机号验证码登录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_target, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(account_target), EM_LIMITTEXT, 320, 0);
    add(account_request_code, L"BUTTON", L"获取验证码", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_code, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_NUMBER | ES_AUTOHSCROLL);
    SendMessageW(get(account_code), EM_LIMITTEXT, 6, 0);
    add(account_login, L"BUTTON", L"登录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_google, L"BUTTON", L"使用 Google 登录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_google_cancel, L"BUTTON", L"取消 Google 登录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_logout, L"BUTTON", L"退出登录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_logout_all, L"BUTTON", L"退出所有设备", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_notice, L"STATIC", L"", SS_LEFT, 100);
    add(account_forget, L"BUTTON", L"清除失效的本机登录状态", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_profile, L"BUTTON", L"刷新账号资料", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_profile_text, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY, 144);
    add(account_avatar, L"STATIC", L"账号头像", SS_OWNERDRAW, 128);
    add(account_avatar_load, L"BUTTON", L"刷新头像", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_avatar_upload, L"BUTTON", L"更换头像…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_avatar_remove, L"BUTTON", L"移除上传的头像", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", L"社区昵称", SS_LEFT);
    add(account_name, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(account_name), EM_LIMITTEXT, 256, 0);
    add(account_rename, L"BUTTON", L"保存昵称", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", account::sync_description.data(), SS_LEFT, 96);
    add(account_sync_load, L"BUTTON", L"刷新云端设置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_sync_upload, L"BUTTON", L"上传本机设置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_sync_apply, L"BUTTON", L"下载并应用云端设置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_sync_notice, L"STATIC", L"", SS_LEFT, 96);
    top += management_.create([&](const wchar_t* type, const wchar_t* title, DWORD style, int id, int offset, int height) {
      return row(type, title, style, id, top + offset, height);
    });
    add(account_delete, L"BUTTON", L"注销账号…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    return top;
  }

  void refresh() {
    updating_ = true;
    SetWindowTextW(get(account_status), signed_in_ ? (L"已登录：" + user_name_) .c_str() : L"当前未登录");
    SetWindowTextW(get(account_notice), notice_.c_str());
    SetWindowTextW(get(account_target), target_.c_str());
    SetWindowTextW(get(account_code), code_.c_str());
    SetWindowTextW(get(account_name), name_.c_str());
    SetWindowTextW(get(account_profile_text), profile_text_.c_str());
    SetWindowTextW(get(account_sync_notice), sync_notice_.c_str());
    updating_ = false;
    update();
  }

  void google_waiting(bool waiting) { google_waiting_ = waiting; update(); }
  void avatar(theme_data::Image image) { avatar_ = std::move(image); InvalidateRect(get(account_avatar), nullptr, TRUE); }
  bool draw(const DRAWITEMSTRUCT& drawing) const {
    return drawing.CtlID == account_avatar && image_data::draw(drawing, avatar_, L"暂无头像");
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    update();
  }

  Command command(WPARAM event) {
    auto managed = management_.command(event);
    if (managed.handled) return {true, std::move(managed.request), std::move(managed.confirmation), false, managed.export_data};
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (id == account_google_cancel && notification == BN_CLICKED && google_waiting_) {
      result.request = Json{{"operation", "google_cancel"}};
      return result;
    }
    if (!enabled_ || updating_) return result;
    if ((id == account_email || id == account_phone) && notification == BN_CLICKED) {
      const auto channel = id == account_email ? L"email" : L"phone";
      if (channel_ != channel) { channel_ = channel; challenge_.clear(); code_.clear(); }
      notice_.clear(); refresh(); return result;
    }
    if (id == account_target && notification == EN_CHANGE) {
      bool changed = false;
      if (const auto value = control_text::read(get(account_target), 320)) {
        const auto next = control_text::wide(*value);
        changed = account::trim_login_target(next) != account::trim_login_target(target_);
        if (changed) { challenge_.clear(); code_.clear(); }
        target_ = next;
      }
      // 目标改变后也清掉可见的旧验证码；不重写目标编辑框，保留光标位置。
      if (changed) {
        updating_ = true; SetWindowTextW(get(account_code), L""); updating_ = false;
      }
      update();
      return result;
    }
    if (id == account_code && notification == EN_CHANGE) {
      if (const auto value = control_text::read(get(account_code), 16)) code_ = account::code_digits(control_text::wide(*value));
      update();
      return result;
    }
    if (id == account_name && notification == EN_CHANGE) {
      if (const auto value = control_text::read(get(account_name), 256)) name_ = control_text::wide(*value);
      return result;
    }
    if (notification != BN_CLICKED || !IsWindowEnabled(get(id))) return result;
    if (id == account_refresh) result.request = Json{{"operation", "providers"}};
    else if (id == account_request_code) {
      const auto target = account::trim_login_target(target_);
      if (channel_.empty() || target.empty()) { notice_ = L"请先选择邮箱或手机号并填写登录目标。"; refresh(); return result; }
      result.request = Json{{"operation", "request_code"}, {"provider", channel_}, {"target", narrow(target)}};
    } else if (id == account_login) {
      if (challenge_.empty() || expires_at_ <= now() || !account::six_digits(code_)) { notice_ = L"请先获取有效验证码并填写 6 位数字验证码。"; refresh(); return result; }
      result.request = Json{{"operation", "login"}, {"challenge_id", challenge_}, {"code", narrow(code_)}};
    } else if (id == account_google) result.request = Json{{"operation", "google_login"}};
    else if (id == account_logout) result.request = Json{{"operation", "logout"}, {"all", false}};
    else if (id == account_logout_all || id == account_delete) {
      const auto copy = account::confirmation_text(id == account_logout_all ? account::Confirmation::logout_all : account::Confirmation::remove);
      result.confirmation = copy.message;
      result.request = id == account_logout_all ? Json{{"operation", "logout"}, {"all", true}} : Json{{"operation", "account_delete"}};
    } else if (id == account_forget) {
      result.request = Json{{"operation", "forget"}};
      result.confirmation = L"只清除这台设备保存的失效登录状态？此操作不会注销云端账号。";
    } else if (id == account_profile) result.request = Json{{"operation", "profile"}};
    else if (id == account_avatar_load) result.request = Json{{"operation", "avatar"}};
    else if (id == account_avatar_upload) result.choose_avatar = true;
    else if (id == account_avatar_remove) {
      result.request = Json{{"operation", "remove_avatar"}};
      result.confirmation = L"移除这个账号上传的头像？此操作会同步到其他设备。";
    }
    else if (id == account_rename) {
      const auto value = theme_data::value(theme_data::call(msime_client_account_name, {{"name", narrow(name_)}, {"id", user_id_}}));
      if (!value || !value->is_object() || !value->contains("valid") || value->at("valid") != true ||
          !value->contains("normalized") || !value->at("normalized").is_string()) {
        notice_ = L"昵称无效，请修改后重试。"; refresh(); return result;
      }
      result.request = Json{{"operation", "rename"}, {"display_name", value->at("normalized")}};
    } else if (id == account_sync_load) { sync_ready_ = false; result.request = Json{{"operation", "preferences_schema"}}; }
    else if (id == account_sync_upload) {
      result.request = Json{{"operation", "preferences_upload"}};
      result.confirmation = account::sync_upload_confirmation;
    } else if (id == account_sync_apply && cloud_) {
      result.request = Json{{"operation", "preferences_apply"}, {"user_id", user_id_}, {"preferences", *cloud_}};
      result.confirmation = account::sync_apply_confirmation;
    }
    return result;
  }

  std::optional<Json> completed(const Json& request, const Json& response) {
    notice_.clear();
    const auto operation = request.value("operation", std::string{});
    if (AccountManagementForm::handles(operation)) {
      const auto value = theme_data::value(response);
      if (operation == "data_delete" && value) { cloud_.reset(); sync_ready_ = false; }
      auto next = management_.completed(request, response);
      if (operation == "data_summary" && value && signed_in_ && !sync_ready_ && !next) next = Json{{"operation", "preferences_schema"}};
      return next;
    }
    const auto result = theme_data::value(response);
    if (!result) {
      const auto error = response.is_object() && response.contains("error") && response.at("error").is_string() ? response.at("error").get<std::string>() : "account_unavailable";
      notice_ = account::error_message(control_text::wide(operation == "upload_avatar" && error == "account_invalid" ? "account_avatar_invalid" : error));
      if (operation == "status") unreadable_ = true;
      if (operation.rfind("preferences_", 0) == 0) sync_notice_ = notice_;
      refresh();
      return std::nullopt;
    }
    const auto& value = *result;
    if ((operation == "providers" && !value.is_object()) ||
        ((operation == "status" || operation == "login" || operation == "google_login") &&
         (!value.is_object() || !value.contains("user") ||
          (!value.at("user").is_null() && (!value.at("user").is_object() || string(value.at("user"), "id").empty()))))) {
      notice_ = L"账号返回内容无法读取，请刷新后重试。";
      if (operation == "status") unreadable_ = true;
      refresh(); return std::nullopt;
    }
    std::optional<Json> next;
    if (operation == "providers") {
      provider_email_ = flag(value, "email");
      provider_phone_ = flag(value, "phone");
      provider_google_ = flag(value, "google");
      notice_ = L"登录方式已刷新。";
      next = status_request();
    } else if (operation == "request_code") {
      challenge_ = string(value, "challengeId");
      const int64_t seconds = value.is_object() && value.contains("expiresIn") && value.at("expiresIn").is_number_unsigned() ?
          static_cast<int64_t>(std::min<uint64_t>(86400, value.at("expiresIn").get<uint64_t>())) : 0;
      expires_at_ = now() + seconds * 1000; resend_at_ = now() + account::resend_interval_ms;
      if (seconds <= 0) challenge_.clear();
      notice_ = challenge_.empty() ? L"无法取得验证码，请稍后重试。" : L"验证码已发送，请查收。";
    } else if (operation == "status" || operation == "login" || operation == "google_login") {
      const auto user = value.is_object() ? value.value("user", Json{}) : Json{};
      unreadable_ = false;
      signed_in_ = user.is_object() && user.contains("id") && user.at("id").is_string();
      const auto id = signed_in_ ? string(user, "id") : std::string{};
      if (id != user_id_) { cloud_.reset(); sync_ready_ = false; profile_text_.clear(); sync_notice_.clear(); avatar({}); management_.clear(); }
      user_id_ = id;
      user_name_ = signed_in_ ? control_text::wide(string(user, "displayName")) : std::wstring{};
      name_ = user_name_;
      if (operation != "status") notice_ = signed_in_ ? L"登录成功。" : L"当前未登录。";
      if (operation == "login" || operation == "google_login") { challenge_.clear(); code_.clear(); }
      if (signed_in_) next = Json{{"operation", "profile"}};
    } else if (operation == "profile" || operation == "rename" || operation == "upload_avatar" || operation == "remove_avatar") {
      const auto user = value.is_object() ? value.value("user", Json{}) : Json{};
      if (!user.is_object() || string(user, "id") != user_id_) {
        notice_ = L"账号资料无法读取，请刷新登录状态后重试。"; refresh(); return std::nullopt;
      }
      account::ProviderNames names;
      bool valid_names = value.contains("providerNames") && value.at("providerNames").is_object();
      if (valid_names) for (const auto& entry : value.at("providerNames").items()) {
        if (!entry.value().is_string() || entry.value().get<std::string>().empty()) { valid_names = false; break; }
        names.emplace(control_text::wide(entry.key()), control_text::wide(entry.value().get<std::string>()));
      }
      if (valid_names && value.contains("providers") && value.at("providers").is_array())
        for (const auto& item : value.at("providers"))
          if (item.is_string() && names.find(control_text::wide(item.get<std::string>())) == names.end()) valid_names = false;
      if (!valid_names) {
        notice_ = L"账号资料无法读取，请刷新登录状态后重试。"; refresh(); return std::nullopt;
      }
      user_name_ = control_text::wide(string(user, "displayName")); name_ = user_name_;
      avatar_uploaded_ = flag(user, "avatarUploaded");
      const auto avatar_url = string(user, "avatarUrl");
      if (avatar_url != avatar_url_) { avatar({}); avatar_url_ = avatar_url; }
      profile_text_ = L"账号 ID：" + control_text::wide(user_id_) + L"\r\n邮箱：" + control_text::wide(string(user, "email")) +
          L"\r\n加入时间：" + control_text::wide(string(user, "createdAt")) + L"\r\n登录方式：";
      if (value.contains("providers") && value.at("providers").is_array()) for (const auto& item : value.at("providers"))
        if (item.is_string()) profile_text_ += account::provider_name(control_text::wide(item.get<std::string>()), names) + L"　";
      if (operation == "rename") notice_ = L"昵称已更新。";
      else {
        if (operation == "upload_avatar") notice_ = L"头像已更新。";
        if (operation == "remove_avatar") notice_ = L"已移除上传的头像。";
        sync_ready_ = false; next = Json{{"operation", "preferences_schema"}};
      }
    } else if (operation == "preferences_schema") {
      sync_ready_ = false;
      next = Json{{"operation", "preferences_load"}};
    } else if (operation == "preferences_load" || operation == "preferences_upload") {
      if (!value.is_object() || !value.contains("revision") || !value.at("revision").is_number_unsigned() ||
          !value.contains("settings") || !value.at("settings").is_object()) {
        sync_notice_ = L"无法读取云端设置，请刷新后重试。"; sync_ready_ = false;
      } else {
        cloud_ = value; sync_ready_ = true;
        sync_notice_ = L"云端版本：" + std::to_wstring(value.at("revision").get<uint64_t>());
        if (operation == "preferences_upload") sync_notice_ += L"\r\n" + std::wstring(account::sync_uploaded);
        if (operation == "preferences_load" && !avatar_url_.empty()) next = Json{{"operation", "avatar"}};
      }
    } else if (operation == "preferences_apply") sync_notice_ = account::sync_applied;
    else if (operation == "logout" || operation == "forget" || operation == "account_delete") {
      signed_in_ = false; unreadable_ = false; user_id_.clear(); user_name_.clear(); name_.clear(); profile_text_.clear();
      cloud_.reset(); sync_ready_ = false; sync_notice_.clear(); challenge_.clear(); code_.clear();
      avatar({}); avatar_url_.clear(); avatar_uploaded_ = false;
      management_.clear();
      if (operation == "account_delete") management_.account_deleted(value);
      notice_ = operation == "account_delete" ? L"账号已注销。" : operation == "forget" ? L"失效的本机登录状态已清除。" : L"已退出登录。";
    }
    refresh();
    return next;
  }

  Json status_request() const { return Json{{"operation", "status"}}; }
  void tick() { update(); }

private:
  HWND get(int id) const { const auto it = controls_.find(id); return it == controls_.end() ? nullptr : it->second; }
  static bool flag(const Json& value, const char* key) { return value.is_object() && value.contains(key) && value.at(key) == true; }
  static std::string string(const Json& value, const char* key) { return value.is_object() && value.contains(key) && value.at(key).is_string() ? value.at(key).get<std::string>() : std::string{}; }
  static int64_t now() { return std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now().time_since_epoch()).count(); }
  static std::string narrow(std::wstring_view value) {
    const auto utf8 = control_text::utf8(value);
    return utf8.value_or(std::string{});
  }
  void update() {
    management_.enable(enabled_, signed_in_);
    SetWindowTextW(get(account_google), google_waiting_ ? L"正在等待浏览器完成 Google 登录…" : L"使用 Google 登录");
    if (!enabled_) {
      for (const auto& entry : controls_) EnableWindow(entry.second, FALSE);
      EnableWindow(get(account_google_cancel), google_waiting_);
      return;
    }
    EnableWindow(get(account_refresh), TRUE);
    EnableWindow(get(account_status), TRUE); EnableWindow(get(account_notice), TRUE);
    EnableWindow(get(account_email), !signed_in_ && provider_email_);
    EnableWindow(get(account_phone), !signed_in_ && provider_phone_);
    EnableWindow(get(account_target), !signed_in_ && !channel_.empty());
    SetWindowTextW(get(account_request_code), account::resend_label(account::resend_seconds(resend_at_ - now())).c_str());
    SetWindowTextW(get(account_login), account::login_label(!challenge_.empty() && expires_at_ <= now(), false).c_str());
    EnableWindow(get(account_request_code), !signed_in_ && !channel_.empty() && !account::trim_login_target(target_).empty() && resend_at_ <= now());
    EnableWindow(get(account_code), !signed_in_ && !challenge_.empty());
    EnableWindow(get(account_login), !signed_in_ && !challenge_.empty() && expires_at_ > now() && account::six_digits(code_));
    EnableWindow(get(account_google), !signed_in_ && provider_google_);
    EnableWindow(get(account_google_cancel), google_waiting_);
    EnableWindow(get(account_logout), signed_in_);
    EnableWindow(get(account_logout_all), signed_in_);
    for (int id : {account_avatar, account_avatar_load, account_avatar_upload}) EnableWindow(get(id), signed_in_);
    EnableWindow(get(account_avatar_remove), signed_in_ && avatar_uploaded_);
    for (int id : {account_profile, account_profile_text, account_name, account_rename, account_sync_load, account_sync_notice, account_delete}) EnableWindow(get(id), signed_in_);
    EnableWindow(get(account_forget), unreadable_);
    EnableWindow(get(account_sync_upload), signed_in_ && sync_ready_ && cloud_.has_value());
    EnableWindow(get(account_sync_apply), signed_in_ && sync_ready_ && cloud_.has_value() && !cloud_->at("settings").empty());
  }

  std::map<int, HWND> controls_;
  std::wstring channel_, target_, code_, user_name_, notice_;
  std::string challenge_;
  bool enabled_ = false, updating_ = false, signed_in_ = false;
  bool google_waiting_ = false;
  bool provider_email_ = false, provider_phone_ = false, provider_google_ = false;
  bool unreadable_ = false, sync_ready_ = false;
  std::string user_id_;
  std::wstring name_, profile_text_, sync_notice_;
  std::optional<Json> cloud_;
  theme_data::Image avatar_;
  std::string avatar_url_;
  bool avatar_uploaded_ = false;
  AccountManagementForm management_;
  int64_t expires_at_ = 0, resend_at_ = 0;
};

} // namespace msime::settings::win7
