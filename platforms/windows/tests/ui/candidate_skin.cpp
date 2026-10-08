#include "CandidateSkin.h"
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
  require(dark.surface == candidate_rgb(0x141B33) && dark.accent == candidate_rgb(0x5B9BFF));
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
