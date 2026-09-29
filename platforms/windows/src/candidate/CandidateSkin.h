#pragma once
#include <algorithm>
#include <filesystem>
#include "CandidatePalette.h"
#include <nlohmann/json.hpp>
#include <optional>
#include <string>

namespace msime::windows {
// Reads the package assets (decoration, background, geometry and toolbar palette) from the catalog msime_client_skin_catalog returns. Its candidate colours come from msime_client_resolve_theme instead, and the assets are drawn exactly when that resolution names the package in candidate_skin. The catalog serializes SkinSummary in camelCase. The shared catalog already validated the manifest; nothing here trusts it further than the bounds below.
// The mascot a package draws above the card, if it has one.
//
// The catalog parses these and the settings preview renders them, but the live
// card never touched any of it - a mascot skin looked right in settings and
// plain while actually typing.
enum class CandidateSkinAlign { left, center, right };
struct CandidateSkinDecoration {
  // Absolute path to the image, empty when the package has none.
  std::wstring image;
  // How far the artwork rises above the card's top edge, in DIPs.
  double top_dip = 0.0;
  // Drawn width in DIPs; the height follows the image's own aspect ratio.
  double width_dip = 0.0;
  // Where along the card's top edge the artwork sits; right is where every host drew it before a manifest could say otherwise.
  CandidateSkinAlign align = CandidateSkinAlign::right;
};
enum class CandidateSkinFit { cover, contain, stretch };
// An image drawn over the card surface and under the candidates, clipped to the card outline. Empty image means none.
struct CandidateSkinBackground {
  std::wstring image;
  CandidateSkinFit fit = CandidateSkinFit::cover;
  float opacity = 1.0f;
};
// One mode of the package's floating toolbar colours: the scan normalizes them to #RRGGBB(AA), and an absent one keeps what the theme derives.
struct CandidateSkinToolbarColors {
  std::optional<std::string> background, border, handle, divider, icon, hover;
};
struct CandidateSkinToolbar {
  std::optional<float> radius;
  CandidateSkinToolbarColors dark, light;
};
namespace detail {
inline const nlohmann::json *candidate_skin_package(const nlohmann::json &catalog,
                                                    const std::string &id) {
  if (id.empty() || !catalog.is_object() || !catalog.contains("packages") ||
      !catalog.at("packages").is_array())
    return nullptr;
  for (const auto &package : catalog.at("packages"))
    if (package.is_object() && package.contains("id") &&
        package.at("id").is_string() && package.at("id") == id)
      return &package;
  return nullptr;
}
// A package-relative file as an absolute path, or empty when the value is not a plausible relative path or the file is not there. The catalog already refused a path that escapes the package directory.
inline std::wstring candidate_skin_file(const nlohmann::json &value,
                                        const std::string &id,
                                        const std::filesystem::path &root) {
  if (!value.is_string())
    return {};
  const auto relative = value.get<std::string>();
  if (relative.empty() || relative.size() > 256)
    return {};
  auto path = root / std::filesystem::u8path(id) / std::filesystem::u8path(relative);
  std::error_code error;
  if (!std::filesystem::is_regular_file(path, error))
    return {};
  return path.wstring();
}
// A DIP radius in the manifest's 0-32 range, or none.
inline std::optional<float> candidate_skin_radius(const nlohmann::json &object,
                                                  const char *key) {
  if (!object.is_object() || !object.contains(key) || !object.at(key).is_number())
    return std::nullopt;
  const auto value = object.at(key).get<double>();
  if (!(value >= 0.0) || value > 32.0)
    return std::nullopt;
  return static_cast<float>(value);
}
} // namespace detail
inline CandidateSkinDecoration
candidate_skin_decoration(const nlohmann::json &catalog, const std::string &id,
                          const std::filesystem::path &root) {
  CandidateSkinDecoration decoration;
  const auto *package = detail::candidate_skin_package(catalog, id);
  // decorationImage is set only for a decorated package: the manifest's decoration.image, or its preview.
  if (!package || !package->contains("decorationImage"))
    return decoration;
  const auto top = package->value("decorationTopDip", 0.0);
  const auto width = package->value("decorationWidthDip", 0.0);
  // Both are required: a decoration with no width would draw nothing, and
  // one that does not rise above the card is not a decoration.
  if (!(top > 0.0) || !(width > 0.0) || top > 512.0 || width > 1024.0)
    return decoration;
  auto image = detail::candidate_skin_file(package->at("decorationImage"), id, root);
  if (image.empty())
    return decoration;
  decoration.image = std::move(image);
  decoration.top_dip = top;
  decoration.width_dip = width;
  const auto align = package->value("decorationAlign", std::string("right"));
  decoration.align = align == "left"     ? CandidateSkinAlign::left
                     : align == "center" ? CandidateSkinAlign::center
                                         : CandidateSkinAlign::right;
  return decoration;
}
// The minimum card width a package asks for, in DIPs, or 0 when it asks for
// none. A mascot skin is drawn against a card of a particular width; a narrower
// card makes the decoration overhang it.
inline double candidate_skin_min_width(const nlohmann::json &catalog,
                                       const std::string &id) {
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("minWidthDip") ||
      !package->at("minWidthDip").is_number())
    return 0.0;
  const auto value = package->at("minWidthDip").get<double>();
  // The catalog validates the manifest, but a value this card cannot use is
  // still refused here rather than propagated into the geometry.
  if (!(value > 0.0) || value > 2000.0)
    return 0.0;
  return value;
}
// The card radius a package asks for, in DIPs; none keeps the theme's.
inline std::optional<float> candidate_skin_corner_radius(const nlohmann::json &catalog,
                                                         const std::string &id) {
  const auto *package = detail::candidate_skin_package(catalog, id);
  return package ? detail::candidate_skin_radius(*package, "cornerRadiusDip")
                 : std::nullopt;
}
inline CandidateSkinBackground
candidate_skin_background(const nlohmann::json &catalog, const std::string &id,
                          const std::filesystem::path &root) {
  CandidateSkinBackground background;
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("background") ||
      !package->at("background").is_object())
    return background;
  const auto &value = package->at("background");
  if (!value.contains("image") || !value.contains("opacity") ||
      !value.at("opacity").is_number())
    return background;
  const auto opacity = value.at("opacity").get<double>();
  if (!(opacity >= 0.0) || opacity > 1.0)
    return background;
  auto image = detail::candidate_skin_file(value.at("image"), id, root);
  if (image.empty())
    return background;
  const auto fit = value.value("fit", std::string("cover"));
  background.image = std::move(image);
  background.fit = fit == "contain"   ? CandidateSkinFit::contain
                   : fit == "stretch" ? CandidateSkinFit::stretch
                                      : CandidateSkinFit::cover;
  background.opacity = static_cast<float>(opacity);
  return background;
}
inline CandidateSkinToolbar candidate_skin_toolbar(const nlohmann::json &catalog,
                                                   const std::string &id) {
  CandidateSkinToolbar toolbar;
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("toolbar") || !package->at("toolbar").is_object())
    return toolbar;
  const auto &value = package->at("toolbar");
  toolbar.radius = detail::candidate_skin_radius(value, "cornerRadiusDip");
  auto colors = [&value](const char *mode) {
    CandidateSkinToolbarColors colors;
    if (!value.contains(mode) || !value.at(mode).is_object())
      return colors;
    const auto &palette = value.at(mode);
    auto read = [&palette](const char *key, std::optional<std::string> &target) {
      // The shared layer only emits #RRGGBB or #RRGGBBAA here.
      if (palette.contains(key) && palette.at(key).is_string() &&
          palette.at(key).get<std::string>().size() <= 16)
        target = palette.at(key).get<std::string>();
    };
    read("background", colors.background);
    read("border", colors.border);
    read("handle", colors.handle);
    read("divider", colors.divider);
    read("icon", colors.icon);
    read("hover", colors.hover);
    return colors;
  };
  toolbar.dark = colors("dark");
  toolbar.light = colors("light");
  return toolbar;
}
// The toolbar palette with a package's own toolbar colours and radius drawn over what the theme derives: background is the bar surface, handle the drag strip (the accent there), divider the separator, icon the glyphs, hover the hovered button.
inline CandidatePalette apply_toolbar_skin(CandidatePalette palette,
                                           const CandidateSkinToolbar &toolbar,
                                           bool dark) {
  const auto &colors = dark ? toolbar.dark : toolbar.light;
  auto apply = [](const std::optional<std::string> &value, CandidateColor &target) {
    if (value && !value->empty())
      target = parse_css_color(*value, target);
  };
  apply(colors.background, palette.surface);
  apply(colors.border, palette.border);
  apply(colors.handle, palette.accent);
  apply(colors.icon, palette.text);
  apply(colors.hover, palette.hover);
  if (colors.divider && !colors.divider->empty())
    palette.divider = parse_css_color(*colors.divider, palette.border);
  if (toolbar.radius)
    palette.radius = *toolbar.radius;
  return palette;
}
// The left edge of a decoration `width` wide over a card spanning [card_left, card_right], kept `pad` in from the side it is aligned to and never left of the window.
inline float candidate_decoration_left(CandidateSkinAlign align, float card_left,
                                       float card_right, float pad, float width) {
  const float left = align == CandidateSkinAlign::left ? card_left + pad
                     : align == CandidateSkinAlign::center
                         ? (card_left + card_right - width) / 2.0f
                         : card_right - pad - width;
  return (std::max)(0.0f, left);
}
struct CandidateSkinRect {
  float left = 0.0f, top = 0.0f, right = 0.0f, bottom = 0.0f;
};
// Where a background image of `natural` size is drawn in `card`, and which part of the image: cover crops the image's overflow to fill the card, contain letterboxes the whole image inside it, stretch scales each axis to the card.
struct CandidateSkinBackgroundRects {
  CandidateSkinRect destination, source;
};
inline std::optional<CandidateSkinBackgroundRects>
candidate_background_rects(CandidateSkinFit fit, const CandidateSkinRect &card,
                           float natural_width, float natural_height) {
  const float width = card.right - card.left, height = card.bottom - card.top;
  if (!(width > 0.0f) || !(height > 0.0f) || !(natural_width > 0.0f) ||
      !(natural_height > 0.0f))
    return std::nullopt;
  CandidateSkinBackgroundRects rects{card, {0.0f, 0.0f, natural_width, natural_height}};
  if (fit == CandidateSkinFit::stretch)
    return rects;
  const float scale_x = width / natural_width, scale_y = height / natural_height;
  if (fit == CandidateSkinFit::cover) {
    const float scale = (std::max)(scale_x, scale_y);
    const float visible_width = width / scale, visible_height = height / scale;
    rects.source.left = (natural_width - visible_width) / 2.0f;
    rects.source.top = (natural_height - visible_height) / 2.0f;
    rects.source.right = rects.source.left + visible_width;
    rects.source.bottom = rects.source.top + visible_height;
    return rects;
  }
  const float scale = (std::min)(scale_x, scale_y);
  const float drawn_width = natural_width * scale, drawn_height = natural_height * scale;
  rects.destination.left = card.left + (width - drawn_width) / 2.0f;
  rects.destination.top = card.top + (height - drawn_height) / 2.0f;
  rects.destination.right = rects.destination.left + drawn_width;
  rects.destination.bottom = rects.destination.top + drawn_height;
  return rects;
}
} // namespace msime::windows
