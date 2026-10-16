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
    // 侧栏：16 个互不重复的页面，分在 6 个非空组里，按组的顺序排列。
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
    require(ids.size() == 16);
    require(groups.size() == page_group_count);
    // 分组和顺序与共享设置 UI 的桌面侧栏相同：`packages/ui/src/settings/settings-page-registry.ts` 里的 `settingsNavGroups`（打字、外观、更多输入方式、工具、账号、支持）去掉 `desktopMergedPages` 并进「关于」的「帮助与反馈」，这样原生窗口和共享设置 UI 以同样的方式列出同样的页面。
    const std::array<std::pair<std::string_view, std::size_t>, 16> sidebar{{
        {"typing", 0},  {"expression", 0}, {"shortcuts", 0}, {"lexicon", 0},
        {"themes", 1},  {"candidate", 1},  {"toolbar", 1},   {"osk", 2},
        {"voice", 2},   {"clip", 3},       {"stats", 3},     {"plugins", 3},
        {"ai", 3},      {"account", 4},    {"dev", 5},
        {"about", 5}}};
    require(sidebar.size() == pages.size());
    for (std::size_t i = 0; i < pages.size(); ++i)
      require(pages[i].id == sidebar[i].first &&
              pages[i].group == sidebar[i].second);
    // 窗口打开时显示侧栏的第一个页面。
    require(pages.front().id == default_page);

    // 每个页面都在本窗口里原生绘制，没有交给共享应用的设置页；万一再有，它必须是共享应用接受的路由，原生页面没有路由。
    for (const auto &page : pages) {
      if (page.host == PageHost::Shell)
        require(launchable(page.shell));
      else
        require(page.shell.panel.empty() && page.shell.page.empty());
    }
    // AI 辅助和剪贴板、打字统计、插件同在「工具」组，都在本窗口里原生绘制。
    require(find_page("plugins")->host == PageHost::Native &&
            find_page("plugins")->group == find_page("clip")->group &&
            find_page("plugins")->group == find_page("stats")->group);
    require(find_page("ai")->host == PageHost::Native &&
            find_page("ai")->group == find_page("plugins")->group);
    for (const auto &page : pages)
      require(page.host == PageHost::Native);
    // 桌面端没有社区页：社区皮肤在共享应用的主题页上，从本窗口的主题页打开。
    require(find_page("community") == nullptr);
    // 下载链接已移到关于页，所以不再有下载页。
    require(find_page("download") == nullptr);
    // 手写设置只有一个打开面板的按钮，并进了屏幕键盘页，没有单独的手写页。
    require(find_page("hand") == nullptr);
    // MCP stays in this window's developer page and the core pages stay native.
    for (auto id : {"themes", "candidate", "typing", "expression", "shortcuts", "lexicon",
                    "clip", "stats", "plugins", "ai", "account", "dev", "about"})
      require(find_page(id)->host == PageHost::Native);
    // 本窗口不再打开共享应用的任何设置页：留下的都是输入工具的面板。
    for (const auto &target : shell_links::all) {
      require(launchable(target));
      require(target.page.empty() && !target.panel.empty());
    }
    // 剪贴板页在本窗口里原生绘制，只有云剪贴板面板仍由共享应用打开。
    require(msime::windows::shell_route_argument(
                request_for(shell_links::cloud_clipboard)) == L"cloud-clipboard");
    require(msime::windows::shell_route_argument(
                request_for(shell_links::keyboard_panel)) == L"keyboard");
    require(msime::windows::shell_route_argument(request_for(
                shell_links::handwriting_panel)) == L"handwriting");
    // 语音页的识别服务与润色在本窗口里原生绘制，只有语音面板仍由共享应用打开，路由与托盘的「语音输入」相同。
    require(msime::windows::shell_route_argument(request_for(shell_links::voice_panel)) == L"voice");
    require(msime::windows::shell_route_argument(
                request_for(shell_links::cloud_dictionary)) == L"cloud-dictionary");

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
    // 帮助与反馈并进了「关于」（共享页的 `desktopMergedPages`），托盘和工具栏菜单的「帮助」「反馈」都打开它；本窗口没有 feedback 页。
    require(page_for_route("help") == "about");
    require(page_for_route("feedback") == "about");
    require(find_page("feedback") == nullptr);
    require(page_for_route("expression") == "expression");
    require(page_for_route("developer") == "dev");
    require(page_for_route("plugins") == "plugins");
    require(page_for_route("typing-statistics") == "stats");
    // A page id of this window opens itself; anything unknown falls back to the default page.
    for (const auto &page : pages)
      if (categories.count(page.id) == 0)
        require(page_for_route(page.id) == page.id);
    require(page_for_route("") == default_page);
    require(page_for_route("nonexistent") == default_page);
    require(find_page(default_page) != nullptr);
    // `handwriting` 打开并进了手写的屏幕键盘页；没有手写的版本（日文、越南文和藏文版）没有手写可设置，它打开默认页。其他路由和每个页面在每个版本里都一样。
    require(page_for_route("handwriting") == "osk");
    require(offered_page_for_route("handwriting", true) == "osk");
    require(offered_page_for_route("handwriting", false) == default_page);
    require(offered_page_for_route("screen-keyboard", false) == "osk");
    require(offered_page_for_route("voice", false) == "voice");
    for (const auto &page : pages)
      for (const bool handwriting : {true, false})
        require(offered_page_for_route(page.id, handwriting) == page.id);
    // 旧的页面 id `hand` 不再是页面，也不是共享路由的类别，和其他未知 id 一样打开默认页。
    require(page_for_route("hand") == default_page);
  } catch (const std::exception &error) {
    std::fputs(error.what(), stderr);
    std::fputs("\n", stderr);
    return 1;
  }
  return 0;
}
