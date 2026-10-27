#pragma once

#include "../CloudClipboardRoute.h"
#include "../DictionarySyncModel.h"
#include "ControlText.h"
#include "PreferencesStore.h"
#include "ThemeData.h"
#include <commctrl.h>
#include <functional>
#include <map>
#include <optional>
#include <vector>

namespace msime::settings::win7 {

// Win7 词库同步面板（panels/win7/DictionarySyncWindow.cpp）的控件与动作：「我的词条」经 WebDAV 网盘同步（docs/dictionary-webdav-sync.md）。网盘配置照云剪贴板面板的 WebDAV 那一套：应用密码遮罩、从不回填（留空沿用已存的），「测试连接」只测不存，「测试并保存」先测、通过了才写偏好 dictionary_sync_webdav，不通过时输入留着。存着网盘时打开面板以 opened 同步一次并取设备，之后「立即同步」、被挡下的大批删除、设备的允许、拒绝、移除和重置都在这里。自动的 periodic 和候选窗口删词后的 local_change 由 Server 做（src/system/DictionarySyncWorker.h）。窗口只持有展示和动作，网络、加密、额度和设备身份都在 worker 与共享 Rust；文字与 WinUI 设置共用 DictionarySyncModel.h。
class DictionarySyncForm {
public:
  enum Id { url = 3800, username, password, test, save, clear, config, sync_now, status, held, confirm_deletions,
      restore_deletions, devices, device_detail, approve, reject, remove, reset, restart, trust_remote, notice };
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    // {probe: 草稿, save} 或者 {action, devices?, status?}，见 DictionarySyncWorker.h 的 run_dictionary_sync。
    std::optional<Json> request;
    std::wstring confirmation;
  };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = control;
      top += height + 12;
      return control;
    };
    add(-1, L"STATIC", std::wstring(dictionary_sync::explanation).c_str(), SS_LEFT, 118);
    add(-1, L"STATIC", (L"WebDAV 网盘服务器地址（例如 " + std::wstring(webdav_url_placeholder) + L"，水杉会在里面自动建 msime-dictionary 文件夹）").c_str(), SS_LEFT, 44);
    add(url, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    add(-1, L"STATIC", L"网盘账号（坚果云是登录邮箱，可以和云剪贴板用同一个账号）", SS_LEFT, 42);
    add(username, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    add(-1, L"STATIC", L"应用密码（不是登录密码；留空沿用已保存的应用密码）", SS_LEFT, 42);
    add(password, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL | ES_PASSWORD);
    SendMessageW(get(url), EM_SETLIMITTEXT, 4096, 0);
    SendMessageW(get(username), EM_SETLIMITTEXT, 1024, 0);
    SendMessageW(get(password), EM_SETLIMITTEXT, 1024, 0);
    add(test, L"BUTTON", L"测试连接", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(save, L"BUTTON", L"测试并保存网盘配置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clear, L"BUTTON", L"停止同步…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(config, L"STATIC", L"", SS_LEFT, 62);
    add(sync_now, L"BUTTON", L"立即同步", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(status, L"STATIC", L"", SS_LEFT, 132);
    add(held, L"STATIC", L"", SS_LEFT, 80);
    add(confirm_deletions, L"BUTTON", L"同步删除…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(restore_deletions, L"BUTTON", L"从网盘恢复", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", std::wstring(dictionary_sync::waiting_note).c_str(), SS_LEFT, 52);
    add(devices, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 140);
    add(device_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY, 72);
    add(approve, L"BUTTON", L"核对一致，允许此设备…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(reject, L"BUTTON", L"拒绝此设备…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(remove, L"BUTTON", L"移除此设备并更换密钥…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(reset, L"BUTTON", L"重置词库同步密钥…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(restart, L"BUTTON", L"以本机为准重新开始…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(trust_remote, L"BUTTON", L"确认网盘现状并恢复同步…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(notice, L"STATIC", L"", SS_LEFT, 98);
    return top;
  }

  // 偏好变了：只在存着的配置变了时重填输入框，保存失败时用户填的内容保留；应用密码框从不回填。配置变了时读到的状态和设备都作废。
  void refresh(const Snapshot& snapshot) {
    const auto& preferences = snapshot.preferences();
    auto webdav = preferences.value("dictionary_sync_webdav", Json::object());
    if (!webdav.is_object()) webdav = Json::object();
    if (!loaded_ || webdav != webdav_) {
      webdav_ = webdav;
      SetWindowTextW(get(url), control_text::wide(webdav.value("url", std::string{})).c_str());
      SetWindowTextW(get(username), control_text::wide(webdav.value("username", std::string{})).c_str());
      SetWindowTextW(get(password), L"");
      forget();
    }
    loaded_ = true;
    const auto saved = configured() ? dictionary_sync::saved_note(control_text::wide(webdav_.value("url", std::string{}))) :
        std::wstring(L"还没有配置词库同步。填好网盘地址、账号和应用密码后点「测试并保存网盘配置」。");
    SetWindowTextW(get(config), saved.c_str());
    update();
  }
  bool configured() const { return !webdav_.value("url", std::string{}).empty(); }
  // 打开面板或保存了新的网盘配置之后：存着网盘时以 opened 同步一次，再取设备和概况。
  std::optional<Json> opened_request() const {
    if (!configured()) return std::nullopt;
    return Json{{"action", {{"operation", "run"}, {"trigger", "opened"}}}, {"devices", true}, {"status", true}};
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }

  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command command; command.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return command;
    if (id == devices && notification == LBN_SELCHANGE) { device_text(); update(); return command; }
    if (notification != BN_CLICKED) return command;
    if (id == test || id == save) {
      const auto address = control_text::read(get(url), 4096), account = control_text::read(get(username), 1024),
          secret = control_text::read(get(password), 1024);
      if (!address || !account || !secret) { SetWindowTextW(get(notice), L"地址、账号或应用密码无法读取。"); return command; }
      // 地址和凭据的规则只在 Rust 里：不合规时测试连接答 dictionary_sync_not_configured，不在这里另判一遍。
      const Json draft{{"url", *address}, {"username", *account}, {"password", secret->empty() ? webdav_.value("password", std::string{}) : *secret}};
      const bool saving = id == save;
      command.request = Json{{"probe", draft}, {"save", saving}};
      // 和已存的一样时只重新测试；普通探测不改同步状态。
      if (saving && draft != webdav_) command.changes.push_back({{"dictionary_sync_webdav"}, draft});
      SetWindowTextW(get(notice), saving ? L"正在测试连接，通过后保存…" : L"正在测试连接…");
    } else if (id == clear) {
      if (configured()) {
        command.changes.push_back({{"dictionary_sync_webdav"}, Json{{"url", ""}, {"username", ""}, {"password", ""}}});
        command.confirmation = std::wstring(dictionary_sync::clear_question);
      }
    } else if (id == sync_now || id == confirm_deletions || id == restore_deletions) {
      Json action{{"operation", "run"}, {"trigger", "manual"}};
      if (id == confirm_deletions) { action["confirm_deletions"] = true; command.confirmation = std::wstring(dictionary_sync::confirm_deletions_question); }
      if (id == restore_deletions) action["restore_deletions"] = true;
      command.request = Json{{"action", action}, {"devices", true}};
    } else if (id == reset) {
      command.request = Json{{"action", {{"operation", "reset"}}}};
      command.confirmation = std::wstring(dictionary_sync::reset_question);
    } else if (id == trust_remote) {
      Json action = webdav_;
      action["operation"] = "probe";
      action["trust_remote"] = true;
      command.request = Json{{"action", action}, {"status", true}};
      command.confirmation = std::wstring(dictionary_sync::trust_remote_question);
    } else if (id == restart) {
      command.request = Json{{"action", {{"operation", "restart"}}}};
      command.confirmation = std::wstring(dictionary_sync::restart_question);
    } else if (const auto* device = selected(); device && (id == approve || id == reject || id == remove)) {
      const auto device_id = control_text::utf8(device->id).value_or("");
      const auto operation = id == approve ? "approve" : id == reject ? "reject" : "remove";
      command.request = Json{{"action", {{"operation", operation}, {"device_id", device_id}}}, {"status", id != remove}};
      command.confirmation = id == approve ? dictionary_sync::approve_question(*device) :
          id == reject ? dictionary_sync::reject_question(*device) : dictionary_sync::remove_question(*device);
    }
    return command;
  }

  // 「测试连接」或「测试并保存」的结果：worker 只在测试通过时才写配置，completion 里的 saved 说明写成了没有。
  void probed(const Json& request, const Json& completion) {
    const auto response = completion.find("probe");
    std::wstring message;
    if (response == completion.end()) message = dictionary_sync::error_message(L"", true);
    else if (theme_data::value(*response)) {
      if (!request.value("save", false)) message = dictionary_sync::probe_passed;
      else message = completion.value("saved", false) ? std::wstring(dictionary_sync::saved) : L"连接正常，但配置没有保存，请重新打开面板后重试。";
      // 测试通过后重新读取展示状态；普通探测不修改同步位置或待发改动。
      forget();
    } else message = dictionary_sync::error_message(error_code(*response), true);
    SetWindowTextW(get(notice), message.c_str());
    update();
  }

  // 一件操作的结果（DictionarySyncWorker.h 的 run_dictionary_sync）：按动作换上概况和设备，失败时说明原因。
  void completed(const Json& request, const Json& completion) {
    const auto operation = request.at("action").value("operation", std::string{});
    const auto response = completion.value("action", Json());
    const auto value = theme_data::value(response);
    std::wstring message;
    if (!value) message = dictionary_sync::error_message(error_code(response));
    else if (operation == "run") message = run(*value, request.at("action").value("trigger", std::string{}) == "manual");
    else if (operation == "approve" || operation == "reject") { if (!devices_from(*value)) message = dictionary_sync::error_message(L""); }
    else if (operation == "remove" || operation == "reset") {
      devices_from(value->value("devices", Json()));
      message = run(value->value("run", Json()), false);
    } else if (operation == "probe") {
      message = L"已确认网盘现状，请点「立即同步」恢复。";
    } else if (operation == "restart") {
      if (const auto parsed = status_from(*value)) { status_ = *parsed; message = dictionary_sync::restarted; }
      else message = dictionary_sync::error_message(L"");
    }
    if (const auto devices_response = completion.find("devices"); devices_response != completion.end()) {
      if (const auto listed = theme_data::value(*devices_response)) devices_from(*listed);
      else if (message.empty()) message = dictionary_sync::error_message(error_code(*devices_response));
    }
    if (const auto status_response = completion.find("status"); status_response != completion.end())
      if (const auto read = theme_data::value(*status_response))
        if (const auto parsed = status_from(*read)) status_ = *parsed;
    if (message.empty() && operation != "run") message = L"操作已完成。";
    SetWindowTextW(get(notice), message.c_str());
    show();
  }

  // 本机还没有 HostOptions（设置向导没跑完）时。
  void unprepared() { SetWindowTextW(get(notice), L"输入法尚未完成初始化，完成设置向导后即可同步词库。"); update(); }

private:
  static std::wstring error_code(const Json& response) {
    return response.is_object() && response.contains("error") && response.at("error").is_string() ?
        control_text::wide(response.at("error").get<std::string>()) : std::wstring();
  }
  static uint64_t count(const Json& object, const char* key) {
    const auto value = object.find(key);
    return value != object.end() && value->is_number_unsigned() ? value->get<uint64_t>() : 0;
  }
  static std::wstring text(const Json& object, const char* key) {
    const auto value = object.find(key);
    return value != object.end() && value->is_string() ? control_text::wide(value->get<std::string>()) : std::wstring();
  }
  static bool flag(const Json& object, const char* key) {
    const auto value = object.find(key);
    return value != object.end() && value->is_boolean() && value->get<bool>();
  }
  // 与共享页的 dictionarySyncStatus 一样：approved 和 joined 不是布尔值时整个不认。
  static std::optional<dictionary_sync::Status> status_from(const Json& value) {
    if (!value.is_object() || !value.contains("approved") || !value.at("approved").is_boolean() ||
        !value.contains("joined") || !value.at("joined").is_boolean()) return std::nullopt;
    dictionary_sync::Status result;
    result.device_id = text(value, "device_id"); result.code = text(value, "code");
    result.approved = value.at("approved").get<bool>(); result.joined = value.at("joined").get<bool>();
    result.words = count(value, "words"); result.pending_upload = count(value, "pending_upload");
    result.pending_apply = count(value, "pending_apply"); result.held_deletions = count(value, "held_deletions");
    result.clock_skew = flag(value, "clock_skew"); result.last_success_ms = count(value, "last_success_ms");
    result.last_error = text(value, "last_error");
    return result;
  }
  // 设备列表按编号每台只留一条（dictionary_sync::group_devices），等待允许的排在前面。
  bool devices_from(const Json& value) {
    if (!value.is_object() || !value.contains("devices") || !value.at("devices").is_array()) return false;
    std::vector<dictionary_sync::Device> listed;
    for (const auto& item : value.at("devices")) {
      if (!item.is_object()) continue;
      dictionary_sync::Device device;
      device.id = text(item, "device_id"); device.name = text(item, "name"); device.created_at = text(item, "created_at");
      device.code = text(item, "code"); device.approved = flag(item, "approved"); device.current = flag(item, "current");
      if (!device.id.empty()) listed.push_back(std::move(device));
    }
    auto groups = dictionary_sync::group_devices(listed);
    devices_ = std::move(groups.waiting);
    waiting_ = devices_.size();
    for (auto& device : groups.approved) devices_.push_back(std::move(device));
    return true;
  }
  // 一次 run 的结果：换上概况，返回要说的一句（失败的原因，或者手动同步的结果）。
  std::wstring run(const Json& value, bool manual) {
    const auto parsed = value.is_object() ? status_from(value.value("status", Json())) : std::nullopt;
    if (!parsed) return dictionary_sync::error_message(L"dictionary_sync_unavailable");
    status_ = *parsed;
    const auto report = value.find("report");
    if (report == value.end() || !report->is_object()) {
      const auto code = text(value, "error");
      return dictionary_sync::error_message(code.empty() ? L"dictionary_sync_unavailable" : code);
    }
    return manual ? dictionary_sync::run_notice(status_->approved, flag(*report, "deferred")) : std::wstring();
  }
  void forget() { status_.reset(); devices_.clear(); waiting_ = 0; show(); }
  // 把读到的概况和设备画出来。
  void show() {
    std::wstring summary;
    if (status_) {
      summary = dictionary_sync::approval_text(*status_);
      if (!status_->code.empty()) summary += L"\r\n本设备核对码：" + status_->code;
      summary += L"\r\n" + dictionary_sync::counts_text(*status_);
      if (!status_->last_error.empty()) summary += L"\r\n上次出错：" + dictionary_sync::error_message(status_->last_error);
      if (status_->clock_skew) summary += L"\r\n" + std::wstring(dictionary_sync::clock_skew_note);
    } else summary = configured() ? L"点「立即同步」读取同步状态。" : L"";
    SetWindowTextW(get(status), summary.c_str());
    SetWindowTextW(get(held), status_ && status_->held_deletions ? dictionary_sync::held_deletions_text(status_->held_deletions).c_str() : L"");
    SendMessageW(get(devices), LB_RESETCONTENT, 0, 0);
    for (size_t index = 0; index < devices_.size(); ++index) {
      const auto label = dictionary_sync::device_label(devices_[index]) + (index < waiting_ ? L" [等待允许]" : L" [已获准]");
      SendMessageW(get(devices), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
    SetWindowTextW(get(device_detail), L"");
    update();
  }
  const dictionary_sync::Device* selected() const {
    const auto index = SendMessageW(get(devices), LB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < devices_.size() ? &devices_[static_cast<size_t>(index)] : nullptr;
  }
  bool selected_waiting() const {
    const auto index = SendMessageW(get(devices), LB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < waiting_;
  }
  void device_text() {
    const auto* device = selected();
    SetWindowTextW(get(device_detail), device ? (dictionary_sync::device_label(*device) + L"\r\n" + dictionary_sync::device_detail(*device)).c_str() : L"");
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_);
    const bool saved = enabled_ && configured();
    for (int id : {clear, sync_now, reset, restart, trust_remote, devices, device_detail}) EnableWindow(get(id), saved);
    const bool holding = saved && status_ && status_->held_deletions;
    for (int id : {confirm_deletions, restore_deletions}) EnableWindow(get(id), holding);
    const auto* device = selected();
    const bool waiting = saved && device && selected_waiting();
    EnableWindow(get(approve), waiting);
    EnableWindow(get(reject), waiting);
    EnableWindow(get(remove), saved && device && !selected_waiting() && !device->current);
  }
  std::map<int, HWND> controls_;
  Json webdav_ = Json::object();
  std::optional<dictionary_sync::Status> status_;
  // 先是等待允许的 waiting_ 台，后面是已获准的（含本机）。
  std::vector<dictionary_sync::Device> devices_;
  size_t waiting_ = 0;
  bool enabled_ = false, loaded_ = false;
};
} // namespace msime::settings::win7
