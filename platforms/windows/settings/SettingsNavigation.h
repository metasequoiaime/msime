#pragma once
#include <array>
#include <cstddef>
#include <string_view>

// The page model of the native Windows settings window (main.cpp): which pages exist, in which sidebar group, which of them this process draws, and which it hands to the shared desktop app (MSIME.exe). Kept free of Windows and WinRT headers so a host unit test can check it against the route vocabulary of ShellSurfaces.h. Labels live beside the controls in main.cpp; this header stays ASCII.
namespace msime::settings {

// Who serves a page. Native pages are drawn by this window. Shell pages belong to the shared desktop app and are opened there on the matching route instead of being rewritten here; since 账号与同步 became native there are none, and every setting is changed in this window.
enum class PageHost { Native, Shell };

// A surface of the shared desktop app, in the two-field form ShellSurfaceRequest carries: a settings category travels as `page` and a panel as `panel`, never both.
struct ShellTarget {
  std::string_view panel;
  std::string_view page;
};

struct Page {
  std::string_view id;
  std::size_t group;
  PageHost host;
  ShellTarget shell;
};

inline constexpr std::size_t page_group_count = 6;

// 侧栏，顺序和分组与共享设置 UI 在桌面上的侧栏一致，即 `settingsNavGroups`（`packages/ui/src/settings/settings-page-registry.ts`）去掉 `desktopMergedPages` 里并进别的页的那些：打字、外观、更多输入方式、工具、账号、支持。「帮助与反馈」并进了「关于」，所以支持组没有 feedback，它的路由打开「关于」。打字排在外观之前，因为打字会反复调整，外观通常只设一次。分组标题在 `main.cpp` 里。「手写输入」没有单独的页面：Windows 的手写设置只有一个打开面板的按钮，与共享设置 UI 的 `settingsPageProjections`（`handwritingOnScreenKeyboardPage`）一样并进「屏幕键盘」页，那一页在提供手写的版本里叫「屏幕键盘与手写」。
inline constexpr std::array<Page, 16> pages{{
    {"typing", 0, PageHost::Native, {}},
    {"expression", 0, PageHost::Native, {}},
    {"shortcuts", 0, PageHost::Native, {}},
    {"lexicon", 0, PageHost::Native, {}},
    {"themes", 1, PageHost::Native, {}},
    {"candidate", 1, PageHost::Native, {}},
    {"toolbar", 1, PageHost::Native, {}},
    {"osk", 2, PageHost::Native, {}},
    {"voice", 2, PageHost::Native, {}},
    {"clip", 3, PageHost::Native, {}},
    {"stats", 3, PageHost::Native, {}},
    {"plugins", 3, PageHost::Native, {}},
    {"ai", 3, PageHost::Native, {}},
    {"account", 4, PageHost::Native, {}},
    {"dev", 5, PageHost::Native, {}},
    {"about", 5, PageHost::Native, {}},
}};

inline constexpr std::string_view default_page = "typing";

// 原生页面自己不绘制、需要跳转到共享应用的界面，都是输入工具的面板而不是设置：云词库和云剪贴板面板，以及屏幕键盘、语音输入和手写面板本身。社区皮肤和社区插件是「主题」「插件」页自己的标签，社区词库与回复模板由「账号与同步」页「我的内容」在本窗口里打开，都不在这里。
namespace shell_links {
// The cloud dictionary is a panel of the shared app on every desktop, opened from the lexicon page.
inline constexpr ShellTarget cloud_dictionary{"cloud-dictionary", ""};
// The cloud clipboard is a panel of the shared app as well, opened from the 剪贴板 page.
inline constexpr ShellTarget cloud_clipboard{"cloud-clipboard", ""};
// The voice panel (the tray's 语音输入 entry opens the same route), from the 语音输入 page's 语音面板 group.
inline constexpr ShellTarget voice_panel{"voice", ""};
inline constexpr ShellTarget keyboard_panel{"keyboard", ""};
inline constexpr ShellTarget handwriting_panel{"handwriting", ""};
inline constexpr std::array<ShellTarget, 5> all{{voice_panel, keyboard_panel, handwriting_panel,
                                                 cloud_dictionary, cloud_clipboard}};
} // namespace shell_links

struct RouteAlias {
  std::string_view route;
  std::string_view page;
};

// 共享路由词汇表（`client-core` 的 `host_surface::SettingsCategory`）认识的每个设置类别，映射到现在承载它的页面。每个 id 保持它在其他所有宿主上的含义：`appearance` 是候选窗口页，`skin` 是主题页。托盘会打开 `skin`、`dictionary` 和 `about`，其他桌面启动入口也用同样的名字。本窗口没有单独的社区页，社区皮肤是主题页的一个标签，所以 `community` 打开主题页；AI 对话是 AI 辅助的子页（Windows 不提供，见 README），`chat` 打开 AI 辅助。帮助与反馈和共享桌面侧栏一样并进了「关于」（`desktopMergedPages`），托盘和工具栏菜单的「帮助」「反馈」（`help`、`feedback`）都打开「关于」。
inline constexpr std::array<RouteAlias, 22> route_aliases{{
    {"account", "account"},
    {"chat", "ai"},
    {"community", "themes"},
    {"appearance", "candidate"},
    {"input", "typing"},
    {"expression", "expression"},
    {"typing-statistics", "stats"},
    {"shortcuts", "shortcuts"},
    {"dictionary", "lexicon"},
    {"vocabulary", "lexicon"},
    {"skin", "themes"},
    {"screen-keyboard", "osk"},
    {"handwriting", "osk"},
    {"voice", "voice"},
    {"ai", "ai"},
    {"tools", "clip"},
    {"plugins", "plugins"},
    {"floating-toolbar", "toolbar"},
    {"developer", "dev"},
    {"help", "about"},
    {"about", "about"},
    {"feedback", "about"},
}};

constexpr const Page *find_page(std::string_view id) {
  for (const auto &page : pages)
    if (page.id == id)
      return &page;
  return nullptr;
}

// The page an incoming `settings:<id>` route opens. A category id resolves through the alias table; a page id of this window resolves to itself. Anything else, including an empty id, opens the default page.
constexpr std::string_view page_for_route(std::string_view route) {
  for (const auto &alias : route_aliases)
    if (alias.route == route)
      return alias.page;
  if (const auto *page = find_page(route))
    return page->id;
  return default_page;
}

// 传入的路由在本次构建里打开的页面：与 `page_for_route` 相同，只是手写识别器只认汉字，不提供手写的版本（日文、越南文和藏文版的 `MSIME_EDITION_HANDWRITING` 为 0）没有手写可设置，`handwriting` 改为打开默认页，与共享设置 UI 只在有手写时才把 `handwriting` 别名到屏幕键盘页一致。每个页面每个版本都有。版本的开关由调用方传进来，这个头文件因此不依赖版本宏，主机上的单元测试也能把两种答案都查到。
constexpr std::string_view offered_page_for_route(std::string_view route, bool handwriting) {
  if (route == "handwriting" && !handwriting)
    return default_page;
  return page_for_route(route);
}

} // namespace msime::settings
