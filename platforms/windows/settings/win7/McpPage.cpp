#include "McpPage.h"
#include "../McpConnect.h"
#include "PreferencesStore.h"
#include "../../common/StateDirectory.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <algorithm>
#include <array>
#include <filesystem>
#include <map>
#include <optional>
#include <thread>
#include <stdexcept>
#include <utility>

namespace msime::settings::win7 {
namespace {
namespace mc = msime::settings::mcp;
constexpr UINT completed = WM_APP + 91;
constexpr wchar_t page_class[] = L"MSIME.Windows7Settings.Mcp" MSIME_EDITION_NAME_SUFFIX;
std::wstring wide(std::string_view value) {
  const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), nullptr, 0);
  std::wstring result(static_cast<size_t>(std::max(length, 0)), L'\0');
  if (length) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), result.data(), length);
  return result;
}
std::string utf8(std::wstring_view value) {
  const int length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
  std::string result(static_cast<size_t>(std::max(length, 0)), '\0');
  if (length) WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), result.data(), length, nullptr, nullptr);
  return result;
}
std::string call(char* (*api)(const uint8_t*, size_t), const std::string& request) {
  auto owned = msime::host_api::own_string(api(reinterpret_cast<const uint8_t*>(request.data()), request.size()));
  return owned ? std::string(owned.get()) : std::string{};
}
std::optional<Json> value(const Json& response) {
  if (!response.is_object() || !response.contains("ok") || response.at("ok") != true || !response.contains("value")) return {};
  return response.at("value");
}
Json options_request() {
  // 只传存在的绝对选项路径；目录解析与文件系统查询均在工作线程进行。
  std::error_code error;
  const auto size = GetEnvironmentVariableW(L"MSIME_CLIENT_HOST_OPTIONS", nullptr, 0);
  if (size) {
    std::wstring text(size, L'\0');
    const auto length = GetEnvironmentVariableW(L"MSIME_CLIENT_HOST_OPTIONS", text.data(), size);
    if (length && length < size) {
      text.resize(length); const std::filesystem::path path(text);
      if (path.is_absolute() && std::filesystem::is_regular_file(path, error)) return {{"options", path.u8string()}};
    }
  }
  const auto root = msime::windows::resolve_state_directory();
  const auto path = root / L"runtime-options.json";
  if (!root.empty() && path.is_absolute() && std::filesystem::is_regular_file(path, error)) return {{"options", path.u8string()}};
  return {{"options", nullptr}};
}
uint32_t load_flags() {
  DWORD bits = 0, size = sizeof(bits);
  if (RegGetValueW(HKEY_CURRENT_USER, MSIME_EDITION_REGISTRY_KEY, L"McpFlags", RRF_RT_REG_DWORD, nullptr, &bits, &size) != ERROR_SUCCESS)
    return mc::all_flag_bits;
  return bits;
}
void save_flags(uint32_t bits) {
  HKEY key = nullptr;
  if (RegCreateKeyExW(HKEY_CURRENT_USER, MSIME_EDITION_REGISTRY_KEY, 0, nullptr, 0, KEY_SET_VALUE, nullptr, &key, nullptr) != ERROR_SUCCESS) return;
  RegSetValueExW(key, L"McpFlags", 0, REG_DWORD, reinterpret_cast<const BYTE*>(&bits), sizeof(bits));
  RegCloseKey(key);
}
bool copy(HWND owner, std::wstring_view text) {
  const auto memory = GlobalAlloc(GMEM_MOVEABLE, (text.size() + 1) * sizeof(wchar_t));
  if (!memory) return false;
  const auto target = static_cast<wchar_t*>(GlobalLock(memory));
  if (!target) { GlobalFree(memory); return false; }
  std::copy(text.begin(), text.end(), target); target[text.size()] = L'\0'; GlobalUnlock(memory);
  if (!OpenClipboard(owner)) { GlobalFree(memory); return false; }
  const bool ok = EmptyClipboard() && SetClipboardData(CF_UNICODETEXT, memory);
  CloseClipboard(); if (!ok) GlobalFree(memory);
  return ok;
}
std::wstring server_name() {
  const std::string_view id = MSIME_EDITION_ID;
  return MSIME_EDITION_IS_FULL ? L"msime" : L"msime-" + wide(id);
}
std::wstring client_name(const std::string& id) {
  return id == "claude_desktop" ? L"Claude Desktop" : id == "cursor" ? L"Cursor" : wide(id);
}
bool valid_status(const Json& status) {
  if (!status.is_object() || !status.contains("command") || !status.at("command").is_string() ||
      !status.contains("installed") || !status.at("installed").is_boolean() ||
      !status.contains("options") || (!status.at("options").is_null() && !status.at("options").is_string()) ||
      !status.contains("config") || (!status.at("config").is_null() && !status.at("config").is_string()) ||
      !status.contains("clients") || !status.at("clients").is_array()) return false;
  for (const auto& entry : status.at("clients")) {
    if (!entry.is_object()) return false;
    for (const auto* key : {"id","path"}) if (!entry.contains(key) || !entry.at(key).is_string()) return false;
    if (!entry.contains("configured") || !entry.at("configured").is_boolean() ||
        (entry.contains("stale") && !entry.at("stale").is_boolean()) ||
        !entry.contains("flags") || !entry.at("flags").is_array()) return false;
    for (const auto& flag : entry.at("flags")) if (!flag.is_string()) return false;
  }
  return true;
}
std::wstring failure(const Json& response) {
  const auto error = response.is_object() && response.contains("error") && response.at("error").is_string() ? response.at("error").get<std::string>() : "";
  if (error == "mcp_client_missing") return L"没有找到助手的配置目录，请先安装并打开一次该助手。";
  if (error == "mcp_config_invalid") return L"助手配置不是有效的 JSON，已保持原样，请先修正该文件。";
  if (error == "mcp_server_missing") return L"没有找到 msime-mcp，请重新安装输入法。";
  if (error == "mcp_options_missing") return L"输入法尚未完成初始化，请先完成设置向导。";
  return L"无法读取或写入助手配置，请重新读取后重试。";
}
}
struct McpPage::State {
  Services services;
  HWND window = nullptr;
  HINSTANCE instance = nullptr;
  HFONT font = nullptr;
  UINT dpi = 96;
  struct Row { HWND handle; int top; int height; };
  std::vector<Row> rows;
  std::thread worker;
  bool busy = false, initialized = false, ready = false;
  Json status;
  mc::Flags preferred;
  std::map<std::string, mc::Flags> drafts;
  std::vector<std::string> tabs{"claude_code", "codex", "terminal", "json"};
  std::string shown = "claude_code";
  std::wstring message, primary_text, removal_text;
  enum class Operation { read, flags, install };
  struct Job { Operation operation; Json request; std::string client; mc::Flags flags; };
  struct Result { Job job; Json response; Json status; uint32_t bits = 0; };
  // 结果只在工作线程完成后读取；join 同步其所有写入，不把裸指针投递到窗口消息。
  std::optional<Result> result;
  int dip(int amount) const { return MulDiv(amount, static_cast<int>(dpi), 96); }
  HWND control(int id) const { return GetDlgItem(window, id); }
  void text(int id, const std::wstring& content) { SetWindowTextW(control(id), content.c_str()); }
  void row(const wchar_t* type, const wchar_t* title, DWORD style, int id, int top, int row_height) {
    const auto child = CreateWindowExW(lstrcmpW(type,L"EDIT") == 0 ? WS_EX_CLIENTEDGE : 0, type, title,
        WS_CHILD | WS_VISIBLE | style | (lstrcmpW(type,L"BUTTON") == 0 ? BS_NOTIFY : 0), 0, 0, 1, 1, window, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), instance, nullptr);
    if (!child) throw std::runtime_error("mcp control");
    SendMessageW(child, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);
    rows.push_back({child, top, row_height});
  }
  void layout() {
    RECT client{}; GetClientRect(window, &client);
    for (const auto& row : rows) MoveWindow(row.handle, 0, dip(row.top), std::max(1L,client.right - dip(8)), dip(row.height), TRUE);
  }
  const Json* client() const {
    if (!ready || !status.contains("clients") || !status.at("clients").is_array()) return nullptr;
    for (const auto& entry : status.at("clients"))
      if (entry.is_object() && entry.value("id",std::string{}) == shown) return &entry;
    return nullptr;
  }
  mc::Flags flags() const {
    const auto entry = client();
    if (!entry || !entry->value("configured",false)) return preferred;
    if (const auto draft = drafts.find(shown); draft != drafts.end()) return draft->second;
    mc::Flags actual;
    for (const auto& flag : entry->value("flags",Json::array())) if (flag.is_string()) actual.push_back(wide(flag.get<std::string>()));
    return mc::canonical(actual);
  }
  void render() {
    const auto selected = flags();
    const auto entry = client();
    const auto command = ready ? wide(status.value("command",std::string{})) : std::wstring{};
    std::optional<std::wstring> options;
    if (ready && status.contains("options") && status.at("options").is_string()) options = wide(status.at("options").get<std::string>());
    const bool initialized_config = ready && status.contains("config") && status.at("config").is_string();
    const bool terminal = shown == "terminal", json = shown == "json";
    const bool cli = shown == "claude_code" || shown == "codex";
    primary_text.clear(); removal_text.clear();
    std::wstring instructions_text;
    if (cli) {
      const auto assistant_kind = shown == "codex" ? mc::Assistant::codex : mc::Assistant::claude_code;
      primary_text = mc::tokens_text(mc::install_command(assistant_kind,server_name(),command,options,selected));
      removal_text = mc::tokens_text(mc::remove_command(assistant_kind,server_name()));
      instructions_text = L"在终端运行下面的安装命令，然后重新启动助手。";
    } else if (terminal) {
      primary_text = mc::tokens_text(mc::terminal_instructions(command,options,selected));
      instructions_text = L"将这段说明告诉能运行终端命令的 AI 助手，或放进项目的 AGENTS.md / CLAUDE.md。";
    } else if (initialized_config) {
      primary_text = mc::tokens_text(mc::config_tokens(wide(status.at("config").get<std::string>()),selected));
      instructions_text = entry ? L"写入 " + wide(entry->value("path",std::string{})) + (entry->value("configured",false) ? L"（已连接）" : L"") : L"粘贴到任意支持 MCP 的助手配置中。";
    }
    if (!initialized_config) instructions_text = ready ? L"输入法尚未完成初始化，完成设置向导后即可连接。" : L"无法读取 MCP 服务器状态，可点「重新读取」。";
    text(instructions,instructions_text); text(primary,primary_text); text(removal,removal_text);
    text(copy_primary,json || entry ? L"复制配置" : terminal ? L"复制说明" : L"复制安装命令");
    for (const auto id : {remove_instructions,removal,copy_removal}) ShowWindow(control(id),cli ? SW_SHOW : SW_HIDE);
    ShowWindow(control(write),entry ? SW_SHOW : SW_HIDE);
    std::wstring status_message = message;
    bool outdated = false;
    if (entry && entry->value("configured",false)) {
      mc::Flags actual;
      for (const auto& flag : entry->value("flags",Json::array())) if (flag.is_string()) actual.push_back(wide(flag.get<std::string>()));
      outdated = entry->value("stale",false) || !mc::same_flags(actual,selected);
      if (outdated) status_message += (status_message.empty() ? L"" : L"\r\n") + mc::outdated_notice(client_name(shown),entry->value("stale",false),command);
    }
    text(write,busy ? L"正在处理…" : (outdated ? L"更新 " : L"写入 ") + client_name(shown));
    for (size_t index = 0; index < mc::permissions.size(); ++index)
      SendMessageW(control(index ? permission_read : permission_write), BM_SETCHECK,mc::has_flag(selected,mc::permissions[index].flag) ? BST_CHECKED : BST_UNCHECKED,0);
    text(notice,status_message);
    text(server,ready ? L"服务器程序 " + command + (status.value("installed",false) ? L"" : L"（未找到，请重新安装输入法）") : L"");
    for (const int id : {assistant,permission_write,permission_read,refresh}) EnableWindow(control(id),!busy);
    EnableWindow(control(copy_primary),!busy && initialized_config && !primary_text.empty());
    EnableWindow(control(copy_removal),!busy && initialized_config && cli);
    EnableWindow(control(write),!busy && initialized_config && entry && status.value("installed",false) && (!entry->value("configured",false) || outdated));
  }
  void start(Job job) {
    if (busy) return;
    if (worker.joinable()) worker.join();
    busy = true; render();
    try {
      worker = std::thread([this,job = std::move(job)] {
        Result next{job,Json{},Json{},0};
        try {
          if (job.operation == Operation::flags) services.save_flags(mc::flag_bits(job.flags));
          else {
            if (job.operation == Operation::read) next.bits = initialized ? mc::flag_bits(preferred) : services.load_flags();
            if (job.operation == Operation::install) next.response = Json::parse(services.install(job.request.dump()),nullptr,false);
            if (job.operation == Operation::read || value(next.response))
              next.status = Json::parse(services.status(options_request().dump()),nullptr,false);
          }
        } catch (...) { next.response = Json(); next.status = Json(); }
        result = std::move(next); PostMessageW(window,completed,0,0);
      });
    } catch (...) { busy = false; message = L"无法启动后台操作，请重试。"; render(); }
  }
  void accept() {
    if (!worker.joinable()) return;
    worker.join(); busy = false;
    if (!result) { message = L"无法读取 MCP 状态。"; render(); return; }
    auto next = std::move(*result); result.reset();
    if (next.job.operation == Operation::flags) { render(); return; }
    if (next.job.operation == Operation::install && !value(next.response)) {
      if (next.response.is_object() && next.response.value("error",std::string{}) == "mcp_entry_exists" && !next.job.request.value("replace",false)) {
        const auto title = L"替换 " + client_name(next.job.client) + L" 中的 " + server_name() + L"？";
        if (services.confirm(window,title.c_str(),L"该助手已有一个同名服务器。替换会覆盖原命令和参数，其余配置保持原样。")) {
          next.job.request["replace"] = true; start(std::move(next.job)); return;
        }
        message = L"已取消替换，助手配置保持原样。";
      } else message = failure(next.response);
      render(); return;
    }
    if (next.job.operation == Operation::read) { preferred = mc::flags_from_bits(next.bits); initialized = true; }
    else {
      const auto outcome = value(next.response);
      if (!outcome || !outcome->is_string()) { message = L"助手返回了无效的写入结果，请重新读取确认。"; render(); return; }
      message = mc::install_result(wide(outcome->get<std::string>()),client_name(next.job.client));
      drafts.erase(next.job.client);
    }
    const auto response = value(next.status);
    ready = response && valid_status(*response);
    if (ready) status = *response;
    else { status = Json(); message += (message.empty() ? L"" : L"\r\n") + std::wstring(L"状态读取失败，请重新读取。每次写入后会重新核对。 "); }
    tabs = {"claude_code","codex","terminal"};
    if (ready && status.value("installed",false) && status.contains("clients") && status.at("clients").is_array())
      for (const auto& entry : status.at("clients")) {
        if (!entry.is_object()) continue;
        const auto id = entry.value("id",std::string{});
        if ((id == "claude_desktop" || id == "cursor") && std::find(tabs.begin(),tabs.end(),id) == tabs.end()) tabs.push_back(id);
      }
    tabs.push_back("json");
    if (std::find(tabs.begin(),tabs.end(),shown) == tabs.end()) shown = "claude_code";
    SendMessageW(control(assistant),CB_RESETCONTENT,0,0);
    for (size_t index = 0; index < tabs.size(); ++index) {
      const auto title = tabs[index] == "terminal" ? L"命令行" : tabs[index] == "json" ? L"其他" : tabs[index] == "claude_code" ? L"Claude Code" : client_name(tabs[index]);
      SendMessageW(control(assistant),CB_ADDSTRING,0,reinterpret_cast<LPARAM>(title.c_str()));
      if (tabs[index] == shown) SendMessageW(control(assistant),CB_SETCURSEL,index,0);
    }
    render();
  }
  void command(WPARAM command) {
    if (busy) return;
    const int id = LOWORD(command), event = HIWORD(command);
    if (id == assistant && event == CBN_SELCHANGE) {
      const auto selected = SendMessageW(control(assistant),CB_GETCURSEL,0,0);
      if (selected >= 0 && static_cast<size_t>(selected) < tabs.size()) { shown = tabs[static_cast<size_t>(selected)]; message.clear(); render(); }
    } else if (event == BN_CLICKED) {
      if (id == refresh) { message.clear(); start({Operation::read,Json{},"",{}}); }
      else if (id == copy_primary || id == copy_removal) {
        message = services.copy(window,id == copy_primary ? primary_text : removal_text) ? L"已复制。" : L"复制失败，请重试。"; render();
      } else if (id == permission_write || id == permission_read) {
        const auto flag = mc::permissions[id == permission_read ? 1 : 0].flag;
        const bool on = SendMessageW(control(id),BM_GETCHECK,0,0) == BST_CHECKED;
        const auto entry = client();
        if (entry && entry->value("configured",false)) drafts[shown] = mc::toggled(flags(),flag,on);
        preferred = mc::toggled(preferred,flag,on);
        start({Operation::flags,Json{},"",preferred});
      } else if (id == write && ready && client()) {
        const auto selected = flags(); const auto name = client_name(shown);
        if (!selected.empty() && !services.confirm(window,mc::grant_title(name).c_str(),mc::grant_message(name,selected).c_str())) { message = L"已取消写入。"; render(); return; }
        Json request{{"options",status.value("options",Json{})},{"client",shown},{"flags",Json::array()},{"replace",false}};
        for (const auto& flag : selected) request["flags"].push_back(utf8(flag));
        message.clear(); start({Operation::install,std::move(request),shown,selected});
      }
    }
  }
  bool controls() {
    row(L"STATIC",L"MCP 让 AI 助手通过本机服务管理水杉输入法的设置、快捷短语、皮肤、打字统计与诊断日志。使用设置里登录的账号可将皮肤发布到社区。两个权限开关决定助手能修改或读取哪些内容；都关闭时只读（诊断日志开关除外）。",SS_LEFT,description,0,116);
    row(L"COMBOBOX",L"AI 助手",WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL,assistant,122,220);
    row(L"STATIC",L"正在读取 MCP 状态…",SS_LEFT,instructions,168,88);
    row(L"EDIT",L"",WS_TABSTOP | ES_READONLY | ES_MULTILINE | WS_VSCROLL,primary,264,330);
    row(L"BUTTON",L"复制安装命令",WS_TABSTOP | BS_PUSHBUTTON,copy_primary,604,30);
    row(L"STATIC",L"之前添加过的，先运行这条移除命令：",SS_LEFT,remove_instructions,648,32);
    row(L"EDIT",L"",WS_TABSTOP | ES_READONLY | ES_MULTILINE | WS_VSCROLL,removal,686,70);
    row(L"BUTTON",L"复制移除命令",WS_TABSTOP | BS_PUSHBUTTON,copy_removal,766,30);
    row(L"BUTTON",L"写入助手配置",WS_TABSTOP | BS_PUSHBUTTON,write,812,32);
    for (size_t index = 0; index < mc::permissions.size(); ++index) {
      const auto& permission = mc::permissions[index]; const auto top = 864 + static_cast<int>(index) * 112;
      row(L"BUTTON",std::wstring(permission.title).c_str(),WS_TABSTOP | BS_AUTOCHECKBOX,index ? permission_read : permission_write,top,30);
      row(L"STATIC",(std::wstring(permission.flag) + L" " + std::wstring(permission.detail)).c_str(),SS_LEFT,index ? permission_read_detail : permission_write_detail,top + 36,68);
    }
    row(L"STATIC",L"只在你信任该助手时保留权限。权限选择只保存在本机，修改后需更新助手配置才生效。",SS_LEFT,-1,1096,58);
    row(L"STATIC",L"",SS_LEFT,notice,1162,104);
    row(L"STATIC",L"",SS_LEFT,server,1274,86);
    row(L"BUTTON",L"重新读取",WS_TABSTOP | BS_PUSHBUTTON,refresh,1380,30);
    layout(); render(); return true;
  }
  static LRESULT CALLBACK proc(HWND hwnd,UINT message,WPARAM wparam,LPARAM lparam) {
    auto self = reinterpret_cast<State*>(GetWindowLongPtrW(hwnd,GWLP_USERDATA));
    if (message == WM_NCCREATE) { self = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); self->window = hwnd; SetWindowLongPtrW(hwnd,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(self)); }
    if (!self) return DefWindowProcW(hwnd,message,wparam,lparam);
    try {
      switch (message) {
      case WM_CREATE: return self->controls() ? 0 : -1;
      case WM_SIZE: self->layout(); return 0;
      case WM_COMMAND: {
        const int event = HIWORD(wparam);
        wchar_t type[32]{}; GetClassNameW(reinterpret_cast<HWND>(lparam),type,32);
        if ((lstrcmpW(type,L"COMBOBOX") == 0 && event == CBN_SETFOCUS) ||
            (lstrcmpW(type,L"EDIT") == 0 && event == EN_SETFOCUS) ||
            (lstrcmpW(type,L"BUTTON") == 0 && event == BN_SETFOCUS))
          SendMessageW(GetParent(hwnd),message,wparam,lparam);
        else self->command(wparam);
        return 0;
      }
      case WM_MOUSEWHEEL: return SendMessageW(GetParent(hwnd),message,wparam,lparam);
      case completed: self->accept(); return 0;
      case WM_DESTROY: if (self->worker.joinable()) self->worker.join(); return 0;
      case WM_NCDESTROY: self->window = nullptr; SetWindowLongPtrW(hwnd,GWLP_USERDATA,0); break;
      }
    } catch (...) { self->message = L"无法读取 MCP 状态，请重新读取。"; self->ready = false; self->render(); }
    return DefWindowProcW(hwnd,message,wparam,lparam);
  }
};
McpPage::McpPage(Services services) : state_(std::make_unique<State>()) {
  if (!services.status) services.status = [](const std::string& request) { return call(msime_client_mcp_status,request); };
  if (!services.install) services.install = [](const std::string& request) { return call(msime_client_mcp_install,request); };
  if (!services.load_flags) services.load_flags = load_flags;
  if (!services.save_flags) services.save_flags = save_flags;
  if (!services.copy) services.copy = copy;
  if (!services.confirm) services.confirm = [](HWND hwnd,const wchar_t* title,const wchar_t* text) { return MessageBoxW(hwnd,text,title,MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES; };
  state_->services = std::move(services);
}
McpPage::~McpPage() { if (state_->window) DestroyWindow(state_->window); if (state_->worker.joinable()) state_->worker.join(); }
bool McpPage::create(HWND parent,HINSTANCE instance,HFONT font,UINT dpi) {
  if (state_->window || state_->worker.joinable()) return false;
  state_->instance = instance; state_->font = font; state_->dpi = dpi;
  WNDCLASSEXW type{}; type.cbSize = sizeof(type); type.hInstance = instance; type.lpfnWndProc = State::proc;
  type.hCursor = LoadCursorW(nullptr,IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_WINDOW); type.lpszClassName = page_class;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  if (!CreateWindowExW(WS_EX_CONTROLPARENT,page_class,L"连接 AI 助手",WS_CHILD | WS_CLIPCHILDREN,0,0,1,1,parent,nullptr,instance,state_.get())) return false;
  state_->start({State::Operation::read,Json{},"",{}}); return true;
}
HWND McpPage::handle() const { return state_->window; }
}
