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
// The card follows the shared design: a header with the logo and the product name, the input language, the mode switches, the scheme as a radio group, the host tools, and the pages of the settings app. Rows carry a check mark column rather than leading icons; only the tool strip is drawn as glyphs.
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
};
// Which surfaces this host can actually open. Anything absent stays visible but disabled, matching how the shipped menu never hides its entries.
struct TrayMenuCapabilities {
  bool floating_toolbar = true;
  bool emoji_panel = false;
  bool handwriting_panel = false;
  bool keyboard_panel = false;
  bool voice_input = false;
  bool settings = false;
  // The Cantonese, Zhuyin and Stroke dictionaries installed beside the resources (language-dictionaries/cantonese.db, zhuyin.db and stroke.db). Without one the Engine answers that scheme with quanpin, so its row is disabled rather than selecting a scheme that would type pinyin.
  bool cantonese = false;
  bool zhuyin = false;
  bool stroke = false;
  // 本版本提供的方案。不在其中的方案行不出现，而不是显示为不可用：那个方案在这个版本里根本不存在。full 提供全部方案。
  scheme::OfferedSchemes schemes = scheme::edition_schemes();
  // 卡片标题和「关于」行里的产品名（UTF-8），按版本取；full 是「水杉输入法」。
  std::string product_name = MSIME_EDITION_DISPLAY_NAME_UTF8;
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
};
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
// Shortcuts the TIP binds itself (KeyEventSink.cpp and the preserved keys in CompositionProcessorEngine.cpp); they are fixed, unlike the CN/EN key.
inline constexpr const char *tray_menu_fullwidth_hint = "Ctrl + Shift + Space";
inline constexpr const char *tray_menu_punctuation_hint = "Ctrl + .";
inline std::vector<TrayMenuItem>
tray_menu_items(const TrayMenuCapabilities &capabilities,
                const TrayMenuState &state) {
  std::vector<TrayMenuItem> items;
  items.reserve(31);
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
  auto caption = [&](std::string label) {
    TrayMenuItem item;
    item.kind = TrayMenuRowKind::Label;
    item.label = std::move(label);
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
  separator();
  row(TrayMenuCommand::ToggleFullwidth, "全角字符",
      state.fullwidth.has_value(), state.fullwidth.value_or(false),
      tray_menu_fullwidth_hint);
  row(TrayMenuCommand::ToggleChinesePunctuation, "中文标点",
      state.chinese_punctuation.has_value(),
      state.chinese_punctuation.value_or(false), tray_menu_punctuation_hint);
  row(TrayMenuCommand::ToggleTranslations, "显示译文", true,
      state.translations);
  separator();
  caption("输入方案");
  // 只列出本版本提供的方案。
  auto scheme_row = [&](TrayMenuCommand command, std::string label,
                        bool available, bool checked) {
    if (capabilities.schemes.offers(
            scheme::scheme_from_name(tray_menu_scheme(command))))
      row(command, std::move(label), available, checked);
  };
  scheme_row(TrayMenuCommand::SelectQuanpin, "全拼", true,
             state.scheme == "quanpin");
  scheme_row(TrayMenuCommand::SelectShuangpin,
             tray_menu_shuangpin_label(state.shuangpin_profile), true,
             state.scheme == "shuangpin");
  scheme_row(TrayMenuCommand::SelectWubi,
             tray_menu_wubi_label(state.wubi_profile), true,
             state.scheme == "wubi");
  scheme_row(TrayMenuCommand::SelectJapanese, "日文", true, japanese);
  scheme_row(TrayMenuCommand::SelectKorean, "韩文", true, korean);
  scheme_row(TrayMenuCommand::SelectCantonese, "粤拼", capabilities.cantonese,
             state.scheme == "cantonese");
  scheme_row(TrayMenuCommand::SelectZhuyin, "注音", capabilities.zhuyin,
             state.scheme == "zhuyin");
  scheme_row(TrayMenuCommand::SelectVietnamese, "越南文", true, vietnamese);
  scheme_row(TrayMenuCommand::SelectTibetan, "藏文", true, tibetan);
  scheme_row(TrayMenuCommand::SelectStroke, "笔画", capabilities.stroke,
             state.scheme == "stroke");
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
  row(TrayMenuCommand::OpenTheme, "主题", capabilities.settings, false,
      state.theme_title);
  row(TrayMenuCommand::OpenDictionary, "词库…", capabilities.settings, false);
  row(TrayMenuCommand::OpenSettings, "设置…", capabilities.settings, false);
  row(TrayMenuCommand::OpenAbout, "关于" + capabilities.product_name,
      capabilities.settings, false);
  return items;
}
// Whether a row that ran closes the menu. The toolbar tile is a switch: the reference flips it in place and leaves the menu open (tray_menu_presenter.cpp:175-186). Every other row either opens a surface or changes a mode the TIP applies asynchronously, and dismisses the menu as a native menu does.
inline bool tray_menu_closes_after(TrayMenuCommand command) {
  return command != TrayMenuCommand::ToggleFloatingToolbar;
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
inline constexpr size_t tray_menu_max_rows = 32;
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
// 在 `available_height` DIP 的工作区内绘制 `items` 用的尺寸：放得下就用设计尺寸，否则把命令行最多缩到 `compact_row_height`，让十个方案和设置页在 1080p、150% 缩放的屏幕上都够得着，最后几行不被裁掉。
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
} // namespace msime::windows
