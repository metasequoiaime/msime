#include "ShellSurfaces.h"
#include <chrono>
#include <cstdio>
#include <fstream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
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
    require(shell_surface_request(TrayMenuCommand::ToggleVoiceInput)->panel ==
            "voice");
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
    // Modes and stored switches are this process's own and never start the shell.
    for (auto command :
         {TrayMenuCommand::SelectChinese, TrayMenuCommand::SelectEnglish,
          TrayMenuCommand::ToggleFullwidth,
          TrayMenuCommand::ToggleChinesePunctuation,
          TrayMenuCommand::ToggleTranslations, TrayMenuCommand::SelectQuanpin,
          TrayMenuCommand::SelectShuangpin, TrayMenuCommand::SelectWubi,
          TrayMenuCommand::SelectJapanese})
      require(!shell_surface_request(command));
    require(shell_route_argument(*settings) == L"settings");
    require(shell_route_argument(*about) == L"settings:about");
    require(shell_route_argument(*shell_surface_request(
                TrayMenuCommand::OpenEmojiPanel)) == L"emoji");

    // A panel request replaces whatever this process inherited and leaves the
    // rest of the environment, including drive current directories, alone.
    const std::wstring existing =
        std::wstring(L"PATH=C:\\Windows") + L'\0' +
        L"msime_client_panel=stale" + L'\0' + L"MSIME_CLIENT_SETTINGS_PAGE=old" +
        L'\0' + L"=C:=C:\\work" + L'\0';
    const auto panel = entries(shell_environment_block(
        existing.c_str(), *shell_surface_request(TrayMenuCommand::OpenEmojiPanel)));
    require(contains(panel, L"PATH=C:\\Windows"));
    require(contains(panel, L"=C:=C:\\work"));
    require(contains(panel, L"MSIME_CLIENT_PANEL=emoji"));
    for (const auto &entry : panel)
      require(entry != L"msime_client_panel=stale" &&
              entry != L"MSIME_CLIENT_SETTINGS_PAGE=old");

    // The settings row asks for no panel at all, so the shell opens its own
    // window; the about row names a section instead.
    const auto plain = entries(shell_environment_block(existing.c_str(), *settings));
    require(plain.size() == 3 && contains(plain, L"PATH=C:\\Windows"));
    require(contains(plain, L"MSIME_CLIENT_ROUTE=settings"));
    const auto about_block = entries(shell_environment_block(existing.c_str(), *about));
    require(contains(about_block, L"MSIME_CLIENT_SETTINGS_PAGE=about"));
    for (const auto &entry : about_block)
      require(entry.rfind(L"MSIME_CLIENT_PANEL=", 0) != 0);

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

    // Discovery accepts only an existing file, and the packaged name wins over
    // a developer build sitting in the same directory.
    const auto root =
        std::filesystem::temp_directory_path() /
        ("msime-shell-fixture-" +
         std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    std::filesystem::create_directories(root);
    require(!shell_executable(root, {}));
    require(!shell_executable("relative", {}));
    std::ofstream(root / "MSIME.exe") << "fixture";
    require(shell_executable(root, {}) == root / "MSIME.exe");
    std::ofstream(root / "msime-client-settings.exe") << "fixture";
    require(shell_executable(root, {}) == root / "msime-client-settings.exe");
    require(shell_executable(
                root, {}, *shell_surface_request(TrayMenuCommand::OpenSettings)) ==
            root / "msime-client-settings.exe");
    require(shell_executable(
                root, {}, *shell_surface_request(TrayMenuCommand::OpenEmojiPanel)) ==
            root / "MSIME.exe");
    const auto configured_shell = root / "configured-settings.exe";
    std::ofstream(configured_shell) << "fixture";
    require(shell_executable(root, configured_shell.wstring(), *settings) ==
            configured_shell);
    require(shell_executable(root, configured_shell.wstring(),
                             *shell_surface_request(TrayMenuCommand::OpenEmojiPanel)) ==
            root / "MSIME.exe");
    const auto configured = root / "elsewhere.exe";
    require(!shell_executable(root, configured.wstring()));
    std::ofstream(configured) << "fixture";
    require(shell_executable(root, configured.wstring()) == configured);
    require(!shell_executable(root, L"msime-client-settings.exe"));

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
