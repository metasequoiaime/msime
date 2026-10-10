#pragma once
#include "../../common/InputSchemeTraits.h"
#include "TrayMenuCommand.h"
#include <algorithm>
#include <cstddef>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace msime::windows {
// Tray menu contents and geometry. This header decides what the menu offers and where each row sits; drawing and window placement stay with the renderer, so the rules are testable without a desktop.
//
// 卡片按共享设计排列：带标志和产品名的标题、输入语言、模式开关、输入方案和主题两个翻页行、宿主工具条，以及云剪贴板和设置应用的几个页面。行用勾选列而不是行首图标，只有工具条画成图标。
enum class TrayMenuRowKind {
  // Logo and product name. Not a command.
  Header,
  // A hairline between groups.
  Separator,
  // A group caption such as 输入方案. Not a command.
  Label,
  // A command row: check mark column, label and an optional trailing hint.
  Item,
  // One cell of the tool strip. Consecutive tools share one row, each an equal share of the width with its glyph over a short caption.
  Tool,
};
struct TrayMenuItem {
  TrayMenuRowKind kind = TrayMenuRowKind::Item;
  // Meaningful only for Item and Tool rows.
  TrayMenuCommand command = TrayMenuCommand::OpenSettings;
  std::string label;
  // Trailing text of an Item row: the shortcut that does the same thing, or the value the row opens (the theme name). Empty draws nothing.
  std::string hint;
  // Glyph of a Tool cell, from the same icon font the toolbar uses.
  wchar_t icon = 0;
  // Text drawn when the icon font lacks the glyph, as on Windows 10 builds whose Segoe MDL2 predates it.
  const wchar_t *icon_fallback = L"";
  // A Tool that turns something on and off instead of opening a surface; it is drawn filled while on.
  bool toggle = false;
  // A row whose host capability or live state is missing is shown disabled rather than silently doing nothing when clicked.
  bool available = true;
  // Item: the check mark. Tool with toggle: on.
  bool checked = false;
  // 带值的一行携带的值：主题页的行是主题 id，其他行为空。
  std::string value;
  // 打开卡片下一页的行（输入方案、主题），右侧画一个向右的箭头。
  bool submenu = false;
  // 返回主页的一行，勾选列画一个向左的箭头。
  bool back = false;
};
// Which surfaces this host can actually open. Anything absent stays visible but disabled, matching how the shipped menu never hides its entries.
struct TrayMenuCapabilities {
  bool floating_toolbar = true;
  bool emoji_panel = false;
  bool handwriting_panel = false;
  bool keyboard_panel = false;
  bool voice_input = false;
  bool settings = false;
  // The Cantonese, Zhuyin and Stroke dictionaries installed beside the resources (language-dictionaries/msime-cantonese.db, msime-zhuyin.db and msime-stroke.db). Without one the Engine answers that scheme with quanpin, so its row is disabled rather than selecting a scheme that would type pinyin.
  bool cantonese = false;
  bool zhuyin = false;
  bool stroke = false;
  // 本版本提供的方案。不在其中的方案行不出现，而不是显示为不可用：那个方案在这个版本里根本不存在。full 提供全部方案。
  scheme::OfferedSchemes schemes = scheme::edition_schemes();
  // 卡片标题和「关于」行里的产品名（UTF-8），按版本取；full 是「水杉输入法」。
  std::string product_name = MSIME_EDITION_DISPLAY_NAME_UTF8;
  // 共享应用 MSIME.exe 在场时「云剪贴板…」才打得开，否则这一行显示为不可用。
  bool cloud_clipboard = false;
};
// 主题目录里的一项：写进 global_theme 的 id 和主题页上显示的标题。
struct TrayMenuTheme {
  std::string id;
  std::string title;
};
// What the menu shows, sampled by the Server each time the card opens or redraws after a switch.
struct TrayMenuState {
  bool floating_toolbar = false;
  // The focused TIP's reported modes. Unknown (no focused session, or the Server could not read them without waiting) disables the rows instead of guessing a default.
  std::optional<bool> chinese, fullwidth, chinese_punctuation;
  // The Engine's own English mode: the TIP still reports Chinese, but letters are typed as English until it is left.
  bool dedicated_english = false;
  // Stored preferences.
  bool translations = true;
  std::string scheme = "quanpin";
  std::string shuangpin_profile = "xiaohe";
  // 存储的 `wubi_profile`，五笔一行按它标出 86 或 98。
  std::string wubi_profile = "wubi86";
  // Title of the selected global theme, shown beside 主题. Empty draws no hint.
  std::string theme_title;
  // The configured CN/EN key, shown beside the language row.
  std::string language_hint;
  // 存储的 traditional_chinese_output，「繁体输出」一行的勾。
  bool traditional_output = false;
  // keybindings.toggle_character_set_ctrl_shift_f 开着时「繁体输出」旁写出 Ctrl + Shift + F，关着时这个键不切换，也就不写。
  bool character_set_shortcut = true;
  // 存储的 punctuation_lock 是 chinese 或 english：标点被钉住，切换不起作用，「中文标点」一行显示为不可用，和 macOS 输入菜单一样。
  bool punctuation_locked = false;
  // 当前的全局主题 id 和本平台的主题目录（按选择器顺序），主题页按它们列出各行。目录为空时主题页没有可选的行。
  std::string theme = "system";
  std::vector<TrayMenuTheme> themes;
};
// 卡片的页。和 macOS 输入菜单的「输入方案」「主题」两个子菜单对应：主页上各占一行，点开后整张卡片换成那一页，第一行返回主页。卡片不抢焦点，没有子菜单那样的悬停展开，翻页是最不容易误触的做法；十个方案收进一页后，加上英文候选模式、繁体输出和云剪贴板，卡片仍放得进 150% 缩放的 1080p 屏幕。
enum class TrayMenuPage { Main, Schemes, Themes };
// 翻页的行要去的页；其他行为空。
inline std::optional<TrayMenuPage> tray_menu_page_target(TrayMenuCommand command) {
  if (command == TrayMenuCommand::ShowSchemes)
    return TrayMenuPage::Schemes;
  if (command == TrayMenuCommand::ShowThemes)
    return TrayMenuPage::Themes;
  if (command == TrayMenuCommand::ShowMain)
    return TrayMenuPage::Main;
  return std::nullopt;
}
// 「英文候选模式」一行点下去做什么。和 TIP 的 Ctrl+Shift+E 用同一条规则：TIP 报告中文状态、方案有 Server 管的英文候选模式（韩文、注音、越南文、藏文在 TIP 自己的宿主会话里组字，没有这个模式）时才可用；TIP 在英文状态或状态未知时不可用。可用时按 Engine 现在的模式进入或退出。
enum class TrayMenuEnglishAction { Unavailable, Enter, Leave };
inline TrayMenuEnglishAction
tray_menu_dedicated_english_action(std::optional<bool> chinese,
                                   bool dedicated_english,
                                   const std::string &scheme_name) {
  if (!chinese || !*chinese ||
      scheme::AlwaysInlinePreedit(scheme::scheme_from_name(scheme_name)))
    return TrayMenuEnglishAction::Unavailable;
  return dedicated_english ? TrayMenuEnglishAction::Leave
                           : TrayMenuEnglishAction::Enter;
}
// The CN/EN key a user presses, in the order the TIP matches them. Empty when every binding is off, since the rows still switch.
inline std::string tray_menu_language_hint(bool shift, bool ctrl,
                                           bool ctrl_alt_space) {
  if (shift)
    return "Shift";
  if (ctrl)
    return "Ctrl";
  if (ctrl_alt_space)
    return "Ctrl + Alt + Space";
  return {};
}
// The shuangpin row names the stored layout, as the design's 双拼（小鹤）does. An unknown profile keeps the plain scheme name rather than a wrong one.
inline std::string tray_menu_shuangpin_label(const std::string &profile) {
  if (profile == "xiaohe")
    return "双拼（小鹤）";
  if (profile == "ziranma")
    return "双拼（自然码）";
  if (profile == "microsoft")
    return "双拼（微软）";
  if (profile == "shoudao")
    return "双拼（首道）";
  return "双拼";
}
// 五笔一行标出存储的码表版本，与工具栏的「五笔 86」「五笔 98」一致；不认识的版本只写方案名，不猜一个错的。
inline std::string tray_menu_wubi_label(const std::string &profile) {
  if (profile == "wubi86")
    return "五笔 86";
  if (profile == "wubi98")
    return "五笔 98";
  return "五笔";
}
// The stored `scheme` value a scheme row selects, or null for every other row.
inline const char *tray_menu_scheme(TrayMenuCommand command) {
  if (command == TrayMenuCommand::SelectQuanpin)
    return "quanpin";
  if (command == TrayMenuCommand::SelectShuangpin)
    return "shuangpin";
  if (command == TrayMenuCommand::SelectWubi)
    return "wubi";
  if (command == TrayMenuCommand::SelectJapanese)
    return "japanese";
  if (command == TrayMenuCommand::SelectKorean)
    return "korean";
  if (command == TrayMenuCommand::SelectCantonese)
    return "cantonese";
  if (command == TrayMenuCommand::SelectZhuyin)
    return "zhuyin";
  if (command == TrayMenuCommand::SelectVietnamese)
    return "vietnamese";
  if (command == TrayMenuCommand::SelectTibetan)
    return "tibetan";
  if (command == TrayMenuCommand::SelectStroke)
    return "stroke";
  return nullptr;
}
// 「输入方案」那一组单选行，托盘卡片和悬浮工具栏的切换输入方案按钮共用这一份：两处列出的方案、名字、可用与否和勾选都一样，选中后都走 Server 同一条写偏好的路径。只列出本版本提供的方案；缺词典的粤拼、注音和笔画显示为不可用。
inline std::vector<TrayMenuItem>
tray_menu_scheme_items(const TrayMenuCapabilities &capabilities,
                       const TrayMenuState &state) {
  std::vector<TrayMenuItem> items;
  items.reserve(10);
  auto scheme_row = [&](TrayMenuCommand command, std::string label,
                        bool available) {
    const char *name = tray_menu_scheme(command);
    if (!capabilities.schemes.offers(scheme::scheme_from_name(name)))
      return;
    TrayMenuItem item;
    item.command = command;
    item.label = std::move(label);
    item.available = available;
    item.checked = state.scheme == name;
    items.push_back(std::move(item));
  };
  scheme_row(TrayMenuCommand::SelectQuanpin, "全拼", true);
  scheme_row(TrayMenuCommand::SelectShuangpin,
             tray_menu_shuangpin_label(state.shuangpin_profile), true);
  scheme_row(TrayMenuCommand::SelectWubi,
             tray_menu_wubi_label(state.wubi_profile), true);
  scheme_row(TrayMenuCommand::SelectJapanese, "日文", true);
  scheme_row(TrayMenuCommand::SelectKorean, "韩文", true);
  scheme_row(TrayMenuCommand::SelectCantonese, "粤拼", capabilities.cantonese);
  scheme_row(TrayMenuCommand::SelectZhuyin, "注音", capabilities.zhuyin);
  scheme_row(TrayMenuCommand::SelectVietnamese, "越南文", true);
  scheme_row(TrayMenuCommand::SelectTibetan, "藏文", true);
  scheme_row(TrayMenuCommand::SelectStroke, "笔画", capabilities.stroke);
  return items;
}
// 悬浮工具栏的右键菜单，行和顺序照搬 macOS 工具栏设置按钮的实用菜单（CreateMetasequoiaFloatingToolbarUtilityMenu）。打开设置、检查更新、帮助、关于和反馈都要设置窗口（检查更新在设置窗口的「关于」页里完成）；没有设置窗口时这几行显示为不可用。表情与符号打开系统自带的表情面板，访问官网和隐藏工具栏不依赖外壳，总是可用。
inline std::vector<TrayMenuItem>
toolbar_utility_menu_items(const TrayMenuCapabilities &capabilities) {
  std::vector<TrayMenuItem> items;
  items.reserve(10);
  auto row = [&](TrayMenuCommand command, std::string label, bool available) {
    TrayMenuItem item;
    item.command = command;
    item.label = std::move(label);
    item.available = available;
    items.push_back(std::move(item));
  };
  auto separator = [&] {
    TrayMenuItem item;
    item.kind = TrayMenuRowKind::Separator;
    item.available = false;
    items.push_back(std::move(item));
  };
  row(TrayMenuCommand::OpenSystemEmoji, "表情与符号…", true);
  row(TrayMenuCommand::OpenSettings, "打开设置…", capabilities.settings);
  row(TrayMenuCommand::CheckForUpdates, "检查更新…", capabilities.settings);
  separator();
  row(TrayMenuCommand::OpenWebsite, "访问 msime.app", true);
  row(TrayMenuCommand::OpenHelp, "使用帮助…", capabilities.settings);
  row(TrayMenuCommand::OpenAbout, "关于" + capabilities.product_name + "…",
      capabilities.settings);
  row(TrayMenuCommand::OpenFeedback, "问题反馈…", capabilities.settings);
  separator();
  row(TrayMenuCommand::HideFloatingToolbar, "隐藏悬浮状态栏", true);
  return items;
}
// 官网地址，与 macOS 工具栏菜单的「访问 msime.app」相同。
inline constexpr const wchar_t *toolbar_website_url = L"https://msime.app/";
// Shortcuts the TIP binds itself (KeyEventSink.cpp and the preserved keys in CompositionProcessorEngine.cpp); they are fixed, unlike the CN/EN key.
inline constexpr const char *tray_menu_fullwidth_hint = "Ctrl + Shift + Space";
inline constexpr const char *tray_menu_punctuation_hint = "Ctrl + .";
inline constexpr const char *tray_menu_english_candidates_hint = "Ctrl + Shift + E";
inline constexpr const char *tray_menu_character_set_hint = "Ctrl + Shift + F";
inline constexpr size_t tray_menu_max_rows = 32;
inline std::vector<TrayMenuItem>
tray_menu_items(const TrayMenuCapabilities &capabilities,
                const TrayMenuState &state,
                TrayMenuPage page = TrayMenuPage::Main) {
  std::vector<TrayMenuItem> items;
  items.reserve(tray_menu_max_rows);
  auto header = [&](std::string label) {
    TrayMenuItem item;
    item.kind = TrayMenuRowKind::Header;
    item.label = std::move(label);
    item.available = false;
    items.push_back(std::move(item));
  };
  auto separator = [&] {
    TrayMenuItem item;
    item.kind = TrayMenuRowKind::Separator;
    item.available = false;
    items.push_back(std::move(item));
  };
  auto row = [&](TrayMenuCommand command, std::string label, bool available,
                 bool checked, std::string hint = {}) {
    TrayMenuItem item;
    item.command = command;
    item.label = std::move(label);
    item.hint = std::move(hint);
    item.available = available;
    item.checked = checked;
    items.push_back(std::move(item));
  };
  auto tool = [&](TrayMenuCommand command, std::string label, wchar_t icon,
                  const wchar_t *fallback, bool available, bool toggle,
                  bool checked) {
    TrayMenuItem item;
    item.kind = TrayMenuRowKind::Tool;
    item.command = command;
    item.label = std::move(label);
    item.icon = icon;
    item.icon_fallback = fallback;
    item.available = available;
    item.toggle = toggle;
    item.checked = checked;
    items.push_back(std::move(item));
  };
  // 子页：第一行回到主页，下面是这一页的单选行。
  auto back = [&](std::string label) {
    TrayMenuItem item;
    item.command = TrayMenuCommand::ShowMain;
    item.label = std::move(label);
    item.back = true;
    items.push_back(std::move(item));
  };
  if (page == TrayMenuPage::Schemes) {
    back("输入方案");
    separator();
    for (auto &item : tray_menu_scheme_items(capabilities, state))
      items.push_back(std::move(item));
    return items;
  }
  if (page == TrayMenuPage::Themes) {
    back("主题");
    separator();
    // 留出返回行、两条分隔线和「主题设置…」，目录再长也不超过卡片的行数上限。
    const size_t room = tray_menu_max_rows - 4;
    for (const auto &theme : state.themes) {
      if (items.size() - 2 >= room)
        break;
      TrayMenuItem item;
      item.command = TrayMenuCommand::SelectTheme;
      item.label = theme.title;
      item.value = theme.id;
      item.checked = theme.id == state.theme;
      items.push_back(std::move(item));
    }
    separator();
    row(TrayMenuCommand::OpenTheme, "主题设置…", capabilities.settings, false);
    return items;
  }
  const bool japanese = state.scheme == "japanese";
  const bool korean = state.scheme == "korean";
  const bool vietnamese = state.scheme == "vietnamese";
  const bool tibetan = state.scheme == "tibetan";
  const bool language_known = state.chinese.has_value();
  // In the Engine's English mode the TIP may still report Chinese, and the toolbar shows English then too.
  const bool english =
      language_known && (!*state.chinese || state.dedicated_english);
  header(capabilities.product_name);
  separator();
  // 日文、韩文、越南文和藏文是各自方案的非英文语言，和工具栏的 日、한、越、藏 按钮一致。粤拼、注音和笔画写的是中文。
  row(TrayMenuCommand::SelectChinese,
      japanese     ? "日文"
      : korean     ? "韩文"
      : vietnamese ? "越南文"
      : tibetan    ? "藏文"
                   : "中文",
      language_known, language_known && !english, state.language_hint);
  row(TrayMenuCommand::SelectEnglish, "英文", language_known, english);
  // Engine 的英文模式下「英文」一行照旧打勾（工具栏这时也显示英文），「英文候选模式」再标出是 Engine 的英文模式；TIP 在英文状态时它不勾，和 macOS 输入菜单一样。
  row(TrayMenuCommand::ToggleDedicatedEnglish, "英文候选模式",
      tray_menu_dedicated_english_action(state.chinese, state.dedicated_english,
                                         state.scheme) !=
          TrayMenuEnglishAction::Unavailable,
      language_known && *state.chinese && state.dedicated_english,
      tray_menu_english_candidates_hint);
  separator();
  // 简体输出是这个开关关着的状态，不另占一行，和 macOS 一样。
  row(TrayMenuCommand::ToggleTraditionalOutput, "繁体输出", true,
      state.traditional_output,
      state.character_set_shortcut ? tray_menu_character_set_hint : "");
  row(TrayMenuCommand::ToggleFullwidth, "全角字符",
      state.fullwidth.has_value(), state.fullwidth.value_or(false),
      tray_menu_fullwidth_hint);
  row(TrayMenuCommand::ToggleChinesePunctuation, "中文标点",
      state.chinese_punctuation.has_value() && !state.punctuation_locked,
      state.chinese_punctuation.value_or(false), tray_menu_punctuation_hint);
  row(TrayMenuCommand::ToggleTranslations, "显示译文", true,
      state.translations);
  separator();
  // 输入方案和主题各是多选一，和 macOS 的两个子菜单一样各占一行，右侧写出当前的选择，点开换到那一页。
  std::string scheme_title;
  for (const auto &item : tray_menu_scheme_items(capabilities, state))
    if (item.checked)
      scheme_title = item.label;
  row(TrayMenuCommand::ShowSchemes, "输入方案", true, false, scheme_title);
  items.back().submenu = true;
  row(TrayMenuCommand::ShowThemes, "主题", !state.themes.empty(), false,
      state.theme_title);
  items.back().submenu = true;
  separator();
  // The host tools the shipped menu offered, kept reachable as one strip so the card still fits a small work area.
  tool(TrayMenuCommand::ToggleFloatingToolbar, "工具栏", 0xE7C4, L"栏",
       capabilities.floating_toolbar, true, state.floating_toolbar);
  tool(TrayMenuCommand::OpenEmojiPanel, "表情", 0xE76E, L"表",
       capabilities.emoji_panel, false, false);
  tool(TrayMenuCommand::OpenHandwritingPanel, "手写", 0xE70F, L"写",
       capabilities.handwriting_panel, false, false);
  tool(TrayMenuCommand::OpenKeyboardPanel, "键盘", 0xE765, L"键",
       capabilities.keyboard_panel, false, false);
  tool(TrayMenuCommand::ToggleVoiceInput, "语音", 0xE720, L"音",
       capabilities.voice_input, false, false);
  separator();
  // 云剪贴板面板像表情面板的剪贴板页一样把条目输入编辑器，macOS 把它放在表情旁边；这里工具条放不下第六格，放在设置页入口之前。
  row(TrayMenuCommand::OpenCloudClipboard, "云剪贴板…",
      capabilities.cloud_clipboard, false);
  row(TrayMenuCommand::OpenDictionary, "词库…", capabilities.settings, false);
  row(TrayMenuCommand::OpenSettings, "设置…", capabilities.settings, false);
  row(TrayMenuCommand::OpenAbout, "关于" + capabilities.product_name,
      capabilities.settings, false);
  return items;
}
// Whether a row that ran closes the menu. The toolbar tile is a switch: the reference flips it in place and leaves the menu open (tray_menu_presenter.cpp:175-186). Every other row either opens a surface or changes a mode the TIP applies asynchronously, and dismisses the menu as a native menu does.
inline bool tray_menu_closes_after(TrayMenuCommand command) {
  // 翻页只换卡片的内容，不收起卡片。
  return command != TrayMenuCommand::ToggleFloatingToolbar &&
         !tray_menu_page_target(command);
}
// Geometry in DIPs, from the design's Windows menu tokens (width 260, padding 4, card radius 8, row 32 with radius 4, 14px text, 12px hints and captions, 1px separators with 4px margins).
struct TrayMenuMetrics {
  double width = 260.0;
  double padding = 4.0;
  double radius = 8.0;
  double border_width = 1.0;
  double row_height = 32.0;
  double item_radius = 4.0;
  // 18px logo inside 6px of vertical padding.
  double header_height = 30.0;
  double logo_size = 18.0;
  // 12px caption with 4px above and 2px below.
  double label_height = 22.0;
  // A 1px line with 4px either side.
  double separator_height = 9.0;
  // Glyph over a 12px caption.
  double tool_height = 52.0;
  // Horizontal padding inside a row.
  double inset = 10.0;
  double mark_column = 16.0;
  double gap = 8.0;
  double font_size = 14.0;
  double hint_font_size = 12.0;
  double icon_font_size = 16.0;
};
// Where the check mark column starts inside a row.
inline double tray_menu_mark_x(const TrayMenuMetrics &metrics) {
  return metrics.inset;
}
// Where a row's label starts: after the mark column, so labels line up whether or not a row is checked.
inline double tray_menu_label_x(const TrayMenuMetrics &metrics) {
  return metrics.inset + metrics.mark_column + metrics.gap;
}
struct TrayMenuSize {
  double width, height;
};
struct TrayMenuRect {
  double left, top, right, bottom;
};
struct TrayMenuGeometry {
  TrayMenuSize size;
  // One rectangle per item, in item order. A Tool's rectangle is its own cell of the shared strip.
  std::vector<TrayMenuRect> rows;
};
inline TrayMenuGeometry tray_menu_geometry(const std::vector<TrayMenuItem> &items,
                                           const TrayMenuMetrics &metrics) {
  if (items.empty() || items.size() > tray_menu_max_rows ||
      metrics.width <= 0.0 || metrics.padding < 0.0 ||
      metrics.width <= metrics.padding * 2.0 || metrics.row_height <= 0.0 ||
      metrics.header_height <= 0.0 || metrics.label_height <= 0.0 ||
      metrics.separator_height <= 0.0 || metrics.tool_height <= 0.0)
    throw std::invalid_argument("Invalid tray menu metrics");
  TrayMenuGeometry geometry{{metrics.width, 0.0}, {}};
  geometry.rows.reserve(items.size());
  const double left = metrics.padding;
  const double right = metrics.width - metrics.padding;
  double top = metrics.padding;
  for (size_t index = 0; index < items.size();) {
    const auto kind = items[index].kind;
    if (kind == TrayMenuRowKind::Tool) {
      size_t end = index;
      while (end < items.size() && items[end].kind == TrayMenuRowKind::Tool)
        ++end;
      const double cell = (right - left) / static_cast<double>(end - index);
      for (size_t tool = index; tool < end; ++tool) {
        const double cell_left = left + cell * static_cast<double>(tool - index);
        // The last cell ends exactly at the row edge, whatever the rounding.
        const double cell_right = tool + 1 == end ? right : cell_left + cell;
        geometry.rows.push_back(
            {cell_left, top, cell_right, top + metrics.tool_height});
      }
      top += metrics.tool_height;
      index = end;
      continue;
    }
    double height = metrics.row_height;
    switch (kind) {
    case TrayMenuRowKind::Header:
      height = metrics.header_height;
      break;
    case TrayMenuRowKind::Separator:
      height = metrics.separator_height;
      break;
    case TrayMenuRowKind::Label:
      height = metrics.label_height;
      break;
    case TrayMenuRowKind::Item:
      height = metrics.row_height;
      break;
    case TrayMenuRowKind::Tool:
      height = metrics.tool_height;
      break;
    }
    geometry.rows.push_back({left, top, right, top + height});
    top += height;
    ++index;
  }
  geometry.size.height = top + metrics.padding;
  return geometry;
}
// 在 `available_height` DIP 的工作区内绘制 `items` 用的尺寸：放得下就用设计尺寸，否则把命令行最多缩到 `compact_row_height`，让更矮的工作区里（更高的缩放、竖放的小屏）最后几行也够得着、不被裁掉。
inline constexpr double tray_menu_compact_row_height = 28.0;
inline TrayMenuMetrics tray_menu_fitted_metrics(
    const std::vector<TrayMenuItem> &items, const TrayMenuMetrics &metrics,
    double available_height) {
  const double height = tray_menu_geometry(items, metrics).size.height;
  if (height <= available_height)
    return metrics;
  const auto rows = static_cast<double>(
      std::count_if(items.begin(), items.end(), [](const TrayMenuItem &item) {
        return item.kind == TrayMenuRowKind::Item;
      }));
  if (rows == 0.0)
    return metrics;
  TrayMenuMetrics fitted = metrics;
  fitted.row_height = (std::max)(tray_menu_compact_row_height,
                                 metrics.row_height -
                                     (height - available_height) / rows);
  return fitted;
}
inline TrayMenuSize tray_menu_size(const std::vector<TrayMenuItem> &items,
                                   const TrayMenuMetrics &metrics) {
  return tray_menu_geometry(items, metrics).size;
}
// Whether a row runs a command when clicked. Headers, captions and separators never do, and neither does a disabled row.
inline bool tray_menu_actionable(const TrayMenuItem &item) {
  return item.available && (item.kind == TrayMenuRowKind::Item ||
                            item.kind == TrayMenuRowKind::Tool);
}
// ---- 键盘导航 ----
// 卡片打开时的按键，和原生菜单一样：↑↓ 在可点的行之间移动，Home/End 到第一行和最后一行，回车或空格执行高亮的行，→ 打开高亮的翻页行，← 和 Esc 在子页上回到主页，Esc 在主页上收起卡片。工具条的几格之间用 ←→ 移动。别的键和带 Ctrl、Alt、Win 的组合键收起卡片并照常交给应用；单按修饰键什么也不做。没有高亮的行时回车和空格也只收起卡片、交给应用：用户多半是接着在编辑器里打字，不该吞掉这个空格或回车。
enum class TrayMenuKey { None, Dismiss, Previous, Next, First, Last, Activate, Forward, Back, Close };
inline TrayMenuKey tray_menu_key(unsigned virtual_key, bool modified,
                                 bool highlighted) {
  switch (virtual_key) {
  // Shift、Ctrl、Alt 和左右各一的修饰键、Win 键、Caps Lock。
  case 0x10: case 0x11: case 0x12: case 0x14:
  case 0x5B: case 0x5C:
  case 0xA0: case 0xA1: case 0xA2: case 0xA3: case 0xA4: case 0xA5:
    return TrayMenuKey::None;
  default:
    break;
  }
  if (modified)
    return TrayMenuKey::Dismiss;
  switch (virtual_key) {
  case 0x26: // VK_UP
    return TrayMenuKey::Previous;
  case 0x28: // VK_DOWN
    return TrayMenuKey::Next;
  case 0x24: // VK_HOME
    return TrayMenuKey::First;
  case 0x23: // VK_END
    return TrayMenuKey::Last;
  case 0x0D: // VK_RETURN
  case 0x20: // VK_SPACE
    return highlighted ? TrayMenuKey::Activate : TrayMenuKey::Dismiss;
  case 0x27: // VK_RIGHT
    return TrayMenuKey::Forward;
  case 0x25: // VK_LEFT
    return TrayMenuKey::Back;
  case 0x1B: // VK_ESCAPE
    return TrayMenuKey::Close;
  default:
    return TrayMenuKey::Dismiss;
  }
}
// 卡片自己吞下的键：导航键不交给应用，免得同一下按键既移动了高亮又移动了编辑器里的光标。
inline bool tray_menu_key_swallowed(TrayMenuKey key) {
  return key != TrayMenuKey::None && key != TrayMenuKey::Dismiss;
}
// 从 `current` 往 `forward` 方向的下一行可点的行，首尾相接；没有高亮时往下从第一行、往上从最后一行开始。整页都不可点时为空。
inline std::optional<size_t> tray_menu_step(const std::vector<TrayMenuItem> &items,
                                            std::optional<size_t> current,
                                            bool forward) {
  const size_t count = items.size();
  if (count == 0)
    return std::nullopt;
  size_t index = current && *current < count ? *current
                 : forward                  ? count - 1
                                            : 0;
  for (size_t step = 0; step < count; ++step) {
    index = forward ? (index + 1) % count : (index + count - 1) % count;
    if (tray_menu_actionable(items[index]))
      return index;
  }
  return std::nullopt;
}
// 一个键之后的结果：高亮哪一行，以及要不要执行它、回到主页或收起卡片。
struct TrayMenuKeyResult {
  std::optional<size_t> highlight;
  bool activate = false;
  bool back = false;
  bool close = false;
};
inline TrayMenuKeyResult tray_menu_key_result(const std::vector<TrayMenuItem> &items,
                                              std::optional<size_t> current,
                                              TrayMenuKey key, bool subpage) {
  if (current && (*current >= items.size() || !tray_menu_actionable(items[*current])))
    current.reset();
  TrayMenuKeyResult result;
  result.highlight = current;
  // 工具条里同一排相邻的一格。
  auto neighbour_tool = [&](bool forward) -> std::optional<size_t> {
    if (!current || items[*current].kind != TrayMenuRowKind::Tool)
      return std::nullopt;
    for (size_t index = *current;;) {
      if (forward ? index + 1 >= items.size() : index == 0)
        return std::nullopt;
      index = forward ? index + 1 : index - 1;
      if (items[index].kind != TrayMenuRowKind::Tool)
        return std::nullopt;
      if (tray_menu_actionable(items[index]))
        return index;
    }
  };
  switch (key) {
  case TrayMenuKey::None:
    break;
  case TrayMenuKey::Dismiss:
    result.close = true;
    break;
  case TrayMenuKey::Previous:
  case TrayMenuKey::Next:
    if (auto next = tray_menu_step(items, current, key == TrayMenuKey::Next))
      result.highlight = next;
    break;
  case TrayMenuKey::First:
  case TrayMenuKey::Last:
    if (auto edge = tray_menu_step(items, std::nullopt, key == TrayMenuKey::First))
      result.highlight = edge;
    break;
  case TrayMenuKey::Activate:
    result.activate = current.has_value();
    break;
  case TrayMenuKey::Forward:
    if (current && items[*current].submenu)
      result.activate = true;
    else if (auto tool = neighbour_tool(true))
      result.highlight = tool;
    break;
  case TrayMenuKey::Back:
    if (auto tool = neighbour_tool(false))
      result.highlight = tool;
    else
      result.back = subpage;
    break;
  case TrayMenuKey::Close:
    if (subpage)
      result.back = true;
    else
      result.close = true;
    break;
  }
  return result;
}
// 用键盘翻到一页后先高亮哪一行：子页上是勾着的那一行，没有就是返回行之后的第一行可点的行；回到主页时是打开那一页的那一行。
inline std::optional<size_t> tray_menu_page_highlight(const std::vector<TrayMenuItem> &items,
                                                      TrayMenuPage from) {
  for (size_t index = 0; index < items.size(); ++index) {
    const auto &item = items[index];
    if (!tray_menu_actionable(item))
      continue;
    if (from == TrayMenuPage::Schemes && item.command == TrayMenuCommand::ShowSchemes)
      return index;
    if (from == TrayMenuPage::Themes && item.command == TrayMenuCommand::ShowThemes)
      return index;
  }
  if (from != TrayMenuPage::Main)
    return tray_menu_step(items, std::nullopt, true);
  for (size_t index = 0; index < items.size(); ++index)
    if (tray_menu_actionable(items[index]) && items[index].checked)
      return index;
  for (size_t index = 0; index < items.size(); ++index)
    if (tray_menu_actionable(items[index]) && !items[index].back)
      return index;
  return std::nullopt;
}
// A click selects the row it landed on, and never a disabled or decorative one.
inline std::optional<size_t>
tray_menu_hit(double x, double y, const std::vector<TrayMenuItem> &items,
              const TrayMenuMetrics &metrics) {
  if (items.empty())
    return std::nullopt;
  const auto geometry = tray_menu_geometry(items, metrics);
  if (x < 0.0 || y < 0.0 || x >= geometry.size.width ||
      y >= geometry.size.height)
    return std::nullopt;
  for (size_t index = 0; index < items.size(); ++index) {
    const auto &row = geometry.rows[index];
    if (x >= row.left && x < row.right && y >= row.top && y < row.bottom)
      return tray_menu_actionable(items[index]) ? std::optional<size_t>(index)
                                                : std::nullopt;
  }
  return std::nullopt;
}
// Anchor the card under the tray icon, kept inside the work area.
struct TrayMenuBounds {
  int x, y, width, height;
};
inline TrayMenuBounds tray_menu_bounds(int icon_center_x, int icon_top,
                                       int left, int top, int right,
                                       int bottom, unsigned dpi,
                                       const TrayMenuSize &size) {
  if (dpi < 48 || dpi > 960 || right <= left || bottom <= top)
    throw std::invalid_argument("Invalid tray menu placement");
  const double scale = static_cast<double>(dpi) / 96.0;
  const auto width = static_cast<int>(size.width * scale + 0.5);
  const auto height = static_cast<int>(size.height * scale + 0.5);
  const int available_width = right - left;
  const int available_height = bottom - top;
  const int placed_width = (std::min)(width, available_width);
  const int placed_height = (std::min)(height, available_height);
  // The menu opens above the icon, which sits in the tray at the bottom.
  const int desired_y = icon_top - placed_height;
  return {(std::clamp)(icon_center_x - placed_width / 2, left,
                       right - placed_width),
          (std::clamp)(desired_y, top, bottom - placed_height), placed_width,
          placed_height};
}
// 从悬浮工具栏的按钮弹出的菜单：上方放得下就贴在工具栏上沿之上，否则贴在下沿之下，横向以按钮为中心并留在工作区内。工具栏常被拖到屏幕顶端，只往上开的托盘规则会把菜单夹回工作区顶部、盖住工具栏本身。`anchor_top` 和 `anchor_bottom` 是工具栏窗口的上下沿，物理像素。
inline TrayMenuBounds toolbar_menu_bounds(int anchor_center_x, int anchor_top,
                                          int anchor_bottom, int left, int top,
                                          int right, int bottom, unsigned dpi,
                                          const TrayMenuSize &size) {
  if (anchor_bottom < anchor_top)
    throw std::invalid_argument("Invalid toolbar menu anchor");
  auto bounds = tray_menu_bounds(anchor_center_x, anchor_top, left, top, right,
                                 bottom, dpi, size);
  if (bounds.y + bounds.height > anchor_top)
    bounds.y = (std::clamp)(anchor_bottom, top, bottom - bounds.height);
  return bounds;
}
} // namespace msime::windows
