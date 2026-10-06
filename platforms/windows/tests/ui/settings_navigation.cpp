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
    // 侧栏：18 个互不重复的页面，分在 6 个非空组里，按组的顺序排列。
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
    // 分组和顺序与 `packages/ui/src/settings/settings-page-registry.ts` 里的 `settingsNavGroups` 相同（打字、外观、更多输入方式、工具、账号、支持），这样原生窗口和共享设置 UI 以同样的方式列出同样的页面。
    const std::array<std::pair<std::string_view, std::size_t>, 18> sidebar{{
        {"typing", 0},   {"expression", 0}, {"shortcuts", 0}, {"lexicon", 0},
        {"themes", 1},   {"candidate", 1},  {"toolbar", 1},   {"osk", 2},
        {"voice", 2},    {"hand", 2},       {"clip", 3},      {"stats", 3},
        {"plugins", 3},  {"ai", 3},         {"account", 4},   {"dev", 5},
        {"feedback", 5}, {"about", 5}}};
    require(sidebar.size() == pages.size());
    for (std::size_t i = 0; i < pages.size(); ++i)
      require(pages[i].id == sidebar[i].first &&
              pages[i].group == sidebar[i].second);
    // 窗口打开时显示侧栏的第一个页面。
    require(pages.front().id == default_page);

    // 跨平台的服务页面在共享应用里以它接受的路由打开；原生页面没有路由。
    for (const auto &page : pages) {
      if (page.host == PageHost::Shell)
        require(launchable(page.shell));
      else
        require(page.shell.panel.empty() && page.shell.page.empty());
    }
    require(find_page("account")->shell.page == "account");
    require(find_page("clip")->shell.page == "tools");
    require(find_page("stats")->shell.page == "typing-statistics");
    // 插件和 AI 辅助打开共享应用的对应页面，和剪贴板、打字统计同在「工具」组。
    require(find_page("plugins")->shell.page == "plugins" &&
            find_page("plugins")->group == find_page("clip")->group &&
            find_page("plugins")->group == find_page("stats")->group);
    require(find_page("ai")->shell.page == "ai" &&
            find_page("ai")->group == find_page("plugins")->group);
    // 桌面端没有社区页：社区皮肤在共享应用的主题页上，从本窗口的主题页打开。
    require(find_page("community") == nullptr);
    // 下载链接已移到关于页，所以不再有下载页。
    require(find_page("download") == nullptr);
    // MCP stays in this window's developer page and the core pages stay native.
    for (auto id : {"themes", "candidate", "typing", "expression", "shortcuts",
                    "lexicon", "dev", "about"})
      require(find_page(id)->host == PageHost::Native);
    for (const auto &target : shell_links::all)
      require(launchable(target));
    require(msime::windows::shell_route_argument(
                request_for(find_page("clip")->shell)) == L"settings:tools");
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
    require(msime::windows::shell_route_argument(request_for(
                find_page("ai")->shell)) == L"settings:ai");
    require(msime::windows::shell_route_argument(
                request_for(shell_links::input)) == L"settings:input");

    // Every settings category the shared routes know still opens a page here, and the ids the tray sends land where the features moved.
    const std::set<std::string_view> categories{
        "account",    "chat",          "community",       "appearance",
        "input",      "expression",    "typing-statistics", "shortcuts",
        "dictionary", "vocabulary",    "skin",            "screen-keyboard",
        "handwriting", "voice",        "ai",              "tools",
        "plugins",    "floating-toolbar", "developer",    "help",
        "about",      "feedback"};
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
    require(page_for_route("ai") == "ai");
    require(page_for_route("chat") == "ai");
    require(page_for_route("community") == "themes");
    require(page_for_route("tools") == "clip");
    require(page_for_route("help") == "feedback");
    require(page_for_route("expression") == "expression");
    require(page_for_route("developer") == "dev");
    require(page_for_route("plugins") == "plugins");
    // A page id of this window opens itself; anything unknown falls back to the default page.
    for (const auto &page : pages)
      if (categories.count(page.id) == 0)
        require(page_for_route(page.id) == page.id);
    require(page_for_route("") == default_page);
    require(page_for_route("nonexistent") == default_page);
    require(find_page(default_page) != nullptr);
    // 手写页只在提供手写的版本里：没有手写的版本（日文、越南文和藏文版）不列出它，指向它的路由打开默认页；其他页面每个版本都有。
    for (const auto &page : pages) {
      require(page_offered(page.id, true));
      require(page_offered(page.id, false) == (page.id != "hand"));
    }
    require(offered_page_for_route("handwriting", true) == "hand");
    require(offered_page_for_route("handwriting", false) == default_page);
    require(offered_page_for_route("hand", false) == default_page);
    require(offered_page_for_route("voice", false) == "voice");
  } catch (const std::exception &error) {
    std::fputs(error.what(), stderr);
    std::fputs("\n", stderr);
    return 1;
  }
  return 0;
}
