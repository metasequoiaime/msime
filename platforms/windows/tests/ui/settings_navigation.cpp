#include "../../settings/SettingsNavigation.h"
#include "ShellSurfaces.h"
#include <array>
#include <cstdio>
#include <set>
#include <stdexcept>
#include <string>
#include <utility>

using namespace msime::settings;
using msime::windows::ShellSurfaceRequest;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Settings navigation test failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)
namespace {
ShellSurfaceRequest request_for(const ShellTarget &target) {
  return {std::string(target.panel), std::string(target.page)};
}
// The launcher's own validation is the contract: a target it refuses would leave a button that silently does nothing.
bool launchable(const ShellTarget &target) {
  if (target.panel.empty() == target.page.empty())
    return false;
  try {
    const auto request = request_for(target);
    static constexpr wchar_t inherited[] = L"PATH=C:\\Windows\0";
    (void)msime::windows::shell_environment_block(inherited, request);
    (void)msime::windows::shell_route_argument(request);
    return true;
  } catch (const std::invalid_argument &) {
    return false;
  }
}
} // namespace
int main() {
  try {
    // The sidebar: 18 unique pages in 6 non-empty groups, in group order.
    std::set<std::string_view> ids;
    std::set<std::size_t> groups;
    std::size_t previous_group = 0;
    for (const auto &page : pages) {
      require(ids.insert(page.id).second);
      require(page.group < page_group_count);
      require(page.group >= previous_group);
      previous_group = page.group;
      groups.insert(page.group);
    }
    require(ids.size() == 18);
    require(groups.size() == page_group_count);
    // The same groups and order as settingsNavGroups in packages/ui/src/settings/settings-page-registry.ts (typing, appearance, more input methods, tools, account and community, support), so the native window and the shared settings UI list the same pages the same way.
    const std::array<std::pair<std::string_view, std::size_t>, 18> sidebar{{
        {"typing", 0},   {"expression", 0}, {"shortcuts", 0}, {"lexicon", 0},
        {"themes", 1},   {"candidate", 1},  {"toolbar", 1},   {"osk", 2},
        {"voice", 2},    {"hand", 2},       {"clip", 3},      {"stats", 3},
        {"plugins", 3},  {"account", 4},    {"community", 4}, {"dev", 5},
        {"feedback", 5}, {"about", 5}}};
    require(sidebar.size() == pages.size());
    for (std::size_t i = 0; i < pages.size(); ++i)
      require(pages[i].id == sidebar[i].first &&
              pages[i].group == sidebar[i].second);
    // The window opens on the first page of the sidebar.
    require(pages.front().id == default_page);

    // The cross-platform service pages open the shared app on a route it accepts; native pages carry no route.
    for (const auto &page : pages) {
      if (page.host == PageHost::Shell)
        require(launchable(page.shell));
      else
        require(page.shell.panel.empty() && page.shell.page.empty());
    }
    require(find_page("account")->shell.page == "account");
    require(find_page("clip")->shell.panel == "cloud-clipboard");
    require(find_page("stats")->shell.page == "typing-statistics");
    require(find_page("community")->shell.page == "community");
    // 插件打开共享应用的插件页，和云剪贴板、打字统计同在「工具」组。
    require(find_page("plugins")->shell.page == "plugins" &&
            find_page("plugins")->group == find_page("clip")->group &&
            find_page("plugins")->group == find_page("stats")->group);
    // The download links moved into the about page, so there is no download page any more.
    require(find_page("download") == nullptr);
    // MCP stays in this window's developer page and the core pages stay native.
    for (auto id : {"themes", "candidate", "typing", "expression", "shortcuts",
                    "lexicon", "dev", "about"})
      require(find_page(id)->host == PageHost::Native);
    for (const auto &target : shell_links::all)
      require(launchable(target));
    require(msime::windows::shell_route_argument(
                request_for(find_page("clip")->shell)) == L"cloud-clipboard");
    require(msime::windows::shell_route_argument(
                request_for(shell_links::appearance)) == L"settings:appearance");
    require(msime::windows::shell_route_argument(
                request_for(shell_links::keyboard_panel)) == L"keyboard");
    require(msime::windows::shell_route_argument(request_for(
                shell_links::handwriting_panel)) == L"handwriting");
    require(msime::windows::shell_route_argument(request_for(
                find_page("stats")->shell)) == L"settings:typing-statistics");
    require(msime::windows::shell_route_argument(request_for(
                find_page("plugins")->shell)) == L"settings:plugins");

    // Every settings category the shared routes know still opens a page here, and the ids the tray sends land where the features moved.
    const std::set<std::string_view> categories{
        "account",     "chat",         "community",       "download",
        "appearance",  "input",        "expression",      "typing-statistics",
        "helpcode",    "shortcuts",    "dictionary",      "vocabulary",
        "skin",        "screen-keyboard", "handwriting",  "voice",
        "ai",          "tools",        "plugins",      "floating-toolbar",
        "developer",   "help",         "about",        "feedback"};
    require(categories.size() == route_aliases.size());
    for (const auto &alias : route_aliases) {
      require(categories.count(alias.route) == 1);
      require(find_page(alias.page) != nullptr);
    }
    // `appearance` is the candidate window page everywhere (packages/ui, macOS, Linux); only `skin` opens the theme page.
    require(page_for_route("appearance") == "candidate");
    require(page_for_route("dictionary") == "lexicon");
    require(page_for_route("about") == "about");
    require(page_for_route("input") == "typing");
    require(page_for_route("skin") == "themes");
    require(page_for_route("vocabulary") == "lexicon");
    require(page_for_route("ai") == "expression");
    require(page_for_route("help") == "feedback");
    require(page_for_route("helpcode") == "typing");
    require(page_for_route("expression") == "expression");
    require(page_for_route("download") == "about");
    require(page_for_route("developer") == "dev");
    require(page_for_route("plugins") == "plugins");
    // A page id of this window opens itself; anything unknown falls back to the default page.
    for (const auto &page : pages)
      if (categories.count(page.id) == 0)
        require(page_for_route(page.id) == page.id);
    require(page_for_route("") == default_page);
    require(page_for_route("nonexistent") == default_page);
    require(find_page(default_page) != nullptr);
  } catch (const std::exception &error) {
    std::fputs(error.what(), stderr);
    std::fputs("\n", stderr);
    return 1;
  }
  return 0;
}
