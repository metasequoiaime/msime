#include "CandidatePalette.h"
#include <cmath>
#include <stdexcept>

using namespace msime::windows;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Candidate palette validation failed");
}
bool same(CandidateColor color, float r, float g, float b, float a) {
  auto near = [](float value, float expected) {
    return std::fabs(value - expected) < 0.002f;
  };
  return near(color.r, r) && near(color.g, g) && near(color.b, b) &&
         near(color.a, a);
}
int main() {
  const CandidateColor fallback = candidate_rgb(0x112233, 0.5f);
  // Hex forms the manifests use, with and without alpha.
  require(same(parse_css_color("#ff8000", fallback), 1.0f, 128 / 255.0f, 0.0f,
               1.0f));
  require(same(parse_css_color("ff8000", fallback), 1.0f, 128 / 255.0f, 0.0f,
               1.0f));
  require(same(parse_css_color("#f80", fallback), 1.0f, 136 / 255.0f, 0.0f,
               1.0f));
  require(same(parse_css_color("#e9e8e89d", fallback), 233 / 255.0f,
               232 / 255.0f, 232 / 255.0f, 157 / 255.0f));
  require(same(parse_css_color("  #ff8000\t", fallback), 1.0f, 128 / 255.0f,
               0.0f, 1.0f));
  // #RGBA, which msime-windows skins use, doubles each digit like #RGB.
  require(same(parse_css_color("#f808", fallback), 1.0f, 136 / 255.0f, 0.0f,
               0x88 / 255.0f));
  require(same(parse_css_color("#FFF8", fallback), 1.0f, 1.0f, 1.0f,
               0x88 / 255.0f));
  // The optional form tells "no colour" apart from a colour equal to the fallback.
  require(!try_parse_css_color("white") && !try_parse_css_color("none") &&
          !try_parse_css_color("rgb(0 0 0 / 50%)"));
  require(try_parse_css_color("transparent") == CandidateColor{0.0f, 0.0f, 0.0f, 0.0f});
  require(try_parse_css_color("#112233") == candidate_rgb(0x112233));

  // Functional notation, including the optional alpha argument.
  require(same(parse_css_color("rgb(255, 128, 0)", fallback), 1.0f,
               128 / 255.0f, 0.0f, 1.0f));
  require(same(parse_css_color("rgba(255, 128, 0, 0.25)", fallback), 1.0f,
               128 / 255.0f, 0.0f, 0.25f));
  require(same(parse_css_color("rgb(255 128 0)", fallback), 1.0f, 128 / 255.0f,
               0.0f, 1.0f));

  // Keywords: transparent is a value, the rest defer to the caller's color.
  require(same(parse_css_color("transparent", fallback), 0.0f, 0.0f, 0.0f,
               0.0f));
  for (const char *keyword : {"auto", "none", "", "   "})
    require(parse_css_color(keyword, fallback) == fallback);

  // Anything this presenter cannot represent keeps the built-in color rather
  // than rendering an invisible or wrong candidate window.
  for (const char *unsupported :
       {"red", "#12345", "#gggggg", "hsl(10, 20%, 30%)", "rgb(255, 128)",
        "rgb(255, 128, 0) trailing",
        "rgb(255 128 0", "var(--accent)", "#"})
    require(parse_css_color(unsupported, fallback) == fallback);

  // 异常数字通道不能把 NaN、无穷大或越界值传给原生绘制层。
  for (const char *unsupported : {"rgb(-1, 0, 0)", "rgb(256, 0, 0)",
                                  "rgba(0, 0, 0, -0.1)",
                                  "rgba(0, 0, 0, 1.1)",
                                  "rgba(nan, 0, 0, 1)",
                                  "rgba(inf, 0, 0, 1)"})
    require(parse_css_color(unsupported, fallback) == fallback);

  // 浅深色默认值与 macOS 的实心选中样式一致。
  const CandidatePalette defaults;
  require(defaults.surface == candidate_rgb(0x28282A, 0.97f));
  require(defaults.text == candidate_rgb(0xF5F5F7));
  require(same(defaults.border, 1, 1, 1, .12f));
  require(same(defaults.hover, 1, 1, 1, .10f));
  require(defaults.number == candidate_rgb(0x98989D));
  require(defaults.accent == candidate_rgb(0x5FBF84));
  require(defaults.selected == defaults.accent);
  require(defaults.selected_text == candidate_rgb(0xFFFFFF));
  require(defaults.selected_number == candidate_rgb(0xFFFFFF, .82f));
  require(!defaults.show_selected_bar && defaults.radius == 10 &&
          defaults.border_width == 1 && defaults.item_radius == 6 &&
          defaults.container_padding == 6);
  require(defaults.menu_fill == defaults.surface);

  const auto light_defaults = candidate_light_palette();
  require(light_defaults.surface == candidate_rgb(0xFFFFFF, .97f));
  require(same(light_defaults.border, 0, 0, 0, .12f));
  require(light_defaults.text == candidate_rgb(0x1D1D1F));
  require(light_defaults.number == candidate_rgb(0x6E6E73));
  require(same(light_defaults.hover, 0, 0, 0, .06f));
  require(light_defaults.accent == candidate_rgb(0x2C7A4B));
  require(light_defaults.selected == light_defaults.accent);
  require(light_defaults.selected_text == defaults.selected_text);
  require(light_defaults.menu_fill == light_defaults.surface);
  require(light_defaults.radius == defaults.radius &&
          light_defaults.shadow_alpha == defaults.shadow_alpha &&
          light_defaults.show_selected_bar == defaults.show_selected_bar);
  require(candidate_native_palette(true).surface == defaults.surface &&
          candidate_native_palette(false).surface == light_defaults.surface);

  // A theme overrides only the slots it sets; the rest keep the native tokens.
  CandidatePaletteOverrides overrides;
  overrides.text = "#101010";
  overrides.show_selected_bar = false;
  const auto skinned = candidate_palette(overrides);
  require(same(skinned.text, 0x10 / 255.0f, 0x10 / 255.0f, 0x10 / 255.0f,
               1.0f));
  require(!skinned.show_selected_bar);
  require(skinned.surface == defaults.surface &&
          skinned.accent == defaults.accent);

  // 改强调色时推导实心选中底及可读前景色，不受悬停色影响。
  CandidatePaletteOverrides derived;
  derived.accent = "#00FF00";
  derived.hover = "#11223344";
  derived.number = "#445566";
  const auto follow = candidate_palette(derived, light_defaults);
  require(same(follow.accent, 0.0f, 1.0f, 0.0f, 1.0f));
  require(follow.selected == follow.accent &&
          follow.selected_text == candidate_rgb(0x1A1A1A) &&
          follow.selected_number == candidate_rgb(0x1A1A1A, .82f));
  require(same(follow.hover, 0x11 / 255.0f, 0x22 / 255.0f, 0x33 / 255.0f,
               0x44 / 255.0f));
  // 中等亮度的橙色和灰色也要用深字；固定亮度阈值会错误地选择白字。
  for (const auto &base : {defaults, light_defaults}) {
    for (const char *color : {"#FF8000", "#999999"}) {
      CandidatePaletteOverrides bright;
      bright.selected = color;
      const auto palette = candidate_palette(bright, base);
      require(palette.selected_text == candidate_rgb(0x1A1A1A));
      require(palette.selected_number == candidate_rgb(0x1A1A1A, .82f));
      bright.selected.reset();
      bright.accent = color;
      require(candidate_palette(bright, base).selected_text == palette.selected_text);
    }
    CandidatePaletteOverrides dark;
    dark.selected = "#003366";
    require(candidate_palette(dark, base).selected_text == candidate_rgb(0xFFFFFF));
  }
  // 主题显式设置的选中颜色始终优先。
  CandidatePaletteOverrides fixed = derived;
  fixed.selected = "#0000FF";
  fixed.selected_text = "#FFFFFF";
  fixed.selected_number = "#FFFFFFCC";
  const auto explicit_slots = candidate_palette(fixed, light_defaults);
  require(same(explicit_slots.selected, 0.0f, 0.0f, 1.0f, 1.0f));
  require(same(explicit_slots.selected_text, 1.0f, 1.0f, 1.0f, 1.0f));
  require(same(explicit_slots.selected_number, 1.0f, 1.0f, 1.0f,
               0xCC / 255.0f));

  CandidatePaletteOverrides translucent;
  translucent.selected = "#11223340";
  const auto translucent_palette = candidate_palette(translucent, light_defaults);
  require(translucent_palette.selected_text == translucent_palette.text);
  require(translucent_palette.selected_number == translucent_palette.number);
  require(candidate_palette({}).selected_text == defaults.selected_text);
  CandidatePaletteOverrides invalid_selection;
  invalid_selection.selected = "invalid";
  require(candidate_palette(invalid_selection).selected_text == defaults.selected_text);
  CandidatePaletteOverrides hover_only;
  hover_only.hover = "#FFFFFF";
  require(candidate_palette(hover_only).selected == defaults.selected);

  // Empty and unparsable slots keep the value being replaced.
  CandidatePaletteOverrides blank;
  blank.surface = "";
  blank.text = "definitely not a color";
  const auto unchanged = candidate_palette(blank);
  require(unchanged.surface == defaults.surface &&
          unchanged.text == defaults.text &&
          unchanged.menu_fill == defaults.menu_fill);

  // The menus take the theme's surface, text, hover and border; with no surface set they keep the native menu material.
  CandidatePaletteOverrides paper;
  paper.surface = "#F7F1E3";
  paper.text = "#2B2B2B";
  paper.hover = "#0000000F";
  paper.border = "#00000014";
  const auto papered = candidate_palette(paper, light_defaults);
  require(papered.menu_fill == papered.surface &&
          papered.menu_text == papered.text &&
          papered.menu_hover == papered.hover &&
          papered.menu_border == papered.border);
  require(skinned.menu_fill == defaults.menu_fill &&
          skinned.menu_text == skinned.text);

  // A fixed candidate uses macOS's fixed #379AD3 only until it becomes the selected row, where the selected text wins; a transparent selected text keeps the normal colour. The theme's accent does not change it.
  const auto normal_text = candidate_rgb(0x123456);
  require(candidate_row_text_color(defaults, normal_text, false, false) ==
          normal_text);
  require(candidate_row_text_color(defaults, normal_text, false, true) ==
          candidate_rgb(0x379AD3));
  CandidatePaletteOverrides tinted;
  tinted.accent = "#FF0000";
  require(candidate_row_text_color(candidate_palette(tinted), normal_text, false, true) ==
          candidate_rgb(0x379AD3));
  // 没有皮肤释义色时，未高亮的固定排位候选的释义是 #379AD3 的 0.62 倍透明度；高亮行和不固定的高亮行一样（选中行的释义色见下面）。
  require(candidate_row_translation_color(defaults, false, true) ==
          candidate_rgb(0x379AD3, 0.62f));
  require(candidate_row_translation_color(defaults, true, true) ==
          candidate_row_translation_color(defaults, true, false));
  require(candidate_row_text_color(defaults, normal_text, true, false) ==
          defaults.selected_text);
  CandidatePaletteOverrides keep;
  keep.selected_text = "transparent";
  const auto kept = candidate_palette(keep);
  require(candidate_row_text_color(kept, normal_text, true, true) ==
          normal_text);

  // The translation is the theme's secondary colour, which the contract fixes to number: number on a plain row, selected_number on the selected one, and number again when selected_number is transparent. It never derives from the text colour.
  CandidatePaletteOverrides numbered;
  numbered.text = "#101010";
  numbered.number = "#7A7A7A";
  numbered.selected_text = "#2C7A4B";
  numbered.selected_number = "#5FBF84";
  const auto secondary = candidate_palette(numbered);
  require(candidate_row_number_color(secondary, false) == secondary.number);
  require(candidate_row_number_color(secondary, true) ==
          secondary.selected_number);
  require(!(candidate_row_number_color(secondary, false) == secondary.text));
  require(!(candidate_row_number_color(secondary, true) ==
            secondary.selected_text));
  CandidatePaletteOverrides keep_number = numbered;
  keep_number.selected_number = "transparent";
  const auto kept_number = candidate_palette(keep_number);
  require(candidate_row_number_color(kept_number, true) == kept_number.number);
  // A secondary equal to number is the contract's default: the translation draws like the numbers. A package translation colour is drawn on every row.
  CandidatePaletteOverrides following = numbered;
  following.secondary = "#7A7A7A";
  const auto followed = candidate_palette(following);
  require(!followed.translation);
  require(candidate_row_translation_color(followed, false) == followed.number);
  // selected_number #5FBF84 on the #5FBF84 fill would vanish, so the selected row derives its gloss from the selected text.
  require(candidate_row_translation_color(followed, true) ==
          candidate_selected_gloss_color(followed.selected_text, followed.selected));
  CandidatePaletteOverrides translated = numbered;
  translated.secondary = "#9FB4E080";
  const auto own = candidate_palette(translated);
  require(own.translation && *own.translation == candidate_rgb(0x9FB4E0, 0x80 / 255.0f));
  require(candidate_row_translation_color(own, false) == *own.translation);
  require(candidate_row_translation_color(own, true) ==
          candidate_selected_gloss_color(own.selected_text, own.selected));
  require(candidate_row_number_color(own, false) == own.number);

  // 选中行的释义和读音跟着选中文字的颜色，同一行只有一种前景色，未选中行不变。默认浅色的白色 0.82 只有约 4.1:1，加到够 4.5:1 为止；默认深色的 #5FBF84 上白色到不了 4.5:1，释义取不透明的白色，和选中文字一样（约 2.3:1），不换成近黑。
  for (const auto &base : {defaults, light_defaults}) {
    const auto gloss = candidate_row_translation_color(base, true);
    require(gloss.r == base.selected_text.r && gloss.g == base.selected_text.g &&
            gloss.b == base.selected_text.b);
    require(gloss.a >= 0.82f);
    require(candidate_contrast_on(gloss, base.selected) >
            candidate_contrast_on(base.selected_number, base.selected));
    require(candidate_row_translation_color(base, false) == base.number);
    require(candidate_row_translation_color(base, false, true) ==
            candidate_rgb(0x379AD3, 0.62f));
  }
  require(candidate_contrast_on(defaults.selected_number, defaults.selected) < 4.5f &&
          candidate_contrast_on(light_defaults.selected_number, light_defaults.selected) < 4.5f);
  const auto light_gloss = candidate_row_translation_color(light_defaults, true);
  require(light_gloss.r == 1.0f && light_gloss.g == 1.0f && light_gloss.b == 1.0f &&
          light_gloss.a > 0.82f && light_gloss.a < 1.0f &&
          candidate_contrast_on(light_gloss, light_defaults.selected) >= 4.5f);
  require(candidate_row_translation_color(defaults, true) == candidate_rgb(0xFFFFFF));
  // 选中序号色本来就够 4.5:1 时照它画，不推导。
  CandidatePaletteOverrides readable_number;
  readable_number.selected = "#202020";
  readable_number.selected_number = "#E0E0E0";
  const auto readable = candidate_palette(readable_number);
  require(candidate_row_translation_color(readable, true) == readable.selected_number);
  // 皮肤包的释义色在不透明选中底色上换成从选中文字推导的颜色，和 macOS 的 DeriveSelectedForegrounds 一样：#23262F 配 #6B7180、#F2C94C 配 #B8860B 到 4.5:1；#4D6BFE 上选中文字是白色，到不了 4.5:1，取不透明的白色（约 4.3:1，和选中文字一样）。
  struct PackageCase { const char *selected, *translation; bool reaches_aa; };
  for (const auto &c : {PackageCase{"#23262F", "#6B7180", true},
                        PackageCase{"#F2C94C", "#B8860B", true},
                        PackageCase{"#4D6BFE", "#2F5BD3", false}}) {
    CandidatePaletteOverrides package;
    package.selected = c.selected;
    package.secondary = c.translation;
    const auto skinned_palette = candidate_palette(package);
    require(skinned_palette.translation.has_value());
    const auto gloss = candidate_row_translation_color(skinned_palette, true);
    require(gloss.a >= 0.82f);
    require(candidate_row_translation_color(skinned_palette, false) == *skinned_palette.translation);
    if (c.reaches_aa)
      require(candidate_contrast_on(gloss, skinned_palette.selected) >= 4.5f);
    else
      require(gloss == candidate_rgb(0xFFFFFF) &&
              candidate_contrast_on(gloss, skinned_palette.selected) > 4.2f);
  }
  // 半透明选中底色下面是卡片底色，选中行保留包的释义色；包写了 selected_translation 就照它画。
  CandidatePaletteOverrides tinted_selection;
  tinted_selection.selected = "#2F5BD329";
  tinted_selection.secondary = "#2F5BD3";
  const auto tinted_palette = candidate_palette(tinted_selection);
  require(candidate_row_translation_color(tinted_palette, true) ==
          *tinted_palette.translation);
  auto given = own;
  given.selected_translation = candidate_rgb(0xFFEE00);
  require(candidate_row_translation_color(given, true) == candidate_rgb(0xFFEE00));

  // The toolbar is the candidate palette in the toolbar's own geometry, so it follows the theme.
  for (const auto &base : {defaults, light_defaults, papered}) {
    const auto toolbar = toolbar_palette(base);
    require(toolbar.surface == base.surface && toolbar.text == base.text &&
            toolbar.hover == base.hover && toolbar.border == base.border &&
            toolbar.selected == base.selected &&
            toolbar.selected_text == base.selected_text &&
            toolbar.accent == base.accent);
    // macOS's toolbar card: radius 10, a 1 DIP hairline.
    require(toolbar.radius == 10.0f && toolbar.border_width == 1.0f);
  }

  // The tray menu draws the menu slots of the same palette.
  for (const auto &base : {defaults, light_defaults, papered}) {
    const auto tray = tray_menu_palette(base);
    require(tray.surface == base.menu_fill && tray.text == base.menu_text &&
            tray.hover == base.menu_hover && tray.border == base.menu_border &&
            tray.accent == base.accent);
    require(tray.border_width == 1.0f);
    // A dimmed row stays distinct from a live one without disappearing.
    require(tray.number != tray.text && tray.number.a > 0.0f);
    // The menu stays readable and its hover visible.
    require(tray.text != tray.surface && tray.hover != tray.surface &&
            tray.surface.a > 0.0f);
    // A switch that is off is an outline in the text colour at 60%.
    const auto off = tray_toggle_off_color(tray);
    require(off.r == tray.text.r && off.g == tray.text.g &&
            off.b == tray.text.b && std::fabs(off.a - 0.6f) < 0.002f);
    // Hints and captions are the row text at 60% of its own opacity, so they stay subordinate to the label and never outshine it.
    const auto tray_secondary = tray_menu_secondary_color(tray);
    require(tray_secondary.r == tray.text.r && tray_secondary.g == tray.text.g &&
            tray_secondary.b == tray.text.b &&
            std::fabs(tray_secondary.a - tray.text.a * 0.6f) < 0.002f &&
            tray_secondary.a > 0.0f && tray_secondary.a < tray.text.a);
  }
  // Light and dark are genuinely different menus.
  require(tray_menu_palette(defaults).surface !=
          tray_menu_palette(light_defaults).surface);

  // 开关圆点按亮度选择黑白：浅色模式的深绿用白，深色模式的浅绿用黑。
  require(candidate_on_accent(light_defaults.accent) == candidate_rgb(0xFFFFFF));
  require(candidate_on_accent(defaults.accent) == candidate_rgb(0x000000));
  require(candidate_on_accent(candidate_rgb(0xFFFFFF)) ==
          candidate_rgb(0x000000));
  require(candidate_on_accent(candidate_rgb(0x000000)) ==
          candidate_rgb(0xFFFFFF));
}
