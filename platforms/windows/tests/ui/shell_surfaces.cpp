#include "ShellSurfaces.h"
#include "SecureFieldPolicy.h"
#include "../../panels/win7/PanelRoutes.h"
#include <chrono>
#include <cstdio>
#include <fstream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
// panels_file 是开关缺省（PanelHost::Shared）时承载面板的程序，native_panels_file 是 MSIME_WINDOWS_PANEL_HOST=native 时的。Win7 包只有原生面板，两种开关状态都是 -win7 程序。
#if defined(MSIME_WINDOWS7_CLIENT) && MSIME_WINDOWS7_CLIENT
constexpr wchar_t settings_file[] = L"msime-client-settings-win7.exe";
constexpr wchar_t panels_file[] = L"msime-client-panels-win7.exe";
constexpr wchar_t native_panels_file[] = L"msime-client-panels-win7.exe";
#else
constexpr wchar_t settings_file[] = L"msime-client-settings.exe";
constexpr wchar_t panels_file[] = L"MSIME.exe";
constexpr wchar_t native_panels_file[] = L"msime-client-panels.exe";
#endif
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Shell surface test failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)
namespace {
// The block is double-NUL terminated, so entries are compared one at a time.
std::vector<std::wstring> entries(const std::wstring &block) {
  std::vector<std::wstring> result;
  for (size_t start = 0; start < block.size() && block[start];) {
    const auto end = block.find(L'\0', start);
    result.emplace_back(block, start, end - start);
    start = end + 1;
  }
  return result;
}
bool contains(const std::vector<std::wstring> &values, const std::wstring &entry) {
  for (const auto &value : values)
    if (value == entry)
      return true;
  return false;
}
} // namespace
int main() {
  try {
    // Every surface the menu offers reaches the shell, and only the toolbar
    // row - which this process owns - stays out of the contract.
    require(shell_surface_request(TrayMenuCommand::OpenEmojiPanel)->panel ==
            "emoji");
    require(shell_surface_request(TrayMenuCommand::OpenHandwritingPanel)
                ->panel == "handwriting");
    require(shell_surface_request(TrayMenuCommand::OpenKeyboardPanel)->panel ==
            "keyboard");
    // The tray's 语音 row toggles the Server's own voice input, so it is not a shell surface.
    require(!shell_surface_request(TrayMenuCommand::ToggleVoiceInput));
    const auto settings = shell_surface_request(TrayMenuCommand::OpenSettings);
    require(settings && settings->panel.empty() && settings->page.empty());
    const auto about = shell_surface_request(TrayMenuCommand::OpenAbout);
    require(about && about->panel.empty() && about->page == "about");
    require(!shell_surface_request(TrayMenuCommand::ToggleFloatingToolbar));
    // 主题 and 词库… open their pages of the settings app, by ids its navigation and the shared SettingsCategory both know.
    const auto theme = shell_surface_request(TrayMenuCommand::OpenTheme);
    require(theme && theme->panel.empty() && theme->page == "skin");
    require(shell_route_argument(*theme) == L"settings:skin");
    const auto dictionary = shell_surface_request(TrayMenuCommand::OpenDictionary);
    require(dictionary && dictionary->panel.empty() &&
            dictionary->page == "dictionary");
    require(shell_route_argument(*dictionary) == L"settings:dictionary");
    // 「云剪贴板…」按共享应用的 cloud-clipboard 路由打开面板，环境块和命令行都接受这个路由。
    const auto cloud = shell_surface_request(TrayMenuCommand::OpenCloudClipboard);
    require(cloud && cloud->panel == "cloud-clipboard" && cloud->page.empty());
    require(shell_surface_route(*cloud) == "cloud-clipboard");
    require(shell_route_argument(*cloud) == L"cloud-clipboard");
    require(shell_executable_names(*cloud, PanelHost::Shared) == std::vector<std::wstring>{panels_file});
    require(entries(shell_environment_block(L"\0", *cloud))[0] ==
            L"MSIME_CLIENT_ROUTE=cloud-clipboard");
    // 「云词库…」按 cloud-dictionary 路由打开共享应用的云词库面板，与 macOS 输入菜单的同名一行相同。
    const auto cloud_dictionary = shell_surface_request(TrayMenuCommand::OpenCloudDictionary);
    require(cloud_dictionary && cloud_dictionary->panel == "cloud-dictionary" &&
            cloud_dictionary->page.empty());
    require(shell_route_argument(*cloud_dictionary) == L"cloud-dictionary");
    require(shell_executable_names(*cloud_dictionary, PanelHost::Shared) == std::vector<std::wstring>{panels_file});
    require(entries(shell_environment_block(L"\0", *cloud_dictionary))[0] ==
            L"MSIME_CLIENT_ROUTE=cloud-dictionary");
    // 「背单词（今日待复习 N）…」按 vocabulary 路由打开共享应用的背单词面板，与 macOS 输入菜单的同一行相同。
    const auto vocabulary = shell_surface_request(TrayMenuCommand::OpenVocabulary);
    require(vocabulary && vocabulary->panel == "vocabulary" && vocabulary->page.empty());
    require(shell_route_argument(*vocabulary) == L"vocabulary");
    require(shell_executable_names(*vocabulary, PanelHost::Shared) == std::vector<std::wstring>{panels_file});
    // 面板宿主开关：只有 native（不分大小写）选原生宿主，没设、为空、拼错或别的值都留在缺省的共享应用上。
    require(panel_host_from_setting(L"native") == PanelHost::Native);
    require(panel_host_from_setting(L"Native") == PanelHost::Native);
    require(panel_host_from_setting(L"NATIVE") == PanelHost::Native);
    for (const wchar_t *value : {L"", L"shared", L"nativ", L"natives", L" native", L"native ", L"1", L"tauri"})
      require(panel_host_from_setting(value) == PanelHost::Shared);
    require(std::wstring_view(panel_host_variable) == L"MSIME_WINDOWS_PANEL_HOST");
    // 托盘、悬浮工具栏和设置窗口发出的每个面板路由，两种开关状态各自落到对应的程序；原生宿主必须认这个路由，否则开关打开后那一行会打开一个立即退出的进程。设置和关于不受开关影响。
    for (auto command :
         {TrayMenuCommand::OpenEmojiPanel, TrayMenuCommand::OpenHandwritingPanel,
          TrayMenuCommand::OpenKeyboardPanel, TrayMenuCommand::OpenCloudClipboard,
          TrayMenuCommand::OpenCloudDictionary, TrayMenuCommand::OpenVocabulary}) {
      const auto request = shell_surface_request(command);
      require(request && !request->panel.empty() && request->page.empty());
      require(shell_executable_names(*request, PanelHost::Shared) ==
              std::vector<std::wstring>{panels_file});
      require(shell_executable_names(*request, PanelHost::Native) ==
              std::vector<std::wstring>{native_panels_file});
      require(msime::windows::win7::panel_route(shell_surface_route(*request)));
    }
    for (auto command : {TrayMenuCommand::OpenSettings, TrayMenuCommand::OpenAbout,
                         TrayMenuCommand::OpenTheme, TrayMenuCommand::OpenDictionary}) {
      const auto request = shell_surface_request(command);
      for (auto host : {PanelHost::Shared, PanelHost::Native})
        require(shell_executable_names(*request, host) == std::vector<std::wstring>{settings_file});
    }
    // 卡片自己的翻页、主题行、繁体输出和英文候选模式都不经过外壳。
    for (auto command :
         {TrayMenuCommand::ShowSchemes, TrayMenuCommand::ShowThemes,
          TrayMenuCommand::ShowMain, TrayMenuCommand::SelectTheme,
          TrayMenuCommand::ToggleTraditionalOutput,
          TrayMenuCommand::ToggleDedicatedEnglish})
      require(!shell_surface_request(command));
    // 焦点在密码框里时不打开云剪贴板：Win32 编辑控件的 ES_PASSWORD 只在编辑类控件上算数，UI Automation 的 IsPassword 次之，没有回答时照常打开。
    require(win32_password_edit(L"Edit", 0x50010020));
    require(win32_password_edit(L"RichEdit20W", 0x20));
    require(win32_password_edit(L"WindowsForms10.EDIT.app.0.141b42a_r6_ad1", 0x20));
    require(!win32_password_edit(L"Edit", 0x50010000));
    require(!win32_password_edit(L"Button", 0x20));
    require(!win32_password_edit(L"", 0x20));
    require(focused_field_kind(true, false) == FocusedFieldKind::Password);
    require(focused_field_kind(false, true) == FocusedFieldKind::Password);
    require(focused_field_kind(false, false) == FocusedFieldKind::Ordinary);
    require(focused_field_kind(false, std::nullopt) == FocusedFieldKind::Unknown);
    require(!cloud_clipboard_may_open(FocusedFieldKind::Password));
    require(cloud_clipboard_may_open(FocusedFieldKind::Ordinary) &&
            cloud_clipboard_may_open(FocusedFieldKind::Unknown));
    // Modes and stored switches are this process's own and never start the shell.
    for (auto command :
         {TrayMenuCommand::SelectChinese, TrayMenuCommand::SelectEnglish,
          TrayMenuCommand::ToggleFullwidth,
          TrayMenuCommand::ToggleChinesePunctuation,
          TrayMenuCommand::ToggleTranslations, TrayMenuCommand::SelectQuanpin,
          TrayMenuCommand::SelectShuangpin, TrayMenuCommand::SelectWubi,
          TrayMenuCommand::SelectJapanese, TrayMenuCommand::SelectKorean})
      require(!shell_surface_request(command));
    require(shell_route_argument(*settings) == L"settings");
    require(shell_route_argument(*about) == L"settings:about");
    require(shell_route_argument(*shell_surface_request(
                TrayMenuCommand::OpenEmojiPanel)) == L"emoji");

    // A panel request replaces whatever route this process inherited and leaves the rest of the environment, including drive current directories, alone.
    const std::wstring existing =
        std::wstring(L"PATH=C:\\Windows") + L'\0' +
        L"msime_client_route=stale" + L'\0' + L"=C:=C:\\work" + L'\0';
    const auto panel = entries(shell_environment_block(
        existing.c_str(), *shell_surface_request(TrayMenuCommand::OpenEmojiPanel)));
    require(contains(panel, L"PATH=C:\\Windows"));
    require(contains(panel, L"=C:=C:\\work"));
    require(!contains(panel, L"msime_client_route=stale"));

    // The settings row asks for no panel at all, so the shell opens its own window; the about row names a section instead.
    const auto plain = entries(shell_environment_block(existing.c_str(), *settings));
    require(plain.size() == 3 && contains(plain, L"PATH=C:\\Windows"));
    require(contains(plain, L"MSIME_CLIENT_ROUTE=settings"));
    const auto about_block = entries(shell_environment_block(existing.c_str(), *about));

    const ShellLaunchContext context{L"C:\\Users\\ime\\state",
                                    L"C:\\Users\\ime\\state\\runtime-options.json"};
    const std::wstring stale_paths =
        std::wstring(L"MSIME_CLIENT_STATE_DIR=C:\\stale") + L'\0' +
        L"MSIME_CLIENT_HOST_OPTIONS=C:\\stale\\runtime-options.json" + L'\0' +
        L"PATH=C:\\Windows" + L'\0';
    const auto configured_entries = entries(
        shell_environment_block(stale_paths.c_str(), *settings, &context));
    require(contains(configured_entries, L"MSIME_CLIENT_STATE_DIR=C:\\Users\\ime\\state"));
    require(contains(configured_entries,
                     L"MSIME_CLIENT_HOST_OPTIONS=C:\\Users\\ime\\state\\runtime-options.json"));
    require(!contains(configured_entries, L"MSIME_CLIENT_STATE_DIR=C:\\stale"));
    require(!contains(configured_entries,
                      L"MSIME_CLIENT_HOST_OPTIONS=C:\\stale\\runtime-options.json"));

    // Nothing but a short lowercase identifier may reach the child.
    for (const char *invalid : {"emoji panel", "Emoji", "emoji=1", "../etc"}) {
      bool rejected = false;
      try {
        (void)shell_environment_block(L"\0", ShellSurfaceRequest{invalid, {}});
      } catch (const std::invalid_argument &) {
        rejected = true;
      }
      require(rejected);
    }

    // 两种产品布局都只接受实际存在的设置与面板文件，保留显式设置路径和相对路径拒绝规则。
    const auto root =
        std::filesystem::temp_directory_path() /
        ("msime-shell-fixture-" +
         std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    std::filesystem::create_directories(root);
    require(!shell_executable(root, {}, *settings, PanelHost::Shared));
    require(!shell_executable("relative", {}, *settings, PanelHost::Shared));
    std::ofstream(root / panels_file) << "fixture";
    require(!shell_executable(root, {}, *settings, PanelHost::Shared));
    std::ofstream(root / settings_file) << "fixture";
    require(shell_executable(
                root, {}, *shell_surface_request(TrayMenuCommand::OpenSettings),
                PanelHost::Shared) == root / settings_file);
    require(shell_executable(
                root, {}, *shell_surface_request(TrayMenuCommand::OpenEmojiPanel),
                PanelHost::Shared) == root / panels_file);
    const auto configured_shell = root / "configured-settings.exe";
    std::ofstream(configured_shell) << "fixture";
    require(shell_executable(root, configured_shell.wstring(), *settings,
                             PanelHost::Shared) == configured_shell);
    require(shell_executable(root, configured_shell.wstring(),
                             *shell_surface_request(TrayMenuCommand::OpenEmojiPanel),
                             PanelHost::Shared) == root / panels_file);
    const auto configured = root / "elsewhere.exe";
    require(!shell_executable(root, configured.wstring(), *settings, PanelHost::Shared));
    require(!shell_executable(root, settings_file, *settings, PanelHost::Shared));
    // 开关打开时只找原生面板宿主，不悄悄退回共享应用：它不在时 Server 把依赖它的面板行显示为不可用。设置窗口的查找与开关无关。Win7 包两种状态是同一个文件。
    const auto emoji = *shell_surface_request(TrayMenuCommand::OpenEmojiPanel);
    require(shell_executable(root, {}, emoji, PanelHost::Native).has_value() ==
            (std::wstring_view(native_panels_file) == panels_file));
    std::ofstream(root / native_panels_file) << "fixture";
    require(shell_executable(root, {}, emoji, PanelHost::Native) == root / native_panels_file);
    require(shell_executable(root, {}, emoji, PanelHost::Shared) == root / panels_file);
    require(shell_executable(root, configured_shell.wstring(), *settings, PanelHost::Native) ==
            configured_shell);

    // Every surface also travels as the cross-platform route the shell parses.
    // A settings section becomes "settings:<category>": the bare section name is
    // not a route head, so emitting it would be rejected and silently fall back
    // to the default page.
    require(shell_surface_route(*shell_surface_request(
                TrayMenuCommand::OpenEmojiPanel)) == "emoji");
    require(shell_surface_route(*shell_surface_request(
                TrayMenuCommand::OpenKeyboardPanel)) == "keyboard");
    require(shell_surface_route(*settings) == "settings");
    require(shell_surface_route(*about) == "settings:about");
    require(contains(panel, L"MSIME_CLIENT_ROUTE=emoji"));
    require(contains(about_block, L"MSIME_CLIENT_ROUTE=settings:about"));

    // A stale route inherited from this process must not reach the shell.
    const std::wstring stale_route =
        std::wstring(L"PATH=C:\\Windows") + L'\0' + L"msime_client_route=stale" +
        L'\0';
    const auto replaced = entries(shell_environment_block(
        stale_route.c_str(), *shell_surface_request(TrayMenuCommand::OpenEmojiPanel)));
    for (const auto &entry : replaced)
      require(entry != L"msime_client_route=stale");
    require(contains(replaced, L"MSIME_CLIENT_ROUTE=emoji"));

    // The value filter still refuses anything outside the contract, and allows
    // at most the single separator a route needs.
    bool rejected = false;
    try {
      (void)shell_environment_block(stale_route.c_str(),
                                    ShellSurfaceRequest{"emoji&calc", {}});
    } catch (const std::invalid_argument &) {
      rejected = true;
    }
    require(rejected);
    rejected = false;
    try {
      (void)shell_environment_block(stale_route.c_str(),
                                    ShellSurfaceRequest{{}, "a:b"});
    } catch (const std::invalid_argument &) {
      rejected = true;
    }
    require(rejected);

    std::error_code error;
    std::filesystem::remove_all(root, error);
  } catch (const std::exception &error) {
    std::fputs(error.what(), stderr);
    std::fputs("\n", stderr);
    return 1;
  }

  return 0;
}
