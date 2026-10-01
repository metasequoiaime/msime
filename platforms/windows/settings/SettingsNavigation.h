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

// The sidebar, in the order and grouping of the shared settings UI's settingsNavGroups (packages/ui/src/settings/settings-page-registry.ts): typing, appearance, more input methods, tools, account and community, and support. Typing comes before appearance because it is adjusted again and again while appearance is usually set once. main.cpp holds the group titles.
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
    {"clip", 3, PageHost::Shell, {"cloud-clipboard", ""}},
    {"stats", 3, PageHost::Shell, {"", "typing-statistics"}},
    // Sound packs, music and command tables are imported and chosen in the shared app, which reads packs from a folder or archive the user picks; this window only opens it there.
    {"plugins", 3, PageHost::Shell, {"", "plugins"}},
    {"account", 4, PageHost::Shell, {"", "account"}},
    {"community", 4, PageHost::Shell, {"", "community"}},
    {"dev", 5, PageHost::Native, {}},
    {"feedback", 5, PageHost::Native, {}},
    {"about", 5, PageHost::Native, {}},
}};

inline constexpr std::string_view default_page = "typing";

// The shared app's surfaces that native pages link to for what they do not draw themselves: the candidate font pickers; the skin editor, colour pickers and theme packages; dictionary management; the AI pages; the panel settings; help and feedback; the about page's update check; and the screen keyboard and handwriting panels themselves.
namespace shell_links {
inline constexpr ShellTarget appearance{"", "appearance"};
inline constexpr ShellTarget skin{"", "skin"};
inline constexpr ShellTarget dictionary{"", "dictionary"};
inline constexpr ShellTarget vocabulary{"", "vocabulary"};
inline constexpr ShellTarget helpcode{"", "helpcode"};
inline constexpr ShellTarget ai{"", "ai"};
inline constexpr ShellTarget chat{"", "chat"};
inline constexpr ShellTarget screen_keyboard{"", "screen-keyboard"};
inline constexpr ShellTarget voice{"", "voice"};
inline constexpr ShellTarget handwriting{"", "handwriting"};
inline constexpr ShellTarget help{"", "help"};
inline constexpr ShellTarget feedback{"", "feedback"};
inline constexpr ShellTarget about{"", "about"};
inline constexpr ShellTarget keyboard_panel{"keyboard", ""};
inline constexpr ShellTarget handwriting_panel{"handwriting", ""};
inline constexpr std::array<ShellTarget, 15> all{
    {appearance, skin, dictionary, vocabulary, helpcode, ai, chat,
     screen_keyboard, voice, handwriting, help, feedback, about, keyboard_panel,
     handwriting_panel}};
} // namespace shell_links

struct RouteAlias {
  std::string_view route;
  std::string_view page;
};

// Every settings category the shared route vocabulary knows (client-core host_surface::SettingsCategory), mapped to the page that now holds it. An id keeps the meaning it has on every other host: `appearance` is the candidate window page and `skin` the theme page. The tray opens `skin`, `dictionary` and `about`, and the other desktop launchers use the same names. `download` is no longer a page of its own: its links sit in the about page's version group, as in the shared settings UI.
inline constexpr std::array<RouteAlias, 24> route_aliases{{
    {"account", "account"},
    {"chat", "expression"},
    {"community", "community"},
    {"download", "about"},
    {"appearance", "candidate"},
    {"input", "typing"},
    {"expression", "expression"},
    {"typing-statistics", "stats"},
    {"helpcode", "typing"},
    {"shortcuts", "shortcuts"},
    {"dictionary", "lexicon"},
    {"vocabulary", "lexicon"},
    {"skin", "themes"},
    {"screen-keyboard", "osk"},
    {"handwriting", "hand"},
    {"voice", "voice"},
    {"ai", "expression"},
    {"tools", "shortcuts"},
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

} // namespace msime::settings
