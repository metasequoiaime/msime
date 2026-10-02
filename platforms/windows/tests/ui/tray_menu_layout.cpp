#include <set>
#include "TrayMenuLayout.h"
#include <cmath>
#include <stdexcept>
#include <utility>

using namespace msime::windows;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Tray menu layout validation failed");
}
bool near(double value, double expected) {
  return std::fabs(value - expected) < 0.001;
}
bool rejected(void (*action)()) {
  try {
    action();
  } catch (const std::invalid_argument &) {
    return true;
  }
  return false;
}
size_t find(const std::vector<TrayMenuItem> &items, TrayMenuCommand command) {
  for (size_t index = 0; index < items.size(); ++index)
    if ((items[index].kind == TrayMenuRowKind::Item ||
         items[index].kind == TrayMenuRowKind::Tool) &&
        items[index].command == command)
      return index;
  throw std::runtime_error("Tray menu row missing");
}
int main() {
  TrayMenuCapabilities all{true, true, true, true, true, true, true, true};
  TrayMenuState state;
  state.chinese = true;
  state.fullwidth = false;
  state.chinese_punctuation = true;
  state.translations = true;
  state.scheme = "quanpin";
  state.shuangpin_profile = "xiaohe";
  state.theme_title = "水杉";
  state.language_hint = tray_menu_language_hint(true, false, true);
  const auto items = tray_menu_items(all, state);

  // The design's order: header, language, switches, the scheme group, the host tools, then the settings pages.
  {
    using K = TrayMenuRowKind;
    using C = TrayMenuCommand;
    const std::vector<std::pair<K, C>> expected{
        {K::Header, C::OpenSettings},
        {K::Separator, C::OpenSettings},
        {K::Item, C::SelectChinese},
        {K::Item, C::SelectEnglish},
        {K::Separator, C::OpenSettings},
        {K::Item, C::ToggleFullwidth},
        {K::Item, C::ToggleChinesePunctuation},
        {K::Item, C::ToggleTranslations},
        {K::Separator, C::OpenSettings},
        {K::Label, C::OpenSettings},
        {K::Item, C::SelectQuanpin},
        {K::Item, C::SelectShuangpin},
        {K::Item, C::SelectWubi},
        {K::Item, C::SelectJapanese},
        {K::Item, C::SelectKorean},
        {K::Item, C::SelectCantonese},
        {K::Item, C::SelectZhuyin},
        {K::Item, C::SelectVietnamese},
        {K::Separator, C::OpenSettings},
        {K::Tool, C::ToggleFloatingToolbar},
        {K::Tool, C::OpenEmojiPanel},
        {K::Tool, C::OpenHandwritingPanel},
        {K::Tool, C::OpenKeyboardPanel},
        {K::Tool, C::ToggleVoiceInput},
        {K::Separator, C::OpenSettings},
        {K::Item, C::OpenTheme},
        {K::Item, C::OpenDictionary},
        {K::Item, C::OpenSettings},
        {K::Item, C::OpenAbout},
    };
    require(items.size() == expected.size());
    for (size_t index = 0; index < items.size(); ++index) {
      require(items[index].kind == expected[index].first);
      if (items[index].kind == K::Item || items[index].kind == K::Tool)
        require(items[index].command == expected[index].second);
    }
    require(items[0].label == "水杉输入法");
    require(items[2].label == "中文" && items[3].label == "英文");
    require(items[5].label == "全角字符" && items[6].label == "中文标点" &&
            items[7].label == "显示译文");
    require(items[9].label == "输入方案");
    require(items[10].label == "全拼" && items[11].label == "双拼（小鹤）" &&
            items[12].label == "五笔 86" && items[13].label == "日文" &&
            items[14].label == "韩文" && items[15].label == "粤拼" &&
            items[16].label == "注音" && items[17].label == "越南文");
    require(items[25].label == "主题" && items[26].label == "词库…" &&
            items[27].label == "设置…" && items[28].label == "关于水杉输入法");
  }

  // Hints: the configured CN/EN key, the TIP's own shortcuts and the theme name.
  require(items[find(items, TrayMenuCommand::SelectChinese)].hint == "Shift");
  require(items[find(items, TrayMenuCommand::ToggleFullwidth)].hint ==
          "Ctrl + Shift + Space");
  require(items[find(items, TrayMenuCommand::ToggleChinesePunctuation)].hint ==
          "Ctrl + .");
  require(items[find(items, TrayMenuCommand::OpenTheme)].hint == "水杉");
  require(items[find(items, TrayMenuCommand::OpenSettings)].hint.empty());
  require(tray_menu_language_hint(true, true, true) == "Shift");
  require(tray_menu_language_hint(false, true, true) == "Ctrl");
  require(tray_menu_language_hint(false, false, true) == "Ctrl + Alt + Space");
  require(tray_menu_language_hint(false, false, false).empty());

  // Marks follow the reported state.
  auto checked = [](const std::vector<TrayMenuItem> &rows, TrayMenuCommand command) {
    return rows[find(rows, command)].checked;
  };
  auto available = [](const std::vector<TrayMenuItem> &rows,
                      TrayMenuCommand command) {
    return rows[find(rows, command)].available;
  };
  require(checked(items, TrayMenuCommand::SelectChinese) &&
          !checked(items, TrayMenuCommand::SelectEnglish));
  require(!checked(items, TrayMenuCommand::ToggleFullwidth) &&
          checked(items, TrayMenuCommand::ToggleChinesePunctuation) &&
          checked(items, TrayMenuCommand::ToggleTranslations));
  require(checked(items, TrayMenuCommand::SelectQuanpin) &&
          !checked(items, TrayMenuCommand::SelectShuangpin) &&
          !checked(items, TrayMenuCommand::SelectWubi) &&
          !checked(items, TrayMenuCommand::SelectJapanese) &&
          !checked(items, TrayMenuCommand::SelectKorean));
  // Exactly one scheme is marked, whichever it is.
  for (const char *scheme : {"quanpin", "shuangpin", "wubi", "japanese", "korean", "cantonese", "zhuyin",
                             "vietnamese"}) {
    auto next = state;
    next.scheme = scheme;
    const auto rows = tray_menu_items(all, next);
    int marked = 0;
    for (auto command :
         {TrayMenuCommand::SelectQuanpin, TrayMenuCommand::SelectShuangpin,
          TrayMenuCommand::SelectWubi, TrayMenuCommand::SelectJapanese,
          TrayMenuCommand::SelectKorean, TrayMenuCommand::SelectCantonese,
          TrayMenuCommand::SelectZhuyin, TrayMenuCommand::SelectVietnamese})
      marked += checked(rows, command) ? 1 : 0;
    require(marked == 1);
  }
  {
    // Japanese is the language of the Japanese scheme, as the toolbar's 日 shows.
    auto japanese = state;
    japanese.scheme = "japanese";
    const auto rows = tray_menu_items(all, japanese);
    require(rows[find(rows, TrayMenuCommand::SelectChinese)].label == "日文");
    require(checked(rows, TrayMenuCommand::SelectJapanese));
  }
  {
    // Korean is the language of the Korean scheme, as the toolbar's 한 shows.
    auto korean = state;
    korean.scheme = "korean";
    const auto rows = tray_menu_items(all, korean);
    require(rows[find(rows, TrayMenuCommand::SelectChinese)].label == "韩文");
    require(checked(rows, TrayMenuCommand::SelectKorean));
  }
  {
    // Vietnamese is a language of its own; Cantonese and Zhuyin write Chinese, so the language row stays 中文 for them.
    auto vietnamese = state;
    vietnamese.scheme = "vietnamese";
    auto rows = tray_menu_items(all, vietnamese);
    require(rows[find(rows, TrayMenuCommand::SelectChinese)].label == "越南文");
    require(checked(rows, TrayMenuCommand::SelectVietnamese));
    for (const char *chinese : {"cantonese", "zhuyin"}) {
      auto next = state;
      next.scheme = chinese;
      rows = tray_menu_items(all, next);
      require(rows[find(rows, TrayMenuCommand::SelectChinese)].label == "中文");
    }
  }
  // The shuangpin row names the stored layout, and falls back to the plain name.
  require(tray_menu_shuangpin_label("ziranma") == "双拼（自然码）");
  require(tray_menu_shuangpin_label("microsoft") == "双拼（微软）");
  require(tray_menu_shuangpin_label("shoudao") == "双拼（首道）");
  require(tray_menu_shuangpin_label("unknown") == "双拼");
  {
    // English, and the Engine's own English mode while the TIP still reports Chinese.
    auto english = state;
    english.chinese = false;
    auto rows = tray_menu_items(all, english);
    require(!checked(rows, TrayMenuCommand::SelectChinese) &&
            checked(rows, TrayMenuCommand::SelectEnglish));
    english.chinese = true;
    english.dedicated_english = true;
    rows = tray_menu_items(all, english);
    require(!checked(rows, TrayMenuCommand::SelectChinese) &&
            checked(rows, TrayMenuCommand::SelectEnglish));
  }
  {
    // Unknown modes are not assumed: the rows stay visible, unmarked and disabled.
    auto unknown = state;
    unknown.chinese.reset();
    unknown.fullwidth.reset();
    unknown.chinese_punctuation.reset();
    const auto rows = tray_menu_items(all, unknown);
    require(rows.size() == items.size());
    for (auto command :
         {TrayMenuCommand::SelectChinese, TrayMenuCommand::SelectEnglish,
          TrayMenuCommand::ToggleFullwidth,
          TrayMenuCommand::ToggleChinesePunctuation})
      require(!available(rows, command) && !checked(rows, command));
    // Stored preferences do not depend on a focused session.
    require(available(rows, TrayMenuCommand::ToggleTranslations) &&
            available(rows, TrayMenuCommand::SelectWubi));
  }

  // The tool strip keeps every entry the shipped menu offered, each with a unique glyph and a text fallback; only the toolbar is a switch, following the live state.
  {
    std::set<wchar_t> glyphs;
    size_t tools = 0;
    for (const auto &row : items) {
      if (row.kind != TrayMenuRowKind::Tool) {
        require(row.icon == 0);
        continue;
      }
      ++tools;
      require(row.icon != 0);
      require(row.icon_fallback && row.icon_fallback[0] != L'\0');
      require(glyphs.insert(row.icon).second);
      require(row.toggle == (row.command == TrayMenuCommand::ToggleFloatingToolbar));
    }
    require(tools == 5);
    require(!checked(items, TrayMenuCommand::ToggleFloatingToolbar));
    auto shown = state;
    shown.floating_toolbar = true;
    require(checked(tray_menu_items(all, shown),
                    TrayMenuCommand::ToggleFloatingToolbar));
  }

  // A host without a capability shows the row disabled rather than hiding it or accepting a click that would do nothing.
  TrayMenuCapabilities server_only;
  const auto limited = tray_menu_items(server_only, state);
  require(limited.size() == items.size());
  require(available(limited, TrayMenuCommand::ToggleFloatingToolbar));
  for (auto command :
       {TrayMenuCommand::OpenEmojiPanel, TrayMenuCommand::OpenHandwritingPanel,
        TrayMenuCommand::OpenKeyboardPanel, TrayMenuCommand::ToggleVoiceInput,
        TrayMenuCommand::OpenTheme, TrayMenuCommand::OpenDictionary,
        TrayMenuCommand::OpenSettings, TrayMenuCommand::OpenAbout})
    require(!available(limited, command));
  // Cantonese and Zhuyin need their dictionary beside the resources; without it the Engine would run another scheme, so the row is disabled. Vietnamese needs no data.
  require(!available(limited, TrayMenuCommand::SelectCantonese) &&
          !available(limited, TrayMenuCommand::SelectZhuyin) &&
          available(limited, TrayMenuCommand::SelectVietnamese));
  {
    auto cantonese_only = server_only;
    cantonese_only.cantonese = true;
    const auto rows = tray_menu_items(cantonese_only, state);
    require(available(rows, TrayMenuCommand::SelectCantonese) &&
            !available(rows, TrayMenuCommand::SelectZhuyin));
  }
  // Modes and stored switches need no shell.
  for (auto command :
       {TrayMenuCommand::SelectChinese, TrayMenuCommand::ToggleFullwidth,
        TrayMenuCommand::ToggleTranslations, TrayMenuCommand::SelectQuanpin})
    require(available(limited, command));

  // Geometry: the design's 260 wide card, 4 padding, 32 rows, one 52 strip shared by the five tools.
  const TrayMenuMetrics metrics;
  require(near(metrics.width, 260.0) && near(metrics.padding, 4.0) &&
          near(metrics.row_height, 32.0) && near(metrics.item_radius, 4.0) &&
          near(metrics.radius, 8.0));
  const auto geometry = tray_menu_geometry(items, metrics);
  require(geometry.rows.size() == items.size());
  require(near(geometry.size.width, 260.0));
  const double expected_height =
      metrics.padding * 2.0 + metrics.header_height +
      metrics.separator_height * 5.0 + metrics.row_height * 17.0 +
      metrics.label_height + metrics.tool_height;
  require(near(geometry.size.height, expected_height));
  require(near(tray_menu_size(items, metrics).height, expected_height));
  // Eight schemes make the design's card taller than a 1080p work area at 150% scaling; fitted to it, the command rows shorten and nothing is clipped. A card that fits keeps the design's metrics.
  {
    const double work = 1040.0 / 1.5;
    require(geometry.size.height > work);
    const auto fitted = tray_menu_fitted_metrics(items, metrics, work);
    require(fitted.row_height < metrics.row_height &&
            fitted.row_height >= tray_menu_compact_row_height);
    require(tray_menu_geometry(items, fitted).size.height <= work + 0.001);
    require(near(tray_menu_fitted_metrics(items, metrics, 1400.0).row_height,
                 metrics.row_height));
    // A work area too short even for compact rows stops at the compact height; the placement clamps what is left.
    require(near(tray_menu_fitted_metrics(items, metrics, 300.0).row_height,
                 tray_menu_compact_row_height));
  }
  // Rows stack without gaps, inside the padding.
  require(near(geometry.rows[0].top, metrics.padding));
  require(near(geometry.rows[0].bottom, metrics.padding + metrics.header_height));
  for (size_t index = 1; index < items.size(); ++index) {
    const auto &row = geometry.rows[index];
    const auto &previous = geometry.rows[index - 1];
    const bool same_strip = items[index].kind == TrayMenuRowKind::Tool &&
                            items[index - 1].kind == TrayMenuRowKind::Tool;
    require(near(row.top, same_strip ? previous.top : previous.bottom));
    if (same_strip)
      require(near(row.left, previous.right));
    require(row.left >= metrics.padding - 0.001 &&
            row.right <= metrics.width - metrics.padding + 0.001);
  }
  {
    const auto chinese = geometry.rows[find(items, TrayMenuCommand::SelectChinese)];
    require(near(chinese.bottom - chinese.top, 32.0));
    require(near(chinese.left, 4.0) && near(chinese.right, 256.0));
    const auto first_tool =
        geometry.rows[find(items, TrayMenuCommand::ToggleFloatingToolbar)];
    const auto last_tool =
        geometry.rows[find(items, TrayMenuCommand::ToggleVoiceInput)];
    require(near(first_tool.left, 4.0) && near(last_tool.right, 256.0));
    require(near(first_tool.right - first_tool.left, 252.0 / 5.0));
    require(near(first_tool.bottom - first_tool.top, metrics.tool_height));
    require(near(geometry.rows[1].bottom - geometry.rows[1].top,
                 metrics.separator_height));
  }
  require(rejected([] {
    (void)tray_menu_geometry(std::vector<TrayMenuItem>{}, TrayMenuMetrics{});
  }));
  require(rejected([] {
    (void)tray_menu_geometry(std::vector<TrayMenuItem>(33), TrayMenuMetrics{});
  }));
  require(rejected([] {
    TrayMenuMetrics narrow;
    narrow.width = 8.0;
    (void)tray_menu_geometry(std::vector<TrayMenuItem>(1), narrow);
  }));

  // Clicks land on the row that was drawn; decorative and disabled rows swallow theirs.
  auto hit = [&](const std::vector<TrayMenuItem> &rows, double x, double y) {
    return tray_menu_hit(x, y, rows, metrics);
  };
  auto centre = [](const TrayMenuRect &row) {
    return std::pair<double, double>{(row.left + row.right) / 2.0,
                                     (row.top + row.bottom) / 2.0};
  };
  for (size_t index = 0; index < items.size(); ++index) {
    const auto [x, y] = centre(geometry.rows[index]);
    const bool actionable = items[index].kind == TrayMenuRowKind::Item ||
                            items[index].kind == TrayMenuRowKind::Tool;
    require(hit(items, x, y) ==
            (actionable ? std::optional<size_t>(index) : std::nullopt));
    require(tray_menu_actionable(items[index]) == actionable);
  }
  require(!hit(items, 10.0, metrics.padding / 2.0));
  require(!hit(items, 10.0, geometry.size.height));
  {
    const auto chinese = geometry.rows[find(items, TrayMenuCommand::SelectChinese)];
    require(!hit(items, -1.0, chinese.top + 1.0) &&
            !hit(items, geometry.size.width, chinese.top + 1.0));
    // The side padding belongs to no row.
    require(!hit(items, 2.0, chinese.top + 1.0));
    const auto emoji = find(limited, TrayMenuCommand::OpenEmojiPanel);
    const auto [ex, ey] = centre(tray_menu_geometry(limited, metrics).rows[emoji]);
    require(!hit(limited, ex, ey));
    const auto toolbar = find(limited, TrayMenuCommand::ToggleFloatingToolbar);
    const auto [tx, ty] =
        centre(tray_menu_geometry(limited, metrics).rows[toolbar]);
    require(hit(limited, tx, ty) == std::optional<size_t>(toolbar));
  }

  // The card opens above the tray icon and stays inside the work area.
  const auto size = geometry.size;
  const auto placed = tray_menu_bounds(1700, 1040, 0, 0, 1920, 1040, 96, size);
  require(placed.width == 260 &&
          placed.height == static_cast<int>(size.height + 0.5));
  require(placed.y == 1040 - placed.height);
  require(placed.x == 1700 - placed.width / 2);
  // A corner icon cannot push the card off screen.
  const auto clamped = tray_menu_bounds(1915, 1040, 0, 0, 1920, 1040, 96, size);
  require(clamped.x == 1920 - clamped.width);
  const auto scaled = tray_menu_bounds(960, 1040, 0, 0, 1920, 1040, 192, size);
  require(scaled.width == 520);
  // A work area smaller than the card clamps instead of overflowing.
  const auto tiny = tray_menu_bounds(50, 80, 0, 0, 100, 80, 96, size);
  require(tiny.width == 100 && tiny.height == 80 && tiny.x == 0 && tiny.y == 0);
  require(rejected([] {
    (void)tray_menu_bounds(0, 0, 0, 0, 0, 0, 96, TrayMenuSize{260.0, 100.0});
  }));
  require(rejected([] {
    (void)tray_menu_bounds(0, 0, 0, 0, 100, 100, 20, TrayMenuSize{260.0, 100.0});
  }));

  // Labels start after the mark column, so they line up whether or not a row is checked.
  require(near(tray_menu_mark_x(metrics), metrics.inset));
  require(near(tray_menu_label_x(metrics),
               metrics.inset + metrics.mark_column + metrics.gap));

  // The toolbar switch flips in place and leaves the menu open, as the reference does; every other row dismisses it.
  require(!tray_menu_closes_after(TrayMenuCommand::ToggleFloatingToolbar));
  for (const auto &row : items)
    if ((row.kind == TrayMenuRowKind::Item || row.kind == TrayMenuRowKind::Tool) &&
        row.command != TrayMenuCommand::ToggleFloatingToolbar)
      require(tray_menu_closes_after(row.command));

  // A scheme row writes the value that checks it, and no other row names a scheme.
  int scheme_rows = 0;
  for (const auto &row : items) {
    if (row.kind != TrayMenuRowKind::Item)
      continue;
    const char *scheme = tray_menu_scheme(row.command);
    if (!scheme)
      continue;
    ++scheme_rows;
    TrayMenuState selected;
    selected.scheme = scheme;
    require(checked(tray_menu_items(all, selected), row.command));
  }
  require(scheme_rows == 8);
  require(!tray_menu_scheme(TrayMenuCommand::SelectChinese));
  require(!tray_menu_scheme(TrayMenuCommand::OpenSettings));
}
