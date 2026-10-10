#pragma once
#include <array>
#include <cstddef>
#include <string_view>

// The page model of the native Windows settings window (main.cpp): which pages exist, in which sidebar group, which of them this process draws, and which it hands to the shared desktop app (MSIME.exe). Kept free of Windows and WinRT headers so a host unit test can check it against the route vocabulary of ShellSurfaces.h. Labels live beside the controls in main.cpp; this header stays ASCII.
namespace msime::settings {

// Who serves a page. Native pages are drawn by this window. Shell pages belong to the shared desktop app and are opened there on the matching route instead of being rewritten here.
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

// 侧栏，顺序和分组与共享设置 UI 的 `settingsNavGroups`（`packages/ui/src/settings/settings-page-registry.ts`）一致：打字、外观、更多输入方式、工具、账号、支持。打字排在外观之前，因为打字会反复调整，外观通常只设一次。分组标题在 `main.cpp` 里。
inline constexpr std::array<Page, 18> pages{{
    {"typing", 0, PageHost::Native, {}},
    {"expression", 0, PageHost::Native, {}},
    {"shortcuts", 0, PageHost::Native, {}},
    {"lexicon", 0, PageHost::Native, {}},
    {"themes", 1, PageHost::Native, {}},
    {"candidate", 1, PageHost::Native, {}},
    {"toolbar", 1, PageHost::Native, {}},
    {"osk", 2, PageHost::Native, {}},
    {"voice", 2, PageHost::Native, {}},
    {"hand", 2, PageHost::Native, {}},
    // The shared 剪贴板 page holds the local clipboard history switch and opens the cloud clipboard panel, so this window draws neither.
    {"clip", 3, PageHost::Shell, {"", "tools"}},
    {"stats", 3, PageHost::Shell, {"", "typing-statistics"}},
    // Sound packs, music and command tables are imported and chosen in the shared app, which reads packs from a folder or archive the user picks; this window only opens it there.
    {"plugins", 3, PageHost::Shell, {"", "plugins"}},
    // AI 辅助 holds the 启用 switch, the providers, models and keys; all of it is set in the shared app.
    {"ai", 3, PageHost::Shell, {"", "ai"}},
    {"account", 4, PageHost::Shell, {"", "account"}},
    {"dev", 5, PageHost::Native, {}},
    {"feedback", 5, PageHost::Native, {}},
    {"about", 5, PageHost::Native, {}},
}};

inline constexpr std::string_view default_page = "typing";

// 原生页面自己不绘制、需要跳转到共享应用的界面：候选字体选择器；皮肤编辑器、取色器、主题包和社区皮肤库；词库管理；「输入」页上的辅助码插件；「标点与翻译」页的在线翻译服务；面板设置；帮助与反馈；以及屏幕键盘和手写面板本身。
namespace shell_links {
inline constexpr ShellTarget appearance{"", "appearance"};
inline constexpr ShellTarget skin{"", "skin"};
inline constexpr ShellTarget dictionary{"", "dictionary"};
inline constexpr ShellTarget vocabulary{"", "vocabulary"};
inline constexpr ShellTarget input{"", "input"};
inline constexpr ShellTarget screen_keyboard{"", "screen-keyboard"};
inline constexpr ShellTarget voice{"", "voice"};
inline constexpr ShellTarget handwriting{"", "handwriting"};
inline constexpr ShellTarget help{"", "help"};
inline constexpr ShellTarget feedback{"", "feedback"};
inline constexpr ShellTarget keyboard_panel{"keyboard", ""};
inline constexpr ShellTarget handwriting_panel{"handwriting", ""};
// 共享应用的「标点与翻译」页：在线翻译服务的选择和凭据只在那里编辑，本窗口的候选词翻译分组只放一个入口。
inline constexpr ShellTarget expression{"", "expression"};
inline constexpr std::array<ShellTarget, 13> all{
    {appearance, skin, dictionary, vocabulary, input, screen_keyboard, voice,
     handwriting, help, feedback, keyboard_panel, handwriting_panel, expression}};
} // namespace shell_links

struct RouteAlias {
  std::string_view route;
  std::string_view page;
};

// 共享路由词汇表（`client-core` 的 `host_surface::SettingsCategory`）认识的每个设置类别，映射到现在承载它的页面。每个 id 保持它在其他所有宿主上的含义：`appearance` 是候选窗口页，`skin` 是主题页。托盘会打开 `skin`、`dictionary` 和 `about`，其他桌面启动入口也用同样的名字。桌面端没有社区页，社区皮肤在共享应用的主题页上，所以 `community` 打开主题页；AI 对话是 AI 辅助的子页，`chat` 打开 AI 辅助。
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
    {"handwriting", "hand"},
    {"voice", "voice"},
    {"ai", "ai"},
    {"tools", "clip"},
    {"plugins", "plugins"},
    {"floating-toolbar", "toolbar"},
    {"developer", "dev"},
    {"help", "feedback"},
    {"about", "about"},
    {"feedback", "feedback"},
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

// 本次构建是否提供这个页面。手写识别器只认汉字，不提供手写的版本（日文、越南文和藏文版的 `MSIME_EDITION_HANDWRITING` 为 0）没有「手写输入」页，其他页面每个版本都有。版本的开关由调用方传进来，这个头文件因此不依赖版本宏，主机上的单元测试也能把两种答案都查到。
constexpr bool page_offered(std::string_view id, bool handwriting) {
  return id != "hand" || handwriting;
}

// 传入的路由在本次构建里打开的页面：与 `page_for_route` 相同，只是本版本不提供的页面改为打开默认页。
constexpr std::string_view offered_page_for_route(std::string_view route, bool handwriting) {
  const auto page = page_for_route(route);
  return page_offered(page, handwriting) ? page : default_page;
}

} // namespace msime::settings
