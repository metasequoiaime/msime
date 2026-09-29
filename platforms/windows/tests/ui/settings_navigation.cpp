#include "../../settings/SettingsNavigation.h"
#include "ShellSurfaces.h"
#include <cstdio>
#include <set>
#include <stdexcept>
#include <string>

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
    // The design's sidebar: 18 unique pages in 5 non-empty groups, in group order.
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

    // The cross-platform service pages open the shared app on a route it accepts; native pages carry no route; the download page only links out.
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
    require(find_page("download")->host == PageHost::Download);
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

    // Every settings category the shared routes know still opens a page here, and the ids the tray sends land where the features moved.
    const std::set<std::string_view> categories{
        "account",     "chat",         "community",       "download",
        "appearance",  "input",        "expression",      "typing-statistics",
        "helpcode",    "shortcuts",    "dictionary",      "vocabulary",
        "skin",        "screen-keyboard", "handwriting",  "voice",
        "ai",          "tools",        "floating-toolbar", "developer",
        "help",        "about",        "feedback"};
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
    require(page_for_route("download") == "download");
    require(page_for_route("developer") == "dev");
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
