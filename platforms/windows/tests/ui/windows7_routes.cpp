#include "../../common/SurfaceRouteArgument.h"
#include "../../settings/win7/SettingsRoutes.h"
#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <commctrl.h>
#include "../../../../shared/contracts/msime_edition.h"

int main() {
  using namespace msime::settings::win7;
  using msime::windows::surface_route_argument;
  const wchar_t* arguments[] = {L"app.exe", L"--route=settings:plugins", L"unexpected"};
  assert(surface_route_argument(1, arguments, L"settings:dictionary", "settings") == "settings:dictionary");
  assert(surface_route_argument(2, arguments, L"settings:dictionary", "settings") == "settings:plugins");
  assert(surface_route_argument(1, arguments, L"", "settings") == "settings");
  assert(!surface_route_argument(3, arguments, L"", "settings"));
  arguments[1] = L"--route="; assert(!surface_route_argument(2, arguments, L"settings", "settings"));
  arguments[1] = L"--route=settings:../input"; assert(!surface_route_argument(2, arguments, L"", "settings"));
  assert(!surface_route_argument(1, arguments, std::wstring(65, L'a'), "settings"));
  assert(!settings_page_for_route("settings:unknown") && !settings_page_for_route("vocabulary"));
  assert(settings_page_for_route("settings") == 3 && settings_page_for_route("settings:input") == 3);
  assert(settings_page_for_route("settings:appearance") == 5 && settings_page_for_route("settings:skin") == 8);
  assert(settings_page_for_route("settings:dictionary") == 13 && settings_page_for_route("settings:vocabulary") == 14);
  assert(settings_page_for_route("settings:plugins") == 15);
  assert(!settings_page_for_route("settings:ai") && !settings_page_for_route("settings:chat") &&
      !settings_page_for_route("settings:account") && !settings_page_for_route("settings:cloud-clipboard") &&
      !settings_page_for_route("settings:community") && !settings_page_for_route("settings:mcp") &&
      !settings_page_for_route("settings:community-plugins") && !settings_page_for_route("settings:community-resources"));
  assert(settings_page_for_route("settings:keyboard-skin") == 22);
  assert(settings_page_for_route("settings:screen-keyboard") == 20 && settings_page_for_route("settings:osk") == 20);
  assert(settings_page_for_route("settings:handwriting") == (MSIME_EDITION_HANDWRITING != 0 ? 20 : 3));
  wchar_t root[MAX_PATH]{}, file[MAX_PATH]{};
  assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7r", 0, file));
  assert(DeleteFileW(file) && CreateDirectoryW(file, nullptr));
  const std::filesystem::path state(file);
  const auto length = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0);
  std::wstring previous(length, L'\0'); if (length) previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), length));
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", state.c_str()));
  {
    { std::ofstream edition(state / L"edition", std::ios::binary); edition << MSIME_EDITION_ID; assert(edition.good()); }
    PreferencesStore store(state.u8string()); const auto before = store.load(); assert(before.snapshot);
    McpPage::Services mcp;
    mcp.load_flags = [] { return 0u; }; mcp.save_flags = [](uint32_t) { assert(false); };
    mcp.status = [](const std::string&) { return R"({"ok":true,"value":{"command":"C:\\Synthetic\\msime-mcp.exe","installed":false,"options":null,"config":null,"clients":[]}})"; };
    SettingsWindow window({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, std::move(mcp)); assert(window.open_route("settings:plugins"));
    assert(window.create(GetModuleHandleW(nullptr)));
    const auto selector = GetDlgItem(window.handle(), page_selector);
    const auto body = GetDlgItem(window.handle(), page_body); const auto deadline = GetTickCount64() + 10000;
    for (;;) {
      MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
      if (SendMessageW(selector, CB_GETCURSEL, 0, 0) == 15 && IsWindowEnabled(GetDlgItem(body, language_switch))) break;
      assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT);
    }
    assert(window.open_route("settings:input") && SendMessageW(selector, CB_GETCURSEL, 0, 0) == 3);
    assert(!window.open_route("settings:unknown") && SendMessageW(selector, CB_GETCURSEL, 0, 0) == 3);
    assert(!window.open_route("settings:mcp") && SendMessageW(selector, CB_GETCURSEL, 0, 0) == 3);
    // 深链接和分类导航必须选中同一页面；切换分类不能写偏好。
    const auto navigation = GetDlgItem(window.handle(), page_navigation);
    assert(navigation && !(GetWindowLongPtrW(selector, GWL_STYLE) & WS_VISIBLE));
    TVITEMW selected{}; selected.mask = TVIF_PARAM; selected.hItem = TreeView_GetSelection(navigation);
    assert(TreeView_GetItem(navigation, &selected) && selected.lParam == 3);
    assert(window.open_route("settings:input"));
    const auto nav_deadline = GetTickCount64() + 5000;
    while (SendMessageW(selector, CB_GETCURSEL, 0, 0) != 3 || !IsWindowEnabled(GetDlgItem(body, language_switch))) {
      MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
      assert(GetTickCount64() < nav_deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT);
    }
    selected.hItem = TreeView_GetSelection(navigation);
    assert(TreeView_GetItem(navigation, &selected) && selected.lParam == 3);
    // 用真实树控件选择相邻的快捷键页，经过正式通知再检查页面标题。
    const auto shortcuts = TreeView_GetNextSibling(navigation, selected.hItem);
    assert(shortcuts && TreeView_SelectItem(navigation, shortcuts));
    assert(SendMessageW(selector, CB_GETCURSEL, 0, 0) == 0);
    wchar_t heading[80]{};
    GetWindowTextW(GetDlgItem(window.handle(), page_heading), heading, 80);
    assert(std::wstring(heading) == L"快捷键");
    // 最小窗口和普通窗口都应把导航、正文、状态栏留在各自区域。
    for (const auto width : {720, 1000}) {
      SetWindowPos(window.handle(), nullptr, 0, 0, width, 650, SWP_NOMOVE | SWP_NOZORDER);
      RECT nav{}, content{}, footer{};
      GetWindowRect(navigation, &nav); GetWindowRect(body, &content);
      GetWindowRect(GetDlgItem(window.handle(), reload_settings), &footer);
      assert(nav.right < content.left && content.bottom <= footer.top);
    }
    assert(store.load().snapshot->revision() == before.snapshot->revision());
    DestroyWindow(window.handle());
  }
  SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", length ? previous.c_str() : nullptr);
  std::error_code error; std::filesystem::remove_all(state, error);
  std::puts("Windows 7 routes: argument precedence, invalid input rejection, deferred initial navigation and unchanged preferences passed");
}
