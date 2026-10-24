#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "ThemeData.h"
#include "../AboutPageModel.h"
#include "../FeedbackReport.h"
#include "../../common/ClientAgent.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <commctrl.h>
#include <cctype>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class AboutForm {
public:
  enum Id { version = 2700, check_update, update_status, update_detail, download, mirror, system_info, copy_system,
    usage_reporting, license, privacy, notices, help, feedback_kind, feedback_detail, feedback_preview,
    copy_feedback, submit_feedback, github, telegram, notice };
  struct Command { bool handled = false, update = false, notices = false; std::vector<Change> changes; std::optional<std::wstring> link, copy; };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, std::string app_version) {
    app_version_ = std::move(app_version); int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height); if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    const auto button = [&](int id, const wchar_t* title) { add(id, L"BUTTON", title, WS_TABSTOP | BS_PUSHBUTTON, 32); };
    add(-1, L"STATIC", MSIME_EDITION_DISPLAY_NAME L" · Windows 7 支持", SS_LEFT, 36);
    add(version, L"STATIC", (L"当前版本 v" + control_text::wide(app_version_)).c_str(), SS_LEFT);
    button(check_update, L"检查 Windows 7 更新");
    add(update_status, L"STATIC", L"只检查本版本的 Windows 7 专用安装包。", SS_LEFT, 56);
    add(update_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 160);
    button(download, L"打开所选 Windows 7 发行页"); button(mirror, L"打开所选 Windows 7 安装包镜像");
    add(-1, L"STATIC", L"系统信息（不包含账号、输入内容或用户目录）", SS_LEFT, 48);
    add(system_info, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 140);
    button(copy_system, L"复制系统信息");
    add(-1, L"STATIC", L"许可与隐私", SS_LEFT);
    button(license, L"查看 GPL-3.0 开源许可"); button(notices, L"打开随包第三方组件许可"); button(privacy, L"查看隐私政策");
    add(usage_reporting, L"BUTTON", L"匿名使用统计", WS_TABSTOP | BS_AUTOCHECKBOX);
    add(-1, L"STATIC", L"每天最多一条活跃记录；进程退出或崩溃后发送会话记录，崩溃时附错误摘要和仅含模块文件名的调用栈。包含随机事件 id、平台、版本和本机随机安装 id；不含输入内容、候选、剪贴板、账号、硬件信息或用户目录。默认开启，可关闭；失败最多暂存 64 条，关闭后不再发送并清空待发送记录。发送到 https://api.msime.app/v1/telemetry/events。", SS_LEFT, 180);
    add(-1, L"STATIC", L"帮助与反馈", SS_LEFT);
    add(help, L"EDIT", L"在 Windows 语言栏选择本输入法。Shift 切换中英文，Space 或数字键选择候选，Esc 取消。翻页快捷键以「快捷键 / 按键速查」中的当前设置为准。\r\n\r\n高亮候选有释义时，Tab / Shift+Tab 切换释义列；没有释义时翻页。Alt 或 Ctrl 加数字键分别使用候选释义列。\r\n\r\n遇到问题时先检查语言栏和当前方案，打开「维护与诊断」读取日志；提交报告前检查并移除私人内容。", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 200);
    add(feedback_kind, WC_COMBOBOXW, L"反馈类型", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 220);
    for (const auto kind : feedback_kinds) SendMessageW(get(feedback_kind), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(kind.data()));
    SendMessageW(get(feedback_kind), CB_SETCURSEL, 0, 0);
    add(feedback_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_WANTRETURN | WS_VSCROLL, 140);
    SendMessageW(get(feedback_detail), EM_SETLIMITTEXT, feedback_text_limit, 0);
    add(feedback_preview, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 180);
    button(copy_feedback, L"复制问题报告"); button(submit_feedback, L"在浏览器填写 GitHub issue");
    button(github, L"打开 GitHub 问题列表"); button(telegram, L"打开 Telegram 群组");
    add(-1, L"STATIC", (L"QQ 群：" + std::wstring(feedback_qq_group)).c_str(), SS_LEFT);
    add(notice, L"STATIC", L"", SS_LEFT, 64); refresh_report(); return top;
  }
  void enable(bool enabled) {
    enabled_ = enabled; for (const auto& item : controls_) EnableWindow(item.second, enabled);
    EnableWindow(get(download), enabled && !release_url_.empty()); EnableWindow(get(mirror), enabled && !mirror_url_.empty());
  }
  void refresh(const Json& preferences) {
    scheme_ = preferences.value("scheme", std::string{});
    SendMessageW(get(usage_reporting), BM_SETCHECK, preferences.value("usage_reporting", true) ? BST_CHECKED : BST_UNCHECKED, 0);
    std::wstring system = control_text::wide(msime::windows::windows_system_description());
    if (system.substr(0, 8) == L"Windows ") system.erase(0, 8);
    system_version_ = system;
    const auto entries = system_info_entries(control_text::wide(app_version_), MSIME_EDITION_IS_FULL ? L"" : MSIME_EDITION_DISPLAY_NAME,
        L"", system, build_architecture(), feedback_scheme_title(control_text::wide(scheme_)));
    system_text_ = system_info_text(entries); SetWindowTextW(get(system_info), system_text_.c_str()); refresh_report();
  }
  void completed(const Json& response) {
    release_url_.clear(); mirror_url_.clear(); SetWindowTextW(get(update_detail), L"");
    const auto fail = [this] { SetWindowTextW(get(update_status), L"检查失败，请稍后重试；不会提供普通 Windows 安装包。"); enable(enabled_); };
    const auto value = theme_data::value(response);
    if (!value || !value->is_object() || !value->contains("status") || !value->at("status").is_string()) return fail();
    const auto state = value->at("status").get<std::string>();
    if (state == "none") { SetWindowTextW(get(update_status), L"暂无可用的 Windows 7 发行版。"); enable(enabled_); return; }
    if ((state != "available" && state != "current") || !value->contains("update") || !value->at("update").is_object()) return fail();
    const auto& update = value->at("update");
    const auto text = [&](const char* field) { return update.contains(field) && update.at(field).is_string() ? update.at(field).get<std::string>() : std::string{}; };
    if (!update.contains("version") || !update.at("version").is_object() || !update.at("version").contains("display") || !update.at("version").at("display").is_string()) return fail();
    const auto next = update.at("version").at("display").get<std::string>();
    if (next.empty() || next.size() > 64 || next.find_first_not_of("0123456789.") != std::string::npos) return fail();
    std::string edition = MSIME_EDITION_ID; if (!edition.empty()) edition[0] = static_cast<char>(std::toupper(static_cast<unsigned char>(edition[0])));
    const auto architecture = build_architecture() == L"x86_64" ? "x64" : "x86";
    const auto expected = "MetasequoiaIME-" + edition + "_Setup_v" + next + "_win7_" + architecture + ".exe";
    auto name = text("installer_name"), url = text("release_url");
    auto lowered = name; auto expected_lower = expected;
    std::transform(lowered.begin(), lowered.end(), lowered.begin(), [](unsigned char ch) { return static_cast<char>(std::tolower(ch)); });
    std::transform(expected_lower.begin(), expected_lower.end(), expected_lower.begin(), [](unsigned char ch) { return static_cast<char>(std::tolower(ch)); });
    if (lowered != expected_lower || url != "https://github.com/metasequoiaime/msime/releases/tag/windows7-v" + next) return fail();
    if (state == "current") { SetWindowTextW(get(update_status), L"已是最新的 Windows 7 版本。"); enable(enabled_); return; }
    const auto sha = control_text::wide(text("installer_sha256"));
    if (!sha.empty() && !installer_sha256_valid(sha)) return fail();
    release_url_ = control_text::wide(url); mirror_url_ = update_mirror_url(release_url_, control_text::wide(name));
    std::wstring detail = L"安装包：" + control_text::wide(name);
    detail += L"\r\n请从发行页核对代码签名和安装包摘要。";
    if (!sha.empty()) detail += L"\r\nSHA-256：" + sha;
    detail += L"\r\n校验命令：certutil -hashfile .\\" + control_text::wide(name) + L" SHA256";
    if (update.contains("signed") && update.at("signed").is_boolean() && !update.at("signed").get<bool>())
      detail += L"\r\n该安装包未经代码签名，可能被系统拦截，uiAccess 也无法生效。";
    SetWindowTextW(get(update_detail), detail.c_str());
    SetWindowTextW(get(update_status), (L"发现 Windows 7 新版本 v" + control_text::wide(next)).c_str()); enable(enabled_);
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event); if (!controls_.count(id)) return {};
    Command action; action.handled = true; if (!enabled_ || !IsWindowEnabled(get(id))) return action;
    if ((id == feedback_detail && notification == EN_CHANGE) || (id == feedback_kind && notification == CBN_SELCHANGE)) { refresh_report(); return action; }
    if (notification != BN_CLICKED) return action;
    if (id == check_update) { release_url_.clear(); mirror_url_.clear(); SetWindowTextW(get(update_detail), L""); SetWindowTextW(get(update_status), L"正在检查 Windows 7 更新…"); action.update = true; }
    else if (id == download && !release_url_.empty()) action.link = release_url_;
    else if (id == mirror && !mirror_url_.empty()) action.link = mirror_url_;
    else if (id == copy_system) action.copy = system_text_;
    else if (id == copy_feedback) { refresh_report(); action.copy = report_; }
    else if (id == submit_feedback) { refresh_report(); action.link = feedback_issue_url(feedback_issues_url, selected_kind(), report_); }
    else if (id == github) action.link = std::wstring(feedback_issues_url);
    else if (id == telegram) action.link = std::wstring(feedback_telegram_url);
    else if (id == license) action.link = L"https://github.com/metasequoiaime/msime/blob/develop/LICENSE";
    else if (id == privacy) action.link = L"https://msime.app/privacy/";
    else if (id == notices) action.notices = true;
    else if (id == usage_reporting) action.changes.push_back({{"usage_reporting"}, SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED});
    return action;
  }
  void copied(bool success) { SetWindowTextW(get(notice), success ? L"已复制。" : L"复制失败，请重试。"); }
private:
  HWND get(int id) const { const auto value = controls_.find(id); return value == controls_.end() ? nullptr : value->second; }
  std::wstring_view selected_kind() const { const auto index = SendMessageW(get(feedback_kind), CB_GETCURSEL, 0, 0); return index >= 0 && static_cast<size_t>(index) < feedback_kinds.size() ? feedback_kinds[static_cast<size_t>(index)] : feedback_kinds.front(); }
  void refresh_report() {
    const auto detail = control_text::read(get(feedback_detail), feedback_text_limit);
    report_ = feedback_report(selected_kind(), feedback_normalize_newlines(detail ? control_text::wide(*detail) : L""),
        feedback_diagnostics(control_text::wide(app_version_), system_version_, feedback_scheme_title(control_text::wide(scheme_))));
    SetWindowTextW(get(feedback_preview), report_.c_str());
  }
  std::map<int, HWND> controls_;
  std::string app_version_, scheme_;
  std::wstring release_url_, mirror_url_, system_text_, system_version_, report_;
  bool enabled_ = false;
};
} // namespace msime::settings::win7
