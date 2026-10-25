#include "../../settings/win7/McpPage.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/McpConnect.h"
#include <atomic>
#include <cassert>
#include <chrono>
#include <cstdio>
#include <thread>

using namespace msime::settings::win7;
namespace mc = msime::settings::mcp;
namespace {
void pump(const std::function<bool()>& ready) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) {
    MSG message{};
    while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (ready()) return;
    assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0,nullptr,FALSE,20,QS_ALLINPUT);
  }
}
std::wstring text(HWND hwnd) {
  const auto length = GetWindowTextLengthW(hwnd); std::wstring result(static_cast<size_t>(length) + 1,L'\0');
  GetWindowTextW(hwnd,result.data(),length + 1); result.resize(static_cast<size_t>(length)); return result;
}
}
int main() {
  using Page = McpPage;
  const auto ui_thread = GetCurrentThreadId();
  Json status{{"command","C:\\Synthetic\\msime-mcp.exe"},{"installed",true},{"options",nullptr},
    {"config",R"({"mcpServers":{"msime":{"command":"C:\\Synthetic\\msime-mcp.exe","args":[]}}})"},
    {"clients",Json::array({Json{{"id","cursor"},{"path","C:\\Synthetic\\cursor.json"},{"configured",true},{"stale",false},{"flags",Json::array({"--allow-dictionary-read"})}},
      Json{{"id","claude_desktop"},{"path","C:\\Synthetic\\claude.json"},{"configured",false},{"stale",false},{"flags",Json::array()}}})}};
  std::atomic<int> reads{0},writes{0},saves{0};
  bool approve = false, replace = false, copied_ok = true; int read_mode = 0;
  int confirmations = 0; uint32_t saved = 3;
  std::wstring copied; Json last_install;
  Page::Services services;
  services.load_flags = [&] { assert(GetCurrentThreadId() != ui_thread); return saved; };
  services.save_flags = [&](uint32_t bits) { assert(GetCurrentThreadId() != ui_thread); saved = bits; ++saves; };
  services.status = [&](const std::string& input) {
    assert(GetCurrentThreadId() != ui_thread); const auto request = Json::parse(input); assert(request.contains("options")); ++reads;
    if (read_mode == 1) return std::string("invalid");
    auto response = status;
    if (read_mode == 2) response["clients"][0]["configured"] = "invalid";
    if (read_mode == 3) response["config"] = nullptr;
    return Json{{"ok",true},{"value",response}}.dump();
  };
  services.install = [&](const std::string& input) {
    assert(GetCurrentThreadId() != ui_thread); ++writes; last_install = Json::parse(input);
    if (!last_install.at("replace").get<bool>()) return Json{{"ok",false},{"error","mcp_entry_exists"}}.dump();
    for (auto& entry : status["clients"]) if (entry.at("id") == last_install.at("client")) { entry["configured"] = true; entry["flags"] = last_install.at("flags"); }
    return Json{{"ok",true},{"value","replaced"}}.dump();
  };
  services.copy = [&](HWND,std::wstring_view value) { assert(GetCurrentThreadId() == ui_thread); copied = value; return copied_ok; };
  services.confirm = [&](HWND,const wchar_t* title,const wchar_t*) {
    assert(GetCurrentThreadId() == ui_thread); ++confirmations;
    return std::wstring_view(title).find(L"替换") != std::wstring_view::npos ? replace : approve;
  };
  const auto instance = GetModuleHandleW(nullptr);
  const auto parent = CreateWindowExW(0,L"STATIC",L"合成 MCP 测试",WS_OVERLAPPEDWINDOW,0,0,800,600,nullptr,nullptr,instance,nullptr); assert(parent);
  {
    Page page(services); assert(page.create(parent,instance,static_cast<HFONT>(GetStockObject(DEFAULT_GUI_FONT)),96));
    const auto root = page.handle(); const auto control = [&](int id) { const auto hwnd = GetDlgItem(root,id); assert(hwnd); return hwnd; };
    const auto wait = [&] { pump([&] { return IsWindowEnabled(control(Page::refresh)) != FALSE; }); };
    const auto click = [&](int id) { assert(IsWindowEnabled(control(id))); SendMessageW(control(id),BM_CLICK,0,0); wait(); };
    const auto select = [&](const char* id) {
      const auto title = std::string(id) == "cursor" ? L"Cursor" : std::string(id) == "claude_desktop" ? L"Claude Desktop" : std::string(id) == "terminal" ? L"命令行" : std::string(id) == "json" ? L"其他" : L"Claude Code";
      const auto index = SendMessageW(control(Page::assistant),CB_FINDSTRINGEXACT,static_cast<WPARAM>(-1),reinterpret_cast<LPARAM>(title)); assert(index != CB_ERR);
      SendMessageW(control(Page::assistant),CB_SETCURSEL,index,0); SendMessageW(root,WM_COMMAND,MAKEWPARAM(Page::assistant,CBN_SELCHANGE),reinterpret_cast<LPARAM>(control(Page::assistant)));
    };
    const auto checked = [&](int id) { return SendMessageW(control(id),BM_GETCHECK,0,0) == BST_CHECKED; };
    wait(); assert(reads == 1 && writes == 0 && saves == 0);
    click(Page::copy_primary); assert(copied.find(L"claude mcp add") == 0 && copied.find(L"--allow-write") != std::wstring::npos);
    click(Page::copy_removal); assert(copied.find(L"mcp remove") != std::wstring::npos);
    select("cursor"); assert(!checked(Page::permission_write) && checked(Page::permission_read)); assert(!IsWindowEnabled(control(Page::write)));
    click(Page::permission_write); assert(checked(Page::permission_write) && saves == 1 && writes == 0 && saved == 3);
    select("claude_desktop"); click(Page::permission_read); assert(saved == 1);
    select("cursor"); assert(checked(Page::permission_read) && checked(Page::permission_write));
    // 权限确认取消不发起写入；冲突确认取消不自动替换。
    click(Page::write); assert(confirmations == 1 && writes == 0);
    approve = true; click(Page::write); assert(confirmations == 3 && writes == 1 && last_install.at("replace") == false);
    replace = true; click(Page::write); assert(confirmations == 5 && writes == 3 && last_install.at("replace") == true);
    assert(last_install.at("flags") == Json::array({"--allow-write","--allow-dictionary-read"}) && reads == 2);
    assert(!IsWindowEnabled(control(Page::write)) && checked(Page::permission_read));
    // 撤销权限也经过同一写入链；成功后重读实际权限，不靠草稿伪造已连接状态。
    click(Page::permission_write); click(Page::permission_read); assert(saved == 0);
    click(Page::write); assert(confirmations == 6 && writes == 5 && last_install.at("flags").empty());
    assert(!checked(Page::permission_write) && !checked(Page::permission_read) && !IsWindowEnabled(control(Page::write)));
    select("terminal"); click(Page::copy_primary); assert(copied.find(L" tools") != std::wstring::npos && copied.find(L"--allow-write") == std::wstring::npos);
    select("json"); click(Page::copy_primary); assert(copied.find(L"mcpServers") != std::wstring::npos && copied.find(L"--allow-write") == std::wstring::npos);
    copied_ok = false; click(Page::copy_primary); assert(text(control(Page::notice)).find(L"复制失败") != std::wstring::npos);
    read_mode = 1; click(Page::refresh); assert(!IsWindowEnabled(control(Page::copy_primary)) && !IsWindowEnabled(control(Page::write)));
    read_mode = 2; click(Page::refresh); assert(!IsWindowEnabled(control(Page::copy_primary)) && !IsWindowEnabled(control(Page::write)));
    read_mode = 3; click(Page::refresh); assert(!IsWindowEnabled(control(Page::copy_primary)));
    read_mode = 0; click(Page::refresh); assert(IsWindowEnabled(control(Page::copy_primary)));
  }
  // 关闭页面会等待串行工作线程，迟到的完成消息不持有悬空结果指针。
  std::atomic<bool> entered{false},finished{false};
  services.status = [&](const std::string&) { entered = true; std::this_thread::sleep_for(std::chrono::milliseconds(80)); finished = true; return Json{{"ok",true},{"value",status}}.dump(); };
  {
    Page page(services); assert(page.create(parent,instance,static_cast<HFONT>(GetStockObject(DEFAULT_GUI_FONT)),96));
    pump([&] { return entered.load(); }); DestroyWindow(page.handle()); assert(finished);
  }
  DestroyWindow(parent);
  std::puts("Windows 7 MCP: worker isolation, existing permissions, drafts, copy, cancellation, explicit conflict replacement, reload failure and close/join passed");
}
