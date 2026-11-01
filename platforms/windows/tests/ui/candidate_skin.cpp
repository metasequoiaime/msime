#include "CandidateSkin.h"
#include "CandidateSkinAssets.h"
#include <chrono>
#include <fstream>
#include <stdexcept>

using namespace msime::windows;
using Json = nlohmann::json;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Candidate skin validation failed");
}
// One msime_client_skin_catalog entry. SkinSummary is serialized in camelCase, and its colours are not read here: they reach the card through msime_client_resolve_theme.
Json package(const char *id, Json min_width, Json top, Json width) {
  return Json{{"id", id},
              {"name", "Sample"},
              {"version", "1.0"},
              {"base", "system"},
              {"layouts", Json::array({"vertical", "horizontal"})},
              {"themes", Json::array({"dark", "light"})},
              {"preview", "preview.png"},
              {"decorationImage", "preview.png"},
              {"minWidthDip", std::move(min_width)},
              {"decorationTopDip", std::move(top)},
              {"decorationWidthDip", std::move(width)}};
}
int main() {
  const auto root =
      std::filesystem::temp_directory_path() /
      ("msime-candidate-skin-" +
       std::to_string(
           std::chrono::steady_clock::now().time_since_epoch().count()));
  for (const char *id : {"mascot", "wide", "hostile"}) {
    std::filesystem::create_directories(root / id);
    // A synthetic file is enough: the reader only checks that the preview exists.
    std::ofstream(root / id / "preview.png") << "synthetic";
  }
  const Json catalog{
      {"packages", Json::array({package("mascot", 320.0, 50.0, 180.0),
                                package("wide", 2400.0, 600.0, 180.0),
                                package("hostile", "320", -1.0, 0.0)})},
      {"issues", Json::array()}};

  // A package that declares artwork is drawn with it, against its own minimum width.
  require(candidate_skin_min_width(catalog, "mascot") == 320.0);
  const auto mascot = candidate_skin_decoration(catalog, "mascot", root);
  require(mascot.image == (root / "mascot" / "preview.png").wstring());
  require(mascot.top_dip == 50.0 && mascot.width_dip == 180.0);


  // 装饰词声明走完整 catalog，采用规范色；无效字段只移除文字，不影响图片。
  Json overlaid = catalog;
  const Json overlay{{"index", 0}, {"fontSizeDip", 20}, {"minFontSizeDip", 8},
      {"maxWidthDip", 0}, {"offsetXDip", -3}, {"offsetYDip", 4},
      {"alignH", "center"}, {"alignV", "bottom"}, {"resolvedColor", "#12345680"}, {"stripBadge", true}};
  overlaid["packages"][0]["decorationCandidate"] = overlay;
  const auto with_word = candidate_skin_assets(overlaid, "mascot", root).decoration;
  require(with_word.candidate && with_word.candidate->index == 0 && with_word.candidate->color == "#12345680");
  require(with_word.candidate->stripBadge && with_word.candidate->seat(3, 2) == 2);
  for (const auto &invalid : {Json{{"index", 0.5}}, Json{{"index", 101}}, Json{{"fontSizeDip", 0}},
      Json{{"minFontSizeDip", 201}}, Json{{"maxWidthDip", -1}}, Json{{"offsetXDip", 2001}},
      Json{{"alignH", "unsafe"}}, Json{{"resolvedColor", "white"}}, Json{{"resolvedColor", "#123456"}},
      Json{{"stripBadge", 1}}, Json{{"fontSizeDip", "20"}}}) {
    auto malformed = overlay;
    malformed.update(invalid);
    overlaid["packages"][0]["decorationCandidate"] = malformed;
    const auto result = candidate_skin_decoration(overlaid, "mascot", root);
    require(!result.image.empty() && !result.candidate);
  }
  auto missing_field = overlay;
  missing_field.erase("fontSizeDip");
  require(!candidate_skin_decoration_candidate(Json{{"decorationCandidate", missing_field}}));
  overlaid["packages"][0]["decorationCandidate"] = overlay;
  overlaid["packages"][0]["decorationImage"] = "missing.png";
  require(!candidate_skin_decoration(overlaid, "mascot", root).candidate);
  require(!mascot.candidate);

#ifndef _WIN32
  const auto outside = root / "outside.png";
  std::ofstream(outside) << "external";
  Json escaped = catalog;
  escaped["packages"][0]["decorationImage"] = "../outside.png";
  require(candidate_skin_decoration(escaped, "mascot", root).image.empty());

  std::filesystem::create_symlink(outside, root / "mascot" / "linked.png");
  Json linked = catalog;
  linked["packages"][0]["decorationImage"] = "linked.png";
  require(candidate_skin_decoration(linked, "mascot", root).image.empty());

  std::filesystem::create_symlink(root / "mascot", root / "linked-package");
  Json linked_package{{"packages", Json::array({package("linked-package", 320.0, 50.0, 180.0)})}};
  require(candidate_skin_decoration(linked_package, "linked-package", root).image.empty());
#endif

  // Values this card cannot use are refused rather than propagated into the geometry.
  require(candidate_skin_min_width(catalog, "wide") == 0.0);
  require(candidate_skin_decoration(catalog, "wide", root).image.empty());
  require(candidate_skin_min_width(catalog, "hostile") == 0.0);
  const auto hostile = candidate_skin_decoration(catalog, "hostile", root);
  require(hostile.image.empty() && hostile.top_dip == 0.0 &&
          hostile.width_dip == 0.0);

  // Unknown, unnamed and malformed catalogs draw no package.
  for (const auto &missing : {Json::object(), Json{{"packages", 7}},
                              Json::array(), catalog}) {
    require(candidate_skin_min_width(missing, "absent") == 0.0);
    require(candidate_skin_decoration(missing, "absent", root).image.empty());
  }
  require(candidate_skin_min_width(catalog, "") == 0.0);
  require(candidate_skin_decoration(catalog, "", root).image.empty());

  // A decoration image that is missing on disk, empty, over-long or absent is not drawn.
  Json unsafe = catalog;
  unsafe["packages"][0]["decorationImage"] = "absent.png";
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());
  unsafe["packages"][0]["decorationImage"] = "";
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());
  unsafe["packages"][0]["decorationImage"] = std::string(300, 'p');
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());
  unsafe["packages"][0]["decorationImage"] = nullptr;
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());
  unsafe["packages"][0].erase("decorationImage");
  require(candidate_skin_decoration(unsafe, "mascot", root).image.empty());

  // The decoration sits where the manifest aligns it; right is the default and anything else reads as right.
  require(mascot.align == CandidateSkinAlign::right);
  Json aligned = catalog;
  aligned["packages"][0]["decorationAlign"] = "left";
  require(candidate_skin_decoration(aligned, "mascot", root).align == CandidateSkinAlign::left);
  aligned["packages"][0]["decorationAlign"] = "center";
  require(candidate_skin_decoration(aligned, "mascot", root).align == CandidateSkinAlign::center);
  aligned["packages"][0]["decorationAlign"] = "top";
  require(candidate_skin_decoration(aligned, "mascot", root).align == CandidateSkinAlign::right);
  require(candidate_decoration_left(CandidateSkinAlign::right, 10, 210, 8, 50) == 152.0f);
  require(candidate_decoration_left(CandidateSkinAlign::left, 10, 210, 8, 50) == 18.0f);
  require(candidate_decoration_left(CandidateSkinAlign::center, 10, 210, 8, 50) == 85.0f);
  // Wider than the card: never left of the window.
  require(candidate_decoration_left(CandidateSkinAlign::right, 0, 40, 8, 100) == 0.0f);

  // The mascot rect: width at the image's aspect, bottom pad_y below the card top, over a card [10, 210] whose top is at 50 under a 40 band, pad 8 and pad_y 6.
  const auto rect_equals = [](const std::optional<CandidateSkinRect> &rect,
                              float left, float top, float right, float bottom) {
    return rect && rect->left == left && rect->top == top &&
           rect->right == right && rect->bottom == bottom;
  };
  // Fits in band + pad_y (46): drawn at its natural aspect, 60 wide and 30 tall, right aligned.
  require(rect_equals(candidate_decoration_rect(CandidateSkinAlign::right, 10, 210, 50, 8, 6, 40, 60, 120, 60),
                      142, 26, 202, 56));
  // Twice as tall as the room (92 at width 46): halved in both axes, not squashed to the band. The aligned edge and the bottom stay put.
  require(rect_equals(candidate_decoration_rect(CandidateSkinAlign::right, 10, 210, 50, 8, 6, 40, 46, 50, 100),
                      179, 10, 202, 56));
  require(rect_equals(candidate_decoration_rect(CandidateSkinAlign::left, 10, 210, 50, 8, 6, 40, 46, 50, 100),
                      18, 10, 41, 56));
  require(rect_equals(candidate_decoration_rect(CandidateSkinAlign::center, 10, 210, 50, 8, 6, 40, 46, 50, 100),
                      98.5f, 10, 121.5f, 56));
  // Exactly the room: not scaled.
  require(rect_equals(candidate_decoration_rect(CandidateSkinAlign::left, 10, 210, 50, 8, 6, 40, 46, 46, 46),
                      18, 10, 64, 56));
  // An image or a width with no size draws nothing.
  require(!candidate_decoration_rect(CandidateSkinAlign::right, 10, 210, 50, 8, 6, 40, 60, 0, 60));
  require(!candidate_decoration_rect(CandidateSkinAlign::right, 10, 210, 50, 8, 6, 40, 60, 120, 0));
  require(!candidate_decoration_rect(CandidateSkinAlign::right, 10, 210, 50, 8, 6, 40, 0, 120, 60));

  // Corner radius, background and toolbar, as a msime-skins package declares them.
  std::ofstream(root / "mascot" / "background.png") << "synthetic";
  Json styled = catalog;
  auto &entry = styled["packages"][0];
  entry["cornerRadiusDip"] = 12.0;
  entry["background"] = {{"image", "background.png"}, {"fit", "contain"}, {"opacity", 0.35}};
  entry["toolbar"] = {{"cornerRadiusDip", 6.0},
                      {"dark", {{"background", "#141B33"}, {"handle", "#5B9BFF"},
                                {"divider", "#5B9BFF47"}, {"icon", "#E3EAFF"},
                                {"hover", nullptr}}},
                      {"light", {{"background", "#F4F8FF"}}}};
  require(candidate_skin_corner_radius(styled, "mascot") == 12.0f);
  require(!candidate_skin_corner_radius(catalog, "mascot"));
  const auto background = candidate_skin_background(styled, "mascot", root);
  require(background.image == (root / "mascot" / "background.png").wstring());
  require(background.fit == CandidateSkinFit::contain && background.opacity == 0.35f);
  for (const auto &[key, value] : {std::pair<const char *, Json>{"opacity", 1.5},
                                   {"opacity", "1"}, {"image", "absent.png"}}) {
    Json broken = styled;
    broken["packages"][0]["background"][key] = value;
    require(candidate_skin_background(broken, "mascot", root).image.empty());
  }
  Json radius = styled;
  radius["packages"][0]["cornerRadiusDip"] = 40.0;
  require(!candidate_skin_corner_radius(radius, "mascot"));
  const auto toolbar = candidate_skin_toolbar(styled, "mascot");
  require(toolbar.radius == 6.0f);
  require(toolbar.dark.background == std::string("#141B33") && !toolbar.dark.hover);
  require(toolbar.light.background == std::string("#F4F8FF") && !toolbar.light.handle);
  const auto base = candidate_native_palette(true);
  const auto dark = apply_toolbar_skin(base, toolbar, true);
  // `handle` has no target: the bar has no accent drag strip, so the theme's accent stays.
  require(dark.surface == candidate_rgb(0x141B33) && dark.accent == base.accent);
  require(dark.text == candidate_rgb(0xE3EAFF) && dark.hover == base.hover);
  require(dark.divider && *dark.divider == candidate_rgb(0x5B9BFF, 0x47 / 255.0f));
  require(dark.radius == 6.0f && dark.border == base.border);
  // The other mode takes only its own colours.
  const auto light = apply_toolbar_skin(candidate_native_palette(false), toolbar, false);
  require(light.surface == candidate_rgb(0xF4F8FF) && !light.divider);
  require(light.accent == candidate_native_palette(false).accent);
  // A package without a toolbar leaves the theme's toolbar untouched.
  const auto plain = apply_toolbar_skin(base, candidate_skin_toolbar(catalog, "mascot"), true);
  require(plain.surface == base.surface && plain.radius == base.radius && !plain.divider);

  // Schema v2 exposes only the shared fixed presets and bounded durations.
  Json animated = styled;
  animated["packages"][0]["schemaVersion"] = 2;
  animated["packages"][0]["animation"] = {{"enter", "spring"},
                                             {"exit", "fade"},
                                             {"selection", "glow"},
                                             {"durationMs", 240},
                                             {"staggerMs", 18}};
  const auto motion = candidate_skin_animation(animated, "mascot");
  require(motion.enter == CandidateSkinAnimationPreset::spring &&
          motion.exit == CandidateSkinAnimationPreset::fade &&
          motion.selection == CandidateSkinAnimationPreset::glow &&
          motion.duration_ms == 240 && motion.stagger_ms == 18);
  const auto defaults = candidate_skin_animation(
      Json{{"packages", Json::array({Json{{"id", "default"}, {"schemaVersion", 2}}})}},
      "default");
  require(defaults.enter == CandidateSkinAnimationPreset::fade_scale &&
          defaults.exit == CandidateSkinAnimationPreset::fade &&
          defaults.selection == CandidateSkinAnimationPreset::spring &&
          defaults.duration_ms == 180 && defaults.stagger_ms == 14);
  animated["packages"][0]["animation"]["durationMs"] = 9999;
  require(candidate_skin_animation(animated, "mascot").duration_ms == 180);
  animated["packages"][0]["animation"]["staggerMs"] = 0;
  require(candidate_skin_animation(animated, "mascot").stagger_ms == 0);
  animated["packages"][0]["schemaVersion"] = "2";
  require(candidate_skin_animation(animated, "mascot").duration_ms == 0);
  // The v2 motion contract holds on any base: a system, a fixed-mode or a canvas base all keep the declared presets.
  animated["packages"][0]["schemaVersion"] = 2;
  animated["packages"][0]["animation"]["durationMs"] = 240;
  for (const char *base_id : {"system", "night", "canvas"}) {
    animated["packages"][0]["base"] = base_id;
    const auto based = candidate_skin_animation(animated, "mascot");
    require(based.enter == CandidateSkinAnimationPreset::spring && based.duration_ms == 240);
  }

  // Extra candidate slots: only what msime_client_resolve_theme does not carry is layered, a value that does not parse stays unset, and the user's pickers keep priority.
  const Json slots{
      {"dark",
       {{"surface", "#101010"},
        {"text", "#112233"},
        {"accent", "#202020"},
        {"translation", "#505050"},
        {"showSelectedBar", true},
        {"candidate_text", "#445566"},
        {"preedit_text", "#ff0000"},
        {"preedit_caret", "#778899"},
        {"selected_text", "#808080"},
        {"selected_number", "#909090"},
        {"selected_translation", "#0000ff"},
        {"selected_bar", "#fd8c"},
        {"preedit_background", "white"},
        {"preedit_divider", "#303030"},
        {"page_arrow_divider", "#404040"},
        {"menu", {{"background", "#111111"}, {"border", "#222222"}, {"text", "#333333"}, {"hover", "#444444"}}}}},
      {"light", {{"candidate_text", "#010101"}, {"selected_bar", nullptr}}}};
  const Json no_pickers = Json::object();
  const auto resolved = candidate_native_palette(true);
  const auto extra = apply_candidate_skin_extra_slots(resolved, slots, no_pickers, true);
  // Slots the shared resolution already layered are left as it answered.
  require(extra.surface == resolved.surface && extra.text == resolved.text &&
          extra.accent == resolved.accent && !extra.translation &&
          extra.show_selected_bar == resolved.show_selected_bar);
  require(extra.candidate_text && *extra.candidate_text == candidate_rgb(0x445566));
  require(extra.preedit_text && *extra.preedit_text == candidate_rgb(0xFF0000));
  require(extra.preedit_caret && *extra.preedit_caret == candidate_rgb(0x778899));
  require(extra.selected_text == candidate_rgb(0x808080) &&
          extra.selected_number == candidate_rgb(0x909090));
  require(extra.selected_translation && *extra.selected_translation == candidate_rgb(0x0000FF));
  require(extra.selected_bar && *extra.selected_bar == candidate_rgb(0xFFDD88, 0xCC / 255.0f));
  // `white` is no colour this host parses: the slot stays unset instead of turning black.
  require(!extra.preedit_background);
  require(extra.preedit_divider && *extra.preedit_divider == candidate_rgb(0x303030));
  require(extra.page_arrow_divider && *extra.page_arrow_divider == candidate_rgb(0x404040));
  require(extra.menu_fill == candidate_rgb(0x111111) && extra.menu_border == candidate_rgb(0x222222) &&
          extra.menu_text == candidate_rgb(0x333333) && extra.menu_hover == candidate_rgb(0x444444));
  // candidate_text colours the rows only: the toolbar, which derives from `text`, keeps it.
  require(apply_toolbar_skin(toolbar_palette(extra), CandidateSkinToolbar{}, true).text == resolved.text);
  // The selected row's translation takes selected_translation; other rows keep the number colour.
  require(candidate_row_translation_color(extra, true) == candidate_rgb(0x0000FF));
  require(candidate_row_translation_color(extra, false) == extra.number);
  // A text picker wins over the package's candidate and preedit text; the caret has no picker and is still drawn.
  auto red_text = resolved;
  red_text.text = candidate_rgb(0xFF0000);
  const auto text_picked = apply_candidate_skin_extra_slots(
      red_text, slots, Json{{"text", "#ff0000"}}, true);
  require(text_picked.text == candidate_rgb(0xFF0000) && !text_picked.candidate_text &&
          !text_picked.preedit_text && text_picked.preedit_caret);
  // A selected picker keeps the readable selected text and number the resolution derived for it.
  const auto selected_picked = apply_candidate_skin_extra_slots(
      resolved, slots, Json{{"selected", "#000000"}}, true);
  require(selected_picked.selected_text == resolved.selected_text &&
          selected_picked.selected_number == resolved.selected_number &&
          !selected_picked.selected_translation && selected_picked.selected_bar);
  // Without the package's selected_translation the selected row's gloss is derived from the selected text on the opaque fill, so the package's #0000ff no longer applies and the row keeps one foreground colour; the other rows are unchanged.
  const auto picked_gloss = candidate_row_translation_color(selected_picked, true);
  require(picked_gloss ==
          candidate_selected_gloss_color(selected_picked.selected_text, selected_picked.selected));
  require(picked_gloss.r == selected_picked.selected_text.r &&
          picked_gloss.g == selected_picked.selected_text.g &&
          picked_gloss.b == selected_picked.selected_text.b);
  require(candidate_row_translation_color(selected_picked, false) == selected_picked.number);
  // The light mode takes only its own values; null and absent slots stay unset.
  const auto light_extra = apply_candidate_skin_extra_slots(
      candidate_native_palette(false), slots, no_pickers, false);
  require(light_extra.candidate_text && *light_extra.candidate_text == candidate_rgb(0x010101));
  require(!light_extra.selected_bar && !light_extra.preedit_caret &&
          light_extra.selected_text == candidate_native_palette(false).selected_text);
  // No candidate block, a block that is not an object, an absent mode or an unknown package leave the palette as it is.
  for (const auto &absent : {Json::object(), Json(7), Json{{"light", Json::object()}}}) {
    const auto unchanged = apply_candidate_skin_extra_slots(resolved, absent, no_pickers, true);
    require(!unchanged.candidate_text && !unchanged.preedit_caret && !unchanged.selected_bar &&
            unchanged.selected_text == resolved.selected_text);
  }
  Json coloured = catalog;
  coloured["packages"][0]["candidate"] = slots;
  require(apply_candidate_skin_extra_slots(resolved, coloured, "mascot", no_pickers, true).candidate_text.has_value());
  require(!apply_candidate_skin_extra_slots(resolved, coloured, "absent", no_pickers, true).candidate_text.has_value());

  // The assets carry the package's font, its full candidate block and whether it is a canvas skin.
  coloured["packages"][0]["fontFamily"] = "Microsoft YaHei";
  const auto coloured_assets = candidate_skin_assets(coloured, "mascot", root);
  require(coloured_assets.font_family == "Microsoft YaHei" &&
          coloured_assets.candidate_colors == slots && !coloured_assets.canvas);
  coloured["packages"][0]["fontFamily"] = 42;
  require(candidate_skin_assets(coloured, "mascot", root).font_family.empty());
  require(candidate_skin_assets(catalog, "mascot", root).candidate_colors == Json::object());
  // Only the catalog's canvas flag makes a canvas skin; canvas-only fields on another base were already dropped by the catalog and do not count.
  coloured["packages"][0]["borderImage"] = {{"source", "preview.png"}};
  require(!candidate_skin_assets(coloured, "mascot", root).canvas);
  coloured["packages"][0]["schemaVersion"] = 2;
  coloured["packages"][0]["base"] = "canvas";
  coloured["packages"][0]["canvas"] = true;
  require(candidate_skin_assets(coloured, "mascot", root).canvas);

  // A skin font goes first and the user's face leads the fallback chain; without one the configured chain is used as it is.
  const std::vector<std::wstring> configured_fallback{L"Segoe UI", L"Microsoft YaHei"};
  const auto plain_chain = candidate_skin_font_chain(L"LXGW WenKai", configured_fallback, L"");
  require(plain_chain.primary == L"LXGW WenKai" && plain_chain.fallback == configured_fallback);
  const auto skin_chain = candidate_skin_font_chain(L"LXGW WenKai", configured_fallback, L"Segoe UI");
  require(skin_chain.primary == L"Segoe UI" &&
          skin_chain.fallback == std::vector<std::wstring>{L"LXGW WenKai", L"Microsoft YaHei"});
  const auto same_chain = candidate_skin_font_chain(L"LXGW WenKai", configured_fallback, L"LXGW WenKai");
  require(same_chain.primary == L"LXGW WenKai" && same_chain.fallback == configured_fallback);

  // Background placement: stretch fills, cover crops the overflow, contain letterboxes.
  const CandidateSkinRect card{0, 0, 200, 100};
  const auto stretch = candidate_background_rects(CandidateSkinFit::stretch, card, 50, 50);
  require(stretch && stretch->destination.right == 200 && stretch->source.right == 50);
  const auto cover = candidate_background_rects(CandidateSkinFit::cover, card, 100, 100);
  require(cover && cover->destination.right == 200 && cover->destination.bottom == 100);
  require(cover->source.left == 0 && cover->source.top == 25 && cover->source.bottom == 75);
  const auto contain = candidate_background_rects(CandidateSkinFit::contain, card, 100, 100);
  require(contain && contain->destination.left == 50 && contain->destination.right == 150);
  require(contain->destination.top == 0 && contain->source.right == 100);
  require(!candidate_background_rects(CandidateSkinFit::cover, card, 0, 10));
  require(!candidate_background_rects(CandidateSkinFit::cover, CandidateSkinRect{0, 0, 0, 10}, 10, 10));

  std::error_code error;
  std::filesystem::remove_all(root, error);
}
